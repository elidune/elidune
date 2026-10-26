//! Local item barcode generation (snowflake string, unique among active copies).

use snowflaked::Generator;
use std::sync::{LazyLock, Mutex};

use crate::{
    error::{error_code, AppError, AppResult},
    repository::Repository,
};

static LOCAL_BARCODE_IDS: LazyLock<Mutex<Generator>> =
    LazyLock::new(|| Mutex::new(Generator::new(7)));

fn next_local_barcode_candidate() -> String {
    LOCAL_BARCODE_IDS
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .generate::<i64>()
        .to_string()
}

/// Allocate a barcode not used by any active copy.
pub async fn allocate_local_item_barcode(repository: &Repository) -> AppResult<String> {
    for _ in 0..32 {
        let candidate = next_local_barcode_candidate();
        let taken = repository
            .items_barcode_exists_active(&candidate, None)
            .await?;
        if !taken {
            return Ok(candidate);
        }
    }
    Err(AppError::Internal(
        "Could not allocate a unique local item barcode".into(),
    ))
}

/// Stable 422 when re-activating would collide with another active copy.
pub fn reactivation_barcode_taken() -> AppError {
    AppError::coded(
        422,
        error_code::ITEM_REACTIVATION_BARCODE_TAKEN,
        "This copy's barcode is already used by an active copy",
    )
}
