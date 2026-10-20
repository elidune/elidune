//! Patron-facing labels inserted into notice emails.
//!
//! French, English, German, and Spanish each have a translation. Every other
//! [`Language`], and a missing preference, uses French.
//!
//! Notice senders pass the language of the template they loaded (reader
//! preference when that template exists, otherwise the library language,
//! otherwise French) so a label is never in a different language from the body.

use crate::models::Language;

/// Short label interpolated into a notice (hold ready, overdue, inventory, events).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Label {
    Barcode,
    UnknownTitle,
    NotAvailable,
    Title,
    Authors,
    Borrowed,
    DueDate,
    /// Inline verb in the plain-text loan list ("borrowed" / "emprunté le").
    BorrowedOn,
    /// Inline verb in the plain-text loan list ("due" / "à rendre le").
    DueOn,
    /// Event announcement intro, including its colon and spacing.
    InvitationConcerns,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatronLanguage {
    French,
    English,
    German,
    Spanish,
}

/// Resolve a stored preference to one of the translated notice languages.
fn patron_language(lang: Option<Language>) -> PatronLanguage {
    match lang.unwrap_or(Language::French) {
        Language::English => PatronLanguage::English,
        Language::German => PatronLanguage::German,
        Language::Spanish => PatronLanguage::Spanish,
        _ => PatronLanguage::French,
    }
}

/// Translated label. French is the fallback for languages without a catalog.
#[must_use]
pub fn label(lang: Option<Language>, key: Label) -> &'static str {
    match patron_language(lang) {
        PatronLanguage::French => french(key),
        PatronLanguage::English => english(key),
        PatronLanguage::German => german(key),
        PatronLanguage::Spanish => spanish(key),
    }
}

/// `Label: value`. French keeps a space before the colon, except the reminder
/// phrases `emprunté le` and `à rendre le`, which take the date directly.
#[must_use]
pub fn labeled(lang: Option<Language>, key: Label, value: &str) -> String {
    let name = label(lang, key);
    if patron_language(lang) == PatronLanguage::French {
        if matches!(key, Label::BorrowedOn | Label::DueOn) {
            format!("{name} {value}")
        } else {
            format!("{name} : {value}")
        }
    } else {
        format!("{name}: {value}")
    }
}

fn french(key: Label) -> &'static str {
    match key {
        Label::Barcode => "Code-barres",
        Label::UnknownTitle => "(titre inconnu)",
        Label::NotAvailable => "n. d.",
        Label::Title => "Titre",
        Label::Authors => "Auteur(s)",
        Label::Borrowed => "Emprunté le",
        Label::DueDate => "Date de retour",
        Label::BorrowedOn => "emprunté le",
        Label::DueOn => "à rendre le",
        Label::InvitationConcerns => "Cette invitation concerne :",
    }
}

fn english(key: Label) -> &'static str {
    match key {
        Label::Barcode => "Barcode",
        Label::UnknownTitle => "(unknown title)",
        Label::NotAvailable => "N/A",
        Label::Title => "Title",
        Label::Authors => "Author(s)",
        Label::Borrowed => "Borrowed",
        Label::DueDate => "Due date",
        Label::BorrowedOn => "borrowed",
        Label::DueOn => "due",
        Label::InvitationConcerns => "This invitation concerns:",
    }
}

fn german(key: Label) -> &'static str {
    match key {
        Label::Barcode => "Barcode",
        Label::UnknownTitle => "(unbekannter Titel)",
        Label::NotAvailable => "k. A.",
        Label::Title => "Titel",
        Label::Authors => "Autor(en)",
        Label::Borrowed => "Ausgeliehen",
        Label::DueDate => "Fälligkeitsdatum",
        Label::BorrowedOn => "ausgeliehen",
        Label::DueOn => "fällig",
        Label::InvitationConcerns => "Diese Einladung betrifft:",
    }
}

fn spanish(key: Label) -> &'static str {
    match key {
        Label::Barcode => "Código de barras",
        Label::UnknownTitle => "(título desconocido)",
        Label::NotAvailable => "N/D",
        Label::Title => "Título",
        Label::Authors => "Autor(es)",
        Label::Borrowed => "Prestado",
        Label::DueDate => "Fecha de devolución",
        Label::BorrowedOn => "prestado",
        Label::DueOn => "devolución",
        Label::InvitationConcerns => "Esta invitación se refiere a:",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: [Label; 10] = [
        Label::Barcode,
        Label::UnknownTitle,
        Label::NotAvailable,
        Label::Title,
        Label::Authors,
        Label::Borrowed,
        Label::DueDate,
        Label::BorrowedOn,
        Label::DueOn,
        Label::InvitationConcerns,
    ];

    #[test]
    fn every_supported_language_has_a_label() {
        for key in KEYS {
            for lang in [Language::French, Language::English, Language::German, Language::Spanish] {
                assert!(!label(Some(lang), key).is_empty(), "{lang:?} {key:?}");
            }
        }
    }

    #[test]
    fn barcode_labels_are_localized() {
        assert_eq!(label(Some(Language::French), Label::Barcode), "Code-barres");
        assert_eq!(label(Some(Language::English), Label::Barcode), "Barcode");
        assert_eq!(label(Some(Language::German), Label::Barcode), "Barcode");
        assert_eq!(label(Some(Language::Spanish), Label::Barcode), "Código de barras");
        assert_eq!(labeled(Some(Language::French), Label::Barcode, "ABC-1"), "Code-barres : ABC-1");
        assert_eq!(labeled(Some(Language::English), Label::Barcode, "ABC-1"), "Barcode: ABC-1");
        assert_eq!(labeled(Some(Language::Spanish), Label::Barcode, "ABC-1"), "Código de barras: ABC-1");
        // German is translated (French is the fallback); "Barcode" matches the UI catalog.
        assert_ne!(label(Some(Language::German), Label::Barcode), label(Some(Language::French), Label::Barcode));
    }

    #[test]
    fn missing_and_untranslated_languages_use_french() {
        assert_eq!(label(None, Label::InvitationConcerns), "Cette invitation concerne :");
        for lang in [Language::Unknown, Language::Italian, Language::Portuguese] {
            assert_eq!(label(Some(lang), Label::Barcode), "Code-barres", "{lang:?}");
            assert_eq!(labeled(Some(lang), Label::DueOn, "01/02/2026"), "à rendre le 01/02/2026");
            assert_eq!(labeled(Some(lang), Label::BorrowedOn, "10/01/2026"), "emprunté le 10/01/2026");
        }
    }

    #[test]
    fn language_aliases_resolve_before_lookup() {
        assert_eq!(label(Some(Language::from("deu")), Label::Title), "Titel");
        assert_eq!(label(Some(Language::from("spa")), Label::DueDate), "Fecha de devolución");
        assert_eq!(label(Some(Language::from("eng")), Label::Title), "Title");
        assert_eq!(label(Some(Language::from("fra")), Label::Authors), "Auteur(s)");
    }
}
