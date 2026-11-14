//! Gabarits de noms de fichiers (issue n° 439, décidé le 14/11/2026) : des champs entre
//! accolades, `{date}_{folder}_{n:3}` → `2026-11-14_Bleu_Jaune_001`. Servent au
//! renommage par lot ([`crate::files::plan_batch`]) ; le déchargement pourra s'en servir
//! aussi. Voir `doc/en/files.md`.
//!
//! Champs : `{date}` (prise de vue, `2026-11-14`), `{time}` (`153012`), `{name}` (nom
//! d'origine sans extension), `{n}` (compteur ; `{n:3}` = `001`), `{folder}` (nom du
//! dossier sans la date en tête), `{camera}` (modèle de l'appareil).

use std::fmt;
use std::path::Path;

use crate::media::{MediaFile, MediaKind};

/// Les champs connus, dans l'ordre de l'aide.
pub const FIELDS: [&str; 6] = ["date", "time", "name", "n", "folder", "camera"];

/// Un champ pour l'aide : comment l'écrire, et ce qu'il donne (exemple et usage compris),
/// dans la langue de l'interface. Les noms des champs, eux, ne se traduisent jamais.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldHelp {
    /// Le champ tel qu'on l'écrit (`{date}`).
    pub token: String,
    /// Ce qu'il donne, un exemple, et où il s'utilise.
    pub description: String,
}

/// Tous les champs des gabarits, pour l'aide (renommage par lot et dossiers d'export) :
/// ceux de [`FIELDS`], puis `{source}`, propre aux dossiers d'export
/// ([`crate::export::SOURCE`]).
pub fn help() -> Vec<FieldHelp> {
    FIELDS
        .iter()
        .copied()
        .chain(std::iter::once("source"))
        .map(|field| FieldHelp {
            token: format!("{{{field}}}"),
            description: crate::t!("naming-help", field = field),
        })
        .collect()
}

/// Ce qu'un gabarit peut citer d'une photo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fields {
    /// Date de prise de vue, `AAAA-MM-JJ`.
    pub date: Option<String>,
    /// Heure de prise de vue, `HHMMSS`.
    pub time: Option<String>,
    /// Nom d'origine, sans extension.
    pub name: String,
    /// Nom du dossier, sans la date en tête (`2026-11-14_Bleu_Jaune` → `Bleu_Jaune`).
    pub folder: String,
    /// Modèle de l'appareil.
    pub camera: Option<String>,
}

impl Fields {
    /// Ce qui se lit sans ouvrir le fichier : nom et dossier.
    pub fn basic(image: &Path) -> Fields {
        Fields {
            name: image
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            folder: image
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| folder_label(&n.to_string_lossy()))
                .unwrap_or_default(),
            ..Fields::default()
        }
    }

    /// Tous les champs, métadonnées comprises (~10 ms par RAW : hors du fil de
    /// l'interface).
    pub fn read(image: &Path) -> Fields {
        let mut fields = Fields::basic(image);
        let kind = crate::media::kind_of(image).unwrap_or(MediaKind::Flat);
        let file = MediaFile {
            path: image.to_path_buf(),
            kind,
        };
        if let Ok(meta) = crate::metadata::read(&file) {
            let digits: Vec<&str> = meta
                .date
                .as_deref()
                .unwrap_or_default()
                .split(|c: char| !c.is_ascii_digit())
                .filter(|p| !p.is_empty())
                .collect();
            if let [y, mo, d, h, mi, s, ..] = digits[..] {
                fields.date = Some(format!("{y}-{mo}-{d}"));
                fields.time = Some(format!("{h}{mi}{s}"));
            }
            fields.camera = meta.model.filter(|m| !m.trim().is_empty());
        }
        fields
    }
}

