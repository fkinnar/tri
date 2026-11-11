//! Réinitialiser un dossier (issue n° 411, décidé le 11/11/2026) : on coche ce qu'on
//! efface, parmi le cache, les données de tri, les notes et les versions.
//!
//! - **Cache** : les entrées des images du dossier, gardées avec les photos
//!   (`.tri/cache`) ou dans le cache central ; elles se recalculent.
//! - **Données de tri** : les fichiers du dossier ([`crate::store`] : filtres et tri,
//!   curseurs, couvertures, développements préférés, mémoire des notes) ; l'interrupteur
//!   « garder les données de tri ici » reste.
//! - **Notes** : étoiles, couleurs et drapeau **remis à zéro dans les sidecars**, jamais
//!   supprimés (idée de l'utilisateur) : seuls nos champs changent, le reste (historique
//!   darktable, mots-clés de Lightroom…) est réécrit à l'identique. Toutes les sources de
//!   la photo, sinon une source restée notée reviendrait. Une photo en conflit est sautée.
//! - **Versions** : la liste `tri:Versions` (recadrages) vidée.
//!
//! Notes et versions s'annulent ensemble, en un pas ([`crate::history::Step::Reset`]) ;
//! le cache et les données, non. Voir `doc/en/reset.md`.

use std::path::{Path, PathBuf};

use crate::cache::{Cache, Usage};
use crate::history::NoteChange;
use crate::notes::NoteBook;
use crate::xmp::{Convention, Notes};

/// Ce qu'on efface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Parts {
    /// Le cache des images.
    pub cache: bool,
    /// Les données de tri du dossier.
    pub data: bool,
    /// Les notes (étoiles, couleurs, drapeau).
    pub notes: bool,
    /// Les versions (recadrages).
    pub versions: bool,
}

impl Parts {
    /// Rien de coché.
    pub fn is_empty(&self) -> bool {
        !(self.cache || self.data || self.notes || self.versions)
    }

    /// Lu de la ligne de commande : `cache,data,notes,versions` (`ratings` accepté pour
    /// les notes).
    pub fn parse(text: &str) -> Option<Parts> {
        let mut parts = Parts::default();
        for word in text.split(',').map(str::trim).filter(|w| !w.is_empty()) {
            match word {
                "cache" => parts.cache = true,
                "data" => parts.data = true,
                "notes" | "ratings" => parts.notes = true,
                "versions" => parts.versions = true,
                _ => return None,
            }
        }
        (!parts.is_empty()).then_some(parts)
    }
}

/// Dossiers concernés : `folder`, et ses sous-dossiers (sauf cachés) sur `depth` niveaux
/// en tout (1 : le dossier seul ; `usize::MAX` : tous).
pub fn folders(folder: &Path, depth: usize) -> Vec<PathBuf> {
    walkdir::WalkDir::new(folder)
        .max_depth(depth.max(1) - 1)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| e.file_type().is_dir() && (e.depth() == 0 || !hidden(e)))
        .flatten()
        .map(|e| e.into_path())
        .collect()
}

fn hidden(entry: &walkdir::DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with('.')
}

/// Ce qu'une réinitialisation effacerait, mesuré pour la boîte (en fond : il faut lire
/// les notes et les versions de chaque image).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Survey {
    /// Place du cache.
    pub cache: Usage,
    /// Fichiers de données de tri.
    pub data: usize,
    /// Images notées (étoiles, couleur ou drapeau).
    pub notes: usize,
    /// Images en conflit de notes (sautées).
    pub conflicts: usize,
    /// Images qui ont des versions, et nombre de versions.
    pub versions: (usize, usize),
}

/// Mesure ce que [`run`] effacerait dans `folders` ([`folders`]).
pub fn survey(
    cache: Option<&Cache>,
    data: Option<&Path>,
    folders: &[PathBuf],
    convention: Convention,
) -> Survey {
    let mut survey = Survey::default();
    for folder in folders {
        if let Some(cache) = cache {
            survey
                .cache
                .add(cache.folder_usage(folder).unwrap_or_default());
        }
        survey.data += crate::store::count(data, folder);
        let images = images(folder);
        let book = NoteBook::load_files(&images, convention);
        for image in images.iter().map(|f| &f.path) {
            if book.conflict(image).is_some() {
                survey.conflicts += 1;
            } else if *book.notes(image) != Notes::EMPTY {
                survey.notes += 1;
            }
            let n = versions_of(image).len();
            if n > 0 {
                survey.versions.0 += 1;
                survey.versions.1 += n;
            }
        }
    }
    survey
}

