//! Moteur de `tri`, visionneuse RAW pour le tri de photos.
//!
//! - [`media`] : parcours de dossier, classification RAW / image plate ;
//! - [`filter`] : filtres sur la liste des images à afficher ;
//! - [`raw`] : lecture et développement des RAW via LibRaw.
//!
//! Le moteur ne dépend d'aucune bibliothèque d'affichage. Voir `doc/README.md`.

#![warn(missing_docs)]

pub mod filter;
pub mod media;
pub mod raw;
