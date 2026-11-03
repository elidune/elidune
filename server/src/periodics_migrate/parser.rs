//! Parse legacy periodical biblio titles into serial title, issue number, and date hints.

use chrono::NaiveDate;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

static RE_HS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)hors[\s-]*s[ée]rie|h\.\s*s\.|\bhs\b|n[°o]?\s*sp[ée]cial|\bsp[ée]cial\b",
    )
    .expect("hs regex")
});
static RE_EXPLICIT_ISSUE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bn[°o\.]?\s*(\d+)\b|\bno\.?\s*(\d+)\b").expect("issue regex"));
static RE_YEAR: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(19|20)\d{2}\b").expect("year regex"));
static RE_TRAILING_NUM: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\s+(\d{1,5})\s*$").expect("trailing num"));

static MONTHS: Lazy<HashMap<&'static str, u32>> = Lazy::new(|| {
    HashMap::from([
        ("janvier", 1),
        ("février", 2),
        ("fevrier", 2),
        ("mars", 3),
        ("avril", 4),
        ("mai", 5),
        ("juin", 6),
        ("juillet", 7),
        ("août", 8),
        ("aout", 8),
        ("septembre", 9),
        ("octobre", 10),
        ("novembre", 11),
        ("décembre", 12),
        ("decembre", 12),
    ])
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseConfidence {
    High,
    Review,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedPeriodicTitle {
    pub serial_base: String,
    pub cluster_key: String,
    pub issue_number: Option<i32>,
    pub planned_date: Option<NaiveDate>,
    pub supplement: bool,
    pub display_label: String,
}

fn fold_diacritics(text: &str) -> String {
    text.nfkd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .collect()
}

pub fn normalize_cluster_key(base: &str, extra_aliases: &HashMap<String, String>) -> String {
    let mut s = fold_diacritics(&base.to_lowercase());
    s = s.replace(['\'', '`', '´', '\u{2019}'], " ");
    s = s.replace(|c: char| !c.is_alphanumeric() && c != ' ', " ");
    s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some(alias) = extra_aliases.get(&s) {
        return alias.clone();
    }
    s
}

/// Strip issue/date suffixes and infer numbering from a legacy biblio title.
pub fn parse_periodic_title(
    title: &str,
    publication_date: Option<&str>,
    extra_aliases: &HashMap<String, String>,
) -> ParsedPeriodicTitle {
    let raw = title.trim();
    let supplement = RE_HS.is_match(raw);

    let month = find_month(raw);
    let mut year = RE_YEAR
        .find(raw)
        .and_then(|m| m.as_str().parse::<i32>().ok());
    if year.is_none() {
        if let Some(pub_date) = publication_date.filter(|s| !s.is_empty()) {
            if pub_date.len() == 4 {
                year = pub_date.parse().ok();
            }
        }
    }
    let serial_base = strip_suffixes(raw);
    let cluster_key = normalize_cluster_key(&serial_base, extra_aliases);
    let issue_number = explicit_issue(raw).or_else(|| implicit_issue(raw, month, year));

    let planned_date = match (year, month) {
        (Some(y), Some(m)) => NaiveDate::from_ymd_opt(y, m, 15),
        (Some(y), None) => NaiveDate::from_ymd_opt(y, 6, 15),
        _ => None,
    };

    ParsedPeriodicTitle {
        serial_base: serial_base.clone(),
        cluster_key,
        issue_number,
        planned_date,
        supplement,
        display_label: raw.to_string(),
    }
}

fn find_month(text: &str) -> Option<u32> {
    let lower = text.to_lowercase();
    let mut best: Option<(usize, u32)> = None;
    for (name, num) in MONTHS.iter() {
        if let Some(pos) = lower.find(name) {
            if best.map(|(at, _)| pos < at).unwrap_or(true) {
                best = Some((pos, *num));
            }
        }
    }
    best.map(|(_, num)| num)
}

fn explicit_issue(text: &str) -> Option<i32> {
    RE_EXPLICIT_ISSUE.captures(text).and_then(|cap| {
        cap.get(1)
            .or_else(|| cap.get(2))
            .and_then(|m| m.as_str().parse().ok())
    })
}

fn implicit_issue(text: &str, month: Option<u32>, year: Option<i32>) -> Option<i32> {
    let nums: Vec<i32> = Regex::new(r"\b(\d{1,5})\b")
        .expect("digits")
        .find_iter(text)
        .filter_map(|m| m.as_str().parse().ok())
        .collect();
    for n in nums.into_iter().rev() {
        if (1900..=2099).contains(&n) && (month.is_some() || year == Some(n)) {
            continue;
        }
        return Some(n);
    }
    None
}

fn strip_suffixes(text: &str) -> String {
    let mut base = text.to_string();
    for re in [
        RE_HS.clone(),
        Regex::new(&format!(
            r"(?i)\b({})\b.*$",
            MONTHS.keys().copied().collect::<Vec<_>>().join("|")
        ))
        .expect("months strip"),
        RE_YEAR.clone(),
        Regex::new(r"(?i)\bn[°o\.]?\s*\d+.*$").expect("strip n"),
        Regex::new(r"(?i)\bno\.?\s*\d+.*$").expect("strip no"),
        RE_TRAILING_NUM.clone(),
        Regex::new(r"(?i)\s+(avec\s+cd|cd).*$").expect("strip cd"),
    ] {
        base = re.replace(&base, "").to_string();
    }
    base.split_whitespace().collect::<Vec<_>>().join(" ")
        .trim_matches(|c: char| c == ',' || c == '-' || c == ' ')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_explicit_issue_and_month() {
        let aliases = HashMap::new();
        let p = parse_periodic_title("Notre Temps  591  mars 2019", None, &aliases);
        assert_eq!(p.issue_number, Some(591));
        assert_eq!(p.planned_date, NaiveDate::from_ymd_opt(2019, 3, 15));
        assert_eq!(p.cluster_key, "notre temps");
    }

    #[test]
    fn year_after_month_is_not_issue_number() {
        let aliases = HashMap::new();
        let p = parse_periodic_title("Le chasseur français février 2008", None, &aliases);
        assert_ne!(p.issue_number, Some(2008));
        assert_eq!(p.planned_date, NaiveDate::from_ymd_opt(2008, 2, 15));
    }

    #[test]
    fn alias_from_corrections_file() {
        let aliases = HashMap::from([("geo".into(), "géo".into())]);
        let a = parse_periodic_title("GEO   456   fevrier  2017", None, &aliases);
        let b = parse_periodic_title("géo 528", None, &aliases);
        assert_eq!(a.cluster_key, b.cluster_key);
    }

    #[test]
    fn detects_supplement() {
        let aliases = HashMap::new();
        let p = parse_periodic_title("Science et vie junior HS 56", None, &aliases);
        assert!(p.supplement);
    }

}
