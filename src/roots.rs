//! Racines surveillées (#561) : des dossiers que `tri` parcourt de lui-même, en fond, pour
//! que « Toutes les photos » sache ce qu'ils contiennent sans qu'on les ouvre un à un, et
//! pour réparer l'index quand des photos ont bougé hors de `tri`.
//!
//! Le parcours ([`walk`]) descend dans chaque racine dossier par dossier (dossiers cachés
//! et cartes d'appareil écartés, `depth` niveaux au plus), avec une petite pause entre deux
//! dossiers pour ne rien gêner. Pour chaque dossier qui a des photos :
//! - **dossier inconnu** de l'index : c'est peut-être un dossier connu, renommé ou déplacé
//!   ([`crate::locate::recognise`]) ; si oui, ses données le suivent ;
//! - **photo inconnue** : c'est peut-être une photo connue déplacée seule (même nom, même
//!   taille, et même contenu si l'index en a l'empreinte), dont l'ancien chemin n'existe
//!   plus ; si oui, l'index la suit ([`crate::index::Index::carry`]) avec ses notes, ses
//!   palettes et son empreinte de similitude, et la mémoire des notes de `tri` aussi ;
//! - puis le dossier est noté comme vu ([`crate::index::Index::seen`]).

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::index::Index;
use crate::media::MediaFile;

/// Les photos d'un dossier : chemin, taille, date.
type Photos = Vec<(PathBuf, u64, u128)>;

/// Ce qu'a fait un parcours.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Dossiers parcourus.
    pub folders: usize,
    /// Photos vues.
    pub photos: usize,
    /// Dossiers reconnus : (ancien, nouveau).
    pub recognised: Vec<(PathBuf, PathBuf)>,
    /// Photos déplacées seules, retrouvées : (ancien, nouveau).
    pub repaired: Vec<(PathBuf, PathBuf)>,
}

impl Summary {
    /// Vrai si l'index a changé autrement que par des photos vues (réparations).
    pub fn repaired_anything(&self) -> bool {
        !self.recognised.is_empty() || !self.repaired.is_empty()
    }
}

/// Réglages d'un parcours.
#[derive(Clone, Copy, Debug)]
pub struct Walk {
    /// Niveaux de dossiers au plus sous chaque racine.
    pub depth: usize,
    /// Pause entre deux dossiers.
    pub pause: Duration,
    /// Octets lus à chaque bout pour une empreinte de contenu.
    pub chunk: usize,
}

/// Parcourt `roots` (voir le module) ; l'index est dans `data`, le cache suit les dossiers
/// reconnus. `progress` reçoit chaque dossier parcouru ; `stop` interrompt.
pub fn walk(
    data: &Path,
    cache: Option<&crate::cache::Cache>,
    roots: &[PathBuf],
    settings: Walk,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(&Path),
) -> Summary {
    let mut summary = Summary::default();
    let db = data.join("index.sqlite");
    // Ce que chaque dossier contient : les photos disparues ne sont oubliées qu'à la fin
    // (une photo partie d'un dossier parcouru avant celui où elle est arrivée doit encore
    // être connue pour y être reconnue), et jamais si le parcours est interrompu.
    let mut seen: Vec<(PathBuf, Photos)> = Vec::new();
    for root in roots {
        let Ok(root) = std::path::absolute(root) else {
            continue;
        };
        if !root.is_dir() || crate::locate::is_camera_card(&root) {
            continue;
        }
        let dirs = walkdir::WalkDir::new(&root)
            .max_depth(settings.depth)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                e.file_type().is_dir()
                    && (e.depth() == 0
                        || (!e.file_name().to_string_lossy().starts_with('.')
                            && !e.file_name().eq_ignore_ascii_case("DCIM")))
            });
        for entry in dirs.flatten() {
            if stop.load(Ordering::Relaxed) {
                return summary;
            }
            let dir = entry.into_path();
            progress(&dir);
            summary.folders += 1;
            match folder(data, &db, cache, &dir, settings.chunk, &mut summary) {
                Ok(Some(photos)) => seen.push((dir, photos)),
                Ok(None) => {}
                Err(e) => {
                    crate::log::warning("index", &format!("{} not walked", dir.display()), &e);
                }
            }
            if !settings.pause.is_zero() {
                std::thread::sleep(settings.pause);
            }
        }
    }
    if let Ok(mut index) = Index::open(&db) {
        for (dir, photos) in &seen {
            if stop.load(Ordering::Relaxed) {
                return summary;
            }
            if let Err(e) = index.seen(dir, 1, photos) {
                crate::log::warning(
                    "index",
                    &format!("{} not recorded", dir.display()),
                    &e.to_string(),
                );
            }
        }
    }
    if summary.repaired_anything() {
        crate::log::info(
            "index",
            &format!(
                "watched folders: {} folders recognised, {} photos found again",
                summary.recognised.len(),
                summary.repaired.len()
            ),
            "",
        );
    }
    summary
}

