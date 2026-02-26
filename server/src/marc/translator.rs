//! MARC to Item translator
//!
//! Translates MARC records (UNIMARC or MARC21) into the internal Item structure
//! using marc-rs typed field structures.
//!
//! Field values in marc-rs may be enums (e.g. Leader positions, indicators);
//! use `.into()` or `char::from()` / library conversion to get char or int when needed.

use z3950_rs::marc_rs::fields::common::NoteData;
use z3950_rs::marc_rs::fields::*;
use z3950_rs::marc_rs::{MarcFormat, Record as MarcRecord};

use crate::models::{
    author::AuthorWithFunction,
    item::{Collection, Edition, Item, Serie},
    specimen::Specimen,
};

/// Detect MARC format from record control fields (008 = MARC21, 009 = UNIMARC).
fn detect_format(record: &MarcRecord) -> MarcFormat {
    let has_008 = record
        .control()
        .iter()
        .any(|c| matches!(c, Control::FixedLengthDataElements(_)));
    let has_009 = record
        .control()
        .iter()
        .any(|c| matches!(c, Control::LocalControlNumber(_)));
    if has_009 && !has_008 {
        MarcFormat::Unimarc
    } else {
        MarcFormat::Marc21
    }
}

/// Get first subfield value from other_data by tag and code.
fn other_subfield(record: &MarcRecord, tag: &str, code: char) -> Option<String> {
    record
        .other_data()
        .iter()
        .find(|f| f.tag == tag)
        .and_then(|f| {
            f.subfields
                .iter()
                .find(|sf| sf.code == code)
                .map(|sf| sf.value.clone())
        })
}

/// Get all subfield values from other_data by tag and code.
fn other_subfields_all(record: &MarcRecord, tag: &str, code: char) -> Vec<String> {
    record
        .other_data()
        .iter()
        .filter(|f| f.tag == tag)
        .flat_map(|f| {
            f.subfields
                .iter()
                .filter(|sf| sf.code == code)
                .map(|sf| sf.value.clone())
        })
        .collect()
}

/// Get value from Control::FixedLengthDataElements (008).
fn control_008(record: &MarcRecord) -> Option<&str> {
    record
        .control()
        .iter()
        .find_map(|c| match c {
            Control::FixedLengthDataElements(_) => Some(c.value()),
            _ => None,
        })
}

/// Get value from other_control by tag (e.g. UNIMARC 100).
fn other_control_value<'a>(record: &'a MarcRecord, tag: &str) -> Option<&'a str> {
    record
        .other_control()
        .iter()
        .find(|f| f.tag == tag)
        .map(|f| f.value.as_str())
}

