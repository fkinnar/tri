//! Paires RAW+JPEG regroupées (issue n° 392, décidé le 09/11/2026).
//!
//! Le JPEG d'une paire n'est qu'un développement fait dans le boîtier : regroupée, la
//! paire est **une seule photo** dans la grille, le JPEG rangé avec les développements du
//! RAW (« Boîtier »). Désactivé par défaut ; un réglage global (`[files] group_pairs`) et
//! une **décision par dossier**, rangée avec ses données de tri ([`crate::store`] : elle
//! suit le dossier quand l'interrupteur « garder les données ici » est activé). La
//! décision d'un dossier l'emporte toujours sur le réglage global.
//!
//! Rien ne change sur le disque : le JPEG reste un fichier à part, ses notes restent
//! partagées avec le RAW, copier / déplacer / renommer emportent déjà la paire. Voir
//! `doc/en/pairs.md`.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::media::{MediaFile, MediaKind};

/// Réglage global `[files] group_pairs` : ce que devient un dossier sans décision.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Grouping {
    /// Demander à l'ouverture d'un dossier qui a des paires (séparées en attendant).
    #[default]
    Ask,
    /// Toujours séparées.
    Separate,
    /// Toujours regroupées.
    Grouped,
}

impl Grouping {
    /// Valeurs du réglage, dans l'ordre de la liste.
    pub const ALL: [Grouping; 3] = [Grouping::Ask, Grouping::Separate, Grouping::Grouped];

    /// Nom dans le fichier de configuration.
    pub fn id(self) -> &'static str {
        match self {
            Grouping::Ask => "ask",
            Grouping::Separate => "separate",
            Grouping::Grouped => "grouped",
        }
    }

    /// Vrai si un dossier sans décision est regroupé.
    pub fn groups(self) -> bool {
        self == Grouping::Grouped
    }
}

/// Clé de la décision dans les valeurs du dossier ([`crate::store::value`]).
const KEY: &str = "pairs_grouped";

/// Décision retenue pour `folder` : `Some(true)` regroupées, `Some(false)` séparées,
/// `None` aucune (le réglage global s'applique).
pub fn decided(data: Option<&Path>, folder: &Path) -> Option<bool> {
    crate::store::value(data, folder, KEY).map(|v| v >= 0.5)
}

/// Retient la décision `grouped` pour `folder` (`None` : l'oublie).
pub fn decide(data: Option<&Path>, folder: &Path, grouped: Option<bool>) -> io::Result<()> {
    let value = grouped.map(|g| if g { 1.0 } else { 0.0 });
    crate::store::set_value(data, folder, KEY, value)
}

/// Vrai si les paires de `folder` sont regroupées : sa décision, sinon `setting`.
pub fn grouped(setting: Grouping, data: Option<&Path>, folder: &Path) -> bool {
    decided(data, folder).unwrap_or(setting.groups())
}

/// Nom sans extension en minuscules, et dossier : clé d'une paire (comme
/// [`crate::notes::NoteBook::siblings`] : `IMG.JPG` va avec `IMG.RAF`).
fn pair_key(path: &Path) -> Option<(PathBuf, String)> {
    let stem = path.file_stem()?.to_string_lossy().to_lowercase();
    Some((path.parent()?.to_path_buf(), stem))
}

/// Les images plates de `files` qui ont un RAW de même nom dans le même dossier, avec ce
/// RAW : JPEG → RAW. Les développements ([`crate::media::is_development`]) ne comptent
/// pas.
pub fn camera_jpegs(files: &[MediaFile]) -> HashMap<PathBuf, PathBuf> {
    let photos = || {
        files
            .iter()
            .filter(|f| !crate::media::is_development(&f.path))
    };
    let raws: HashMap<(PathBuf, String), &PathBuf> = photos()
        .filter(|f| f.kind == MediaKind::Raw)
        .filter_map(|f| Some((pair_key(&f.path)?, &f.path)))
        .collect();
    photos()
        .filter(|f| f.kind == MediaKind::Flat)
        .filter_map(|f| {
            let raw = raws.get(&pair_key(&f.path)?)?;
            Some((f.path.clone(), (*raw).clone()))
        })
        .collect()
}

/// Sorte d'une image dans un dossier regroupé, pour le filtre à trois cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairKind {
    /// Un RAW sans JPEG (et ses développements).
    Raw,
    /// Une paire regroupée : le RAW, son JPEG et leurs développements.
    Pair,
    /// Un JPEG (ou autre image plate) sans RAW.
    Jpeg,
}

/// Cases du filtre d'un dossier regroupé (décidé le 09/11/2026) : RAW seuls, paires
/// RAW+JPEG, JPEG seuls ; combinables, une au moins cochée.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PairKinds {
    /// RAW sans JPEG.
    pub raw: bool,
    /// Paires regroupées.
    pub pairs: bool,
    /// JPEG sans RAW.
    pub jpeg: bool,
}

impl Default for PairKinds {
    fn default() -> Self {
        PairKinds {
            raw: true,
            pairs: true,
            jpeg: true,
        }
    }
}

impl PairKinds {
    /// Suite du clavier (`core.filter.cycle`) : tout, puis chaque case seule.
    pub const CYCLE: [PairKinds; 4] = [
        PairKinds {
            raw: true,
            pairs: true,
            jpeg: true,
        },
        PairKinds {
            raw: true,
            pairs: false,
            jpeg: false,
        },
        PairKinds {
            raw: false,
            pairs: true,
            jpeg: false,
        },
        PairKinds {
            raw: false,
            pairs: false,
            jpeg: true,
        },
    ];