/// Nom de dossier sans la date en tête (`AAAA-MM-JJ` suivie d'un `_`, `-` ou espace),
/// comme les dossiers de séance du déchargement.
pub fn folder_label(name: &str) -> String {
    let bytes = name.as_bytes();
    let dated = bytes.len() >= 10
        && bytes[..10].iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    if !dated {
        return name.to_string();
    }
    let rest = name[10..].trim_start_matches(['_', '-', ' ']);
    if rest.is_empty() {
        name.to_string()
    } else {
        rest.to_string()
    }
}

/// Erreur d'un gabarit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplateError {
    /// Accolade ouverte et jamais fermée.
    Unclosed,
    /// Champ inconnu (`{dat}`).
    Unknown(String),
    /// Largeur invalide (`{n:x}`), ou largeur sur un autre champ que `n`.
    Width(String),
    /// Champ sans valeur pour cette photo (pas de date de prise de vue…).
    Missing(String),
    /// Champ qui n'a pas de sens ici (`{n}` dans un dossier d'export).
    NotHere(String),
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            TemplateError::Unclosed => crate::t!("naming-unclosed"),
            TemplateError::Unknown(field) => crate::t!(
                "naming-unknown",
                field = field.as_str(),
                fields = FIELDS.map(|f| format!("{{{f}}}")).join(" ")
            ),
            TemplateError::Width(field) => crate::t!("naming-width", field = field.as_str()),
            TemplateError::Missing(field) => crate::t!("naming-missing", field = field.as_str()),
            TemplateError::NotHere(field) => crate::t!("naming-not-here", field = field.as_str()),
        };
        f.write_str(&text)
    }
}

impl std::error::Error for TemplateError {}

/// Un morceau de gabarit : texte tel quel, ou champ (avec sa largeur pour `n`).
enum Part<'a> {
    Text(&'a str),
    Field(&'a str, Option<usize>),
}

fn parts(template: &str) -> Result<Vec<Part<'_>>, TemplateError> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        if open > 0 {
            out.push(Part::Text(&rest[..open]));
        }
        let after = &rest[open + 1..];
        let close = after.find('}').ok_or(TemplateError::Unclosed)?;
        let inside = after[..close].trim();
        let (name, width) = match inside.split_once(':') {
            Some((name, width)) => {
                let name = name.trim();
                let width = width
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .filter(|w| (1..=9).contains(w) && name == "n")
                    .ok_or_else(|| TemplateError::Width(inside.to_string()))?;
                (name, Some(width))
            }
            None => (inside, None),
        };
        if !FIELDS.contains(&name) {
            return Err(TemplateError::Unknown(name.to_string()));
        }
        out.push(Part::Field(name, width));
        rest = &after[close + 1..];
    }
    if !rest.is_empty() {
        out.push(Part::Text(rest));
    }
    Ok(out)
}

/// Erreur de forme du gabarit (accolade, champ inconnu), sans photo.
pub fn check(template: &str) -> Result<(), TemplateError> {
    parts(template).map(|_| ())
}

/// Vrai si le gabarit cite le compteur `{n}`.
pub fn uses_counter(template: &str) -> bool {
    parts(template).is_ok_and(|parts| parts.iter().any(|p| matches!(p, Part::Field("n", _))))
}

/// Vrai si le gabarit cite un champ lu dans le fichier (date, heure, appareil).
pub fn needs_metadata(template: &str) -> bool {
    parts(template).is_ok_and(|parts| {
        parts
            .iter()
            .any(|p| matches!(p, Part::Field("date" | "time" | "camera", _)))
    })
}

