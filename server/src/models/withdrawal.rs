//! Withdrawal event for a weeded physical copy (#139).
//!
//! `items.weeding_status` stays the circulation flag. Reason, destination, and
//! the staff member live on `item_withdrawals`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::{IntoParams, ToSchema};

use super::Language;

/// Closed weeding reason. Stored as snake_case; JSON is camelCase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum WithdrawalReason {
    Worn,
    Obsolete,
    NewEdition,
    Surplus,
    NotBorrowed,
    OutOfPolicy,
    LostNotReplaced,
    Other,
}

impl WithdrawalReason {
    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Worn => "worn",
            Self::Obsolete => "obsolete",
            Self::NewEdition => "new_edition",
            Self::Surplus => "surplus",
            Self::NotBorrowed => "not_borrowed",
            Self::OutOfPolicy => "out_of_policy",
            Self::LostNotReplaced => "lost_not_replaced",
            Self::Other => "other",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Option<Self> {
        Some(match value {
            "worn" => Self::Worn,
            "obsolete" => Self::Obsolete,
            "new_edition" => Self::NewEdition,
            "surplus" => Self::Surplus,
            "not_borrowed" => Self::NotBorrowed,
            "out_of_policy" => Self::OutOfPolicy,
            "lost_not_replaced" => Self::LostNotReplaced,
            "other" => Self::Other,
            _ => return None,
        })
    }

    #[must_use]
    pub fn label(self, language: Language) -> &'static str {
        let french = matches!(language, Language::French);
        match self {
            Self::Worn => {
                if french {
                    "Usé ou abîmé"
                } else {
                    "Worn or damaged"
                }
            }
            Self::Obsolete => {
                if french {
                    "Contenu obsolète"
                } else {
                    "Outdated content"
                }
            }
            Self::NewEdition => {
                if french {
                    "Nouvelle édition"
                } else {
                    "Replaced by a new edition"
                }
            }
            Self::Surplus => {
                if french {
                    "Exemplaire en surnombre"
                } else {
                    "Surplus copy"
                }
            }
            Self::NotBorrowed => {
                if french {
                    "Non emprunté depuis longtemps"
                } else {
                    "Not borrowed for a long time"
                }
            }
            Self::OutOfPolicy => {
                if french {
                    "Hors politique documentaire"
                } else {
                    "Outside the collection policy"
                }
            }
            Self::LostNotReplaced => {
                if french {
                    "Perdu, non remplacé"
                } else {
                    "Lost, not replaced"
                }
            }
            Self::Other => {
                if french {
                    "Autre"
                } else {
                    "Other"
                }
            }
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for WithdrawalReason {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for WithdrawalReason {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Self::from_db_str(&s).ok_or_else(|| format!("unknown withdrawal reason: {s}").into())
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for WithdrawalReason {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_db_str().to_string(), buf)
    }
}

/// Where the weeded copy goes. NULL is allowed only on legacy rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum WithdrawalDestination {
    Pulp,
    Donation,
    Sale,
    Deposit,
}

impl WithdrawalDestination {
    #[must_use]
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Pulp => "pulp",
            Self::Donation => "donation",
            Self::Sale => "sale",
            Self::Deposit => "deposit",
        }
    }

    #[must_use]
    pub fn from_db_str(value: &str) -> Option<Self> {
        Some(match value {
            "pulp" => Self::Pulp,
            "donation" => Self::Donation,
            "sale" => Self::Sale,
            "deposit" => Self::Deposit,
            _ => return None,
        })
    }

    #[must_use]
    pub fn label(self, language: Language) -> &'static str {
        let french = matches!(language, Language::French);
        match self {
            Self::Pulp => {
                if french {
                    "Pilon"
                } else {
                    "Pulped"
                }
            }
            Self::Donation => {
                if french {
                    "Don"
                } else {
                    "Donated"
                }
            }
            Self::Sale => {
                if french {
                    "Vente"
                } else {
                    "Sold"
                }
            }
            Self::Deposit => {
                if french {
                    "Dépôt"
                } else {
                    "Deposited"
                }
            }
        }
    }
}

