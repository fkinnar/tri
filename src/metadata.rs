//! Métadonnées de prise de vue d'une image : appareil, objectif, exposition, focale,
//! date, dimensions, taille du fichier.
//!
//! Pour un RAW, LibRaw fournit l'essentiel ([`RawFile::shooting_info`]) ; l'EXIF du JPEG
//! embarqué complète ce que LibRaw n'expose pas (correction d'exposition, mode, flash).
//! Pour une image plate, son propre EXIF (`kamadak-exif` : JPEG, TIFF, HEIF, PNG, WebP).
//! Le module fournit aussi les lignes à afficher ([`Metadata::rows`]), en français, pour
//! n'importe quelle interface. Voir `doc/metadata.md`.

use std::fs;
use std::io::{BufReader, Cursor};
use std::path::Path;

use exif::{Exif, In, Tag, Value};

use crate::bitmap::Rotation;
use crate::media::{MediaFile, MediaKind};
use crate::raw::{RawFile, Thumbnail};

/// Métadonnées d'une image ; `None` : inconnue.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    /// Fabricant de l'appareil.
    pub make: Option<String>,
    /// Modèle de l'appareil.
    pub model: Option<String>,
    /// Objectif.
    pub lens: Option<String>,
    /// Sensibilité ISO.
    pub iso: Option<f32>,
    /// Temps de pose (secondes).
    pub shutter: Option<f32>,
    /// Ouverture (nombre f).
    pub aperture: Option<f32>,
    /// Focale (mm).
    pub focal: Option<f32>,
    /// Focale équivalente 24 × 36 (mm).
    pub focal_35mm: Option<f32>,
    /// Correction d'exposition (IL).
    pub exposure_bias: Option<f32>,
    /// Mode d'exposition (« Priorité ouverture »…).
    pub program: Option<String>,
    /// Flash déclenché.
    pub flash: Option<bool>,
    /// Date de prise de vue, « AAAA-MM-JJ HH:MM:SS ».
    pub date: Option<String>,
    /// Dimensions de l'image affichée (pixels, orientation appliquée).
    pub size: Option<(u32, u32)>,
    /// Taille du fichier (octets).
    pub file_size: u64,
    /// Orientation EXIF d'un quart de tour (5 à 8) : dimensions à échanger.
    quarter_turn: bool,
}

/// Erreurs de lecture.
#[derive(Debug)]
pub enum MetadataError {
    /// Fichier illisible.
    Io(std::io::Error),
    /// RAW illisible par LibRaw.
    Raw(crate::raw::RawError),
}