/// Le nom (sans extension) que donne `template` pour la photo `fields`, numéro `n`. Les
/// séparateurs de dossier et `:` d'une valeur deviennent `-`, ses blancs `_`.
pub fn render(template: &str, fields: &Fields, n: usize) -> Result<String, TemplateError> {
    let clean = |value: &str| -> String {
        value
            .trim()
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' => '-',
                c if c.is_whitespace() => '_',
                c => c,
            })
            .collect()
    };
    let missing = |field: &str| TemplateError::Missing(field.to_string());
    let mut out = String::new();
    for part in parts(template)? {
        match part {
            Part::Text(text) => out.push_str(text),
            Part::Field("n", width) => out.push_str(&format!("{n:0w$}", w = width.unwrap_or(1))),
            Part::Field("name", _) => out.push_str(&fields.name),
            Part::Field("folder", _) => out.push_str(&clean(&fields.folder)),
            Part::Field("date", _) => out.push_str(fields.date.as_deref().ok_or(missing("date"))?),
            Part::Field("time", _) => out.push_str(fields.time.as_deref().ok_or(missing("time"))?),
            Part::Field("camera", _) => {
                out.push_str(&clean(fields.camera.as_deref().ok_or(missing("camera"))?))
            }
            Part::Field(other, _) => return Err(TemplateError::Unknown(other.to_string())),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Fields {
        Fields {
            date: Some("2026-11-14".into()),
            time: Some("153012".into()),
            name: "DSCF8158".into(),
            folder: "Bleu Jaune".into(),
            camera: Some("X-T4".into()),
        }
    }

    #[test]
    fn a_template_fills_its_fields() {
        let f = fields();
        assert_eq!(
            render("{date}_{folder}_{n:3}", &f, 7).unwrap(),
            "2026-11-14_Bleu_Jaune_007"
        );
        assert_eq!(render("{name}_{camera}", &f, 1).unwrap(), "DSCF8158_X-T4");
        assert_eq!(render("{time}-{n}", &f, 12).unwrap(), "153012-12");
        assert_eq!(render("{ n : 2 }", &f, 3).unwrap(), "03", "blancs tolérés");
        assert_eq!(render("sans champ", &f, 3).unwrap(), "sans champ");
    }

    #[test]
    fn template_errors_are_named() {
        let f = fields();
        assert_eq!(render("{date", &f, 1), Err(TemplateError::Unclosed));
        assert_eq!(
            render("{dat}", &f, 1),
            Err(TemplateError::Unknown("dat".into()))
        );
        assert_eq!(
            render("{n:x}", &f, 1),
            Err(TemplateError::Width("n:x".into()))
        );
        assert_eq!(
            render("{date:2}", &f, 1),
            Err(TemplateError::Width("date:2".into()))
        );
        let none = Fields::basic(Path::new("/a/IMG.JPG"));
        assert_eq!(
            render("{date}", &none, 1),
            Err(TemplateError::Missing("date".into()))
        );
        assert!(needs_metadata("{name}_{camera}"));
        assert!(!needs_metadata("{name}_{n}"));
        assert!(check("{folder}").is_ok());
    }

    #[test]
    fn the_help_lists_every_field_with_a_description() {
        let help = help();
        assert_eq!(help.len(), FIELDS.len() + 1, "les champs et {{source}}");
        for field in FIELDS.iter().chain(&["source"]) {
            let token = format!("{{{field}}}");
            let line = help.iter().find(|h| h.token == token).unwrap();
            assert!(
                !line.description.is_empty() && !line.description.contains("naming-help"),
                "{token} : {}",
                line.description
            );
        }
    }

    #[test]
    fn the_folder_loses_its_date() {
        assert_eq!(folder_label("2026-11-14_Bleu_Jaune"), "Bleu_Jaune");
        assert_eq!(folder_label("2026-11-14 Mariage"), "Mariage");
        assert_eq!(
            folder_label("2026-11-14"),
            "2026-11-14",
            "rien d'autre : gardé"
        );
        assert_eq!(folder_label("Vacances"), "Vacances");
        let basic = Fields::basic(Path::new("/p/2026-11-14_Bleu_Jaune/DSCF1.RAF"));
        assert_eq!(
            (basic.name.as_str(), basic.folder.as_str()),
            ("DSCF1", "Bleu_Jaune")
        );
    }
}
