//! Ordre des images d'un dossier (22/10/2026) : par nom (l'ordre du boîtier, celui de
//! [`crate::media::scan`]), par date de prise de vue (plusieurs boîtiers dans une séance,
//! numérotation repartie à 0001, fichiers renommés), par date de modification, ou, depuis
//! le 31/10/2026, par note : étoiles, couleur ou drapeau ; croissant ou décroissant. Voir
//! `doc/en/sort.md`.
//!
//! Les deux fichiers d'une paire RAW+JPEG (même nom sans extension) restent côte à côte,
//! le RAW d'abord, dans les deux sens : la paire est classée d'après le premier des deux.
//! Une image dont la clé est inconnue (date de prise de vue pas encore lue, ou absente ;
//! aucune couleur) vient après les autres, dans l'ordre des noms.
//!
//! Les piles et le jugement de netteté s'appuient sur les voisines : sous un tri par note,
//! les piles se forment sur l'ordre affiché (une rafale notée pareil reste contiguë, à
//! égalité l'ordre des noms départage) ; le jugement de netteté, lui, se fait dans le
//! moteur sur l'ordre de prise de vue.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::media::MediaFile;
use crate::xmp::{Color, Flag, Notes};

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
    /// Étoiles (une photo rejetée sous 0 étoile, comme `xmp:Rating = -1`).
    Stars,
    /// Couleur : dans l'ordre de `[sort] color_order`, qui peut ne citer que quelques
    /// couleurs (usage de l'utilisateur : rouge / jaune / vert ordonnées, bleu et violet
    /// pour autre chose) ; une image est classée d'après la mieux placée de ses couleurs ;
    /// sans couleur, ou aucune dans la liste, à la fin.
    Color,
    /// Drapeau : acceptée, sans drapeau, rejetée.
    Flag,
}

/// Réglages du tri (section `[sort]` du fichier de configuration).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SortSettings {
    /// Ordre des couleurs pour [`SortKey::Color`] (décidé le 31/10/2026 : réglable, chacun
    /// donne son sens à ses couleurs).
    pub color_order: Vec<Color>,
}

impl Default for SortSettings {
    fn default() -> Self {
        Self {
            color_order: Color::ALL.to_vec(),
        }
    }
}

impl SortKey {
    /// Toutes les clés, dans l'ordre où on les fait défiler.
    pub const ALL: [SortKey; 6] = [
        SortKey::Name,
        SortKey::Captured,
        SortKey::Modified,
        SortKey::Stars,
        SortKey::Color,
        SortKey::Flag,
    ];

    /// Clé qui dépend des notes (à retrier quand une note change).
    pub fn by_notes(self) -> bool {
        matches!(self, SortKey::Stars | SortKey::Color | SortKey::Flag)
    }

    /// Valeur de tri des notes `n` pour cette clé (`colors` : l'ordre des couleurs) ;
    /// `None` pour une clé qui n'en dépend pas, ou une image sans couleur connue.
    pub fn of_notes(self, n: &Notes, colors: &[Color]) -> Option<i64> {
        match self {
            SortKey::Stars => Some(if n.flag == Flag::Rejected {
                -1
            } else {
                i64::from(n.stars)
            }),
            SortKey::Color => n
                .colors
                .iter()
                .filter_map(|c| colors.iter().position(|k| k == c))
                .min()
                .map(|i| i as i64),
            SortKey::Flag => Some(match n.flag {
                Flag::Accepted => 0,
                Flag::None => 1,
                Flag::Rejected => 2,
            }),
            SortKey::Name | SortKey::Captured | SortKey::Modified => None,
        }
    }

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
/// les dates de modification sont lues sur le disque ; `notes` : les notes d'une image,
/// pour les clés par note (une paire est classée d'après son RAW, un développement
/// d'après sa photo) ; `settings` : l'ordre des couleurs.
pub fn sort(
    files: &mut [MediaFile],
    order: Order,
    captured: impl Fn(&Path) -> Option<i64>,
    notes: impl Fn(&Path) -> Notes,
    settings: &SortSettings,
) {
    // D'abord l'ordre des noms (paires : RAW puis JPEG), qui départage aussi.
    files.sort_by(crate::media::compare);
    if order.key == SortKey::Name {
        if order.descending {
            reverse_pairs(files);
        }
        return;
    }
    // Clé d'une paire : celle de son premier fichier (le RAW).
    // Un développement a la clé de sa photo (même prise de vue), comme l'autre fichier de
    // la paire.
    let stem = |f: &MediaFile| crate::media::source(&f.path).with_extension("");
    let mut keys: HashMap<PathBuf, Option<i64>> = HashMap::new();
    for file in files.iter() {
        let stem = stem(file);
        keys.entry(stem).or_insert_with(|| match order.key {
            SortKey::Captured => captured(&file.path),
            SortKey::Modified => modified(&file.path),
            SortKey::Name => None,
            key => key.of_notes(&notes(&file.path), &settings.color_order),
        });
    }
    let rank: HashMap<PathBuf, usize> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.path.clone(), i))
        .collect();
    let key = |f: &MediaFile| keys[&stem(f)];
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
            stem(a)
                .cmp(&stem(b))
                .then(rank[&a.path].cmp(&rank[&b.path]))
        })
    });
}

