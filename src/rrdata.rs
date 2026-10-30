//! Notes dans les sidecars de RapidRAW (`IMAGE.RAF.rrdata`, et `IMAGE.RAF.<id>.rrdata`
//! pour une copie virtuelle) : un développement RapidRAW garde ses notes là où RapidRAW
//! les lit (décidé le 30/10/2026). Voir `doc/en/rrdata.md`.
//!
//! Le fichier est du JSON : `rating` (0 à 5), `flag` (`"pick"` / `"reject"`, absent sinon),
//! `tags` (la couleur y est une étiquette `"color:red"`, une seule), `adjustments` (les
//! réglages). Comme pour les XMP, rien ne se perd : seuls ces trois champs changent, le
//! reste (réglages, mots-clés, champs inconnus) est réécrit tel quel, dans le même ordre.

use std::io;
use std::path::Path;

use serde_json::{Map, Value};

use crate::xmp::{Color, Flag, Notes};

/// Préfixe d'une étiquette de couleur de RapidRAW (`"color:red"`).
const COLOR_TAG: &str = "color:";

/// Nom d'une couleur dans RapidRAW.
fn color_name(color: Color) -> &'static str {
    match color {
        Color::Red => "red",
        Color::Yellow => "yellow",
        Color::Green => "green",
        Color::Blue => "blue",
        Color::Purple => "purple",
    }
}

fn invalid(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}

fn object(json: &str) -> io::Result<Map<String, Value>> {
    match serde_json::from_str(json).map_err(invalid)? {
        Value::Object(map) => Ok(map),
        _ => Err(invalid("rrdata : objet JSON attendu")),
    }
}

/// Notes du contenu `json` d'un `.rrdata`.
pub fn parse(json: &str) -> io::Result<Notes> {
    let map = object(json)?;
    let mut notes = Notes::default();
    notes.set_stars(
        map.get("rating")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(5) as u8,
    );
    notes.flag = match map.get("flag").and_then(Value::as_str) {
        Some("pick") => Flag::Accepted,
        Some("reject") => Flag::Rejected,
        _ => Flag::None,
    };
    let tags = map.get("tags").and_then(Value::as_array);
    notes.colors = tags
        .into_iter()
        .flatten()
        .filter_map(|t| t.as_str()?.strip_prefix(COLOR_TAG))
        .filter_map(|name| Color::ALL.into_iter().find(|&c| color_name(c) == name))
        .collect();
    notes.colors.sort();
    notes.colors.dedup();
    Ok(notes)
}

/// `json` avec les notes `notes`, le reste inchangé. RapidRAW n'a qu'une couleur : la
/// première des couleurs de `notes`.
pub fn apply(json: &str, notes: &Notes) -> io::Result<String> {
    let mut map = object(json)?;
    map.insert("rating".into(), Value::from(notes.stars));
    match notes.flag {
        Flag::Accepted => {
            map.insert("flag".into(), Value::from("pick"));
        }
        Flag::Rejected => {
            map.insert("flag".into(), Value::from("reject"));
        }
        Flag::None => {
            map.remove("flag");
        }
    }
    let mut tags: Vec<Value> = map
        .get("tags")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|t| !t.as_str().is_some_and(|t| t.starts_with(COLOR_TAG)))
        .cloned()
        .collect();
    if let Some(&color) = notes.colors.first() {
        tags.push(Value::from(format!("{COLOR_TAG}{}", color_name(color))));
    }
    if tags.is_empty() {
        // Comme RapidRAW : pas de liste vide ajoutée.
        if map.get("tags").is_some_and(|t| !t.is_null()) {
            map.insert("tags".into(), Value::Array(tags));
        }
    } else {
        map.insert("tags".into(), Value::Array(tags));
    }
    serde_json::to_string_pretty(&Value::Object(map)).map_err(invalid)
}

/// Notes du sidecar RapidRAW `path`.
pub fn read(path: &Path) -> io::Result<Notes> {
    parse(&std::fs::read_to_string(path)?)
}

