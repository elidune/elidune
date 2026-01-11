//! MARC to Item translator
//!
//! Translates MARC records (UNIMARC or MARC21) into the internal Item structure.

use super::parser::{MarcRecord, MarcFormat};
use crate::models::item::{CreateItem, CreateItemAuthor, CreateCollection, CreateEdition, CreateSerie};

/// MARC record translator
pub struct MarcTranslator;

impl MarcTranslator {
    /// Create a new translator
    pub fn new() -> Self {
        Self
    }

    /// Translate a MARC record into a CreateItem structure
    /// Uses the format stored in the record
    pub fn translate(&self, record: &MarcRecord) -> CreateItem {
        match record.format {
            MarcFormat::Unimarc => self.translate_unimarc(record),
            MarcFormat::Marc21 => self.translate_marc21(record),
        }
    }

    /// Translate UNIMARC record
    fn translate_unimarc(&self, record: &MarcRecord) -> CreateItem {
        // UNIMARC field mappings
        // 010$a - ISBN
        // 200$a - Title proper
        // 200$e - Other title information
        // 200$f - First statement of responsibility
        // 700$a - Author surname
        // 700$b - Author forename
        // 210$a - Place of publication
        // 210$c - Publisher name
        // 210$d - Date of publication
        // 225$a - Series title
        // 215$a - Pagination

        let isbn = record.get_subfield("010", 'a').map(normalize_isbn);
        let title1 = record.get_subfield("200", 'a').map(String::from);
        let title2 = record.get_subfield("200", 'e').map(String::from);
        let publication_date = record.get_subfield("210", 'd').map(String::from);
        let nb_pages = record.get_subfield("215", 'a').map(String::from);

        // Authors from 700, 701, 702 fields
        let mut authors1 = Vec::new();
        for tag in ["700", "701", "702"] {
            for field in record.get_fields(tag) {
                if let (Some(lastname), Some(firstname)) = (
                    field.get_subfield('a'),
                    field.get_subfield('b'),
                ) {
                    authors1.push(CreateItemAuthor {
                        id: None,
                        lastname: Some(lastname.to_string()),
                        firstname: Some(firstname.to_string()),
                        function: field.get_subfield('4').map(String::from),
                    });
                }
            }
        }

        // Edition from 210
        let edition = if let Some(publisher) = record.get_subfield("210", 'c') {
            Some(CreateEdition {
                id: None,
                name: Some(publisher.to_string()),
                place: record.get_subfield("210", 'a').map(String::from),
                date: record.get_subfield("210", 'd').map(String::from),
            })
        } else {
            None
        };

        // Series from 225
        let serie = if let Some(series_title) = record.get_subfield("225", 'a') {
            Some(CreateSerie {
                id: None,
                name: Some(series_title.to_string()),
                volume_number: record
                    .get_subfield("225", 'v')
                    .and_then(|v| v.parse().ok()),
            })
        } else {
            None
        };

        // Collection from 410
        let collection = if let Some(coll_title) = record.get_subfield("410", 't') {
            Some(CreateCollection {
                id: None,
                title1: Some(coll_title.to_string()),
                title2: None,
                title3: None,
                issn: record.get_subfield("410", 'x').map(String::from),
                number_sub: None,
                volume_number: None,
            })
        } else {
            None
        };

        // Media type from leader position 6
        let media_type = self.determine_media_type_unimarc(record);

        // Language from 101$a
        let lang = record
            .get_subfield("101", 'a')
            .and_then(|l| language_code_to_id(l));

        CreateItem {
            media_type: Some(media_type),
            identification: isbn,
            price: None,
            barcode: None,
            dewey: record.get_subfield("676", 'a').map(String::from),
            publication_date,
            lang,
            lang_orig: None,
            title1: title1.unwrap_or_default(),
            title2,
            title3: None,
            title4: None,
            genre: None,
            subject: record.get_subfield("606", 'a').map(String::from),
            public_type: self.determine_audience_unimarc(record),
            nb_pages,
            format: record.get_subfield("215", 'd').map(String::from),
            content: None,
            addon: record.get_subfield("215", 'e').map(String::from),
            abstract_: record.get_subfield("330", 'a').map(String::from),
            notes: record.get_subfield("300", 'a').map(String::from),
            keywords: record.get_subfield("610", 'a').map(String::from),
            is_valid: Some(1),
            authors1: if authors1.is_empty() { None } else { Some(authors1) },
            authors2: None,
            authors3: None,
            serie,
            collection,
            edition,
            specimens: None,
        }
    }

