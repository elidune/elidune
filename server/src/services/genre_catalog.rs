//! In-memory genre catalog loaded from `genres` / `genre_labels` / `genre_synonyms`.
//!
//! Install via [`GenreCatalogService::reload`]; [`snapshot`] is used by the MARC translator
//! for heading match and localized 655 export without holding AppState.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use unicode_normalization::UnicodeNormalization;
use z3950_rs::marc_rs::record::{Record as MarcRecord, SubjectType};

use crate::{
    error::{AppError, AppResult},
    models::{Genre, Language, MediaType},
    repository::Repository,
};

/// Process-wide catalog snapshot (updated on reload).
static CURRENT: RwLock<Option<Arc<GenreCatalogSnapshot>>> = RwLock::new(None);

/// Fold accents and lowercase for synonym matching (same rules as migration seed).
#[must_use]
pub fn normalize_heading(s: &str) -> String {
    s.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone)]
pub struct GenreCatalogEntry {
    pub code: Genre,
    pub shelf_code: Option<String>,
    pub active: bool,
    pub sort_order: i32,
    /// language code (`fr`/`en`/`de`/`es`) → label
    pub labels: HashMap<String, String>,
    pub synonyms: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GenreCatalogSnapshot {
    pub(crate) entries: Vec<GenreCatalogEntry>,
    pub(crate) by_code: HashMap<i16, usize>,
    pub(crate) exact: HashMap<String, Genre>,
    /// Longer needles first for contains matching.
    pub(crate) contains: Vec<(String, Genre)>,
}

impl GenreCatalogSnapshot {
    #[must_use]
    pub fn entries(&self) -> &[GenreCatalogEntry] {
        &self.entries
    }

    #[must_use]
    pub fn get(&self, genre: Genre) -> Option<&GenreCatalogEntry> {
        let idx = self.by_code.get(&(genre as i16))?;
        self.entries.get(*idx)
    }

    #[must_use]
    pub fn shelf_code(&self, genre: Genre) -> Option<&str> {
        self.get(genre).and_then(|e| e.shelf_code.as_deref())
    }

    #[must_use]
    pub fn label(&self, genre: Genre, lang: Option<Language>) -> Option<&str> {
        let entry = self.get(genre)?;
        let key = language_key(lang.unwrap_or(Language::English));
        entry
            .labels
            .get(key)
            .or_else(|| entry.labels.get("en"))
            .or_else(|| entry.labels.values().next())
            .map(String::as_str)
    }

    #[must_use]
    pub fn match_heading(&self, raw: &str) -> Option<Genre> {
        let n = normalize_heading(raw);
        if n.is_empty() {
            return None;
        }
        if let Some(g) = self.exact.get(&n) {
            return Some(*g);
        }
        for (needle, genre) in &self.contains {
            if n.contains(needle.as_str()) {
                return Some(*genre);
            }
        }
        None
    }

    /// Priority: form/genre headings (655) → comics media type → Dewey class.
    #[must_use]
    pub fn resolve_genre(
        &self,
        record: &MarcRecord,
        media_type: &MediaType,
        dewey: Option<&str>,
    ) -> Option<Genre> {
        for subject in &record.indexing.subjects {
            if matches!(subject.heading_type, SubjectType::Genre) {
                if let Some(g) = self.match_heading(&subject.value) {
                    return Some(g);
                }
            }
        }
        if matches!(media_type, MediaType::Comics) {
            return Some(Genre::LitteratureComic);
        }
        if let Some(d) = dewey {
            return genre_from_dewey(d);
        }
        None
    }
}

/// Map a Dewey class number to a documentary-style [`Genre`] (hundreds). Kept in code (stable).
#[must_use]
pub fn genre_from_dewey(dewey: &str) -> Option<Genre> {
    let digits: String = dewey
        .chars()
        .filter(|c| c.is_ascii_digit())
        .take(3)
        .collect();
    if digits.is_empty() {
        return None;
    }
    let class: u16 = digits.parse().ok()?;
    let hundred = (class / 100) * 100;
    match hundred {
        0 => Some(Genre::LitteratureGeneral),
        100 => Some(Genre::LitteraturePhilosophy),
        200 => Some(Genre::LitteratureReligion),
        300 => Some(Genre::LitteratureSocialSciences),
        400 => Some(Genre::LitteratureLanguages),
        500 => Some(Genre::LitteratureSciences),
        600 => Some(Genre::LitteratureTechnical),
        700 => {
            if (796..=799).contains(&class) {
                Some(Genre::LitteratureSport)
            } else {
                Some(Genre::LitteratureArt)
            }
        }
        800 => Some(Genre::LitteratureLitterature),
        900 => {
            if (910..=919).contains(&class) {
                Some(Genre::LitteratureGeography)
            } else {
                Some(Genre::LitteratureHistory)
            }
        }
        _ => None,
    }
}

fn language_key(lang: Language) -> &'static str {
    match lang {
        Language::French => "fr",
        Language::German => "de",
        Language::Spanish => "es",
        _ => "en",
    }
}

