//! Calcul des images mises en cache : vignette, aperçu standard, rendu 1:1.
//!
//! Fonctions synchrones, appelées par les workers du [`scheduler`](crate::scheduler).
//! Voir `doc/preview.md`.

use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bitmap::Bitmap;
use crate::cache::{Cache, Key, Level};
use crate::flat::{self, FlatError};
use crate::jpeg;
use crate::media::{MediaFile, MediaKind};
use crate::raw::{RawError, RawFile, RenderOptions, Thumbnail};

/// Tailles (plus grand côté, en pixels) des niveaux réduits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sizes {
    /// Vignette de la grille.
    pub thumbnail: u32,
    /// Aperçu standard (plein écran). Le mode « Auto » de l'interface y met le plus grand
    /// côté de l'écran principal.
    pub standard: u32,
}

impl Default for Sizes {
    fn default() -> Self {
        Self {
            thumbnail: 400,
            standard: 2560,
        }
    }
}

/// Réglages du calcul des images : tailles, développement RAW, seuils d'acceptation.
///
/// Chaque réglage figure dans la clé de cache des niveaux qu'il influence (voir
/// [`params`]) : en changer donne un recalcul de ces niveaux seulement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Tailles des niveaux réduits.
    pub sizes: Sizes,
    /// Options du développement LibRaw. `half_size` y est ignoré : il est choisi par niveau
    /// (voir [`half_size_suffices`]).
    pub render: RenderOptions,
    /// Règle `half_size` de l'aperçu standard : la demi-résolution est utilisée si la moitié
    /// du côté long du RAW vaut au moins `half_size_margin` fois le côté demandé. `1.0` :
    /// dès qu'elle couvre la taille demandée (PSNR 40–43 dB contre le rendu complet sur
    /// RAF X-T4, voir `doc/preview.md`) ; plus grand : marge de netteté.
    pub half_size_margin: f32,
    /// Aperçu boîtier refusé s'il est plus petit que cette fraction du côté demandé
    /// (certains boîtiers n'embarquent qu'une vignette de 160 px).
    pub embedded_min_ratio: f32,
    /// Plus petit JPEG embarqué accepté pour le 1:1 du JPEG boîtier (pixels, côté long).
    pub min_embedded_full: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sizes: Sizes::default(),
            render: RenderOptions::default(),
            half_size_margin: 1.0,
            embedded_min_ratio: 0.5,
            min_embedded_full: 1024,
        }
    }
}

/// Erreurs de calcul d'un aperçu.
#[derive(Debug)]
pub enum PreviewError {
    /// Lecture ou développement du RAW.
    Raw(RawError),
    /// Décodage d'une image plate.
    Flat(FlatError),
    /// Accès au fichier source ou au cache.
    Io(io::Error),
    /// Le RAW n'embarque pas d'aperçu assez grand pour servir d'aperçu plein écran.
    NoEmbeddedPreview,
    /// Calcul annulé.
    Cancelled,
}

impl fmt::Display for PreviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Raw(e) => e.fmt(f),
            Self::Flat(e) => e.fmt(f),
            Self::Io(e) => write!(f, "accès fichier impossible : {e}"),
            Self::NoEmbeddedPreview => write!(f, "pas d'aperçu embarqué exploitable"),
            Self::Cancelled => write!(f, "calcul annulé"),
        }
    }
}

impl std::error::Error for PreviewError {}

impl From<RawError> for PreviewError {
    fn from(e: RawError) -> Self {
        match e {
            RawError::Cancelled => Self::Cancelled,
            e => Self::Raw(e),
        }
    }
}

impl From<FlatError> for PreviewError {
    fn from(e: FlatError) -> Self {
        Self::Flat(e)
    }
}

impl From<io::Error> for PreviewError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// `Result` spécialisé pour le module preview.
pub type Result<T> = std::result::Result<T, PreviewError>;

/// Version des règles de calcul. L'incrémenter invalide toutes les entrées existantes
/// (à faire si l'algorithme change sans que les paramètres visibles changent).
const RULES_VERSION: u32 = 1;

