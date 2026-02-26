//! MARC record parsing and translation
//!
//! This module provides functionality to parse MARC21 and UNIMARC records
//! and translate them into the internal Item structure.

pub mod translator;

pub use z3950_rs::marc_rs::{Record as MarcRecord, MarcFormat, DataField, Subfield, ControlField};

use crate::models::item::Item;
use z3950_rs::marc_rs::{parse, Encoding, FormatEncoding};

/// Parse UNIMARC binary data into a list of items (with specimens from 995/952).
pub fn parse_unimarc_to_items(
    data: &[u8],
) -> Result<Vec<Item>, z3950_rs::marc_rs::ParseError> {
    let format_encoding = FormatEncoding::new(MarcFormat::Unimarc, Encoding::Utf8);
    let records = parse(data, format_encoding)?;
    Ok(records.into_iter().map(Item::from).collect())
}


