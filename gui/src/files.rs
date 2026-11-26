//! Boîtes des actions sur les fichiers : renommer, mettre à la corbeille, copier /
//! déplacer vers un dossier. Le travail est fait par le moteur ([`tri::files`]) ; ici,
//! seulement la saisie, la liste des fichiers touchés et la confirmation.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;

use eframe::egui::{self, Key, Modifiers, Pos2, ScrollArea};
use tri::files::{self, DeletePlan, FileError, FileSettings, TransferMode};

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
    /// Renommer plusieurs photos d'un coup, par un gabarit (#439).
    RenameBatch(Box<crate::rename_batch::BatchDialog>),
    /// Confirmer la suppression de `images` (`plan` : avec leurs paires si `pairs`, case
    /// montrée quand `has_pairs` ; `chosen` : cases des développements, quand il y en a
    /// plusieurs).
    Delete {
        images: Vec<PathBuf>,
        pairs: bool,
        has_pairs: bool,
        plan: DeletePlan,
        chosen: Vec<bool>,
        at: Pos2,
        /// Suppression lancée (confirmée).
        job: Option<Deleting>,
    },
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
    /// Nommer le favori `path` (vide : le nom du dossier).
    NameFavorite {
        path: PathBuf,
        name: String,
        at: Pos2,
        fresh: bool,
    },
    /// Exporter (profil, dossier) : voir [`crate::export`].
    Export(Box<crate::export::ExportDialog>),
    /// Confirmer la suppression des dossiers `paths` (#606 : plusieurs d'un coup) ;
    /// (fichiers, sous-dossiers) dedans.
    DeleteFolder {
        paths: Vec<PathBuf>,
        contents: (usize, usize),
        at: Pos2,
    },
}

/// Ce qu'on copie ou déplace.
pub enum Source {
    /// Des images (paires et sidecars suivent).
    Images(Vec<PathBuf>),
    /// Des dossiers entiers (panneau des dossiers ; plusieurs d'un coup, #606).
    Folders(Vec<PathBuf>),
}

/// Reprend le cache des images copiées ou déplacées (#508) : (avant, après) de chaque
/// fichier, et vrai pour une copie. Fourni par l'application (cache et réglages du
/// moteur), appelé dans le fil de la copie.
pub type Carrier = Arc<dyn Fn(&[(PathBuf, PathBuf)], bool) + Send + Sync>;

/// Plan d'une copie ou d'un déplacement, selon la source.
enum Plan {
    Images(files::Transfer),
    Folders(Vec<files::DirTransfer>),
}

impl Plan {
    fn of(
        source: &Source,
        dest: &Path,
        mode: TransferMode,
        settings: &FileSettings,
    ) -> Result<Plan, FileError> {
        match source {
            Source::Images(images) => {
                files::plan_transfer(images, dest, mode, settings).map(Plan::Images)
            }
            Source::Folders(dirs) => dirs
                .iter()
                .map(|dir| files::plan_transfer_dir(dir, dest, mode))
                .collect::<Result<Vec<_>, _>>()
                .map(Plan::Folders),
        }
    }

    fn len(&self) -> usize {
        match self {
            Plan::Images(p) => p.len(),
            Plan::Folders(p) => p.iter().map(files::DirTransfer::len).sum(),
        }
    }

    /// Exécute le plan ; `progress` : (faits, total).
    fn run(&self, progress: impl FnMut(usize, usize)) -> Result<(), FileError> {
        match self {
            Plan::Images(p) => files::transfer(p, progress),
            Plan::Folders(plans) => {
                let total = self.len();
                let mut progress = progress;
                let mut before = 0;
                for p in plans {
                    files::transfer_dir(p, |done, _| progress(before + done, total))?;
                    before += p.len();
                }
                Ok(())
            }
        }
    }

