//! Public type model (borrower audience: child, adult, school, staff, senior)

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sqlx::FromRow;
use utoipa::ToSchema;

use super::loan::LoanSettingsRenewAt;

/// Public type from database (borrower audience category)
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicType {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub name: String,
    pub label: String,
    pub subscription_duration_days: Option<i32>,
    pub age_min: Option<i16>,
    pub age_max: Option<i16>,
    /// Subscription price in cents (e.g. 1500 = 15.00€)
    pub subscription_price: Option<i32>,
    /// Optional unpaid-fine threshold override. `None` inherits the global circulation policy.
    pub unpaid_fine_threshold: Option<rust_decimal::Decimal>,
    /// Optional max `pending`/`ready` holds. `None` inherits the global circulation policy.
    pub max_active_holds: Option<i16>,
}

impl PublicType {
    /// Seeded `child` (a person under 18) requires a linked legal guardian.
    /// `school` is a collectivity and does not.
    #[must_use]
    pub fn requires_legal_guardian(name: &str) -> bool {
        name == "child"
    }

    /// A guardian must be a major patron: not `child` and not `school`.
    #[must_use]
    pub fn can_be_legal_guardian(name: &str) -> bool {
        !matches!(name, "child" | "school")
    }
}

/// Per-media loan settings for a public type: on the default row (`media_type` IS NULL), `nb_max` caps total active loans;
/// on a medium-specific row, `nb_max` caps loans for that medium.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicTypeLoanSettings {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    #[schema(value_type = String)]
    pub public_type_id: i64,
    /// `None` = default loan rules row for this public type (`media_type` IS NULL in DB).
    pub media_type: Option<String>,
    pub duration: Option<i16>,
    pub nb_max: Option<i16>,
    pub nb_renews: Option<i16>,
    /// When set, overrides [`crate::models::loan::LoanSettings`] `renew_at` for this public type and media type.
    pub renew_at: Option<LoanSettingsRenewAt>,
}

/// Create public type request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatePublicType {
    pub name: String,
    pub label: String,
    pub subscription_duration_days: Option<i32>,
    pub age_min: Option<i16>,
    pub age_max: Option<i16>,
    pub subscription_price: Option<i32>,
    /// Optional unpaid-fine threshold override. `None` inherits the global circulation policy.
    pub unpaid_fine_threshold: Option<rust_decimal::Decimal>,
    /// Optional max `pending`/`ready` holds. `None` inherits the global circulation policy.
    pub max_active_holds: Option<i16>,
}

/// One row when replacing all loan settings for a public type (`mediaType` null or omitted = default row).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicTypeLoanSettingInput {
    /// `None` = audience-wide default row (`media_type` IS NULL in DB). On that row, `nbMax` caps total active loans.
    pub media_type: Option<String>,
    pub duration: Option<i16>,
    pub nb_max: Option<i16>,
    pub nb_renews: Option<i16>,
    /// `None` = inherit from global `loans_settings` for that media type.
    pub renew_at: Option<LoanSettingsRenewAt>,
}

/// Replaces every `public_type_loan_settings` row for this audience with the given list (full snapshot).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReplacePublicTypeLoanSettingsRequest {
    pub settings: Vec<PublicTypeLoanSettingInput>,
}

/// Update public type request.
///
/// Nullable overrides use `Option<Option<T>>`: omit the field to keep the stored
/// value, send JSON `null` to clear it (inherit the global cap / threshold).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePublicType {
    pub name: Option<String>,
    pub label: Option<String>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub subscription_duration_days: Option<Option<i32>>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub age_min: Option<Option<i16>>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub age_max: Option<Option<i16>>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub subscription_price: Option<Option<i32>>,
    /// `null` clears the override so the global unpaid-fine threshold applies.
    #[serde(default, with = "serde_with::rust::double_option")]
    pub unpaid_fine_threshold: Option<Option<rust_decimal::Decimal>>,
    /// `null` clears the override so the global active-hold cap applies.
    #[serde(default, with = "serde_with::rust::double_option")]
    pub max_active_holds: Option<Option<i16>>,
}

pub(crate) fn merge_nullable<T>(incoming: Option<Option<T>>, existing: Option<T>) -> Option<T> {
    match incoming {
        Some(value) => value,
        None => existing,
    }
}

#[cfg(test)]
mod tests {
    use super::{PublicType, UpdatePublicType};

    #[test]
    fn only_child_requires_a_legal_guardian() {
        assert!(PublicType::requires_legal_guardian("child"));
        assert!(!PublicType::requires_legal_guardian("school"));
        assert!(!PublicType::requires_legal_guardian("adult"));
        assert!(!PublicType::requires_legal_guardian("staff"));
        assert!(!PublicType::requires_legal_guardian("senior"));
    }

    #[test]
    fn child_and_school_cannot_be_a_legal_guardian() {
        assert!(!PublicType::can_be_legal_guardian("child"));
        assert!(!PublicType::can_be_legal_guardian("school"));
        assert!(PublicType::can_be_legal_guardian("adult"));
        assert!(PublicType::can_be_legal_guardian("staff"));
        assert!(PublicType::can_be_legal_guardian("senior"));
    }

    #[test]
    fn explicit_null_clears_nullable_overrides_and_omission_does_not() {
        let omitted: UpdatePublicType = serde_json::from_str("{}").expect("empty object");
        assert!(omitted.max_active_holds.is_none());
        assert!(omitted.unpaid_fine_threshold.is_none());
        assert!(omitted.subscription_duration_days.is_none());
        assert!(omitted.age_min.is_none());
        assert!(omitted.subscription_price.is_none());

        let cleared: UpdatePublicType = serde_json::from_str(
            r#"{"maxActiveHolds":null,"unpaidFineThreshold":null,"ageMax":null}"#,
        )
        .expect("nulls");
        assert_eq!(cleared.max_active_holds, Some(None));
        assert_eq!(cleared.unpaid_fine_threshold, Some(None));
        assert_eq!(cleared.age_max, Some(None));

        let set: UpdatePublicType = serde_json::from_str(r#"{"maxActiveHolds":7}"#).expect("value");
        assert_eq!(set.max_active_holds, Some(Some(7)));
    }
}
