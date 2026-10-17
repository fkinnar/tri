//! Lecture des fichiers RAW via LibRaw : aperçu embarqué et développement complet.
//!
//! Voir `doc/raw.md` pour le détail.

mod ffi;

use std::ffi::CStr;
use std::fmt;
use std::os::raw::c_int;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::bitmap::{Bitmap, Rotation};

/// Aperçu embarqué : presque toujours un JPEG, parfois un bitmap brut.
pub enum Thumbnail {
    /// Octets du fichier JPEG, tels qu'embarqués (orientation EXIF non appliquée).
    Jpeg(Vec<u8>),
    /// Aperçu non compressé, converti en RGB8.
    Bitmap(Bitmap),
}

/// Paramètres du développement complet (`RawFile::render`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderOptions {
    /// Balance des blancs du boîtier (sinon : multiplicateurs « lumière du jour »).
    pub camera_wb: bool,
    /// Demi-résolution : dématriçage sauté, beaucoup plus rapide.
    pub half_size: bool,
    /// Luminosité (1.0 = défaut LibRaw).
    pub bright: f32,
    /// Luminosité automatique de LibRaw : éclaircit jusqu'à écrêter 1 % des pixels. Désactivée
    /// par défaut : elle « corrige » une sous-exposition et masque donc l'erreur de prise de
    /// vue qu'on cherche justement à voir en triant.
    pub auto_bright: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            camera_wb: true,
            half_size: false,
            bright: 1.0,
            auto_bright: false,
        }
    }
}

/// Erreurs du module raw.
#[derive(Debug)]
pub enum RawError {
    /// `libraw_init` a renvoyé un pointeur nul.
    Init,
    /// Chemin impossible à transmettre en C (octet nul).
    InvalidPath(PathBuf),
    /// Code d'erreur renvoyé par LibRaw, avec son message.
    LibRaw {
        /// Code LibRaw (négatif).
        code: i32,
        /// Message de `libraw_strerror`.
        message: String,
    },
    /// Traitement annulé à la demande (voir [`RawFile::render_cancellable`]).
    Cancelled,
    /// Image nulle sans code d'erreur.
    EmptyImage,
    /// Pas de mosaïque de filtres couleur (DNG linéaire, capteur Foveon) : pas de données
    /// capteur à lire pixel par pixel (voir [`RawFile::sensor`]).
    NoMosaic,
    /// Image produite dans un format que `Bitmap` ne gère pas.
    UnsupportedImage {
        /// Type LibRaw (1 = JPEG, 2 = bitmap).
        kind: i32,
        /// Nombre de canaux.
        colors: u16,
        /// Bits par canal.
        bits: u16,
    },
}

impl fmt::Display for RawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Init => write!(f, "libraw_init a échoué"),
            Self::InvalidPath(p) => {
                write!(f, "chemin non transmissible à LibRaw : {}", p.display())
            }
            Self::LibRaw { code, message } => write!(f, "LibRaw erreur {code} : {message}"),
            Self::Cancelled => write!(f, "traitement LibRaw annulé"),
            Self::EmptyImage => write!(f, "LibRaw a renvoyé une image nulle sans code d'erreur"),
            Self::NoMosaic => write!(
                f,
                "RAW sans mosaïque de filtres couleur (DNG linéaire, Foveon)"
            ),
            Self::UnsupportedImage { kind, colors, bits } => {
                write!(
                    f,
                    "image non gérée (type {kind}, {colors} canaux, {bits} bits)"
                )
            }
        }
    }
}

impl std::error::Error for RawError {}

/// `Result` spécialisé pour le module raw.
pub type Result<T> = std::result::Result<T, RawError>;

fn check(code: c_int) -> Result<()> {
    match code {
        0 => return Ok(()),
        ffi::CANCELLED_BY_CALLBACK => return Err(RawError::Cancelled),
        _ => {}
    }
    let message = unsafe { CStr::from_ptr(ffi::libraw_strerror(code)) }
        .to_string_lossy()
        .into_owned();
    Err(RawError::LibRaw { code, message })
}

/// Version de la LibRaw chargée (ex. `0.22.2-Release`).
pub fn version() -> String {
    unsafe { CStr::from_ptr(ffi::libraw_version()) }
        .to_string_lossy()
        .into_owned()
}

