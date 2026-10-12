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
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
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
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
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
pub fn normalize_reason_detail(
    reason: WithdrawalReason,
    detail: Option<&str>,
) -> Result<Option<String>, WithdrawalInputError> {
    let trimmed = detail
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if trimmed
        .as_ref()
        .is_some_and(|s| s.chars().count() > DETAIL_MAX_CHARS)
    {
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

/// UTF-8 CSV for the municipal disposal record. A NULL legacy destination is a localized phrase.
#[must_use]
pub fn withdrawals_csv(rows: &[ItemWithdrawal], language: Language) -> String {
    let french = matches!(language, Language::French);
    let headers = if french {
        "identifiant,exemplaire,code_barres,cote,titre,site,date,motif,precision,destination,agent,reprise"
    } else {
        "id,item_id,barcode,call_number,title,site,withdrawn_at,reason,detail,destination,agent,legacy"
    };
    let mut csv = String::from('\u{feff}');
    csv.push_str(headers);
    csv.push('\n');
    for row in rows {
        let destination = match row.destination {
            Some(destination) => destination.label(language).to_string(),
            None if row.legacy => legacy_destination_label(language).to_string(),
            None => String::new(),
        };
        let legacy = if french {
            if row.legacy {
                "oui"
            } else {
                "non"
            }
        } else if row.legacy {
            "yes"
        } else {
            "no"
        };
        let fields = [
            row.id.to_string(),
            row.item_id.to_string(),
            row.barcode.clone().unwrap_or_default(),
            row.call_number.clone().unwrap_or_default(),
            row.biblio_title.clone().unwrap_or_default(),
            row.site_name.clone().unwrap_or_default(),
            row.withdrawn_at.to_rfc3339(),
            row.reason.label(language).to_string(),
            row.reason_detail.clone().unwrap_or_default(),
            destination,
            row.agent_name.clone().unwrap_or_default(),
            legacy.to_string(),
        ];
        csv.push_str(&fields.map(|f| escape_csv(&f)).join(","));
        csv.push('\n');
    }
    csv
}

fn escape_csv(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
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
    use super::{
        legacy_destination_label, normalize_reason_detail, withdrawals_csv, ItemWithdrawal,
        WithdrawalDestination, WithdrawalInputError, WithdrawalReason,
    };
    use crate::models::Language;
    use chrono::TimeZone;

    fn sample(destination: Option<WithdrawalDestination>, legacy: bool) -> ItemWithdrawal {
        ItemWithdrawal {
            id: 1,
            item_id: 2,
            withdrawn_at: chrono::Utc.with_ymd_and_hms(2026, 10, 12, 8, 0, 0).unwrap(),
            reason: WithdrawalReason::Other,
            reason_detail: Some("old note".into()),
            destination,
            agent_id: None,
            agent_name: None,
            cancelled_at: None,
            cancelled_by: None,
            legacy,
            barcode: Some("BC".into()),
            call_number: None,
            biblio_id: Some(9),
            biblio_title: Some("Title".into()),
            site_id: None,
            site_name: None,
        }
    }

    #[test]
    fn other_requires_three_characters() {
        assert_eq!(
            normalize_reason_detail(WithdrawalReason::Other, Some("  ab ")),
            Err(WithdrawalInputError::DetailRequired)
        );
        assert_eq!(
            normalize_reason_detail(WithdrawalReason::Other, Some("  abc ")).unwrap(),
            Some("abc".into())
        );
        assert_eq!(
            normalize_reason_detail(WithdrawalReason::Worn, Some("  ")).unwrap(),
            None
        );
    }

    #[test]
    fn french_csv_renders_legacy_destination() {
        let csv = withdrawals_csv(&[sample(None, true)], Language::French);
        assert!(csv.contains("non renseignée (reprise)"));
        assert!(csv.contains("Autre"));
    }

    #[test]
    fn english_csv_renders_legacy_destination() {
        assert_eq!(
            legacy_destination_label(Language::English),
            "not specified (legacy import)"
        );
        let csv = withdrawals_csv(&[sample(None, true)], Language::English);
        assert!(csv.contains("not specified (legacy import)"));
        assert!(!csv.contains("non renseignée (reprise)"));
    }
}
