//! ISO 3166-1 alpha-2 country codes for `library_info.addr_country`.
//!
//! Free-text values saved before the country select ("France", "france", "états-unis")
//! are mapped to a code when the name is known with certainty. A 2-letter token is
//! kept only when it is a country in the shared zone table (`fr` → `FR`). The
//! non-ISO tokens `uk` and `el` are aliases for GB and GR. Anything else is rejected
//! on update. Migration `043_library_country_iso.sql` applies the same rules and
//! sets every unrecognized `addr_country` to NULL. It does not change the zone column.

use crate::error::{AppError, AppResult};

/// Lowercased French and English names, sorted for binary search.
/// `uk` → GB, `el` → GR. `en` is not mapped: it is a language code, not a country.
pub const COUNTRY_ALIASES: &[(&str, &str)] = &[
    ("afghanistan", "AF"),
    ("afrique du sud", "ZA"),
    ("albania", "AL"),
    ("albanie", "AL"),
    ("algeria", "DZ"),
    ("algerie", "DZ"),
    ("algérie", "DZ"),
    ("allemagne", "DE"),
    ("andorra", "AD"),
    ("andorre", "AD"),
    ("angleterre", "GB"),
    ("angola", "AO"),
    ("antigua and barbuda", "AG"),
    ("antigua et barbuda", "AG"),
    ("antigua-et-barbuda", "AG"),
    ("arabie saoudite", "SA"),
    ("argentina", "AR"),
    ("argentine", "AR"),
    ("armenia", "AM"),
    ("armenie", "AM"),
    ("arménie", "AM"),
    ("australia", "AU"),
    ("australie", "AU"),
    ("austria", "AT"),
    ("autriche", "AT"),
    ("azerbaidjan", "AZ"),
    ("azerbaijan", "AZ"),
    ("azerbaïdjan", "AZ"),
    ("bahamas", "BS"),
    ("bahrain", "BH"),
    ("bahrein", "BH"),
    ("bahreïn", "BH"),
    ("bangladesh", "BD"),
    ("barbade", "BB"),
    ("barbados", "BB"),
    ("belarus", "BY"),
    ("belgique", "BE"),
    ("belgium", "BE"),
    ("belize", "BZ"),
    ("benin", "BJ"),
    ("bhoutan", "BT"),
    ("bhutan", "BT"),
    ("bielorussie", "BY"),
    ("birmanie", "MM"),
    ("biélorussie", "BY"),
    ("bolivia", "BO"),
    ("bolivie", "BO"),
    ("bosnia", "BA"),
    ("bosnia and herzegovina", "BA"),
    ("bosnie herzegovine", "BA"),
    ("bosnie-herzegovine", "BA"),
    ("bosnie-herzégovine", "BA"),
    ("botswana", "BW"),
    ("brazil", "BR"),
    ("bresil", "BR"),
    ("britain", "GB"),
    ("brunei", "BN"),
    ("brésil", "BR"),
    ("bulgaria", "BG"),
    ("bulgarie", "BG"),
    ("burkina", "BF"),
    ("burkina faso", "BF"),
    ("burma", "MM"),
    ("burundi", "BI"),
    ("bénin", "BJ"),
    ("cambodge", "KH"),
    ("cambodia", "KH"),
    ("cameroon", "CM"),
    ("cameroun", "CM"),
    ("canada", "CA"),
    ("cap vert", "CV"),
    ("cap-vert", "CV"),
    ("cape verde", "CV"),
    ("central african republic", "CF"),
    ("chad", "TD"),
    ("chile", "CL"),
    ("chili", "CL"),
    ("china", "CN"),
    ("chine", "CN"),
    ("chypre", "CY"),
    ("colombia", "CO"),
    ("colombie", "CO"),
    ("comores", "KM"),
    ("comoros", "KM"),
    ("congo", "CG"),
    ("congo-brazzaville", "CG"),
    ("congo-kinshasa", "CD"),
    ("coree", "KR"),
    ("coree du nord", "KP"),
    ("coree du sud", "KR"),
    ("corée", "KR"),
    ("corée du nord", "KP"),
    ("corée du sud", "KR"),
    ("costa rica", "CR"),
    ("cote d'ivoire", "CI"),
    ("cote divoire", "CI"),
    ("croatia", "HR"),
    ("croatie", "HR"),
    ("cuba", "CU"),
    ("cyprus", "CY"),
    ("czech republic", "CZ"),
    ("czechia", "CZ"),
    ("côte d'ivoire", "CI"),
    ("danemark", "DK"),
    ("democratic republic of the congo", "CD"),
    ("denmark", "DK"),
    ("djibouti", "DJ"),
    ("dominica", "DM"),
    ("dominican republic", "DO"),
    ("dominique", "DM"),
    ("dr congo", "CD"),
    ("east timor", "TL"),
    ("ecuador", "EC"),
    ("egypt", "EG"),
    ("egypte", "EG"),
    ("el", "GR"),
    ("el salvador", "SV"),
    ("emirats arabes unis", "AE"),
    ("england", "GB"),
    ("equateur", "EC"),
    ("equatorial guinea", "GQ"),
    ("eritrea", "ER"),
    ("erythree", "ER"),
    ("espagne", "ES"),
    ("estonie", "EE"),
    ("eswatini", "SZ"),
    ("etats unis", "US"),
    ("etats-unis", "US"),
    ("etats-unis d'amerique", "US"),
    ("ethiopia", "ET"),
    ("ethiopie", "ET"),
    ("fidji", "FJ"),
    ("fiji", "FJ"),
    ("finland", "FI"),
    ("finlande", "FI"),
    ("france", "FR"),
    ("gabon", "GA"),
    ("gambia", "GM"),
    ("gambie", "GM"),
    ("gb", "GB"),
    ("georgia", "GE"),
    ("georgie", "GE"),
    ("germany", "DE"),
    ("ghana", "GH"),
    ("grande bretagne", "GB"),
    ("grande-bretagne", "GB"),
    ("great britain", "GB"),
    ("grece", "GR"),
    ("greece", "GR"),
    ("grenada", "GD"),
    ("grenade", "GD"),
    ("grèce", "GR"),
    ("guatemala", "GT"),
    ("guinea", "GN"),
    ("guinea-bissau", "GW"),
    ("guinee", "GN"),
    ("guinee equatoriale", "GQ"),
    ("guinee-bissau", "GW"),
    ("guinée", "GN"),
    ("guinée équatoriale", "GQ"),
    ("guinée-bissau", "GW"),
    ("guyana", "GY"),
    ("géorgie", "GE"),
    ("haiti", "HT"),
    ("haïti", "HT"),
    ("holland", "NL"),
    ("honduras", "HN"),
    ("hongrie", "HU"),
    ("hungary", "HU"),
    ("iceland", "IS"),
    ("iles marshall", "MH"),
    ("iles salomon", "SB"),
    ("inde", "IN"),
    ("india", "IN"),
    ("indonesia", "ID"),
    ("indonesie", "ID"),
    ("irak", "IQ"),
    ("iran", "IR"),
    ("iraq", "IQ"),
    ("ireland", "IE"),
    ("irlande", "IE"),
    ("islande", "IS"),
    ("israel", "IL"),
    ("israël", "IL"),
    ("italie", "IT"),
    ("italy", "IT"),
    ("ivory coast", "CI"),
    ("jamaica", "JM"),
    ("jamaique", "JM"),
    ("jamaïque", "JM"),
    ("japan", "JP"),
    ("japon", "JP"),
    ("jordan", "JO"),
    ("jordanie", "JO"),
    ("kazakhstan", "KZ"),
    ("kenya", "KE"),
    ("kirghizistan", "KG"),
    ("kiribati", "KI"),
    ("korea", "KR"),
    ("koweit", "KW"),
    ("koweït", "KW"),
    ("kuwait", "KW"),
    ("kyrgyzstan", "KG"),
    ("laos", "LA"),
    ("latvia", "LV"),
    ("lebanon", "LB"),
    ("lesotho", "LS"),
    ("lettonie", "LV"),
    ("liban", "LB"),
    ("liberia", "LR"),
    ("libya", "LY"),
    ("libye", "LY"),
    ("liechtenstein", "LI"),
    ("lithuania", "LT"),
    ("lituanie", "LT"),
    ("luxembourg", "LU"),
    ("macedoine", "MK"),
    ("macedoine du nord", "MK"),
    ("macédoine", "MK"),
    ("macédoine du nord", "MK"),
    ("madagascar", "MG"),
    ("malaisie", "MY"),
    ("malawi", "MW"),
    ("malaysia", "MY"),
    ("maldives", "MV"),
    ("mali", "ML"),
    ("malta", "MT"),
    ("malte", "MT"),
    ("maroc", "MA"),
    ("marshall islands", "MH"),
    ("maurice", "MU"),
    ("mauritania", "MR"),
    ("mauritanie", "MR"),
    ("mauritius", "MU"),
    ("mexico", "MX"),
    ("mexique", "MX"),
    ("micronesia", "FM"),
    ("micronesie", "FM"),
    ("micronésie", "FM"),
    ("moldavie", "MD"),
    ("moldova", "MD"),
    ("monaco", "MC"),
    ("mongolia", "MN"),
    ("mongolie", "MN"),
    ("montenegro", "ME"),
    ("monténégro", "ME"),
    ("morocco", "MA"),
    ("mozambique", "MZ"),
    ("myanmar", "MM"),
    ("namibia", "NA"),
    ("namibie", "NA"),
    ("nauru", "NR"),
    ("nepal", "NP"),
    ("netherlands", "NL"),
    ("new zealand", "NZ"),
    ("nicaragua", "NI"),
    ("niger", "NE"),
    ("nigeria", "NG"),
    ("north korea", "KP"),
    ("north macedonia", "MK"),
    ("norvege", "NO"),
    ("norvège", "NO"),
    ("norway", "NO"),
    ("nouvelle zelande", "NZ"),
    ("nouvelle-zelande", "NZ"),
    ("nouvelle-zélande", "NZ"),
    ("népal", "NP"),
    ("oman", "OM"),
    ("ouganda", "UG"),
    ("ouzbekistan", "UZ"),
    ("ouzbékistan", "UZ"),
    ("pakistan", "PK"),
    ("palaos", "PW"),
    ("palau", "PW"),
    ("palestine", "PS"),
    ("panama", "PA"),
    ("papouasie-nouvelle-guinee", "PG"),
    ("papouasie-nouvelle-guinée", "PG"),
    ("papua new guinea", "PG"),
    ("paraguay", "PY"),
    ("pays bas", "NL"),
    ("pays-bas", "NL"),
    ("peru", "PE"),
    ("philippines", "PH"),
    ("poland", "PL"),
    ("pologne", "PL"),
    ("portugal", "PT"),
    ("pérou", "PE"),
    ("qatar", "QA"),
    ("rd congo", "CD"),
    ("rdc", "CD"),
    ("republic of the congo", "CG"),
    ("republique centrafricaine", "CF"),
    ("republique democratique du congo", "CD"),
    ("republique dominicaine", "DO"),
    ("republique du congo", "CG"),
    ("republique francaise", "FR"),
    ("republique tcheque", "CZ"),
    ("romania", "RO"),
    ("roumanie", "RO"),
    ("royaume uni", "GB"),
    ("royaume-uni", "GB"),
    ("russia", "RU"),
    ("russie", "RU"),
    ("rwanda", "RW"),
    ("république centrafricaine", "CF"),
    ("république dominicaine", "DO"),
    ("république du congo", "CG"),
    ("république démocratique du congo", "CD"),
    ("république française", "FR"),
    ("république tchèque", "CZ"),
    ("saint kitts and nevis", "KN"),
    ("saint lucia", "LC"),
    ("saint marin", "SM"),
    ("saint vincent and the grenadines", "VC"),
    ("saint-kitts-et-nevis", "KN"),
    ("saint-marin", "SM"),
    ("saint-siege", "VA"),
    ("saint-siège", "VA"),
    ("saint-vincent-et-les-grenadines", "VC"),
    ("sainte-lucie", "LC"),
    ("salvador", "SV"),
    ("samoa", "WS"),
    ("san marino", "SM"),
    ("sao tome and principe", "ST"),
    ("sao tome-et-principe", "ST"),
    ("sao tomé-et-principe", "ST"),
    ("saudi arabia", "SA"),
    ("senegal", "SN"),
    ("serbia", "RS"),
    ("serbie", "RS"),
    ("seychelles", "SC"),
    ("sierra leone", "SL"),
    ("singapore", "SG"),
    ("singapour", "SG"),
    ("slovakia", "SK"),
    ("slovaquie", "SK"),
    ("slovenia", "SI"),
    ("slovenie", "SI"),
    ("slovénie", "SI"),
    ("solomon islands", "SB"),
    ("somalia", "SO"),
    ("somalie", "SO"),
    ("soudan", "SD"),
    ("soudan du sud", "SS"),
    ("south africa", "ZA"),
    ("south korea", "KR"),
    ("south sudan", "SS"),
    ("spain", "ES"),
    ("sri lanka", "LK"),
    ("sudan", "SD"),
    ("suede", "SE"),
    ("suisse", "CH"),
    ("suriname", "SR"),
    ("suède", "SE"),
    ("swaziland", "SZ"),
    ("sweden", "SE"),
    ("switzerland", "CH"),
    ("syria", "SY"),
    ("syrie", "SY"),
    ("sénégal", "SN"),
    ("tadjikistan", "TJ"),
    ("taiwan", "TW"),
    ("tajikistan", "TJ"),
    ("tanzania", "TZ"),
    ("tanzanie", "TZ"),
    ("taïwan", "TW"),
    ("tchad", "TD"),
    ("tchequie", "CZ"),
    ("tchéquie", "CZ"),
    ("thailand", "TH"),
    ("thailande", "TH"),
    ("thaïlande", "TH"),
    ("timor oriental", "TL"),
    ("timor-leste", "TL"),
    ("togo", "TG"),
    ("tonga", "TO"),
    ("trinidad and tobago", "TT"),
    ("trinite-et-tobago", "TT"),
    ("trinité-et-tobago", "TT"),
    ("tunisia", "TN"),
    ("tunisie", "TN"),
    ("turkey", "TR"),
    ("turkmenistan", "TM"),
    ("turkménistan", "TM"),
    ("turquie", "TR"),
    ("tuvalu", "TV"),
    ("u.s.", "US"),
    ("u.s.a.", "US"),
    ("uae", "AE"),
    ("uganda", "UG"),
    ("uk", "GB"),
    ("ukraine", "UA"),
    ("united arab emirates", "AE"),
    ("united kingdom", "GB"),
    ("united states", "US"),
    ("united states of america", "US"),
    ("uruguay", "UY"),
    ("usa", "US"),
    ("uzbekistan", "UZ"),
    ("vanuatu", "VU"),
    ("vatican", "VA"),
    ("venezuela", "VE"),
    ("viet nam", "VN"),
    ("vietnam", "VN"),
    ("viêt nam", "VN"),
    ("yemen", "YE"),
    ("yémen", "YE"),
    ("zambia", "ZM"),
    ("zambie", "ZM"),
    ("zimbabwe", "ZW"),
    ("égypte", "EG"),
    ("émirats arabes unis", "AE"),
    ("équateur", "EC"),
    ("érythrée", "ER"),
    ("états unis", "US"),
    ("états-unis", "US"),
    ("états-unis d'amérique", "US"),
    ("éthiopie", "ET"),
    ("îles marshall", "MH"),
    ("îles salomon", "SB"),
];

