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
use crate::sidecars::{self, Format};
use crate::xmp::{Color, Convention, Flag, Notes, SidecarIndex, XmpError};

/// Notes d'une image inconnue (une `const` contenant un `Vec` ne se prête pas en `&'static`).
static EMPTY: Notes = Notes::EMPTY;

/// Notes d'une image et sidecars d'où elles viennent.
#[derive(Clone, Debug)]
struct Entry {
    notes: Notes,
    /// Sidecars existants, tous formats ([`crate::sidecars`]), le plus récent en tête :
    /// lu, et tous écrits. Vide : aucun, le XMP de la convention sera créé.
    sidecars: Vec<PathBuf>,
    /// Sidecar illisible ou mal formé : notes vides, et jamais réécrit.
    error: Option<String>,
}

impl Entry {
    /// Le sidecar lu (le plus récent), ou celui qui sera créé.
    fn primary(&self, image: &Path, convention: Convention) -> PathBuf {
        self.sidecars
            .first()
            .cloned()
            .unwrap_or_else(|| convention.sidecar(image))
    }
}

/// Notes des images d'un ou plusieurs dossiers.
#[derive(Debug, Default)]
pub struct NoteBook {
    convention: Convention,
    /// Formats créés quand on note une photo qui n'en a pas (identifiants,
    /// [`crate::sidecars::SidecarSettings::creates`]) ; vide : le XMP seul.
    creates: Vec<&'static str>,
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
    done: Option<&AtomicUsize>,
) -> Vec<(PathBuf, Entry)> {
    let index = SidecarIndex::new(dir);
    images
        .iter()
        .map(|image| {
            let entry = read_notes(image, &index);
            if let Some(done) = done {
                done.fetch_add(1, Ordering::Relaxed);
            }
            (image.clone(), entry)
        })
        .collect()
}

/// Notes de `image` et les sidecars qui les portent. Un développement
/// ([`crate::developments`]) a les siennes dans son sidecar : XMP de darktable, `.rrdata`
/// de RapidRAW ([`crate::rrdata`]) ; une photo, dans tous ses sidecars
/// ([`crate::sidecars`]), lues dans le plus récent.
fn read_notes(image: &Path, index: &SidecarIndex) -> Entry {
    let mut sidecars: Vec<PathBuf> = match crate::developments::get(image) {
        Some(development) => vec![development.sidecar],
        None => sidecars::formats()
            .iter()
            .filter_map(|f| f.existing(image, index))
            .collect(),
    };
    if sidecars.len() > 1 {
        // Le plus récent en tête (noté ailleurs entre-temps : c'est lui qui a raison).
        sidecars.sort_by_key(|p| {
            std::cmp::Reverse(
                std::fs::metadata(p)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            )
        });
    }
    let (notes, error) = match sidecars.first() {
        None => (Notes::default(), None),
        Some(sidecar) => match read_sidecar(sidecar) {
            Ok(notes) => (notes, None),
            Err(e) => (Notes::default(), Some(e)),
        },
    };
    Entry {
        notes,
        sidecars,
        error,
    }
}

/// Notes du sidecar `sidecar`, de son format.
fn read_sidecar(sidecar: &Path) -> Result<Notes, String> {
    sidecars::of(sidecar).read(sidecar)
}