/// Image allouée par LibRaw ; Drop garantit libraw_dcraw_clear_mem.
struct MemImage(*mut ffi::ProcessedImage);

impl MemImage {
    fn new(ptr: *mut ffi::ProcessedImage, err: c_int) -> Result<Self> {
        if ptr.is_null() {
            check(err)?;
            return Err(RawError::EmptyImage);
        }
        Ok(Self(ptr))
    }

    fn header(&self) -> &ffi::ProcessedImage {
        unsafe { &*self.0 }
    }

    fn bytes(&self) -> &[u8] {
        unsafe {
            let p = std::ptr::addr_of!((*self.0).data) as *const u8;
            std::slice::from_raw_parts(p, (*self.0).data_size as usize)
        }
    }

    fn to_bitmap(&self) -> Result<Bitmap> {
        bitmap_from_processed(self.header(), self.bytes())
    }
}

/// Convertit une image LibRaw (en-tête + données) en `Bitmap` RGB8.
/// Accepte les bitmaps 8 bits en RGB ou en niveaux de gris (étendus en RGB).
fn bitmap_from_processed(h: &ffi::ProcessedImage, bytes: &[u8]) -> Result<Bitmap> {
    let unsupported = RawError::UnsupportedImage {
        kind: h.kind,
        colors: h.colors,
        bits: h.bits,
    };
    if h.kind != ffi::IMAGE_BITMAP || h.bits != 8 {
        return Err(unsupported);
    }
    let (width, height) = (u32::from(h.width), u32::from(h.height));
    let pixels = width as usize * height as usize;
    let data = match h.colors {
        3 if bytes.len() >= pixels * 3 => bytes[..pixels * 3].to_vec(),
        1 if bytes.len() >= pixels => bytes[..pixels].iter().flat_map(|&g| [g, g, g]).collect(),
        _ => return Err(unsupported),
    };
    Ok(Bitmap {
        width,
        height,
        data,
    })
}

impl Drop for MemImage {
    fn drop(&mut self) {
        unsafe { ffi::libraw_dcraw_clear_mem(self.0) };
    }
}

/// Fichier RAW ouvert. Drop garantit libraw_close même en cas d'erreur.
///
/// `Send` mais pas `Sync` : déplaçable vers un thread de travail, pas partageable.
pub struct RawFile {
    ptr: *mut ffi::LibRawData,
}

// Une instance LibRaw n'est liée à aucun thread : on peut la déplacer (pas la partager).
unsafe impl Send for RawFile {}

