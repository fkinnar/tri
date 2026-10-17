//! Déclarations brutes de l'API C de LibRaw (+ shim `csrc/libraw_shim.c`).

use std::os::raw::{c_char, c_float, c_int, c_uchar, c_uint, c_ushort, c_void};

/// Handle opaque : on ne connaît pas (et n'a pas besoin de connaître) son contenu.
#[repr(C)]
pub struct LibRawData {
    _private: [u8; 0],
}

/// Code renvoyé quand le callback de progression a demandé l'arrêt.
pub const CANCELLED_BY_CALLBACK: c_int = -100010;

/// `progress_callback` : une valeur de retour non nulle annule le traitement en cours.
pub type ProgressCallback = unsafe extern "C" fn(
    data: *mut c_void,
    stage: c_int,
    iteration: c_int,
    expected: c_int,
) -> c_int;

pub const IMAGE_JPEG: c_int = 1;
pub const IMAGE_BITMAP: c_int = 2;

/// Équivalent de libraw_processed_image_t (tableau de taille flexible en fin de struct).
#[repr(C)]
pub struct ProcessedImage {
    pub kind: c_int,
    pub height: c_ushort,
    pub width: c_ushort,
    pub colors: c_ushort,
    pub bits: c_ushort,
    pub data_size: c_uint,
    pub data: [c_uchar; 1],
}

/// Équivalent de `tri_raw_info` (shim) : données brutes du capteur après `unpack`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RawInfo {
    pub raw_image: *const c_ushort,
    pub raw_width: c_uint,
    pub raw_height: c_uint,
    pub raw_pitch: c_uint,
    pub left_margin: c_uint,
    pub top_margin: c_uint,
    pub width: c_uint,
    pub height: c_uint,
    pub black: c_uint,
    pub maximum: c_uint,
    pub cblack: *const c_uint,
    pub cblack_len: c_uint,
    pub filters: c_uint,
}

/// Équivalent de `tri_metadata` (shim) : métadonnées de prise de vue.
#[repr(C)]
pub struct Metadata {
    pub make: [c_char; 64],
    pub model: [c_char; 64],
    pub lens: [c_char; 128],
    pub iso: c_float,
    pub shutter: c_float,
    pub aperture: c_float,
    pub focal: c_float,
    pub focal_35mm: c_float,
    pub date: [c_char; 32],
    pub width: c_int,
    pub height: c_int,
}

unsafe extern "C" {
    pub fn libraw_version() -> *const c_char;
    pub fn libraw_strerror(err: c_int) -> *const c_char;
    pub fn libraw_init(flags: c_uint) -> *mut LibRawData;
    #[cfg(not(windows))]
    pub fn libraw_open_file(lr: *mut LibRawData, path: *const c_char) -> c_int;
    #[cfg(windows)]
    pub fn libraw_open_wfile(lr: *mut LibRawData, path: *const u16) -> c_int;
    pub fn libraw_unpack_thumb(lr: *mut LibRawData) -> c_int;
    pub fn libraw_unpack(lr: *mut LibRawData) -> c_int;
    pub fn libraw_dcraw_process(lr: *mut LibRawData) -> c_int;
    pub fn libraw_dcraw_make_mem_thumb(lr: *mut LibRawData, err: *mut c_int)
    -> *mut ProcessedImage;
    pub fn libraw_dcraw_make_mem_image(lr: *mut LibRawData, err: *mut c_int)
    -> *mut ProcessedImage;
    pub fn libraw_dcraw_clear_mem(img: *mut ProcessedImage);
    pub fn libraw_set_output_bps(lr: *mut LibRawData, value: c_int);
    pub fn libraw_set_bright(lr: *mut LibRawData, value: c_float);
    pub fn libraw_set_no_auto_bright(lr: *mut LibRawData, value: c_int);
    pub fn libraw_set_progress_handler(
        lr: *mut LibRawData,
        cb: Option<ProgressCallback>,
        data: *mut c_void,
    );
    pub fn libraw_close(lr: *mut LibRawData);
    pub fn libraw_COLOR(lr: *mut LibRawData, row: c_int, col: c_int) -> c_int;

    // Shim C : champs de `params` sans setter dans l'API C.
    pub fn tri_libraw_set_use_camera_wb(lr: *mut LibRawData, value: c_int);
    pub fn tri_libraw_set_half_size(lr: *mut LibRawData, value: c_int);
    pub fn tri_libraw_get_flip(lr: *mut LibRawData) -> c_int;
    pub fn tri_libraw_get_size(lr: *mut LibRawData, width: *mut c_int, height: *mut c_int);
    pub fn tri_libraw_raw_info(lr: *mut LibRawData, out: *mut RawInfo);
    pub fn tri_libraw_metadata(lr: *mut LibRawData, out: *mut Metadata);
}
