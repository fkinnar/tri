//! Boîtes des actions sur les fichiers : renommer, mettre à la corbeille, copier /
//! déplacer vers un dossier. Le travail est fait par le moteur ([`tri::files`]) ; ici,
//! seulement la saisie, la liste des fichiers touchés et la confirmation.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;

use eframe::egui::{self, Key, Modifiers, Pos2, ScrollArea};
use tri::files::{self, FileError, FileSettings, Group, TransferMode};

/// Boîte ouverte.
pub enum FileDialog {
    /// Renommer `image` ; `name` : nouveau nom sans extension, en cours de saisie.
    Rename {
        image: PathBuf,
        name: String,
        at: Pos2,
        /// Première image : le champ prend le focus, nom sélectionné.
        fresh: bool,
    },
    /// Confirmer la suppression des fichiers de `group`.
    Delete { group: Group, at: Pos2 },
    /// Copier / déplacer vers un dossier.
    Transfer(Box<Transfer>),
    /// Créer un dossier dans `parent`.
    NewFolder {
        parent: PathBuf,
        name: String,
        at: Pos2,
        fresh: bool,
    },
    /// Renommer le dossier `path`.
    RenameFolder {
        path: PathBuf,
        name: String,
        at: Pos2,
        fresh: bool,
    },
    /// Exporter (profil, dossier) : voir [`crate::export`].
    Export(Box<crate::export::ExportDialog>),
    /// Confirmer la suppression du dossier `path` ; (fichiers, sous-dossiers) dedans.
    DeleteFolder {
        path: PathBuf,
        contents: (usize, usize),
        at: Pos2,
    },
}

/// Boîte « Copier / déplacer vers… ».
pub struct Transfer {
    images: Vec<PathBuf>,
    mode: TransferMode,
    /// Dossier de destination, en cours de saisie (`~` accepté).
    input: String,
    at: Pos2,
    /// Première image : le champ prend le focus.
    fresh: bool,
    /// Champ du chemin (complétion par Tab).
    field: crate::path_field::PathField,
    /// Destinations récentes (la plus récente d'abord), et celle montrée par les flèches.
    recents: Vec<PathBuf>,
    recent: Option<usize>,
    /// Erreur au lancement (création du dossier…), montrée dans la boîte.
    error: Option<String>,
    job: Option<Job>,
}

/// Copie en cours, dans un fil à part (copies vérifiées : longues vers un autre disque).
struct Job {
    done: Arc<AtomicUsize>,
    total: usize,
    dest: PathBuf,
    /// Le fil rend le plan exécuté (pour l'historique).
    handle: Option<JoinHandle<Result<files::Transfer, FileError>>>,
}

/// Résultat d'une image de la boîte.
pub enum Outcome {
    /// Boîte toujours ouverte.
    Open,
    /// Annulée.
    Cancelled,
    /// Fichiers modifiés ; `select` : image à sélectionner ensuite (nouveau nom) ; `step` :
    /// le pas à mettre dans l'historique (aucun pour une suppression définitive).
    Done {
        select: Option<PathBuf>,
        step: Option<tri::history::Step>,
    },
    /// Dossier créé, renommé ou supprimé : `step` pour l'historique ; `point` : dossier à
    /// pointer dans l'arborescence (créé, ou nouveau nom) ; `renamed` : (ancien, nouveau).
    Folder {
        step: Option<tri::history::Step>,
        point: Option<PathBuf>,
        renamed: Option<(PathBuf, PathBuf)>,
    },
    /// Échec (message pour la barre d'état) ; la boîte est fermée.
    Failed(String),
    /// Export demandé (lancé en fond par l'application) ; `choice` : profil choisi, retenu
    /// pour la prochaine fois.
    Export {
        images: Vec<PathBuf>,
        profile: tri::export::Profile,
        folder: String,
        render: crate::export::Render,
        choice: usize,
    },
}