/// Un dossier du parcours : ses photos notées (sans rien oublier encore) et réparées ;
/// rend ses photos, pour oublier à la fin celles qui n'y sont plus (`None` : rien à noter).
fn folder(
    data: &Path,
    db: &Path,
    cache: Option<&crate::cache::Cache>,
    dir: &Path,
    chunk: usize,
    summary: &mut Summary,
) -> Result<Option<Photos>, String> {
    let files: Vec<MediaFile> = crate::media::scan(dir, 1)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|f| !crate::media::is_development(&f.path))
        .collect();
    let known = {
        let index = Index::open(db).map_err(|e| e.to_string())?;
        index.photos_in(dir).map_err(|e| e.to_string())?
    };
    if files.is_empty() && known.is_empty() {
        return Ok(None);
    }
    // Un dossier connu, renommé ou déplacé hors de `tri`.
    if known.is_empty()
        && !files.is_empty()
        && let Some(old) = crate::locate::recognise(data, cache, dir, &files, chunk)
    {
        summary.recognised.push((old, dir.to_path_buf()));
    }
    let mut index = Index::open(db).map_err(|e| e.to_string())?;
    let known: HashSet<OsString> = index
        .photos_in(dir)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|s| s.name)
        .collect();
    // Les photos inconnues : déplacées seules ?
    let mut photos = Vec::with_capacity(files.len());
    let mut moved = Vec::new();
    for file in &files {
        let Some(name) = file.path.file_name() else {
            continue;
        };
        let Ok(meta) = std::fs::metadata(&file.path) else {
            continue;
        };
        let stamp = (
            meta.len(),
            crate::cache::read_stamp(&file.path).map_or(0, |s| s.1),
        );
        photos.push((file.path.clone(), stamp.0, stamp.1));
        if known.contains(name) {
            continue;
        }
        if let Some(old) = moved_alone(&index, &file.path, name, stamp.0, chunk) {
            moved.push((old, file.path.clone(), Some(stamp)));
        }
    }
    if !moved.is_empty() {
        index.carry(&moved, false).map_err(|e| e.to_string())?;
        let pairs: Vec<(PathBuf, PathBuf)> = moved
            .iter()
            .map(|(a, b, _)| (a.clone(), b.clone()))
            .collect();
        crate::reconcile::carry(&pairs, false);
        summary.repaired.extend(pairs);
    }
    summary.photos += photos.len();
    // Profondeur 0 : noter sans rien oublier (voir `walk`).
    index.seen(dir, 0, &photos).map_err(|e| e.to_string())?;
    Ok(Some(photos))
}

