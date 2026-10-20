//! Historique d'annulation : chaque action qui modifie quelque chose (notes, renommage,
//! copie / déplacement, corbeille) devient un **pas**, qu'on peut défaire et refaire.
//!
//! Décisions du 18/10/2026 (voir `doc/history.md`) :
//!
//! - historique **de session**, en mémoire : rien n'est gardé d'un lancement à l'autre ;
//! - une action = un pas, même sur 30 images ;
//! - **jamais d'écrasement d'un travail fait ailleurs** : avant de défaire (ou refaire)
//!   un pas, on vérifie que les fichiers sont toujours dans l'état laissé par ce pas
//!   (notes relues sur le disque, fichiers présents, copies inchangées) ; sinon le pas
//!   est refusé avec la raison, et reste où il est ;
//! - hors historique : navigation, filtres, sélection (rien n'est modifié), suppression
//!   définitive (confirmée), déchargement d'une carte.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::files::{self, DirTransfer, FileError, Rename, Transfer, TransferMode, Trashed};
use crate::notes::NoteBook;
use crate::xmp::{Color, Convention, Flag, Notes};

/// Notes d'une image avant et après un pas.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteChange {
    /// L'image (une paire RAW+JPEG donne deux changements).
    pub image: PathBuf,
    /// Notes avant.
    pub before: Notes,
    /// Notes après.
    pub after: Notes,
}

/// Empreinte d'un fichier écrit par un pas (taille, date de modification) : s'il a changé
/// depuis, il a été modifié ailleurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Print {
    len: u64,
    modified: Option<SystemTime>,
}

impl Print {
    /// Empreinte actuelle de `path` (`None` s'il n'existe pas).
    pub fn of(path: &Path) -> Option<Print> {
        let m = std::fs::metadata(path).ok()?;
        Some(Print {
            len: m.len(),
            modified: m.modified().ok(),
        })
    }
}

/// Un pas de l'historique.
#[derive(Clone, Debug)]
pub enum Step {
    /// Notes modifiées (étoiles, couleurs, drapeau).
    Notes(Vec<NoteChange>),
    /// Renommage (paire et sidecars compris).
    Rename(Rename),
    /// Copie ou déplacement ; `written` : fichiers écrits dans la destination et leur
    /// empreinte (une copie modifiée depuis n'est pas supprimée).
    Transfer {
        /// Le plan exécuté.
        plan: Transfer,
        /// Fichiers écrits dans la destination.
        written: Vec<(PathBuf, Print)>,
    },
    /// Copie ou déplacement d'un dossier entier ; `written` : fichiers écrits dans la
    /// cible et leur empreinte (une copie modifiée depuis n'est pas supprimée).
    TransferDir {
        /// Le plan exécuté.
        plan: DirTransfer,
        /// Fichiers écrits dans la cible.
        written: Vec<(PathBuf, Print)>,
    },
    /// Mise à la corbeille (fichiers, ou un dossier entier).
    Trash(Vec<Trashed>),
    /// Dossier créé.
    CreateDir(PathBuf),
    /// Dossier renommé : (ancien, nouveau).
    RenameDir(PathBuf, PathBuf),
    /// Versions d'une image (recadrages, `tri::versions`) : liste `tri:Versions` avant et
    /// après, et ce qui a changé.
    Versions {
        /// L'image.
        image: PathBuf,
        /// Éléments avant.
        before: Vec<String>,
        /// Éléments après.
        after: Vec<String>,
        /// Ce qui a changé (pour la liste de l'historique).
        change: VersionChange,
    },
}

/// Changement d'une version, pour l'historique.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VersionChange {
    /// Version créée (son nom).
    Created(String),
    /// Version modifiée.
    Changed(String),
    /// Version supprimée.
    Deleted(String),
}

