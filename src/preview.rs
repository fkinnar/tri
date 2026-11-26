//! Calcul des images mises en cache : vignette, aperçu standard, rendu 1:1.
//!
//! Fonctions synchrones, appelées par les workers du [`scheduler`](crate::scheduler).
//! Voir `doc/en/preview.md`.

use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bitmap::Bitmap;
use crate::cache::{Cache, Key, Level};
use crate::colour::Origin;
use crate::exposure::{Exposure, ExposureError, ExposureSettings};
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
    /// RAF X-T4, voir `doc/en/preview.md`) ; plus grand : marge de netteté.
    pub half_size_margin: f32,
    /// Aperçu boîtier refusé s'il est plus petit que cette fraction du côté demandé
    /// (certains boîtiers n'embarquent qu'une vignette de 160 px).
    pub embedded_min_ratio: f32,
    /// Plus petit JPEG embarqué accepté pour le 1:1 du JPEG boîtier (pixels, côté long).
    pub min_embedded_full: u32,
    /// Analyse de l'exposition (niveau [`Level::Exposure`]).
    pub exposure: ExposureSettings,
    /// Score de netteté (niveau [`Level::Sharpness`]).
    pub sharpness: crate::sharpness::SharpnessSettings,
    /// Empreinte de regroupement (niveau [`Level::Similarity`]).
    pub similarity: crate::similarity::SimilaritySettings,
    /// Palettes de couleurs (niveaux [`Level::Palette`]).
    pub palette: crate::palette::PaletteSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sizes: Sizes::default(),
            render: RenderOptions::default(),
            half_size_margin: 1.0,
            embedded_min_ratio: 0.5,
            min_embedded_full: 1024,
            exposure: ExposureSettings::default(),
            sharpness: crate::sharpness::SharpnessSettings::default(),
            similarity: crate::similarity::SimilaritySettings::default(),
            palette: crate::palette::PaletteSettings::default(),
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
    /// Niveau qui n'est pas une image ([`Level::Exposure`] : voir [`exposure`]).
    NotAnImage,
    /// Rendu par un logiciel de développement ([`Level::Developed`]).
    Develop(crate::develop::DevelopError),
}

impl fmt::Display for PreviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Raw(e) => e.fmt(f),
            Self::Flat(e) => e.fmt(f),
            Self::Io(e) => write!(f, "{}", crate::t!("preview-io", error = e.to_string())),
            Self::NoEmbeddedPreview => write!(f, "{}", crate::t!("preview-no-embedded")),
            Self::Cancelled => write!(f, "{}", crate::t!("preview-cancelled")),
            Self::NotAnImage => write!(f, "{}", crate::t!("preview-not-an-image")),
            Self::Develop(e) => e.fmt(f),
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