    /// (avant, après) de chaque fichier copié ou déplacé.
    fn files(&self) -> Vec<(PathBuf, PathBuf)> {
        match self {
            Plan::Images(p) => p.files.iter().chain(&p.shared).cloned().collect(),
            Plan::Folders(p) => p.iter().flat_map(|p| p.files.iter().cloned()).collect(),
        }
    }
}

/// Boîte « Copier / déplacer vers… ».
pub struct Transfer {
    source: Source,
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
    /// Paires RAW+JPEG suivies (case de la boîte ; `None` : `[files] pairs`), et si la
    /// sélection en a (case montrée), calculé à la première image.
    pairs: Option<bool>,
    has_pairs: Option<bool>,
    /// Dossier déplacé : son cache le suit (coché d'office).
    bring_cache: bool,
    /// Images copiées ou déplacées, dossier copié : leurs aperçus déjà calculés suivent
    /// (#508 ; case de la boîte, `None` : `[files] carry_cache`), par `carrier`.
    carry: Option<bool>,
    carrier: Option<Carrier>,
    /// Et il sera gardé avec le dossier à sa nouvelle place ([`CacheGoes::WithFolder`]),
    /// décidé dans la boîte (avant le déplacement).
    keep_here: bool,
    /// Copie en cours mise en arrière-plan (issue n° 72) : plus de boîte, la progression
    /// est dans la barre d'état.
    hidden: bool,
}

/// Suppression en cours, dans un fil à part : une centaine d'images à la corbeille prend
/// plusieurs secondes (issue n° 43).
pub struct Deleting {
    done: Arc<AtomicUsize>,
    total: usize,
    handle: Option<JoinHandle<Result<files::Deleted, FileError>>>,
}

