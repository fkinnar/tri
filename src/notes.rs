//! Notes de tri d'un ensemble d'images : chargement depuis leurs sidecars XMP, modification,
//! et filtres (étoiles, drapeau, couleurs).
//!
//! [`NoteBook`] garde en mémoire les notes de toutes les images affichées : filtrer reste
//! instantané, seule une modification touche le disque. Le format des sidecars est l'affaire
//! du module [`crate::xmp`]. Voir `doc/en/notes.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::filter::{Filter, Groups, retain};
use crate::media::MediaFile;
use crate::xmp::{self, Color, Convention, Flag, Notes, SidecarIndex, XmpError};

/// Notes d'une image inconnue (une `const` contenant un `Vec` ne se prête pas en `&'static`).
static EMPTY: Notes = Notes::EMPTY;

/// Notes d'une image et sidecar d'où elles viennent.
#[derive(Clone, Debug)]
struct Entry {
    notes: Notes,
    /// Sidecar lu et à écrire (le plus récent, sinon celui de la convention).
    sidecar: PathBuf,
    /// Sidecar illisible ou mal formé : notes vides, et jamais réécrit.
    error: Option<String>,
}

/// Notes des images d'un ou plusieurs dossiers.
#[derive(Debug, Default)]
pub struct NoteBook {
    convention: Convention,
    entries: HashMap<PathBuf, Entry>,
    /// Images de chaque dossier, pour retrouver celles qui partagent un sidecar (convention
    /// Adobe : `IMG.xmp` pour `IMG.RAF` et `IMG.JPG`).
    by_dir: HashMap<PathBuf, Vec<PathBuf>>,
}

fn dir_of(image: &Path) -> PathBuf {
    image.parent().unwrap_or(Path::new(".")).to_path_buf()
}

