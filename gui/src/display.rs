//! Couleurs à l'écran (retour du 11/11/2026 : sur un écran wide gamut calibré, le rendu
//! darktable sortait « largement trop saturé » ; issue n° 422).
//!
//! Les images arrivent dans l'espace de travail de `tri` ([`tri::colour`] : primaires
//! Rec.2020, courbe sRGB).
//!
//! - **macOS** : la fenêtre est déclarée dans cet espace ([`declare_working`]), et le
//!   système convertit vers le profil de l'écran (ColorSync, calibration comprise), d'un
//!   écran à l'autre. L'interface d'egui (couleurs sRGB) y est lue avec les mêmes gris ;
//!   ses couleurs vives en sortent plus saturées (accepté le 11/11/2026).
//! - **Linux, Windows** : le système ne convertit rien ; chaque image est convertie vers
//!   l'écran avant d'en faire une texture ([`to_screen`]).
//!
//! Sur macOS, wgpu traduit « sRGB » par **aucun** espace
//! couleur sur la couche Metal (`CAMetalLayer.colorspace = nil`) ; macOS n'applique alors
//! aucune conversion : les valeurs sRGB partent telles quelles vers l'écran, et un écran
//! wide gamut les étire sur tout son gamut (couleurs trop saturées, profil de l'écran
//! ignoré). wgpu remet cet espace à zéro à chaque reconfiguration de la surface
//! (redimensionnement…), et ni eframe ni egui-wgpu ne permettent de le choisir : on le
//! redéclare donc à chaque image, ce qui ne coûte que deux messages Objective-C quand il
//! est déjà en place.

use tri::bitmap::Bitmap;

/// Ramène `bitmap` (espace de travail) à l'écran, juste avant d'en faire une texture :
/// rien sur macOS (le système s'en charge), vers le sRGB ailleurs (le profil de l'écran
/// viendra, issue n° 422).
pub fn to_screen(bitmap: &mut Bitmap) {
    if cfg!(target_os = "macos") {
        return;
    }
    if let Some(t) = tri::colour::to_screen(None) {
        t.apply(bitmap);
    }
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
