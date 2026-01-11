//! MARC record parser
//!
//! Parses raw MARC data into a structured representation.

use std::collections::HashMap;

/// MARC format type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarcFormat {
    /// UNIMARC (European format)
    Unimarc,
    /// MARC21 / USMARC (American format)
    #[default]
    Marc21,
}

impl MarcFormat {
    /// Parse format from yaz-client output string (e.g., "Unimarc", "USmarc", "MARC21")
    pub fn from_yaz_output(s: &str) -> Self {
        let s_lower = s.to_lowercase();
        if s_lower.contains("unimarc") {
            MarcFormat::Unimarc
        } else {
            MarcFormat::Marc21
        }
    }
}

/// A MARC record containing leader and fields
#[derive(Debug, Clone)]
pub struct MarcRecord {
    /// The 24-character record leader
    pub leader: String,
    /// Control fields (00X)
    pub control_fields: HashMap<String, String>,
    /// Data fields with indicators and subfields
    pub data_fields: Vec<DataField>,
    /// Record format (UNIMARC or MARC21)
    pub format: MarcFormat,
}

/// A MARC data field (010-999)
#[derive(Debug, Clone)]
pub struct DataField {
    /// Field tag (3 characters)
    pub tag: String,
    /// First indicator
    pub ind1: char,
    /// Second indicator
    pub ind2: char,
    /// Subfields
    pub subfields: Vec<Subfield>,
}

/// A MARC subfield
#[derive(Debug, Clone)]
pub struct Subfield {
    /// Subfield code (single character)
    pub code: char,
    /// Subfield data
    pub data: String,
}

impl MarcRecord {
    /// Parse a MARC record from raw bytes (ISO 2709 format) with default format (MARC21)
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        Self::from_bytes_with_format(data, MarcFormat::default())
    }

    /// Parse a MARC record from raw bytes (ISO 2709 format) with specified format
    pub fn from_bytes_with_format(data: &[u8], format: MarcFormat) -> Option<Self> {
        if data.len() < 24 {
            return None;
        }

        // Parse leader
        let leader = String::from_utf8_lossy(&data[0..24]).to_string();

        // Get base address of data
        let base_address: usize = String::from_utf8_lossy(&data[12..17])
            .parse()
            .ok()?;

        // Parse directory (between leader and record separator)
        let directory_data = &data[24..base_address - 1];
        let mut control_fields = HashMap::new();
        let mut data_fields = Vec::new();

        // Each directory entry is 12 bytes: tag(3) + length(4) + start(5)
        let mut pos = 0;
        while pos + 12 <= directory_data.len() {
            let entry = &directory_data[pos..pos + 12];
            let tag = String::from_utf8_lossy(&entry[0..3]).to_string();
            let length: usize = String::from_utf8_lossy(&entry[3..7])
                .parse()
                .ok()?;
            let start: usize = String::from_utf8_lossy(&entry[7..12])
                .parse()
                .ok()?;

            // Get field data
            let field_start = base_address + start;
            let field_end = field_start + length - 1; // -1 for field terminator

            if field_end <= data.len() {
                let field_data = &data[field_start..field_end];
                
                if tag.starts_with("00") {
                    // Control field
                    control_fields.insert(tag, String::from_utf8_lossy(field_data).to_string());
                } else {
                    // Data field
                    if let Some(data_field) = Self::parse_data_field(&tag, field_data) {
                        data_fields.push(data_field);
                    }
                }
            }

            pos += 12;
        }

        Some(MarcRecord {
            leader,
            control_fields,
            data_fields,
            format,
        })
    }

    /// Parse a data field from raw bytes
    fn parse_data_field(tag: &str, data: &[u8]) -> Option<DataField> {
        if data.len() < 2 {
            return None;
        }

        let ind1 = data[0] as char;
        let ind2 = data[1] as char;

        let mut subfields = Vec::new();
        let subfield_data = &data[2..];

        // Subfields are separated by 0x1F (unit separator)
        for part in subfield_data.split(|&b| b == 0x1F) {
            if part.is_empty() {
                continue;
            }
            let code = part[0] as char;
            let data = String::from_utf8_lossy(&part[1..]).to_string();
            subfields.push(Subfield { code, data });
        }

        Some(DataField {
            tag: tag.to_string(),
            ind1,
            ind2,
            subfields,
        })
    }

    /// Get a subfield value by tag and subfield code
    pub fn get_subfield(&self, tag: &str, code: char) -> Option<&str> {
        for field in &self.data_fields {
            if field.tag == tag {
                for subfield in &field.subfields {
                    if subfield.code == code {
                        return Some(&subfield.data);
                    }
                }
            }
        }
        None
    }

    /// Get all subfield values for a tag and code
    pub fn get_all_subfields(&self, tag: &str, code: char) -> Vec<&str> {
        let mut results = Vec::new();
        for field in &self.data_fields {
            if field.tag == tag {
                for subfield in &field.subfields {
                    if subfield.code == code {
                        results.push(subfield.data.as_str());
                    }
                }
            }
        }
        results
    }

    /// Get a control field value
    pub fn get_control_field(&self, tag: &str) -> Option<&str> {
        self.control_fields.get(tag).map(String::as_str)
    }

    /// Get all data fields with a specific tag
    pub fn get_fields(&self, tag: &str) -> Vec<&DataField> {
        self.data_fields
            .iter()
            .filter(|f| f.tag == tag)
            .collect()
    }
}

impl DataField {
    /// Get a subfield value by code
    pub fn get_subfield(&self, code: char) -> Option<&str> {
        self.subfields
            .iter()
            .find(|sf| sf.code == code)
            .map(|sf| sf.data.as_str())
    }

    /// Get all subfield values for a code
    pub fn get_all_subfields(&self, code: char) -> Vec<&str> {
        self.subfields
            .iter()
            .filter(|sf| sf.code == code)
            .map(|sf| sf.data.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        assert!(MarcRecord::from_bytes(&[]).is_none());
    }

    #[test]
    fn test_parse_short() {
        assert!(MarcRecord::from_bytes(&[0; 20]).is_none());
    }
}