/// Pourquoi un pas ne peut pas être défait (ou refait).
#[derive(Debug)]
pub enum HistoryError {
    /// Notes changées ailleurs depuis ce pas (autre logiciel).
    NotesChanged(PathBuf),
    /// Fichier absent (supprimé, déplacé ailleurs).
    Missing(PathBuf),
    /// Fichier modifié ailleurs depuis ce pas.
    Modified(PathBuf),
    /// Erreur sur les fichiers (nom déjà pris, corbeille vidée…).
    Files(FileError),
    /// Sidecar illisible ou impossible à écrire.
    Notes(String),
    /// Rien à défaire ou à refaire.
    Nothing,
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = |p: &PathBuf| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        match self {
            Self::NotesChanged(p) => {
                write!(f, "{}", crate::t!("history-notes-changed", name = name(p)))
            }
            Self::Missing(p) => write!(f, "{}", crate::t!("history-missing", name = name(p))),
            Self::Modified(p) => write!(f, "{}", crate::t!("history-modified", name = name(p))),
            Self::Files(e) => write!(f, "{e}"),
            Self::Notes(e) => write!(f, "{e}"),
            Self::Nothing => write!(f, "{}", crate::t!("history-nothing")),
        }
    }
}

impl std::error::Error for HistoryError {}

impl From<FileError> for HistoryError {
    fn from(e: FileError) -> Self {
        HistoryError::Files(e)
    }
}

/// `Result` spécialisé pour l'historique.
pub type Result<T> = std::result::Result<T, HistoryError>;

/// Notes de `images` et de leurs paires, avant une action ([`notes_step`] après).
pub fn snapshot(book: &NoteBook, images: &[PathBuf]) -> Vec<(PathBuf, Notes)> {
    let mut seen = Vec::new();
    for image in images {
        for p in std::iter::once(image.clone()).chain(book.siblings(image)) {
            if !seen.iter().any(|(q, _)| *q == p) {
                let notes = book.notes(&p).clone();
                seen.push((p, notes));
            }
        }
    }
    seen
}

/// Le pas d'une action sur les notes : ce qui a changé depuis `before` ([`snapshot`]) ;
/// `None` si rien n'a changé.
pub fn notes_step(book: &NoteBook, before: Vec<(PathBuf, Notes)>) -> Option<Step> {
    let changes: Vec<NoteChange> = before
        .into_iter()
        .filter_map(|(image, before)| {
            let after = book.notes(&image).clone();
            (after != before).then_some(NoteChange {
                image,
                before,
                after,
            })
        })
        .collect();
    (!changes.is_empty()).then_some(Step::Notes(changes))
}

/// Le pas d'une copie ou d'un déplacement réussi (empreintes des fichiers écrits).
pub fn transfer_step(plan: Transfer) -> Step {
    let written = plan
        .files
        .iter()
        .chain(&plan.shared)
        .filter_map(|(_, to)| Print::of(to).map(|p| (to.clone(), p)))
        .collect();
    Step::Transfer { plan, written }
}

/// Le pas d'une copie ou d'un déplacement de dossier réussi (empreintes des fichiers
/// écrits).
pub fn transfer_dir_step(plan: DirTransfer) -> Step {
    let written = plan
        .files
        .iter()
        .filter_map(|(_, to)| Print::of(to).map(|p| (to.clone(), p)))
        .collect();
    Step::TransferDir { plan, written }
}

fn name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Ce qui change entre deux notes, en mots (« 3 étoiles », « rejetée », « rouge »).
fn note_diff(before: &Notes, after: &Notes) -> String {
    let mut parts = Vec::new();
    if before.stars != after.stars {
        parts.push(crate::t!("history-stars", n = after.stars));
    }
    if before.flag != after.flag {
        parts.push(match after.flag {
            Flag::Accepted => crate::t!("history-accepted"),
            Flag::Rejected => crate::t!("history-rejected"),
            Flag::None => crate::t!("history-no-flag"),
        });
    }
    for c in Color::ALL {
        let (was, is) = (before.has_color(c), after.has_color(c));
        if was != is {
            let color = crate::t!(color_id(c));
            parts.push(if is {
                crate::t!("history-color-added", color = color)
            } else {
                crate::t!("history-color-removed", color = color)
            });
        }
    }
    parts.join(", ")
}

fn color_id(c: Color) -> &'static str {
    match c {
        Color::Red => "history-red",
        Color::Yellow => "history-yellow",
        Color::Green => "history-green",
        Color::Blue => "history-blue",
        Color::Purple => "history-purple",
    }
}

