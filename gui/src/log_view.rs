//! Fenêtre du journal de l'application (issue n° 398, « pour les fous comme moi ») :
//! `tri.log` relu ([`tri::log::read`]), les entrées les plus récentes d'abord, filtre par
//! gravité, recherche floue, détail déplié au clic, copie d'une entrée ou de tout,
//! « Afficher dans le gestionnaire de fichiers ». Ouverte depuis la palette, l'aide, ou un
//! clic sur une erreur de la barre d'état : ses entrées sont alors mises en avant et
//! dépliées.

use std::collections::HashSet;
use std::path::PathBuf;

use eframe::egui::{self, Color32};
use tri::log::{Entry, Level};

/// Entrées montrées au plus (les plus récentes) : un journal plein en compte des milliers.
pub const MAX_SHOWN: usize = 500;

/// Gravités montrées.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    /// Erreurs et plantages.
    Errors,
    /// Avertissements, erreurs et plantages (par défaut).
    Warnings,
    /// Tout, informations et trace comprises.
    All,
}

impl Shown {
    fn keeps(self, level: Level) -> bool {
        match self {
            Shown::Errors => level >= Level::Error,
            Shown::Warnings => level >= Level::Warning,
            Shown::All => true,
        }
    }

    fn label(self) -> String {
        t!(match self {
            Shown::Errors => "log-shown-errors",
            Shown::Warnings => "log-shown-warnings",
            Shown::All => "log-shown-all",
        })
    }
}

/// Ce que la fenêtre demande à l'application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogAction {
    /// Montrer le journal dans le gestionnaire de fichiers.
    Reveal(PathBuf),
    /// Fermer la fenêtre.
    Close,
}

/// État de la fenêtre.
pub struct LogView {
    /// Dossier du journal (`None` : journal inactif, pas de dossier de données).
    pub dir: Option<PathBuf>,
    /// Entrées, les plus récentes d'abord.
    pub entries: Vec<Entry>,
    /// Recherche floue.
    pub query: String,
    /// Gravités montrées.
    pub shown: Shown,
    /// Entrées mises en avant (celles de l'erreur cliquée), dépliées à l'ouverture.
    pub focus: Vec<String>,
    /// Entrées dépliées (identifiants).
    pub open: HashSet<String>,
    /// Faire défiler jusqu'à la première entrée mise en avant (une fois).
    scroll: bool,
    /// Première image : le clavier va à la recherche.
    fresh: bool,
}

impl LogView {
    /// Relit le journal de `dir` ; `focus` : entrées à mettre en avant.
    pub fn open(dir: Option<PathBuf>, focus: Vec<String>) -> LogView {
        let entries = dir.as_deref().map(tri::log::read).unwrap_or_default();
        // Une entrée mise en avant moins grave que le filtre (avertissement) : tout montrer.
        let shown = if entries
            .iter()
            .any(|e| focus.contains(&e.id) && e.level < Level::Warning)
        {
            Shown::All
        } else {
            Shown::Warnings
        };
        LogView {
            dir,
            entries,
            query: String::new(),
            shown,
            open: focus.iter().cloned().collect(),
            scroll: !focus.is_empty(),
            focus,
            fresh: true,
        }
    }

    /// Relit le fichier (bouton « Actualiser »), filtres gardés.
    pub fn refresh(&mut self) {
        self.entries = self.dir.as_deref().map(tri::log::read).unwrap_or_default();
    }

    /// Entrées montrées : gravité et recherche, ordre du journal (les plus récentes
    /// d'abord), au plus [`MAX_SHOWN`].
    pub fn visible(&self) -> Vec<&Entry> {
        let query = self.query.trim();
        let found: Vec<(&Entry, tri::fuzzy::Match)> = self
            .entries
            .iter()
            .filter(|e| self.shown.keeps(e.level) || self.focus.contains(&e.id))
            .filter_map(|e| {
                let found = tri::fuzzy::find(
                    query,
                    &[&e.message, &e.source, &e.detail, &e.time, e.level.tag()],
                )?;
                Some((e, found))
            })
            .collect();
        let mut kept: Vec<&Entry> = if query.is_empty() {
            found.into_iter().map(|(e, _)| e).collect()
        } else {
            tri::fuzzy::relevant(found)
                .into_iter()
                .map(|(e, _)| e)
                .collect()
        };
        kept.truncate(MAX_SHOWN);
        kept
    }