/// Inverse l'ordre des paires en gardant, dans chacune, le RAW avant le JPEG.
fn reverse_pairs(files: &mut [MediaFile]) {
    let stem = |f: &MediaFile| crate::media::source(&f.path).with_extension("");
    files.sort_by(|a, b| {
        stem(b)
            .cmp(&stem(a))
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
    fn developments_follow_their_photo_in_both_directions() {
        use crate::develop::Engine;
        use crate::developments::{Copy, Development, set_for};
        let photo = PathBuf::from("/o/IMG.RAF");
        let dt = Development {
            engine: Engine::Darktable,
            copy: Copy::Main,
            photo: photo.clone(),
            sidecar: PathBuf::from("/o/IMG.RAF.xmp"),
        };
        set_for(&photo, std::slice::from_ref(&dt));
        let mut files = vec![
            file("/o/IMG2.RAF"),
            file("/o/IMG.JPG"),
            MediaFile {
                path: dt.path(),
                kind: MediaKind::Raw,
            },
            file("/o/IMG.RAF"),
        ];
        // Le chemin virtuel suit le séparateur du système (« /o\\IMG@dt.RAF » sous Windows).
        let dt_path = dt.path();
        let dt_name = dt_path.to_str().unwrap();
        let s = SortSettings::default();
        sort(&mut files, Order::default(), |_| None, |_| Notes::EMPTY, &s);
        // Le nom propre « IMG@dt » trierait après « IMG2 » : c'est la photo qui compte.
        assert_eq!(
            names(&files),
            ["/o/IMG.RAF", dt_name, "/o/IMG.JPG", "/o/IMG2.RAF"]
        );
        let descending = Order {
            descending: true,
            ..Order::default()
        };
        sort(&mut files, descending, |_| None, |_| Notes::EMPTY, &s);
        assert_eq!(
            names(&files),
            ["/o/IMG2.RAF", "/o/IMG.RAF", dt_name, "/o/IMG.JPG"]
        );
        // Par date : celle de la photo, inconnue pour le développement lui-même.
        let by_date = Order {
            key: SortKey::Captured,
            descending: true,
        };
        let captured = |p: &Path| match p.to_str().unwrap() {
            "/o/IMG.RAF" => Some(10),
            "/o/IMG2.RAF" => Some(5),
            _ => None,
        };
        sort(&mut files, by_date, captured, |_| Notes::EMPTY, &s);
        assert_eq!(
            names(&files),
            ["/o/IMG.RAF", dt_name, "/o/IMG.JPG", "/o/IMG2.RAF"]
        );
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
        let s = SortSettings::default();
        sort(&mut files, by_name, |_| None, |_| Notes::EMPTY, &s);
        assert_eq!(
            names(&files),
            ["A.RAF", "A.JPG", "B.RAF", "B.JPG", "C.RAF", "D.JPG"]
        );
        let descending = Order {
            descending: true,
            ..by_name
        };
        sort(&mut files, descending, |_| None, |_| Notes::EMPTY, &s);
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
        sort(
            &mut files,
            by_date,
            captured,
            |_| Notes::EMPTY,
            &SortSettings::default(),
        );
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
            |_| Notes::EMPTY,
            &SortSettings::default(),
        );
        assert_eq!(
            names(&files),
            ["B.RAF", "B.JPG", "A.RAF", "A.JPG", "C.RAF", "D.JPG"],
            "plus récent d'abord ; la paire suit son RAW ; sans date toujours à la fin"
        );
    }

    #[test]
    fn ratings_sort_by_stars_colour_and_flag() {
        // 31/10/2026 : la paire d'après son RAW (B.JPG n'a pas de note propre ici), une
        // rejetée sous 0 étoile, sans couleur à la fin dans les deux sens.
        let mut files = vec![
            file("A.RAF"),
            file("B.RAF"),
            file("B.JPG"),
            file("C.RAF"),
            file("D.JPG"),
        ];
        let note = |stars: u8, flag: Flag, colors: &[Color]| Notes {
            stars,
            flag,
            colors: colors.to_vec(),
        };
        let all: HashMap<&str, Notes> = [
            ("A.RAF", note(1, Flag::None, &[Color::Blue])),
            ("B.RAF", note(3, Flag::Accepted, &[])),
            ("C.RAF", note(5, Flag::Rejected, &[Color::Red])),
            ("D.JPG", note(0, Flag::None, &[Color::Green])),
        ]
        .into();
        let notes = |p: &Path| all.get(p.to_str().unwrap()).cloned().unwrap_or_default();
        let by = |key: SortKey, descending: bool| Order { key, descending };
        let s = SortSettings::default();
        sort(&mut files, by(SortKey::Stars, false), |_| None, notes, &s);
        assert_eq!(
            names(&files),
            ["C.RAF", "D.JPG", "A.RAF", "B.RAF", "B.JPG"],
            "rejetée (-1), 0, 1, 3 ; la paire ensemble"
        );
        sort(&mut files, by(SortKey::Stars, true), |_| None, notes, &s);
        assert_eq!(names(&files), ["B.RAF", "B.JPG", "A.RAF", "D.JPG", "C.RAF"]);
        sort(&mut files, by(SortKey::Color, false), |_| None, notes, &s);
        assert_eq!(
            names(&files),
            ["C.RAF", "D.JPG", "A.RAF", "B.RAF", "B.JPG"],
            "rouge, vert, bleu ; sans couleur à la fin"
        );
        sort(&mut files, by(SortKey::Color, true), |_| None, notes, &s);
        assert_eq!(
            names(&files),
            ["A.RAF", "D.JPG", "C.RAF", "B.RAF", "B.JPG"],
            "sans couleur toujours à la fin"
        );
        sort(&mut files, by(SortKey::Flag, false), |_| None, notes, &s);
        assert_eq!(
            names(&files),
            ["B.RAF", "B.JPG", "A.RAF", "D.JPG", "C.RAF"],
            "acceptée, sans drapeau (ordre des noms), rejetée"
        );
        // Ordre des couleurs réglable et partiel (31/10/2026) : bleu avant rouge ; le vert,
        // hors liste, à la fin avec les sans couleur, dans l'ordre des noms.
        let mine = SortSettings {
            color_order: vec![Color::Blue, Color::Red],
        };
        sort(
            &mut files,
            by(SortKey::Color, false),
            |_| None,
            notes,
            &mine,
        );
        assert_eq!(names(&files), ["A.RAF", "C.RAF", "B.RAF", "B.JPG", "D.JPG"]);
        // Plusieurs couleurs : la mieux placée compte (D : vert et rouge → rouge).
        let two = |p: &Path| match p.to_str().unwrap() {
            "D.JPG" => note(0, Flag::None, &[Color::Green, Color::Red]),
            other => notes(Path::new(other)),
        };
        sort(&mut files, by(SortKey::Color, false), |_| None, two, &mine);
        assert_eq!(names(&files), ["A.RAF", "C.RAF", "D.JPG", "B.RAF", "B.JPG"]);
        assert!(SortKey::Stars.by_notes() && !SortKey::Captured.by_notes());
        assert_eq!(SortKey::ALL.len(), 6);
        assert_eq!(SortSettings::default().color_order, Color::ALL);
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
        sort(
            &mut files,
            order,
            |_| None,
            |_| Notes::EMPTY,
            &SortSettings::default(),
        );
        assert_eq!(files[0].path, old, "le plus ancien d'abord");
        assert_eq!(SortKey::Name.next(), SortKey::Captured);
        assert_eq!(SortKey::Modified.next(), SortKey::Stars);
        assert_eq!(SortKey::Flag.next(), SortKey::Name);
    }
}