impl Step {
    /// Description courte, pour la liste de l'historique et la barre d'état.
    pub fn describe(&self) -> String {
        match self {
            Step::Notes(changes) => {
                let first = &changes[0];
                let what = note_diff(&first.before, &first.after);
                // Une image et sa paire : un seul nom.
                let stems: std::collections::HashSet<_> =
                    changes.iter().map(|c| c.image.with_extension("")).collect();
                if stems.len() == 1 {
                    crate::t!("history-notes-one", what = what, name = name(&first.image))
                } else {
                    crate::t!("history-notes-many", what = what, n = stems.len())
                }
            }
            Step::Rename(plan) => {
                let (from, to) = plan.moves.first().cloned().unwrap_or_default();
                crate::t!("history-rename", from = name(&from), to = name(&to))
            }
            Step::Transfer { plan, .. } => {
                let n = plan.images().count();
                let dest = plan.dest.display().to_string();
                match plan.mode {
                    TransferMode::Copy => crate::t!("history-copy", n = n, dest = dest),
                    TransferMode::Move => crate::t!("history-move", n = n, dest = dest),
                }
            }
            Step::TransferDir { plan, .. } => {
                let (folder, dest) = (name(&plan.from), plan.to.parent().unwrap_or(Path::new("")));
                let dest = dest.display().to_string();
                match plan.mode {
                    TransferMode::Copy => crate::t!("history-copy-dir", name = folder, dest = dest),
                    TransferMode::Move => crate::t!("history-move-dir", name = folder, dest = dest),
                }
            }
            Step::Trash(items) => match items.as_slice() {
                [one] => crate::t!("history-trash-one", name = name(&one.original)),
                _ => crate::t!("history-trash", n = items.len()),
            },
            Step::CreateDir(path) => crate::t!("history-create-dir", name = name(path)),
            Step::RenameDir(from, to) => {
                crate::t!("history-rename", from = name(from), to = name(to))
            }
            Step::Versions { image, change, .. } => {
                let (id, version) = match change {
                    VersionChange::Created(v) => ("history-version-created", v),
                    VersionChange::Changed(v) => ("history-version-changed", v),
                    VersionChange::Deleted(v) => ("history-version-deleted", v),
                };
                crate::t!(id, version = version.as_str(), name = name(image))
            }
        }
    }

    /// Images à montrer après avoir défait ou refait ce pas (là où elles sont alors).
    pub fn images(&self, undone: bool) -> Vec<PathBuf> {
        match self {
            Step::Notes(changes) => changes.iter().map(|c| c.image.clone()).collect(),
            Step::Rename(plan) => plan
                .moves
                .iter()
                .map(|(from, to)| if undone { from.clone() } else { to.clone() })
                .collect(),
            Step::Transfer { plan, .. } => plan
                .images()
                .map(|(from, to)| {
                    if undone || plan.mode == TransferMode::Copy {
                        from.clone()
                    } else {
                        to.clone()
                    }
                })
                .collect(),
            Step::Trash(items) => {
                if undone {
                    items.iter().map(|t| t.original.clone()).collect()
                } else {
                    Vec::new()
                }
            }
            Step::CreateDir(_) | Step::TransferDir { .. } => Vec::new(),
            Step::RenameDir(from, to) => vec![if undone { from.clone() } else { to.clone() }],
            Step::Versions { image, .. } => vec![image.clone()],
        }
    }