/// Map a stored or submitted country to an uppercase ISO alpha-2 code.
pub fn normalize_country_code(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("Country code is required".to_string()));
    }
    let key = trimmed.to_lowercase();
    let code =
        if let Ok(index) = COUNTRY_ALIASES.binary_search_by(|entry| entry.0.cmp(key.as_str())) {
            COUNTRY_ALIASES[index].1.to_string()
        } else if trimmed.len() == 2 && trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
            trimmed.to_ascii_uppercase()
        } else {
            return Err(AppError::Validation(format!(
                "Unrecognized country '{trimmed}'"
            )));
        };
    if !crate::services::country_zones::is_known_country_code(&code) {
        return Err(AppError::Validation(format!(
            "Unrecognized country '{trimmed}'"
        )));
    }
    Ok(code)
}

/// `None` and blank leave the stored country unchanged (`COALESCE`).
pub fn normalize_optional_country(value: Option<String>) -> AppResult<Option<String>> {
    match value {
        None => Ok(None),
        Some(raw) if raw.trim().is_empty() => Ok(None),
        Some(raw) => Ok(Some(normalize_country_code(&raw)?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_are_unique_and_sorted() {
        let mut previous = "";
        for &(alias, code) in COUNTRY_ALIASES {
            assert!(alias > previous, "{alias} is not sorted after {previous}");
            assert_eq!(alias, alias.to_lowercase());
            assert_eq!(code.len(), 2);
            assert!(code.chars().all(|c| c.is_ascii_uppercase()));
            assert!(
                crate::services::country_zones::is_known_country_code(code),
                "{alias} maps to unknown country {code}"
            );
            previous = alias;
        }
    }

    #[test]
    fn maps_common_names_and_codes() {
        for (input, expected) in [
            ("France", "FR"),
            ("france", "FR"),
            ("FR", "FR"),
            ("fr", "FR"),
            ("  France  ", "FR"),
            ("république française", "FR"),
            ("United States", "US"),
            ("états-unis", "US"),
            ("etats unis", "US"),
            ("USA", "US"),
            ("uk", "GB"),
            ("UK", "GB"),
            ("England", "GB"),
            ("royaume-uni", "GB"),
            ("el", "GR"),
            ("Allemagne", "DE"),
            ("de", "DE"),
        ] {
            assert_eq!(normalize_country_code(input).unwrap(), expected, "{input}");
        }
    }

    #[test]
    fn rejects_unknown_free_text_and_blank() {
        assert!(normalize_country_code("Narnia").is_err());
        assert!(normalize_country_code("XX").is_err());
        assert!(normalize_country_code("en").is_err());
        assert!(normalize_country_code("   ").is_err());
        assert!(normalize_country_code("").is_err());
    }

    #[test]
    fn blank_optional_is_unchanged() {
        assert_eq!(normalize_optional_country(None).unwrap(), None);
        assert_eq!(normalize_optional_country(Some("  ".into())).unwrap(), None);
        assert_eq!(
            normalize_optional_country(Some("france".into()))
                .unwrap()
                .as_deref(),
            Some("FR")
        );
    }

    #[test]
    fn migration_lists_every_alias() {
        let sql = include_str!("../../migrations/043_library_country_iso.sql");
        for (alias, code) in COUNTRY_ALIASES {
            let escaped = alias.replace('\'', "''");
            let needle = format!("WHEN '{escaped}' THEN '{code}'");
            assert!(
                sql.contains(&needle),
                "migration missing alias {alias} → {code}"
            );
        }
        assert!(sql.contains("SET addr_country = NULL"));
        let executable = sql
            .lines()
            .map(|line| line.split("--").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !executable.to_lowercase().contains("timezone"),
            "migration must not touch the zone column"
        );
    }
}
