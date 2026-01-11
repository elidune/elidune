//! User model and related types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::error::AppError;

/// User rights levels (matching original C implementation)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rights {
    None = 0,
    Read = 1,
    Write = 2,
}

impl From<char> for Rights {
    fn from(c: char) -> Self {
        match c {
            'r' | 'R' => Rights::Read,
            'w' | 'W' => Rights::Write,
            _ => Rights::None,
        }
    }
}

impl From<Option<String>> for Rights {
    fn from(s: Option<String>) -> Self {
        s.and_then(|s| s.chars().next())
            .map(Rights::from)
            .unwrap_or(Rights::None)
    }
}

/// User account types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum AccountType {
    Unknown = 0,
    Guest = 1,
    Reader = 2,
    Librarian = 3,
    Admin = 4,
    Group = 8,
}

impl From<i16> for AccountType {
    fn from(v: i16) -> Self {
        match v {
            1 => AccountType::Guest,
            2 => AccountType::Reader,
            3 => AccountType::Librarian,
            4 => AccountType::Admin,
            8 => AccountType::Group,
            _ => AccountType::Unknown,
        }
    }
}

/// User status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[repr(i16)]
pub enum UserStatus {
    Active = 0,
    Blocked = 1,
    Deleted = 2,
}

impl From<i16> for UserStatus {
    fn from(v: i16) -> Self {
        match v {
            0 => UserStatus::Active,
            1 => UserStatus::Blocked,
            2 => UserStatus::Deleted,
            _ => UserStatus::Active,
        }
    }
}

impl From<Option<i16>> for UserStatus {
    fn from(v: Option<i16>) -> Self {
        v.map(UserStatus::from).unwrap_or(UserStatus::Active)
    }
}

/// Full user model from database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct User {
    pub id: i32,
    pub group_id: Option<i32>,
    pub barcode: Option<String>,
    pub login: Option<String>,
    /// Hashed password (argon2)
    #[serde(skip_serializing)]
    pub password: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub email: Option<String>,
    pub addr_street: Option<String>,
    pub addr_zip_code: Option<i32>,
    pub addr_city: Option<String>,
    pub phone: Option<String>,
    pub occupation: Option<String>,
    pub occupation_id: Option<i32>,
    pub birthdate: Option<String>,
    pub crea_date: Option<DateTime<Utc>>,
    pub modif_date: Option<DateTime<Utc>>,
    pub issue_date: Option<DateTime<Utc>>,
    pub account_type_id: Option<i16>,
    pub subscription_type_id: Option<i16>,
    pub public_type: Option<i32>,
    pub notes: Option<String>,
    pub status: Option<i16>,
    pub archived_date: Option<DateTime<Utc>>,
    /// User preferred language (ISO 639-1 code: "fr", "en", etc.)
    pub language: Option<String>,
    // 2FA fields
    pub two_factor_enabled: Option<bool>,
    pub two_factor_method: Option<String>,
    #[serde(skip_serializing)]
    pub totp_secret: Option<String>,
    #[serde(skip_serializing)]
    pub recovery_codes: Option<String>,
    #[serde(skip_serializing)]
    pub recovery_codes_used: Option<String>,
    // Joined fields
    #[sqlx(skip)]
    pub account_type: Option<String>,
}

/// Occupation code for users
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Occupation {
    pub id: i32,
    pub code: String,
    pub label: String,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub sort_order: Option<i32>,
}

/// Short user representation for lists
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct UserShort {
    pub id: i32,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    pub account_type: Option<String>,
    pub nb_loans: Option<i64>,
    pub nb_late_loans: Option<i64>,
}

