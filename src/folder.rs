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

#[cfg(test)]
mod tests {
    use super::*;

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