    /// Défait ce pas. `convention` : celle des sidecars (notes).
    fn undo(&mut self, convention: Convention) -> Result<()> {
        match self {
            Step::Notes(changes) => set_notes(changes, convention, true),
            Step::Versions {
                image,
                before,
                after,
                ..
            } => set_versions(image, after, before, convention),
            Step::Rename(plan) => {
                for (from, to) in &plan.moves {
                    if !to.exists() {
                        return Err(HistoryError::Missing(to.clone()));
                    }
                    let same = from.canonicalize().ok() == to.canonicalize().ok();
                    if from.exists() && !same {
                        return Err(HistoryError::Files(FileError::Exists(from.clone())));
                    }
                }
                for (_, copy) in &plan.copies {
                    if !copy.exists() {
                        return Err(HistoryError::Missing(copy.clone()));
                    }
                }
                let back = Rename {
                    moves: plan
                        .moves
                        .iter()
                        .map(|(a, b)| (b.clone(), a.clone()))
                        .collect(),
                    copies: Vec::new(),
                };
                files::rename(&back)?;
                for (_, copy) in &plan.copies {
                    let _ = std::fs::remove_file(copy);
                }
                Ok(())
            }
            Step::Transfer { plan, written } => {
                for (path, print) in written.iter() {
                    match Print::of(path) {
                        None => return Err(HistoryError::Missing(path.clone())),
                        Some(now) if now != *print => {
                            return Err(HistoryError::Modified(path.clone()));
                        }
                        _ => {}
                    }
                }
                match plan.mode {
                    TransferMode::Copy => {
                        for (path, _) in written.iter() {
                            std::fs::remove_file(path)
                                .map_err(|e| FileError::Io(path.clone(), e))?;
                        }
                    }
                    TransferMode::Move => {
                        for (from, _) in &plan.files {
                            if from.exists() {
                                return Err(HistoryError::Files(FileError::Exists(from.clone())));
                            }
                        }
                        let back = Transfer {
                            mode: TransferMode::Move,
                            dest: PathBuf::new(),
                            files: plan
                                .files
                                .iter()
                                .map(|(a, b)| (b.clone(), a.clone()))
                                .collect(),
                            shared: Vec::new(),
                        };
                        files::transfer(&back, |_, _| {})?;
                        // Sidecars partagés : copiés à l'aller, l'original est resté.
                        for (_, copy) in &plan.shared {
                            let _ = std::fs::remove_file(copy);
                        }
                    }
                }
                Ok(())
            }
            Step::TransferDir { plan, written } => {
                if !plan.to.is_dir() {
                    return Err(HistoryError::Missing(plan.to.clone()));
                }
                match plan.mode {
                    TransferMode::Copy => {
                        // Copie retirée seulement telle qu'on l'a laissée : rien d'ajouté ni
                        // de modifié depuis.
                        for (path, print) in written.iter() {
                            match Print::of(path) {
                                None => return Err(HistoryError::Missing(path.clone())),
                                Some(now) if now != *print => {
                                    return Err(HistoryError::Modified(path.clone()));
                                }
                                _ => {}
                            }
                        }
                        let known: std::collections::HashSet<&PathBuf> =
                            written.iter().map(|(p, _)| p).collect();
                        for entry in walkdir::WalkDir::new(&plan.to).min_depth(1) {
                            let entry =
                                entry.map_err(|_| HistoryError::Modified(plan.to.clone()))?;
                            let path = entry.path().to_path_buf();
                            if !entry.file_type().is_dir() && !known.contains(&path) {
                                return Err(HistoryError::Modified(path));
                            }
                        }
                        std::fs::remove_dir_all(&plan.to)
                            .map_err(|e| FileError::Io(plan.to.clone(), e))?;
                    }
                    TransferMode::Move => {
                        if plan.from.exists() {
                            return Err(HistoryError::Files(FileError::Exists(plan.from.clone())));
                        }
                        let back = plan.from.parent().unwrap_or(Path::new("."));
                        let undo = files::plan_transfer_dir(&plan.to, back, TransferMode::Move)?;
                        files::transfer_dir(&undo, |_, _| {})?;
                    }
                }
                Ok(())
            }
            Step::Trash(items) => Ok(files::untrash(items)?),
            Step::CreateDir(path) => {
                // Seulement vide : ce qu'on y a mis depuis ne disparaît pas.
                if !path.is_dir() {
                    return Err(HistoryError::Missing(path.clone()));
                }
                std::fs::remove_dir(&*path).map_err(|_| HistoryError::Modified(path.clone()))
            }
            Step::RenameDir(from, to) => {
                if !to.is_dir() {
                    return Err(HistoryError::Missing(to.clone()));
                }
                let name = from.file_name().unwrap_or_default().to_string_lossy();
                files::rename_dir(to, &name)?;
                Ok(())
            }
        }
    }

