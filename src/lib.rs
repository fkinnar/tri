//! Moteur de `tri`, visionneuse RAW pour le tri de photos.
//!
//! - [`media`] : parcours de dossier, classification RAW / image plate ;
//! - [`bitmap`] : type d'image RGB8 commun ;
//! - [`cache`] : cache persistant des vignettes et aperçus ;
//! - [`flat`] : décodage des images plates (JPEG, PNG, TIFF, WebP, HEIF) ;
//! - [`jpeg`] : décodage / encodage JPEG rapides (libjpeg-turbo, repli pur Rust) ;
//! - [`exposure`] : histogrammes sur les vraies données, pixels saturés / bouchés, masque ;
//! - [`filter`] : filtres sur la liste des images à afficher ;
//! - [`folder`] : dossier ouvert (images, notes, liste filtrée) ;
//! - [`raw`] : lecture et développement des RAW via LibRaw ;
//! - [`preview`] : calcul des vignettes, aperçus et rendus 1:1 ;
//! - [`scheduler`] : file de travail en tâche de fond, priorités, progression ;
//! - [`settings`] : fichier de configuration (TOML), emplacement et lecture ;
//! - [`xmp`] : notes de tri (étoiles, couleurs, accepté / rejeté) dans les sidecars XMP ;
//! - [`metadata`] : métadonnées de prise de vue (LibRaw, EXIF), lignes à afficher ;
//! - [`notes`] : notes d'un dossier en mémoire, modification, filtres sur les notes ;
//! - [`offload`] : déchargement d'une carte vers un dossier local (copie vérifiée) ;
//! - [`places`] : emplacements (volumes montés, dossier personnel, racine) et sous-dossiers.
//!
//! Le moteur ne dépend d'aucune bibliothèque d'affichage. Voir `doc/README.md`.

#![warn(missing_docs)]

pub mod bitmap;
pub mod cache;
pub mod config_edit;
pub mod exposure;
pub mod files;
pub mod filter;
pub mod flat;
pub mod folder;
pub mod history;
pub mod i18n;
pub mod jpeg;
pub mod media;
pub mod metadata;
pub mod notes;
pub mod offload;
pub mod places;
pub mod preview;
pub mod raw;
pub mod scheduler;
pub mod selection;
pub mod settings;
pub mod sharpness;
pub mod similarity;
pub mod xmp;