impl NoteBook {
    /// Lit les notes de `images`. Chaque dossier n'est listé qu'une fois ; un sidecar
    /// illisible donne des notes vides et une erreur ([`NoteBook::error`]), sans arrêter le
    /// chargement. `convention` : nom d'un sidecar créé par une écriture.
    pub fn load<'a>(
        images: impl IntoIterator<Item = &'a Path>,
        convention: Convention,
    ) -> NoteBook {
        let mut book = NoteBook {
            convention,
            ..NoteBook::default()
        };
        for image in images {
            book.by_dir
                .entry(dir_of(image))
                .or_default()
                .push(image.to_path_buf());
        }
        let dirs: Vec<PathBuf> = book.by_dir.keys().cloned().collect();
        for dir in dirs {
            book.reload_dir(&dir);
        }
        book
    }

    /// Lit les notes des images de `files` (sortie de [`crate::media::scan`]).
    pub fn load_files(files: &[MediaFile], convention: Convention) -> NoteBook {
        NoteBook::load(files.iter().map(|f| f.path.as_path()), convention)
    }

    /// Relit le dossier `dir` (sidecars créés, supprimés, ou modifiés par un autre
    /// logiciel).
    fn reload_dir(&mut self, dir: &Path) {
        let index = SidecarIndex::new(dir);
        for image in self.by_dir.get(dir).into_iter().flatten() {
            let sidecar = index.sidecar_path(image, self.convention);
            let (notes, error) = match xmp::read_sidecar(&sidecar) {
                Ok(notes) => (notes, None),
                Err(e) => (Notes::default(), Some(e.to_string())),
            };
            self.entries.insert(
                image.clone(),
                Entry {
                    notes,
                    sidecar,
                    error,
                },
            );
        }
    }

    /// Relit tout depuis le disque.
    pub fn reload(&mut self) {
        let dirs: Vec<PathBuf> = self.by_dir.keys().cloned().collect();
        for dir in dirs {
            self.reload_dir(&dir);
        }
    }

    /// Relit les notes des images concernées par les sidecars `sidecars` (créés, modifiés
    /// ou supprimés par un autre logiciel, voir [`crate::watch`]) : `IMG.RAF.xmp` pour
    /// `IMG.RAF`, `IMG.xmp` pour `IMG.RAF` et `IMG.JPG`. Renvoie les images dont les notes
    /// (ou l'erreur de lecture) ont changé : rien quand c'est `tri` qui vient d'écrire.
    pub fn reload_sidecars(&mut self, sidecars: &[PathBuf]) -> Vec<PathBuf> {
        let mut by_dir: HashMap<PathBuf, Vec<String>> = HashMap::new();
        for sidecar in sidecars {
            if let Some(stem) = sidecar.file_stem() {
                by_dir
                    .entry(dir_of(sidecar))
                    .or_default()
                    .push(stem.to_string_lossy().to_lowercase());
            }
        }
        let mut changed = Vec::new();
        for (dir, stems) in by_dir {
            let Some(images) = self.by_dir.get(&dir).cloned() else {
                continue;
            };
            let index = SidecarIndex::new(&dir);
            let lower = |s: Option<&std::ffi::OsStr>| s.map(|s| s.to_string_lossy().to_lowercase());
            for image in images {
                let (name, stem) = (lower(image.file_name()), lower(image.file_stem()));
                if !stems
                    .iter()
                    .any(|s| Some(s) == name.as_ref() || Some(s) == stem.as_ref())
                {
                    continue;
                }
                let sidecar = index.sidecar_path(&image, self.convention);
                let (notes, error) = match xmp::read_sidecar(&sidecar) {
                    Ok(notes) => (notes, None),
                    Err(e) => (Notes::default(), Some(e.to_string())),
                };
                let old = self.entries.get(&image);
                if old.is_none_or(|e| e.notes != notes || e.error != error) {
                    changed.push(image.clone());
                }
                self.entries.insert(
                    image,
                    Entry {
                        notes,
                        sidecar,
                        error,
                    },
                );
            }
        }
        changed.sort();
        changed
    }

    /// Notes de `image` (vides si elle est inconnue ou sans sidecar).
    pub fn notes(&self, image: &Path) -> &Notes {
        self.entries.get(image).map(|e| &e.notes).unwrap_or(&EMPTY)
    }

    /// Erreur de lecture du sidecar de `image`, s'il est illisible ou mal formé.
    pub fn error(&self, image: &Path) -> Option<&str> {
        self.entries.get(image).and_then(|e| e.error.as_deref())
    }

    /// Sidecar de `image` : celui qui a été lu, ou celui qui sera créé.
    pub fn sidecar(&self, image: &Path) -> Option<&Path> {
        self.entries.get(image).map(|e| e.sidecar.as_path())
    }

    /// Nombre d'images suivies.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Vrai si aucune image n'est suivie.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Images du même nom que `image` dans son dossier, à l'extension près, sans tenir compte
    /// de la casse : `IMG.JPG` pour `IMG.RAF` (paire RAW+JPEG d'une carte SD).
    pub fn siblings(&self, image: &Path) -> Vec<PathBuf> {
        let stem = |p: &Path| p.file_stem().map(|s| s.to_string_lossy().to_lowercase());
        let wanted = stem(image);
        self.by_dir
            .get(&dir_of(image))
            .into_iter()
            .flatten()
            .filter(|p| p.as_path() != image && stem(p) == wanted)
            .cloned()
            .collect()
    }

    /// Écrit `notes` pour `image` seule, sans toucher à sa paire (annulation : chaque
    /// image de la paire retrouve ses propres notes d'avant). Relit ensuite le dossier.
    pub fn write_only(&mut self, image: &Path, notes: &Notes) -> Result<(), XmpError> {
        let dir = dir_of(image);
        if !self.entries.contains_key(image) {
            self.by_dir
                .entry(dir.clone())
                .or_default()
                .push(image.to_path_buf());
            self.reload_dir(&dir);
        }
        let entry = &self.entries[image];
        if let Some(error) = &entry.error {
            return Err(XmpError::Image {
                image: image.to_path_buf(),
                message: error.clone(),
            });
        }
        if entry.notes != *notes {
            xmp::write_sidecar(&entry.sidecar, notes).map_err(|e| XmpError::Image {
                image: image.to_path_buf(),
                message: e.to_string(),
            })?;
        }
        self.reload_dir(&dir);
        Ok(())
    }

    /// Modifie les notes de `image` avec `edit`, puis les écrit dans son sidecar. Les
    /// images de même nom (paire RAW+JPEG, voir [`NoteBook::siblings`]) reçoivent les mêmes
    /// notes : noter le RAF note aussi le JPG, quelle que soit la convention (choix du
    /// 17/10/2026). Renvoie les images dont les notes ont changé, à rafraîchir à l'écran.
    /// Une image inconnue est ajoutée.
    ///
    /// En cas d'erreur (sidecar mal formé, disque en lecture seule…), rien n'est écrit pour
    /// `image` ; pour une image de la paire, les autres sont écrites et l'erreur est
    /// renvoyée. La mémoire est toujours relue du disque : ce qui est affiché reste ce qui
    /// y est.
    pub fn edit(
        &mut self,
        image: &Path,
        edit: impl FnOnce(&mut Notes),
    ) -> Result<Vec<PathBuf>, XmpError> {
        let dir = dir_of(image);
        if !self.entries.contains_key(image) {
            self.by_dir
                .entry(dir.clone())
                .or_default()
                .push(image.to_path_buf());
            self.reload_dir(&dir);
        }
        let entry = &self.entries[image];
        if let Some(error) = &entry.error {
            return Err(XmpError::Image {
                image: image.to_path_buf(),
                message: error.clone(),
            });
        }
        let mut notes = entry.notes.clone();
        edit(&mut notes);
        let mut targets = vec![image.to_path_buf()];
        targets.extend(self.siblings(image));
        let mut written: Vec<PathBuf> = Vec::new();
        let mut failure = None;
        for target in &targets {
            let entry = &self.entries[target];
            // Sidecar partagé (convention Adobe) : déjà écrit pour l'autre image.
            if entry.notes == notes || written.contains(&entry.sidecar) {
                continue;
            }
            let result = match &entry.error {
                Some(error) => Err(error.clone()),
                None => xmp::write_sidecar(&entry.sidecar, &notes)
                    .map(|_| ())
                    .map_err(|e| e.to_string()),
            };
            match result {
                Ok(()) => written.push(entry.sidecar.clone()),
                Err(message) => {
                    failure.get_or_insert(XmpError::Image {
                        image: target.clone(),
                        message,
                    });
                }
            }
        }
        if written.is_empty() && failure.is_none() {
            return Ok(Vec::new());
        }
        // Des sidecars ont pu être créés, et d'autres images les partager : on relit le
        // dossier (une liste de fichiers, puis seulement les sidecars).
        let before: Vec<(PathBuf, Notes)> = self.by_dir[&dir]
            .iter()
            .map(|p| (p.clone(), self.notes(p).clone()))
            .collect();
        self.reload_dir(&dir);
        if let Some(e) = failure {
            return Err(e);
        }
        Ok(before
            .into_iter()
            .filter(|(p, old)| self.notes(p) != old)
            .map(|(p, _)| p)
            .collect())
    }
}