/// Identifiant du champ du nouveau nom.
pub fn name_field_id() -> egui::Id {
    egui::Id::unique("rename_field")
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl FileDialog {
    /// Boîte « Renommer » pour `image`, nom actuel proposé.
    pub fn rename(image: PathBuf, at: Pos2) -> FileDialog {
        let name = image
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        FileDialog::Rename {
            image,
            name,
            at,
            fresh: true,
        }
    }

    /// Boîte de confirmation de la suppression de `images` (paires et sidecars compris).
    pub fn delete(images: &[PathBuf], settings: &FileSettings, at: Pos2) -> FileDialog {
        FileDialog::Delete {
            group: files::plan_delete(images, settings),
            at,
        }
    }

    /// Copie en cours : la boîte ne se ferme pas.
    pub fn busy(&self) -> bool {
        matches!(self, FileDialog::Transfer(t) if t.job.is_some())
    }

    /// Boîte « Copier / déplacer vers… » pour `images` ; destinations récentes lues dans
    /// le dossier de données `data`.
    pub fn transfer(
        images: Vec<PathBuf>,
        mode: TransferMode,
        data: Option<&Path>,
        at: Pos2,
    ) -> FileDialog {
        let recents = data.map(files::recent_destinations).unwrap_or_default();
        FileDialog::Transfer(Box::new(Transfer {
            images,
            mode,
            input: String::new(),
            at,
            fresh: true,
            field: Default::default(),
            recents,
            recent: None,
            error: None,
            job: None,
        }))
    }

    /// Affiche la boîte et applique l'action confirmée ; `data` : dossier de données
    /// (destinations récentes).
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        settings: &FileSettings,
        data: Option<&Path>,
    ) -> Outcome {
        match self {
            FileDialog::Transfer(t) => t.show(ctx, settings, data),
            FileDialog::Export(e) => match e.show(ctx) {
                crate::export::Outcome::Open => Outcome::Open,
                crate::export::Outcome::Cancelled => Outcome::Cancelled,
                crate::export::Outcome::Start {
                    images,
                    profile,
                    folder,
                    render,
                } => Outcome::Export {
                    images,
                    profile: *profile,
                    folder,
                    render,
                    choice: e.choice(),
                },
            },
            FileDialog::NewFolder {
                parent,
                name,
                at,
                fresh,
            } => {
                let title = t!("folder-new-title", name = file_name(parent));
                let check = |n: &str| {
                    let p = parent.join(n.trim());
                    (n.trim().is_empty() || p.exists()).then(|| FileError::Exists(p).to_string())
                };
                match name_dialog(
                    ctx,
                    "new_folder",
                    title,
                    name,
                    *at,
                    fresh,
                    &t!("folder-new-button"),
                    check,
                ) {
                    Some(true) => match files::create_dir(parent, name) {
                        Ok(path) => Outcome::Folder {
                            step: Some(tri::history::Step::CreateDir(path.clone())),
                            point: Some(path),
                            renamed: None,
                        },
                        Err(e) => Outcome::Failed(t!("folder-failed", error = e.to_string())),
                    },
                    Some(false) => Outcome::Cancelled,
                    None => Outcome::Open,
                }
            }
            FileDialog::RenameFolder {
                path,
                name,
                at,
                fresh,
            } => {
                let title = t!("folder-rename-title", name = file_name(path));
                let original = file_name(path);
                let check = |n: &str| {
                    let to = path.with_file_name(n.trim());
                    let taken = to.exists() && !n.trim().eq_ignore_ascii_case(&original);
                    (n.trim().is_empty() || taken).then(|| FileError::Exists(to).to_string())
                };
                match name_dialog(
                    ctx,
                    "rename_folder",
                    title,
                    name,
                    *at,
                    fresh,
                    &t!("rename-button"),
                    check,
                ) {
                    Some(true) => match files::rename_dir(path, name) {
                        Ok(to) => Outcome::Folder {
                            step: Some(tri::history::Step::RenameDir(path.clone(), to.clone())),
                            point: Some(to.clone()),
                            renamed: Some((path.clone(), to)),
                        },
                        Err(e) => Outcome::Failed(t!("folder-failed", error = e.to_string())),
                    },
                    Some(false) => Outcome::Cancelled,
                    None => Outcome::Open,
                }
            }
            FileDialog::DeleteFolder { path, contents, at } => {
                delete_folder_dialog(ctx, settings, path, *contents, *at)
            }
            FileDialog::Rename {
                image,
                name,
                at,
                fresh,
            } => rename_dialog(ctx, settings, image, name, *at, fresh),
            FileDialog::Delete { group, at } => delete_dialog(ctx, settings, group, *at),
        }
    }
}

