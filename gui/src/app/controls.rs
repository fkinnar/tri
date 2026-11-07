//! Contrôles de l'interface en blocs réutilisables (proposition du 07/11/2026, idée de
//! l'utilisateur) : chaque case, liste ou curseur du panneau de droite est dessiné par
//! une seule fonction, [`TriApp::control`], qui l'affiche *et* applique ce qu'on y
//! change. Le panneau l'appelle à sa place ; la palette de commandes l'appelle sous la
//! ligne de l'action correspondante ([`Control::of_action`]) : on y voit l'état (case
//! cochée, choix de la liste, curseur) et on le change sans quitter la palette. Changer
//! un contrôle ici le change partout.
//!
//! Le pointage au clavier du panneau (`side_begin` / `side_end`) ne vaut que dans le
//! panneau : dans la palette, le contrôle est hors de sa liste ([`Host::Palette`]).

use super::*;

/// Un contrôle de l'interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Control {
    /// Fichiers montrés : cases RAW et JPEG (une des deux reste cochée).
    FileKinds,
    /// Un filtre sur le contenu, sa case et, allumé, ses réglages (curseur de sévérité
    /// du flou, curseurs de l'exposition, logiciels des développées) :
    /// `FilterBlurry`, `FilterWellExposed` ou `FilterDeveloped`.
    Filter(Action),
    /// Ordre : la clé (liste) et le sens (flèche).
    Sort,
    /// Rendu affiché (liste).
    Render,
    /// Empiler automatiquement (case).
    Stacks,
    /// Passer en revue les développements (case, grisée en JPEG boîtier).
    ReviewDevelopments,
    /// Autoriser les actions sur les fichiers (case, écrite dans la configuration).
    FileActions,
    /// Taille des vignettes (curseur de la barre du bas ; `+` / `-` en grille).
    GridSize,
}

/// Où le contrôle est dessiné.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Host {
    /// Le panneau de droite : pointable au clavier.
    Panel,
    /// Une ligne de la palette de commandes : à la souris.
    Palette,
}

impl Control {
    /// Le contrôle qui montre l'état de `action` (palette) ; `None` : une action sans
    /// contrôle (navigation, notes…).
    pub(super) fn of_action(action: Action) -> Option<Control> {
        Some(match action {
            Action::FilterKind(_) | Action::FilterCycle => Control::FileKinds,
            Action::FilterBlurry | Action::FilterWellExposed | Action::FilterDeveloped => {
                Control::Filter(action)
            }
            Action::SortNext | Action::SortReverse | Action::SortBy(_) => Control::Sort,
            Action::ToggleSource | Action::GridRenderNext | Action::Render(_) => Control::Render,
            Action::StackMode => Control::Stacks,
            Action::ReviewDevelopments => Control::ReviewDevelopments,
            Action::FileActionsAllow => Control::FileActions,
            Action::ZoomIn | Action::ZoomOut => Control::GridSize,
            _ => return None,
        })
    }
}

impl TriApp {
    /// Dessine `control` dans `ui` et applique aussitôt ce qui y change (comme le clavier :
    /// les mêmes actions). Dans la palette, hors du pointage au clavier du panneau.
    pub(super) fn control(&mut self, ui: &mut Ui, control: Control, host: Host) {
        let detached = std::mem::replace(&mut self.side_detached, host == Host::Palette);
        match control {
            Control::FileKinds => self.file_kinds_control(ui),
            Control::Filter(action) => self.filter_control(ui, action),
            Control::Sort => self.sort_control(ui),
            Control::Render => self.render_control(ui),
            Control::Stacks => {
                let mut stacking = self.stacking;
                if self
                    .side_checkbox(ui, &mut stacking, t!("panel-grid-stacks-on"))
                    .on_hover_text(format!(
                        "{}\n{}",
                        t!("panel-grid-stacks-tip"),
                        self.action_tip(Action::StackMode)
                    ))
                    .changed()
                {
                    let ctx = ui.ctx().clone();
                    self.perform(&ctx, Action::StackMode);
                }
            }
            Control::ReviewDevelopments => {
                // Source « JPEG boîtier » : pas de développement à voir, grisée (retour du
                // 05/11/2026 : le JPEG à un seul endroit, la source).
                let jpeg = self.jpeg_source();
                ui.add_enabled_ui(!jpeg, |ui| {
                    let mut review = self.review_developments;
                    if self
                        .side_checkbox(ui, &mut review, t!("panel-view-review"))
                        .on_hover_text(self.review_tip())
                        .changed()
                    {
                        self.review_developments = review;
                    }
                });
            }
            Control::GridSize => {
                let (item, key) = self.side_begin(false);
                if let Some(SideKey::Left | SideKey::Right) = key {
                    let g = self.gui.grid;
                    self.grid_cell = grid::step_cell(
                        self.grid_cell,
                        key == Some(SideKey::Right),
                        g.cell_min,
                        g.cell_max,
                    );
                    self.reveal = Some(Reveal::Minimal);
                }
                let width = panel_slider_width(ui);
                let slider = self.grid_size_slider(ui, width);
                self.side_end(ui, item, slider.rect, slider.drag_started());
            }
            Control::FileActions => {
                let mut allowed = self.settings.files.allow_actions;
                if self
                    .side_checkbox(ui, &mut allowed, t!("panel-grid-files-allow"))
                    .on_hover_text(t!("panel-grid-files-tip"))
                    .changed()
                {
                    self.set_file_actions(allowed);
                }
            }
        }
        self.side_detached = detached;
    }

