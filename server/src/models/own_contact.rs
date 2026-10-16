//! Contact details and preferences a patron may change on their own user record.
//!
//! `PUT /users/{id}` with `users_rights = own` accepts only these fields. Any other
//! key, including account type, rights, card number, status, and fines, is rejected
//! and nothing is written.
//!
//! `birthdate` is not one of those fields. A self-service body may repeat the stored
//! date (the key is ignored). A different date is `403` and nothing is written.

use chrono::NaiveDate;
use serde_json::Value;
use validator::ValidateEmail;

use super::Language;
use crate::error::{AppError, AppResult};

/// One optional column on an own-record update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContactField<T> {
    /// Key omitted.
    Absent,
    /// JSON null or blank string. Stored as SQL NULL.
    Clear,
    /// Replacement value.
    Set(T),
}

/// Fields applied by `PUT /users/{own id}` when `users_rights` is own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnContactPatch {
    pub email: ContactField<String>,
    pub addr_street: ContactField<String>,
    pub addr_zip_code: ContactField<i32>,
    pub addr_city: ContactField<String>,
    pub phone: ContactField<String>,
    pub language: ContactField<Language>,
    /// `eventsConsent`. Absent leaves the stored choice unchanged.
    pub events_consent: Option<bool>,
    /// `receiveReminders`. Absent leaves the stored choice unchanged.
    pub receive_reminders: Option<bool>,
}

/// Birth date key on a self-service body (`PUT /auth/profile` or own `PUT /users/{id}`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmittedBirthdate {
    /// Key omitted. The stored date is left unchanged.
    Absent,
    /// Key present. `None` is JSON null.
    Present(Option<NaiveDate>),
}

/// Read `birthdate` without removing it.
///
/// An unparseable value is `400`. Comparison with the stored date is separate so a
/// matching value can be ignored and a different value can be refused before any write.
pub fn submitted_birthdate(body: &Value) -> AppResult<SubmittedBirthdate> {
    let Some(object) = body.as_object() else {
        return Err(AppError::Validation(
            "JSON body must be an object".to_string(),
        ));
    };
    let Some(value) = object.get("birthdate") else {
        return Ok(SubmittedBirthdate::Absent);
    };
    if value.is_null() {
        return Ok(SubmittedBirthdate::Present(None));
    }
    let date = serde_json::from_value(value.clone()).map_err(|_| {
        AppError::Validation("birthdate must be an ISO date (YYYY-MM-DD) or null".to_string())
    })?;
    Ok(SubmittedBirthdate::Present(Some(date)))
}

/// `true` when a present birth date is not the stored value.
///
/// An omitted key is not a change. JSON null differs from a stored date, and a stored
/// null differs from a date. Equal values, including null and null, are not a change.
#[must_use]
pub fn self_service_birthdate_differs(
    submitted: &SubmittedBirthdate,
    stored: Option<NaiveDate>,
) -> bool {
    match submitted {
        SubmittedBirthdate::Absent => false,
        SubmittedBirthdate::Present(value) => *value != stored,
    }
}

/// Drop `birthdate` so a repeated stored date is not applied as a write.
pub fn without_birthdate_key(mut body: Value) -> Value {
    if let Some(object) = body.as_object_mut() {
        object.remove("birthdate");
    }
    body
}

const ALLOWED: &[&str] = &[
    "email",
    "phone",
    "addrStreet",
    "addrZipCode",
    "addrCity",
    "language",
    "eventsConsent",
    "receiveReminders",
];

/// Parse an own-record body. A key outside the contact allowlist is `403`.
/// A present allowlisted key with the wrong JSON type is `400`. Nothing is implied
/// about persistence: the caller writes only after this returns `Ok`.
pub fn parse_own_contact(body: &Value) -> AppResult<OwnContactPatch> {
    let object = body
        .as_object()
        .ok_or_else(|| AppError::Validation("JSON body must be an object".to_string()))?;

    for key in object.keys() {
        if !ALLOWED.contains(&key.as_str()) {
            return Err(AppError::Authorization(format!(
                "Field '{key}' cannot be changed on your own record"
            )));
        }
    }

    Ok(OwnContactPatch {
        email: optional_string(object.get("email"), "email", 255, true)?,
        phone: optional_string(object.get("phone"), "phone", 50, false)?,
        addr_street: optional_string(object.get("addrStreet"), "addrStreet", 255, false)?,
        addr_city: optional_string(object.get("addrCity"), "addrCity", 255, false)?,
        addr_zip_code: optional_zip(object.get("addrZipCode"))?,
        language: optional_language(object.get("language"))?,
        events_consent: optional_bool(object.get("eventsConsent"), "eventsConsent")?,
        receive_reminders: optional_bool(object.get("receiveReminders"), "receiveReminders")?,
    })
}