fn rename_dialog(
    ctx: &egui::Context,
    settings: &FileSettings,
    image: &Path,
    name: &mut String,
    at: Pos2,
    fresh: &mut bool,
) -> Outcome {
    let plan = files::plan_rename(image, name, settings);
    let unchanged = plan
        .as_ref()
        .is_ok_and(|p| p.moves.is_empty() && p.copies.is_empty());
    let (mut confirm, mut cancel) = (false, false);
    let title = t!("rename-title", name = file_name(image));
    let modal = crate::app::modal_at("rename_dialog", at)
        .title(title)
        .show(ctx, |ui| {
            ui.add_space(6.0);
            let field = ui.add(
                egui::TextEdit::singleline(name)
                    .id(name_field_id())
                    .desired_width(320.0),
            );
            if std::mem::take(fresh) {
                field.request_focus();
                // Tout le nom sélectionné : on tape directement le nouveau.
                let id = name_field_id();
                let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                let all = egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(name.chars().count()),
                );
                state.cursor.set_char_range(Some(all));
                state.store(ctx, id);
            }
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            ui.add_space(6.0);
            match &plan {
                Ok(plan) => {
                    for (from, to) in &plan.moves {
                        ui.weak(t!(
                            "rename-becomes",
                            from = file_name(from),
                            to = file_name(to)
                        ));
                    }
                    for (from, to) in &plan.copies {
                        ui.weak(t!(
                            "rename-copied",
                            from = file_name(from),
                            to = file_name(to)
                        ));
                    }
                }
                Err(e) => {
                    ui.colored_label(ui.visuals().error_fg_color, e.to_string());
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let ok = plan.is_ok() && !unchanged;
                confirm = ui
                    .add_enabled(ok, egui::Button::new(t!("rename-button")))
                    .clicked()
                    || (enter && ok);
                cancel = ui.button(t!("cancel")).clicked();
                ui.weak(t!("enter-or-escape"));
            });
        });
    if confirm && let Ok(plan) = plan {
        let select = plan
            .moves
            .iter()
            .find(|(from, _)| from == image)
            .map(|(_, to)| to.clone());
        return match files::rename(&plan) {
            Ok(()) => Outcome::Done {
                select,
                step: Some(tri::history::Step::Rename(plan)),
            },
            Err(e) => Outcome::Failed(t!("rename-failed", error = e.to_string())),
        };
    }
    if cancel || modal.should_close() {
        Outcome::Cancelled
    } else {
        Outcome::Open
    }
}