impl From<ExposureError> for PreviewError {
    fn from(e: ExposureError) -> Self {
        match e {
            ExposureError::Raw(e) => e.into(),
            ExposureError::Format => Self::Io(io::Error::other(e.to_string())),
        }
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
    // Images dans l'espace de travail de l'affichage (`colour`) : une règle de conversion
    // changée, les images sont refaites. Pas les rendus des logiciels de développement,
    // gardés tels quels et convertis à la lecture.
    let cs = format!("cs={}", crate::colour::VERSION);
    let render = format!(
        "wb={};bright={};autobright={}",
        r.camera_wb, r.bright, r.auto_bright
    );
    match level {
        Level::Thumbnail => format!("r{RULES_VERSION};{cs};side={}", sizes.thumbnail),
        Level::Embedded => format!(
            "r{RULES_VERSION};{cs};side={};src=embedded;min={}",
            sizes.standard, settings.embedded_min_ratio
        ),
        Level::Standard => format!(
            "r{RULES_VERSION};{cs};side={};{render};half={}",
            sizes.standard, settings.half_size_margin
        ),
        Level::EmbeddedFull => format!(
            "r{RULES_VERSION};{cs};src=embedded;full;min={}",
            settings.min_embedded_full
        ),
        Level::Full => format!("r{RULES_VERSION};{cs};{render}"),
        // Le sidecar s'y ajoute dans `key` (propre à chaque image).
        Level::Developed(engine) => {
            format!("r{RULES_VERSION};{};side={}", engine.id(), sizes.standard)
        }
        Level::Exposure => {
            let e = &settings.exposure;
            format!(
                "r{RULES_VERSION};v{};over={};under={};lin={};evr={};evb={};mask={};share={}",
                crate::exposure::VERSION,
                e.over_ev,
                e.under_ev,
                e.linear_bins,
                e.ev_range,
                e.ev_bins_per_stop,
                e.mask_size,
                e.mask_under_share
            )
        }
        Level::Similarity => format!(
            "r{RULES_VERSION};v{};side={}",
            crate::similarity::VERSION,
            settings.similarity.side
        ),
        // L'image d'où elle est tirée compte aussi : refaite, la palette l'est aussi.
        Level::Palette(source) => {
            let p = &settings.palette;
            format!(
                "r{RULES_VERSION};v{};colors={};side={};it={};merge={};min={};\
                 perceived={},{},{},{},{},{},{};from={}",
                crate::palette::VERSION,
                p.colors,
                p.side,
                p.iterations,
                p.merge,
                p.min_share,
                p.perceived_adaptation,
                p.perceived_surround,
                p.perceived_salience,
                p.perceived_saturation,
                p.perceived_saturation_colours,
                p.perceived_saturation_falloff,
                p.perceived_side,
                params(palette_input(source), settings)
            )
        }
        Level::Sharpness => {
            let s = &settings.sharpness;
            format!(
                "r{RULES_VERSION};v{};side={};tiles={};top={}",
                crate::sharpness::SCORE_VERSION,
                s.score_side,
                s.score_tiles,
                s.score_top_share
            )
        }
    }
}

/// Clé de cache de `file` au niveau `level`.
pub fn key(file: &MediaFile, level: Level, settings: &Settings) -> io::Result<Key> {
    key_with(file, level, settings, "")
}

/// Clé de la copie à `side` pixels (côté long) de l'entrée de `file` au niveau `level`,
/// pour la grille (11/11/2026) : relire un aperçu de 2560 px pour une vignette prenait
/// ~30 ms, la petite copie ~1 ms. Dérivée de la clé de l'entrée : l'entrée change, la
/// copie aussi. Rangée avec elle (vidée et comptée avec son niveau).
pub fn grid_key(file: &MediaFile, level: Level, settings: &Settings, side: u32) -> io::Result<Key> {
    key_with(
        file,
        level,
        settings,
        &format!(";grid={side};cs={}", crate::colour::VERSION),
    )
}

/// Clé de l'heure de prise de vue seule de `file` ([`capture_time`]) : rangée avec les
/// empreintes de similarité (qui la contiennent aussi), vidée et comptée avec elles.
pub fn captured_key(file: &MediaFile, settings: &Settings) -> io::Result<Key> {
    // « 2 » : relire les dates absentes gardées avant le repli sur l'EXIF (26/11/2026).
    key_with(file, Level::Similarity, settings, ";captured3")
}

/// Paramètres complets de l'entrée de `file` au niveau `level` : ceux des réglages
/// ([`params`]) et, pour un rendu développé ou sa palette, l'état du développement dans le
/// sidecar. Ce qui, avec le contenu du fichier, décide de l'entrée (index des palettes,
/// #561 : deux palettes au même tampon sont les mêmes).
pub fn stamp(file: &MediaFile, level: Level, settings: &Settings) -> String {
    let mut params = params(level, settings);
    if let Level::Developed(engine) | Level::Palette(crate::palette::Source::Developed(engine)) =
        level
    {
        let stamp = engine
            .sidecar(&file.path)
            .map_or_else(|| "aucun".to_string(), |s| crate::develop::render_stamp(&s));
        params.push_str(&format!(";xmp={stamp}"));
    }
    params
}

/// Les entrées déjà calculées des images copiées ou déplacées suivent leurs copies (#508) :
/// `files` donne (avant, après) chaque fichier, une fois la copie faite (nom, taille et date
/// gardés). Tous les niveaux aux réglages `settings`, et les copies pour la grille des
/// aperçus ([`grid_key`], à la taille des vignettes). Un rendu développé ne suit que si son sidecar a suivi
/// (son état entre dans la clé). `keep` : copie (les entrées restent aussi à l'ancienne
/// place) ; sinon déplacement. Renvoie le nombre d'entrées reprises.
pub fn carry(
    cache: &Cache,
    settings: &Settings,
    files: &[(std::path::PathBuf, std::path::PathBuf)],
    keep: bool,
) -> usize {
    let side = settings.sizes.thumbnail;
    let mut carried = 0;
    for (from, to) in files {
        let (Some(kind), Some(folder)) = (crate::media::kind_of(to), from.parent()) else {
            continue;
        };
        let file = MediaFile {
            path: to.clone(),
            kind,
        };
        for level in Level::ALL {
            let grid = (level.is_image() && level != Level::Thumbnail)
                .then(|| grid_key(&file, level, settings, side));
            let captured = (level == Level::Similarity).then(|| captured_key(&file, settings));
            for key in std::iter::once(key(&file, level, settings))
                .chain(grid)
                .chain(captured)
            {
                if let Ok(key) = key
                    && cache.carry(&key, folder, keep).unwrap_or(false)
                {
                    carried += 1;
                }
            }
        }
    }
    carried
}

fn key_with(file: &MediaFile, level: Level, settings: &Settings, extra: &str) -> io::Result<Key> {
    let mut params = stamp(file, level, settings);
    // Développé : le développement du sidecar aussi (modifié dans le logiciel : le rendu
    // est refait ; une note posée par `tri` dans le même fichier ne compte pas), dans
    // `stamp`.
    params.push_str(extra);
    Key::new(&file.path, level, &params)
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
        (MediaKind::Flat, Level::Thumbnail) => {
            Ok(flat::decode_working(&file.path, sizes.thumbnail)?)
        }
        // Une image plate n'a pas d'aperçu « boîtier » distinct : c'est son aperçu standard.
        (MediaKind::Flat, Level::Embedded | Level::Standard) => {
            Ok(flat::decode_working(&file.path, sizes.standard)?)
        }
        // Pas de JPEG « embarqué » distinct pour une image plate : c'est l'image elle-même.
        (MediaKind::Flat, Level::EmbeddedFull | Level::Full) => {
            Ok(flat::decode_working(&file.path, 0)?)
        }
        (_, Level::Exposure | Level::Sharpness | Level::Similarity | Level::Palette(_)) => {
            Err(PreviewError::NotAnImage)
        }
        // Développé : par le producteur, qui connaît le logiciel
        // (`scheduler::CachedPreviews`).
        (_, Level::Developed(engine)) => Err(PreviewError::Develop(
            crate::develop::DevelopError::Unavailable(engine),
        )),
    }
}

