//! Résumé du dossier ouvert, en bas du panneau des dossiers (issue n° 388, décidée le
//! 09/11/2026) : de quoi le dossier est fait (photos par type, paires RAW+JPEG,
//! développements par logiciel), où en est le tri, les conflits de notes et où vivent les
//! données de tri. Sur **tout** le dossier, pas la vue filtrée ([`tri::folder::Summary`]).
//!
//! Première étape (12/11/2026, demande de l'utilisateur) : les comptes seulement. Plus
//! tard, chaque ligne cliquable comme filtre (un clic : ce filtre seul).

use super::*;

/// Clé de la section dépliée dans l'état d'affichage (`panel_folded` garde les sections
/// repliées ; celle-ci est repliée par défaut, d'où une clé « ouverte »).
pub(super) const OPEN: &str = "folders-summary-open";

impl TriApp {
    /// Section « Ce dossier » : titre repliable, puis les comptes.
    pub(super) fn folder_summary(&mut self, ui: &mut Ui, f: &crate::settings::FolderSettings) {
        let open = self.panel_folded.contains(OPEN);
        if section_title(ui, &t!("summary-title"), open, f) {
            if open {
                self.panel_folded.remove(OPEN);
            } else {
                self.panel_folded.insert(OPEN.to_string());
            }
        }
        if !open {
            return;
        }
        // Peu coûteux (quelques milliers de recherches en mémoire) : refait à chaque image
        // affichée, toujours à jour des notes et des développements.
        let summary = self.folder.summary();
        let engines = self.developers.engines();
        let weak = ui.visuals().weak_text_color();
        egui::Grid::new("folder_summary")
            .num_columns(2)
            .spacing(vec2(12.0, 2.0))
            .show(ui, |ui| {
                // Ligne de détail (une extension, un logiciel) : en retrait.
                let indented = |ui: &mut Ui, text: &str, color: Option<Color32>| {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        match color {
                            Some(c) => ui.colored_label(c, text),
                            None => ui.label(text),
                        }
                    })
                    .inner
                };
                let row = |ui: &mut Ui, label: String, n: usize, tip: Option<String>| {
                    let l = ui.label(label);
                    let c = ui.label(n.to_string());
                    if let Some(tip) = tip {
                        l.on_hover_text(&tip);
                        c.on_hover_text(&tip);
                    }
                    ui.end_row();
                };
                row(ui, t!("summary-photos"), summary.photos, None);
                for t in &summary.types {
                    indented(ui, &t.extension, None);
                    ui.label(t.count.to_string());
                    ui.end_row();
                }
                if summary.pairs > 0 {
                    row(
                        ui,
                        t!("summary-pairs"),
                        summary.pairs,
                        Some(t!("summary-pairs-tip")),
                    );
                }
                if !summary.developments.is_empty() {
                    let total = summary.developments.iter().map(|(_, n)| n).sum();
                    row(ui, t!("summary-developments"), total, None);
                    for &(engine, n) in &summary.developments {
                        // Logiciel absent ou désactivé : ses développements ne sont pas dans
                        // la grille (#386), grisés ici avec la raison.
                        if engines.contains(&engine) {
                            indented(ui, engine.name(), None);
                            ui.label(n.to_string());
                        } else {
                            let tip = t!("summary-engine-off", engine = engine.name());
                            indented(ui, engine.name(), Some(weak)).on_hover_text(&tip);
                            ui.colored_label(weak, n.to_string()).on_hover_text(&tip);
                        }
                        ui.end_row();
                    }
                }
                row(ui, t!("summary-untouched"), summary.untouched, None);
                row(ui, t!("summary-starred"), summary.starred, None);
                row(ui, t!("summary-accepted"), summary.accepted, None);
                row(ui, t!("summary-rejected"), summary.rejected, None);
                // Conflits : seulement s'il y en a, en couleur d'avertissement.
                if summary.conflicts > 0 {
                    let warn = ui.visuals().warn_fg_color;
                    let tip = t!("summary-conflicts-tip");
                    ui.colored_label(warn, t!("summary-conflicts"))
                        .on_hover_text(&tip);
                    ui.colored_label(warn, summary.conflicts.to_string())
                        .on_hover_text(&tip);
                    ui.end_row();
                }
            });
        // Où vivent les données de tri (cache, filtres, curseurs…) de ce dossier (#378).
        let dir = &self.folder.dir;
        let place = match tri::store::holder(dir) {
            Some(root) if root == *dir => t!("summary-data-folder"),
            Some(root) => t!("summary-data-disk", path = root.display().to_string()),
            None => t!("summary-data-machine"),
        };
        ui.weak(place).on_hover_text(t!("folder-menu-cache-tip"));
    }
}