    /// Refait ce pas (défait plus tôt).
    fn redo(&mut self, convention: Convention) -> Result<()> {
        match self {
            Step::Notes(changes) => set_notes(changes, convention, false),
            Step::Versions {
                image,
                before,
                after,
                ..
            } => set_versions(image, before, after, convention),
            Step::Rename(plan) => {
                for (from, _) in &plan.moves {
                    if !from.exists() {
                        return Err(HistoryError::Missing(from.clone()));
                    }
                }
                Ok(files::rename(plan)?)
            }
            Step::Transfer { plan, written } => {
                for (from, _) in plan.files.iter().chain(&plan.shared) {
                    if !from.exists() {
                        return Err(HistoryError::Missing(from.clone()));
                    }
                }
                files::transfer(plan, |_, _| {})?;
                *written = match transfer_step(plan.clone()) {
                    Step::Transfer { written, .. } => written,
                    _ => Vec::new(),
                };
                Ok(())
            }
            Step::TransferDir { plan, written } => {
                if !plan.from.is_dir() {
                    return Err(HistoryError::Missing(plan.from.clone()));
                }
                let dest = plan.to.parent().unwrap_or(Path::new("."));
                let again = files::plan_transfer_dir(&plan.from, dest, plan.mode)?;
                files::transfer_dir(&again, |_, _| {})?;
                if let Step::TransferDir {
                    plan: p,
                    written: w,
                } = transfer_dir_step(again)
                {
                    *plan = p;
                    *written = w;
                }
                Ok(())
            }
            Step::Trash(items) => {
                let group = files::Group {
                    images: items.iter().map(|t| t.original.clone()).collect(),
                    sidecars: Vec::new(),
                    shared: Vec::new(),
                };
                for p in &group.images {
                    if !p.exists() {
                        return Err(HistoryError::Missing(p.clone()));
                    }
                }
                *items = match group.images.as_slice() {
                    [one] if one.is_dir() => files::delete_dir(one, true)?,
                    _ => files::delete(&group, true)?,
                };
                Ok(())
            }
            Step::CreateDir(path) => {
                let parent = path.parent().unwrap_or(Path::new("."));
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                files::create_dir(parent, &name)?;
                Ok(())
            }
            Step::RenameDir(from, to) => {
                if !from.is_dir() {
                    return Err(HistoryError::Missing(from.clone()));
                }
                let name = to.file_name().unwrap_or_default().to_string_lossy();
                files::rename_dir(from, &name)?;
                Ok(())
            }
        }
    }
}

/// Remet les notes d'avant (`undo`) ou d'après : seulement si celles du disque sont
/// encore celles laissées par le pas (sinon : modifiées ailleurs, refus).
fn set_notes(changes: &[NoteChange], convention: Convention, undo: bool) -> Result<()> {
    let images: Vec<&Path> = changes.iter().map(|c| c.image.as_path()).collect();
    let mut book = NoteBook::load(images, convention);
    for c in changes {
        if let Some(e) = book.error(&c.image) {
            return Err(HistoryError::Notes(e.to_string()));
        }
        let expected = if undo { &c.after } else { &c.before };
        if book.notes(&c.image) != expected {
            return Err(HistoryError::NotesChanged(c.image.clone()));
        }
    }
    for c in changes {
        let wanted = if undo { &c.before } else { &c.after };
        book.write_only(&c.image, wanted)
            .map_err(|e| HistoryError::Notes(e.to_string()))?;
    }
    Ok(())
}

/// Remet la liste des versions de `image` à `wanted`, si elle vaut bien `expected` (sinon
/// modifiée ailleurs depuis : refusé, rien n'est écrasé).
fn set_versions(
    image: &Path,
    expected: &[String],
    wanted: &[String],
    convention: Convention,
) -> Result<()> {
    let current = crate::xmp::read_list_of(image, crate::xmp::VERSIONS)
        .map_err(|e| HistoryError::Notes(e.to_string()))?;
    if current != expected {
        return Err(HistoryError::NotesChanged(image.to_path_buf()));
    }
    crate::xmp::write_list(image, crate::xmp::VERSIONS, wanted, convention)
        .map_err(|e| HistoryError::Notes(e.to_string()))?;
    Ok(())
}

/// Un pas, et s'il est en vigueur.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Le pas.
    pub step: Step,
    /// Description (calculée une fois : les fichiers peuvent changer de nom ensuite).
    pub label: String,
}

/// L'historique de la session : pas faits (les plus anciens d'abord), et pas défaits
/// qu'on peut refaire.
#[derive(Debug, Default)]
pub struct History {
    entries: Vec<Entry>,
    /// Nombre de pas en vigueur : `entries[..position]` faits, le reste défait.
    position: usize,
    /// Nombre de pas gardés au plus (les plus anciens sont oubliés).
    max: usize,
}