/// Aperçu embarqué d'un RAW déjà ouvert, décodé au moins à `min_side` si possible (non
/// redressé), avec son espace couleur (ICC ou EXIF du JPEG : un boîtier réglé en Adobe RGB).
/// `None` s'il n'y en a pas ou s'il est illisible.
fn embedded_source(raw: &mut RawFile, min_side: u32) -> Option<(Bitmap, Origin)> {
    match raw.thumbnail() {
        // jpeg::decode n'applique pas la balise EXIF : c'est voulu, l'orientation du RAW
        // fait foi (voir RawFile::orientation). Décodage directement réduit (IDCT).
        Ok(Thumbnail::Jpeg(bytes)) => Some((
            jpeg::decode(&bytes, min_side).ok()?,
            Origin::of_jpeg(&bytes),
        )),
        Ok(Thumbnail::Bitmap(bmp)) => Some((bmp, Origin::Srgb)),
        Err(_) => None,
    }
}

/// JPEG embarqué redressé, réduit à `side` (`0` : taille d'origine, pour le 1:1), dans
/// l'espace de travail (`working`) ou dans son propre espace. Refusé s'il est trop petit
/// pour servir (côté long inférieur à `min`).
fn raw_embedded_in(file: &MediaFile, side: u32, min: u32, working: bool) -> Result<Bitmap> {
    let mut raw = RawFile::open(&file.path)?;
    let rotation = raw.orientation();
    let (bmp, origin) = embedded_source(&mut raw, side)
        .filter(|(b, _)| b.width.max(b.height) >= min)
        .ok_or(PreviewError::NoEmbeddedPreview)?;
    // Réduit avant de convertir : moins de pixels à convertir.
    let mut bmp = bmp.fit(side);
    if working {
        crate::colour::bitmap_to_working(&mut bmp, &origin);
    }
    Ok(bmp.rotate(rotation))
}

