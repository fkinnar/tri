//! Un dossier ouvert : ses images, leurs notes, et la liste à afficher selon les filtres.
//!
//! C'est ce qu'une interface (graphique, TUI) ou la ligne de commande manipule quand on
//! « ouvre un dossier » : parcours ([`crate::media::scan`]), notes
//! ([`crate::notes::NoteBook`]), puis filtres par type et par notes, combinés ici une fois
//! pour toutes. Voir `doc/folder.md`.

use std::io;
use std::path::{Path, PathBuf};

use crate::filter::{Filter, Groups, KindFilter, ungrouped};
use crate::media::{self, MediaFile};
use crate::notes::{NoteBook, NotesRule};
use crate::xmp::Convention;

/// Dossier ouvert.
#[derive(Debug, Default)]
pub struct Folder {
    /// Dossier, en chemin absolu (vide si aucun dossier n'est ouvert).
    pub dir: PathBuf,
    /// Images du dossier, dans l'ordre de [`media::scan`].
    pub files: Vec<MediaFile>,
    /// Notes de ces images.
    pub notes: NoteBook,
}

impl Folder {
    /// Ouvre `dir` : parcourt ses images sur `depth` niveaux (1 : le dossier seul) et lit
    /// leurs notes ; `convention` : nom d'un sidecar créé par une note. Le chemin est
    /// rendu absolu (sans résoudre les liens). Erreur si `dir` n'est pas un dossier
    /// lisible.
    pub fn open(dir: &Path, depth: usize, convention: Convention) -> io::Result<Folder> {
        let dir = std::path::absolute(dir)?;
        let files = media::scan(&dir, depth)?;
        let notes = NoteBook::load_files(&files, convention);
        Ok(Folder { dir, files, notes })
    }

    /// Vrai si aucun dossier n'est ouvert.
    pub fn is_none(&self) -> bool {
        self.dir.as_os_str().is_empty()
    }

    /// Images à afficher : filtre par type, puis filtre sur les notes.
    pub fn filtered(&self, kind: KindFilter, rule: &NotesRule) -> Groups {
        rule.filter(&self.notes)
            .apply(kind.apply(ungrouped(self.files.clone())))
    }
}

/// Mémorise `dir` comme dernier dossier ouvert (fichier `last-folder` du dossier de
/// données `data`, voir `EngineSettings::data_dir`), pour le rouvrir au prochain
/// lancement.
pub fn remember(data: &Path, dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(data)?;
    std::fs::write(data.join("last-folder"), path_bytes(dir))
}

/// Dernier dossier ouvert ([`remember`]) dans le dossier de données `data`, s'il existe
/// toujours.
pub fn last(data: &Path) -> Option<PathBuf> {
    let bytes = std::fs::read(data.join("last-folder")).ok()?;
    let path = bytes_path(bytes)?;
    path.is_dir().then_some(path)
}

/// Octets d'un chemin : bruts sous Unix (les noms non UTF-8 survivent), UTF-8 ailleurs.
#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().into_owned().into_bytes()
}

#[cfg(unix)]
fn bytes_path(bytes: Vec<u8>) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

#[cfg(not(unix))]
fn bytes_path(bytes: Vec<u8>) -> Option<PathBuf> {
    String::from_utf8(bytes).ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_folder_is_remembered_if_it_still_exists() {
        let data = tempfile::tempdir().unwrap();
        let photos = tempfile::tempdir().unwrap();
        assert_eq!(last(data.path()), None);
        remember(data.path(), photos.path()).unwrap();
        assert_eq!(last(data.path()).as_deref(), Some(photos.path()));
        drop(photos);
        assert_eq!(last(data.path()), None, "dossier disparu : rien");
    }

    #[test]
    fn default_is_no_folder() {
        let folder = Folder::default();
        assert!(folder.is_none());
        assert!(
            folder
                .filtered(KindFilter::All, &NotesRule::default())
                .is_empty()
        );
    }
}