fn delete_dialog(ctx: &egui::Context, settings: &FileSettings, group: &Group, at: Pos2) -> Outcome {
    let (mut confirm, mut cancel) = (false, false);
    let verb = if settings.trash {
        t!("delete-to-trash")
    } else {
        t!("delete-permanently")
    };
    let n = group.files().count();
    let title = if settings.trash {
        t!("delete-trash-question", n = n)
    } else {
        t!("delete-permanently-question", n = n)
    };
    let modal = crate::app::modal_at("delete_dialog", at)
        .title(title)
        .show(ctx, |ui| {
            ui.add_space(6.0);
            ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                for file in group.files() {
                    ui.weak(file_name(file));
                }
            });
            for file in &group.shared {
                ui.weak(t!("delete-kept-shared", name = file_name(file)));
            }
            if !settings.trash {
                ui.colored_label(ui.visuals().error_fg_color, t!("delete-irreversible"));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                confirm = ui.button(verb).clicked();
                cancel = ui.button(t!("cancel")).clicked();
                ui.weak(t!("enter-or-escape"));
            });
        });
    // Entrée confirme (pas de champ de saisie ici).
    confirm |= ctx.input(|i| i.key_pressed(Key::Enter));
    if confirm {
        return match files::delete(group, settings.trash) {
            Ok(trashed) => Outcome::Done {
                select: None,
                step: settings.trash.then_some(tri::history::Step::Trash(trashed)),
            },
            Err(e) => Outcome::Failed(t!("delete-failed", error = e.to_string())),
        };
    }
    if cancel || modal.should_close() {
        Outcome::Cancelled
    } else {
        Outcome::Open
    }
}

/// Boîte de saisie d'un nom (nouveau dossier, renommer un dossier) : `Some(true)`
/// confirmée, `Some(false)` annulée, `None` encore ouverte ; `check` donne l'erreur à
/// montrer pour un nom refusé.
#[allow(clippy::too_many_arguments)]
fn name_dialog(
    ctx: &egui::Context,
    id: &str,
    title: String,
    name: &mut String,
    at: Pos2,
    fresh: &mut bool,
    button: &str,
    check: impl Fn(&str) -> Option<String>,
) -> Option<bool> {
    let field_id = egui::Id::unique(id).with("field");
    let error = check(name);
    let (mut confirm, mut cancel) = (false, false);
    let modal = crate::app::modal_at(id, at).title(title).show(ctx, |ui| {
        ui.add_space(6.0);
        let field = ui.add(
            egui::TextEdit::singleline(name)
                .id(field_id)
                .desired_width(320.0),
        );
        if std::mem::take(fresh) {
            field.request_focus();
            let mut state = egui::TextEdit::load_state(ctx, field_id).unwrap_or_default();
            let all = egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(name.chars().count()),
            );
            state.cursor.set_char_range(Some(all));
            state.store(ctx, field_id);
        }
        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if let Some(e) = &error
            && !name.trim().is_empty()
        {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let ok = error.is_none();
            confirm = ui.add_enabled(ok, egui::Button::new(button)).clicked() || (enter && ok);
            cancel = ui.button(t!("cancel")).clicked();
            ui.weak(t!("enter-or-escape"));
        });
    });
    if confirm {
        Some(true)
    } else if cancel || modal.should_close() {
        Some(false)
    } else {
        None
    }
}

fn delete_folder_dialog(
    ctx: &egui::Context,
    settings: &FileSettings,
    path: &Path,
    (files_in, dirs_in): (usize, usize),
    at: Pos2,
) -> Outcome {
    let (mut confirm, mut cancel) = (false, false);
    let name = file_name(path);
    let title = if settings.trash {
        t!("folder-delete-trash", name = &name)
    } else {
        t!("folder-delete-permanently", name = &name)
    };
    let modal = crate::app::modal_at("delete_folder", at)
        .title(title)
        .show(ctx, |ui| {
            ui.add_space(6.0);
            ui.label(t!("folder-contents", files = files_in, dirs = dirs_in));
            if !settings.trash {
                ui.colored_label(ui.visuals().error_fg_color, t!("delete-irreversible"));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let verb = if settings.trash {
                    t!("delete-to-trash")
                } else {
                    t!("delete-permanently")
                };
                confirm = ui.button(verb).clicked();
                cancel = ui.button(t!("cancel")).clicked();
                ui.weak(t!("enter-or-escape"));
            });
        });
    confirm |= ctx.input(|i| i.key_pressed(Key::Enter));
    if confirm {
        return match files::delete_dir(path, settings.trash) {
            Ok(trashed) => Outcome::Folder {
                step: settings.trash.then_some(tri::history::Step::Trash(trashed)),
                point: path.parent().map(Path::to_path_buf),
                renamed: None,
            },
            Err(e) => Outcome::Failed(t!("folder-failed", error = e.to_string())),
        };
    }
    if cancel || modal.should_close() {
        Outcome::Cancelled
    } else {
        Outcome::Open
    }
}

