//! Serde helpers shared by request DTOs.

use serde::de::Error;
use serde::{Deserialize, Deserializer};

/// Missing field stays `None` when the field also has `#[serde(default)]`.
/// JSON `null` is rejected so it cannot mean both "clear" and "leave unchanged".
pub fn reject_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    match Option::<T>::deserialize(deserializer)? {
        Some(value) => Ok(Some(value)),
        None => Err(D::Error::custom("null is not accepted; omit the field to leave it unchanged")),
    }
}
