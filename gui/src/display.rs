//! Espace couleur déclaré au système pour la fenêtre (retour du 11/11/2026 : sur un
//! écran wide gamut calibré, le rendu darktable sortait « largement trop saturé »).
//!
//! egui dessine en sRGB. Sur macOS, wgpu traduit pourtant « sRGB » par **aucun** espace
//! couleur sur la couche Metal (`CAMetalLayer.colorspace = nil`) ; macOS n'applique alors
//! aucune conversion : les valeurs sRGB partent telles quelles vers l'écran, et un écran
//! wide gamut les étire sur tout son gamut (couleurs trop saturées, profil de l'écran
//! ignoré). wgpu remet cet espace à zéro à chaque reconfiguration de la surface
//! (redimensionnement…), et ni eframe ni egui-wgpu ne permettent de le choisir : on le
//! redéclare donc à chaque image, ce qui ne coûte que deux messages Objective-C quand il
//! est déjà en place. macOS convertit ensuite du sRGB vers le profil de l'écran (ColorSync,
//! calibration comprise), et suit la fenêtre d'un écran à l'autre.
//!
//! Linux et Windows ne convertissent rien pour les applications : rien à déclarer ici.

/// Déclare la fenêtre de `frame` en sRGB ; `true` si l'espace vient d'être (re)posé
/// (l'image déjà présentée l'a été sans : à redessiner).
#[cfg(target_os = "macos")]
pub fn declare_srgb(frame: &eframe::Frame) -> bool {
    use eframe::wgpu::rwh::{HasWindowHandle, RawWindowHandle};
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::NSObject;
    use objc2_core_graphics::{CGColorSpace, kCGColorSpaceSRGB};
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
        // SAFETY : constante de CoreGraphics, toujours définie.
        let srgb = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }));
        metal.setColorspace(srgb.as_deref());
        crate::trace::trace(|| "fenêtre déclarée en sRGB".into());
        changed = true;
    }
    changed
}

/// Rien à déclarer hors macOS (voir le module).
#[cfg(not(target_os = "macos"))]
pub fn declare_srgb(_frame: &eframe::Frame) -> bool {
    false
}
