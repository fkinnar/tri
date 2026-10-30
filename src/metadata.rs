//! Métadonnées de prise de vue d'une image : appareil, objectif, exposition, focale,
//! date, dimensions, taille du fichier.
//!
//! Pour un RAW, LibRaw fournit l'essentiel ([`RawFile::shooting_info`]) ; l'EXIF du JPEG
//! embarqué complète ce que LibRaw n'expose pas (correction d'exposition, mode, flash).
//! Pour une image plate, son propre EXIF (`kamadak-exif` : JPEG, TIFF, HEIF, PNG, WebP).
//! Le module fournit aussi les lignes à afficher ([`Metadata::rows`]), en français, pour
//! n'importe quelle interface. Voir `doc/en/metadata.md`.

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
    /// RAW : dimensions du JPEG embarqué par le boîtier (pixels, orientation appliquée),
    /// lues dans son en-tête : la résolution du « JPEG boîtier » à 100 %.
    pub embedded_size: Option<(u32, u32)>,
    /// Taille du fichier (octets).
    pub file_size: u64,
    /// Collimateur de mise au point, en coordonnées relatives (0 à 1) de l'image affichée
    /// (orientation appliquée). Lu dans la note du fabricant : Fujifilm (`FocusPixel`)
    /// pour l'instant.
    pub focus_point: Option<(f32, f32)>,
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
    // Clone darktable : le fichier de l'image d'origine.
    let path = crate::media::source(&file.path);
    let file_size = fs::metadata(&path).map_err(MetadataError::Io)?.len();
    let mut m = match file.kind {
        MediaKind::Raw => read_raw(&file.path)?,
        MediaKind::Flat => read_flat(&path),
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
    if let Ok(Thumbnail::Jpeg(bytes)) = raw.thumbnail() {
        // Sa taille : l'en-tête seulement. Il n'est pas pivoté : l'orientation du RAW.
        m.embedded_size = image::ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_dimensions().ok())
            .map(|(w, h)| match rotation {
                Rotation::Cw90 | Rotation::Cw270 => (h, w),
                _ => (w, h),
            });
        if let Ok(exif) = exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) {
            m.fill_from(&exif);
        }
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
        let orientation = number(exif, Tag::Orientation).map_or(1, |o| o as u32);
        self.quarter_turn = (5..=8).contains(&orientation);
        if self.focus_point.is_none()
            && let (Some(Value::Undefined(note, _)), Some(w), Some(h)) = (
                field(exif, Tag::MakerNote),
                number(exif, Tag::PixelXDimension),
                number(exif, Tag::PixelYDimension),
            )
            && let Some((x, y)) = fuji_focus_pixel(note)
            && w > 0.0
            && h > 0.0
        {
            let point = (x as f32 / w, y as f32 / h);
            if (0.0..=1.0).contains(&point.0) && (0.0..=1.0).contains(&point.1) {
                self.focus_point = Some(oriented(point, orientation));
            }
        }
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

/// Collimateur d'une note du fabricant Fujifilm : balise `FocusPixel` (0x1023, deux
/// entiers 16 bits x, y, en pixels de l'image du JPEG, avant orientation). Format : « FUJIFILM »,
/// position de l'IFD (32 bits), puis l'IFD, petit-boutiste, positions relatives au début de
/// la note. `None` pour une autre marque ou une note illisible.
fn fuji_focus_pixel(note: &[u8]) -> Option<(u16, u16)> {
    let rest = note.strip_prefix(b"FUJIFILM")?;
    let u16_at = |i: usize| -> Option<u16> {
        Some(u16::from_le_bytes(note.get(i..i + 2)?.try_into().ok()?))
    };
    let offset = u32::from_le_bytes(rest.get(..4)?.try_into().ok()?) as usize;
    let count = u16_at(offset)? as usize;
    (0..count).find_map(|i| {
        let entry = offset + 2 + i * 12;
        let (tag, kind) = (u16_at(entry)?, u16_at(entry + 2)?);
        // Type 3 (entiers 16 bits), 2 valeurs : rangées dans le champ de valeur lui-même.
        (tag == 0x1023 && kind == 3).then(|| Some((u16_at(entry + 8)?, u16_at(entry + 10)?)))?
    })
}

/// Point relatif (x, y) d'une image stockée, ramené dans l'image affichée selon
/// l'orientation EXIF (1 à 8).
fn oriented((x, y): (f32, f32), orientation: u32) -> (f32, f32) {
    match orientation {
        2 => (1.0 - x, y),
        3 => (1.0 - x, 1.0 - y),
        4 => (x, 1.0 - y),
        5 => (y, x),
        6 => (1.0 - y, x),
        7 => (1.0 - y, 1.0 - x),
        8 => (y, 1.0 - x),
        _ => (x, y),
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
            embedded_size: Some((2944, 4416)),
            file_size: 26_300_000,
            focus_point: None,
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

    /// Note du fabricant Fujifilm minimale : en-tête, IFD de deux entrées dont
    /// `FocusPixel`.
    fn fuji_note(x: u16, y: u16) -> Vec<u8> {
        let mut note = b"FUJIFILM".to_vec();
        note.extend_from_slice(&12u32.to_le_bytes());
        note.extend_from_slice(&2u16.to_le_bytes());
        // Entrée quelconque (version), puis FocusPixel.
        for (tag, kind, count, value) in [
            (0u16, 7u16, 4u32, [0u8; 4]),
            (0x1023, 3, 2, {
                let mut v = [0u8; 4];
                v[..2].copy_from_slice(&x.to_le_bytes());
                v[2..].copy_from_slice(&y.to_le_bytes());
                v
            }),
        ] {
            note.extend_from_slice(&tag.to_le_bytes());
            note.extend_from_slice(&kind.to_le_bytes());
            note.extend_from_slice(&count.to_le_bytes());
            note.extend_from_slice(&value);
        }
        note
    }

    #[test]
    fn fuji_focus_pixel_is_read_from_the_maker_note() {
        assert_eq!(fuji_focus_pixel(&fuji_note(2517, 1317)), Some((2517, 1317)));
        assert_eq!(fuji_focus_pixel(b"Nikon\0\x02\x10"), None, "autre marque");
        assert_eq!(fuji_focus_pixel(&fuji_note(1, 2)[..20]), None, "tronquée");
    }

    #[test]
    fn the_focus_point_follows_the_orientation() {
        // Vérifié sur un RAF X-T4 tenu verticalement (orientation 6) : le collimateur
        // (2517, 1317) d'un JPEG 4416 × 2944 tombe à (0,553 ; 0,570) de l'image affichée.
        let p = oriented((2517.0 / 4416.0, 1317.0 / 2944.0), 6);
        assert!(
            (p.0 - 0.553).abs() < 0.001 && (p.1 - 0.570).abs() < 0.001,
            "{p:?}"
        );
        assert_eq!(oriented((0.25, 0.75), 1), (0.25, 0.75));
        assert_eq!(oriented((0.25, 0.75), 3), (0.75, 0.25));
        assert_eq!(oriented((0.25, 0.75), 8), (0.75, 0.75));
    }
}