fn optional_string(
    value: Option<&Value>,
    field: &str,
    max_len: usize,
    email: bool,
) -> AppResult<ContactField<String>> {
    let Some(value) = value else {
        return Ok(ContactField::Absent);
    };
    if value.is_null() {
        return Ok(ContactField::Clear);
    }
    let Some(raw) = value.as_str() else {
        return Err(AppError::Validation(format!(
            "{field} must be a string or null"
        )));
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(ContactField::Clear);
    }
    if trimmed.chars().count() > max_len {
        return Err(AppError::Validation(format!(
            "{field} must be at most {max_len} characters"
        )));
    }
    if email && !trimmed.validate_email() {
        return Err(AppError::Validation("Invalid email format".to_string()));
    }
    Ok(ContactField::Set(trimmed.to_string()))
}

fn optional_zip(value: Option<&Value>) -> AppResult<ContactField<i32>> {
    let Some(value) = value else {
        return Ok(ContactField::Absent);
    };
    if value.is_null() {
        return Ok(ContactField::Clear);
    }
    let Some(number) = value.as_i64() else {
        return Err(AppError::Validation(
            "addrZipCode must be an integer or null".to_string(),
        ));
    };
    let zip = i32::try_from(number)
        .map_err(|_| AppError::Validation("addrZipCode is out of range".to_string()))?;
    Ok(ContactField::Set(zip))
}

fn optional_language(value: Option<&Value>) -> AppResult<ContactField<Language>> {
    let Some(value) = value else {
        return Ok(ContactField::Absent);
    };
    if value.is_null() {
        return Ok(ContactField::Clear);
    }
    let language = serde_json::from_value(value.clone())
        .map_err(|_| AppError::Validation("language is not a known language".to_string()))?;
    Ok(ContactField::Set(language))
}

fn optional_bool(value: Option<&Value>, field: &str) -> AppResult<Option<bool>> {
    let Some(value) = value else {
        return Ok(None);
    };
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| AppError::Validation(format!("{field} must be true or false")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn allowed_contact_fields_parse() {
        let patch = parse_own_contact(&json!({
            "email": "ada@library.test",
            "phone": " 0600000000 ",
            "addrStreet": "1 Rue",
            "addrZipCode": 75001,
            "addrCity": "Paris",
            "language": "english",
            "eventsConsent": true,
            "receiveReminders": false
        }))
        .expect("allowed");
        assert_eq!(patch.email, ContactField::Set("ada@library.test".into()));
        assert_eq!(patch.phone, ContactField::Set("0600000000".into()));
        assert_eq!(patch.addr_zip_code, ContactField::Set(75001));
        assert_eq!(patch.language, ContactField::Set(Language::English));
        assert_eq!(patch.events_consent, Some(true));
        assert_eq!(patch.receive_reminders, Some(false));
    }

    #[test]
    fn birthdate_key_matches_only_the_stored_value() {
        let stored = NaiveDate::from_ymd_opt(1990, 6, 1).unwrap();
        assert!(!self_service_birthdate_differs(
            &submitted_birthdate(&json!({"phone": "1"})).unwrap(),
            Some(stored)
        ));
        assert!(!self_service_birthdate_differs(
            &submitted_birthdate(&json!({"birthdate": "1990-06-01"})).unwrap(),
            Some(stored)
        ));
        assert!(self_service_birthdate_differs(
            &submitted_birthdate(&json!({"birthdate": "1991-06-01"})).unwrap(),
            Some(stored)
        ));
        assert!(self_service_birthdate_differs(
            &submitted_birthdate(&json!({"birthdate": null})).unwrap(),
            Some(stored)
        ));
        assert!(!self_service_birthdate_differs(
            &submitted_birthdate(&json!({"birthdate": null})).unwrap(),
            None
        ));
        assert!(submitted_birthdate(&json!({"birthdate": "tomorrow"})).is_err());
    }

    #[test]
    fn privileged_fields_are_forbidden_and_do_not_parse_the_rest() {
        for body in [
            json!({"email": "next@library.test", "accountType": "admin"}),
            json!({"usersRights": "w"}),
            json!({"rights": {"usersRights": "w"}}),
            json!({"barcode": "CARD"}),
            json!({"cardNumber": "CARD"}),
            json!({"status": "blocked"}),
            json!({"fee": "local"}),
            json!({"fines": 100}),
            json!({"firstname": "Ada"}),
        ] {
            let err = parse_own_contact(&body).expect_err("forbidden");
            assert!(
                matches!(err, AppError::Authorization(_)),
                "{body} -> {err:?}"
            );
        }
    }
}
