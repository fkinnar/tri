//! Notes de tri d'un ensemble d'images : chargement depuis leurs sidecars XMP, modification,
//! et filtres (étoiles, drapeau, couleurs).
//!
//! [`NoteBook`] garde en mémoire les notes de toutes les images affichées : filtrer reste
//! instantané, seule une modification touche le disque. Le format des sidecars est l'affaire
//! du module [`crate::xmp`]. Voir `doc/en/notes.md`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

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
    /// Pendant une lecture en fond ([`NoteBook::pending`]) : les images lues ou écrites
    /// depuis, plus fraîches que ce que rapportera la lecture ([`NoteBook::adopt`]).
    touched: Option<HashSet<PathBuf>>,
}

/// Notes de `images` (toutes dans `dir`), lues avec une seule liste du dossier ; un
/// sidecar n'est ouvert que s'il existe. `done` compte les images lues.
fn read_entries(
    dir: &Path,
    images: &[PathBuf],
    convention: Convention,
    done: Option<&AtomicUsize>,
) -> Vec<(PathBuf, Entry)> {
    let index = SidecarIndex::new(dir);
    images
        .iter()
        .map(|image| {
            let (notes, sidecar, error) = read_notes(image, &index, convention);
            if let Some(done) = done {
                done.fetch_add(1, Ordering::Relaxed);
            }
            (
                image.clone(),
                Entry {
                    notes,
                    sidecar,
                    error,
                },
            )
        })
        .collect()
}

/// Notes de `image`, le sidecar qui les porte (lu, ou à créer selon `convention`) et
/// l'erreur de lecture. Un développement ([`crate::developments`]) a les siennes dans
/// son sidecar : XMP de darktable, `.rrdata` de RapidRAW ([`crate::rrdata`]).
fn read_notes(
    image: &Path,
    index: &SidecarIndex,
    convention: Convention,
) -> (Notes, PathBuf, Option<String>) {
    let existing = match crate::developments::get(image) {
        Some(development) => Some(development.sidecar),
        None => index.existing_sidecar(image),
    };
    let (notes, error) = match &existing {
        None => (Notes::default(), None),
        Some(sidecar) => match read_sidecar(sidecar) {
            Ok(notes) => (notes, None),
            Err(e) => (Notes::default(), Some(e)),
        },
    };
    let sidecar = existing.unwrap_or_else(|| convention.sidecar(image));
    (notes, sidecar, error)
}

/// Vrai si `sidecar` est un `.rrdata` de RapidRAW.
fn is_rrdata(sidecar: &Path) -> bool {
    sidecar
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("rrdata"))
}

/// Notes du sidecar `sidecar`, XMP ou RapidRAW.
fn read_sidecar(sidecar: &Path) -> Result<Notes, String> {
    if is_rrdata(sidecar) {
        crate::rrdata::read(sidecar).map_err(|e| e.to_string())
    } else {
        xmp::read_sidecar(sidecar).map_err(|e| e.to_string())
    }
}