/// User query parameters
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct UserQuery {
    pub name: Option<String>,
    pub barcode: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// Create user request
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateUser {
    pub barcode: Option<String>,
    /// Login (username) - required and unique, used for authentication
    #[validate(length(min = 3, message = "Login must be at least 3 characters"))]
    pub login: String,
    #[validate(length(min = 4, message = "Password must be at least 4 characters"))]
    pub password: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    /// Email address (optional)
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,
    pub addr_street: Option<String>,
    pub addr_zip_code: Option<i32>,
    pub addr_city: Option<String>,
    pub phone: Option<String>,
    pub occupation: Option<String>,
    pub occupation_id: Option<i32>,
    pub birthdate: Option<String>,
    pub account_type_id: Option<i16>,
    pub subscription_type_id: Option<i16>,
    pub public_type: Option<i32>,
    pub notes: Option<String>,
    pub group_id: Option<i32>,
}

/// Update user request
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateUser {
    pub barcode: Option<String>,
    pub login: Option<String>,
    pub password: Option<String>,
    pub firstname: Option<String>,
    pub lastname: Option<String>,
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,
    pub addr_street: Option<String>,
    pub addr_zip_code: Option<i32>,
    pub addr_city: Option<String>,
    pub phone: Option<String>,
    pub occupation: Option<String>,
    pub occupation_id: Option<i32>,
    pub birthdate: Option<String>,
    pub account_type_id: Option<i16>,
    pub subscription_type_id: Option<i16>,
    pub public_type: Option<i32>,
    pub notes: Option<String>,
    pub group_id: Option<i32>,
    pub status: Option<i16>,
}

/// Update own profile request (for authenticated users)
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateProfile {
    /// First name
    pub firstname: Option<String>,
    /// Last name
    pub lastname: Option<String>,
    /// Email address (must be unique)
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,
    /// Login/username (must be unique if provided)
    #[validate(length(min = 3, message = "Login must be at least 3 characters"))]
    pub login: Option<String>,
    /// Street address
    pub addr_street: Option<String>,
    /// Zip code
    pub addr_zip_code: Option<i32>,
    /// City
    pub addr_city: Option<String>,
    /// Phone number
    pub phone: Option<String>,
    /// Occupation ID
    pub occupation_id: Option<i32>,
    /// Birth date
    pub birthdate: Option<String>,
    /// Current password (required to change password)
    pub current_password: Option<String>,
    /// New password
    #[validate(length(min = 4, message = "Password must be at least 4 characters"))]
    pub new_password: Option<String>,
    /// Preferred language (ISO 639-1 code: "fr", "en", etc.)
    #[validate(length(min = 2, max = 5, message = "Language code must be 2-5 characters"))]
    pub language: Option<String>,
}

/// Update account type request (admin only)
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAccountType {
    /// New account type ID (1=Guest, 2=Reader, 3=Librarian, 4=Admin)
    pub account_type_id: i16,
}

/// User rights structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRights {
    pub items_rights: Rights,
    pub users_rights: Rights,
    pub loans_rights: Rights,
    pub borrows_rights: Rights,
    pub settings_rights: Rights,
    pub items_archive_rights: Rights,
}

impl Default for UserRights {
    fn default() -> Self {
        Self {
            items_rights: Rights::None,
            users_rights: Rights::None,
            loans_rights: Rights::None,
            borrows_rights: Rights::None,
            settings_rights: Rights::None,
            items_archive_rights: Rights::None,
        }
    }
}

/// JWT Claims for authenticated users
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserClaims {
    pub sub: String,
    pub user_id: i32,
    pub account_type_id: i16,
    pub rights: UserRights,
    pub exp: i64,
    pub iat: i64,
}

impl UserClaims {
    /// Create a new JWT token
    pub fn create_token(&self, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
        use jsonwebtoken::{encode, EncodingKey, Header};
        encode(
            &Header::default(),
            self,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
    }

    /// Parse JWT token
    pub fn from_token(token: &str, secret: &str) -> Result<Self, jsonwebtoken::errors::Error> {
        use jsonwebtoken::{decode, DecodingKey, Validation};
        let token_data = decode::<Self>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )?;
        Ok(token_data.claims)
    }

    // Authorization checks
    pub fn require_read_items(&self) -> Result<(), AppError> {
        if self.rights.items_rights as u8 >= Rights::Read as u8 {
            Ok(())
        } else {
            Err(AppError::Authorization("Insufficient rights to read items".to_string()))
        }
    }

    pub fn require_write_items(&self) -> Result<(), AppError> {
        if self.rights.items_rights as u8 >= Rights::Write as u8 {
            Ok(())
        } else {
            Err(AppError::Authorization("Insufficient rights to write items".to_string()))
        }
    }

    pub fn require_read_users(&self) -> Result<(), AppError> {
        if self.rights.users_rights as u8 >= Rights::Read as u8 {
            Ok(())
        } else {
            Err(AppError::Authorization("Insufficient rights to read users".to_string()))
        }
    }

    pub fn require_write_users(&self) -> Result<(), AppError> {
        if self.rights.users_rights as u8 >= Rights::Write as u8 {
            Ok(())
        } else {
            Err(AppError::Authorization("Insufficient rights to write users".to_string()))
        }
    }

    pub fn require_read_loans(&self) -> Result<(), AppError> {
        if self.rights.loans_rights as u8 >= Rights::Read as u8 {
            Ok(())
        } else {
            Err(AppError::Authorization("Insufficient rights to read loans".to_string()))
        }
    }

    pub fn require_write_borrows(&self) -> Result<(), AppError> {
        if self.rights.borrows_rights as u8 >= Rights::Write as u8 {
            Ok(())
        } else {
            Err(AppError::Authorization("Insufficient rights to manage borrows".to_string()))
        }
    }

    pub fn require_read_settings(&self) -> Result<(), AppError> {
        if self.rights.settings_rights as u8 >= Rights::Read as u8 {
            Ok(())
        } else {
            Err(AppError::Authorization("Insufficient rights to read settings".to_string()))
        }
    }

    pub fn require_write_settings(&self) -> Result<(), AppError> {
        if self.rights.settings_rights as u8 >= Rights::Write as u8 {
            Ok(())
        } else {
            Err(AppError::Authorization("Insufficient rights to write settings".to_string()))
        }
    }

    /// Check if user is admin (account_type_id = 4)
    pub fn is_admin(&self) -> bool {
        self.account_type_id == 4
    }

    /// Require admin privileges
    pub fn require_admin(&self) -> Result<(), AppError> {
        if self.is_admin() {
            Ok(())
        } else {
            Err(AppError::Authorization("Administrator privileges required".to_string()))
        }
    }
}