    /// Les contrôles à montrer pour `action` dans la palette : le sien, s'il est utile
    /// ici (le filtre « développées » n'existe qu'avec un logiciel de développement).
    pub(super) fn palette_control(&self, action: Action) -> Option<Control> {
        let control = Control::of_action(action)?;
        match control {
            Control::Filter(a) => self
                .content_toggles()
                .iter()
                .any(|(t, _, _)| *t == a)
                .then_some(control),
            Control::ReviewDevelopments if self.developers.is_empty() => None,
            // En plein écran, + / - sont le zoom (son réglage est dans la barre du bas).
            Control::GridSize if self.mode != Mode::Grid => None,
            _ => Some(control),
        }
    }

    /// Cases RAW et JPEG (retour du 28/10/2026 : en tête de la section des filtres, sans
    /// titre). Le type tourne en boucle, comme au clavier : autant de crans que
    /// nécessaire.
    fn file_kinds_control(&mut self, ui: &mut Ui) {
        let (raw, jpeg) = match self.filter {
            KindFilter::All => (true, true),
            KindFilter::RawOnly => (true, false),
            KindFilter::FlatOnly => (false, true),
        };
        let mut cycles = 0;
        ui.horizontal(|ui| {
            for (is_raw, on, name, tip) in [
                (true, raw, "RAW", t!("filter-bar-raw-tip")),
                (false, jpeg, "JPEG", t!("filter-bar-flat-tip")),
            ] {
                let tip = format!("{tip}\n{}", self.action_tip(Action::FilterCycle));
                let mut checked = on;
                let alone = on && raw != jpeg;
                let (item, key) = self.side_begin(false);
                let mut response = ui
                    .add_enabled(!alone, egui::Checkbox::new(&mut checked, name))
                    .on_hover_text(&tip)
                    .on_disabled_hover_text(&tip);
                if key == Some(SideKey::Activate) && !alone {
                    checked = !checked;
                    response.mark_changed();
                }
                self.side_end(ui, item, response.rect, response.changed());
                if response.changed() {
                    let (raw, jpeg) = if is_raw {
                        (checked, jpeg)
                    } else {
                        (raw, checked)
                    };
                    let kind = match (raw, jpeg) {
                        (true, false) => KindFilter::RawOnly,
                        (false, true) => KindFilter::FlatOnly,
                        _ => KindFilter::All,
                    };
                    let order = [KindFilter::All, KindFilter::RawOnly, KindFilter::FlatOnly];
                    let from = order.iter().position(|&k| k == self.filter).unwrap_or(0);
                    let to = order.iter().position(|&k| k == kind).unwrap_or(0);
                    cycles = (to + 3 - from) % 3;
                }
            }
        });
        let ctx = ui.ctx().clone();
        for _ in 0..cycles {
            self.perform(&ctx, Action::FilterCycle);
        }
    }