/// Description stable des paramètres d'un niveau, incluse dans la clé de cache : changer une
/// taille, un réglage de rendu ou un seuil donne de nouvelles clés, donc un recalcul.
///
/// Seuls les réglages qui influencent le niveau y figurent. Exception assumée : le repli
/// `half_size` des vignettes de RAW sans aperçu embarqué (rare) n'y met pas le rendu, pour
/// ne pas invalider toutes les vignettes à chaque réglage de rendu.
pub fn params(level: Level, settings: &Settings) -> String {
    let (sizes, r) = (&settings.sizes, &settings.render);
    let render = format!(
        "wb={};bright={};autobright={}",
        r.camera_wb, r.bright, r.auto_bright
    );
    match level {
        Level::Thumbnail => format!("r{RULES_VERSION};side={}", sizes.thumbnail),
        Level::Embedded => format!(
            "r{RULES_VERSION};side={};src=embedded;min={}",
            sizes.standard, settings.embedded_min_ratio
        ),
        Level::Standard => format!(
            "r{RULES_VERSION};side={};{render};half={}",
            sizes.standard, settings.half_size_margin
        ),
        Level::EmbeddedFull => format!(
            "r{RULES_VERSION};src=embedded;full;min={}",
            settings.min_embedded_full
        ),
        Level::Full => format!("r{RULES_VERSION};{render}"),
    }
}

/// Clé de cache de `file` au niveau `level`.
pub fn key(file: &MediaFile, level: Level, settings: &Settings) -> io::Result<Key> {
    Key::new(&file.path, level, &params(level, settings))
}

/// Vrai si `half_size` suffit pour un aperçu de `side` pixels : la moitié du côté long du
/// rendu complet doit valoir au moins `margin` fois la taille demandée (voir
/// [`Settings::half_size_margin`]).
pub fn half_size_suffices(full_size: (u32, u32), side: u32, margin: f32) -> bool {
    (full_size.0.max(full_size.1) / 2) as f32 >= margin * side as f32
}

/// Calcule l'image de `file` au niveau `level` (orientation appliquée), sans cache.
pub fn compute(
    file: &MediaFile,
    level: Level,
    settings: &Settings,
    cancel: &AtomicBool,
) -> Result<Bitmap> {
    if cancel.load(Ordering::Relaxed) {
        return Err(PreviewError::Cancelled);
    }
    let sizes = &settings.sizes;
    match (file.kind, level) {
        (MediaKind::Raw, Level::Thumbnail) => raw_thumbnail(file, settings, cancel),
        (MediaKind::Raw, Level::Embedded) => {
            let min = (sizes.standard as f32 * settings.embedded_min_ratio) as u32;
            raw_embedded(file, sizes.standard, min)
        }
        (MediaKind::Raw, Level::EmbeddedFull) => raw_embedded(file, 0, settings.min_embedded_full),
        (MediaKind::Raw, Level::Standard) => {
            let mut raw = RawFile::open(&file.path)?;
            let options = RenderOptions {
                half_size: half_size_suffices(
                    raw.size(),
                    sizes.standard,
                    settings.half_size_margin,
                ),
                ..settings.render
            };
            Ok(raw
                .render_cancellable(&options, cancel)?
                .fit(sizes.standard))
        }
        (MediaKind::Raw, Level::Full) => {
            let options = RenderOptions {
                half_size: false,
                ..settings.render
            };
            Ok(RawFile::open(&file.path)?.render_cancellable(&options, cancel)?)
        }
        (MediaKind::Flat, Level::Thumbnail) => Ok(flat::decode_fit(&file.path, sizes.thumbnail)?),
        // Une image plate n'a pas d'aperçu « boîtier » distinct : c'est son aperçu standard.
        (MediaKind::Flat, Level::Embedded | Level::Standard) => {
            Ok(flat::decode_fit(&file.path, sizes.standard)?)
        }
        // Pas de JPEG « embarqué » distinct pour une image plate : c'est l'image elle-même.
        (MediaKind::Flat, Level::EmbeddedFull | Level::Full) => Ok(flat::decode(&file.path)?),
    }
}

/// Aperçu embarqué d'un RAW déjà ouvert, décodé au moins à `min_side` si possible (non
/// redressé). `None` s'il n'y en a pas ou s'il est illisible.
fn embedded_bitmap(raw: &mut RawFile, min_side: u32) -> Option<Bitmap> {
    match raw.thumbnail() {
        // jpeg::decode n'applique pas la balise EXIF : c'est voulu, l'orientation du RAW
        // fait foi (voir RawFile::orientation). Décodage directement réduit (IDCT).
        Ok(Thumbnail::Jpeg(bytes)) => jpeg::decode(&bytes, min_side).ok(),
        Ok(Thumbnail::Bitmap(bmp)) => Some(bmp),
        Err(_) => None,
    }
}

/// JPEG embarqué redressé, réduit à `side` (`0` : taille d'origine, pour le 1:1). Refusé
/// s'il est trop petit pour servir (côté long inférieur à `min`).
fn raw_embedded(file: &MediaFile, side: u32, min: u32) -> Result<Bitmap> {
    let mut raw = RawFile::open(&file.path)?;
    let rotation = raw.orientation();
    let bmp = embedded_bitmap(&mut raw, side)
        .filter(|b| b.width.max(b.height) >= min)
        .ok_or(PreviewError::NoEmbeddedPreview)?;
    Ok(bmp.fit(side).rotate(rotation))
}