/// Résultat d'une annulation ou d'un rétablissement.
#[derive(Clone, Debug)]
pub struct Applied {
    /// Description du pas.
    pub label: String,
    /// Défait (sinon refait).
    pub undone: bool,
    /// Images concernées, à montrer.
    pub images: Vec<PathBuf>,
    /// Des fichiers ont bougé (dossier à relire), pas seulement des notes.
    pub files_changed: bool,
}

impl History {
    /// Historique vide, `max` pas au plus.
    pub fn new(max: usize) -> History {
        History {
            entries: Vec::new(),
            position: 0,
            max: max.max(1),
        }
    }

    /// Ajoute un pas fait : les pas défaits ne peuvent plus être refaits.
    pub fn push(&mut self, step: Step) {
        self.entries.truncate(self.position);
        let label = step.describe();
        self.entries.push(Entry { step, label });
        if self.entries.len() > self.max {
            let extra = self.entries.len() - self.max;
            self.entries.drain(..extra);
        }
        self.position = self.entries.len();
    }

    /// Les pas, les plus anciens d'abord.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Nombre de pas en vigueur (les suivants sont défaits).
    pub fn position(&self) -> usize {
        self.position
    }

    /// Vrai si un pas peut être défait.
    pub fn can_undo(&self) -> bool {
        self.position > 0
    }

    /// Vrai si un pas peut être refait.
    pub fn can_redo(&self) -> bool {
        self.position < self.entries.len()
    }

    /// Défait le dernier pas en vigueur. Refusé (fichiers modifiés ailleurs…) : le pas
    /// reste en vigueur.
    pub fn undo(&mut self, convention: Convention) -> Result<Applied> {
        if !self.can_undo() {
            return Err(HistoryError::Nothing);
        }
        let entry = &mut self.entries[self.position - 1];
        entry.step.undo(convention)?;
        self.position -= 1;
        Ok(applied(entry, true))
    }

    /// Refait le premier pas défait.
    pub fn redo(&mut self, convention: Convention) -> Result<Applied> {
        if !self.can_redo() {
            return Err(HistoryError::Nothing);
        }
        let entry = &mut self.entries[self.position];
        entry.step.redo(convention)?;
        self.position += 1;
        Ok(applied(entry, false))
    }

    /// Revient à `position` pas en vigueur (liste de l'historique), en défaisant ou
    /// refaisant un à un ; s'arrête au premier refus. Renvoie les pas appliqués.
    pub fn go_to(
        &mut self,
        position: usize,
        convention: Convention,
    ) -> (Vec<Applied>, Option<HistoryError>) {
        let position = position.min(self.entries.len());
        let mut done = Vec::new();
        while self.position != position {
            let result = if self.position > position {
                self.undo(convention)
            } else {
                self.redo(convention)
            };
            match result {
                Ok(a) => done.push(a),
                Err(e) => return (done, Some(e)),
            }
        }
        (done, None)
    }
}

