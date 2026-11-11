//! Couleurs à l'écran (retour du 11/11/2026 : sur un écran wide gamut calibré, le rendu
//! darktable sortait « largement trop saturé » ; issue n° 422, `doc/en/colour.md`).
//!
//! Les images arrivent dans l'espace de travail de `tri` ([`tri::colour`] : primaires
//! Rec.2020, courbe sRGB).
//!
//! - **macOS** : la fenêtre est déclarée dans cet espace ([`declare_working`]), et le
//!   système convertit vers le profil de l'écran (ColorSync, calibration comprise), d'un
//!   écran à l'autre. L'interface d'egui (couleurs sRGB) y est lue avec les mêmes gris ;
//!   ses couleurs vives en sortent plus saturées (accepté le 11/11/2026).
//! - **Linux, Windows** : le système ne convertit rien pour les applications. Chaque image
//!   est convertie vers le profil de l'écran avant d'en faire une texture ([`to_screen`]) :
//!   celui de `[gui] display_profile` s'il est donné, sinon celui du système
//!   ([`Screen::follow`] : sous Linux, la propriété X11 `_ICC_PROFILE` que publie colord,
//!   écran principal ; sous Windows, celui de l'écran où est la fenêtre, suivi quand elle
//!   change d'écran), sinon sRGB.
//!
//! Sur macOS, wgpu traduit « sRGB » par **aucun** espace couleur sur la couche Metal
//! (`CAMetalLayer.colorspace = nil`) ; macOS n'applique alors aucune conversion : les
//! valeurs partaient telles quelles vers l'écran, et un écran wide gamut les étirait sur
//! tout son gamut. wgpu remet cet espace à zéro à chaque reconfiguration de la surface
//! (redimensionnement…), et ni eframe ni egui-wgpu ne permettent de le choisir : on le
//! redéclare donc à chaque image, ce qui ne coûte que deux messages Objective-C quand il
//! est déjà en place.

use std::path::Path;
use std::sync::{PoisonError, RwLock};

use tri::bitmap::Bitmap;
use tri::colour::Transform;

/// Conversion vers l'écran en vigueur (hors macOS), lue par les fils de chargement ;
/// `None` avant le premier [`Screen::follow`] : vers le sRGB.
static SCREEN: RwLock<Option<Transform>> = RwLock::new(None);

/// Ramène `bitmap` (espace de travail) à l'écran, juste avant d'en faire une texture :
/// rien sur macOS (le système s'en charge), vers le profil de l'écran ailleurs.
pub fn to_screen(bitmap: &mut Bitmap) {
    if cfg!(target_os = "macos") {
        return;
    }
    let current = SCREEN
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    if let Some(t) = current.or_else(|| tri::colour::to_screen(None)) {
        t.apply(bitmap);
    }
}

/// D'où vient le profil de l'écran.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Source {
    /// Pas encore regardé.
    Unknown,
    /// Le fichier de `[gui] display_profile`.
    Forced,
    /// Celui du système (l'écran, sous Windows ; `0` ailleurs).
    System(isize),
    /// Aucun profil trouvé : sRGB (l'écran, comme pour `System`).
    Srgb(isize),
}

/// Suivi du profil de l'écran, gardé par l'application.
#[derive(Debug)]
pub struct Screen {
    source: Source,
    /// Réglage `[gui] display_profile` au dernier passage.
    forced: Option<String>,
    /// Description du profil en vigueur (journal, trace).
    pub description: String,
}

impl Default for Screen {
    fn default() -> Self {
        Screen {
            source: Source::Unknown,
            forced: None,
            description: "sRGB".into(),
        }
    }
}

impl Screen {
    /// À chaque image : le profil en vigueur pour la fenêtre de `frame` (`forced` : le
    /// réglage `[gui] display_profile`, vide : celui du système). `true` s'il vient de
    /// changer : les textures déjà faites sont à refaire. Rien sur macOS.
    pub fn follow(&mut self, frame: &eframe::Frame, forced: &str) -> bool {
        if cfg!(target_os = "macos") {
            return false;
        }
        let forced = forced.trim();
        let forced_changed = self.forced.as_deref() != Some(forced);
        if forced_changed {
            self.forced = Some(forced.to_string());
        }
        if !forced.is_empty() {
            if !forced_changed {
                return false;
            }
            let path = tri::settings::expand_home(forced);
            match read_profile(&path) {
                Ok(icc) => return self.install(Source::Forced, Some(icc)),
                Err(e) => {
                    tri::log::warning(
                        "display",
                        &format!("[gui] display_profile: {}", path.display()),
                        &e,
                    );
                }
            }
        }
        // Le profil du système : relu seulement au démarrage, quand le réglage change, ou
        // quand la fenêtre passe sur un autre écran (Windows).
        let monitor = monitor_of(frame);
        if !forced_changed
            && matches!(self.source, Source::System(m) | Source::Srgb(m) if m == monitor)
        {
            return false;
        }
        match system_profile(frame) {
            Some(icc) => self.install(Source::System(monitor), Some(icc)),
            None => self.install(Source::Srgb(monitor), None),
        }
    }

