//! MARC to Item translator
//!
//! Translates MARC records (UNIMARC or MARC21) into the internal Item structure.

use marc_rs::{Record as MarcRecord, DataField};
use crate::models::{
    author::AuthorWithFunction,
    item::{Collection, Edition, Item, Serie},
};

fn has_tag(record: &MarcRecord, tag: &str) -> bool {
    record.data_fields.iter().any(|f| f.tag == tag)
}

/// Translate UNIMARC record
fn translate_unimarc(record: &MarcRecord) -> Item {
        // Extract all ISBNs from 010$a (UNIMARC) - concatenate all valid ISBNs
        let isbn = {
            let isbns: Vec<String> = get_all_subfields(record, "010", 'a')
                .iter()
                .filter_map(|isbn_str| {
                    let normalized = normalize_isbn(isbn_str);
                    if !normalized.is_empty() {
                        Some(normalized)
                    } else {
                        None
                    }
                })
                .collect();
            if isbns.is_empty() {
                None
            } else {
                Some(isbns.join(", "))
            }
        };
        let title1 = get_subfield(record, "200", 'a').map(String::from);
        let title2 = get_subfield(record, "200", 'e').map(String::from);
        let publication_date = get_subfield(record, "210", 'd').map(String::from);
        let nb_pages = get_subfield(record, "215", 'a').map(String::from);

        // Authors from 700, 701, 702 fields
        let mut authors1 = Vec::new();
        for tag in ["700", "701", "702"] {
            for field in get_fields(record, tag) {
                if let (Some(lastname), Some(firstname)) = (
                    get_datafield_subfield(field, 'a'),
                    get_datafield_subfield(field, 'b'),
                ) {
                    authors1.push(AuthorWithFunction {
                        id: 0,
                        lastname: Some(lastname.to_string()),
                        firstname: Some(firstname.to_string()),
                        bio: None,
                        notes: None,
                        function: get_datafield_subfield(field, '4').map(String::from),
                    });
                }
            }
        }

        // Edition from 210
        let edition = if let Some(publisher) = get_subfield(record, "210", 'c') {
            Some(Edition {
                id: None,
                name: Some(publisher.to_string()),
                place: get_subfield(record, "210", 'a').map(String::from),
                date: get_subfield(record, "210", 'd').map(String::from),
            })
        } else {
            None
        };

        // Series from 225
        let serie = if let Some(series_title) = get_subfield(record, "225", 'a') {
            Some(Serie {
                id: None,
                key: None,
                name: Some(series_title.to_string()),
                issn: get_subfield(record, "225", 'x').map(String::from),
            })
        } else {
            None
        };

        // Volume number goes to Item, not Serie
        let serie_vol_number = get_subfield(record, "225", 'v')
            .and_then(|v| extract_volume_number(v));

        // Collection from 410
        let collection = if let Some(coll_title) = get_subfield(record, "410", 't') {
            Some(Collection {
                id: None,
                key: None,
                title1: Some(coll_title.to_string()),
                title2: None,
                title3: None,
                issn: get_subfield(record, "410", 'x').map(String::from),
            })
        } else {
            None
        };

        // Collection volume number from 410$v
        let collection_vol_number = get_subfield(record, "410", 'v')
            .and_then(|v| extract_volume_number(v));

        // Media type from leader position 6
        let media_type = determine_media_type_unimarc(record);

        // Language from 101$a
        let lang = get_subfield(record, "101", 'a')
            .and_then(|l| language_code_to_id(l));

        Item {
            id: None,
            serie_id: None,
            serie_vol_number,
            edition_id: None,
            collection_id: None,
            collection_number_sub: None,
            collection_vol_number,
            media_type: Some(media_type),
            isbn: isbn,
            price: None,
            barcode: None,
            dewey: get_subfield(record, "676", 'a').map(String::from),
            publication_date,
            lang,
            lang_orig: None,
            title1: Some(title1.unwrap_or_default()),
            title2,
            title3: None,
            title4: None,
            genre: None,
            subject: get_subfield(record, "606", 'a').map(String::from),
            public_type: determine_audience_unimarc(record),
            nb_pages,
            format: get_subfield(record, "215", 'd').map(String::from),
            content: None,
            addon: get_subfield(record, "215", 'e').map(String::from),
            abstract_: get_subfield(record, "330", 'a').map(String::from),
            notes: get_subfield(record, "300", 'a').map(String::from),
            keywords: get_subfield(record, "610", 'a').map(String::from),
            state: None,
            is_archive: None,
            is_valid: Some(1),
            lifecycle_status: 0,
            crea_date: None,
            modif_date: None,
            archived_date: None,
            authors1,
            authors2: Vec::new(),
            authors3: Vec::new(),
            serie,
            collection,
            edition,
            specimens: Vec::new(),
        }
}

