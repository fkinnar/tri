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

/// Erreurs de calcul d'un aperçu.
#[derive(Debug)]
pub enum PreviewError {
    /// Lecture ou développement du RAW.
    Raw(RawError),
    /// Décodage d'une image plate.
    Flat(FlatError),
    /// Accès au fichier source ou au cache.
    Io(io::Error),
    /// Calcul annulé.
    Cancelled,
}

impl fmt::Display for PreviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Raw(e) => e.fmt(f),
            Self::Flat(e) => e.fmt(f),
            Self::Io(e) => write!(f, "accès fichier impossible : {e}"),
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
/// taille ou une règle donne de nouvelles clés, donc un recalcul.
pub fn params(level: Level, sizes: &Sizes) -> String {
    let defaults = RenderOptions::default();
    let render = format!("wb={};bright={}", defaults.camera_wb, defaults.bright);
    match level {
        Level::Thumbnail => format!("r{RULES_VERSION};side={}", sizes.thumbnail),
        Level::Standard => format!("r{RULES_VERSION};side={};{render}", sizes.standard),
        Level::Full => format!("r{RULES_VERSION};{render}"),
    }
}

/// Clé de cache de `file` au niveau `level`.
pub fn key(file: &MediaFile, level: Level, sizes: &Sizes) -> io::Result<Key> {
    Key::new(&file.path, level, &params(level, sizes))
}

/// Vrai si `half_size` suffit pour un aperçu de `side` pixels : la moitié du côté long du
/// rendu complet doit couvrir la taille demandée.
pub fn half_size_suffices(full_size: (u32, u32), side: u32) -> bool {
    full_size.0.max(full_size.1) / 2 >= side
}

/// Calcule l'image de `file` au niveau `level` (orientation appliquée), sans cache.
pub fn compute(
    file: &MediaFile,
    level: Level,
    sizes: &Sizes,
    cancel: &AtomicBool,
) -> Result<Bitmap> {
    if cancel.load(Ordering::Relaxed) {
        return Err(PreviewError::Cancelled);
    }
    match (file.kind, level) {
        (MediaKind::Raw, Level::Thumbnail) => raw_thumbnail(file, sizes, cancel),
        (MediaKind::Raw, Level::Standard) => {
            let mut raw = RawFile::open(&file.path)?;
            let options = RenderOptions {
                half_size: half_size_suffices(raw.size(), sizes.standard),
                ..RenderOptions::default()
            };
            Ok(raw
                .render_cancellable(&options, cancel)?
                .fit(sizes.standard))
        }
        (MediaKind::Raw, Level::Full) => {
            Ok(RawFile::open(&file.path)?.render_cancellable(&RenderOptions::default(), cancel)?)
        }
        (MediaKind::Flat, Level::Thumbnail) => Ok(flat::decode_fit(&file.path, sizes.thumbnail)?),
        (MediaKind::Flat, Level::Standard) => Ok(flat::decode_fit(&file.path, sizes.standard)?),
        (MediaKind::Flat, Level::Full) => Ok(flat::decode(&file.path)?),
    }
}

/// Vignette d'un RAW : aperçu embarqué, réduit puis redressé selon l'orientation du RAW.
/// Sans aperçu exploitable, repli sur un rendu `half_size`.
fn raw_thumbnail(file: &MediaFile, sizes: &Sizes, cancel: &AtomicBool) -> Result<Bitmap> {
    let mut raw = RawFile::open(&file.path)?;
    let rotation = raw.orientation();
    let embedded = match raw.thumbnail() {
        // jpeg::decode n'applique pas la balise EXIF : c'est voulu, l'orientation du RAW
        // fait foi (voir RawFile::orientation). Décodage directement réduit (IDCT).
        Ok(Thumbnail::Jpeg(bytes)) => jpeg::decode(&bytes, sizes.thumbnail).ok(),
        Ok(Thumbnail::Bitmap(bmp)) => Some(bmp),
        Err(_) => None,
    };
    match embedded {
        Some(bmp) => Ok(bmp.fit(sizes.thumbnail).rotate(rotation)),
        None => {
            let options = RenderOptions {
                half_size: true,
                ..RenderOptions::default()
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
    sizes: &Sizes,
    cancel: &AtomicBool,
) -> Result<Outcome> {
    let key = key(file, level, sizes)?;
    if cache.contains(&key) {
        return Ok(Outcome::Cached);
    }
    let bitmap = compute(file, level, sizes, cancel)?;
    cache.put(&key, &bitmap)?;
    Ok(Outcome::Computed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_size_rule() {
        // RAF X-T4 : 6240×4160 → half 3120.
        assert!(half_size_suffices((6240, 4160), 2560));
        assert!(half_size_suffices((6240, 4160), 3120));
        assert!(!half_size_suffices((6240, 4160), 3121));
        assert!(!half_size_suffices((6240, 4160), 3840));
        assert!(
            half_size_suffices((4160, 6240), 2560),
            "orientation indifférente"
        );
    }

    #[test]
    fn params_differ_by_level_and_size() {
        let s = Sizes::default();
        let all = Level::ALL.map(|l| params(l, &s));
        assert_ne!(all[0], all[1]);
        assert_ne!(all[1], all[2]);
        let bigger = Sizes {
            standard: 3840,
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
    fn cancelled_before_start() {
        let file = MediaFile {
            path: "nimporte.jpg".into(),
            kind: MediaKind::Flat,
        };
        let res = compute(
            &file,
            Level::Full,
            &Sizes::default(),
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