    /// Pose la conversion vers `icc` (`None` : sRGB) ; `true` si elle change.
    fn install(&mut self, source: Source, icc: Option<Vec<u8>>) -> bool {
        let transform = icc
            .as_deref()
            .and_then(|icc| tri::colour::to_screen(Some(icc)));
        let monitor = match source {
            Source::System(m) | Source::Srgb(m) => m,
            _ => 0,
        };
        let (source, description) = match (&icc, &transform) {
            (Some(icc), Some(_)) => (source, describe(icc)),
            (Some(icc), None) => {
                // Profil illisible par `moxcms` : sRGB, et on le dit.
                tri::log::warning(
                    "display",
                    &format!("screen profile not usable: {}", describe(icc)),
                    "",
                );
                (Source::Srgb(monitor), "sRGB".to_string())
            }
            (None, _) => (Source::Srgb(monitor), "sRGB".to_string()),
        };
        let changed = self.description != description || self.source == Source::Unknown;
        self.source = source;
        self.description = description;
        *SCREEN.write().unwrap_or_else(PoisonError::into_inner) =
            transform.or_else(|| tri::colour::to_screen(None));
        crate::trace::trace(|| format!("profil de l'écran : {}", self.description));
        changed
    }
}

/// Lit un fichier de profil ICC (`.icc`, `.icm`), refusé s'il est illisible.
fn read_profile(path: &Path) -> Result<Vec<u8>, String> {
    let icc = std::fs::read(path).map_err(|e| e.to_string())?;
    moxcms::ColorProfile::new_from_slice(&icc).map_err(|e| format!("{e:?}"))?;
    Ok(icc)
}

/// Description lisible d'un profil ICC (sa balise `desc`), sinon « ICC ».
fn describe(icc: &[u8]) -> String {
    use moxcms::ProfileText;
    let text = moxcms::ColorProfile::new_from_slice(icc)
        .ok()
        .and_then(|p| p.description)
        .and_then(|d| match d {
            ProfileText::PlainString(s) => Some(s),
            ProfileText::Localizable(l) => l.into_iter().next().map(|s| s.value),
            ProfileText::Description(d) => Some(d.ascii_string),
        });
    text.filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "ICC".into())
}

/// L'écran où est la fenêtre (Windows : son `HMONITOR`) ; `0` ailleurs (un seul profil
/// suivi : celui de l'écran principal).
#[cfg(windows)]
fn monitor_of(frame: &eframe::Frame) -> isize {
    use eframe::wgpu::rwh::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow};
    let Ok(handle) = frame.window_handle() else {
        return 0;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return 0;
    };
    // SAFETY : la poignée d'une fenêtre vivante ; l'appel ne fait que la lire.
    let monitor = unsafe { MonitorFromWindow(h.hwnd.get() as _, MONITOR_DEFAULTTONEAREST) };
    monitor as isize
}

#[cfg(not(windows))]
fn monitor_of(_frame: &eframe::Frame) -> isize {
    0
}

/// Profil de l'écran de la fenêtre, d'après Windows (`GetICMProfileW` sur le contexte de
/// son écran : le fichier `.icm` associé dans « Gestion des couleurs »).
#[cfg(windows)]
fn system_profile(frame: &eframe::Frame) -> Option<Vec<u8>> {
    use windows_sys::Win32::Graphics::Gdi::{
        CreateDCW, DeleteDC, GetMonitorInfoW, MONITORINFO, MONITORINFOEXW,
    };
    use windows_sys::Win32::UI::ColorSystem::GetICMProfileW;
    let monitor = monitor_of(frame);
    if monitor == 0 {
        return None;
    }
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    // SAFETY : `info` est un MONITORINFOEXW dont `cbSize` dit la taille, comme demandé.
    let ok = unsafe { GetMonitorInfoW(monitor as _, (&raw mut info).cast::<MONITORINFO>()) };
    if ok == 0 {
        return None;
    }
    let device = info.szDevice.as_ptr();
    // SAFETY : `szDevice` est le nom de l'écran, terminé par un zéro.
    let dc = unsafe { CreateDCW(device, device, std::ptr::null(), std::ptr::null()) };
    if dc.is_null() {
        return None;
    }
    let mut len: u32 = 0;
    // SAFETY : premier appel sans tampon, pour la taille (en caractères, zéro compris).
    unsafe { GetICMProfileW(dc, &mut len, std::ptr::null_mut()) };
    let mut name = vec![0u16; len.max(1) as usize];
    // SAFETY : tampon de `len` caractères.
    let ok = unsafe { GetICMProfileW(dc, &mut len, name.as_mut_ptr()) };
    // SAFETY : contexte créé juste au-dessus.
    unsafe { DeleteDC(dc) };
    if ok == 0 {
        return None;
    }
    let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    let path = std::path::PathBuf::from(String::from_utf16_lossy(&name[..end]));
    read_profile(&path).ok()
}