/// Translate MARC21 record
fn translate_marc21(record: &MarcRecord) -> Item {
        // Extract all ISBNs from 020$a (MARC21) - concatenate all valid ISBNs
        let isbn = {
            let isbns: Vec<String> = get_all_subfields(record, "020", 'a')
                .iter()
                .filter_map(|isbn_str| {
                    let normalized = normalize_isbn(isbn_str);
                    if !normalized.is_empty() {
                        Some(normalized)
                    } else {
                        None
                    }
                })
                .collect();
            if isbns.is_empty() {
                None
            } else {
                Some(isbns.join(", "))
            }
        };
        let title1 = get_subfield(record, "245", 'a').map(clean_title);
        let title2 = get_subfield(record, "245", 'b').map(clean_title);
        let publication_date = get_subfield(record, "260", 'c').map(String::from);
        let nb_pages = get_subfield(record, "300", 'a').map(String::from);

        // Main author from 100
        let mut authors1 = Vec::new();
        if let Some(author_name) = get_subfield(record, "100", 'a') {
            let (lastname, firstname) = parse_author_name(author_name);
            authors1.push(AuthorWithFunction {
                id: 0,
                lastname: Some(lastname),
                firstname,
                bio: None,
                notes: None,
                function: Some("70".to_string()), // Author
            });
        }

        // Additional authors from 700
        for field in get_fields(record, "700") {
            if let Some(author_name) = get_datafield_subfield(field, 'a') {
                let (lastname, firstname) = parse_author_name(author_name);
                authors1.push(AuthorWithFunction {
                    id: 0,
                    lastname: Some(lastname),
                    firstname,
                    bio: None,
                    notes: None,
                    function: get_datafield_subfield(field, 'e').map(String::from),
                });
            }
        }

        // Edition from 260
        let edition = if let Some(publisher) = get_subfield(record, "260", 'b') {
            Some(Edition {
                id: None,
                name: Some(clean_publisher(publisher)),
                place: get_subfield(record, "260", 'a').map(|p| clean_place(p)),
                date: get_subfield(record, "260", 'c').map(String::from),
            })
        } else {
            None
        };

        // Series from 490
        let mut serie = if let Some(series_title) = get_subfield(record, "490", 'a') {
            Some(Serie {
                id: None,
                key: None,
                name: Some(series_title.to_string()),
                issn: get_subfield(record, "490", 'x').map(String::from),
            })
        } else {
            None
        };

        // Volume number goes to Item, not Serie
        let serie_vol_number = get_subfield(record, "490", 'v')
            .and_then(|v| extract_volume_number(v));

        // Override serie name with authorized form from 830 if available
        if let Some(uniform_title) = get_subfield(record, "830", 'a') {
            if let Some(ref mut s) = serie {
                s.name = Some(uniform_title.to_string());
                // Use ISSN from 830 if not already set
                if s.issn.is_none() {
                    s.issn = get_subfield(record, "830", 'x').map(String::from);
                }
            } else {
                serie = Some(Serie {
                    id: None,
                    key: None,
                    name: Some(uniform_title.to_string()),
                    issn: get_subfield(record, "830", 'x').map(String::from),
                });
            }
        }

        // Media type from leader
        let media_type = determine_media_type_marc21(record);

        // Language from 008 positions 35-37
        let lang = get_control_field(record, "008")
            .and_then(|cf| {
                if cf.len() >= 38 {
                    Some(&cf[35..38])
                } else {
                    None
                }
            })
            .and_then(language_code_to_id);

        Item {
            id: None,
            serie_id: None,
            serie_vol_number,
            edition_id: None,
            collection_id: None,
            collection_number_sub: None,
            collection_vol_number: None,
            media_type: Some(media_type),
            isbn: isbn,
            price: None,
            barcode: None,
            dewey: get_subfield(record, "082", 'a').map(String::from),
            publication_date,
            lang,
            lang_orig: None,
            title1: Some(title1.unwrap_or_default()),
            title2,
            title3: None,
            title4: None,
            genre: None,
            subject: get_subfield(record, "650", 'a').map(String::from),
            public_type: determine_audience_marc21(record),
            nb_pages,
            format: get_subfield(record, "300", 'c').map(String::from),
            content: None,
            addon: get_subfield(record, "300", 'e').map(String::from),
            abstract_: get_subfield(record, "520", 'a').map(String::from),
            notes: get_subfield(record, "500", 'a').map(String::from),
            keywords: get_all_subfields(record, "653", 'a')
                .join(", ")
                .into(),
            state: None,
            is_archive: None,
            is_valid: Some(1),
            lifecycle_status: 0,
            crea_date: None,
            modif_date: None,
            archived_date: None,
            authors1,
            authors2: Vec::new(),
            authors3: Vec::new(),
            serie,
            collection: None,
            edition,
            specimens: Vec::new(),
        }
}