    /// Un filtre sur le contenu : sa case, son icône (celle de la barre du haut) et son
    /// nom ; allumé, ses réglages dessous (retour du 27/10/2026 : cachés quand ils ne
    /// servent pas).
    fn filter_control(&mut self, ui: &mut Ui, action: Action) {
        let Some((_, on, enabled)) = self
            .content_toggles()
            .into_iter()
            .find(|(a, _, _)| *a == action)
        else {
            return;
        };
        let name = match action {
            Action::FilterBlurry => t!("panel-filter-sharp"),
            Action::FilterWellExposed => t!("panel-filter-exposed"),
            _ => t!("panel-filter-developed"),
        };
        let size = self.gui.grid.marks + 4.0;
        let tip = self.action_tip(action);
        let (item, key) = self.side_begin(false);
        let row = ui.horizontal(|ui| {
            if !enabled {
                ui.disable();
            }
            let mut checked = on;
            let check = ui.checkbox(&mut checked, "").on_hover_text(&tip);
            let (rect, icon) = marks::clickable(ui, size, &tip);
            let color = if on {
                marks::accent_mark(ui.ctx())
            } else {
                ui.visuals().weak_text_color()
            };
            filter_icon(ui.painter(), rect, action, color);
            let label = ui
                .add(egui::Label::new(name).sense(Sense::click()))
                .on_hover_text(&tip);
            check.changed() || icon.clicked() || label.clicked()
        });
        self.side_end(ui, item, row.response.rect, row.inner);
        if on {
            ui.indent(action.id(), |ui| {
                let width = panel_slider_width(ui);
                match action {
                    Action::FilterBlurry => {
                        let (item, key) = self.side_begin(false);
                        let slider = self.blur_slider(ui, width, key);
                        self.side_end(ui, item, slider.rect, slider.drag_started());
                        slider_ends(ui, width);
                    }
                    Action::FilterWellExposed => {
                        for side in [tri::exposure::Clip::Over, tri::exposure::Clip::Under] {
                            let (item, key) = self.side_begin(false);
                            let slider = self.clip_slider(ui, side, width, key);
                            self.side_end(ui, item, slider.rect, slider.drag_started());
                        }
                        slider_ends(ui, width);
                    }
                    _ => self.developed_checkboxes(ui),
                }
            });
        }
        if enabled && (row.inner || key == Some(SideKey::Activate)) {
            let ctx = ui.ctx().clone();
            self.perform(&ctx, action);
        }
    }

    /// Ordre : la clé (← / → la précédente / suivante, Espace la suivante, au clavier du
    /// panneau), puis le sens.
    fn sort_control(&mut self, ui: &mut Ui) {
        let size = ui.text_style_height(&egui::TextStyle::Body) + 4.0;
        let mut key = self.order.key;
        let mut reverse = false;
        ui.horizontal(|ui| {
            let (item, pressed) = self.side_begin(false);
            let keys = tri::sort::SortKey::ALL;
            let at = keys.iter().position(|&k| k == key).unwrap_or(0);
            match pressed {
                Some(SideKey::Left) => key = keys[at.saturating_sub(1)],
                Some(SideKey::Right) => key = keys[(at + 1).min(keys.len() - 1)],
                Some(SideKey::Activate) => key = keys[(at + 1) % keys.len()],
                None => {}
            }
            let combo = egui::ComboBox::from_id_salt("sort")
                .selected_text(sort_name(key))
                .show_ui(ui, |ui| {
                    for k in tri::sort::SortKey::ALL {
                        ui.selectable_value(&mut key, k, sort_name(k));
                    }
                })
                .response
                .on_hover_text(self.action_tip(Action::SortNext));
            self.side_end(ui, item, combo.rect, combo.clicked());
            let (item, pressed) = self.side_begin(false);
            let (rect, response) =
                marks::clickable(ui, size, &self.action_tip(Action::SortReverse));
            self.side_end(ui, item, rect, response.clicked());
            let color = ui.style().interact(&response).fg_stroke.color;
            marks::sort_arrow(
                ui.painter(),
                rect.shrink(size * 0.1),
                self.order.descending,
                color,
            );
            reverse = response.clicked() || pressed == Some(SideKey::Activate);
        });
        if key != self.order.key {
            self.set_order(tri::sort::Order { key, ..self.order });
        }
        if reverse {
            let ctx = ui.ctx().clone();
            self.perform(&ctx, Action::SortReverse);
        }
    }

    /// Rendu affiché (#114, un seul pour la grille et le plein écran depuis le
    /// 06/11/2026) : JPEG boîtier, la couverture, chaque logiciel installé, le rendu RAW.
    fn render_control(&mut self, ui: &mut Ui) {
        let engines = self.developers.engines();
        let mut render = self.grid_render;
        let choices = crate::grid_render::GridRender::choices(&engines);
        let weak = ui.visuals().weak_text_color();
        let tip = self.render_tip();
        self.side_combo(
            ui,
            "grid_render",
            &mut render,
            &choices,
            |c| c.label(weak),
            Some(tip),
        );
        if self.shown_render() != self.grid_render {
            ui.weak(t!("panel-grid-render-filtered"));
        }
        if render != self.grid_render {
            self.set_grid_render(render);
            // Plein écran : l'image dans le nouveau rendu.
            if self.mode != Mode::Grid {
                self.request_zoom();
            }
        }
    }

    /// Bulle du rendu affiché (son sous-titre et sa liste).
    pub(super) fn render_tip(&self) -> String {
        format!(
            "{}\n{}\n{}",
            t!("panel-render-tip"),
            self.action_tip(Action::ToggleSource),
            self.action_tip(Action::GridRenderNext)
        )
    }
}