/// L'ancien chemin d'une photo déplacée seule : connu de l'index, même nom, même taille,
/// qui n'existe plus, et même contenu si l'index en a l'empreinte.
fn moved_alone(
    index: &Index,
    path: &Path,
    name: &std::ffi::OsStr,
    size: u64,
    chunk: usize,
) -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = index
        .with_sizes(&[size])
        .ok()?
        .into_iter()
        .map(|(p, _)| p)
        .filter(|p| p.as_path() != path && p.file_name() == Some(name) && !p.exists())
        .collect();
    candidates.into_iter().find(|old| {
        let samples = index
            .photos_in(old.parent().unwrap_or(old))
            .unwrap_or_default();
        let fingerprint = samples
            .iter()
            .find(|s| s.name == name)
            .and_then(|s| s.fingerprint);
        match fingerprint {
            Some(fp) => crate::index::fingerprint(path, chunk).is_ok_and(|(_, found)| found == fp),
            None => true,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn settings() -> Walk {
        Walk {
            depth: 8,
            pause: Duration::ZERO,
            chunk: 1024,
        }
    }

    fn index_paths(data: &Path) -> Vec<PathBuf> {
        let index = Index::open(&data.join("index.sqlite")).unwrap();
        let mut paths: Vec<PathBuf> = index
            .photos(None)
            .unwrap()
            .into_iter()
            .map(|e| e.0)
            .collect();
        paths.sort();
        paths
    }

    #[test]
    fn a_walk_records_photos_and_skips_cards_and_hidden_folders() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let root = std::path::absolute(dir.path().join("Photos")).unwrap();
        write(&root.join("2026/a/A.jpg"), b"a");
        write(&root.join("2026/b/B.jpg"), b"b");
        write(&root.join(".cache/C.jpg"), b"c");
        write(&root.join("carte/DCIM/100/D.jpg"), b"d");
        std::fs::create_dir_all(&data).unwrap();
        let stop = AtomicBool::new(false);
        let summary = walk(
            &data,
            None,
            std::slice::from_ref(&root),
            settings(),
            &stop,
            &mut |_| {},
        );
        assert_eq!(summary.photos, 2);
        assert_eq!(
            index_paths(&data),
            vec![root.join("2026/a/A.jpg"), root.join("2026/b/B.jpg")]
        );
    }

    #[test]
    fn a_photo_moved_alone_keeps_what_the_index_knew() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let root = std::path::absolute(dir.path().join("Photos")).unwrap();
        write(&root.join("a/A.jpg"), b"photo a");
        write(&root.join("a/B.jpg"), b"photo b");
        std::fs::create_dir_all(&data).unwrap();
        let stop = AtomicBool::new(false);
        walk(
            &data,
            None,
            std::slice::from_ref(&root),
            settings(),
            &stop,
            &mut |_| {},
        );
        {
            let mut index = Index::open(&data.join("index.sqlite")).unwrap();
            let notes = crate::xmp::Notes {
                stars: 4,
                ..Default::default()
            };
            index
                .record_notes(&[(root.join("a").join("A.jpg"), notes)])
                .unwrap();
        }
        // Déplacée seule, hors de `tri`, dans un autre dossier de la racine.
        write(&root.join("b/A.jpg"), b"photo a");
        std::fs::remove_file(root.join("a/A.jpg")).unwrap();
        let summary = walk(
            &data,
            None,
            std::slice::from_ref(&root),
            settings(),
            &stop,
            &mut |_| {},
        );
        assert_eq!(
            summary.repaired,
            vec![(root.join("a/A.jpg"), root.join("b/A.jpg"))]
        );
        assert_eq!(
            index_paths(&data),
            vec![root.join("a/B.jpg"), root.join("b/A.jpg")]
        );
        let index = Index::open(&data.join("index.sqlite")).unwrap();
        assert_eq!(
            index.notes_in(&root.join("b")).unwrap()[0].1.stars,
            4,
            "ses notes"
        );
    }

    #[test]
    fn a_renamed_folder_is_recognised_by_the_walk() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let root = std::path::absolute(dir.path().join("Photos")).unwrap();
        write(&root.join("TEST3/A.jpg"), b"photo a");
        write(&root.join("TEST3/B.jpg"), b"photo bb");
        std::fs::create_dir_all(&data).unwrap();
        let stop = AtomicBool::new(false);
        walk(
            &data,
            None,
            std::slice::from_ref(&root),
            settings(),
            &stop,
            &mut |_| {},
        );
        std::fs::rename(root.join("TEST3"), root.join("TEST4")).unwrap();
        let summary = walk(
            &data,
            None,
            std::slice::from_ref(&root),
            settings(),
            &stop,
            &mut |_| {},
        );
        assert_eq!(
            summary.recognised,
            vec![(root.join("TEST3"), root.join("TEST4"))]
        );
        assert_eq!(
            index_paths(&data),
            vec![root.join("TEST4/A.jpg"), root.join("TEST4/B.jpg")]
        );
    }
}
