//! Déclarations brutes de l'API C de LibRaw (+ shim `csrc/libraw_shim.c`).

use std::os::raw::{c_char, c_float, c_int, c_uchar, c_uint, c_ushort};

/// Handle opaque : on ne connaît pas (et n'a pas besoin de connaître) son contenu.
#[repr(C)]
pub struct LibRawData {
    _private: [u8; 0],
}

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
    pub fn libraw_close(lr: *mut LibRawData);

    // Shim C : champs de `params` sans setter dans l'API C.
    pub fn tri_libraw_set_use_camera_wb(lr: *mut LibRawData, value: c_int);
    pub fn tri_libraw_set_half_size(lr: *mut LibRawData, value: c_int);
}