impl RawFile {
    /// Ouvre un fichier RAW : seul le format est analysé, les données ne sont pas lues.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let ptr = unsafe { ffi::libraw_init(0) };
        if ptr.is_null() {
            return Err(RawError::Init);
        }
        let raw = Self { ptr };
        check(raw.open_path(path)?)?;
        Ok(raw)
    }

    #[cfg(not(windows))]
    fn open_path(&self, path: &Path) -> Result<c_int> {
        use std::os::unix::ffi::OsStrExt;
        // Octets bruts : fonctionne aussi pour les noms non UTF-8.
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
            .map_err(|_| RawError::InvalidPath(path.to_owned()))?;
        Ok(unsafe { ffi::libraw_open_file(self.ptr, c_path.as_ptr()) })
    }

    #[cfg(windows)]
    fn open_path(&self, path: &Path) -> Result<c_int> {
        use std::os::windows::ffi::OsStrExt;
        // UTF-16 : seule façon d'ouvrir les chemins non ASCII sous Windows.
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        if wide[..wide.len() - 1].contains(&0) {
            return Err(RawError::InvalidPath(path.to_owned()));
        }
        Ok(unsafe { ffi::libraw_open_wfile(self.ptr, wide.as_ptr()) })
    }

    /// Aperçu embarqué, sans développement.
    pub fn thumbnail(&mut self) -> Result<Thumbnail> {
        check(unsafe { ffi::libraw_unpack_thumb(self.ptr) })?;
        let mut err: c_int = 0;
        let img = MemImage::new(
            unsafe { ffi::libraw_dcraw_make_mem_thumb(self.ptr, &mut err) },
            err,
        )?;
        match img.header().kind {
            ffi::IMAGE_JPEG => Ok(Thumbnail::Jpeg(img.bytes().to_vec())),
            _ => img.to_bitmap().map(Thumbnail::Bitmap),
        }
    }

    /// Rotation à appliquer à l'aperçu embarqué (le rendu, lui, est déjà pivoté).
    /// Lue dans les métadonnées du RAW (`sizes.flip`), qui font foi : l'aperçu embarqué est
    /// stocké dans l'orientation du capteur.
    pub fn orientation(&self) -> Rotation {
        rotation_from_flip(unsafe { ffi::tri_libraw_get_flip(self.ptr) })
    }

    /// Dimensions (largeur, hauteur) du rendu pleine résolution, avant rotation.
    /// Disponibles dès l'ouverture, sans développer.
    pub fn size(&self) -> (u32, u32) {
        let (mut w, mut h): (c_int, c_int) = (0, 0);
        unsafe { ffi::tri_libraw_get_size(self.ptr, &mut w, &mut h) };
        (w.max(0) as u32, h.max(0) as u32)
    }

    /// Métadonnées de prise de vue lues par LibRaw (dès l'ouverture, sans décompresser) :
    /// voir [`crate::metadata`], qui les complète par l'EXIF du JPEG embarqué.
    pub fn shooting_info(&self) -> ShootingInfo {
        let mut m = std::mem::MaybeUninit::<ffi::Metadata>::uninit();
        let m = unsafe {
            ffi::tri_libraw_metadata(self.ptr, m.as_mut_ptr());
            m.assume_init()
        };
        let text = |chars: &[std::os::raw::c_char]| {
            let bytes: Vec<u8> = chars
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| c as u8)
                .collect();
            let s = String::from_utf8_lossy(&bytes).trim().to_string();
            (!s.is_empty()).then_some(s)
        };
        let positive = |v: f32| (v > 0.0 && v.is_finite()).then_some(v);
        ShootingInfo {
            make: text(&m.make),
            model: text(&m.model),
            lens: text(&m.lens),
            iso: positive(m.iso),
            shutter: positive(m.shutter),
            aperture: positive(m.aperture),
            focal: positive(m.focal),
            focal_35mm: positive(m.focal_35mm),
            date: text(&m.date),
            size: (m.width > 0 && m.height > 0).then_some((m.width as u32, m.height as u32)),
        }
    }

    /// Données brutes du capteur : décompresse le RAW (`libraw_unpack`, sans
    /// dématriçage) et donne accès aux valeurs de la mosaïque, à leur profondeur réelle
    /// (12, 14 ou 16 bits selon le boîtier), avec les niveaux de noir et de blanc. Base de
    /// l'histogramme RAW. Erreur [`RawError::NoMosaic`] pour un RAW sans mosaïque.
    pub fn sensor(&mut self) -> Result<Sensor<'_>> {
        check(unsafe { ffi::libraw_unpack(self.ptr) })?;
        let mut info = std::mem::MaybeUninit::<ffi::RawInfo>::uninit();
        let info = unsafe {
            ffi::tri_libraw_raw_info(self.ptr, info.as_mut_ptr());
            info.assume_init()
        };
        if info.raw_image.is_null() || info.filters == 0 {
            return Err(RawError::NoMosaic);
        }
        let pitch = info.raw_pitch as usize / 2;
        let len = pitch * info.raw_height as usize;
        // Sûr : buffer de LibRaw, valide tant que `self` n'est ni modifié ni fermé, ce que
        // garantit l'emprunt de `self` par `Sensor`.
        let pixels = unsafe { std::slice::from_raw_parts(info.raw_image, len) };
        // Sûr : tableau de LibRaw de `cblack_len` entrées, même durée de vie que `pixels`.
        let cblack = unsafe { std::slice::from_raw_parts(info.cblack, info.cblack_len as usize) };
        // Motif de noir (LibRaw : cblack[4] × cblack[5] valeurs à partir de cblack[6] ;
        // Fuji : 6 × 6). Replié dans la table 24 × 24 s'il en divise la période (1, 2, 3,
        // 4, 6, 8, 12, 24) ; sinon sa moyenne, faute de mieux (aucun cas connu).
        let (prows, pcols) = (cblack[4] as usize, cblack[5] as usize);
        let plen = prows * pcols;
        let pattern_ok = plen > 0 && 6 + plen <= cblack.len();
        let folds = pattern_ok && PATTERN.is_multiple_of(prows) && PATTERN.is_multiple_of(pcols);
        let mean = if pattern_ok && !folds {
            (cblack[6..6 + plen].iter().map(|&v| v as u64).sum::<u64>() / plen as u64) as u32
        } else {
            0
        };
        let mut channels = [[0u8; PATTERN]; PATTERN];
        let mut blacks = [[0u16; PATTERN]; PATTERN];
        for row in 0..PATTERN {
            for col in 0..PATTERN {
                let c = unsafe { ffi::libraw_COLOR(self.ptr, row as c_int, col as c_int) };
                let c = c.clamp(0, 3) as usize;
                let local = if folds {
                    cblack[6 + (row % prows) * pcols + col % pcols]
                } else {
                    mean
                };
                let black = info.black + cblack[c] + local;
                blacks[row][col] = black.min(u16::MAX as u32) as u16;
                // 0 rouge, 1 vert, 2 bleu, 3 second vert (Bayer) : ramené au vert.
                channels[row][col] = match c {
                    0 => 0,
                    2 => 2,
                    _ => 1,
                };
            }
        }
        Ok(Sensor {
            pixels,
            pitch,
            left: info.left_margin as usize,
            top: info.top_margin as usize,
            width: (info.width as usize).min(pitch.saturating_sub(info.left_margin as usize)),
            height: (info.height as usize)
                .min((info.raw_height as usize).saturating_sub(info.top_margin as usize)),
            white: info.maximum.min(u16::MAX as u32) as u16,
            channels,
            blacks,
        })
    }

    /// Développement complet des données RAW en RGB 8 bits (orientation appliquée).
    pub fn render(&mut self, options: &RenderOptions) -> Result<Bitmap> {
        self.render_cancellable(options, &AtomicBool::new(false))
    }

    /// Comme [`RawFile::render`], mais s'arrête avec [`RawError::Cancelled`] dès que
    /// `cancel` passe à `true`. LibRaw ne consulte ce drapeau qu'entre deux étapes du
    /// traitement. Mesuré sur RAF X-T4 : le dématriçage X-Trans (~3 s sur ~3,5 s) est un
    /// seul bloc sans point d'arrêt, donc une annulation pendant ce bloc ne prend effet qu'à
    /// sa fin. En `half_size`, il n'y a pas de dématriçage.
    pub fn render_cancellable(
        &mut self,
        options: &RenderOptions,
        cancel: &AtomicBool,
    ) -> Result<Bitmap> {
        if cancel.load(Ordering::Relaxed) {
            return Err(RawError::Cancelled);
        }
        // Le handler pointe sur `cancel` : il est retiré à la sortie, quoi qu'il arrive.
        struct ResetHandler(*mut ffi::LibRawData);
        impl Drop for ResetHandler {
            fn drop(&mut self) {
                unsafe { ffi::libraw_set_progress_handler(self.0, None, std::ptr::null_mut()) };
            }
        }
        unsafe {
            ffi::libraw_set_progress_handler(
                self.ptr,
                Some(cancel_requested),
                cancel as *const AtomicBool as *mut _,
            )
        };
        let _reset = ResetHandler(self.ptr);
        unsafe {
            ffi::libraw_set_output_bps(self.ptr, 8);
            ffi::tri_libraw_set_use_camera_wb(self.ptr, options.camera_wb.into());
            ffi::tri_libraw_set_half_size(self.ptr, options.half_size.into());
            ffi::libraw_set_bright(self.ptr, options.bright);
            ffi::libraw_set_no_auto_bright(self.ptr, c_int::from(!options.auto_bright));
        }
        check(unsafe { ffi::libraw_unpack(self.ptr) })?;
        check(unsafe { ffi::libraw_dcraw_process(self.ptr) })?;
        let mut err: c_int = 0;
        let img = MemImage::new(
            unsafe { ffi::libraw_dcraw_make_mem_image(self.ptr, &mut err) },
            err,
        )?;
        img.to_bitmap()
    }
}

