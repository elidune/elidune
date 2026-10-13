//! Staff routes that report a lost library card and replace it.

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use utoipa::ToSchema;
use validator::Validate;

use crate::models::user::User;

/// Why the desk marked the library card lost or stolen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CardLostReason {
    Lost,
    Stolen,
    Other,
}

impl CardLostReason {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lost => "lost",
            Self::Stolen => "stolen",
            Self::Other => "other",
        }
    }
}

/// `POST /users/{id}/card-lost` body. `reason` is required.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReportCardLostRequest {
    pub reason: CardLostReason,
    /// Free text. Optional for every reason, including `other`.
    #[validate(length(max = 500, message = "detail must be at most 500 characters"))]
    #[serde(default)]
    pub detail: Option<String>,
}

/// Fields the handler needs to shape a response that matches the caller's rights.
#[derive(Debug)]
pub struct ReportedLostCard {
    pub user_id: i64,
    pub card_number: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub reason: CardLostReason,
    pub detail: Option<String>,
}

/// `POST /users/{id}/card-lost` for a caller who can read users.
///
/// No name, email, address, or login. Those stay on `GET /users/{id}`.
#[serde_as]
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReportCardLostStaffBody {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub user_id: i64,
    pub card_lost: bool,
    pub reason: CardLostReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// `POST /users/{id}/card-lost` for loans write without users read.
///
/// Same identity as a loans-only card scan: id, card number, initials.
#[serde_as]
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReportCardLostDeskBody {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    /// `users.barcode`. Null when the stored card is blank.
    pub card_number: Option<String>,
    /// Uppercase first letter of `firstname` plus first letter of `lastname`.
    pub initials: String,
}

/// Card flagged lost. The variant follows the caller's users rights.
#[derive(Debug, Serialize, ToSchema)]
#[serde(untagged)]
pub enum ReportCardLostResponse {
    /// Users read or write.
    Staff(ReportCardLostStaffBody),
    /// Loans write without users read.
    Desk(ReportCardLostDeskBody),
}

/// `POST /users/{id}/replace-card` body. `barcode` is the new library card number.
#[derive(Debug, Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceCardRequest {
    #[validate(length(min = 1, max = 100, message = "barcode must be 1 to 100 characters"))]
    pub barcode: String,
}

/// New card number assigned and `users.card_lost` cleared.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceCardResponse {
    pub user: User,
    pub card_lost: bool,
    /// Previous `users.barcode`, now retired. Absent when the patron had no card number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_barcode: Option<String>,
    pub barcode: String,
}
