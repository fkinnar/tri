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