/// Determine media type from UNIMARC leader
fn determine_media_type_unimarc(record: &MarcRecord) -> String {
        let type_code = record.leader.record_type;
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
fn determine_media_type_marc21(record: &MarcRecord) -> String {
        let type_code = record.leader.record_type;
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
fn determine_audience_unimarc(record: &MarcRecord) -> Option<i16> {
        get_subfield(record, "100", 'a')
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
fn determine_audience_marc21(record: &MarcRecord) -> Option<i16> {
        get_control_field(record, "008")
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

impl From<MarcRecord> for Item {
    fn from(record: MarcRecord) -> Self {
        // Heuristic: UNIMARC records usually have 200, MARC21 usually has 245.
        // Default to MARC21 if ambiguous.
        if has_tag(&record, "200") && !has_tag(&record, "245") {
            translate_unimarc(&record)
        } else {
            translate_marc21(&record)
        }
    }
}

/// Get a subfield value by tag and subfield code
fn get_subfield<'a>(record: &'a MarcRecord, tag: &str, code: char) -> Option<&'a str> {
    record.data_fields
        .iter()
        .find(|f| f.tag == tag)
        .and_then(|f| get_datafield_subfield(f, code))
}

/// Get all subfield values for a tag and code
fn get_all_subfields<'a>(record: &'a MarcRecord, tag: &str, code: char) -> Vec<&'a str> {
    record.data_fields
        .iter()
        .filter(|f| f.tag == tag)
        .flat_map(|f| f.subfields.iter()
            .filter(|sf| sf.code == code)
            .map(|sf| sf.value.as_str()))
        .collect()
}

/// Get all data fields with a specific tag
fn get_fields<'a>(record: &'a MarcRecord, tag: &str) -> Vec<&'a DataField> {
    record.data_fields
        .iter()
        .filter(|f| f.tag == tag)
        .collect()
}

/// Get a subfield value from a data field
fn get_datafield_subfield(field: &DataField, code: char) -> Option<&str> {
    field.subfields
        .iter()
        .find(|sf| sf.code == code)
        .map(|sf| sf.value.as_str())
}

/// Get a control field value
fn get_control_field<'a>(record: &'a MarcRecord, tag: &str) -> Option<&'a str> {
    record.control_fields
        .iter()
        .find(|f| f.tag == tag)
        .map(|f| f.value.as_str())
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

    #[test]
    fn test_extract_volume_number() {
        assert_eq!(extract_volume_number("1"), Some(1));
        assert_eq!(extract_volume_number("vol. 5"), Some(5));
        assert_eq!(extract_volume_number("tome 12"), Some(12));
        assert_eq!(extract_volume_number("no. 3"), Some(3));
        assert_eq!(extract_volume_number("abc"), None);
        assert_eq!(extract_volume_number(""), None);
    }
}
