//! Boîte du renommage par lot (#439, décidé le 14/11/2026) : un gabarit
//! ([`tri::naming`]), le premier numéro, et la liste des anciens et nouveaux noms en
//! direct ; un nom pris ou donné deux fois en rouge, « Renommer » grisé tant qu'il en
//! reste. Le travail est fait par le moteur ([`tri::files::plan_batch`]).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use eframe::egui::{self, Key, Pos2, ScrollArea};
use tri::files::{BatchRename, FileSettings};
use tri::naming::{self, Fields};

use crate::files::Outcome;

/// Renommer plusieurs photos d'un coup.
pub struct BatchDialog {
    /// Les photos, dans l'ordre de la grille (le compteur suit cet ordre).
    photos: Vec<PathBuf>,
    /// Gabarit en cours de saisie.
    pub template: String,
    /// Premier numéro du compteur `{n}`.
    start: usize,
    /// Champs de chaque photo : nom et dossier d'abord, puis les métadonnées lues en fond.
    fields: Vec<Fields>,
    /// Lecture des métadonnées en cours.
    reading: Option<Receiver<Vec<Fields>>>,
    /// Plan pour (gabarit, numéro, métadonnées lues) : recalculé seulement quand ils
    /// changent (chaque plan relit le dossier).
    planned: Option<((String, usize, bool), BatchRename)>,
    at: Pos2,
    fresh: bool,
}

impl BatchDialog {
    /// Boîte pour `photos` ([`tri::files::photos`]) ; `template` : le dernier utilisé.
    pub fn new(photos: Vec<PathBuf>, template: String, at: Pos2) -> Self {
        let fields = photos.iter().map(|p| Fields::basic(p)).collect();
        let (tx, rx) = mpsc::channel();
        let list = photos.clone();
        std::thread::spawn(move || {
            let read: Vec<Fields> = list.iter().map(|p| Fields::read(p)).collect();
            let _ = tx.send(read);
        });
        BatchDialog {
            photos,
            template,
            start: 1,
            fields,
            reading: Some(rx),
            planned: None,
            at,
            fresh: true,
        }
    }

    /// Les métadonnées sont-elles lues ?
    fn read(&mut self) -> bool {
        if let Some(rx) = &self.reading
            && let Ok(fields) = rx.try_recv()
        {
            self.fields = fields;
            self.reading = None;
        }
        self.reading.is_none()
    }

    /// Métadonnées lues et prises en compte par le dernier plan (tests).
    #[cfg(test)]
    pub fn is_read(&self) -> bool {
        self.planned.as_ref().is_some_and(|((_, _, read), _)| *read)
    }

    /// Le plan pour le gabarit et le numéro actuels.
    fn plan(&mut self, settings: &FileSettings) -> &BatchRename {
        let read = self.read();
        let key = (self.template.clone(), self.start, read);
        if self.planned.as_ref().is_none_or(|(k, _)| *k != key) {
            let stems = self
                .fields
                .iter()
                .enumerate()
                .map(|(i, f)| naming::render(&self.template, f, self.start + i))
                .collect();
            let plan = tri::files::plan_batch(&self.photos, stems, settings);
            self.planned = Some((key, plan));
        }
        &self.planned.as_ref().expect("calculé").1
    }

    /// Affiche la boîte.
    pub fn show(&mut self, ctx: &egui::Context, settings: &FileSettings) -> Outcome {
        let read = self.read();
        if !read {
            // Lecture en fond : la boîte se redessine pour la voir finir.
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        let waiting = !read && naming::needs_metadata(&self.template);
        let template_error = naming::check(&self.template).err();
        let batch = self.plan(settings).clone();
        let (mut confirm, mut cancel) = (false, false);
        let title = t!("batch-rename-title", n = self.photos.len());
        let at = self.at;
        let modal = crate::app::modal_at("batch_rename_dialog", at)
            .title(title)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                let mut enter = false;
                ui.horizontal(|ui| {
                    ui.label(t!("batch-rename-template"));
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut self.template)
                            .id(crate::files::name_field_id())
                            .desired_width(300.0),
                    );
                    if std::mem::take(&mut self.fresh) {
                        field.request_focus();
                    }
                    enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    ui.label(t!("batch-rename-start"));
                    ui.add(egui::DragValue::new(&mut self.start).range(0..=999_999));
                });
                // Les champs, cliquables : ajoutés au bout du gabarit.
                ui.horizontal_wrapped(|ui| {
                    ui.weak(t!("batch-rename-fields"));
                    for field in naming::FIELDS {
                        let text = if field == "n" {
                            "{n:3}".to_string()
                        } else {
                            format!("{{{field}}}")
                        };
                        let tip = t!("batch-rename-field", field = field);
                        if ui.small_button(&text).on_hover_text(tip).clicked() {
                            self.template.push_str(&text);
                        }
                    }
                });
                if let Some(e) = &template_error {
                    ui.colored_label(ui.visuals().error_fg_color, e.to_string());
                }
                ui.add_space(6.0);
                if waiting {
                    ui.weak(t!("batch-rename-reading"));
                }
                rows(ui, &batch, waiting);
                crate::files::library_warning(ui);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let ok = !waiting
                        && batch
                            .plan
                            .as_ref()
                            .is_some_and(|p| !p.moves.is_empty() || !p.copies.is_empty());
                    confirm = ui
                        .add_enabled(ok, egui::Button::new(t!("rename-button")))
                        .clicked()
                        || (enter && ok);
                    cancel = ui.button(t!("cancel")).clicked();
                });
            });
        if confirm && let Some(plan) = batch.plan {
            let select = batch
                .rows
                .first()
                .and_then(|row| plan.moves.iter().find(|(from, _)| *from == row.image))
                .map(|(_, to)| to.clone());
            return match tri::files::rename(&plan) {
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
}

/// Une ligne par photo : « ancien devient nouveau », en rouge avec la raison si elle bloque.
fn rows(ui: &mut egui::Ui, batch: &BatchRename, waiting: bool) {
    let name = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let others = batch
        .plan
        .as_ref()
        .map_or(0, |p| p.moves.len() + p.copies.len())
        .saturating_sub(batch.rows.len());
    ScrollArea::vertical()
        .max_height(320.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for row in &batch.rows {
                let from = name(&row.image);
                let ext = row
                    .image
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy()))
                    .unwrap_or_default();
                let to = match (&row.stem, waiting) {
                    (_, true) => "…".to_string(),
                    (Some(stem), _) => format!("{stem}{ext}"),
                    (None, _) => "—".to_string(),
                };
                let line = t!("batch-rename-row", from = from.as_str(), to = to.as_str());
                match (&row.problem, waiting) {
                    (Some(problem), false) => {
                        let error = ui.visuals().error_fg_color;
                        ui.colored_label(error, format!("{line} · {problem}"));
                    }
                    _ => {
                        ui.label(line);
                    }
                }
            }
        });
    if others > 0 && !waiting {
        ui.weak(t!("batch-rename-others", n = others));
    }
}