/// Règle sur les étoiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StarsRule {
    /// Au moins n étoiles.
    AtLeast(u8),
    /// Exactement n étoiles (0 : non notées).
    Exactly(u8),
    /// Au plus n étoiles.
    AtMost(u8),
}

impl StarsRule {
    /// Vrai si `stars` vérifie la règle.
    pub fn accepts(self, stars: u8) -> bool {
        match self {
            StarsRule::AtLeast(n) => stars >= n,
            StarsRule::Exactly(n) => stars == n,
            StarsRule::AtMost(n) => stars <= n,
        }
    }
}

/// Règle sur les couleurs : une photo passe si elle porte l'une des couleurs `any_of`, ou si
/// elle n'en porte aucune et que `none` est vrai.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColorRule {
    /// Couleurs acceptées.
    pub any_of: Vec<Color>,
    /// Accepter les photos sans couleur.
    pub none: bool,
}

impl ColorRule {
    /// Vrai si des couleurs `colors` vérifient la règle.
    pub fn accepts(&self, colors: &[Color]) -> bool {
        if colors.is_empty() {
            self.none
        } else {
            colors.iter().any(|c| self.any_of.contains(c))
        }
    }
}

/// Critères de filtrage sur les notes ; `None` : critère ignoré. Les critères se
/// combinent (et).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NotesRule {
    /// Étoiles.
    pub stars: Option<StarsRule>,
    /// Drapeaux acceptés (ex. `[None, Accepted]` : masquer les rejetées).
    pub flags: Option<Vec<Flag>>,
    /// Couleurs.
    pub colors: Option<ColorRule>,
}

impl NotesRule {
    /// Vrai si aucun critère n'est actif.
    pub fn is_empty(&self) -> bool {
        *self == NotesRule::default()
    }

    /// Vrai si `notes` vérifie tous les critères.
    pub fn accepts(&self, notes: &Notes) -> bool {
        self.stars.is_none_or(|r| r.accepts(notes.stars))
            && self.flags.as_ref().is_none_or(|f| f.contains(&notes.flag))
            && self
                .colors
                .as_ref()
                .is_none_or(|c| c.accepts(&notes.colors))
    }

    /// Le filtre correspondant, qui lit les notes dans `book`.
    pub fn filter<'a>(&'a self, book: &'a NoteBook) -> NotesFilter<'a> {
        NotesFilter { rule: self, book }
    }
}