/// Profil de l'écran principal, d'après la propriété X11 `_ICC_PROFILE` de la fenêtre
/// racine (convention « ICC Profiles in X », publiée par colord ou `dispwin`) ; sous
/// Wayland, lue par XWayland quand le compositeur la publie.
#[cfg(all(unix, not(target_os = "macos")))]
fn system_profile(_frame: &eframe::Frame) -> Option<Vec<u8>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
    let (conn, screen) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen)?.root;
    let atom = conn
        .intern_atom(true, b"_ICC_PROFILE")
        .ok()?
        .reply()
        .ok()?
        .atom;
    if atom == 0 {
        return None;
    }
    let reply = conn
        .get_property(false, root, atom, AtomEnum::ANY, 0, u32::MAX / 4)
        .ok()?
        .reply()
        .ok()?;
    (reply.format == 8 && !reply.value.is_empty()).then_some(reply.value)
}

#[cfg(target_os = "macos")]
fn system_profile(_frame: &eframe::Frame) -> Option<Vec<u8>> {
    None
}

/// Déclare la fenêtre de `frame` dans l'espace de travail ; `true` si l'espace vient
/// d'être (re)posé (l'image déjà présentée l'a été sans : à redessiner).
#[cfg(target_os = "macos")]
pub fn declare_working(frame: &eframe::Frame) -> bool {
    use eframe::wgpu::rwh::{HasWindowHandle, RawWindowHandle};
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::NSObject;
    use objc2_core_foundation::CFData;
    use objc2_core_graphics::CGColorSpace;
    use objc2_quartz_core::{CALayer, CAMetalLayer};

    let Ok(handle) = frame.window_handle() else {
        return false;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return false;
    };
    // SAFETY : la poignée d'une fenêtre AppKit vivante pointe vers son `NSView`.
    let view: &NSObject = unsafe { handle.ns_view.cast().as_ref() };
    // SAFETY : `-[NSView layer]` renvoie un `CALayer` facultatif.
    let root: Option<Retained<CALayer>> = unsafe { msg_send![view, layer] };
    let Some(root) = root else {
        return false;
    };
    // La couche Metal de wgpu : la couche du `NSView`, ou l'une de ses sous-couches
    // (`raw-window-metal` en ajoute une quand la vue n'en a pas).
    let mut layers = vec![root.clone()];
    // SAFETY : lecture de la liste des sous-couches, sur le fil principal (`ui`).
    if let Some(subs) = unsafe { root.sublayers() } {
        layers.extend(subs.iter());
    }
    let mut changed = false;
    for layer in layers {
        let Ok(metal) = layer.downcast::<CAMetalLayer>() else {
            continue;
        };
        if metal.colorspace().is_some() {
            continue;
        }
        let icc = CFData::from_bytes(tri::colour::working_icc());
        // SAFETY : `CGColorSpaceCreateWithICCData` attend un `CFData`, c'en est un.
        let space = unsafe { CGColorSpace::with_icc_data(Some(&icc)) };
        metal.setColorspace(space.as_deref());
        crate::trace::trace(|| "fenêtre déclarée dans l'espace de travail".into());
        changed = true;
    }
    changed
}

/// Rien à déclarer hors macOS (voir le module).
#[cfg(not(target_os = "macos"))]
pub fn declare_working(_frame: &eframe::Frame) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_file_is_read_and_described() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p3.icc");
        std::fs::write(
            &path,
            moxcms::ColorProfile::new_display_p3().encode().unwrap(),
        )
        .unwrap();
        let icc = read_profile(&path).unwrap();
        assert_eq!(describe(&icc), "Display P3");
        let bad = dir.path().join("bad.icc");
        std::fs::write(&bad, b"pas un profil").unwrap();
        assert!(read_profile(&bad).is_err());
        assert!(read_profile(&dir.path().join("absent.icc")).is_err());
    }

    #[test]
    fn a_wide_screen_gets_less_saturated_values_than_srgb() {
        // Vers un écran Display P3, le rouge sRGB (passé par l'espace de travail) est moins
        // saturé que vers un écran sRGB : c'est la conversion qu'un écran large demande.
        let p3 = moxcms::ColorProfile::new_display_p3().encode().unwrap();
        let red = || {
            let mut b = Bitmap {
                width: 1,
                height: 1,
                data: vec![255, 0, 0],
            };
            tri::colour::bitmap_to_working(&mut b, &tri::colour::Origin::Srgb);
            b
        };
        let mut to_p3 = red();
        tri::colour::to_screen(Some(&p3)).unwrap().apply(&mut to_p3);
        let mut to_srgb = red();
        tri::colour::to_screen(None).unwrap().apply(&mut to_srgb);
        assert!(
            to_srgb.data[0] >= 253 && to_srgb.data[1] <= 2,
            "{:?}",
            to_srgb.data
        );
        assert!(
            to_p3.data[0] < 245 && to_p3.data[1] > 20,
            "{:?}",
            to_p3.data
        );
    }
}