    /// Translate MARC21 record
    fn translate_marc21(&self, record: &MarcRecord) -> CreateItem {
        // MARC21 field mappings
        // 020$a - ISBN
        // 245$a - Title
        // 245$b - Subtitle
        // 100$a - Main author
        // 260$a - Place of publication
        // 260$b - Publisher name
        // 260$c - Date of publication
        // 300$a - Pagination
        // 490$a - Series

        let isbn = record.get_subfield("020", 'a').map(normalize_isbn);
        let title1 = record.get_subfield("245", 'a').map(clean_title);
        let title2 = record.get_subfield("245", 'b').map(clean_title);
        let publication_date = record.get_subfield("260", 'c').map(String::from);
        let nb_pages = record.get_subfield("300", 'a').map(String::from);

        // Main author from 100
        let mut authors1 = Vec::new();
        if let Some(author_name) = record.get_subfield("100", 'a') {
            let (lastname, firstname) = parse_author_name(author_name);
            authors1.push(CreateItemAuthor {
                id: None,
                lastname: Some(lastname),
                firstname,
                function: Some("70".to_string()), // Author
            });
        }

        // Additional authors from 700
        for field in record.get_fields("700") {
            if let Some(author_name) = field.get_subfield('a') {
                let (lastname, firstname) = parse_author_name(author_name);
                authors1.push(CreateItemAuthor {
                    id: None,
                    lastname: Some(lastname),
                    firstname,
                    function: field.get_subfield('e').map(String::from),
                });
            }
        }

        // Edition from 260
        let edition = if let Some(publisher) = record.get_subfield("260", 'b') {
            Some(CreateEdition {
                id: None,
                name: Some(clean_publisher(publisher)),
                place: record.get_subfield("260", 'a').map(|p| clean_place(p)),
                date: record.get_subfield("260", 'c').map(String::from),
            })
        } else {
            None
        };

        // Series from 490
        let serie = if let Some(series_title) = record.get_subfield("490", 'a') {
            Some(CreateSerie {
                id: None,
                name: Some(series_title.to_string()),
                volume_number: record
                    .get_subfield("490", 'v')
                    .and_then(|v| extract_volume_number(v)),
            })
        } else {
            None
        };

        // Media type from leader
        let media_type = self.determine_media_type_marc21(record);

        // Language from 008 positions 35-37
        let lang = record
            .get_control_field("008")
            .and_then(|cf| {
                if cf.len() >= 38 {
                    Some(&cf[35..38])
                } else {
                    None
                }
            })
            .and_then(language_code_to_id);

        CreateItem {
            media_type: Some(media_type),
            identification: isbn,
            price: None,
            barcode: None,
            dewey: record.get_subfield("082", 'a').map(String::from),
            publication_date,
            lang,
            lang_orig: None,
            title1: title1.unwrap_or_default(),
            title2,
            title3: None,
            title4: None,
            genre: None,
            subject: record.get_subfield("650", 'a').map(String::from),
            public_type: self.determine_audience_marc21(record),
            nb_pages,
            format: record.get_subfield("300", 'c').map(String::from),
            content: None,
            addon: record.get_subfield("300", 'e').map(String::from),
            abstract_: record.get_subfield("520", 'a').map(String::from),
            notes: record.get_subfield("500", 'a').map(String::from),
            keywords: record
                .get_all_subfields("653", 'a')
                .join(", ")
                .into(),
            is_valid: Some(1),
            authors1: if authors1.is_empty() { None } else { Some(authors1) },
            authors2: None,
            authors3: None,
            serie,
            collection: None,
            edition,
            specimens: None,
        }
    }

    /// Determine media type from UNIMARC leader
    fn determine_media_type_unimarc(&self, record: &MarcRecord) -> String {
        let leader = &record.leader;
        if leader.len() < 7 {
            return "u".to_string();
        }

        let type_code = leader.chars().nth(6).unwrap_or('a');
        match type_code {
            'a' | 'b' => "b".to_string(),  // Printed text
            'c' | 'd' => "bc".to_string(), // Comics (scores)
            'g' => "v".to_string(),         // Video
            'i' | 'j' => "a".to_string(),  // Audio
            'm' => "c".to_string(),         // CD-ROM
            'k' => "i".to_string(),         // Images
            _ => "u".to_string(),           // Unknown
        }
    }

