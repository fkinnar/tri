//! Notes dans les profils de RawTherapee (`IMAGE.RAF.pp3`) et d'ART (`IMAGE.RAF.arp`,
//! même format : ART est un dérivé de RawTherapee), ajoutés le 31/10/2026 sur le modèle
//! de [`crate::rrdata`]. Voir `doc/en/pp3.md`.
//!
//! Le fichier est de la forme INI : des sections `[Nom]` et des lignes `Clé=Valeur`. Les
//! notes sont dans `[General]` : `Rank` (0 à 5), `ColorLabel` (0 aucune, 1 rouge,
//! 2 jaune, 3 vert, 4 bleu, 5 violet), `InTrash` (`true` : dans la corbeille du logiciel,
//! ce que `tri` lit et écrit comme « rejetée »). Pas d'équivalent pour « acceptée » : ce
//! drapeau reste dans le XMP. Comme partout, rien ne se perd : seules ces trois clés
//! changent, le reste du fichier (les réglages, l'ordre des lignes, les commentaires) est
//! réécrit tel quel.

use std::io;
use std::path::{Path, PathBuf};

use crate::xmp::{Color, Flag, Notes};

/// Section des notes.
const GENERAL: &str = "[General]";
/// Sections qui ne développent rien : la version du logiciel et les notes.
const NEUTRAL_SECTIONS: &[&str] = &["[Version]", GENERAL];

/// Numéro d'une couleur dans RawTherapee et ART.
fn color_number(color: Color) -> u8 {
    match color {
        Color::Red => 1,
        Color::Yellow => 2,
        Color::Green => 3,
        Color::Blue => 4,
        Color::Purple => 5,
    }
}

fn color_of(number: u8) -> Option<Color> {
    Color::ALL.into_iter().find(|&c| color_number(c) == number)
}

/// Lignes du profil, découpées (fins de ligne gardées à part : `\r\n` sous Windows).
fn lines(text: &str) -> (Vec<&str>, &'static str) {
    let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    if text.is_empty() {
        return (Vec::new(), ending);
    }
    (text.split(ending).collect(), ending)
}

/// Nom de section d'une ligne (`[General]`), espaces ignorés.
fn section(line: &str) -> Option<&str> {
    let t = line.trim();
    (t.starts_with('[') && t.ends_with(']')).then_some(t)
}

/// Clé et valeur d'une ligne `Clé=Valeur`.
fn key_value(line: &str) -> Option<(&str, &str)> {
    let t = line.trim();
    if t.starts_with('#') || t.starts_with(';') || t.starts_with('[') {
        return None;
    }
    let (k, v) = t.split_once('=')?;
    Some((k.trim(), v.trim()))
}

/// Valeur de `key` dans la section `[General]` du profil `text`.
fn general_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let mut in_general = false;
    for line in text.lines() {
        if let Some(s) = section(line) {
            in_general = s.eq_ignore_ascii_case(GENERAL);
            continue;
        }
        if in_general
            && let Some((k, v)) = key_value(line)
            && k.eq_ignore_ascii_case(key)
        {
            return Some(v);
        }
    }
    None
}

/// Notes du contenu `text` d'un profil.
pub fn parse(text: &str) -> io::Result<Notes> {
    let mut notes = Notes::default();
    let number = |key: &str| general_value(text, key).and_then(|v| v.parse::<u8>().ok());
    notes.set_stars(number("Rank").unwrap_or(0).min(5));
    notes.colors = number("ColorLabel")
        .and_then(color_of)
        .into_iter()
        .collect();
    if general_value(text, "InTrash").is_some_and(|v| v.eq_ignore_ascii_case("true")) {
        notes.flag = Flag::Rejected;
    }
    Ok(notes)
}

