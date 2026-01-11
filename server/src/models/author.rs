//! Author model and related types

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

/// Author function codes (matching original C implementation)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum AuthorFunction {
    Unknown = 0,
    Actor = 5,
    Adapter = 10,
    Annotator = 20,
    Arranger = 30,
    Artist = 40,
    Author = 70,
    AuthorQuote = 72,
    AuthorAfterwork = 75,
    AuthorIntroduction = 80,
    AuthorDialog = 90,
    Collaborator = 205,
    Composer = 230,
    Conductor = 250,
    Director = 300,
    Editor = 340,
    Illustrator = 440,
    Lyricist = 520,
    Musician = 545,
    Narrator = 550,
    Performer = 590,
    Photographer = 600,
    Producer = 630,
    Publisher = 650,
    Scenarist = 690,
    Singer = 721,
    Translator = 730,
}

impl From<i32> for AuthorFunction {
    fn from(v: i32) -> Self {
        match v {
            5 => AuthorFunction::Actor,
            10 => AuthorFunction::Adapter,
            20 => AuthorFunction::Annotator,
            30 => AuthorFunction::Arranger,
            40 => AuthorFunction::Artist,
            70 => AuthorFunction::Author,
            72 => AuthorFunction::AuthorQuote,
            75 => AuthorFunction::AuthorAfterwork,
            80 => AuthorFunction::AuthorIntroduction,
            90 => AuthorFunction::AuthorDialog,
            205 => AuthorFunction::Collaborator,
            230 => AuthorFunction::Composer,
            250 => AuthorFunction::Conductor,
            300 => AuthorFunction::Director,
            340 => AuthorFunction::Editor,
            440 => AuthorFunction::Illustrator,
            520 => AuthorFunction::Lyricist,
            545 => AuthorFunction::Musician,
            550 => AuthorFunction::Narrator,
            590 => AuthorFunction::Performer,
            600 => AuthorFunction::Photographer,
            630 => AuthorFunction::Producer,
            650 => AuthorFunction::Publisher,
            690 => AuthorFunction::Scenarist,
            721 => AuthorFunction::Singer,
            730 => AuthorFunction::Translator,
            _ => AuthorFunction::Unknown,
        }
    }
}

/// Full author model from database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Author {
    pub id: i32,
    pub key: Option<String>,
    pub lastname: Option<String>,
    pub firstname: Option<String>,
    pub bio: Option<String>,
    pub notes: Option<String>,
}

/// Author with function for item relationships
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthorWithFunction {
    pub id: i32,
    pub lastname: Option<String>,
    pub firstname: Option<String>,
    pub bio: Option<String>,
    pub notes: Option<String>,
    pub function: Option<String>,
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

