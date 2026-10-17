//! Langue de l'interface : catalogues de l'interface (`gui/locales`), ajoutés à ceux du
//! moteur ([`tri::i18n`]).

use tri::i18n::{self, Language, LanguageSetting};

/// Catalogues de l'interface.
const CATALOGS: &[(Language, &str)] = &[
    (Language::Fr, include_str!("../locales/fr.ftl")),
    (Language::En, include_str!("../locales/en.ftl")),
];

/// Ajoute les catalogues de l'interface et choisit la langue (`[i18n] language`).
pub fn init(setting: LanguageSetting) {
    register();
    i18n::set_language(setting.resolve());
}

/// Ajoute les catalogues de l'interface une fois (macro `t!` de l'interface).
pub fn ensure() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(register);
}

/// Ajoute les catalogues de l'interface (sans changer de langue).
pub fn register() {
    for &(language, source) in CATALOGS {
        i18n::register(language, source);
    }
}

/// Système, pour les textes qui en dépendent (« Afficher dans le Finder »).
pub fn os() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn every_id_used_by_the_interface_exists_in_every_language() {
        register();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let source = std::fs::read_to_string(&path).unwrap();
            for (i, _) in source.match_indices("t!(\"") {
                // `t!(`, pas `format!(` ni `print!(`.
                let before = source[..i].chars().next_back();
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                // Exemples des commentaires de documentation.
                let line_start = source[..i].rfind('\n').map_or(0, |n| n + 1);
                if source[line_start..i].trim_start().starts_with("//") {
                    continue;
                }
                let rest = &source[i + 4..];
                let id = &rest[..rest.find('"').unwrap()];
                for language in Language::ALL {
                    if !i18n::has(language, id) {
                        missing.push(format!("{id} ({}, {})", language.code(), path.display()));
                    }
                }
            }
        }
        // Libellés des actions et des thèmes, aux identifiants construits.
        for action in crate::actions::Action::ALL {
            let id = format!("action-{}", action.id().replace('.', "-"));
            for language in Language::ALL {
                if !i18n::has(language, &id) {
                    missing.push(format!("{id} ({})", language.code()));
                }
            }
        }
        for group in crate::actions::Group::ALL {
            let id = format!("group-{group:?}").to_lowercase();
            for language in Language::ALL {
                if !i18n::has(language, &id) {
                    missing.push(format!("{id} ({})", language.code()));
                }
            }
        }
        assert!(missing.is_empty(), "identifiants absents : {missing:#?}");
    }

    #[test]
    fn interface_catalogs_have_the_same_ids_and_no_duplicate() {
        let mut reference = i18n::ids(CATALOGS[0].1);
        reference.sort();
        let mut unique = reference.clone();
        unique.dedup();
        assert_eq!(unique, reference, "pas d'identifiant en double");
        for (language, source) in &CATALOGS[1..] {
            let mut ids = i18n::ids(source);
            ids.sort();
            assert_eq!(ids, reference, "{}", language.code());
        }
        // Pas de collision avec le moteur (la définition du moteur l'emporterait).
        let engine = i18n::ids(i18n::engine_catalogs()[0].1);
        let clash: Vec<&String> = reference.iter().filter(|id| engine.contains(id)).collect();
        assert!(
            clash.is_empty(),
            "identifiants déjà dans le moteur : {clash:?}"
        );
    }

    #[test]
    fn english_is_available() {
        // Bundle anglais complet (sans changer la langue globale, partagée entre tests).
        register();
        assert!(i18n::has(Language::En, "action-core-app-quit"));
    }
}