/// Écrit `notes` dans `sidecar`, XMP ou RapidRAW (le reste du fichier gardé).
fn write_sidecar(sidecar: &Path, notes: &Notes) -> Result<(), String> {
    if is_rrdata(sidecar) {
        crate::rrdata::write(sidecar, notes)
            .map(|_| ())
            .map_err(|e| e.to_string())
    } else {
        xmp::write_sidecar(sidecar, notes)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
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
        let mut book = NoteBook::pending(images, convention);
        book.touched = None;
        book.reload();
        book
    }

    /// Lit les notes des images de `files` (sortie de [`crate::media::scan`]).
    pub fn load_files(files: &[MediaFile], convention: Convention) -> NoteBook {
        NoteBook::load(files.iter().map(|f| f.path.as_path()), convention)
    }

    /// [`NoteBook::load`], en comptant les images lues dans `done` (lecture en fond avec
    /// progression, voir [`crate::folder::Loading`]).
    pub fn load_counting(
        files: &[MediaFile],
        convention: Convention,
        done: &AtomicUsize,
    ) -> NoteBook {
        let mut book = NoteBook::pending(files.iter().map(|f| f.path.as_path()), convention);
        book.touched = None;
        for (dir, images) in &book.by_dir {
            let entries = read_entries(dir, images, convention, Some(done));
            book.entries.extend(entries);
        }
        book
    }

    /// Carnet de `images` dont les notes ne sont pas encore lues (elles le sont en fond,
    /// puis reprises par [`NoteBook::adopt`]) : notes vides en attendant ; une image notée
    /// entre-temps est lue seule, à la demande.
    pub fn pending<'a>(
        images: impl IntoIterator<Item = &'a Path>,
        convention: Convention,
    ) -> NoteBook {
        let mut book = NoteBook {
            convention,
            touched: Some(HashSet::new()),
            ..NoteBook::default()
        };
        for image in images {
            book.by_dir
                .entry(dir_of(image))
                .or_default()
                .push(image.to_path_buf());
        }
        book
    }

    /// Convention des sidecars créés par une écriture.
    pub fn convention(&self) -> Convention {
        self.convention
    }

    /// Vrai si des notes sont attendues d'une lecture en fond ([`NoteBook::pending`]).
    pub fn is_pending(&self) -> bool {
        self.touched.is_some()
    }

    /// Reprend les notes lues en fond : celles des images lues ou écrites depuis le
    /// début de la lecture sont gardées, plus fraîches.
    pub fn adopt(&mut self, loaded: NoteBook) {
        let touched = self.touched.take().unwrap_or_default();
        for (image, entry) in loaded.entries {
            if !touched.contains(&image) {
                self.entries.insert(image, entry);
            }
        }
    }

    /// Notes en attente d'une lecture en fond : celles d'un carnet `known` (le même dossier
    /// lu avant) en attendant, sans les marquer comme plus fraîches.
    pub fn seed(&mut self, known: &NoteBook) {
        for images in self.by_dir.values() {
            for image in images {
                if let Some(entry) = known.entries.get(image) {
                    self.entries.insert(image.clone(), entry.clone());
                }
            }
        }
    }

    /// Relit les notes de `images` seules (une liste du dossier par dossier).
    fn reload_images(&mut self, images: &[PathBuf]) {
        let mut by_dir: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        for image in images {
            by_dir.entry(dir_of(image)).or_default().push(image.clone());
        }
        for (dir, images) in by_dir {
            for (image, entry) in read_entries(&dir, &images, self.convention, None) {
                if let Some(touched) = &mut self.touched {
                    touched.insert(image.clone());
                }
                self.entries.insert(image, entry);
            }
        }
    }

    /// Relit le dossier `dir` (sidecars créés, supprimés, ou modifiés par un autre
    /// logiciel).
    fn reload_dir(&mut self, dir: &Path) {
        let images = self.by_dir.get(dir).cloned().unwrap_or_default();
        self.reload_images(&images);
    }

    /// Ajoute `image` (inconnue) et lit ses notes, avec celles de sa paire.
    fn add(&mut self, image: &Path) {
        let dir = dir_of(image);
        let images = self.by_dir.entry(dir).or_default();
        if !images.iter().any(|p| p == image) {
            images.push(image.to_path_buf());
        }
        let mut targets = vec![image.to_path_buf()];
        targets.extend(self.siblings(image));
        self.reload_images(&targets);
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
                // Son sidecar, ou (développements) celui qu'il a lu.
                let own = self
                    .entries
                    .get(&image)
                    .is_some_and(|e| sidecars.contains(&e.sidecar));
                if !own
                    && !stems
                        .iter()
                        .any(|s| Some(s) == name.as_ref() || Some(s) == stem.as_ref())
                {
                    continue;
                }
                let sidecar = match crate::developments::get(&image) {
                    Some(development) => development.sidecar,
                    None => index.sidecar_path(&image, self.convention),
                };
                let (notes, error) = match read_sidecar(&sidecar) {
                    Ok(notes) => (notes, None),
                    Err(e) => (Notes::default(), Some(e)),
                };
                let old = self.entries.get(&image);
                if old.is_none_or(|e| e.notes != notes || e.error != error) {
                    changed.push(image.clone());
                }
                if let Some(touched) = &mut self.touched {
                    touched.insert(image.clone());
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

    /// `image` et les images qui reçoivent les mêmes notes : sa paire
    /// ([`NoteBook::siblings`]) ; pour le développement darktable principal, la photo (même
    /// XMP) et sa paire ; pour la photo, ses développements qui lisent le même XMP.
    fn shared_with(&mut self, image: &Path) -> Vec<PathBuf> {
        let base = match crate::developments::get(image) {
            Some(d) if d.copy == crate::developments::Copy::Main && !d.notes_in_rrdata() => d.photo,
            _ => image.to_path_buf(),
        };
        let mut targets = vec![image.to_path_buf()];
        if base != image {
            if !self.entries.contains_key(&base) {
                self.add(&base);
            }
            targets.push(base.clone());
        }
        targets.extend(self.siblings(&base));
        // Développements de la photo qui lisent son XMP, déjà suivis.
        let sidecar = self.entries.get(&base).map(|e| e.sidecar.clone());
        let sharing: Vec<PathBuf> = self
            .entries
            .iter()
            .filter(|(p, e)| Some(&e.sidecar) == sidecar.as_ref() && !targets.contains(p))
            .map(|(p, _)| p.clone())
            .collect();
        targets.extend(sharing);
        targets
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
        if !self.entries.contains_key(image) {
            self.add(image);
        }
        let entry = &self.entries[image];
        if let Some(error) = &entry.error {
            return Err(XmpError::Image {
                image: image.to_path_buf(),
                message: error.clone(),
            });
        }
        if entry.notes != *notes {
            write_sidecar(&entry.sidecar, notes).map_err(|message| XmpError::Image {
                image: image.to_path_buf(),
                message,
            })?;
        }
        // L'image et sa paire seulement (un sidecar n'est partagé qu'entre elles) : relire
        // tout le dossier coûtait cher sur un disque lent ou un NAS (issue n° 15).
        let targets = self.shared_with(image);
        self.reload_images(&targets);
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
        if !self.entries.contains_key(image) {
            self.add(image);
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
        let targets = self.shared_with(image);
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
                None => write_sidecar(&entry.sidecar, &notes),
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
        // Des sidecars ont pu être créés, et les images de la paire les partager : on les
        // relit, elles seules (tout le dossier coûtait cher sur un disque lent ou un NAS,
        // issue n° 15 ; un sidecar n'est partagé qu'entre images de même nom).
        let before: Vec<(PathBuf, Notes)> = targets
            .iter()
            .map(|p| (p.clone(), self.notes(p).clone()))
            .collect();
        self.reload_images(&targets);
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StarsRule {
    /// Au moins n étoiles.
    AtLeast(u8),
    /// Exactement n étoiles (0 : non notées).
    Exactly(u8),
    /// Au plus n étoiles.
    AtMost(u8),
}

/// Comparaison d'un filtre d'étoiles (#165 : pas seulement « au moins »).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StarsOp {
    /// Au moins.
    #[default]
    AtLeast,
    /// Exactement.
    Exactly,
    /// Au plus.
    AtMost,
}

impl StarsOp {
    /// La règle « `self` n étoiles ».
    pub fn rule(self, n: u8) -> StarsRule {
        match self {
            StarsOp::AtLeast => StarsRule::AtLeast(n),
            StarsOp::Exactly => StarsRule::Exactly(n),
            StarsOp::AtMost => StarsRule::AtMost(n),
        }
    }

    /// Comparaison suivante : au moins → exactement → au plus → au moins.
    pub fn next(self) -> StarsOp {
        match self {
            StarsOp::AtLeast => StarsOp::Exactly,
            StarsOp::Exactly => StarsOp::AtMost,
            StarsOp::AtMost => StarsOp::AtLeast,
        }
    }

    /// Symbole (« ≥ », « = », « ≤ »).
    pub fn symbol(self) -> &'static str {
        match self {
            StarsOp::AtLeast => "≥",
            StarsOp::Exactly => "=",
            StarsOp::AtMost => "≤",
        }
    }
}

impl StarsRule {
    /// Comparaison et nombre d'étoiles de la règle.
    pub fn parts(self) -> (StarsOp, u8) {
        match self {
            StarsRule::AtLeast(n) => (StarsOp::AtLeast, n),
            StarsRule::Exactly(n) => (StarsOp::Exactly, n),
            StarsRule::AtMost(n) => (StarsOp::AtMost, n),
        }
    }

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
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

    #[test]
    fn notes_read_in_the_background_keep_those_set_meanwhile() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("A.RAF"), dir.path().join("B.RAF"));
        for p in [&a, &b] {
            std::fs::write(p, b"raw").unwrap();
        }
        let three = Notes {
            stars: 3,
            ..Notes::default()
        };
        xmp::write(&a, &three, Convention::Darktable).unwrap();
        xmp::write(&b, &three, Convention::Darktable).unwrap();
        let images = [a.as_path(), b.as_path()];
        // En attente : notes vides, sans rien lire.
        let mut book = NoteBook::pending(images, Convention::Darktable);
        assert!(book.is_pending());
        assert_eq!(book.notes(&a).stars, 0);
        // Lecture « en fond » faite maintenant ; puis A notée avant qu'elle soit reprise.
        let done = AtomicUsize::new(0);
        let files: Vec<MediaFile> = images
            .iter()
            .map(|p| MediaFile {
                path: p.to_path_buf(),
                kind: crate::media::MediaKind::Raw,
            })
            .collect();
        let loaded = NoteBook::load_counting(&files, Convention::Darktable, &done);
        assert_eq!(done.load(Ordering::Relaxed), 2);
        book.edit(&a, |n| n.stars = 5).unwrap();
        book.adopt(loaded);
        assert!(!book.is_pending());
        assert_eq!(book.notes(&a).stars, 5, "notée entre-temps : gardée");
        assert_eq!(book.notes(&b).stars, 3, "lue en fond");
        // Issue n° 29 : noter A ne relit pas B (changée ailleurs, sans signal ici).
        xmp::write(&b, &Notes::default(), Convention::Darktable).unwrap();
        book.edit(&a, |n| n.stars = 1).unwrap();
        assert_eq!(book.notes(&b).stars, 3, "B pas relue");
    }

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
    fn a_stars_operator_cycles_and_builds_its_rule() {
        let mut op = StarsOp::default();
        assert_eq!(op.rule(3), StarsRule::AtLeast(3));
        op = op.next();
        assert_eq!((op.rule(3), op.symbol()), (StarsRule::Exactly(3), "="));
        op = op.next();
        assert_eq!(op.rule(3).parts(), (StarsOp::AtMost, 3));
        assert_eq!(op.next(), StarsOp::AtLeast);
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

    #[test]
    fn each_development_keeps_its_notes_where_its_software_reads_them() {
        // Décidé le 30/10/2026 : la photo et son développement darktable principal
        // partagent `IMG.RAF.xmp` ; un clone a son XMP ; RapidRAW, son `.rrdata`.
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::write(d.join("IMG.RAF"), b"x").unwrap();
        let darktable = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:darktable="http://darktable.sf.net/" darktable:xmp_version="5"/></rdf:RDF></x:xmpmeta>"#;
        std::fs::write(d.join("IMG.RAF.xmp"), darktable).unwrap();
        std::fs::write(d.join("IMG_01.RAF.xmp"), darktable).unwrap();
        std::fs::write(
            d.join("IMG.RAF.rrdata"),
            r#"{"rating": 1, "adjustments": {}}"#,
        )
        .unwrap();
        let files = crate::media::scan(d, 1).unwrap();
        let mut book = NoteBook::load_files(&files, Convention::Darktable);
        let (photo, main, clone, rr) = (
            d.join("IMG.RAF"),
            d.join("IMG@dt.RAF"),
            d.join("IMG@dt-01.RAF"),
            d.join("IMG@rr.RAF"),
        );
        assert_eq!(book.notes(&rr).stars, 1, "lu dans le .rrdata");
        book.edit(&main, |n| n.set_stars(3)).unwrap();
        assert_eq!(book.notes(&photo).stars, 3, "même XMP que la photo");
        assert_eq!(xmp::read(&photo).unwrap().stars, 3);
        assert_eq!(book.notes(&clone).stars, 0, "le clone a les siennes");
        book.edit(&clone, |n| n.set_stars(5)).unwrap();
        assert_eq!(
            xmp::read_sidecar(&d.join("IMG_01.RAF.xmp")).unwrap().stars,
            5
        );
        book.edit(&rr, |n| n.flag = Flag::Rejected).unwrap();
        let rrdata = crate::rrdata::read(&d.join("IMG.RAF.rrdata")).unwrap();
        assert_eq!((rrdata.stars, rrdata.flag), (1, Flag::Rejected));
        assert_eq!(book.notes(&photo).stars, 3, "RapidRAW à part");
    }
}