fn images(folder: &Path) -> Vec<crate::media::MediaFile> {
    crate::media::scan(folder, 1).unwrap_or_default()
}

fn versions_of(image: &Path) -> Vec<String> {
    crate::xmp::read_list_of(image, crate::xmp::VERSIONS).unwrap_or_default()
}

/// Ce qu'une réinitialisation a fait.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    /// Place libérée dans le cache.
    pub freed: Usage,
    /// Fichiers de données de tri effacés.
    pub data: usize,
    /// Notes remises à zéro (pour l'historique).
    pub notes: Vec<NoteChange>,
    /// Versions retirées : image et liste d'avant (pour l'historique).
    pub versions: Vec<(PathBuf, Vec<String>)>,
    /// Images laissées, et pourquoi (conflit, sidecar illisible…).
    pub skipped: Vec<(PathBuf, String)>,
}

impl Outcome {
    /// Le pas d'historique de ce qui s'annule (notes et versions), s'il y a quelque chose.
    pub fn step(&self, folder: &Path) -> Option<crate::history::Step> {
        (!self.notes.is_empty() || !self.versions.is_empty()).then(|| crate::history::Step::Reset {
            folder: folder.to_path_buf(),
            notes: self.notes.clone(),
            versions: self.versions.clone(),
        })
    }
}

/// Efface `parts` dans `folders` ([`folders`]). `cache` : sans lui, le cache n'est pas
/// touché ; `data` : dossier de données de `tri` (replis des données de tri). Au mieux :
/// une image qu'on ne peut pas écrire est laissée et signalée, le reste continue.
pub fn run(
    cache: Option<&Cache>,
    data: Option<&Path>,
    folders: &[PathBuf],
    parts: Parts,
    convention: Convention,
) -> Outcome {
    let mut outcome = Outcome::default();
    for folder in folders {
        if parts.cache
            && let Some(cache) = cache
            && let Ok(freed) = cache.clear_folder(folder, None)
        {
            outcome.freed.add(freed);
        }
        if parts.notes || parts.versions {
            let images = images(folder);
            if parts.notes {
                clear_notes(&images, convention, &mut outcome);
            }
            if parts.versions {
                for image in images.iter().map(|f| &f.path) {
                    let before = versions_of(image);
                    if before.is_empty() {
                        continue;
                    }
                    match crate::xmp::write_list(image, crate::xmp::VERSIONS, &[], convention) {
                        Ok(_) => outcome.versions.push((image.clone(), before)),
                        Err(e) => outcome.skipped.push((image.clone(), e.to_string())),
                    }
                }
            }
        }
        // Les données après les notes : la mémoire des notes vient d'être réécrite.
        if parts.data {
            outcome.data += crate::store::clear(data, folder).unwrap_or(0);
        }
    }
    outcome
}