impl sqlx::Type<sqlx::Postgres> for WithdrawalDestination {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for WithdrawalDestination {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s: String = sqlx::Decode::<sqlx::Postgres>::decode(value)?;
        Self::from_db_str(&s).ok_or_else(|| format!("unknown withdrawal destination: {s}").into())
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for WithdrawalDestination {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.as_db_str().to_string(), buf)
    }
}

/// Phrase used in the CSV when a legacy row has no destination.
#[must_use]
pub fn legacy_destination_label(language: Language) -> &'static str {
    if matches!(language, Language::French) {
        "non renseignée (reprise)"
    } else {
        "not specified (legacy import)"
    }
}

/// Why a withdrawal body was rejected before it reached the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WithdrawalInputError {
    DetailRequired,
    DetailTooLong,
    DestinationRequired,
}

const DETAIL_MAX_CHARS: usize = 2000;

/// Trim `reason_detail`. Required and at least 3 characters when `reason` is `other`.
pub fn normalize_reason_detail(reason: WithdrawalReason, detail: Option<&str>) -> Result<Option<String>, WithdrawalInputError> {
    let trimmed = detail.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if trimmed.as_ref().is_some_and(|s| s.chars().count() > DETAIL_MAX_CHARS) {
        return Err(WithdrawalInputError::DetailTooLong);
    }
    if reason == WithdrawalReason::Other {
        match trimmed {
            Some(text) if text.chars().count() >= 3 => Ok(Some(text)),
            _ => Err(WithdrawalInputError::DetailRequired),
        }
    } else {
        Ok(trimmed)
    }
}

/// Staff list filters. `destination = "unset"` selects legacy rows with no destination.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalQuery {
    /// Inclusive RFC3339 lower bound on `withdrawnAt`.
    pub from: Option<String>,
    /// Inclusive RFC3339 upper bound on `withdrawnAt`.
    pub to: Option<String>,
    pub site_id: Option<i64>,
    pub reason: Option<WithdrawalReason>,
    /// `pulp`, `donation`, `sale`, `deposit`, or `unset` (NULL destination).
    pub destination: Option<String>,
    pub collection_id: Option<i64>,
    /// Shelfmark substring.
    pub call_number: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub include_cancelled: Option<bool>,
    /// `true` keeps rows with `disposedAt` set. `false` keeps rows not yet disposed.
    pub disposed: Option<bool>,
    /// `french` or `english`. Export only; overrides the caller's profile language.
    pub lang: Option<String>,
}

/// Create or complete a withdrawal. `destination` is required by the service.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalWrite {
    pub reason: WithdrawalReason,
    pub reason_detail: Option<String>,
    pub destination: Option<WithdrawalDestination>,
}

/// Several copies, one reason and one destination.
#[serde_as]
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchWithdrawalRequest {
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[schema(value_type = Vec<String>)]
    pub item_ids: Vec<i64>,
    pub reason: WithdrawalReason,
    pub reason_detail: Option<String>,
    pub destination: Option<WithdrawalDestination>,
}

impl BatchWithdrawalRequest {
    #[must_use]
    pub fn write(&self) -> WithdrawalWrite {
        WithdrawalWrite {
            reason: self.reason,
            reason_detail: self.reason_detail.clone(),
            destination: self.destination,
        }
    }
}

/// Mark existing withdrawals as physically disposed. `disposedAt` defaults to now.
#[serde_as]
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchDisposeRequest {
    #[serde_as(as = "Vec<DisplayFromStr>")]
    #[schema(value_type = Vec<String>)]
    pub withdrawal_ids: Vec<i64>,
    /// RFC3339. Omit to use the server clock.
    pub disposed_at: Option<String>,
}