fn normalize_isbn(isbn: &str) -> String {
    isbn.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn clean_title(title: &str) -> String {
    title
        .trim_end_matches(|c| c == '/' || c == ':' || c == ';' || c == ' ')
        .to_string()
}

fn clean_publisher(publisher: &str) -> String {
    publisher.trim_end_matches(',').trim().to_string()
}

fn clean_place(place: &str) -> String {
    place
        .trim_end_matches(|c| c == ':' || c == ';' || c == ' ')
        .to_string()
}

/// Parse author name in "Lastname, Firstname" format (MARC21 100/700).
fn parse_author_name(name: &str) -> (String, Option<String>) {
    if let Some(pos) = name.find(',') {
        let lastname = name[..pos].trim().to_string();
        let firstname = name[pos + 1..].trim().to_string();
        (
            lastname,
            if firstname.is_empty() {
                None
            } else {
                Some(firstname)
            },
        )
    } else {
        (name.trim().to_string(), None)
    }
}

fn extract_volume_number(vol: &str) -> Option<i16> {
    vol.chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
}

fn language_code_to_id(code: &str) -> Option<i16> {
    match code.to_lowercase().as_str() {
        "fre" | "fra" => Some(1),
        "eng" => Some(2),
        "ger" | "deu" => Some(3),
        "jpn" => Some(4),
        "spa" => Some(5),
        "por" => Some(6),
        _ => Some(0),
    }
}

/// Get record type as char (Leader position 6). Works whether marc-rs uses char or RecordType enum.
fn record_type_as_char(record: &MarcRecord) -> char {
    let rt = record.leader().record_type;
    char::from(rt)
}

/// Audience from MARC21 008 position 22.
fn audience_marc21(record: &MarcRecord) -> Option<i16> {
    control_008(record).and_then(|cf| {
        if cf.len() >= 23 {
            cf.chars().nth(22)
        } else {
            None
        }
    }).map(|c| match c {
        'a' | 'b' | 'c' | 'd' | 'j' => 106,
        _ => 97,
    })
}

/// Audience from UNIMARC 100$a position 17 (via other_control "100").
fn audience_unimarc(record: &MarcRecord) -> Option<i16> {
    other_control_value(record, "100").and_then(|cf| {
        if cf.len() >= 18 {
            cf.chars().nth(17)
        } else {
            None
        }
    }).map(|c| match c {
        'a' | 'b' | 'c' | 'd' | 'e' => 106,
        _ => 97,
    })
}

/// Get note text from NoteData (first $a).
fn note_text(d: &NoteData) -> &str {
    d.text.as_str()
}

impl From<MarcRecord> for Item {
    fn from(record: MarcRecord) -> Self {
        let format = detect_format(&record);
        let is_marc21 = format == MarcFormat::Marc21;

        // Price: first typed ISBN price_or_acquisition (020$c MARC21, 010$d UNIMARC)
        let price = record
            .isbns()
            .first()
            .and_then(|i| i.price_or_acquisition.clone());

        // ISBN: from typed isbns() first, then fallback to other_data 020/010
        let isbn = {
            let from_typed: Vec<String> = record
                .isbns()
                .iter()
                .map(|i| i.sanitized_number())
                .filter(|n| !n.is_empty())
                .collect();
            let isbns: Vec<String> = if from_typed.is_empty() {
                let isbn_tag = if is_marc21 { "020" } else { "010" };
                other_subfields_all(&record, isbn_tag, 'a')
                    .iter()
                    .filter_map(|s| {
                        let n = normalize_isbn(s);
                        if n.is_empty() {
                            None
                        } else {
                            Some(n)
                        }
                    })
                    .collect()
            } else {
                from_typed
            };
            if isbns.is_empty() {
                None
            } else {
                Some(isbns.join(", "))
            }
        };

        // Title: first TitleStatement; title3/title4 from other Title variants (246, 510, etc.)
        let (title1, title2) = record
            .titles()
            .iter()
            .find_map(|t| {
                if let Title::TitleStatement(ref d) = t {
                    Some((
                        d.title.clone(),
                        d.remainder
                            .clone()
                            .map(|s| if is_marc21 { clean_title(&s) } else { s }),
                    ))
                } else {
                    None
                }
            })
            .unwrap_or((String::new(), None));
        let extra_titles: Vec<String> = record
            .titles()
            .iter()
            .filter_map(|t| {
                match t {
                    Title::VaryingFormOfTitle(d)
                    | Title::FormerTitle(d)
                    | Title::ParallelTitle(d)
                    | Title::OtherTitleInformation(d) => {
                        let s = d.title.clone();
                        let with_remainder = d
                            .remainder
                            .as_ref()
                            .map(|r| format!("{} — {}", s, r))
                            .unwrap_or(s);
                        Some(if is_marc21 {
                            clean_title(&with_remainder)
                        } else {
                            with_remainder
                        })
                    }
                    _ => None,
                }
            })
            .collect();
        let title3 = extra_titles.first().cloned();
        let title4 = extra_titles.get(1).cloned();

        // Authors: main_entries (100/700) + added_entries (700/701/702)
        let mut authors1 = Vec::new();
        for me in record.main_entries() {
            if let MainEntry::PersonalName(ref d) = me {
                let (lastname, firstname) = if is_marc21 {
                    parse_author_name(&d.name)
                } else {
                    (d.name.clone(), d.numeration.clone().or(d.titles.clone()))
                };
                authors1.push(AuthorWithFunction {
                    id: 0,
                    lastname: Some(lastname),
                    firstname,
                    bio: None,
                    notes: None,
                    function: if is_marc21 {
                        Some("70".to_string())
                    } else {
                        d.relator_code.clone()
                    },
                });
            }
        }
        for ae in record.added_entries() {
            if let AddedEntry::PersonalName(ref d) = ae {
                let (lastname, firstname) = if is_marc21 {
                    parse_author_name(&d.name)
                } else {
                    (d.name.clone(), d.numeration.clone().or(d.titles.clone()))
                };
                authors1.push(AuthorWithFunction {
                    id: 0,
                    lastname: Some(lastname),
                    firstname,
                    bio: None,
                    notes: None,
                    function: if is_marc21 {
                        d.relator_term.clone()
                    } else {
                        d.relator_code.clone()
                    },
                });
            }
        }
        // Corporate/Meeting as single string in lastname
        for me in record.main_entries() {
            match me {
                MainEntry::CorporateName(d) => {
                    authors1.push(AuthorWithFunction {
                        id: 0,
                        lastname: Some(d.name.clone()),
                        firstname: d.subordinate_unit.clone(),
                        bio: None,
                        notes: None,
                        function: d.relator_code.clone(),
                    });
                }
                MainEntry::MeetingName(d) => {
                    authors1.push(AuthorWithFunction {
                        id: 0,
                        lastname: Some(d.name.clone()),
                        firstname: d.subordinate_unit.clone(),
                        bio: None,
                        notes: None,
                        function: None,
                    });
                }
                _ => {}
            }
        }
        for ae in record.added_entries() {
            match ae {
                AddedEntry::CorporateName(d) => {
                    authors1.push(AuthorWithFunction {
                        id: 0,
                        lastname: Some(d.name.clone()),
                        firstname: d.subordinate_unit.clone(),
                        bio: None,
                        notes: None,
                        function: d.relator_code.clone(),
                    });
                }
                AddedEntry::MeetingName(d) => {
                    authors1.push(AuthorWithFunction {
                        id: 0,
                        lastname: Some(d.name.clone()),
                        firstname: d.subordinate_unit.clone(),
                        bio: None,
                        notes: None,
                        function: None,
                    });
                }
                _ => {}
            }
        }

        // Edition and publication: from record.edition_info() (250/260/264 MARC21, 205/210 UNIMARC)
        let edition_info = record.edition_info();
        let place = edition_info.place.map(|s| clean_place(&s));
        let publisher = edition_info.publisher.map(|s| clean_publisher(&s));
        let publication_date = edition_info.date.map(String::from);
        let edition = publisher.map(|name| Edition {
            id: None,
            name: Some(name),
            place,
            date: publication_date.clone(),
        });

        // Series: SeriesStatement / SeriesTitle
        let (serie, serie_vol_number) = record
            .series()
            .iter()
            .find_map(|s| {
                match s {
                    Series::SeriesStatement(d) | Series::SeriesTitle(d) => Some((
                        Serie {
                            id: None,
                            key: None,
                            name: Some(d.statement.clone()),
                            issn: d.issn.clone(),
                        },
                        d.volume
                            .as_deref()
                            .and_then(extract_volume_number),
                    )),
                    _ => None,
                }
            })
            .unwrap_or((
                Serie {
                    id: None,
                    key: None,
                    name: None,
                    issn: None,
                },
                None,
            ));
        let serie_vol_number = serie_vol_number.or_else(|| {
            record.series().iter().find_map(|s| match s {
                Series::SeriesStatement(d) | Series::SeriesTitle(d) => {
                    d.volume.as_deref().and_then(extract_volume_number)
                }
                _ => None,
            })
        });

        // Override series with 830 (MARC21) from other_data if present
        let (serie, serie_vol_number) = if is_marc21 {
            if let Some(uniform) = other_subfield(&record, "830", 'a') {
                let mut s = serie;
                s.name = Some(uniform);
                if s.issn.is_none() {
                    s.issn = other_subfield(&record, "830", 'x');
                }
                (Some(s), serie_vol_number)
            } else if serie.name.is_some() {
                (Some(serie), serie_vol_number)
            } else {
                (None, serie_vol_number)
            }
        } else {
            (if serie.name.is_some() { Some(serie) } else { None }, serie_vol_number)
        };

        // Collection (UNIMARC 410): from linking or other_data
        let (collection, collection_vol_number) = if is_marc21 {
            (None, None)
        } else {
            let title = record
                .linking()
                .iter()
                .find_map(|l| {
                    if let Linking::MainSeriesEntry(d) = l {
                        d.title.clone()
                    } else {
                        None
                    }
                })
                .or_else(|| other_subfield(&record, "410", 't'));
            let coll = title.map(|title1| Collection {
                id: None,
                key: None,
                title1: Some(title1),
                title2: None,
                title3: None,
                issn: other_subfield(&record, "410", 'x'),
            });
            let vol = other_subfield(&record, "410", 'v').and_then(|v| extract_volume_number(&v));
            (coll, vol)
        };

        // Physical: extent, dimensions, accompanying
        let (nb_pages, format, addon) = record
            .physical()
            .iter()
            .find_map(|p| {
                if let Physical::PhysicalDescription(ref d) = p {
                    Some((
                        Some(d.extent.clone()),
                        d.dimensions.clone(),
                        d.accompanying_material.clone(),
                    ))
                } else {
                    None
                }
            })
            .unwrap_or((None, None, None));

        // Notes: Summary -> abstract_, GeneralNote -> notes, FormattedContentsNote (505) -> content
        let abstract_ = record.notes().iter().find_map(|n| {
            if let Note::Summary(d) = n {
                Some(note_text(d).to_string())
            } else {
                None
            }
        });
        let notes = record.notes().iter().find_map(|n| {
            if let Note::GeneralNote(d) = n {
                Some(note_text(d).to_string())
            } else {
                None
            }
        });
        let content = record.notes().iter().find_map(|n| {
            if let Note::FormattedContentsNote(d) = n {
                Some(note_text(d).to_string())
            } else {
                None
            }
        });

        // Subjects: topical term -> subject, uncontrolled -> keywords
        let subject = record.subjects().iter().find_map(|s| {
            if let Subject::SubjectTopicalTerm(d) = s {
                Some(d.term.clone())
            } else if let Subject::IndexTermUncontrolled(_) = s {
                None
            } else {
                None
            }
        });
        let keywords = {
            let kws: Vec<String> = record
                .subjects()
                .iter()
                .filter_map(|s| {
                    if let Subject::IndexTermUncontrolled(d) = s {
                        Some(d.term.clone())
                    } else if let Subject::SubjectTopicalTerm(d) = s {
                        Some(d.term.clone())
                    } else {
                        None
                    }
                })
                .collect();
            if kws.is_empty() {
                None
            } else {
                Some(kws.join(", "))
            }
        };
        let subject = subject.or_else(|| keywords.clone());

        // Dewey: from typed classifications() first, then other_data 082/676
        let dewey = record
            .dewey()
            .map(String::from)
            .or_else(|| {
                let dewey_tag = if is_marc21 { "082" } else { "676" };
                other_subfield(&record, dewey_tag, 'a')
            });

        // Language: record.language_codes() first, then 008 (MARC21) or other_control 101 (UNIMARC)
        let lang_codes = record.language_codes();
        let lang = lang_codes
            .first()
            .and_then(|s| language_code_to_id(s))
            .or_else(|| {
                if is_marc21 {
                    control_008(&record)
                        .and_then(|cf| if cf.len() >= 38 { Some(&cf[35..38]) } else { None })
                        .and_then(language_code_to_id)
                } else {
                    other_subfield(&record, "101", 'a')
                        .as_ref()
                        .and_then(|s| language_code_to_id(s))
                }
            });
        let lang_orig = lang_codes
            .get(1)
            .and_then(|s| language_code_to_id(s));

        let marc_format = if is_marc21 {
            MarcFormat::Marc21
        } else {
            MarcFormat::Unimarc
        };
        let media_type = crate::repository::items::record_type_to_media_type_db(
            record_type_as_char(&record),
            marc_format,
        );
        let public_type = if is_marc21 {
            audience_marc21(&record)
        } else {
            audience_unimarc(&record)
        };

        Item {
            id: None,
            serie_id: None,
            serie_vol_number,
            edition_id: None,
            collection_id: None,
            collection_number_sub: None,
            collection_vol_number,
            media_type: Some(media_type),
            isbn,
            price,
            barcode: None,
            dewey,
            publication_date,
            lang,
            lang_orig,
            title1: Some(if title1.is_empty() {
                "".to_string()
            } else if is_marc21 {
                clean_title(&title1)
            } else {
                title1
            }),
            title2,
            title3,
            title4,
            genre: None,
            subject,
            public_type,
            nb_pages,
            format,
            content,
            addon,
            abstract_,
            notes,
            keywords,
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
            specimens: record
                .specimens()
                .iter()
                .map(marc_specimen_to_specimen)
                .collect(),
        }
    }
}

/// Map marc_rs specimen (995/952) to our Specimen model (preview, id=0).
fn marc_specimen_to_specimen(s: &z3950_rs::marc_rs::fields::Specimen) -> Specimen {
    let notes = match (&s.section, &s.document_type) {
        (Some(sec), Some(doc)) => Some(format!("{} — {}", sec, doc)),
        (Some(sec), None) => Some(sec.clone()),
        (None, Some(doc)) => Some(doc.clone()),
        (None, None) => None,
    };
    Specimen {
        id: 0,
        id_item: None,
        source_id: None,
        barcode: s.barcode.clone(),
        call_number: s.call_number.clone(),
        place: None,
        status: Some(98), // Borrowable
        codestat: None,
        notes,
        price: None,
        crea_date: None,
        modif_date: None,
        archive_date: None,
        lifecycle_status: 0,
        source_name: s.library.clone(),
        availability: Some(0),
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