/// Identifiant du champ de destination.
pub fn destination_field_id() -> egui::Id {
    egui::Id::unique("transfer_field")
}

impl Transfer {
    fn show(
        &mut self,
        ctx: &egui::Context,
        settings: &FileSettings,
        data: Option<&Path>,
    ) -> Outcome {
        if self.job.is_some() {
            return self.progress(ctx, settings, data);
        }
        let n = self.images.len();
        let title = match self.mode {
            TransferMode::Copy => t!("transfer-copy-title", n = n),
            TransferMode::Move => t!("transfer-move-title", n = n),
        };
        let dest = tri::settings::expand_home(self.input.trim());
        let plan = files::plan_transfer(&self.images, &dest, self.mode, settings);
        // Dossier absent dont le parent existe : il sera créé.
        let create = matches!(plan, Err(FileError::NotADirectory(_)))
            && !self.input.trim().is_empty()
            && dest.parent().is_some_and(Path::is_dir);
        let (mut confirm, mut cancel) = (false, false);
        let id = destination_field_id();
        let focused = ctx.memory(|m| m.has_focus(id));
        // Flèches pour les destinations récentes (Tab : le champ de chemin).
        let (up, down) = if focused {
            ctx.input_mut(|i| {
                (
                    i.consume_key(Modifiers::NONE, Key::ArrowUp),
                    i.consume_key(Modifiers::NONE, Key::ArrowDown),
                )
            })
        } else {
            (false, false)
        };
        if up && !self.recents.is_empty() {
            let i = self
                .recent
                .map_or(0, |i| (i + 1).min(self.recents.len() - 1));
            self.use_recent(i);
        }
        if down {
            match self.recent {
                Some(0) | None => {
                    self.recent = None;
                    self.input.clear();
                }
                Some(i) => self.use_recent(i - 1),
            }
        }
        let mut picked = None;
        let modal = crate::app::modal_at("transfer_dialog", self.at)
            .title(title)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.mode, TransferMode::Move, t!("transfer-move"));
                    ui.selectable_value(&mut self.mode, TransferMode::Copy, t!("transfer-copy"));
                });
                let field = self.field.show(ui, id, &mut self.input, 420.0);
                if std::mem::take(&mut self.fresh) {
                    field.request_focus();
                }
                if field.changed() {
                    self.recent = None;
                }
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.weak(t!("transfer-hint"));
                ui.add_space(6.0);
                match &plan {
                    Ok(plan) => {
                        ui.label(t!("transfer-files", n = plan.files.len()));
                        for (from, _) in &plan.shared {
                            ui.weak(t!("transfer-shared", name = file_name(from)));
                        }
                    }
                    Err(_) if create => {
                        ui.label(t!("transfer-create"));
                    }
                    Err(_) if self.input.trim().is_empty() => {}
                    Err(e) => {
                        ui.colored_label(ui.visuals().error_fg_color, e.to_string());
                    }
                }
                if let Some(e) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                if !self.recents.is_empty() {
                    ui.add_space(6.0);
                    ui.weak(t!("transfer-recent"));
                    for (i, recent) in self.recents.iter().enumerate() {
                        let text = tilde(recent);
                        if ui.selectable_label(self.recent == Some(i), text).clicked() {
                            picked = Some(i);
                        }
                    }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let ok = plan.is_ok() || create;
                    let label = match (self.mode, create) {
                        (TransferMode::Copy, false) => t!("transfer-copy"),
                        (TransferMode::Move, false) => t!("transfer-move"),
                        (TransferMode::Copy, true) => t!("transfer-create-copy"),
                        (TransferMode::Move, true) => t!("transfer-create-move"),
                    };
                    confirm =
                        ui.add_enabled(ok, egui::Button::new(label)).clicked() || (enter && ok);
                    cancel = ui.button(t!("cancel")).clicked();
                    ui.weak(t!("enter-or-escape"));
                });
            });
        if let Some(i) = picked {
            self.use_recent(i);
            ctx.memory_mut(|m| m.request_focus(id));
        }
        if confirm {
            self.start(ctx, settings, &dest, create);
            return Outcome::Open;
        }
        if cancel || modal.should_close() {
            Outcome::Cancelled
        } else {
            Outcome::Open
        }
    }

    fn use_recent(&mut self, i: usize) {
        if let Some(path) = self.recents.get(i) {
            self.recent = Some(i);
            self.input = tilde(path);
            self.field.replaced();
        }
    }

    /// Lance la copie dans un fil (dossier créé d'abord si besoin).
    fn start(&mut self, ctx: &egui::Context, settings: &FileSettings, dest: &Path, create: bool) {
        self.error = None;
        if create && let Err(e) = std::fs::create_dir(dest) {
            self.error = Some(format!("{} : {e}", dest.display()));
            return;
        }
        let plan = match files::plan_transfer(&self.images, dest, self.mode, settings) {
            Ok(plan) => plan,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        let done = Arc::new(AtomicUsize::new(0));
        let total = plan.len();
        let (counter, ctx) = (done.clone(), ctx.clone());
        let handle = std::thread::spawn(move || {
            files::transfer(&plan, |n, _| {
                counter.store(n, Ordering::Relaxed);
                ctx.request_repaint();
            })
            .map(|()| plan)
        });
        self.job = Some(Job {
            done,
            total,
            dest: dest.to_path_buf(),
            handle: Some(handle),
        });
    }

    /// Copie en cours : progression ; à la fin, destination mémorisée et dossier relu.
    fn progress(
        &mut self,
        ctx: &egui::Context,
        settings: &FileSettings,
        data: Option<&Path>,
    ) -> Outcome {
        let Some(job) = &mut self.job else {
            return Outcome::Open;
        };
        if job.handle.as_ref().is_some_and(|h| h.is_finished()) {
            let result = job.handle.take().map(|h| h.join());
            return match result {
                Some(Ok(Ok(plan))) => {
                    if let Some(data) = data {
                        let _ = files::remember_destination(data, &job.dest, settings.recent);
                    }
                    Outcome::Done {
                        select: None,
                        step: Some(tri::history::transfer_step(plan)),
                    }
                }
                Some(Ok(Err(e))) => Outcome::Failed(t!("transfer-failed", error = e.to_string())),
                _ => Outcome::Failed(t!("transfer-failed", error = "?")),
            };
        }
        let done = job.done.load(Ordering::Relaxed);
        let title = match self.mode {
            TransferMode::Copy => t!("transfer-copy-title", n = self.images.len()),
            TransferMode::Move => t!("transfer-move-title", n = self.images.len()),
        };
        crate::app::modal_at("transfer_dialog", self.at)
            .title(title)
            .show(ctx, |ui| {
                ui.label(job.dest.display().to_string());
                ui.add(
                    egui::ProgressBar::new(done as f32 / job.total.max(1) as f32).text(t!(
                        "transfer-progress",
                        done = done,
                        total = job.total
                    )),
                );
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        Outcome::Open
    }
}

/// `path` avec `~` pour le dossier personnel (plus court, et accepté à la saisie).
pub fn tilde(path: &Path) -> String {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    match home.and_then(|h| path.strip_prefix(h).ok().map(Path::to_path_buf)) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => path.display().to_string(),
    }
}