    /// Vrai si tout passe.
    pub fn is_all(self) -> bool {
        self.raw && self.pairs && self.jpeg
    }

    /// Vrai si les images de sorte `kind` passent.
    pub fn accepts(self, kind: PairKind) -> bool {
        match kind {
            PairKind::Raw => self.raw,
            PairKind::Pair => self.pairs,
            PairKind::Jpeg => self.jpeg,
        }
    }

    /// Les mêmes avec la case de `kind` mise à `on` ; refusé (inchangé) si plus rien ne
    /// serait coché.
    pub fn with(self, kind: PairKind, on: bool) -> PairKinds {
        let mut next = self;
        match kind {
            PairKind::Raw => next.raw = on,
            PairKind::Pair => next.pairs = on,
            PairKind::Jpeg => next.jpeg = on,
        }
        if next.raw || next.pairs || next.jpeg {
            next
        } else {
            self
        }
    }

    /// Suivant dans [`PairKinds::CYCLE`] (une combinaison hors de la suite : tout).
    pub fn next(self) -> PairKinds {
        let at = Self::CYCLE.iter().position(|&k| k == self);
        at.map_or(Self::CYCLE[0], |i| Self::CYCLE[(i + 1) % Self::CYCLE.len()])
    }
}

/// Filtre à trois cases d'un dossier regroupé : `camera` donne le RAW de chaque JPEG
/// regroupé ([`camera_jpegs`], restreint aux dossiers regroupés).
pub struct PairFilter<'a> {
    /// Cases cochées.
    pub kinds: PairKinds,
    /// JPEG regroupé → son RAW.
    pub camera: &'a HashMap<PathBuf, PathBuf>,
}

impl PairFilter<'_> {
    /// Sorte de `file` : celle de sa photo (un développement suit la sienne, le JPEG
    /// regroupé et ses développements suivent le RAW).
    pub fn kind(&self, file: &MediaFile, paired: &std::collections::HashSet<&Path>) -> PairKind {
        let source = crate::media::source(&file.path);
        let photo: &Path = self
            .camera
            .get(source.as_ref())
            .map_or(&source, |p| p.as_path());
        if paired.contains(photo) {
            PairKind::Pair
        } else if file.kind == MediaKind::Raw {
            PairKind::Raw
        } else {
            PairKind::Jpeg
        }
    }
}

impl crate::filter::Filter for PairFilter<'_> {
    fn apply(&self, groups: crate::filter::Groups) -> crate::filter::Groups {
        if self.kinds.is_all() {
            return groups;
        }
        let paired: std::collections::HashSet<&Path> =
            self.camera.values().map(|p| p.as_path()).collect();
        crate::filter::retain(groups, |f| self.kinds.accepts(self.kind(f, &paired)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, kind: MediaKind) -> MediaFile {
        MediaFile {
            path: PathBuf::from(path),
            kind,
        }
    }

    #[test]
    fn a_jpeg_pairs_with_the_raw_of_its_name_in_its_folder() {
        let files = [
            file("/a/IMG_1.RAF", MediaKind::Raw),
            file("/a/img_1.jpg", MediaKind::Flat),
            file("/a/IMG_2.JPG", MediaKind::Flat),
            file("/b/IMG_1.JPG", MediaKind::Flat),
            file("/a/IMG_3.NEF", MediaKind::Raw),
        ];
        let pairs = camera_jpegs(&files);
        assert_eq!(pairs.len(), 1, "{pairs:?}");
        assert_eq!(
            pairs.get(Path::new("/a/img_1.jpg")),
            Some(&PathBuf::from("/a/IMG_1.RAF"))
        );
    }

    #[test]
    fn the_three_boxes_sort_raws_pairs_and_jpegs() {
        use crate::filter::Filter;
        let files = vec![
            file("/a/A.RAF", MediaKind::Raw),
            file("/a/A.JPG", MediaKind::Flat),
            file("/a/B.RAF", MediaKind::Raw),
            file("/a/C.JPG", MediaKind::Flat),
        ];
        let camera = camera_jpegs(&files);
        let names = |kinds: PairKinds| -> Vec<String> {
            PairFilter {
                kinds,
                camera: &camera,
            }
            .apply(vec![files.clone()])
            .concat()
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
        };
        assert_eq!(names(PairKinds::default()).len(), 4);
        assert_eq!(names(PairKinds::CYCLE[1]), ["B.RAF"], "RAW seuls");
        assert_eq!(names(PairKinds::CYCLE[2]), ["A.RAF", "A.JPG"], "paires");
        assert_eq!(names(PairKinds::CYCLE[3]), ["C.JPG"], "JPEG seuls");
        let none = PairKinds::CYCLE[1].with(PairKind::Raw, false);
        assert_eq!(none, PairKinds::CYCLE[1], "une case au moins");
        assert_eq!(PairKinds::CYCLE[3].next(), PairKinds::default());
    }

    #[test]
    fn a_folder_decision_wins_over_the_setting() {
        let data = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let (data, folder) = (Some(data.path()), folder.path());
        assert_eq!(decided(data, folder), None);
        assert!(!grouped(Grouping::Ask, data, folder));
        assert!(grouped(Grouping::Grouped, data, folder));
        decide(data, folder, Some(true)).unwrap();
        assert_eq!(decided(data, folder), Some(true));
        assert!(grouped(Grouping::Separate, data, folder));
        decide(data, folder, Some(false)).unwrap();
        assert!(!grouped(Grouping::Grouped, data, folder));
        decide(data, folder, None).unwrap();
        assert_eq!(decided(data, folder), None, "oubliée");
    }
}