/// Install a snapshot for MARC translator / call sites without AppState.
pub fn install_snapshot(snapshot: Arc<GenreCatalogSnapshot>) {
    if let Ok(mut guard) = CURRENT.write() {
        *guard = Some(snapshot);
    }
}

/// Current process-wide snapshot, if loaded.
#[must_use]
pub fn snapshot() -> Option<Arc<GenreCatalogSnapshot>> {
    CURRENT.read().ok().and_then(|g| g.clone())
}

/// Loads and caches the genre catalog from PostgreSQL.
#[derive(Clone)]
pub struct GenreCatalogService {
    repository: Arc<Repository>,
}

impl GenreCatalogService {
    #[must_use]
    pub fn new(repository: Arc<Repository>) -> Self {
        Self { repository }
    }

    /// Load from DB and publish as the process-wide snapshot.
    pub async fn reload(&self) -> AppResult<Arc<GenreCatalogSnapshot>> {
        let snap = Arc::new(self.load().await?);
        install_snapshot(Arc::clone(&snap));
        Ok(snap)
    }

    /// Current snapshot (loads from DB once if not yet installed).
    pub async fn current(&self) -> AppResult<Arc<GenreCatalogSnapshot>> {
        if let Some(s) = snapshot() {
            return Ok(s);
        }
        self.reload().await
    }

    async fn load(&self) -> AppResult<GenreCatalogSnapshot> {
        let pool = self.repository.pool();

        let genre_rows = sqlx::query_as::<_, (i16, Option<String>, bool, i32)>(
            r#"SELECT code, shelf_code, active, sort_order FROM genres ORDER BY sort_order, code"#,
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(format!("load genres: {e}")))?;

        let label_rows = sqlx::query_as::<_, (i16, String, String)>(
            r#"SELECT genre_code, language, label FROM genre_labels"#,
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(format!("load genre_labels: {e}")))?;

        let syn_rows = sqlx::query_as::<_, (i16, String, String, String)>(
            r#"SELECT genre_code, term, term_normalized, match_kind FROM genre_synonyms"#,
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Internal(format!("load genre_synonyms: {e}")))?;

        let mut labels_by_code: HashMap<i16, HashMap<String, String>> = HashMap::new();
        for (code, lang, label) in label_rows {
            labels_by_code.entry(code).or_default().insert(lang, label);
        }

        let mut synonyms_by_code: HashMap<i16, Vec<String>> = HashMap::new();
        let mut exact: HashMap<String, Genre> = HashMap::new();
        let mut contains: Vec<(String, Genre)> = Vec::new();

        for (code, term, term_normalized, match_kind) in syn_rows {
            let genre = Genre::from(code);
            synonyms_by_code.entry(code).or_default().push(term);
            match match_kind.as_str() {
                "contains" => contains.push((term_normalized, genre)),
                _ => {
                    exact.insert(term_normalized, genre);
                }
            }
        }
        contains.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        let mut entries = Vec::with_capacity(genre_rows.len());
        let mut by_code = HashMap::new();
        for (code, shelf_code, active, sort_order) in genre_rows {
            let genre = Genre::from(code);
            let idx = entries.len();
            by_code.insert(code, idx);
            entries.push(GenreCatalogEntry {
                code: genre,
                shelf_code,
                active,
                sort_order,
                labels: labels_by_code.remove(&code).unwrap_or_default(),
                synonyms: synonyms_by_code.remove(&code).unwrap_or_default(),
            });
        }

        Ok(GenreCatalogSnapshot {
            entries,
            by_code,
            exact,
            contains,
        })
    }
}