/// `text` avec les notes `notes`, le reste inchangé : les clés `Rank`, `ColorLabel` et
/// `InTrash` de `[General]` sont remplacées sur place, ajoutées si elles manquent, la
/// section créée en tête si elle manque. Une seule couleur : la première de `notes`.
pub fn apply(text: &str, notes: &Notes) -> String {
    let (lines, ending) = lines(text);
    let wanted: [(&str, String); 3] = [
        ("Rank", notes.stars.to_string()),
        (
            "ColorLabel",
            notes
                .colors
                .first()
                .map_or(0, |&c| color_number(c))
                .to_string(),
        ),
        ("InTrash", (notes.flag == Flag::Rejected).to_string()),
    ];
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 4);
    let mut in_general = false;
    let mut seen = [false; 3];
    let mut general_found = false;
    let flush_missing = |out: &mut Vec<String>, seen: &[bool; 3]| {
        for (i, (k, v)) in wanted.iter().enumerate() {
            if !seen[i] {
                out.push(format!("{k}={v}"));
            }
        }
    };
    for line in &lines {
        if let Some(s) = section(line) {
            if in_general {
                // Fin de `[General]` : les clés manquantes avant la section suivante.
                flush_missing(&mut out, &seen);
                in_general = false;
            }
            if s.eq_ignore_ascii_case(GENERAL) {
                in_general = true;
                general_found = true;
            }
            out.push((*line).to_string());
            continue;
        }
        if in_general
            && let Some((k, _)) = key_value(line)
            && let Some(i) = wanted.iter().position(|(w, _)| w.eq_ignore_ascii_case(k))
        {
            out.push(format!("{k}={}", wanted[i].1));
            seen[i] = true;
            continue;
        }
        out.push((*line).to_string());
    }
    if in_general {
        // `[General]` était la dernière section : avant la fin (et sa ligne vide finale).
        let trailing_empty = out.last().is_some_and(|l| l.trim().is_empty());
        let tail = if trailing_empty { out.pop() } else { None };
        flush_missing(&mut out, &seen);
        if let Some(t) = tail {
            out.push(t);
        }
    }
    if !general_found {
        let mut head = vec![GENERAL.to_string()];
        for (k, v) in &wanted {
            head.push(format!("{k}={v}"));
        }
        head.push(String::new());
        head.extend(out);
        out = head;
    }
    out.join(ending)
}

/// Le profil de `image` pour l'extension `extension` (`"pp3"`, `"arp"`) : `IMG.RAF.pp3`.
pub fn path(image: &Path, extension: &str) -> PathBuf {
    let mut name = image.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(extension);
    image.with_file_name(name)
}

/// Vrai si le contenu `text` d'un profil développe l'image : une section autre que
/// `[Version]` et `[General]` (RawTherapee écrit un profil réduit à celles-ci quand on
/// note une image dans son explorateur sans l'ouvrir ; `tri` aussi, voir
/// [`crate::sidecars`]).
pub fn is_development(text: &str) -> bool {
    text.lines()
        .filter_map(section)
        .any(|s| !NEUTRAL_SECTIONS.iter().any(|n| n.eq_ignore_ascii_case(s)))
}

/// [`is_development`] pour le fichier `path` (faux s'il est illisible).
pub fn file_is_development(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|text| is_development(&text))
}

/// Le contenu de `text` sans la section `[General]` (les notes) : ce qui décrit le
/// développement, pour la clé de cache du rendu (noter ne refait pas le rendu).
pub fn development_content(text: &str) -> String {
    let (lines, ending) = lines(text);
    let mut in_general = false;
    lines
        .into_iter()
        .filter(|line| {
            if let Some(s) = section(line) {
                in_general = s.eq_ignore_ascii_case(GENERAL);
                return !in_general;
            }
            !in_general
        })
        .collect::<Vec<_>>()
        .join(ending)
}

/// Crée le profil `path` avec `notes` seules (section `[General]`, comme RawTherapee pour
/// une image notée sans être développée : un profil partiel, complété par ses défauts).
pub fn create(path: &Path, notes: &Notes) -> io::Result<()> {
    std::fs::write(path, apply("", notes))?;
    crate::cache::forget_stamps([path]);
    Ok(())
}

/// Notes du profil `path`.
pub fn read(path: &Path) -> io::Result<Notes> {
    parse(&std::fs::read_to_string(path)?)
}

