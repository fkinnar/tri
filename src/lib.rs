//! Moteur de `tri`, visionneuse RAW pour le tri de photos.
//!
//! - [`media`] : parcours de dossier, classification RAW / image plate ;
//! - [`bitmap`] : type d'image RGB8 commun ;
//! - [`flat`] : décodage des images plates (JPEG, PNG, TIFF, WebP, HEIF) ;
//! - [`filter`] : filtres sur la liste des images à afficher ;
//! - [`raw`] : lecture et développement des RAW via LibRaw.
//!
//! Le moteur ne dépend d'aucune bibliothèque d'affichage. Voir `doc/README.md`.

#![warn(missing_docs)]

pub mod bitmap;
pub mod filter;
pub mod flat;
pub mod media;
pub mod raw;