/// Écrit `notes` dans le sidecar RapidRAW `path` (relu juste avant : ce que RapidRAW y a
/// mis entre-temps est gardé). Faux si rien n'a changé (fichier pas réécrit).
pub fn write(path: &Path, notes: &Notes) -> io::Result<bool> {
    let text = std::fs::read_to_string(path)?;
    if parse(&text)? == *notes {
        return Ok(false);
    }
    let new = apply(&text, notes)?;
    std::fs::write(path, new)?;
    crate::cache::forget_stamps([path]);
    Ok(true)
}

/// Réglages (`adjustments`) du contenu `json` d'un `.rrdata` : ce que `rapidraw export
/// --adjustments` attend pour rendre une copie virtuelle.
pub fn adjustments(json: &str) -> io::Result<Value> {
    object(json)?
        .remove("adjustments")
        .ok_or_else(|| invalid("rrdata : pas de réglages (adjustments)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
  "version": 1,
  "rating": 2,
  "adjustments": {"exposure": 0.5, "zz": 1, "aa": 2},
  "tags": ["darktable", "color:green"],
  "unknown": [1, 2]
}"#;

    #[test]
    fn notes_are_read_from_rating_flag_and_color_tags() {
        let notes = parse(SAMPLE).unwrap();
        assert_eq!(notes.stars, 2);
        assert_eq!(notes.flag, Flag::None);
        assert_eq!(notes.colors, [Color::Green]);
        let rejected = parse(r#"{"rating": 9, "flag": "reject"}"#).unwrap();
        assert_eq!((rejected.stars, rejected.flag), (5, Flag::Rejected));
        assert_eq!(parse(r#"{"flag": "pick"}"#).unwrap().flag, Flag::Accepted);
        assert!(parse("[1]").is_err());
    }

    #[test]
    fn writing_notes_keeps_everything_else_in_place() {
        let mut notes = Notes::default();
        notes.set_stars(4);
        notes.flag = Flag::Accepted;
        notes.colors = vec![Color::Red, Color::Blue];
        let new = apply(SAMPLE, &notes).unwrap();
        let back = parse(&new).unwrap();
        assert_eq!((back.stars, back.flag), (4, Flag::Accepted));
        assert_eq!(back.colors, [Color::Red], "une seule couleur dans RapidRAW");
        let value: Value = serde_json::from_str(&new).unwrap();
        assert_eq!(value["tags"], serde_json::json!(["darktable", "color:red"]));
        assert_eq!(value["unknown"], serde_json::json!([1, 2]));
        // Ordre des champs gardé, réglages compris.
        let keys: Vec<&String> = value.as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            [
                "version",
                "rating",
                "adjustments",
                "tags",
                "unknown",
                "flag"
            ]
        );
        let adjustment_keys: Vec<&String> =
            value["adjustments"].as_object().unwrap().keys().collect();
        assert_eq!(adjustment_keys, ["exposure", "zz", "aa"]);
        // Sans drapeau ni couleur : le drapeau part, l'étiquette de couleur aussi.
        let plain = apply(&new, &Notes::default()).unwrap();
        let value: Value = serde_json::from_str(&plain).unwrap();
        assert!(value.get("flag").is_none());
        assert_eq!(value["tags"], serde_json::json!(["darktable"]));
    }

    #[test]
    fn a_sidecar_is_rewritten_only_when_its_notes_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("IMG.RAF.c33b94.rrdata");
        std::fs::write(&path, SAMPLE).unwrap();
        let same = read(&path).unwrap();
        assert!(!write(&path, &same).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), SAMPLE);
        let mut notes = same;
        notes.set_stars(5);
        assert!(write(&path, &notes).unwrap());
        assert_eq!(read(&path).unwrap().stars, 5);
        assert_eq!(
            adjustments(&std::fs::read_to_string(&path).unwrap()).unwrap()["exposure"],
            0.5
        );
    }
}