    /// Dessine la fenêtre.
    pub fn show(&mut self, ctx: &egui::Context, accent: Color32) -> Option<LogAction> {
        let mut action = None;
        let size = ctx.content_rect().size();
        let at = ctx.content_rect().center() - egui::vec2(340.0, size.y * 0.35);
        let modal = crate::app::modal_at("app_log", at)
            .title(t!("log-title"))
            .show(ctx, |ui| {
                ui.set_width(680.0);
                ui.horizontal(|ui| {
                    let search = ui.add(
                        egui::TextEdit::singleline(&mut self.query)
                            .hint_text(t!("log-search-hint"))
                            .desired_width(300.0),
                    );
                    if std::mem::take(&mut self.fresh) {
                        search.request_focus();
                    }
                    egui::ComboBox::from_id_salt("app_log_shown")
                        .selected_text(self.shown.label())
                        .show_ui(ui, |ui| {
                            for shown in [Shown::Errors, Shown::Warnings, Shown::All] {
                                ui.selectable_value(&mut self.shown, shown, shown.label());
                            }
                        });
                    if ui.button(t!("log-refresh")).clicked() {
                        self.refresh();
                    }
                });
                ui.add_space(4.0);
                let visible: Vec<Entry> = self.visible().into_iter().cloned().collect();
                if self.dir.is_none() {
                    ui.weak(t!("log-inactive"));
                } else if visible.is_empty() {
                    ui.weak(t!(if self.entries.is_empty() {
                        "log-empty"
                    } else {
                        "log-no-match"
                    }));
                }
                egui::ScrollArea::vertical()
                    .max_height(size.y * 0.6)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for entry in &visible {
                            self.entry(ui, entry, accent);
                        }
                    });
                if visible.len() == MAX_SHOWN {
                    ui.weak(t!("log-truncated", n = MAX_SHOWN));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!visible.is_empty(), egui::Button::new(t!("log-copy-all")))
                        .on_hover_text(t!("log-copy-all-tip"))
                        .clicked()
                    {
                        let text: String = visible.iter().rev().map(Entry::to_text).collect();
                        ui.ctx().copy_text(text);
                    }
                    if let Some(path) = self.dir.as_ref().map(|d| d.join(tri::log::FILE))
                        && ui
                            .add_enabled(
                                path.exists(),
                                egui::Button::new(crate::actions::Action::FileReveal.label()),
                            )
                            .on_hover_text(crate::files::tilde(&path))
                            .clicked()
                    {
                        action = Some(LogAction::Reveal(path));
                    }
                });
            });
        if modal.should_close() {
            action = Some(LogAction::Close);
        }
        action
    }

    /// Une entrée : en-tête (cliquable : déplie le détail), détail en chasse fixe.
    fn entry(&mut self, ui: &mut egui::Ui, entry: &Entry, accent: Color32) {
        let focused = self.focus.contains(&entry.id);
        let open = self.open.contains(&entry.id);
        let frame = egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(4, 2))
            .corner_radius(3.0)
            .fill(if focused {
                accent.gamma_multiply(0.18)
            } else {
                Color32::TRANSPARENT
            });
        let response = frame
            .show(ui, |ui| {
                let header = ui
                    .horizontal_wrapped(|ui| {
                        ui.weak(egui::RichText::new(&entry.time).monospace());
                        let color = level_color(ui, entry.level);
                        ui.label(
                            egui::RichText::new(entry.level.tag())
                                .monospace()
                                .strong()
                                .color(color),
                        );
                        if !entry.source.is_empty() {
                            ui.weak(&entry.source);
                        }
                        let size = ui.text_style_height(&egui::TextStyle::Body);
                        let job = crate::app::marked_job(
                            &entry.message,
                            &self.query,
                            size * 0.9,
                            ui.visuals().text_color(),
                            accent,
                        );
                        ui.label(job);
                    })
                    .response
                    .interact(egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(t!(if entry.detail.is_empty() {
                        "log-entry-copy-tip"
                    } else {
                        "log-entry-tip"
                    }));
                if header.clicked() && !self.open.remove(&entry.id) {
                    self.open.insert(entry.id.clone());
                }
                if open {
                    if !entry.detail.is_empty() {
                        ui.add(
                            egui::Label::new(egui::RichText::new(&entry.detail).monospace())
                                .wrap()
                                .selectable(true),
                        );
                    }
                    if ui
                        .small_button(t!("log-copy"))
                        .on_hover_text(t!("log-copy-tip"))
                        .clicked()
                    {
                        ui.ctx().copy_text(entry.to_text());
                    }
                }
            })
            .response;
        if focused && std::mem::take(&mut self.scroll) {
            response.scroll_to_me(Some(egui::Align::TOP));
        }
    }
}

/// Couleur d'une gravité.
fn level_color(ui: &egui::Ui, level: Level) -> Color32 {
    match level {
        Level::Crash | Level::Error => ui.visuals().error_fg_color,
        Level::Warning => ui.visuals().warn_fg_color,
        Level::Info | Level::Trace => ui.visuals().weak_text_color(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, level: Level, message: &str) -> Entry {
        Entry {
            id: id.into(),
            time: "2026-11-10 15:03:12.345".into(),
            level,
            source: "develop".into(),
            message: message.into(),
            detail: String::new(),
        }
    }

    fn view(entries: Vec<Entry>) -> LogView {
        let mut view = LogView::open(None, Vec::new());
        view.entries = entries;
        view
    }

    #[test]
    fn warnings_and_errors_are_shown_by_default_newest_first() {
        let view = view(vec![
            entry("1-3", Level::Error, "darktable failed"),
            entry("1-2", Level::Info, "started"),
            entry("1-1", Level::Warning, "thumbnail failed"),
        ]);
        let ids: Vec<&str> = view.visible().iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["1-3", "1-1"]);
    }

    #[test]
    fn the_search_and_the_level_filter_narrow_the_list() {
        let mut view = view(vec![
            entry("1-3", Level::Error, "darktable failed"),
            entry("1-2", Level::Warning, "thumbnail failed"),
            entry("1-1", Level::Crash, "index out of bounds"),
        ]);
        view.query = "darktable".into();
        assert_eq!(view.visible().len(), 1);
        view.query.clear();
        view.shown = Shown::Errors;
        let ids: Vec<&str> = view.visible().iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["1-3", "1-1"], "crashes count as errors");
    }

    #[test]
    fn focused_entries_are_open_and_shown_whatever_the_filter() {
        let dir = tempfile::tempdir().unwrap();
        let settings = tri::log::LogSettings::default();
        for (id, level) in [("9-1", Level::Info), ("9-2", Level::Error)] {
            let mut e = entry(id, level, "m");
            e.detail = "full detail".into();
            tri::log::append(dir.path(), &settings, &e).unwrap();
        }
        let view = LogView::open(Some(dir.path().to_path_buf()), vec!["9-1".into()]);
        assert!(view.open.contains("9-1"));
        assert_eq!(view.shown, Shown::All, "an info entry in focus shows all");
        assert_eq!(view.visible().len(), 2);
    }
}