fn raw_embedded(file: &MediaFile, side: u32, min: u32) -> Result<Bitmap> {
    raw_embedded_in(file, side, min, true)
}

/// JPEG embarqué (« JPEG boîtier ») d'un RAW, à sa taille et redressé : export de ce
/// qu'on regarde en plein écran (voir `tri::export`).
pub fn embedded_full(path: &std::path::Path) -> Result<Bitmap> {
    let mut raw = RawFile::open(path)?;
    let rotation = raw.orientation();
    // Dans son propre espace : l'export le lit à part (`colour::Origin`).
    let (bmp, _) = embedded_source(&mut raw, 0).ok_or(PreviewError::NoEmbeddedPreview)?;
    Ok(bmp.rotate(rotation))
}

/// Vignette d'un RAW : aperçu embarqué, réduit puis redressé selon l'orientation du RAW.
/// Sans aperçu exploitable, repli sur un rendu `half_size`.
fn raw_thumbnail(file: &MediaFile, settings: &Settings, cancel: &AtomicBool) -> Result<Bitmap> {
    let sizes = &settings.sizes;
    let mut raw = RawFile::open(&file.path)?;
    let rotation = raw.orientation();
    match embedded_source(&mut raw, sizes.thumbnail) {
        Some((bmp, origin)) => {
            let mut bmp = bmp.fit(sizes.thumbnail);
            crate::colour::bitmap_to_working(&mut bmp, &origin);
            Ok(bmp.rotate(rotation))
        }
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
    if level == Level::Exposure {
        let exposure = compute_exposure(file, settings, cancel)?;
        cache.put_bytes(&key, &exposure.to_bytes())?;
        return Ok(Outcome::Computed);
    }
    if level == Level::Sharpness {
        let score = compute_sharpness(file, settings, cancel)?;
        cache.put_bytes(&key, &score.to_bytes())?;
        return Ok(Outcome::Computed);
    }
    if level == Level::Similarity {
        let signature = compute_signature(file, settings, cancel)?;
        cache.put_bytes(&key, &signature.to_bytes())?;
        return Ok(Outcome::Computed);
    }
    if let Level::Palette(source) = level {
        let palette = compute_palette(cache, file, source, settings, cancel)?;
        cache.put_bytes(&key, &palette.to_bytes())?;
        return Ok(Outcome::Computed);
    }
    let bitmap = compute(file, level, settings, cancel)?;
    cache.put(&key, &bitmap)?;
    Ok(Outcome::Computed)
}

/// Analyse l'exposition de `file`, sans cache : données du capteur pour un RAW (~0,4 s,
/// non interruptible une fois la décompression lancée), image décodée pour une image
/// plate.
pub fn compute_exposure(
    file: &MediaFile,
    settings: &Settings,
    cancel: &AtomicBool,
) -> Result<Exposure> {
    if cancel.load(Ordering::Relaxed) {
        return Err(PreviewError::Cancelled);
    }
    Ok(match file.kind {
        MediaKind::Raw => Exposure::from_raw(&file.path, &settings.exposure)?,
        MediaKind::Flat => Exposure::from_deep(&flat::decode_deep(&file.path)?, &settings.exposure),
    })
}

/// Score de netteté de `file`, sans cache : sur le JPEG boîtier d'un RAW (réduit à
/// `score_side` au décodage), sur l'image elle-même pour une image plate.
pub fn compute_sharpness(
    file: &MediaFile,
    settings: &Settings,
    cancel: &AtomicBool,
) -> Result<crate::sharpness::Score> {
    if cancel.load(Ordering::Relaxed) {
        return Err(PreviewError::Cancelled);
    }
    let s = &settings.sharpness;
    let side = s.score_side.max(16);
    let image = match file.kind {
        // Un JPEG embarqué trop petit (vignette de 160 px) ne dit rien de la netteté.
        MediaKind::Raw => raw_embedded_in(file, side, side / 2, false)?,
        MediaKind::Flat => flat::decode_fit(&file.path, side)?,
    };
    Ok(crate::sharpness::score(&image, s))
}

/// Empreinte de regroupement de `file`, sans cache : sur une petite version (JPEG boîtier
/// d'un RAW, image plate réduite), avec l'heure de prise de vue.
pub fn compute_signature(
    file: &MediaFile,
    settings: &Settings,
    cancel: &AtomicBool,
) -> Result<crate::similarity::Signature> {
    if cancel.load(Ordering::Relaxed) {
        return Err(PreviewError::Cancelled);
    }
    let side = settings.similarity.side.max(16);
    let (image, date) = match file.kind {
        MediaKind::Raw => {
            let mut raw = RawFile::open(&file.path)?;
            let date = raw.shooting_info().date;
            let rotation = raw.orientation();
            // Analyse : sur les données du fichier, pas converties (clés inchangées).
            let image = match embedded_source(&mut raw, side).map(|(b, _)| b) {
                Some(b) => b.fit(side).rotate(rotation),
                // Sans JPEG embarqué : un rendu réduit.
                None => raw_thumbnail(file, settings, cancel)?.fit(side),
            };
            (image, date)
        }
        MediaKind::Flat => (
            flat::decode_fit(&file.path, side)?,
            crate::metadata::read(file).ok().and_then(|m| m.date),
        ),
    };
    let time = date.as_deref().and_then(crate::similarity::parse_time);
    Ok(crate::similarity::Signature::of(&image, time))
}

/// Empreinte de regroupement de `file` lue dans le cache, `None` si elle n'y est pas
/// (encore) ou si l'entrée est illisible (supprimée, pour être recalculée).
pub fn signature(
    cache: &Cache,
    file: &MediaFile,
    settings: &Settings,
) -> Option<crate::similarity::Signature> {
    let key = key(file, Level::Similarity, settings).ok()?;
    let bytes = cache.get_bytes(&key)?;
    match crate::similarity::Signature::from_bytes(&bytes) {
        Some(s) => Some(s),
        None => {
            let _ = cache.remove(&key);
            None
        }
    }
}

/// Image d'où la palette de la source `source` est tirée : la vignette (JPEG boîtier
/// d'un RAW, image plate réduite), le rendu RAW, ou le rendu du logiciel.
pub fn palette_input(source: crate::palette::Source) -> Level {
    match source {
        crate::palette::Source::Camera => Level::Thumbnail,
        crate::palette::Source::Raw => Level::Standard,
        crate::palette::Source::Developed(engine) => Level::Developed(engine),
    }
}

/// Heure de prise de vue de `file` (secondes, comme [`crate::similarity::Signature::time`]),
/// pour le tri par date de prise de vue sans attendre les empreintes (26/11/2026 : en mode
/// léger, elles attendent « Tout calculer », et le tri restait sur les noms). Lue dans le
/// cache, sinon dans le fichier — ouverture LibRaw sans rien décompresser pour un RAW,
/// EXIF d'une image plate — puis rangée, même absente (pas de date : pas relue). `None` :
/// pas de date, ou fichier illisible.
pub fn capture_time(cache: &Cache, file: &MediaFile, settings: &Settings) -> Option<i64> {
    let key = captured_key(file, settings).ok()?;
    if let Some(bytes) = cache.get_bytes(&key) {
        match <[u8; 8]>::try_from(bytes.as_slice()) {
            Ok(b) => return Some(i64::from_le_bytes(b)),
            Err(_) if bytes.is_empty() => return None,
            Err(_) => {
                let _ = cache.remove(&key);
            }
        }
    }
    let date = match file.kind {
        // Sans date pour LibRaw (des DNG) : l'EXIF du fichier, puis celle du JPEG embarqué.
        MediaKind::Raw => RawFile::open(&file.path)
            .ok()
            .and_then(|raw| raw.shooting_info().date)
            .or_else(|| crate::metadata::exif_date(&file.path))
            .or_else(|| crate::metadata::read(file).ok().and_then(|m| m.date)),
        MediaKind::Flat => crate::metadata::read(file).ok().and_then(|m| m.date),
    };
    let time = date.as_deref().and_then(crate::similarity::parse_time);
    let bytes = time.map(i64::to_le_bytes);
    let _ = cache.put_bytes(&key, bytes.as_ref().map_or(&[][..], |b| &b[..]));
    time
}

/// Palette de `file` tirée de `source`, sans la ranger : l'image d'où elle vient est
/// prise dans le cache, calculée d'abord si elle manque (sauf un rendu de logiciel, que
/// le producteur du scheduler fait avant, voir `scheduler::CachedPreviews`).
pub fn compute_palette(
    cache: &Cache,
    file: &MediaFile,
    source: crate::palette::Source,
    settings: &Settings,
    cancel: &AtomicBool,
) -> Result<crate::palette::Palette> {
    let input = palette_input(source);
    ensure(cache, file, input, settings, cancel)?;
    let image = cache
        .get(&key(file, input, settings)?)
        .ok_or(PreviewError::NotAnImage)?;
    let mut palette = crate::palette::extract(&image, &settings.palette);
    palette.perceived = Some(Box::new(crate::palette::extract_perceived(
        &image,
        &settings.palette,
    )));
    Ok(palette)
}

/// Palette de `file` tirée de `source`, lue dans le cache ; `None` si elle n'y est pas
/// (encore) ou si l'entrée est illisible (supprimée, pour être recalculée).
pub fn palette(
    cache: &Cache,
    file: &MediaFile,
    source: crate::palette::Source,
    settings: &Settings,
) -> Option<crate::palette::Palette> {
    let key = key(file, Level::Palette(source), settings).ok()?;
    let bytes = cache.get_bytes(&key)?;
    match crate::palette::Palette::from_bytes(&bytes) {
        Some(p) => Some(p),
        None => {
            let _ = cache.remove(&key);
            None
        }
    }
}

/// Score de netteté de `file` lu dans le cache, `None` s'il n'y est pas (encore) ou si
/// l'entrée est illisible (elle est alors supprimée, pour être recalculée).
pub fn sharpness(
    cache: &Cache,
    file: &MediaFile,
    settings: &Settings,
) -> Option<crate::sharpness::Score> {
    let key = key(file, Level::Sharpness, settings).ok()?;
    let bytes = cache.get_bytes(&key)?;
    match crate::sharpness::Score::from_bytes(&bytes) {
        Some(s) => Some(s),
        None => {
            let _ = cache.remove(&key);
            None
        }
    }
}

/// Analyse d'exposition de `file` lue dans le cache, `None` si elle n'y est pas (encore)
/// ou si l'entrée est illisible (elle est alors supprimée, pour être recalculée).
pub fn exposure(cache: &Cache, file: &MediaFile, settings: &Settings) -> Option<Exposure> {
    let key = key(file, Level::Exposure, settings).ok()?;
    let bytes = cache.get_bytes(&key)?;
    match Exposure::from_bytes(&bytes) {
        Ok(e) => Some(e),
        Err(_) => {
            let _ = cache.remove(&key);
            None
        }
    }
}

/// Exposition du **JPEG boîtier** d'un RAW (idée de l'utilisateur, 27/10/2026 : voir ce
/// que le boîtier a fait des hautes lumières et des ombres, à côté des données du
/// capteur), d'après le JPEG embarqué **dans son propre espace** (l'aperçu en cache est
/// converti vers l'espace de travail : les saturés du boîtier n'y seraient plus à 255) :
/// `None` pour une image plate, ou tant que l'aperçu boîtier ([`Level::Embedded`]) n'est pas
/// calculé. Pas de cache propre : quelques dizaines de ms, à garder en mémoire par
/// l'appelant.
pub fn camera_exposure(cache: &Cache, file: &MediaFile, settings: &Settings) -> Option<Exposure> {
    if file.kind != MediaKind::Raw || !cache.contains(&key(file, Level::Embedded, settings).ok()?) {
        return None;
    }
    let min = (settings.sizes.standard as f32 * settings.embedded_min_ratio) as u32;
    let bitmap = raw_embedded_in(file, settings.sizes.standard, min, false).ok()?;
    Some(Exposure::from_bitmap(&bitmap, &settings.exposure))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_time_is_read_once_then_kept() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path().join("cache")).unwrap();
        let settings = Settings::default();
        let photo = dir.path().join("IMG_0001.jpg");
        std::fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/exif.jpg"),
            &photo,
        )
        .unwrap();
        let file = MediaFile {
            path: photo.clone(),
            kind: MediaKind::Flat,
        };
        let expected = crate::similarity::parse_time("2026:10:16 14:32:05");
        assert!(expected.is_some());
        assert_eq!(capture_time(&cache, &file, &settings), expected);
        assert!(cache.contains(&captured_key(&file, &settings).unwrap()));
        // Rangée : lue dans le cache (le fichier n'est plus lisible comme image).
        let key = captured_key(&file, &settings).unwrap();
        cache.put_bytes(&key, &42i64.to_le_bytes()).unwrap();
        assert_eq!(capture_time(&cache, &file, &settings), Some(42));
        // Sans date : rangée vide, pas relue.
        let plain = dir.path().join("plain.png");
        image::RgbImage::new(4, 4).save(&plain).unwrap();
        let plain = MediaFile {
            path: plain,
            kind: MediaKind::Flat,
        };
        assert_eq!(capture_time(&cache, &plain, &settings), None);
        let key = captured_key(&plain, &settings).unwrap();
        assert_eq!(cache.get_bytes(&key).as_deref(), Some(&[][..]));
    }

    #[test]
    fn computed_entries_follow_a_copied_or_moved_image() {
        // #508 : une copie (nom, taille et date gardés) retrouve ses aperçus et analyses
        // au lieu de les recalculer ; un déplacement les emporte.
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path().join("cache")).unwrap();
        let settings = Settings::default();
        let (a, b, c) = (
            dir.path().join("a"),
            dir.path().join("b"),
            dir.path().join("c"),
        );
        for d in [&a, &b, &c] {
            std::fs::create_dir(d).unwrap();
        }
        let photo = a.join("IMG_0001.jpg");
        std::fs::write(&photo, b"jpeg").unwrap();
        let file = |path: &std::path::Path| MediaFile {
            path: path.to_path_buf(),
            kind: MediaKind::Flat,
        };
        let levels = [Level::Thumbnail, Level::Exposure, Level::Standard];
        for level in levels {
            cache
                .put_bytes(&key(&file(&photo), level, &settings).unwrap(), b"x")
                .unwrap();
        }
        let side = settings.sizes.thumbnail;
        let small = grid_key(&file(&photo), Level::Standard, &settings, side).unwrap();
        cache.put_bytes(&small, b"s").unwrap();
        // La copie de `tri` garde la date.
        let copy = |to: &std::path::Path| {
            std::fs::copy(&photo, to).unwrap();
            let modified = std::fs::metadata(&photo).unwrap().modified().unwrap();
            let f = std::fs::File::options().write(true).open(to).unwrap();
            f.set_modified(modified).unwrap();
        };
        let in_b = b.join("IMG_0001.jpg");
        copy(&in_b);
        let n = carry(&cache, &settings, &[(photo.clone(), in_b.clone())], true);
        assert_eq!(n, 4);
        for level in levels {
            assert!(cache.contains(&key(&file(&in_b), level, &settings).unwrap()));
            assert!(cache.contains(&key(&file(&photo), level, &settings).unwrap()));
        }
        assert!(cache.contains(&grid_key(&file(&in_b), Level::Standard, &settings, side).unwrap()));
        // Déplacée : plus rien à l'ancienne place.
        let in_c = c.join("IMG_0001.jpg");
        std::fs::rename(&in_b, &in_c).unwrap();
        crate::cache::forget_stamps_under(&b);
        let n = carry(&cache, &settings, &[(in_b.clone(), in_c.clone())], false);
        assert_eq!(n, 4);
        assert!(cache.contains(&key(&file(&in_c), Level::Exposure, &settings).unwrap()));
        assert_eq!(
            carry(&cache, &settings, &[(in_b.clone(), in_c.clone())], false),
            0,
            "déjà là : rien d'écrasé"
        );
    }

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
        // La palette des données brutes suit le rendu RAW d'où elle est tirée (#492).
        let raw_palette = Level::Palette(crate::palette::Source::Raw);
        assert_eq!(
            changed(&|t| t.render.auto_bright = true),
            [Level::Standard, Level::Full, raw_palette]
        );
        assert_eq!(
            changed(&|t| t.half_size_margin = 1.5),
            [Level::Standard, raw_palette]
        );
        assert_eq!(
            changed(&|t| t.palette.colors = 8).len(),
            6,
            "toutes les palettes, rien d'autre"
        );
        assert_eq!(changed(&|t| t.embedded_min_ratio = 0.8), [Level::Embedded]);
        assert_eq!(
            changed(&|t| t.min_embedded_full = 2000),
            [Level::EmbeddedFull]
        );
    }

    #[test]
    fn the_camera_jpeg_histogram_waits_for_the_embedded_preview_and_reads_the_raw() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path().join("cache")).unwrap();
        let settings = Settings::default();
        let raw = MediaFile {
            path: dir.path().join("a.raf"),
            kind: MediaKind::Raw,
        };
        std::fs::write(&raw.path, b"raw").unwrap();
        assert!(
            camera_exposure(&cache, &raw, &settings).is_none(),
            "pas encore d'aperçu boîtier"
        );
        // L'aperçu en cache n'est qu'un signal : l'histogramme lit le JPEG embarqué dans
        // son propre espace (l'aperçu est converti vers l'espace de travail), ici illisible.
        let white = Bitmap {
            width: 16,
            height: 16,
            data: vec![255; 16 * 16 * 3],
        };
        let key = key(&raw, Level::Embedded, &settings).unwrap();
        cache.put(&key, &white).unwrap();
        assert!(
            camera_exposure(&cache, &raw, &settings).is_none(),
            "RAW illisible"
        );
        let flat = MediaFile {
            path: dir.path().join("b.jpg"),
            kind: MediaKind::Flat,
        };
        std::fs::write(&flat.path, b"jpg").unwrap();
        assert!(
            camera_exposure(&cache, &flat, &settings).is_none(),
            "image plate"
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
