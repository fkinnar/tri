//! Lecture des fichiers RAW via LibRaw : aperçu embarqué et développement complet.
//!
//! Voir `doc/raw.md` pour le détail.

mod ffi;

use std::ffi::CStr;
use std::fmt;
use std::os::raw::c_int;
use std::path::{Path, PathBuf};

/// Image RGB 8 bits, entrelacée (R, G, B, R, G, B...), ligne par ligne.
#[derive(Clone)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Aperçu embarqué : presque toujours un JPEG, parfois un bitmap brut.
pub enum Thumbnail {
    Jpeg(Vec<u8>),
    Bitmap(Bitmap),
}

/// Paramètres du développement complet (`RawFile::render`).
#[derive(Clone, Debug)]
pub struct RenderOptions {
    /// Balance des blancs du boîtier (sinon : multiplicateurs « lumière du jour »).
    pub camera_wb: bool,
    /// Demi-résolution : dématriçage sauté, beaucoup plus rapide.
    pub half_size: bool,
    /// Luminosité (1.0 = défaut LibRaw).
    pub bright: f32,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            camera_wb: true,
            half_size: false,
            bright: 1.0,
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
    LibRaw { code: i32, message: String },
    /// Image nulle sans code d'erreur.
    EmptyImage,
    /// Image produite dans un format que `Bitmap` ne gère pas.
    UnsupportedImage { kind: i32, colors: u16, bits: u16 },
}

impl fmt::Display for RawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Init => write!(f, "libraw_init a échoué"),
            Self::InvalidPath(p) => {
                write!(f, "chemin non transmissible à LibRaw : {}", p.display())
            }
            Self::LibRaw { code, message } => write!(f, "LibRaw erreur {code} : {message}"),
            Self::EmptyImage => write!(f, "LibRaw a renvoyé une image nulle sans code d'erreur"),
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
    if code == 0 {
        return Ok(());
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

    /// Développement complet des données RAW en RGB 8 bits (orientation appliquée).
    pub fn render(&mut self, options: &RenderOptions) -> Result<Bitmap> {
        unsafe {
            ffi::libraw_set_output_bps(self.ptr, 8);
            ffi::tri_libraw_set_use_camera_wb(self.ptr, options.camera_wb.into());
            ffi::tri_libraw_set_half_size(self.ptr, options.half_size.into());
            ffi::libraw_set_bright(self.ptr, options.bright);
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

impl Drop for RawFile {
    fn drop(&mut self) {
        unsafe { ffi::libraw_close(self.ptr) };
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
