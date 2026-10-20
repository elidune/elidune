//! Public type model (borrower audience: child, adult, school, staff, senior)

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sqlx::FromRow;
use utoipa::ToSchema;

use super::fine::{AudienceUnpaidThresholdMode, Cents, MoneyCents};
use super::loan::LoanSettingsRenewAt;
use super::serde_util::reject_null;

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
    /// Subscription price in integer cents (1500 = 15.00 EUR). Stored as BIGINT.
    /// Widened from INTEGER; never multiplied by 100.
    #[schema(value_type = Option<i64>)]
    pub subscription_price: Option<Cents>,
    /// `inherit` uses the global circulation mode. `none` never blocks on amount.
    /// `amount` blocks when the unpaid balance is strictly above `unpaidThresholdCents`.
    pub unpaid_threshold_mode: AudienceUnpaidThresholdMode,
    /// Integer cents. Present only when `unpaidThresholdMode` is `amount`.
    /// `0` blocks any positive unpaid balance.
    #[sqlx(rename = "unpaid_fine_threshold")]
    #[serde(rename = "unpaidThresholdCents", skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<i64>)]
    pub unpaid_threshold_cents: Option<MoneyCents>,
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
    /// Integer cents (1500 = 15.00 EUR).
    #[schema(value_type = Option<i64>)]
    pub subscription_price: Option<Cents>,
    /// Omitted on create defaults to `inherit`. `amount` requires `unpaidThresholdCents`.
    /// JSON `null` is rejected.
    #[serde(default, deserialize_with = "reject_null")]
    pub unpaid_threshold_mode: Option<AudienceUnpaidThresholdMode>,
    /// Required when `unpaidThresholdMode` is `amount`. Ignored for `inherit` and `none`.
    #[serde(default, deserialize_with = "reject_null")]
    pub unpaid_threshold_cents: Option<i64>,
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
/// Nullable overrides other than the unpaid-fine mode use `Option<Option<T>>`:
/// omit the field to keep the stored value, send JSON `null` to clear it.
/// `unpaidThresholdMode` has no null: omit it to leave the mode and cents unchanged.
/// JSON `null` is rejected. `amount` requires `unpaidThresholdCents` in the same body.
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
    /// Integer cents. `null` clears the price.
    #[schema(value_type = Option<i64>)]
    pub subscription_price: Option<Option<Cents>>,
    /// Omit to leave the stored mode and cents unchanged. `amount` requires cents.
    #[serde(default, deserialize_with = "reject_null")]
    pub unpaid_threshold_mode: Option<AudienceUnpaidThresholdMode>,
    /// Required in this body when mode is `amount`. Ignored when mode is omitted, `inherit`, or `none`.
    #[serde(default, deserialize_with = "reject_null")]
    pub unpaid_threshold_cents: Option<i64>,
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
        assert!(omitted.unpaid_threshold_mode.is_none());
        assert!(omitted.unpaid_threshold_cents.is_none());
        assert!(omitted.subscription_duration_days.is_none());
        assert!(omitted.age_min.is_none());
        assert!(omitted.subscription_price.is_none());

        let cleared: UpdatePublicType = serde_json::from_str(r#"{"maxActiveHolds":null,"ageMax":null}"#).expect("nulls");
        assert_eq!(cleared.max_active_holds, Some(None));
        assert!(cleared.unpaid_threshold_mode.is_none());
        assert_eq!(cleared.age_max, Some(None));

        assert!(serde_json::from_str::<UpdatePublicType>(r#"{"unpaidThresholdMode":null}"#).is_err());

        let set: UpdatePublicType = serde_json::from_str(r#"{"maxActiveHolds":7}"#).expect("value");
        assert_eq!(set.max_active_holds, Some(Some(7)));

        let amount: UpdatePublicType = serde_json::from_str(r#"{"unpaidThresholdMode":"amount","unpaidThresholdCents":450}"#).expect("amount");
        assert_eq!(amount.unpaid_threshold_mode, Some(super::AudienceUnpaidThresholdMode::Amount));
        assert_eq!(amount.unpaid_threshold_cents, Some(450));
    }
}