/// Filtre sur les notes : un [`NotesRule`] appliqué avec les notes d'un [`NoteBook`].
/// Implémente le même trait [`Filter`] que les autres filtres.
#[derive(Clone, Copy, Debug)]
pub struct NotesFilter<'a> {
    rule: &'a NotesRule,
    book: &'a NoteBook,
}

impl Filter for NotesFilter<'_> {
    fn apply(&self, groups: Groups) -> Groups {
        if self.rule.is_empty() {
            return groups;
        }
        retain(groups, |f| self.rule.accepts(self.book.notes(&f.path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notes(stars: u8, flag: Flag, colors: &[Color]) -> Notes {
        Notes {
            stars,
            flag,
            colors: colors.to_vec(),
        }
    }

    #[test]
    fn stars_rules() {
        assert!(StarsRule::AtLeast(3).accepts(3));
        assert!(!StarsRule::AtLeast(3).accepts(2));
        assert!(StarsRule::Exactly(0).accepts(0));
        assert!(!StarsRule::Exactly(0).accepts(1));
        assert!(StarsRule::AtMost(1).accepts(0));
        assert!(!StarsRule::AtMost(1).accepts(2));
    }

    #[test]
    fn color_rule_any_of_or_none() {
        let rule = ColorRule {
            any_of: vec![Color::Red, Color::Green],
            none: false,
        };
        assert!(rule.accepts(&[Color::Green, Color::Blue]));
        assert!(!rule.accepts(&[Color::Blue]));
        assert!(!rule.accepts(&[]));
        let uncolored = ColorRule {
            any_of: vec![],
            none: true,
        };
        assert!(uncolored.accepts(&[]));
        assert!(!uncolored.accepts(&[Color::Red]));
    }

    #[test]
    fn empty_rule_accepts_everything() {
        let rule = NotesRule::default();
        assert!(rule.is_empty());
        assert!(rule.accepts(&notes(0, Flag::Rejected, &[])));
    }

    #[test]
    fn criteria_combine() {
        let rule = NotesRule {
            stars: Some(StarsRule::AtLeast(2)),
            flags: Some(vec![Flag::None, Flag::Accepted]),
            colors: None,
        };
        assert!(rule.accepts(&notes(3, Flag::Accepted, &[])));
        assert!(!rule.accepts(&notes(3, Flag::Rejected, &[])), "rejetée");
        assert!(
            !rule.accepts(&notes(1, Flag::None, &[])),
            "pas assez d'étoiles"
        );
    }

    #[test]
    fn sidecars_changed_elsewhere_are_reread() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        for name in ["A.RAF", "A.JPG", "B.RAF"] {
            std::fs::write(d.join(name), b"x").unwrap();
        }
        let images = [d.join("A.RAF"), d.join("A.JPG"), d.join("B.RAF")];
        let mut book = NoteBook::load(images.iter().map(|p| p.as_path()), Convention::Darktable);
        // Un autre logiciel (convention Adobe) note la paire A : les deux images relues.
        let three = notes(3, Flag::None, &[]);
        xmp::write_sidecar(&d.join("A.xmp"), &three).unwrap();
        let changed = book.reload_sidecars(&[d.join("A.xmp")]);
        assert_eq!(changed, [d.join("A.JPG"), d.join("A.RAF")]);
        assert_eq!(book.notes(&d.join("A.RAF")).stars, 3);
        assert_eq!(book.notes(&d.join("B.RAF")).stars, 0, "pas concernée");
        // Relu sans changement (tri vient d'écrire) : rien à signaler.
        assert!(book.reload_sidecars(&[d.join("A.xmp")]).is_empty());
        // B.RAF.xmp (darktable) supprimé, puis créé.
        xmp::write_sidecar(&d.join("B.RAF.xmp"), &notes(5, Flag::None, &[])).unwrap();
        assert_eq!(
            book.reload_sidecars(&[d.join("B.RAF.xmp")]),
            [d.join("B.RAF")]
        );
        std::fs::remove_file(d.join("B.RAF.xmp")).unwrap();
        assert_eq!(
            book.reload_sidecars(&[d.join("B.RAF.xmp")]),
            [d.join("B.RAF")]
        );
        assert_eq!(book.notes(&d.join("B.RAF")).stars, 0);
    }

    #[test]
    fn unknown_image_has_empty_notes() {
        let book = NoteBook::default();
        assert_eq!(book.notes(Path::new("/nulle/part.RAF")), &Notes::EMPTY);
        assert!(book.error(Path::new("/nulle/part.RAF")).is_none());
        assert!(book.is_empty());
    }
}