fn clear_notes(images: &[crate::media::MediaFile], convention: Convention, outcome: &mut Outcome) {
    let mut book = NoteBook::load_files(images, convention);
    for image in images.iter().map(|f| &f.path) {
        let before = book.notes(image).clone();
        if before == Notes::EMPTY && book.conflict(image).is_none() {
            continue;
        }
        match book.clear(image) {
            Ok(()) => outcome.notes.push(NoteChange {
                image: image.clone(),
                before,
                after: Notes::EMPTY,
            }),
            Err(e) => outcome.skipped.push((image.clone(), e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xmp::{Color, Flag};

    fn image(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        image::RgbImage::new(4, 4).save(&path).unwrap();
        path
    }

    #[test]
    fn parts_are_read_from_the_command_line() {
        let all = Parts::parse("cache, data,ratings,versions").unwrap();
        assert!(all.cache && all.data && all.notes && all.versions);
        assert_eq!(
            Parts::parse("notes"),
            Some(Parts {
                notes: true,
                ..Parts::default()
            })
        );
        assert_eq!(Parts::parse("tout"), None);
        assert_eq!(Parts::parse(""), None);
    }

    #[test]
    fn ratings_and_versions_are_reset_and_the_rest_of_the_sidecar_kept() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path();
        let a = image(photos, "a.png");
        let b = image(photos, "b.png");
        let c = image(photos, "c.png");
        let mut book = NoteBook::load_files(&images(photos), Convention::Darktable);
        book.edit(&a, |n| {
            n.set_stars(4);
            n.colors = vec![Color::Red];
        })
        .unwrap();
        book.edit(&b, |n| n.flag = Flag::Rejected).unwrap();
        // Un champ d'un autre logiciel, à garder.
        let sidecar = book.sidecar(&a).unwrap();
        let text = std::fs::read_to_string(&sidecar).unwrap();
        let text = text.replacen(
            "xmp:Rating=",
            "xmp:CreatorTool=\"Lightroom\" xmp:Rating=",
            1,
        );
        std::fs::write(&sidecar, text).unwrap();
        crate::xmp::write_list(
            &a,
            crate::xmp::VERSIONS,
            &["{\"name\":\"Story\"}".into()],
            Convention::Darktable,
        )
        .unwrap();
        let folders = folders(photos, 1);
        let survey = survey(None, None, &folders, Convention::Darktable);
        assert_eq!((survey.notes, survey.versions), (2, (1, 1)));
        // Notes seulement : les versions restent.
        let parts = Parts {
            notes: true,
            ..Parts::default()
        };
        let outcome = run(None, None, &folders, parts, Convention::Darktable);
        assert_eq!(outcome.notes.len(), 2);
        assert!(outcome.skipped.is_empty(), "{:?}", outcome.skipped);
        let book = NoteBook::load_files(&images(photos), Convention::Darktable);
        assert_eq!(*book.notes(&a), Notes::EMPTY);
        assert_eq!(*book.notes(&b), Notes::EMPTY);
        assert!(sidecar.is_file(), "jamais supprimé");
        assert!(
            std::fs::read_to_string(&sidecar)
                .unwrap()
                .contains("xmp:CreatorTool=\"Lightroom\"")
        );
        assert!(!c.with_extension("png.xmp").exists(), "rien créé");
        assert_eq!(versions_of(&a).len(), 1);
        // Puis les versions ; l'historique les rend toutes (un pas).
        let parts = Parts {
            versions: true,
            ..Parts::default()
        };
        let more = run(None, None, &folders, parts, Convention::Darktable);
        assert!(versions_of(&a).is_empty());
        let mut history = crate::history::History::new(10);
        history.push(outcome.step(photos).unwrap());
        history.push(more.step(photos).unwrap());
        history.undo(Convention::Darktable).unwrap();
        assert_eq!(versions_of(&a).len(), 1);
        history.undo(Convention::Darktable).unwrap();
        let book = NoteBook::load_files(&images(photos), Convention::Darktable);
        assert_eq!(book.notes(&a).stars, 4);
        assert_eq!(book.notes(&b).flag, Flag::Rejected);
    }

    #[test]
    fn data_and_cache_are_cleared_in_subfolders_too() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let photos = dir.path().join("séance");
        let sub = photos.join("b");
        std::fs::create_dir_all(&sub).unwrap();
        let cache = Cache::open(dir.path().join("cache")).unwrap();
        let x = image(&sub, "x.png");
        let key = crate::cache::Key::new(&x, crate::cache::Level::Thumbnail, "p").unwrap();
        cache.put_bytes(&key, &[0; 100]).unwrap();
        let d = Some(data.as_path());
        crate::store::set_value(d, &photos, "group-strictness", Some(0.3)).unwrap();
        crate::store::set_value(d, &sub, "blur-strictness", Some(0.1)).unwrap();
        assert_eq!(folders(&photos, 1), std::slice::from_ref(&photos));
        let all = folders(&photos, usize::MAX);
        assert_eq!(all, [photos.clone(), sub.clone()]);
        let survey = survey(Some(&cache), d, &all, Convention::Darktable);
        assert_eq!((survey.data, survey.cache.entries), (2, 1));
        let parts = Parts {
            cache: true,
            data: true,
            ..Parts::default()
        };
        let outcome = run(Some(&cache), d, &all, parts, Convention::Darktable);
        assert_eq!((outcome.data, outcome.freed.entries), (2, 1));
        assert!(outcome.step(&photos).is_none(), "rien à annuler");
        assert!(!cache.contains(&key));
        assert_eq!(crate::store::value(d, &sub, "blur-strictness"), None);
    }
}
