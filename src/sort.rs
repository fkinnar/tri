//! Ordre des images d'un dossier (22/10/2026) : par nom (l'ordre du boîtier, celui de
//! [`crate::media::scan`]), par date de prise de vue (plusieurs boîtiers dans une séance,
//! numérotation repartie à 0001, fichiers renommés) ou par date de modification, croissant
//! ou décroissant. Voir `doc/en/sort.md`.
//!
//! Les deux fichiers d'une paire RAW+JPEG (même nom sans extension) restent côte à côte,
//! le RAW d'abord, dans les deux sens : la paire est classée d'après le premier des deux.
//! Une image dont la clé est inconnue (date de prise de vue pas encore lue, ou absente)
//! vient après les autres, dans l'ordre des noms.
//!
//! Pas (encore) de tri par note ou par couleur : les piles et le jugement de netteté
//! s'appuient sur l'ordre de prise de vue (voisines, rafales).

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::media::MediaFile;

/// Clé de tri.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortKey {
    /// Nom de fichier (chemin), l'ordre du boîtier.
    #[default]
    Name,
    /// Date de prise de vue (EXIF).
    Captured,
    /// Date de modification du fichier.
    Modified,
}

impl SortKey {
    /// Toutes les clés, dans l'ordre où on les fait défiler.
    pub const ALL: [SortKey; 3] = [SortKey::Name, SortKey::Captured, SortKey::Modified];

    /// Clé suivante (après la dernière, la première).
    pub fn next(self) -> SortKey {
        let i = SortKey::ALL.iter().position(|k| *k == self).unwrap_or(0);
        SortKey::ALL[(i + 1) % SortKey::ALL.len()]
    }
}

/// Ordre : clé et sens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Order {
    /// Clé de tri.
    pub key: SortKey,
    /// Du plus grand au plus petit (Z → A, plus récent d'abord).
    pub descending: bool,
}

/// Date de modification de `path`, en secondes depuis 1970 (`None` si illisible).
pub fn modified(path: &Path) -> Option<i64> {
    let time = std::fs::metadata(crate::media::source(path))
        .ok()?
        .modified()
        .ok()?;
    let secs = match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    };
    Some(secs)
}

/// Trie `files` selon `order`. `captured` : heure de prise de vue connue d'une image
/// (secondes, origine quelconque mais la même pour toutes), pour [`SortKey::Captured`] ;
/// les dates de modification sont lues sur le disque.
pub fn sort(files: &mut [MediaFile], order: Order, captured: impl Fn(&Path) -> Option<i64>) {
    // D'abord l'ordre des noms (paires : RAW puis JPEG), qui départage aussi.
    files.sort_by(crate::media::compare);
    if order.key == SortKey::Name {
        if order.descending {
            reverse_pairs(files);
        }
        return;
    }
    // Clé d'une paire : celle de son premier fichier (le RAW).
    let mut keys: HashMap<PathBuf, Option<i64>> = HashMap::new();
    for file in files.iter() {
        let stem = file.path.with_extension("");
        keys.entry(stem).or_insert_with(|| match order.key {
            SortKey::Captured => captured(&file.path),
            SortKey::Modified => modified(&file.path),
            SortKey::Name => None,
        });
    }
    let rank: HashMap<PathBuf, usize> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.path.clone(), i))
        .collect();
    let key = |f: &MediaFile| keys[&f.path.with_extension("")];
    files.sort_by(|a, b| {
        let by_key = match (key(a), key(b)) {
            (Some(x), Some(y)) if order.descending => y.cmp(&x),
            (Some(x), Some(y)) => x.cmp(&y),
            // Inconnue : après les autres, dans les deux sens.
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        // À clé égale : l'ordre des noms (paire gardée ensemble, RAW d'abord).
        by_key.then_with(|| {
            a.path
                .with_extension("")
                .cmp(&b.path.with_extension(""))
                .then(rank[&a.path].cmp(&rank[&b.path]))
        })
    });
}

/// Inverse l'ordre des paires en gardant, dans chacune, le RAW avant le JPEG.
fn reverse_pairs(files: &mut [MediaFile]) {
    files.sort_by(|a, b| {
        b.path
            .with_extension("")
            .cmp(&a.path.with_extension(""))
            .then_with(|| crate::media::compare(a, b))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::MediaKind;

    fn file(name: &str) -> MediaFile {
        let kind = if name.ends_with(".RAF") {
            MediaKind::Raw
        } else {
            MediaKind::Flat
        };
        MediaFile {
            path: PathBuf::from(name),
            kind,
        }
    }

    fn names(files: &[MediaFile]) -> Vec<&str> {
        files.iter().map(|f| f.path.to_str().unwrap()).collect()
    }

    #[test]
    fn pairs_stay_together_and_unknown_dates_come_last() {
        let mut files = vec![
            file("B.JPG"),
            file("A.RAF"),
            file("C.RAF"),
            file("B.RAF"),
            file("A.JPG"),
            file("D.JPG"),
        ];
        let by_name = Order::default();
        sort(&mut files, by_name, |_| None);
        assert_eq!(
            names(&files),
            ["A.RAF", "A.JPG", "B.RAF", "B.JPG", "C.RAF", "D.JPG"]
        );
        let descending = Order {
            descending: true,
            ..by_name
        };
        sort(&mut files, descending, |_| None);
        assert_eq!(
            names(&files),
            ["D.JPG", "C.RAF", "B.RAF", "B.JPG", "A.RAF", "A.JPG"]
        );
        // Deux boîtiers : C pris avant A ; D sans date, en dernier.
        let times: HashMap<&str, i64> = [
            ("C.RAF", 10),
            ("A.RAF", 20),
            ("A.JPG", 20),
            ("B.RAF", 30),
            ("B.JPG", 31),
        ]
        .into();
        let captured = |p: &Path| times.get(p.to_str().unwrap()).copied();
        let by_date = Order {
            key: SortKey::Captured,
            descending: false,
        };
        sort(&mut files, by_date, captured);
        assert_eq!(
            names(&files),
            ["C.RAF", "A.RAF", "A.JPG", "B.RAF", "B.JPG", "D.JPG"]
        );
        sort(
            &mut files,
            Order {
                descending: true,
                ..by_date
            },
            captured,
        );
        assert_eq!(
            names(&files),
            ["B.RAF", "B.JPG", "A.RAF", "A.JPG", "C.RAF", "D.JPG"],
            "plus récent d'abord ; la paire suit son RAW ; sans date toujours à la fin"
        );
    }

    #[test]
    fn modification_dates_are_read_from_the_disk() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("Z.JPG");
        let new = dir.path().join("A.JPG");
        std::fs::write(&old, b"x").unwrap();
        std::fs::write(&new, b"x").unwrap();
        let earlier = SystemTime::now() - std::time::Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(earlier)
            .unwrap();
        let mut files = vec![
            MediaFile {
                path: new.clone(),
                kind: MediaKind::Flat,
            },
            MediaFile {
                path: old.clone(),
                kind: MediaKind::Flat,
            },
        ];
        let order = Order {
            key: SortKey::Modified,
            descending: false,
        };
        sort(&mut files, order, |_| None);
        assert_eq!(files[0].path, old, "le plus ancien d'abord");
        assert_eq!(SortKey::Name.next(), SortKey::Captured);
        assert_eq!(SortKey::Modified.next(), SortKey::Name);
    }
}