fn applied(entry: &Entry, undone: bool) -> Applied {
    Applied {
        label: entry.label.clone(),
        undone,
        images: entry.step.images(undone),
        files_changed: !matches!(entry.step, Step::Notes(_) | Step::Versions { .. }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::FileSettings;

    fn touch(dir: &Path, names: &[&str]) {
        for n in names {
            std::fs::write(dir.join(n), n).unwrap();
        }
    }

    #[test]
    fn notes_are_undone_and_redone_unless_changed_elsewhere() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), &["A.RAF", "A.JPG"]);
        let raf = dir.path().join("A.RAF");
        let mut book = NoteBook::load(
            [raf.as_path(), dir.path().join("A.JPG").as_path()],
            Convention::Darktable,
        );
        let before = snapshot(&book, std::slice::from_ref(&raf));
        assert_eq!(before.len(), 2, "la paire aussi");
        book.edit(&raf, |n| n.set_stars(3)).unwrap();
        let step = notes_step(&book, before).unwrap();
        assert!(step.describe().contains("3"), "{}", step.describe());
        let mut history = History::new(10);
        history.push(step);
        let undone = history.undo(Convention::Darktable).unwrap();
        assert!(undone.undone && !undone.files_changed);
        book.reload();
        assert_eq!(book.notes(&raf).stars, 0);
        assert_eq!(book.notes(&dir.path().join("A.JPG")).stars, 0);
        history.redo(Convention::Darktable).unwrap();
        book.reload();
        assert_eq!(book.notes(&raf).stars, 3);
        // Modifié ailleurs (2 étoiles) : refus, rien ne bouge.
        book.edit(&raf, |n| n.set_stars(2)).unwrap();
        assert!(matches!(
            history.undo(Convention::Darktable),
            Err(HistoryError::NotesChanged(_))
        ));
        book.reload();
        assert_eq!(book.notes(&raf).stars, 2);
        assert_eq!(history.position(), 1, "le pas reste en vigueur");
    }

    #[test]
    fn a_move_is_undone_and_a_modified_copy_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let (src, dest) = (dir.path().join("src"), dir.path().join("dest"));
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        touch(&src, &["A.RAF", "A.RAF.xmp", "B.RAF"]);
        let s = FileSettings::default();
        let mut history = History::new(10);
        // Déplacement, puis annulation : tout revient.
        let plan =
            files::plan_transfer(&[src.join("A.RAF")], &dest, TransferMode::Move, &s).unwrap();
        files::transfer(&plan, |_, _| {}).unwrap();
        history.push(transfer_step(plan));
        let a = history.undo(Convention::Darktable).unwrap();
        assert!(a.files_changed);
        assert!(src.join("A.RAF").exists() && src.join("A.RAF.xmp").exists());
        assert!(!dest.join("A.RAF").exists());
        history.redo(Convention::Darktable).unwrap();
        assert!(dest.join("A.RAF").exists() && !src.join("A.RAF").exists());
        // Copie modifiée ailleurs : son annulation est refusée.
        let plan =
            files::plan_transfer(&[src.join("B.RAF")], &dest, TransferMode::Copy, &s).unwrap();
        files::transfer(&plan, |_, _| {}).unwrap();
        history.push(transfer_step(plan));
        std::fs::write(dest.join("B.RAF"), "retouchée ailleurs").unwrap();
        assert!(matches!(
            history.undo(Convention::Darktable),
            Err(HistoryError::Modified(_))
        ));
        assert!(dest.join("B.RAF").exists());
    }

    #[test]
    fn a_rename_is_undone_and_go_to_walks_the_list() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), &["A.RAF", "A.RAF.xmp"]);
        let s = FileSettings::default();
        let mut history = History::new(10);
        for name in ["B", "C"] {
            let current = std::fs::read_dir(dir.path())
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|e| e == "RAF"))
                .unwrap();
            let plan = files::plan_rename(&current, name, &s).unwrap();
            files::rename(&plan).unwrap();
            history.push(Step::Rename(plan));
        }
        assert!(dir.path().join("C.RAF").exists());
        let (applied, error) = history.go_to(0, Convention::Darktable);
        assert!(error.is_none());
        assert_eq!(applied.len(), 2);
        assert!(dir.path().join("A.RAF").exists() && dir.path().join("A.RAF.xmp").exists());
        let (_, error) = history.go_to(2, Convention::Darktable);
        assert!(error.is_none());
        assert!(dir.path().join("C.RAF").exists());
        // Un nouveau pas après une annulation efface les pas défaits.
        history.undo(Convention::Darktable).unwrap();
        assert!(history.can_redo());
        history.push(Step::Notes(vec![NoteChange {
            image: dir.path().join("B.RAF"),
            before: Notes::default(),
            after: Notes::default(),
        }]));
        assert!(!history.can_redo());
        assert_eq!(history.entries().len(), 2);
    }

    #[test]
    fn a_folder_copy_and_move_are_undone_and_redone() {
        let dir = tempfile::tempdir().unwrap();
        let seance = dir.path().join("Séance");
        let archive = dir.path().join("Archive");
        std::fs::create_dir_all(seance.join("JPEG")).unwrap();
        std::fs::create_dir(&archive).unwrap();
        touch(&seance, &["A.RAF", "A.RAF.xmp"]);
        touch(&seance.join("JPEG"), &["A.JPG"]);
        let mut history = History::new(10);
        // Copie : défaite, la copie disparaît ; refaite, elle revient.
        let plan = files::plan_transfer_dir(&seance, &archive, TransferMode::Copy).unwrap();
        files::transfer_dir(&plan, |_, _| {}).unwrap();
        history.push(transfer_dir_step(plan));
        let copy = archive.join("Séance");
        assert!(copy.join("JPEG/A.JPG").exists() && seance.join("A.RAF").exists());
        history.undo(Convention::Darktable).unwrap();
        assert!(!copy.exists());
        history.redo(Convention::Darktable).unwrap();
        assert!(copy.join("A.RAF.xmp").exists());
        // Un fichier ajouté dans la copie depuis : elle n'est pas retirée.
        touch(&copy, &["B.RAF"]);
        assert!(matches!(
            history.undo(Convention::Darktable),
            Err(HistoryError::Modified(_))
        ));
        assert!(copy.join("B.RAF").exists());
        std::fs::remove_dir_all(&copy).unwrap();
        // Déplacement : défait, le dossier revient à sa place.
        let mut history = History::new(10);
        let plan = files::plan_transfer_dir(&seance, &archive, TransferMode::Move).unwrap();
        files::transfer_dir(&plan, |_, _| {}).unwrap();
        let applied_label = transfer_dir_step(plan.clone()).describe();
        assert!(applied_label.contains("Séance"), "{applied_label}");
        history.push(transfer_dir_step(plan));
        assert!(!seance.exists() && archive.join("Séance/JPEG/A.JPG").exists());
        history.undo(Convention::Darktable).unwrap();
        assert!(seance.join("JPEG/A.JPG").exists() && !archive.join("Séance").exists());
        history.redo(Convention::Darktable).unwrap();
        assert!(!seance.exists() && archive.join("Séance/A.RAF").exists());
    }

    #[test]
    fn folder_steps_are_undone_but_a_filled_folder_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let mut history = History::new(10);
        let made = files::create_dir(dir.path(), "Nouveau").unwrap();
        history.push(Step::CreateDir(made.clone()));
        let renamed = files::rename_dir(&made, "Séance").unwrap();
        history.push(Step::RenameDir(made.clone(), renamed.clone()));
        history.undo(Convention::Darktable).unwrap();
        assert!(made.is_dir() && !renamed.exists());
        history.undo(Convention::Darktable).unwrap();
        assert!(!made.exists());
        history.redo(Convention::Darktable).unwrap();
        assert!(made.is_dir());
        // Rempli depuis : la création n'est plus défaite.
        std::fs::write(made.join("photo.jpg"), "x").unwrap();
        assert!(matches!(
            history.undo(Convention::Darktable),
            Err(HistoryError::Modified(_))
        ));
        assert!(made.join("photo.jpg").exists());
    }

    #[test]
    fn the_history_is_capped() {
        let mut history = History::new(2);
        for _ in 0..3 {
            history.push(Step::Trash(Vec::new()));
        }
        assert_eq!(history.entries().len(), 2);
        assert_eq!(history.position(), 2);
    }

    #[test]
    fn a_version_change_is_undone_and_redone_unless_changed_elsewhere() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("DSCF0001.RAF");
        std::fs::write(&image, b"raw").unwrap();
        let conv = Convention::Darktable;
        let after = vec![r#"{"name":"Story"}"#.to_string()];
        crate::xmp::write_list(&image, crate::xmp::VERSIONS, &after, conv).unwrap();
        let mut history = History::new(10);
        history.push(Step::Versions {
            image: image.clone(),
            before: Vec::new(),
            after: after.clone(),
            change: VersionChange::Created("Story".into()),
        });
        let read = || crate::xmp::read_list_of(&image, crate::xmp::VERSIONS).unwrap();
        let undone = history.undo(conv).unwrap();
        assert!(!undone.files_changed);
        assert!(read().is_empty(), "défaite");
        history.redo(conv).unwrap();
        assert_eq!(read(), after, "refaite");
        // Modifiée ailleurs depuis : refus, rien d'écrasé.
        let elsewhere = vec![r#"{"name":"Autre"}"#.to_string()];
        crate::xmp::write_list(&image, crate::xmp::VERSIONS, &elsewhere, conv).unwrap();
        assert!(matches!(
            history.undo(conv),
            Err(HistoryError::NotesChanged(_))
        ));
        assert_eq!(read(), elsewhere);
    }
}
