//! Complétude des catalogues du moteur : chaque identifiant utilisé dans le code
//! (`t!("…")`) existe en français et en anglais, et les deux catalogues ont les mêmes
//! identifiants.

use std::path::Path;

use tri::i18n::{self, Language};

/// Identifiants littéraux des appels `t!("…")` des fichiers `.rs` de `dir` (récursif).
fn used_ids(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            used_ids(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
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
                out.push((id.to_string(), path.display().to_string()));
            }
        }
    }
}

#[test]
fn every_id_used_by_the_engine_exists_in_every_language() {
    let mut used = Vec::new();
    used_ids(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut used,
    );
    assert!(!used.is_empty());
    let mut missing = Vec::new();
    for (id, file) in &used {
        for language in Language::ALL {
            if !i18n::has(language, id) {
                missing.push(format!("{id} ({}, {file})", language.code()));
            }
        }
    }
    assert!(missing.is_empty(), "identifiants absents : {missing:#?}");
}

#[test]
fn engine_catalogs_have_the_same_ids() {
    let catalogs = i18n::engine_catalogs();
    let mut reference = i18n::ids(catalogs[0].1);
    reference.sort();
    for (language, source) in &catalogs[1..] {
        let mut ids = i18n::ids(source);
        ids.sort();
        assert_eq!(ids, reference, "{}", language.code());
    }
}
