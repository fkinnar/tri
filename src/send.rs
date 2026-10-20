//! Envoi d'images vers une destination : plugins d'envoi (idée de l'utilisateur, « après
//! la V1 », avancée au 19/10/2026 pour éprouver l'architecture des plugins).
//!
//! Un trait ([`Sender`]) et une liste ([`senders`]) : chaque plugin est écrit en Rust et
//! compilé avec `tri`, comme décidé pour les plugins (pas de `libloading` tant qu'aucun
//! plugin externe n'existe). Son action porte l'identifiant `plugin.<nom>.send`, dans
//! l'espace de noms réservé aux plugins. Le premier : **AirDrop** (macOS). Voir
//! `doc/en/send.md`.

use std::path::PathBuf;

/// Un plugin d'envoi.
pub trait Sender: Send + Sync {
    /// Nom court, dans l'identifiant de son action (`plugin.<nom>.send`).
    fn id(&self) -> &'static str;
    /// Nom affiché.
    fn name(&self) -> String;
    /// Vrai si l'envoi est possible sur cette machine (AirDrop : macOS seulement).
    fn available(&self) -> bool;
    /// Envoie `files`. Peut ouvrir une fenêtre du système (AirDrop : choix du
    /// destinataire) et rendre la main avant la fin de l'envoi.
    fn send(&self, files: &[PathBuf]) -> Result<(), String>;
}

/// AirDrop, par la feuille de partage de macOS (`NSSharingService`) : le système montre
/// les appareils à portée, l'utilisateur choisit, l'envoi se fait en fond. À appeler sur
/// le fil principal de l'application (celui de l'interface).
pub struct AirDrop;

impl Sender for AirDrop {
    fn id(&self) -> &'static str {
        "airdrop"
    }

    fn name(&self) -> String {
        "AirDrop".into()
    }

    fn available(&self) -> bool {
        cfg!(target_os = "macos")
    }

    fn send(&self, files: &[PathBuf]) -> Result<(), String> {
        if files.is_empty() {
            return Err(crate::t!("send-nothing"));
        }
        airdrop(files)
    }
}

#[cfg(target_os = "macos")]
fn airdrop(files: &[PathBuf]) -> Result<(), String> {
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSSharingService, NSSharingServiceNameSendViaAirDrop};
    use objc2_foundation::{NSArray, NSString, NSURL};
    // La feuille de partage est de l'interface AppKit : fil principal obligatoire.
    if MainThreadMarker::new().is_none() {
        return Err(crate::t!("send-main-thread"));
    }
    let urls: Vec<Retained<NSURL>> = files
        .iter()
        .map(|file| {
            let absolute = std::path::absolute(file).unwrap_or_else(|_| file.clone());
            NSURL::fileURLWithPath(&NSString::from_str(&absolute.to_string_lossy()))
        })
        .collect();
    let items = NSArray::from_retained_slice(&urls);
    // SAFETY : `NSArray<NSURL>` et `NSArray` (d'objets quelconques) sont le même objet
    // Objective-C ; le paramètre générique n'existe que côté Rust. Les éléments (NSURL)
    // sont des objets que la feuille de partage accepte (NSPasteboardWriting).
    let items: &NSArray = unsafe { &*(Retained::as_ptr(&items) as *const NSArray) };
    // SAFETY : constante du système, initialisée par AppKit.
    let name = unsafe { NSSharingServiceNameSendViaAirDrop };
    let service = NSSharingService::sharingServiceNamed(name)
        .ok_or_else(|| crate::t!("send-unavailable", name = "AirDrop"))?;
    // SAFETY : `items` est un tableau de NSURL (voir plus haut).
    unsafe {
        if !service.canPerformWithItems(Some(items)) {
            return Err(crate::t!("send-refused", name = "AirDrop"));
        }
        service.performWithItems(items);
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn airdrop(_files: &[PathBuf]) -> Result<(), String> {
    Err(crate::t!("send-unavailable", name = "AirDrop"))
}

/// Les plugins d'envoi, disponibles ou non sur cette machine.
pub fn senders() -> Vec<Box<dyn Sender>> {
    vec![Box::new(AirDrop)]
}

/// Le plugin de nom `id`.
pub fn sender(id: &str) -> Option<Box<dyn Sender>> {
    senders().into_iter().find(|s| s.id() == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn airdrop_is_listed_and_refuses_an_empty_list() {
        let airdrop = sender("airdrop").expect("listé");
        assert_eq!(airdrop.name(), "AirDrop");
        assert_eq!(airdrop.available(), cfg!(target_os = "macos"));
        assert!(airdrop.send(&[]).is_err());
        assert!(sender("pigeon").is_none());
    }

    /// Hors du fil principal (les tests tournent dans des fils à part) : refusé
    /// proprement, sans ouvrir de fenêtre ni planter.
    #[test]
    #[cfg(target_os = "macos")]
    fn airdrop_refuses_outside_the_main_thread() {
        let err = AirDrop.send(&[PathBuf::from("/tmp/x.jpg")]).unwrap_err();
        assert_eq!(err, crate::t!("send-main-thread"));
    }
}
