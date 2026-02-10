//! API handlers for Elidune REST endpoints

pub mod auth;
pub mod health;
pub mod items;
pub mod loans;
pub mod openapi;
pub mod settings;
pub mod stats;
pub mod users;
pub mod z3950;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use crate::{error::AppError, models::user::UserClaims, AppState};

/// Extractor for authenticated user from JWT token
pub struct AuthenticatedUser(pub UserClaims);

#[async_trait]
impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        // Get the Authorization header
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::Authentication("Missing authorization header".to_string()))?;

        // Check for Bearer token
        if !auth_header.starts_with("Bearer ") {
            return Err(AppError::Authentication("Invalid authorization header format".to_string()));
        }

        let token = &auth_header[7..];

        // Validate JWT token using the secret from configuration
        let claims = UserClaims::from_token(token, &state.config.users.jwt_secret)
            .map_err(|e| AppError::Authentication(e.to_string()))?;

        Ok(AuthenticatedUser(claims))
    }
}