/// Écrit `notes` dans `sidecar`, de son format (le reste du fichier gardé).
fn write_sidecar(sidecar: &Path, notes: &Notes) -> Result<(), String> {
    sidecars::of(sidecar).write(sidecar, notes).map(|_| ())
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
            let entries = read_entries(dir, images, Some(done));
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

    /// Formats créés quand on note une photo qui n'en a pas (identifiants,
    /// [`crate::sidecars::SidecarSettings::creates`]).
    pub fn set_creates(&mut self, creates: Vec<&'static str>) {
        self.creates = creates;
    }

    /// Vrai si une image suivie a déjà un sidecar du format `format` (le dossier en
    /// contient : le logiciel y est utilisé).
    pub fn has_format(&self, format: &dyn Format) -> bool {
        self.entries
            .values()
            .any(|e| e.sidecars.iter().any(|s| format.owns(s)))
    }

    /// Sidecars à créer pour `image` quand on la note : ceux des formats de
    /// [`NoteBook::set_creates`] qu'elle n'a pas (jamais pour un développement, qui n'a
    /// que le sien ; le XMP toujours).
    fn missing(&self, image: &Path) -> Vec<(&'static dyn Format, PathBuf)> {
        let Some(entry) = self.entries.get(image) else {
            return Vec::new();
        };
        if crate::developments::get(image).is_some() {
            return Vec::new();
        }
        sidecars::formats()
            .iter()
            .copied()
            .filter(|f| f.id() == "xmp" || self.creates.contains(&f.id()))
            .filter(|f| !entry.sidecars.iter().any(|s| f.owns(s)))
            .map(|f| (f, f.new_path(image, self.convention)))
            .collect()
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
            for (image, entry) in read_entries(&dir, &images, None) {
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
                // Ses sidecars, ou (développements) celui qu'il a lu.
                let own = self
                    .entries
                    .get(&image)
                    .is_some_and(|e| e.sidecars.iter().any(|s| sidecars.contains(s)));
                if !own
                    && !stems
                        .iter()
                        .any(|s| Some(s) == name.as_ref() || Some(s) == stem.as_ref())
                {
                    continue;
                }
                let entry = read_notes(&image, &index);
                let old = self.entries.get(&image);
                if old.is_none_or(|e| e.notes != entry.notes || e.error != entry.error) {
                    changed.push(image.clone());
                }
                if let Some(touched) = &mut self.touched {
                    touched.insert(image.clone());
                }
                self.entries.insert(image, entry);
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

    /// Sidecar de `image` : celui qui a été lu (le plus récent), ou celui qui sera créé.
    pub fn sidecar(&self, image: &Path) -> Option<PathBuf> {
        self.entries
            .get(image)
            .map(|e| e.primary(image, self.convention))
    }

    /// Tous les sidecars existants de `image` (le plus récent en tête).
    pub fn sidecars(&self, image: &Path) -> &[PathBuf] {
        self.entries
            .get(image)
            .map_or(&[], |e| e.sidecars.as_slice())
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
    /// ([`NoteBook::siblings`]) ; pour un développement **principal** (darktable,
    /// RapidRAW), la photo et sa paire ; pour la photo, ses développements principaux,
    /// chacun dans son sidecar (le XMP de darktable, partagé avec la photo ; le `.rrdata`
    /// de RapidRAW). Un développement principal est la photo dans son logiciel : la noter
    /// dans `tri` doit se voir dans darktable **et** dans RapidRAW (issue n° 234, 31/10/2026 ;
    /// le `.rrdata` principal avait ses notes à part, RapidRAW ne voyait pas celles de la
    /// photo). Un clone et une copie virtuelle gardent les leurs.
    pub fn shared(&self, image: &Path) -> Vec<PathBuf> {
        let base = match crate::developments::get(image) {
            Some(d) if d.copy == crate::developments::Copy::Main => d.photo,
            _ => image.to_path_buf(),
        };
        let mut targets = vec![image.to_path_buf()];
        if base != image {
            targets.push(base.clone());
        }
        targets.extend(self.siblings(&base));
        // Développements principaux de la photo (chaque sidecar écrit une fois).
        for development in crate::developments::of(&base) {
            let path = development.path();
            if development.copy == crate::developments::Copy::Main && !targets.contains(&path) {
                targets.push(path);
            }
        }
        targets
    }

    /// [`NoteBook::shared`], les images encore inconnues ajoutées au carnet.
    fn shared_with(&mut self, image: &Path) -> Vec<PathBuf> {
        let targets = self.shared(image);
        for target in &targets {
            if !self.entries.contains_key(target) {
                self.add(target);
            }
        }
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

    /// Crée, pour les photos `images` qui ont des notes, les sidecars des formats `formats`
    /// qu'elles n'ont pas encore, avec leurs notes (à la demande : `tri --sidecars`, pour
    /// garder les notes dans un logiciel qu'on n'utilise qu'à l'occasion). Les
    /// développements, les images sans notes et celles qui ont déjà le sidecar sont
    /// passés. `dry_run` : rien n'est écrit, le bilan dit ce qui le serait.
    pub fn generate(
        &mut self,
        images: &[PathBuf],
        formats: &[&'static dyn Format],
        dry_run: bool,
        rewrite: bool,
    ) -> Generated {
        let mut report = Generated::default();
        for image in images {
            if !self.entries.contains_key(image) {
                self.add(image);
            }
            if crate::developments::get(image).is_some() {
                report.skipped += 1;
                continue;
            }
            let Some(entry) = self.entries.get(image) else {
                continue;
            };
            if entry.notes == Notes::default() {
                report.skipped += 1;
                continue;
            }
            let notes = entry.notes.clone();
            // Par format : le sidecar manquant à créer, ou l'existant à réécrire (`rewrite`,
            // 31/10/2026 : « on a fait une bêtise, on veut régénérer »).
            let mut todo: Vec<(&'static dyn Format, PathBuf, bool)> = Vec::new();
            for &format in formats {
                match entry.sidecars.iter().find(|s| format.owns(s)) {
                    Some(existing) if rewrite => todo.push((format, existing.clone(), true)),
                    Some(_) => {}
                    None => todo.push((format, format.new_path(image, self.convention), false)),
                }
            }
            if todo.is_empty() {
                report.unchanged += 1;
                continue;
            }
            for (format, sidecar, existing) in todo {
                let done = if existing {
                    &mut report.rewritten
                } else {
                    &mut report.created
                };
                if dry_run {
                    done.push(sidecar);
                    continue;
                }
                let result = if existing {
                    // Les notes réécrites, le reste conservé (développement, mots-clés…) ;
                    // un fichier illisible est recréé.
                    format
                        .write(&sidecar, &notes)
                        .map(|_| ())
                        .or_else(|_| format.create(&sidecar, &notes))
                } else {
                    format.create(&sidecar, &notes)
                };
                match result {
                    Ok(()) => done.push(sidecar),
                    Err(e) => report.errors.push((image.clone(), e)),
                }
            }
            if !dry_run {
                self.reload_images(std::slice::from_ref(image));
            }
        }
        report
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
            // Les sidecars existants seulement : rien n'est créé en annulant.
            let targets = if entry.sidecars.is_empty() {
                vec![entry.primary(image, self.convention)]
            } else {
                entry.sidecars.clone()
            };
            for sidecar in &targets {
                write_sidecar(sidecar, notes).map_err(|message| XmpError::Image {
                    image: image.to_path_buf(),
                    message,
                })?;
            }
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
            if entry.notes == notes && !entry.sidecars.is_empty() {
                continue;
            }
            if let Some(error) = &entry.error {
                failure.get_or_insert(XmpError::Image {
                    image: target.clone(),
                    message: error.clone(),
                });
                continue;
            }
            // Tous ses sidecars existants (un sidecar partagé, convention Adobe ou
            // `.rrdata` du développement principal : écrit une fois), puis ceux à créer.
            let mut planned: Vec<(&'static dyn Format, PathBuf, bool)> = entry
                .sidecars
                .iter()
                .map(|s| (sidecars::of(s), s.clone(), false))
                .collect();
            planned.extend(
                self.missing(target)
                    .into_iter()
                    .map(|(f, path)| (f, path, true)),
            );
            for (format, sidecar, create) in planned {
                if written.contains(&sidecar) {
                    continue;
                }
                let result = if create {
                    format.create(&sidecar, &notes)
                } else {
                    format.write(&sidecar, &notes).map(|_| ())
                };
                match result {
                    Ok(()) => written.push(sidecar),
                    Err(message) => {
                        failure.get_or_insert(XmpError::Image {
                            image: target.clone(),
                            message,
                        });
                    }
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

/// Bilan de [`NoteBook::generate`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Generated {
    /// Sidecars créés (ou à créer, en simulation).
    pub created: Vec<PathBuf>,
    /// Sidecars existants réécrits (ou à réécrire), mode `rewrite`.
    pub rewritten: Vec<PathBuf>,
    /// Photos qui avaient déjà tous les sidecars demandés.
    pub unchanged: usize,
    /// Développements et photos sans notes : rien à créer.
    pub skipped: usize,
    /// Photos dont un sidecar n'a pas pu être créé.
    pub errors: Vec<(PathBuf, String)>,
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
    use crate::xmp;

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
    fn a_photo_is_noted_in_every_sidecar_and_the_missing_ones_follow_the_policy() {
        // 31/10/2026 (`sidecars.md`) : le `.rrdata` d'une photo seulement notée n'est pas
        // un développement, mais un de ses sidecars ; noter écrit tous les existants et
        // crée les manquants des formats actifs.
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::write(d.join("A.RAF"), b"x").unwrap();
        std::fs::write(d.join("B.RAF"), b"x").unwrap();
        std::fs::write(
            d.join("A.RAF.rrdata"),
            r#"{"rating": 2, "adjustments": null}"#,
        )
        .unwrap();
        let files = crate::media::scan(d, 1).unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["A.RAF", "B.RAF"], "pas de développement A@rr");
        let mut book = NoteBook::load_files(&files, Convention::Darktable);
        let (a, b) = (d.join("A.RAF"), d.join("B.RAF"));
        assert_eq!(book.notes(&a).stars, 2, "lu dans le .rrdata, seul sidecar");
        assert_eq!(book.sidecars(&a), [d.join("A.RAF.rrdata")]);
        // Politique par défaut (XMP seul) : A écrit son .rrdata existant et crée son XMP ;
        // B n'a que son XMP.
        book.edit(&a, |n| n.set_stars(4)).unwrap();
        assert_eq!(
            crate::rrdata::read(&d.join("A.RAF.rrdata")).unwrap().stars,
            4
        );
        assert_eq!(xmp::read_sidecar(&d.join("A.RAF.xmp")).unwrap().stars, 4);
        assert_eq!(book.sidecars(&a).len(), 2, "les deux, désormais");
        book.edit(&b, |n| n.set_stars(1)).unwrap();
        assert!(d.join("B.RAF.xmp").is_file());
        assert!(
            !d.join("B.RAF.rrdata").exists(),
            "pas de .rrdata sans politique"
        );
        // RapidRAW actif : le .rrdata manquant est créé, notes seules, pas de développement.
        book.set_creates(vec!["xmp", "rrdata"]);
        book.edit(&b, |n| n.flag = Flag::Accepted).unwrap();
        let created = std::fs::read_to_string(d.join("B.RAF.rrdata")).unwrap();
        assert_eq!(
            crate::rrdata::parse(&created).unwrap(),
            notes(1, Flag::Accepted, &[]),
            "la note et le drapeau, dans un sidecar neuf"
        );
        assert!(!crate::rrdata::is_development(&created));
        assert_eq!(book.sidecars(&b).len(), 2);
        // Annuler n'écrit que les existants (ici les deux).
        book.write_only(&b, &notes(1, Flag::None, &[])).unwrap();
        assert_eq!(
            crate::rrdata::read(&d.join("B.RAF.rrdata")).unwrap().flag,
            Flag::None
        );
    }

    #[test]
    fn generating_sidecars_covers_the_rated_photos_only() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        for name in ["A.RAF", "B.RAF", "C.RAF"] {
            std::fs::write(d.join(name), b"x").unwrap();
        }
        xmp::write_sidecar(&d.join("A.RAF.xmp"), &notes(3, Flag::None, &[Color::Red])).unwrap();
        xmp::write_sidecar(&d.join("B.RAF.xmp"), &notes(5, Flag::None, &[])).unwrap();
        std::fs::write(
            d.join("B.RAF.rrdata"),
            r#"{"rating": 5, "adjustments": null}"#,
        )
        .unwrap();
        let files = crate::media::scan(d, 1).unwrap();
        let mut book = NoteBook::load_files(&files, Convention::Darktable);
        let images: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
        let rrdata = crate::sidecars::by_id("rrdata").unwrap();
        // Simulation : A à créer ; B l'a déjà ; C sans notes, passée.
        let dry = book.generate(&images, &[rrdata], true, false);
        assert_eq!(dry.created, [d.join("A.RAF.rrdata")]);
        assert_eq!((dry.unchanged, dry.skipped), (1, 1));
        assert!(
            !d.join("A.RAF.rrdata").exists(),
            "simulation : rien d'écrit"
        );
        let done = book.generate(&images, &[rrdata], false, false);
        assert_eq!(done.created, [d.join("A.RAF.rrdata")]);
        let created = crate::rrdata::read(&d.join("A.RAF.rrdata")).unwrap();
        assert_eq!(created, notes(3, Flag::None, &[Color::Red]));
        assert_eq!(book.sidecars(&d.join("A.RAF")).len(), 2, "relu");
        // Encore : plus rien à créer.
        let again = book.generate(&images, &[rrdata], false, false);
        assert!(again.created.is_empty());
        assert_eq!(again.unchanged, 2);
        // Réécrire (31/10/2026) : la note remise dans les sidecars existants, le reste
        // conservé ; un fichier illisible est recréé.
        std::fs::write(
            d.join("A.RAF.rrdata"),
            br#"{"version": 1, "rating": 0, "adjustments": {"exposure": 0.7}, "extra": true}"#,
        )
        .unwrap();
        std::fs::write(d.join("B.RAF.rrdata"), b"pas du JSON").unwrap();
        crate::cache::forget_stamps([
            d.join("A.RAF.rrdata").as_path(),
            d.join("B.RAF.rrdata").as_path(),
        ]);
        let dry = book.generate(&images, &[rrdata], true, true);
        assert_eq!(dry.rewritten.len(), 2, "A et B existent : à réécrire");
        assert!(dry.created.is_empty());
        let redone = book.generate(&images, &[rrdata], false, true);
        assert_eq!(redone.rewritten.len(), 2);
        assert!(redone.errors.is_empty(), "{:?}", redone.errors);
        let a = std::fs::read_to_string(d.join("A.RAF.rrdata")).unwrap();
        assert!(a.contains("\"rating\": 3") && a.contains("exposure") && a.contains("extra"));
        assert_eq!(
            crate::rrdata::read(&d.join("B.RAF.rrdata")).unwrap().stars,
            5,
            "recréé"
        );
    }

    #[test]
    fn each_development_keeps_its_notes_where_its_software_reads_them() {
        // Décidé le 30/10/2026 : la photo et son développement darktable principal
        // partagent `IMG.RAF.xmp` ; un clone a son XMP ; RapidRAW, son `.rrdata`. Issue
        // n° 234 (31/10/2026) : le développement RapidRAW principal a les notes de la
        // photo, écrites aussi dans son `.rrdata` ; une copie virtuelle garde les siennes.
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::write(d.join("IMG.RAF"), b"x").unwrap();
        std::fs::write(d.join("IMG.JPG"), b"x").unwrap();
        let darktable = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:darktable="http://darktable.sf.net/" darktable:xmp_version="5"/></rdf:RDF></x:xmpmeta>"#;
        std::fs::write(d.join("IMG.RAF.xmp"), darktable).unwrap();
        std::fs::write(d.join("IMG_01.RAF.xmp"), darktable).unwrap();
        std::fs::write(
            d.join("IMG.RAF.rrdata"),
            r#"{"rating": 1, "adjustments": {"exposure": 0.5}}"#,
        )
        .unwrap();
        std::fs::write(
            d.join("IMG.RAF.ab12.rrdata"),
            r#"{"rating": 2, "adjustments": {}}"#,
        )
        .unwrap();
        let files = crate::media::scan(d, 1).unwrap();
        let mut book = NoteBook::load_files(&files, Convention::Darktable);
        let (photo, jpg, main, clone, rr, copy) = (
            d.join("IMG.RAF"),
            d.join("IMG.JPG"),
            d.join("IMG@dt.RAF"),
            d.join("IMG@dt-01.RAF"),
            d.join("IMG@rr.RAF"),
            d.join("IMG@rr-ab12.RAF"),
        );
        assert_eq!(book.notes(&rr).stars, 1, "lu dans le .rrdata");
        assert_eq!(book.notes(&copy).stars, 2, "lu dans le .rrdata de la copie");
        // Noter le développement darktable principal : la photo, sa paire et le
        // développement RapidRAW principal suivent (XMP et .rrdata écrits).
        let changed = book.edit(&main, |n| n.set_stars(3)).unwrap();
        assert_eq!(book.notes(&photo).stars, 3, "même XMP que la photo");
        assert_eq!(xmp::read(&photo).unwrap().stars, 3);
        assert_eq!(xmp::read(&jpg).unwrap().stars, 3, "la paire");
        assert_eq!(
            crate::rrdata::read(&d.join("IMG.RAF.rrdata"))
                .unwrap()
                .stars,
            3,
            "écrit dans le .rrdata : RapidRAW voit la note (issue n° 234)"
        );
        assert_eq!(book.notes(&rr).stars, 3);
        assert!(changed.contains(&rr) && changed.contains(&photo) && changed.contains(&jpg));
        assert_eq!(book.notes(&clone).stars, 0, "le clone a les siennes");
        assert_eq!(book.notes(&copy).stars, 2, "la copie virtuelle aussi");
        book.edit(&clone, |n| n.set_stars(5)).unwrap();
        assert_eq!(
            xmp::read_sidecar(&d.join("IMG_01.RAF.xmp")).unwrap().stars,
            5
        );
        assert_eq!(
            book.notes(&photo).stars,
            3,
            "le clone ne touche pas la photo"
        );
        // Noter le développement RapidRAW principal : la photo et darktable suivent.
        book.edit(&rr, |n| n.flag = Flag::Rejected).unwrap();
        let rrdata = crate::rrdata::read(&d.join("IMG.RAF.rrdata")).unwrap();
        assert_eq!((rrdata.stars, rrdata.flag), (3, Flag::Rejected));
        assert_eq!(
            xmp::read(&photo).unwrap().flag,
            Flag::Rejected,
            "la photo aussi"
        );
        assert_eq!(book.notes(&main).flag, Flag::Rejected);
        // Noter la photo : les deux développements principaux suivent, pas la copie.
        book.edit(&photo, |n| n.colors = vec![Color::Red]).unwrap();
        assert_eq!(
            crate::rrdata::read(&d.join("IMG.RAF.rrdata"))
                .unwrap()
                .colors,
            [Color::Red]
        );
        assert_eq!(
            crate::rrdata::read(&d.join("IMG.RAF.ab12.rrdata"))
                .unwrap()
                .stars,
            2
        );
        assert!(book.notes(&copy).colors.is_empty());
    }
}
