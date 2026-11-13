//! Réglages de visualisation à l'affichage (issue n° 137) : les réglages connus des images
//! ([`tri::adjust::Layers`]), partagés entre l'interface et le fil de chargement des
//! grandes images, et appliqués aux pixels juste avant d'en faire une texture.
//!
//! À l'affichage, tout est fait sur l'image (approximation sur un JPEG, exacte pour le
//! mélangeur) : l'exposition sur les données du capteur (LibRaw) sert à la ligne de
//! commande et à l'export (`tri::export`). Une image développée par un autre logiciel n'est
//! jamais réglée (les réglages valent *avant* le développement).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use tri::adjust::Layers;
use tri::bitmap::Bitmap;
use tri::cache::Level;
use tri::media::MediaKind;

/// État partagé.
#[derive(Debug, Default)]
pub struct Shared {
    /// Réglages des images qui en ont (lus dans les sidecars, ou posés dans la session).
    pub layers: HashMap<PathBuf, Layers>,
    /// « Avant » : les images sans leurs réglages (bascule avant / après).
    pub original: bool,
}

/// Réglages partagés avec le fil de chargement.
pub type AdjustMap = Arc<RwLock<Shared>>;

/// Réglages à appliquer à `path` au niveau `level`, s'il y en a (aucun en « avant », ni
/// pour un rendu développé).
pub fn layers_for(map: &AdjustMap, path: &Path, level: Level) -> Option<Layers> {
    if matches!(level, Level::Developed(_)) {
        return None;
    }
    let shared = map.read().unwrap_or_else(|e| e.into_inner());
    if shared.original {
        return None;
    }
    shared.layers.get(path).copied()
}

/// Applique à `bitmap` (image `path` de nature `kind`, niveau `level`) ses réglages.
pub fn apply(map: &AdjustMap, path: &Path, kind: MediaKind, level: Level, bitmap: &mut Bitmap) {
    if let Some(layers) = layers_for(map, path, level) {
        let transfer = tri::adjust::transfer_of(kind == MediaKind::Raw, level);
        tri::adjust::apply_bitmap(bitmap, transfer, layers.total(), layers.mono);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjustments_apply_unless_before_or_developed() {
        let map = AdjustMap::default();
        let path = PathBuf::from("/photos/a.jpg");
        let mut layers = Layers::default();
        layers.correction.exposure = 1.0;
        map.write().unwrap().layers.insert(path.clone(), layers);
        let grey = || Bitmap {
            width: 1,
            height: 1,
            data: vec![80, 80, 80],
        };
        let mut b = grey();
        apply(&map, &path, MediaKind::Flat, Level::Standard, &mut b);
        assert!(b.data[0] > 80, "brighter");
        let mut d = grey();
        let developed = Level::Developed(tri::develop::Engine::Darktable);
        apply(&map, &path, MediaKind::Raw, developed, &mut d);
        assert_eq!(d.data[0], 80, "a development is never adjusted");
        map.write().unwrap().original = true;
        let mut o = grey();
        apply(&map, &path, MediaKind::Flat, Level::Standard, &mut o);
        assert_eq!(o.data[0], 80, "before");
    }
}