impl std::fmt::Display for MetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(
                f,
                "{}",
                crate::t!("metadata-unreadable", error = e.to_string())
            ),
            Self::Raw(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for MetadataError {}

/// Lit les métadonnées de `file`. Rapide (ouverture du RAW sans décompression, ~10 ms
/// pour l'EXIF du JPEG embarqué), mais à faire hors du fil de l'interface (disque lent,
/// NAS). Une image sans EXIF donne des champs vides, pas une erreur.
pub fn read(file: &MediaFile) -> Result<Metadata, MetadataError> {
    let file_size = fs::metadata(&file.path).map_err(MetadataError::Io)?.len();
    let mut m = match file.kind {
        MediaKind::Raw => read_raw(&file.path)?,
        MediaKind::Flat => read_flat(&file.path),
    };
    m.file_size = file_size;
    Ok(m)
}

fn read_raw(path: &Path) -> Result<Metadata, MetadataError> {
    let mut raw = RawFile::open(path).map_err(MetadataError::Raw)?;
    let info = raw.shooting_info();
    let rotation = raw.orientation();
    let mut m = Metadata {
        make: info.make,
        model: info.model,
        lens: info.lens,
        iso: info.iso,
        shutter: info.shutter,
        aperture: info.aperture,
        focal: info.focal,
        focal_35mm: info.focal_35mm,
        date: info.date,
        size: info.size.map(|(w, h)| match rotation {
            Rotation::Cw90 | Rotation::Cw270 => (h, w),
            _ => (w, h),
        }),
        ..Metadata::default()
    };
    // L'EXIF du JPEG embarqué complète (LibRaw fait foi pour ce qu'il connaît).
    if let Ok(Thumbnail::Jpeg(bytes)) = raw.thumbnail()
        && let Ok(exif) = exif::Reader::new().read_from_container(&mut Cursor::new(bytes))
    {
        m.fill_from(&exif);
    }
    Ok(m)
}

fn read_flat(path: &Path) -> Metadata {
    let mut m = Metadata::default();
    if let Ok(file) = fs::File::open(path)
        && let Ok(exif) = exif::Reader::new().read_from_container(&mut BufReader::new(file))
    {
        m.fill_from(&exif);
    }
    if m.size.is_none() {
        // En-tête seulement (JPEG, PNG, TIFF, WebP ; pas HEIF).
        m.size = image::image_dimensions(path).ok();
    }
    // Dimensions stockées : les échanger si l'image s'affiche tournée d'un quart de tour.
    if m.quarter_turn {
        m.size = m.size.map(|(w, h)| (h, w));
    }
    m
}

fn field(exif: &Exif, tag: Tag) -> Option<&Value> {
    exif.get_field(tag, In::PRIMARY).map(|f| &f.value)
}

fn ascii(exif: &Exif, tag: Tag) -> Option<String> {
    match field(exif, tag)? {
        Value::Ascii(parts) => {
            let text = String::from_utf8_lossy(parts.first()?)
                .trim_matches(|c: char| c == '\0' || c.is_whitespace())
                .to_string();
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}

fn number(exif: &Exif, tag: Tag) -> Option<f32> {
    let v = match field(exif, tag)? {
        Value::Rational(r) => r.first()?.to_f64(),
        Value::SRational(r) => r.first()?.to_f64(),
        other => other.get_uint(0)? as f64,
    };
    v.is_finite().then_some(v as f32)
}

impl Metadata {
    /// Complète les champs inconnus avec l'EXIF `exif`.
    fn fill_from(&mut self, exif: &Exif) {
        fn set<T>(slot: &mut Option<T>, value: Option<T>) {
            if slot.is_none() {
                *slot = value;
            }
        }
        let positive = |v: Option<f32>| v.filter(|&x| x > 0.0);
        set(&mut self.make, ascii(exif, Tag::Make));
        set(&mut self.model, ascii(exif, Tag::Model));
        set(&mut self.lens, ascii(exif, Tag::LensModel));
        set(
            &mut self.iso,
            positive(number(exif, Tag::PhotographicSensitivity)),
        );
        set(&mut self.shutter, positive(number(exif, Tag::ExposureTime)));
        set(&mut self.aperture, positive(number(exif, Tag::FNumber)));
        set(&mut self.focal, positive(number(exif, Tag::FocalLength)));
        set(
            &mut self.focal_35mm,
            positive(number(exif, Tag::FocalLengthIn35mmFilm)),
        );
        set(
            &mut self.exposure_bias,
            number(exif, Tag::ExposureBiasValue),
        );
        set(
            &mut self.program,
            number(exif, Tag::ExposureProgram).and_then(|p| program_name(p as u32)),
        );
        set(
            &mut self.flash,
            number(exif, Tag::Flash).map(|f| (f as u32) & 1 == 1),
        );
        set(
            &mut self.date,
            ascii(exif, Tag::DateTimeOriginal).map(|d| {
                // « AAAA:MM:JJ HH:MM:SS » → « AAAA-MM-JJ HH:MM:SS ».
                d.char_indices()
                    .map(|(i, c)| if c == ':' && i < 10 { '-' } else { c })
                    .collect()
            }),
        );
        self.quarter_turn =
            number(exif, Tag::Orientation).is_some_and(|o| (5.0..=8.0).contains(&o));
        if self.size.is_none()
            && let (Some(w), Some(h)) = (
                number(exif, Tag::PixelXDimension),
                number(exif, Tag::PixelYDimension),
            )
        {
            self.size = Some((w as u32, h as u32));
        }
    }

    /// Appareil : fabricant et modèle, sans répéter le fabricant quand le modèle le
    /// contient déjà (« Canon EOS R6 », pas « Canon Canon EOS R6 »).
    pub fn camera(&self) -> Option<String> {
        match (&self.make, &self.model) {
            (Some(make), Some(model)) if model.to_lowercase().starts_with(&make.to_lowercase()) => {
                Some(model.clone())
            }
            (Some(make), Some(model)) => Some(format!("{make} {model}")),
            (None, Some(x)) | (Some(x), None) => Some(x.clone()),
            (None, None) => None,
        }
    }

    /// Temps de pose lisible : « 1/250 s », « 1,3 s », « 30 s ».
    pub fn shutter_text(&self) -> Option<String> {
        let s = self.shutter?;
        Some(if s >= 0.3 {
            format!("{} s", decimal_text(s as f64, 1))
        } else {
            format!("1/{} s", (1.0 / s).round() as u32)
        })
    }

    /// Exposition sur une ligne : « 1/250 s · f/2,8 · ISO 400 ».
    pub fn exposure_text(&self) -> Option<String> {
        let parts: Vec<String> = [
            self.shutter_text(),
            self.aperture
                .map(|a| format!("f/{}", decimal_text(a as f64, 1))),
            self.iso.map(|i| format!("ISO {}", i.round() as u32)),
        ]
        .into_iter()
        .flatten()
        .collect();
        (!parts.is_empty()).then(|| parts.join(" · "))
    }

    /// Lignes à afficher (libellé, valeur), dans l'ordre, dans la langue choisie
    /// ([`crate::i18n`]) ; les champs inconnus sont omis.
    pub fn rows(&self) -> Vec<(String, String)> {
        let mut rows = Vec::new();
        let mut push = |label: String, value: Option<String>| {
            if let Some(v) = value {
                rows.push((label, v));
            }
        };
        push(crate::t!("metadata-camera"), self.camera());
        push(crate::t!("metadata-lens"), self.lens.clone());
        push(crate::t!("metadata-exposure"), self.exposure_text());
        push(
            crate::t!("metadata-bias"),
            self.exposure_bias.map(|b| {
                let sign = if b > 0.0 { "+" } else { "" };
                crate::t!(
                    "metadata-bias-value",
                    value = format!("{sign}{}", decimal_text(b as f64, 1))
                )
            }),
        );
        push(crate::t!("metadata-mode"), self.program.clone());
        push(
            crate::t!("metadata-focal"),
            self.focal.map(|f| match self.focal_35mm {
                Some(eq) if (eq - f).abs() >= 1.0 => crate::t!(
                    "metadata-focal-equivalent",
                    focal = decimal_text(f as f64, 1),
                    equivalent = eq.round() as i64
                ),
                _ => crate::t!("metadata-focal-value", focal = decimal_text(f as f64, 1)),
            }),
        );
        push(
            crate::t!("metadata-flash"),
            self.flash.map(|f| {
                if f {
                    crate::t!("metadata-flash-fired")
                } else {
                    crate::t!("metadata-flash-not-fired")
                }
            }),
        );
        push(
            crate::t!("metadata-date"),
            self.date.as_deref().map(date_text),
        );
        push(
            crate::t!("metadata-dimensions"),
            self.size.map(|(w, h)| {
                let mpx = w as f64 * h as f64 / 1e6;
                crate::t!(
                    "metadata-dimensions-value",
                    width = w,
                    height = h,
                    mpx = decimal_text(mpx, 1)
                )
            }),
        );
        push(
            crate::t!("metadata-file"),
            (self.file_size > 0).then(|| match self.file_size {
                n if n < 1_000_000 => {
                    crate::t!("metadata-size-kb", size = decimal_text(n as f64 / 1e3, 0))
                }
                n => crate::t!("metadata-size-mb", size = decimal_text(n as f64 / 1e6, 1)),
            }),
        );
        rows
    }
}

/// Mode d'exposition EXIF (`ExposureProgram`).
fn program_name(code: u32) -> Option<String> {
    let id = match code {
        1 => "program-manual",
        2 => "program-normal",
        3 => "program-aperture",
        4 => "program-shutter",
        5 => "program-creative",
        6 => "program-action",
        7 => "program-portrait",
        8 => "program-landscape",
        _ => return None,
    };
    Some(crate::i18n::translate(id, None))
}

/// « AAAA-MM-JJ HH:MM:SS » mise en forme selon la langue (« JJ/MM/AAAA à HH:MM:SS » en
/// français) ; inchangée si autre forme.
fn date_text(date: &str) -> String {
    match (
        date.get(0..4),
        date.get(5..7),
        date.get(8..10),
        date.get(11..),
    ) {
        (Some(y), Some(m), Some(d), Some(t)) if date.len() >= 19 => {
            crate::t!(
                "metadata-date-value",
                year = y,
                month = m,
                day = d,
                time = t
            )
        }
        _ => date.to_string(),
    }
}

/// Nombre avec au plus `decimals` chiffres après la séparation, sans zéro final inutile
/// (« 2,8 », « 4 », « 1,3 » en français), à la façon de la langue.
fn decimal_text(value: f64, decimals: usize) -> String {
    let text = crate::i18n::decimal(value, decimals);
    let separator = if text.contains(',') { ',' } else { '.' };
    if text.contains(separator) {
        text.trim_end_matches('0')
            .trim_end_matches(separator)
            .to_string()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Metadata {
        Metadata {
            make: Some("Fujifilm".into()),
            model: Some("X-T4".into()),
            lens: Some("XF16-80mmF4 R OIS WR".into()),
            iso: Some(640.0),
            shutter: Some(1.0 / 250.0),
            aperture: Some(4.0),
            focal: Some(16.0),
            focal_35mm: Some(24.0),
            exposure_bias: Some(0.67),
            program: Some("Priorité ouverture".into()),
            flash: Some(false),
            date: Some("2026-10-16 14:32:05".into()),
            size: Some((4160, 6240)),
            file_size: 26_300_000,
            quarter_turn: false,
        }
    }

    #[test]
    fn readable_rows_in_french() {
        let rows = sample().rows();
        let get = |label: &str| {
            rows.iter()
                .find(|(l, _)| *l == label)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("Appareil"), Some("Fujifilm X-T4"));
        assert_eq!(get("Exposition"), Some("1/250 s · f/4 · ISO 640"));
        assert_eq!(get("Correction"), Some("+0,7 IL"));
        assert_eq!(get("Focale"), Some("16 mm (éq. 24 × 36 : 24 mm)"));
        assert_eq!(get("Date"), Some("16/10/2026 à 14:32:05"));
        assert_eq!(get("Dimensions"), Some("4160 × 6240 (26 Mpx)"));
        assert_eq!(get("Fichier"), Some("26,3 Mo"));
        assert_eq!(get("Flash"), Some("non déclenché"));
    }

    #[test]
    fn unknown_fields_are_omitted() {
        let rows = Metadata {
            file_size: 1_000_000,
            ..Metadata::default()
        }
        .rows();
        assert_eq!(rows, [("Fichier".to_string(), "1 Mo".to_string())]);
    }

    #[test]
    fn camera_name_does_not_repeat_the_make() {
        let m = Metadata {
            make: Some("Canon".into()),
            model: Some("Canon EOS R6".into()),
            ..Metadata::default()
        };
        assert_eq!(m.camera().as_deref(), Some("Canon EOS R6"));
    }

    #[test]
    fn shutter_speeds() {
        let t = |s: f32| {
            Metadata {
                shutter: Some(s),
                ..Metadata::default()
            }
            .shutter_text()
            .unwrap()
        };
        assert_eq!(t(1.0 / 8000.0), "1/8000 s");
        assert_eq!(t(1.0 / 30.0), "1/30 s");
        assert_eq!(t(1.3), "1,3 s");
        assert_eq!(t(30.0), "30 s");
        assert_eq!(t(0.5), "0,5 s");
    }

    #[test]
    fn short_focal_keeps_its_decimal() {
        let m = Metadata {
            focal: Some(5.7),
            focal_35mm: Some(26.0),
            ..Metadata::default()
        };
        assert_eq!(
            m.rows(),
            [(
                "Focale".to_string(),
                "5,7 mm (éq. 24 × 36 : 26 mm)".to_string()
            )]
        );
    }

    #[test]
    fn same_focal_in_35mm_is_not_repeated() {
        let m = Metadata {
            focal: Some(50.0),
            focal_35mm: Some(50.0),
            ..Metadata::default()
        };
        assert_eq!(m.rows(), [("Focale".to_string(), "50 mm".to_string())]);
    }
}
