//! Moteur de `tri`, visionneuse RAW pour le tri de photos.
//!
//! - [`media`] : parcours de dossier, classification RAW / image plate ;
//! - [`bitmap`] : type d'image RGB8 commun ;
//! - [`cache`] : cache persistant des vignettes et aperçus ;
//! - [`flat`] : décodage des images plates (JPEG, PNG, TIFF, WebP, HEIF) ;
//! - [`jpeg`] : décodage / encodage JPEG rapides (libjpeg-turbo, repli pur Rust) ;
//! - [`filter`] : filtres sur la liste des images à afficher ;
//! - [`raw`] : lecture et développement des RAW via LibRaw ;
//! - [`preview`] : calcul des vignettes, aperçus et rendus 1:1 ;
//! - [`scheduler`] : file de travail en tâche de fond, priorités, progression ;
//! - [`settings`] : fichier de configuration (TOML), emplacement et lecture ;
//! - [`xmp`] : notes de tri (étoiles, couleurs, accepté / rejeté) dans les sidecars XMP.
//!
//! Le moteur ne dépend d'aucune bibliothèque d'affichage. Voir `doc/README.md`.

#![warn(missing_docs)]

pub mod bitmap;
pub mod cache;
pub mod filter;
pub mod flat;
pub mod jpeg;
pub mod media;
pub mod preview;
pub mod raw;
pub mod scheduler;
pub mod settings;
pub mod xmp;