/// Écrit `notes` dans le profil `path` (relu juste avant : ce que le logiciel y a mis
/// entre-temps est gardé). Faux si rien n'a changé (fichier pas réécrit).
pub fn write(path: &Path, notes: &Notes) -> io::Result<bool> {
    let text = std::fs::read_to_string(path)?;
    if parse(&text)? == *notes {
        return Ok(false);
    }
    std::fs::write(path, apply(&text, notes))?;
    crate::cache::forget_stamps([path]);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = "[Version]\nAppVersion=5.11\nVersion=351\n\n[General]\nRank=2\nColorLabel=3\nInTrash=false\n\n[Exposure]\nAuto=false\nCompensation=0.5\n";

    #[test]
    fn notes_are_read_from_general_and_rewritten_in_place() {
        let notes = parse(PROFILE).unwrap();
        assert_eq!(notes.stars, 2);
        assert_eq!(notes.colors, [Color::Green]);
        assert_eq!(notes.flag, Flag::None);
        let mut wanted = Notes::default();
        wanted.set_stars(5);
        wanted.colors = vec![Color::Red, Color::Blue];
        wanted.flag = Flag::Rejected;
        let text = apply(PROFILE, &wanted);
        assert_eq!(
            text,
            "[Version]\nAppVersion=5.11\nVersion=351\n\n[General]\nRank=5\nColorLabel=1\nInTrash=true\n\n[Exposure]\nAuto=false\nCompensation=0.5\n",
            "une seule couleur, le reste intact"
        );
        let mut back = wanted.clone();
        back.colors = vec![Color::Red];
        assert_eq!(parse(&text).unwrap(), back);
        assert!(is_development(PROFILE));
        assert!(!is_development(
            "[Version]\nVersion=351\n\n[General]\nRank=3\n"
        ));
        assert!(!is_development(""));
        assert_eq!(
            development_content(PROFILE),
            "[Version]\nAppVersion=5.11\nVersion=351\n\n[Exposure]\nAuto=false\nCompensation=0.5\n"
        );
    }

    #[test]
    fn missing_keys_and_sections_are_added_without_touching_the_rest() {
        let mut notes = Notes::default();
        notes.set_stars(4);
        // `[General]` en dernier, sans `ColorLabel` ni `InTrash`.
        let text = apply("[Exposure]\nAuto=false\n\n[General]\nRank=1\n", &notes);
        assert_eq!(
            text,
            "[Exposure]\nAuto=false\n\n[General]\nRank=4\nColorLabel=0\nInTrash=false\n"
        );
        // Sans `[General]` : créée en tête ; fins de ligne Windows gardées.
        let text = apply("[Exposure]\r\nAuto=false\r\n", &notes);
        assert_eq!(
            text,
            "[General]\r\nRank=4\r\nColorLabel=0\r\nInTrash=false\r\n\r\n[Exposure]\r\nAuto=false\r\n"
        );
        // Profil vide (création) : la section seule.
        assert_eq!(
            apply("", &notes),
            "[General]\nRank=4\nColorLabel=0\nInTrash=false\n"
        );
        assert_eq!(parse(&apply("", &notes)).unwrap(), notes);
    }

    #[test]
    fn files_are_created_read_and_written() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("IMG.RAF");
        let pp3 = path(&image, "pp3");
        assert_eq!(pp3, dir.path().join("IMG.RAF.pp3"));
        let mut notes = Notes::default();
        notes.set_stars(3);
        create(&pp3, &notes).unwrap();
        assert_eq!(read(&pp3).unwrap(), notes);
        assert!(!file_is_development(&pp3), "notes seules");
        assert!(!write(&pp3, &notes).unwrap(), "rien à changer");
        std::fs::write(
            &pp3,
            format!(
                "{}[Crop]\nEnabled=true\n",
                std::fs::read_to_string(&pp3).unwrap()
            ),
        )
        .unwrap();
        assert!(file_is_development(&pp3));
        notes.colors = vec![Color::Purple];
        assert!(write(&pp3, &notes).unwrap());
        let text = std::fs::read_to_string(&pp3).unwrap();
        assert!(text.contains("ColorLabel=5") && text.contains("[Crop]\nEnabled=true"));
    }
}