/// API row for `GET /genres`.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenreApiRow {
    /// Enum camelCase value (e.g. `litteratureFiction`).
    pub code: Genre,
    pub shelf_code: Option<String>,
    pub labels: HashMap<String, String>,
    pub synonyms: Vec<String>,
    pub sort_order: i32,
}

impl GenreCatalogSnapshot {
    #[must_use]
    pub fn api_rows(&self) -> Vec<GenreApiRow> {
        self.entries
            .iter()
            .filter(|e| e.active && !matches!(e.code, Genre::Unknown))
            .map(|e| GenreApiRow {
                code: e.code,
                shelf_code: e.shelf_code.clone(),
                labels: e.labels.clone(),
                synonyms: e.synonyms.clone(),
                sort_order: e.sort_order,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_snapshot() -> GenreCatalogSnapshot {
        let mut exact = HashMap::new();
        exact.insert("romans".into(), Genre::LitteratureFiction);
        exact.insert("fiction".into(), Genre::LitteratureFiction);
        exact.insert("fiktion".into(), Genre::LitteratureFiction);
        exact.insert("ficcion".into(), Genre::LitteratureFiction);
        let mut contains: Vec<(String, Genre)> = vec![("roman".into(), Genre::LitteratureFiction)];
        contains.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        let mut labels = HashMap::new();
        labels.insert("fr".into(), "Romans".into());
        labels.insert("en".into(), "Fiction".into());
        labels.insert("de".into(), "Fiktion".into());
        labels.insert("es".into(), "Ficción".into());

        let entry = GenreCatalogEntry {
            code: Genre::LitteratureFiction,
            shelf_code: Some("ROM".into()),
            active: true,
            sort_order: 20,
            labels,
            synonyms: vec!["Romans".into(), "Fiction".into()],
        };
        let mut by_code = HashMap::new();
        by_code.insert(Genre::LitteratureFiction as i16, 0);
        GenreCatalogSnapshot {
            entries: vec![entry],
            by_code,
            exact,
            contains,
        }
    }

    #[test]
    fn match_multilingual_headings() {
        let snap = fixture_snapshot();
        assert_eq!(
            snap.match_heading("Romans"),
            Some(Genre::LitteratureFiction)
        );
        assert_eq!(
            snap.match_heading("Fiction"),
            Some(Genre::LitteratureFiction)
        );
        assert_eq!(
            snap.match_heading("Fiktion"),
            Some(Genre::LitteratureFiction)
        );
        assert_eq!(
            snap.match_heading("Ficción"),
            Some(Genre::LitteratureFiction)
        );
        assert_eq!(
            snap.match_heading("Romans policiers"),
            Some(Genre::LitteratureFiction)
        );
    }

    #[test]
    fn shelf_and_label() {
        let snap = fixture_snapshot();
        assert_eq!(snap.shelf_code(Genre::LitteratureFiction), Some("ROM"));
        assert_eq!(
            snap.label(Genre::LitteratureFiction, Some(Language::French)),
            Some("Romans")
        );
        assert_eq!(
            snap.label(Genre::LitteratureFiction, Some(Language::English)),
            Some("Fiction")
        );
        assert_eq!(snap.label(Genre::LitteratureFiction, None), Some("Fiction"));
    }

    #[test]
    fn dewey_mapping() {
        assert_eq!(
            genre_from_dewey("843.914"),
            Some(Genre::LitteratureLitterature)
        );
        assert_eq!(genre_from_dewey("796.3"), Some(Genre::LitteratureSport));
        assert_eq!(genre_from_dewey("914.4"), Some(Genre::LitteratureGeography));
    }
}
