//! Author model and related types

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sqlx::FromRow;
use utoipa::ToSchema;

/// Author function in item relationship
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum Function {
    Author,
    Illustrator,
    Translator,
    ScientificAdvisor,
    PrefaceWriter,
    Photographer,
    PublishingDirector,
    Composer,
}

impl Function {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Function::Author => "author",
            Function::Illustrator => "illustrator",
            Function::Translator => "translator",
            Function::ScientificAdvisor => "scientificAdvisor",
            Function::PrefaceWriter => "prefaceWriter",
            Function::Photographer => "photographer",
            Function::PublishingDirector => "publishingDirector",
            Function::Composer => "composer",
        }
    }
}

impl From<&str> for Function {
    fn from(s: &str) -> Self {
        match s {
            "author" | "aut" => Function::Author,
            "illustrator" | "ill" => Function::Illustrator,
            "translator" | "trl" => Function::Translator,
            "scientificAdvisor" | "edt" | "editor" => Function::ScientificAdvisor,
            "prefaceWriter" | "aui" => Function::PrefaceWriter,
            "photographer" | "pht" => Function::Photographer,
            "publishingDirector" | "pbd" | "publisher" => Function::PublishingDirector,
            "composer" | "cmp" => Function::Composer,
            _ => Function::Author,
        }
    }
}

impl std::fmt::Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_db_str())
    }
}

impl std::str::FromStr for Function {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Function::from(s))
    }
}

impl sqlx::Type<sqlx::Postgres> for Function {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for Function {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
        let s: String = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Ok(Function::from(s.as_str()))
    }
}

impl sqlx::Encode<'_, sqlx::Postgres> for Function {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        <String as sqlx::Encode<sqlx::Postgres>>::encode(self.to_string(), buf)
    }
}

/// Author with function for item relationships
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, FromRow)]
pub struct Author {
    #[serde_as(as = "DisplayFromStr")]
    #[schema(value_type = String)]
    pub id: i64,
    pub key: Option<String>,
    pub lastname: Option<String>,
    pub firstname: Option<String>,
    pub bio: Option<String>,
    pub notes: Option<String>,
    pub function: Option<Function>,
}

/// Create author request
#[derive(Debug, Deserialize)]
pub struct CreateAuthor {
    pub lastname: String,
    pub firstname: Option<String>,
    pub bio: Option<String>,
    pub notes: Option<String>,
}

/// Update author request
#[derive(Debug, Deserialize)]
pub struct UpdateAuthor {
    pub lastname: Option<String>,
    pub firstname: Option<String>,
    pub bio: Option<String>,
    pub notes: Option<String>,
}