/// Vignette d'un RAW : aperçu embarqué, réduit puis redressé selon l'orientation du RAW.
/// Sans aperçu exploitable, repli sur un rendu `half_size`.
fn raw_thumbnail(file: &MediaFile, settings: &Settings, cancel: &AtomicBool) -> Result<Bitmap> {
    let sizes = &settings.sizes;
    let mut raw = RawFile::open(&file.path)?;
    let rotation = raw.orientation();
    match embedded_bitmap(&mut raw, sizes.thumbnail) {
        Some(bmp) => Ok(bmp.fit(sizes.thumbnail).rotate(rotation)),
        None => {
            let options = RenderOptions {
                half_size: true,
                ..settings.render
            };
            Ok(raw
                .render_cancellable(&options, cancel)?
                .fit(sizes.thumbnail))
        }
    }
}

/// Résultat de [`ensure`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// L'entrée était déjà en cache.
    Cached,
    /// L'entrée vient d'être calculée et écrite.
    Computed,
}

/// Garantit que l'entrée de `file` au niveau `level` est en cache : ne calcule que si elle
/// manque.
pub fn ensure(
    cache: &Cache,
    file: &MediaFile,
    level: Level,
    settings: &Settings,
    cancel: &AtomicBool,
) -> Result<Outcome> {
    let key = key(file, level, settings)?;
    if cache.contains(&key) {
        return Ok(Outcome::Cached);
    }
    let bitmap = compute(file, level, settings, cancel)?;
    cache.put(&key, &bitmap)?;
    Ok(Outcome::Computed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_size_rule() {
        // RAF X-T4 : 6240×4160 → half 3120.
        assert!(half_size_suffices((6240, 4160), 2560, 1.0));
        assert!(half_size_suffices((6240, 4160), 3120, 1.0));
        assert!(!half_size_suffices((6240, 4160), 3121, 1.0));
        assert!(!half_size_suffices((6240, 4160), 3840, 1.0));
        assert!(
            half_size_suffices((4160, 6240), 2560, 1.0),
            "orientation indifférente"
        );
    }

    #[test]
    fn half_size_margin_demands_more_resolution() {
        // 3120 / 2560 ≈ 1,22 : suffit avec 1,2 de marge, pas avec 1,25.
        assert!(half_size_suffices((6240, 4160), 2560, 1.2));
        assert!(!half_size_suffices((6240, 4160), 2560, 1.25));
    }

    #[test]
    fn params_differ_by_level_and_size() {
        let s = Settings::default();
        let mut all: Vec<String> = Level::ALL.iter().map(|l| params(*l, &s)).collect();
        all.sort();
        all.dedup();
        assert_eq!(
            all.len(),
            Level::ALL.len(),
            "paramètres distincts par niveau"
        );
        let bigger = Settings {
            sizes: Sizes {
                standard: 3840,
                ..s.sizes
            },
            ..s
        };
        assert_ne!(
            params(Level::Standard, &s),
            params(Level::Standard, &bigger)
        );
        assert_eq!(params(Level::Full, &s), params(Level::Full, &bigger));
        assert_eq!(
            params(Level::Thumbnail, &s),
            params(Level::Thumbnail, &bigger)
        );
    }

    #[test]
    fn params_follow_only_the_settings_that_matter() {
        let s = Settings::default();
        let changed = |f: &dyn Fn(&mut Settings)| {
            let mut t = s;
            f(&mut t);
            Level::ALL
                .into_iter()
                .filter(|&l| params(l, &s) != params(l, &t))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            changed(&|t| t.render.auto_bright = true),
            [Level::Standard, Level::Full]
        );
        assert_eq!(changed(&|t| t.half_size_margin = 1.5), [Level::Standard]);
        assert_eq!(changed(&|t| t.embedded_min_ratio = 0.8), [Level::Embedded]);
        assert_eq!(
            changed(&|t| t.min_embedded_full = 2000),
            [Level::EmbeddedFull]
        );
    }

    #[test]
    fn cancelled_before_start() {
        let file = MediaFile {
            path: "nimporte.jpg".into(),
            kind: MediaKind::Flat,
        };
        let res = compute(
            &file,
            Level::Full,
            &Settings::default(),
            &AtomicBool::new(true),
        );
        assert!(matches!(res, Err(PreviewError::Cancelled)));
    }

    #[test]
    fn raw_cancellation_maps_to_preview_cancellation() {
        assert!(matches!(
            PreviewError::from(RawError::Cancelled),
            PreviewError::Cancelled
        ));
    }
}