/// Callback de progression LibRaw : renvoie 1 (annuler) si le drapeau est levé.
/// Ne peut pas paniquer (une simple lecture atomique) : rien ne se déroule à travers le C.
unsafe extern "C" fn cancel_requested(
    data: *mut std::ffi::c_void,
    _stage: c_int,
    _iteration: c_int,
    _expected: c_int,
) -> c_int {
    let cancel = unsafe { &*(data as *const AtomicBool) };
    c_int::from(cancel.load(Ordering::Relaxed))
}

/// Traduit `sizes.flip` (convention dcraw) en rotation horaire.
fn rotation_from_flip(flip: c_int) -> Rotation {
    match flip {
        3 => Rotation::Cw180,
        5 => Rotation::Cw270,
        6 => Rotation::Cw90,
        _ => Rotation::None,
    }
}

impl Drop for RawFile {
    fn drop(&mut self) {
        unsafe { ffi::libraw_close(self.ptr) };
    }
}

/// Métadonnées de prise de vue lues par LibRaw ([`RawFile::shooting_info`]) ; `None` :
/// inconnue.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShootingInfo {
    /// Fabricant.
    pub make: Option<String>,
    /// Modèle.
    pub model: Option<String>,
    /// Objectif.
    pub lens: Option<String>,
    /// Sensibilité ISO.
    pub iso: Option<f32>,
    /// Temps de pose (s).
    pub shutter: Option<f32>,
    /// Ouverture (f/).
    pub aperture: Option<f32>,
    /// Focale (mm).
    pub focal: Option<f32>,
    /// Focale équivalente 24 × 36 (mm).
    pub focal_35mm: Option<f32>,
    /// Date de prise de vue, « AAAA-MM-JJ HH:MM:SS », heure locale.
    pub date: Option<String>,
    /// Dimensions de la zone visible (pixels, avant rotation).
    pub size: Option<(u32, u32)>,
}