/// Copie en cours, dans un fil à part (copies vérifiées : longues vers un autre disque).
struct Job {
    done: Arc<AtomicUsize>,
    total: usize,
    dest: PathBuf,
    /// Le fil rend le plan exécuté (pour l'historique).
    handle: Option<JoinHandle<Result<Plan, FileError>>>,
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
        renamed: Vec<(PathBuf, PathBuf)>,
        /// Renommé ou déplacé : son cache le suit (case « Emporter le cache »).
        cache_follows: bool,
        /// Et il est gardé avec le dossier à sa nouvelle place ([`CacheGoes::WithFolder`]).
        keep_here: bool,
    },
    /// Nom donné au favori `path` (vide : retiré).
    FavoriteNamed { path: PathBuf, name: String },
    /// Échec (message pour la barre d'état) ; la boîte est fermée.
    Failed(String),
    /// Export demandé (lancé en fond par l'application).
    Export {
        images: Vec<PathBuf>,
        profile: tri::export::Profile,
        folder: String,
        /// Dossier à retenir pour ce préréglage (vide : l'oublier).
        remember: String,
        /// Gabarit de nom à retenir pour ce préréglage (vide : l'oublier ; #485).
        remember_name: String,
        render: crate::export::Render,
        cut: Option<tri::export::Cut>,
        /// Les versions des images plutôt qu'elles, voir [`crate::export::VersionsExport`].
        versions: Option<crate::export::VersionsExport>,
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
        // Un clone darktable se renomme avec son image d'origine (jamais séparés).
        let image = tri::media::source(&image).into_owned();
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
            images: images.to_vec(),
            pairs: settings.pairs,
            has_pairs: files::adds_pairs(images),
            plan: files::plan_delete(images, settings),
            chosen: vec![true; images.len()],
            at,
            job: None,
        }
    }

    /// Copie mise en arrière-plan (issue n° 72) : l'application la garde à part, la
    /// boîte n'est plus affichée.
    pub fn is_hidden(&self) -> bool {
        matches!(self, FileDialog::Transfer(t) if t.hidden)
    }

    /// Met une copie en cours en arrière-plan.
    pub fn hide(&mut self) {
        if let FileDialog::Transfer(t) = self {
            t.hidden = true;
        }
    }

    /// Réaffiche une copie mise en arrière-plan.
    pub fn reveal(&mut self) {
        if let FileDialog::Transfer(t) = self {
            t.hidden = false;
        }
    }

    /// Avancement d'une copie, pour la barre d'état : (texte, fraction).
    pub fn transfer_status(&self) -> Option<(String, f32)> {
        let FileDialog::Transfer(t) = self else {
            return None;
        };
        let job = t.job.as_ref()?;
        let done = job.done.load(Ordering::Relaxed);
        let text = t!(
            "transfer-status",
            name = file_name(&job.dest),
            done = done,
            total = job.total
        );
        Some((text, done as f32 / job.total.max(1) as f32))
    }

    /// Copie ou suppression en cours : la boîte ne se ferme pas.
    pub fn busy(&self) -> bool {
        match self {
            FileDialog::Transfer(t) => t.job.is_some(),
            FileDialog::Delete { job, .. } => job.is_some(),
            _ => false,
        }
    }

    /// Boîte « Copier / déplacer vers… » pour `source` (des images ou un dossier) ;
    /// `dest` : destination déjà remplie (glisser-déposer), sinon à saisir ;
    /// destinations récentes lues dans le dossier de données `data`.
    pub fn transfer(
        source: Source,
        mode: TransferMode,
        dest: Option<&Path>,
        data: Option<&Path>,
        at: Pos2,
    ) -> FileDialog {
        let recents = data.map(files::recent_destinations).unwrap_or_default();
        FileDialog::Transfer(Box::new(Transfer {
            source,
            mode,
            input: dest.map(tilde).unwrap_or_default(),
            at,
            fresh: true,
            field: Default::default(),
            recents,
            recent: None,
            error: None,
            job: None,
            pairs: None,
            has_pairs: None,
            bring_cache: true,
            carry: None,
            carrier: None,
            keep_here: false,
            hidden: false,
        }))
    }

    /// Donne à la boîte de copie de quoi reprendre le cache des images (#508).
    pub fn with_carrier(mut self, carrier: Option<Carrier>) -> FileDialog {
        if let FileDialog::Transfer(t) = &mut self {
            t.carrier = carrier;
        }
        self
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
            FileDialog::RenameBatch(b) => b.show(ctx, settings),
            FileDialog::Export(e) => match e.show(ctx) {
                crate::export::Outcome::Open => Outcome::Open,
                crate::export::Outcome::Cancelled => Outcome::Cancelled,
                crate::export::Outcome::Start {
                    images,
                    profile,
                    folder,
                    render,
                    cut,
                    remember,
                    remember_name,
                    versions,
                } => Outcome::Export {
                    images,
                    profile: *profile,
                    folder,
                    remember,
                    remember_name,
                    render,
                    cut,
                    versions,
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
                    false,
                    check,
                ) {
                    Some(true) => match files::create_dir(parent, name) {
                        Ok(path) => Outcome::Folder {
                            step: Some(tri::history::Step::CreateDir(path.clone())),
                            point: Some(path),
                            renamed: Vec::new(),
                            cache_follows: false,
                            keep_here: false,
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
                    true,
                    check,
                ) {
                    Some(true) => match files::rename_dir(path, name) {
                        Ok(to) => Outcome::Folder {
                            step: Some(tri::history::Step::RenameDir(path.clone(), to.clone())),
                            point: Some(to.clone()),
                            renamed: vec![(path.clone(), to)],
                            cache_follows: true,
                            keep_here: false,
                        },
                        Err(e) => Outcome::Failed(t!("folder-failed", error = e.to_string())),
                    },
                    Some(false) => Outcome::Cancelled,
                    None => Outcome::Open,
                }
            }
            FileDialog::DeleteFolder {
                paths,
                contents,
                at,
            } => delete_folder_dialog(ctx, settings, paths, *contents, *at),
            FileDialog::NameFavorite {
                path,
                name,
                at,
                fresh,
            } => {
                let title = t!("favorite-name-title", name = file_name(path));
                // Tout nom convient ; vide : le nom du dossier revient.
                match name_dialog(
                    ctx,
                    "name_favorite",
                    title,
                    name,
                    *at,
                    fresh,
                    &t!("favorite-name-button"),
                    false,
                    |_| None,
                ) {
                    Some(true) => Outcome::FavoriteNamed {
                        path: path.clone(),
                        name: name.trim().to_string(),
                    },
                    Some(false) => Outcome::Cancelled,
                    None => Outcome::Open,
                }
            }
            FileDialog::Rename {
                image,
                name,
                at,
                fresh,
            } => rename_dialog(ctx, settings, image, name, *at, fresh),
            FileDialog::Delete {
                images,
                pairs,
                has_pairs,
                plan,
                chosen,
                at,
                job,
            } => {
                if job.is_some() {
                    return deleting(ctx, settings, plan, *at, job);
                }
                let before = (*pairs, chosen.clone());
                let outcome = delete_dialog(
                    ctx,
                    settings,
                    plan,
                    images,
                    chosen,
                    *at,
                    has_pairs.then_some(pairs),
                    job,
                );
                if (*pairs, &*chosen) != (before.0, &before.1) {
                    let kept: Vec<PathBuf> = images
                        .iter()
                        .zip(chosen.iter())
                        .filter(|(_, on)| **on)
                        .map(|(p, _)| p.clone())
                        .collect();
                    *plan = files::plan_delete(&kept, &with_pairs(settings, *pairs));
                }
                outcome
            }
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
            library_warning(ui);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let ok = plan.is_ok() && !unchanged;
                confirm = ui
                    .add_enabled(ok, egui::Button::new(t!("rename-button")))
                    .clicked()
                    || (enter && ok);
                cancel = ui.button(t!("cancel")).clicked();
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

/// Les réglages `settings`, paires RAW+JPEG suivies ou non selon la case de la boîte.
fn with_pairs(settings: &FileSettings, pairs: bool) -> FileSettings {
    FileSettings {
        pairs,
        ..settings.clone()
    }
}

/// Case « avec l'autre fichier de la paire RAW+JPEG » (retour du 21/10/2026 : supprimer ou
/// copier tous les JPEG emportait les RAW).
fn pairs_checkbox(ui: &mut egui::Ui, pairs: &mut bool) {
    ui.checkbox(pairs, t!("files-with-pairs"))
        .on_hover_text(t!("files-with-pairs-tip"));
}

/// Titre de la boîte de suppression.
fn delete_title(settings: &FileSettings, plan: &DeletePlan) -> String {
    let n = plan.count();
    if only_strips(plan) {
        t!("delete-developments-question", n = n)
    } else if settings.trash {
        t!("delete-trash-question", n = n)
    } else {
        t!("delete-permanently-question", n = n)
    }
}

/// Vrai si rien ne part : seulement des XMP darktable réécrits sans leur développement.
fn only_strips(plan: &DeletePlan) -> bool {
    plan.group.files().next().is_none()
}

/// Lance la suppression de `plan` dans un fil à part.
fn start_delete(ctx: &egui::Context, settings: &FileSettings, plan: &DeletePlan) -> Deleting {
    let done = Arc::new(AtomicUsize::new(0));
    let (counter, ctx, plan, trash) = (done.clone(), ctx.clone(), plan.clone(), settings.trash);
    let total = plan.count();
    let handle = std::thread::spawn(move || {
        files::delete_plan(&plan, trash, |n, _| {
            counter.store(n, Ordering::Relaxed);
            ctx.request_repaint();
        })
    });
    Deleting {
        done,
        total,
        handle: Some(handle),
    }
}

/// Suppression en cours : progression ; à la fin, le pas de l'historique.
fn deleting(
    ctx: &egui::Context,
    settings: &FileSettings,
    plan: &DeletePlan,
    at: Pos2,
    job: &mut Option<Deleting>,
) -> Outcome {
    let Some(running) = job else {
        return Outcome::Open;
    };
    if running.handle.as_ref().is_some_and(|h| h.is_finished()) {
        let result = running.handle.take().map(|h| h.join());
        *job = None;
        return match result {
            Some(Ok(Ok(deleted))) => Outcome::Done {
                select: None,
                // Un XMP réécrit se défait toujours ; des fichiers, seulement depuis la
                // corbeille.
                step: if deleted.rewritten.is_empty() {
                    settings
                        .trash
                        .then_some(tri::history::Step::Trash(deleted.trashed))
                } else {
                    Some(tri::history::Step::Developments(deleted))
                },
            },
            Some(Ok(Err(e))) => Outcome::Failed(t!("delete-failed", error = e.to_string())),
            _ => Outcome::Failed(t!("delete-failed", error = "?")),
        };
    }
    let (done, total) = (running.done.load(Ordering::Relaxed), running.total);
    crate::app::modal_at("delete_dialog", at)
        .title(delete_title(settings, plan))
        .busy()
        .show(ctx, |ui| {
            ui.add(
                egui::ProgressBar::new(done as f32 / total.max(1) as f32).text(t!(
                    "transfer-progress",
                    done = done,
                    total = total
                )),
            );
        });
    ctx.request_repaint_after(crate::app::poll_interval(ctx));
    Outcome::Open
}

/// Rappel, dans une boîte qui renomme, déplace ou supprime (actions autorisées par
/// `[files] allow_actions`) : la bibliothèque d'un logiciel de développement ne
/// retrouvera plus ces fichiers (demandé le 30/10/2026).
pub(crate) fn library_warning(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    let text = egui::RichText::new(t!("files-library-warning")).color(ui.visuals().warn_fg_color);
    ui.add(egui::Label::new(text).wrap());
}

/// Boîte de confirmation ; confirmée, la suppression part en fond (`job`).
#[allow(clippy::too_many_arguments)]
fn delete_dialog(
    ctx: &egui::Context,
    settings: &FileSettings,
    plan: &DeletePlan,
    images: &[PathBuf],
    chosen: &mut [bool],
    at: Pos2,
    pairs: Option<&mut bool>,
    job: &mut Option<Deleting>,
) -> Outcome {
    let (mut confirm, mut cancel) = (false, false);
    let only_strips = only_strips(plan);
    let verb = if only_strips {
        t!("delete-developments")
    } else if settings.trash {
        t!("delete-to-trash")
    } else {
        t!("delete-permanently")
    };
    let ok = plan.count() > 0;
    let modal = crate::app::modal_at("delete_dialog", at)
        .title(delete_title(settings, plan))
        .show(ctx, |ui| {
            ui.add_space(6.0);
            // Plusieurs développements demandés : une case par développement, cochée
            // (30/10/2026) ; décocher l'un le garde. Pas de case pour un développement
            // dont la photo part aussi : son sidecar suit la photo de toute façon.
            let photos: std::collections::HashSet<&PathBuf> = images
                .iter()
                .filter(|image| tri::developments::get(image).is_none())
                .collect();
            let alone = |image: &PathBuf| {
                tri::developments::get(image).is_some_and(|d| !photos.contains(&d.photo))
            };
            if images.len() > 1 && images.iter().any(&alone) {
                for (image, on) in images.iter().zip(chosen.iter_mut()) {
                    if alone(image) {
                        ui.checkbox(on, crate::app::shown_name(image));
                    }
                }
                ui.add_space(4.0);
            }
            ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                for rewrite in &plan.stripped {
                    ui.weak(t!("delete-stripped", name = file_name(&rewrite.sidecar)));
                }
                for file in plan.group.files() {
                    ui.weak(file_name(file));
                }
            });
            for file in &plan.group.shared {
                ui.weak(t!("delete-kept-shared", name = file_name(file)));
            }
            if let Some(pairs) = pairs {
                ui.add_space(4.0);
                pairs_checkbox(ui, pairs);
            }
            library_warning(ui);
            if !settings.trash && !only_strips {
                ui.colored_label(ui.visuals().error_fg_color, t!("delete-irreversible"));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                confirm = ui.add_enabled(ok, egui::Button::new(verb)).clicked();
                cancel = ui.button(t!("cancel")).clicked();
            });
        });
    // Entrée confirme (pas de champ de saisie ici).
    confirm |= ok && ctx.input(|i| i.key_pressed(Key::Enter));
    if confirm {
        *job = Some(start_delete(ctx, settings, plan));
        return Outcome::Open;
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
    warn: bool,
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
        if warn {
            library_warning(ui);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let ok = error.is_none();
            confirm = ui.add_enabled(ok, egui::Button::new(button)).clicked() || (enter && ok);
            cancel = ui.button(t!("cancel")).clicked();
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
    paths: &[PathBuf],
    (files_in, dirs_in): (usize, usize),
    at: Pos2,
) -> Outcome {
    let (mut confirm, mut cancel) = (false, false);
    let title = match (paths, settings.trash) {
        ([one], true) => t!("folder-delete-trash", name = file_name(one)),
        ([one], false) => t!("folder-delete-permanently", name = file_name(one)),
        (_, true) => t!("folders-delete-trash", n = paths.len()),
        (_, false) => t!("folders-delete-permanently", n = paths.len()),
    };
    let modal = crate::app::modal_at("delete_folder", at)
        .title(title)
        .show(ctx, |ui| {
            ui.add_space(6.0);
            // Plusieurs dossiers : leurs noms (#606).
            if paths.len() > 1 {
                for path in paths.iter().take(8) {
                    ui.label(format!("· {}", file_name(path)));
                }
                if paths.len() > 8 {
                    ui.weak(t!("folders-more", n = paths.len() - 8));
                }
            }
            ui.label(t!("folder-contents", files = files_in, dirs = dirs_in));
            library_warning(ui);
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
            });
        });
    confirm |= ctx.input(|i| i.key_pressed(Key::Enter));
    if confirm {
        return match files::delete_dirs(paths, settings.trash) {
            Ok(trashed) => Outcome::Folder {
                step: settings.trash.then_some(tri::history::Step::Trash(trashed)),
                point: paths[0].parent().map(Path::to_path_buf),
                renamed: Vec::new(),
                cache_follows: false,
                keep_here: false,
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
        let has_pairs = *self.has_pairs.get_or_insert_with(|| match &self.source {
            Source::Images(images) => files::adds_pairs(images),
            Source::Folders(_) => false,
        });
        let mut pairs = self.pairs.unwrap_or(settings.pairs);
        let settings = &with_pairs(settings, pairs);
        let title = self.title();
        let dest = tri::settings::expand_home(self.input.trim());
        let plan = Plan::of(&self.source, &dest, self.mode, settings);
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
                    Ok(Plan::Images(plan)) => {
                        ui.label(t!("transfer-files", n = plan.files.len()));
                        for (from, _) in &plan.shared {
                            ui.weak(t!("transfer-shared", name = file_name(from)));
                        }
                    }
                    Ok(Plan::Folders(plans)) => {
                        ui.label(t!(
                            "folder-contents",
                            files = plans.iter().map(|p| p.files.len()).sum::<usize>(),
                            dirs = plans.iter().map(|p| p.dirs.len() - 1).sum::<usize>()
                        ));
                    }
                    Err(_) if create => {
                        ui.label(t!("transfer-create"));
                    }
                    Err(_) if self.input.trim().is_empty() => {}
                    Err(e) => {
                        ui.colored_label(ui.visuals().error_fg_color, e.to_string());
                    }
                }
                if has_pairs {
                    pairs_checkbox(ui, &mut pairs);
                }
                // Copier ne casse rien ; déplacer, si (30/10/2026).
                if self.mode == TransferMode::Move {
                    library_warning(ui);
                }
                // Images, ou dossier copié : leurs aperçus déjà calculés suivent (#508).
                let folder_move =
                    matches!(self.source, Source::Folders(_)) && self.mode == TransferMode::Move;
                if !folder_move && self.carrier.is_some() {
                    let mut carry = self.carry.unwrap_or(settings.carry_cache);
                    ui.checkbox(&mut carry, t!("transfer-carry-cache"))
                        .on_hover_text(t!("transfer-carry-cache-tip"));
                    if carry != settings.carry_cache || self.carry.is_some() {
                        self.carry = Some(carry);
                    }
                }
                // Dossier déplacé : son cache le suit, sur demande (issue n° 55).
                if folder_move {
                    ui.checkbox(&mut self.bring_cache, t!("transfer-bring-cache"))
                        .on_hover_text(t!("transfer-bring-cache-tip"));
                    // Où il ira, selon où il est et ce que garde la destination (retour du
                    // 19/11/2026 : déplacé d'une clé vers iCloud, il était resté sur le Mac).
                    if let Ok(Plan::Folders(plans)) = &plan
                        && let Some(plan) = plans.first()
                    {
                        let goes = cache_goes(&plan.from, &plan.to);
                        self.keep_here = goes == CacheGoes::WithFolder;
                        let text = match (self.bring_cache, goes) {
                            (false, _) => t!("transfer-cache-dropped"),
                            (true, CacheGoes::Computer) => t!("transfer-cache-computer"),
                            (true, CacheGoes::Kept(root)) => {
                                t!("transfer-cache-kept", place = tilde(&root))
                            }
                            (true, CacheGoes::WithFolder) => t!("transfer-cache-with-folder"),
                        };
                        ui.weak(text);
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
                    // « Valider » : le verbe n'est qu'à un endroit, le choix déplacer /
                    // copier (retour du 20/10/2026 : trois « Déplacer » dans la boîte).
                    let label = t!("transfer-confirm");
                    confirm =
                        ui.add_enabled(ok, egui::Button::new(label)).clicked() || (enter && ok);
                    cancel = ui.button(t!("cancel")).clicked();
                });
            });
        if pairs != settings.pairs {
            self.pairs = Some(pairs);
        }
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

    /// Titre de la boîte, sans verbe (le choix déplacer / copier est en dessous) :
    /// « 3 images vers… », « Dossier « Séance » vers… ».
    fn title(&self) -> String {
        match &self.source {
            Source::Images(images) => t!("transfer-title", n = images.len()),
            Source::Folders(dirs) => match dirs.as_slice() {
                [one] => t!("transfer-folder-title", name = file_name(one)),
                _ => t!("transfer-folders-title", n = dirs.len()),
            },
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
        let plan = match Plan::of(&self.source, dest, self.mode, settings) {
            Ok(plan) => plan,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        let done = Arc::new(AtomicUsize::new(0));
        let total = plan.len();
        let (counter, ctx) = (done.clone(), ctx.clone());
        // Un dossier déplacé emmène son cache autrement (« Emporter le cache »).
        let folder_move =
            matches!(self.source, Source::Folders(_)) && self.mode == TransferMode::Move;
        let carrier = self
            .carrier
            .clone()
            .filter(|_| !folder_move && self.carry.unwrap_or(settings.carry_cache));
        let copy = self.mode == TransferMode::Copy;
        let handle = std::thread::spawn(move || {
            plan.run(|n, _| {
                counter.store(n, Ordering::Relaxed);
                ctx.request_repaint();
            })?;
            if let Some(carry) = carrier {
                carry(&plan.files(), copy);
            }
            Ok(plan)
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
                    match plan {
                        Plan::Images(plan) => Outcome::Done {
                            select: None,
                            step: Some(tri::history::transfer_step(plan)),
                        },
                        // Dossier déplacé : comme un renommage (favoris, dossier ouvert
                        // dedans) ; pointé à sa nouvelle place.
                        Plan::Folders(plans) => Outcome::Folder {
                            point: plans.first().map(|p| p.to.clone()),
                            renamed: plans
                                .iter()
                                .filter(|p| p.mode == TransferMode::Move)
                                .map(|p| (p.from.clone(), p.to.clone()))
                                .collect(),
                            step: {
                                let mut steps: Vec<tri::history::Step> = plans
                                    .into_iter()
                                    .map(tri::history::transfer_dir_step)
                                    .collect();
                                match steps.len() {
                                    0 => None,
                                    1 => steps.pop(),
                                    _ => Some(tri::history::Step::Many(steps)),
                                }
                            },
                            cache_follows: self.bring_cache,
                            keep_here: self.bring_cache && self.keep_here,
                        },
                    }
                }
                Some(Ok(Err(e))) => Outcome::Failed(t!("transfer-failed", error = e.to_string())),
                _ => Outcome::Failed(t!("transfer-failed", error = "?")),
            };
        }
        ctx.request_repaint_after(crate::app::poll_interval(ctx));
        if self.hidden {
            return Outcome::Open;
        }
        let done = job.done.load(Ordering::Relaxed);
        let title = self.title();
        let Some(job) = &self.job else {
            return Outcome::Open;
        };
        let mut hide = false;
        crate::app::modal_at("transfer_dialog", self.at)
            .title(title)
            .busy()
            .show(ctx, |ui| {
                ui.label(job.dest.display().to_string());
                ui.add(
                    egui::ProgressBar::new(done as f32 / job.total.max(1) as f32).text(t!(
                        "transfer-progress",
                        done = done,
                        total = job.total
                    )),
                );
                // Pour continuer à trier pendant une longue copie (issue n° 72).
                ui.add_space(6.0);
                hide = ui.button(t!("transfer-background")).clicked();
            });
        self.hidden = hide;
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

/// Où va le cache d'un dossier déplacé de `from` en `to` (case « Emporter le cache »).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CacheGoes {
    /// Dans le cache de cet ordinateur : il y était, et la destination ne garde pas le sien.
    Computer,
    /// Dans le cache gardé avec les photos de la destination, à cette racine.
    Kept(PathBuf),
    /// Il était gardé avec les photos, la destination ne garde pas le sien : gardé avec
    /// le dossier à sa nouvelle place (« Garder les données de tri dans ce dossier »).
    WithFolder,
}

/// [`CacheGoes`] pour `from` déplacé en `to`.
pub(crate) fn cache_goes(from: &Path, to: &Path) -> CacheGoes {
    if let Some(root) = to.parent().and_then(tri::store::holder) {
        return CacheGoes::Kept(root);
    }
    if tri::store::holder(from).is_some() {
        CacheGoes::WithFolder
    } else {
        CacheGoes::Computer
    }
}

#[cfg(test)]
impl FileDialog {
    /// Copie de `images` vers `dest` déjà lancée, qui n'avance qu'une fois `go` reçu
    /// (tests de la copie en arrière-plan, sans dépendre de sa vitesse).
    pub fn transfer_held(
        images: Vec<PathBuf>,
        dest: &Path,
        settings: &FileSettings,
        go: std::sync::mpsc::Receiver<()>,
    ) -> FileDialog {
        let mut dialog = FileDialog::transfer(
            Source::Images(images),
            TransferMode::Copy,
            Some(dest),
            None,
            Pos2::ZERO,
        );
        if let FileDialog::Transfer(t) = &mut dialog {
            let plan = Plan::of(&t.source, dest, t.mode, settings).unwrap();
            let total = plan.len();
            let handle = std::thread::spawn(move || {
                let _ = go.recv();
                plan.run(|_, _| {}).map(|()| plan)
            });
            t.job = Some(Job {
                done: Arc::new(AtomicUsize::new(0)),
                total,
                dest: dest.to_path_buf(),
                handle: Some(handle),
            });
        }
        dialog
    }
}
