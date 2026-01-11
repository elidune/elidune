//! MARC record parsing and translation
//!
//! This module provides functionality to parse MARC21 and UNIMARC records
//! and translate them into the internal Item structure.

pub mod parser;
pub mod translator;

pub use parser::{MarcRecord, MarcFormat, DataField, Subfield};
pub use translator::MarcTranslator;


