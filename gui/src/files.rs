//! Boîtes des actions sur les fichiers : renommer, mettre à la corbeille. Le travail est
//! fait par le moteur ([`tri::files`]) ; ici, seulement la saisie, la liste des fichiers
//! touchés et la confirmation.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Key, Pos2, ScrollArea};
use tri::files::{self, FileSettings, Group};

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
}

/// Résultat d'une image de la boîte.
pub enum Outcome {
    /// Boîte toujours ouverte.
    Open,
    /// Annulée.
    Cancelled,
    /// Fichiers modifiés ; `select` : image à sélectionner ensuite (nouveau nom).
    Done { select: Option<PathBuf> },
    /// Échec (message pour la barre d'état) ; la boîte est fermée.
    Failed(String),
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

    /// Affiche la boîte et applique l'action confirmée.
    pub fn show(&mut self, ctx: &egui::Context, settings: &FileSettings) -> Outcome {
        match self {
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
            Ok(()) => Outcome::Done { select },
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
            Ok(()) => Outcome::Done { select: None },
            Err(e) => Outcome::Failed(t!("delete-failed", error = e.to_string())),
        };
    }
    if cancel || modal.should_close() {
        Outcome::Cancelled
    } else {
        Outcome::Open
    }
}