    /// Determine media type from MARC21 leader
    fn determine_media_type_marc21(&self, record: &MarcRecord) -> String {
        let leader = &record.leader;
        if leader.len() < 7 {
            return "u".to_string();
        }

        let type_code = leader.chars().nth(6).unwrap_or('a');
        match type_code {
            'a' | 't' => "b".to_string(),  // Language material
            'c' | 'd' => "bc".to_string(), // Notated music
            'g' => "v".to_string(),         // Projected medium (video)
            'i' | 'j' => "a".to_string(),  // Sound recording
            'm' => "c".to_string(),         // Computer file
            'k' => "i".to_string(),         // Still image
            _ => "u".to_string(),           // Unknown
        }
    }

    /// Determine audience from UNIMARC 100$a position 17-19
    fn determine_audience_unimarc(&self, record: &MarcRecord) -> Option<i16> {
        record
            .get_subfield("100", 'a')
            .and_then(|cf| {
                if cf.len() >= 18 {
                    cf.chars().nth(17)
                } else {
                    None
                }
            })
            .map(|c| match c {
                'a' | 'b' | 'c' | 'd' | 'e' => 106, // Children/Youth
                _ => 97, // Adult
            })
    }

    /// Determine audience from MARC21 008 position 22
    fn determine_audience_marc21(&self, record: &MarcRecord) -> Option<i16> {
        record
            .get_control_field("008")
            .and_then(|cf| {
                if cf.len() >= 23 {
                    cf.chars().nth(22)
                } else {
                    None
                }
            })
            .map(|c| match c {
                'a' | 'b' | 'c' | 'd' | 'j' => 106, // Juvenile
                _ => 97, // General/Adult
            })
    }
}

/// Normalize ISBN by removing hyphens and spaces
fn normalize_isbn(isbn: &str) -> String {
    isbn.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// Clean title by removing trailing punctuation
fn clean_title(title: &str) -> String {
    title.trim_end_matches(|c| c == '/' || c == ':' || c == ';' || c == ' ').to_string()
}

/// Clean publisher name
fn clean_publisher(publisher: &str) -> String {
    publisher.trim_end_matches(',').trim().to_string()
}

/// Clean place of publication
fn clean_place(place: &str) -> String {
    place.trim_end_matches(|c| c == ':' || c == ';' || c == ' ').to_string()
}

/// Parse author name in "Lastname, Firstname" format
fn parse_author_name(name: &str) -> (String, Option<String>) {
    if let Some(pos) = name.find(',') {
        let lastname = name[..pos].trim().to_string();
        let firstname = name[pos + 1..].trim().to_string();
        (lastname, if firstname.is_empty() { None } else { Some(firstname) })
    } else {
        (name.trim().to_string(), None)
    }
}

/// Extract volume number from series volume string
fn extract_volume_number(vol: &str) -> Option<i16> {
    vol.chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
}

/// Convert language code to internal ID
fn language_code_to_id(code: &str) -> Option<i16> {
    match code.to_lowercase().as_str() {
        "fre" | "fra" => Some(1),
        "eng" => Some(2),
        "ger" | "deu" => Some(3),
        "jpn" => Some(4),
        "spa" => Some(5),
        "por" => Some(6),
        _ => Some(0), // Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_isbn() {
        assert_eq!(normalize_isbn("978-2-07-040850-4"), "9782070408504");
        assert_eq!(normalize_isbn("2 07 040850 4"), "2070408504");
    }

    #[test]
    fn test_parse_author_name() {
        let (last, first) = parse_author_name("Tolkien, J.R.R.");
        assert_eq!(last, "Tolkien");
        assert_eq!(first, Some("J.R.R.".to_string()));

        let (last, first) = parse_author_name("Anonymous");
        assert_eq!(last, "Anonymous");
        assert_eq!(first, None);
    }

    #[test]
    fn test_language_code() {
        assert_eq!(language_code_to_id("fre"), Some(1));
        assert_eq!(language_code_to_id("fra"), Some(1));
        assert_eq!(language_code_to_id("eng"), Some(2));
        assert_eq!(language_code_to_id("xxx"), Some(0));
    }
}