/// Période de la table du motif de filtres : 24 couvre Bayer (8 × 2) et X-Trans (6 × 6).
const PATTERN: usize = 24;

/// Données brutes du capteur (mosaïque) d'un RAW décompressé, empruntées à LibRaw : voir
/// [`RawFile::sensor`]. Coordonnées dans la zone visible (marges du capteur exclues).
pub struct Sensor<'a> {
    pixels: &'a [u16],
    pitch: usize,
    left: usize,
    top: usize,
    /// Largeur de la zone visible.
    pub width: usize,
    /// Hauteur de la zone visible.
    pub height: usize,
    /// Niveau de saturation du capteur (blanc), en valeur brute.
    pub white: u16,
    channels: [[u8; PATTERN]; PATTERN],
    blacks: [[u16; PATTERN]; PATTERN],
}

impl Sensor<'_> {
    /// Ligne `row` de la zone visible.
    pub fn row(&self, row: usize) -> &[u16] {
        let start = (self.top + row) * self.pitch + self.left;
        &self.pixels[start..start + self.width]
    }

    /// Canal (0 rouge, 1 vert, 2 bleu) du filtre au pixel (`row`, `col`).
    pub fn channel(&self, row: usize, col: usize) -> usize {
        self.channels[row % PATTERN][col % PATTERN] as usize
    }

    /// Niveau de noir au pixel (`row`, `col`), en valeur brute : noir général, par canal
    /// et motif de noir du boîtier (Fuji : 6 × 6, ~1022 en 14 bits).
    pub fn black(&self, row: usize, col: usize) -> u16 {
        self.blacks[row % PATTERN][col % PATTERN]
    }

    /// Canaux et noirs d'une ligne, à répéter tous les 24 pixels (motif) : pour parcourir
    /// une ligne sans calcul de motif par pixel.
    pub fn pattern(&self, row: usize) -> (&[u8; PATTERN], &[u16; PATTERN]) {
        (&self.channels[row % PATTERN], &self.blacks[row % PATTERN])
    }

    /// Plus petit noir du capteur (tous canaux et positions confondus).
    pub fn min_black(&self) -> u16 {
        self.blacks.iter().flatten().copied().min().unwrap_or(0)
    }

    /// Profondeur utile en bits, déduite du blanc (14 pour un blanc de 16383).
    pub fn bits(&self) -> u32 {
        u16::BITS - self.white.leading_zeros()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(kind: c_int, width: u16, height: u16, colors: u16, bits: u16) -> ffi::ProcessedImage {
        ffi::ProcessedImage {
            kind,
            height,
            width,
            colors,
            bits,
            data_size: 0, // ignoré : on passe les octets à part
            data: [0],
        }
    }

    #[test]
    fn rgb8_bitmap_is_copied() {
        let bytes: Vec<u8> = (0..12).collect();
        let bmp = bitmap_from_processed(&header(ffi::IMAGE_BITMAP, 2, 2, 3, 8), &bytes).unwrap();
        assert_eq!((bmp.width, bmp.height), (2, 2));
        assert_eq!(bmp.data, bytes);
    }

    #[test]
    fn trailing_bytes_are_ignored() {
        let bytes = [1, 2, 3, 99, 99];
        let bmp = bitmap_from_processed(&header(ffi::IMAGE_BITMAP, 1, 1, 3, 8), &bytes).unwrap();
        assert_eq!(bmp.data, [1, 2, 3]);
    }

    #[test]
    fn grayscale_is_expanded_to_rgb() {
        let bmp = bitmap_from_processed(&header(ffi::IMAGE_BITMAP, 2, 1, 1, 8), &[10, 20]).unwrap();
        assert_eq!(bmp.data, [10, 10, 10, 20, 20, 20]);
    }

    #[test]
    fn truncated_data_is_rejected() {
        let res = bitmap_from_processed(&header(ffi::IMAGE_BITMAP, 2, 2, 3, 8), &[0; 11]);
        assert!(matches!(res, Err(RawError::UnsupportedImage { .. })));
    }

    #[test]
    fn jpeg_16bit_and_odd_channels_are_rejected() {
        for h in [
            header(ffi::IMAGE_JPEG, 1, 1, 3, 8),
            header(ffi::IMAGE_BITMAP, 1, 1, 3, 16),
            header(ffi::IMAGE_BITMAP, 1, 1, 4, 8),
        ] {
            let res = bitmap_from_processed(&h, &[0; 8]);
            assert!(matches!(res, Err(RawError::UnsupportedImage { .. })));
        }
    }

    #[test]
    fn check_maps_codes_to_errors() {
        assert!(check(0).is_ok());
        let Err(RawError::LibRaw { code, message }) = check(-2) else {
            panic!("erreur LibRaw attendue");
        };
        assert_eq!(code, -2);
        assert!(!message.is_empty());
    }

    #[test]
    fn cancellation_code_maps_to_cancelled() {
        assert!(matches!(
            check(ffi::CANCELLED_BY_CALLBACK),
            Err(RawError::Cancelled)
        ));
    }

    #[test]
    fn flip_values_map_to_clockwise_rotations() {
        assert_eq!(rotation_from_flip(0), Rotation::None);
        assert_eq!(rotation_from_flip(3), Rotation::Cw180);
        assert_eq!(rotation_from_flip(5), Rotation::Cw270);
        assert_eq!(rotation_from_flip(6), Rotation::Cw90);
        assert_eq!(rotation_from_flip(1), Rotation::None);
    }

    #[test]
    fn cancel_callback_reads_the_flag() {
        let flag = AtomicBool::new(false);
        let data = &flag as *const AtomicBool as *mut std::ffi::c_void;
        assert_eq!(unsafe { cancel_requested(data, 0, 0, 0) }, 0);
        flag.store(true, Ordering::Relaxed);
        assert_eq!(unsafe { cancel_requested(data, 0, 0, 0) }, 1);
    }

    #[test]
    fn default_options_use_camera_wb_at_full_size() {
        let o = RenderOptions::default();
        assert!(o.camera_wb);
        assert!(!o.half_size);
        assert_eq!(o.bright, 1.0);
    }

    #[test]
    fn version_is_reported() {
        assert!(
            version().starts_with("0."),
            "version inattendue : {}",
            version()
        );
    }
}