/// One id the batch disposal refused. The batch writes nothing when any id fails.
#[serde_as]
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalDisposeFailure {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub code: String,
    pub message: String,
}

/// Body of a rejected disposal batch. `code` is `withdrawal_dispose_rejected`.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalDisposeRejectedBody {
    pub code: String,
    pub error: String,
    pub message: String,
    pub failures: Vec<WithdrawalDisposeFailure>,
}

/// One withdrawal event, with the copy fields the staff list needs.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemWithdrawal {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub item_id: i64,
    pub withdrawn_at: DateTime<Utc>,
    pub reason: WithdrawalReason,
    pub reason_detail: Option<String>,
    pub destination: Option<WithdrawalDestination>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub agent_id: Option<i64>,
    pub agent_name: Option<String>,
    pub cancelled_at: Option<DateTime<Utc>>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub cancelled_by: Option<i64>,
    pub disposed_at: Option<DateTime<Utc>>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub disposed_by: Option<i64>,
    pub disposed_by_name: Option<String>,
    pub legacy: bool,
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub biblio_id: Option<i64>,
    pub biblio_title: Option<String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[schema(value_type = Option<String>)]
    pub site_id: Option<i64>,
    pub site_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReasonTotal {
    pub reason: WithdrawalReason,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DestinationTotal {
    pub destination: Option<WithdrawalDestination>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalList {
    pub items: Vec<ItemWithdrawal>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub totals_by_reason: Vec<ReasonTotal>,
    pub totals_by_destination: Vec<DestinationTotal>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchWithdrawalResponse {
    pub withdrawals: Vec<ItemWithdrawal>,
}

/// One export row. Staff JSON stays on [`ItemWithdrawal`]; these joins exist only for the CSV.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WithdrawalCsvSource {
    pub withdrawn_at: DateTime<Utc>,
    pub reason: WithdrawalReason,
    pub reason_detail: Option<String>,
    pub destination: Option<WithdrawalDestination>,
    pub agent_name: Option<String>,
    pub disposed_at: Option<DateTime<Utc>>,
    pub disposed_by_name: Option<String>,
    pub legacy: bool,
    pub barcode: Option<String>,
    pub call_number: Option<String>,
    pub biblio_title: Option<String>,
    pub authors: Option<String>,
    pub site_name: Option<String>,
    pub acquired_at: Option<DateTime<Utc>>,
    /// `items.price` in integer cents. Null when unknown.
    pub purchase_price: Option<crate::models::fine::Cents>,
}

/// Field separator for the single French-Excel disposal file. There is no query parameter.
const CSV_SEPARATOR: char = ';';

/// Plain French headers, in column order. The same list is used for every language.
const CSV_HEADERS: &str = "Code-barres;Titre;Auteur;Cote;Site;Date d'acquisition;Prix d'achat;Motif;Précision;Destination;Date de retrait;Retiré par;Éliminé le;Éliminé par";

/// UTF-8 CSV for the municipal disposal record.
///
/// One file: a leading BOM, `;` separators, plain French headers, and dates as
/// `DD/MM/YYYY` in `zone`. `language` localizes cell labels only. A NULL legacy
/// destination is a localized phrase. Purchase price is stored cents
/// (1250 → `12,50`, 0 → `0,00`, null → empty). Cells that Excel would treat as
/// formulas are prefixed with `'` before quoting.
#[must_use]
pub fn withdrawals_csv(rows: &[WithdrawalCsvSource], language: Language, zone: chrono_tz::Tz) -> String {
    let mut csv = String::from('\u{feff}');
    csv.push_str(CSV_HEADERS);
    csv.push('\n');
    for row in rows {
        let destination = match row.destination {
            Some(destination) => destination.label(language).to_string(),
            None if row.legacy => legacy_destination_label(language).to_string(),
            None => String::new(),
        };
        let fields = [
            row.barcode.clone().unwrap_or_default(),
            row.biblio_title.clone().unwrap_or_default(),
            row.authors.clone().unwrap_or_default(),
            row.call_number.clone().unwrap_or_default(),
            row.site_name.clone().unwrap_or_default(),
            row.acquired_at.map(|at| format_csv_date(at, zone)).unwrap_or_default(),
            format_purchase_price(row.purchase_price),
            row.reason.label(language).to_string(),
            row.reason_detail.clone().unwrap_or_default(),
            destination,
            format_csv_date(row.withdrawn_at, zone),
            row.agent_name.clone().unwrap_or_default(),
            row.disposed_at.map(|at| format_csv_date(at, zone)).unwrap_or_default(),
            row.disposed_by_name.clone().unwrap_or_default(),
        ];
        csv.push_str(&fields.map(|f| escape_csv(&f)).join(";"));
        csv.push('\n');
    }
    csv
}

/// Stored cents as `12,50`. Null is an empty cell. Zero stays `0,00`.
fn format_purchase_price(raw: Option<crate::models::fine::Cents>) -> String {
    raw.map(|amount| amount.to_csv_euro()).unwrap_or_default()
}

/// `DD/MM/YYYY` in the library zone.
fn format_csv_date(at: DateTime<Utc>, zone: chrono_tz::Tz) -> String {
    at.with_timezone(&zone).format("%d/%m/%Y").to_string()
}

/// Prefix formula-like cells, then quote when the separator, a quote, or a line break is present.
fn escape_csv(value: &str) -> String {
    let mut neutralized = String::new();
    if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        neutralized.push('\'');
    }
    neutralized.push_str(value);
    if neutralized.contains([CSV_SEPARATOR, '"', '\n', '\r']) {
        format!("\"{}\"", neutralized.replace('"', "\"\""))
    } else {
        neutralized
    }
}

#[must_use]
pub fn parse_export_language(raw: &str) -> Option<Language> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "french" | "fr" => Some(Language::French),
        "english" | "en" => Some(Language::English),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{legacy_destination_label, normalize_reason_detail, withdrawals_csv, WithdrawalCsvSource, WithdrawalDestination, WithdrawalInputError, WithdrawalReason};
    use crate::models::Language;
    use chrono::TimeZone;

    fn sample(destination: Option<WithdrawalDestination>, legacy: bool) -> WithdrawalCsvSource {
        WithdrawalCsvSource {
            withdrawn_at: chrono::Utc.with_ymd_and_hms(2026, 10, 12, 8, 0, 0).unwrap(),
            reason: WithdrawalReason::Other,
            reason_detail: Some("old note".into()),
            destination,
            agent_name: None,
            disposed_at: None,
            disposed_by_name: None,
            legacy,
            barcode: Some("BC".into()),
            call_number: None,
            biblio_title: Some("Title".into()),
            authors: None,
            site_name: None,
            acquired_at: None,
            purchase_price: None,
        }
    }

    #[test]
    fn other_requires_three_characters() {
        assert_eq!(normalize_reason_detail(WithdrawalReason::Other, Some("  ab ")), Err(WithdrawalInputError::DetailRequired));
        assert_eq!(normalize_reason_detail(WithdrawalReason::Other, Some("  abc ")).unwrap(), Some("abc".into()));
        assert_eq!(normalize_reason_detail(WithdrawalReason::Worn, Some("  ")).unwrap(), None);
    }

    fn paris() -> chrono_tz::Tz {
        chrono_tz::Europe::Paris
    }

    #[test]
    fn french_csv_renders_legacy_destination() {
        let csv = withdrawals_csv(&[sample(None, true)], Language::French, paris());
        assert!(csv.starts_with('\u{feff}'));
        let mut lines = csv.trim_start_matches('\u{feff}').lines();
        assert_eq!(lines.next(), Some(super::CSV_HEADERS));
        assert_eq!(lines.next(), Some("BC;Title;;;;;;Autre;old note;non renseignée (reprise);12/10/2026;;;"));
        assert!(!csv.contains('_'));
        assert!(!csv.contains("Withdrawn on"));
        assert!(!csv.contains("2026-10-12T"));
    }

    #[test]
    fn english_csv_renders_legacy_destination() {
        assert_eq!(legacy_destination_label(Language::English), "not specified (legacy import)");
        let csv = withdrawals_csv(&[sample(None, true)], Language::English, paris());
        assert!(csv.starts_with('\u{feff}'));
        let mut lines = csv.trim_start_matches('\u{feff}').lines();
        assert_eq!(lines.next(), Some(super::CSV_HEADERS));
        assert_eq!(lines.next(), Some("BC;Title;;;;;;Other;old note;not specified (legacy import);12/10/2026;;;"));
        assert!(!csv.contains('_'));
        assert!(!csv.contains("Withdrawn on"));
        assert!(!csv.contains("disposed_by"));
        assert!(!csv.contains("non renseignée (reprise)"));
        assert!(!csv.contains("2026-10-12T"));
    }

    #[test]
    fn csv_neutralizes_formulas_and_formats_dates_in_the_library_zone() {
        let mut row = sample(Some(WithdrawalDestination::Pulp), false);
        row.reason_detail = Some("=HYPERLINK(\"http://evil\")".into());
        row.barcode = Some("+1".into());
        row.call_number = Some("-cmd".into());
        row.biblio_title = Some("@sum".into());
        row.authors = Some("=Hugo".into());
        row.site_name = Some("\tcmd".into());
        row.agent_name = Some("\rcmd".into());
        row.disposed_by_name = Some("A;B".into());
        row.purchase_price = Some(crate::models::fine::Cents(1250));
        // 15 March 23:30 UTC is 16 March in Europe/Paris (CET, UTC+1).
        row.acquired_at = Some(chrono::Utc.with_ymd_and_hms(2026, 3, 15, 23, 30, 0).unwrap());
        // 23:30 UTC is the next civil day in Europe/Paris (UTC+2 in October).
        row.withdrawn_at = chrono::Utc.with_ymd_and_hms(2026, 10, 12, 23, 30, 0).unwrap();
        row.disposed_at = Some(chrono::Utc.with_ymd_and_hms(2026, 1, 1, 23, 30, 0).unwrap());

        let csv = withdrawals_csv(&[row], Language::French, paris());
        let line = csv.trim_start_matches('\u{feff}').lines().nth(1).unwrap();
        assert_eq!(
            line,
            "'+1;'@sum;'=Hugo;'-cmd;'\tcmd;16/03/2026;12,50;Autre;\"'=HYPERLINK(\"\"http://evil\"\")\";Pilon;13/10/2026;\"'\rcmd\";02/01/2026;\"A;B\""
        );
        assert!(!csv.contains("2026-10-12T"), "{csv}");
        assert!(!csv.contains("2026-01-01T"), "{csv}");
        assert!(!csv.contains("2026-03-15T"), "{csv}");
    }

    #[test]
    fn purchase_price_prints_cents_with_a_comma_or_stays_empty() {
        use crate::models::fine::Cents;

        let cases = [(Some(Cents(1250)), "12,50"), (Some(Cents(1500)), "15,00"), (Some(Cents(0)), "0,00"), (None, "")];
        for (raw, expected) in cases {
            let mut row = sample(Some(WithdrawalDestination::Donation), false);
            row.reason_detail = None;
            row.purchase_price = raw;
            let csv = withdrawals_csv(&[row], Language::French, paris());
            let line = csv.trim_start_matches('\u{feff}').lines().nth(1).unwrap();
            let price = line.split(';').nth(6).unwrap();
            assert_eq!(price, expected, "raw {raw:?} line {line}");
            if expected.is_empty() {
                assert_ne!(price, "0");
                assert_ne!(price, "0,00");
            }
        }
    }
}
