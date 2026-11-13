//! Recadrage en plein écran (19/10/2026) : la photo en grand, le cadre réglable (libre ou
//! à rapport fixe) et un voile gris translucide autour, comme l'a demandé l'utilisateur.
//! Tout se fait à la souris (poignées, barre des rapports, orientation) et au clavier
//! (actions du contexte « recadrage »). La géométrie est dans le moteur
//! ([`tri::versions`]) ; ici, seulement le dessin et les gestes.

use std::path::PathBuf;

use eframe::egui::{self, Color32, CursorIcon, Pos2, Rect as ScreenRect, Response, Stroke, Ui};
use tri::versions::{Frame, Handle, Ratio, Rect, Source, Tiles, Version, Versions};

use crate::settings::CropSettings;

/// Recadrage en cours.
pub struct Crop {
    /// Image recadrée.
    pub image: PathBuf,
    /// Cadre (proportions de l'image vue).
    pub rect: Rect,
    /// Rapport (d'un morceau).
    pub ratio: Ratio,
    /// Morceaux (bande horizontale pour un carrousel, 13/11/2026).
    pub tiles: Tiles,
    /// Rapport largeur / hauteur de l'image vue (pixels), connu au premier dessin.
    aspect: Option<f64>,
    /// Version modifiée (sa place dans la liste), sinon nouvelle.
    pub editing: Option<usize>,
    /// Nom ; il suit le rapport tant qu'on ne l'a pas tapé.
    pub name: String,
    name_edited: bool,
    /// Profil d'export associé (facultatif).
    pub profile: Option<String>,
    /// Image vue (développé, rendu RAW, JPEG boîtier).
    pub source: Source,
    /// Geste en cours : poignée, cadre au départ, point de départ.
    drag: Option<(Handle, Rect, Pos2)>,
    /// Premier dessin : le champ du nom ne prend pas le focus (le clavier reste au cadre).
    pub fresh: bool,
    /// Préréglage appliqué (identifiant).
    pub preset: Option<String>,
    /// Rapport personnalisé en cours de saisie dans la liste des rapports (« 17:23 »).
    custom: String,
    /// Champ qui prendra le clavier au prochain dessin.
    focus: Option<Focus>,
    /// Identifiant de la liste des rapports (image précédente) : `:` l'ouvre.
    ratio_list_id: Option<egui::Id>,
    /// Les autres versions de l'image : un nom déjà pris est refusé (issue n° 14).
    others: Versions,
    /// Bulle de la liste des préréglages, avec les raccourcis en vigueur (posée par
    /// l'appelant avant [`Crop::bar`], 08/11/2026 : « p » écrit en dur, périmé).
    pub preset_tip: String,
    /// Bulles des listes des colonnes et des lignes de morceaux, avec leurs raccourcis
    /// (posées par l'appelant).
    pub tiles_tip: String,
    /// Voir `tiles_tip`.
    pub rows_tip: String,
}

/// Ce que la barre du recadrage demande.
pub enum BarAction {
    /// Enregistrer la version.
    Save,
    /// Abandonner.
    Cancel,
    /// Appliquer le préréglage n°.
    Preset(usize),
    /// Enregistrer le cadre en cours comme préréglage.
    SavePreset,
    /// Ajouter ce rapport à la liste, dans la configuration (`[versions] ratios`).
    AddRatio(Ratio),
}

/// Champ de la barre qui prend le clavier à la demande (raccourcis, 07/11/2026).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// Le rapport personnalisé (la liste des rapports s'ouvre dessus).
    CustomRatio,
    /// Le nom.
    Name,
}

/// Les rapports de la liste : ceux fournis, puis ceux ajoutés (`[versions] ratios`), sans
/// doublon.
pub fn ratio_list(added: &[Ratio]) -> Vec<Ratio> {
    let mut list: Vec<Ratio> = Ratio::PRESETS.to_vec();
    for &r in added {
        if !list.iter().any(|l| l.same_shape(r)) {
            list.push(r);
        }
    }
    list
}

/// Nom affiché d'un rapport (« Libre », « Original », « 4:5 »…).
pub fn ratio_name(ratio: Ratio) -> String {
    match ratio {
        Ratio::Free => t!("crop-ratio-free"),
        Ratio::Original { .. } => t!("crop-ratio-original"),
        Ratio::Fixed(w, h) => format!("{w}:{h}"),
    }
}

/// Nom proposé d'une version de forme `frame` : celui du rapport (suivi du nombre de
/// morceaux : « 4:5 × 2 »), sauf « Original »,
/// qui désigne l'image non recadrée dans la liste des versions (retour du 23/10/2026 : deux
/// « Original » indiscernables) ; « Recadrage » à la place.
fn default_name(frame: Frame) -> String {
    let name = match frame.ratio {
        Ratio::Original { .. } => t!("crop-name-original-ratio"),
        ratio => ratio_name(ratio),
    };
    let t = frame.tiles;
    if t.is_one() {
        name
    } else if t.rows == 1 {
        format!("{name} × {}", t.columns)
    } else {
        format!("{name} {}×{}", t.columns, t.rows)
    }
}

impl Crop {
    /// Nouvelle version de `image` vue en `source` : cadre « original » sur toute l'image.
    /// `aspect` : rapport largeur / hauteur de l'image, s'il est déjà connu (sinon au
    /// premier dessin) ; connu, les touches tapées avant le premier dessin tombent juste.
    /// `others` : les versions qu'elle a déjà.
    pub fn new(image: PathBuf, source: Source, aspect: Option<f64>, others: Versions) -> Crop {
        let ratio = Ratio::Original { flipped: false };
        let mut crop = Crop {
            image,
            rect: Rect::FULL,
            ratio,
            tiles: Tiles::ONE,
            aspect,
            editing: None,
            name: String::new(),
            name_edited: false,
            profile: None,
            source,
            drag: None,
            fresh: true,
            custom: String::new(),
            focus: None,
            ratio_list_id: None,
            preset: None,
            others,
            preset_tip: String::new(),
            tiles_tip: String::new(),
            rows_tip: String::new(),
        };
        crop.name = crop.free_name(&default_name(crop.frame()));
        crop
    }

    /// Forme du cadre : rapport d'un morceau et morceaux.
    pub fn frame(&self) -> Frame {
        Frame {
            ratio: self.ratio,
            tiles: self.tiles,
        }
    }

    /// Le nom proposé suit la forme, tant qu'on ne l'a pas tapé.
    fn rename(&mut self) {
        if !self.name_edited {
            self.name = self.free_name(&default_name(self.frame()));
        }
    }

    /// Reprendre la version `index` (`version`) ; `others` : les autres versions de l'image.
    pub fn edit(image: PathBuf, index: usize, version: &Version, others: Versions) -> Crop {
        Crop {
            image,
            rect: version.crop,
            ratio: version.ratio,
            tiles: version.tiles,
            aspect: None,
            editing: Some(index),
            name: version.name.clone(),
            name_edited: true,
            profile: version.profile.clone(),
            source: version.source,
            drag: None,
            fresh: true,
            custom: String::new(),
            focus: None,
            ratio_list_id: None,
            preset: None,
            others,
            preset_tip: String::new(),
            tiles_tip: String::new(),
            rows_tip: String::new(),
        }
    }

    /// Le nom tapé est déjà celui d'une autre version de l'image.
    pub fn name_taken(&self) -> bool {
        self.taken(&self.name)
    }

    /// `name` est pris : par une autre version, ou c'est celui de l'image non recadrée.
    fn taken(&self, name: &str) -> bool {
        self.others.name_taken(name) || tri::versions::same_name(name, &t!("versions-original"))
    }

    /// `base`, numéroté s'il est pris.
    fn free_name(&self, base: &str) -> String {
        tri::versions::free_name(base, |n| self.taken(n))
    }

    /// Nom enregistrable : non vide, pas déjà pris.
    pub fn name_ok(&self) -> bool {
        !self.name.trim().is_empty() && !self.name_taken()
    }

    /// Applique un préréglage : son rapport exact (orientation comprise), ses morceaux, son
    /// nom et son profil ; le cadre se recentre au plus grand, on l'ajuste ensuite.
    pub fn apply_preset(&mut self, id: &str, name: String, frame: Frame, profile: Option<String>) {
        self.ratio = frame.ratio;
        self.tiles = frame.tiles;
        if let Some(aspect) = self.aspect {
            self.rect = Rect::fit(frame, aspect);
        }
        self.name = self.free_name(&name);
        self.name_edited = true;
        self.profile = profile;
        self.preset = Some(id.to_string());
    }

    /// Proportions de l'image connues (cadre dessiné au moins une fois, ou image déjà
    /// chargée) : on peut enregistrer sans risque de cadre faux.
    pub fn ready(&self) -> bool {
        self.aspect.is_some()
    }

    /// La version telle qu'elle sera enregistrée.
    pub fn version(&self) -> Version {
        Version {
            name: self.name.trim().to_string(),
            crop: self.rect,
            ratio: self.ratio,
            source: self.source,
            profile: self.profile.clone(),
            tiles: self.tiles,
        }
    }

    /// Change de rapport : même centre, au plus grand qui tienne (ou, pour un rapport
    /// choisi alors que le cadre couvre toute l'image, le plus grand de ce rapport).
    pub fn set_ratio(&mut self, ratio: Ratio) {
        // Même forme que l'actuel : garder l'orientation choisie.
        let ratio = if ratio.same_shape(self.ratio) {
            self.ratio
        } else {
            ratio
        };
        self.ratio = ratio;
        self.refit();
        self.rename();
    }

    /// Le cadre à la forme en cours : même centre, au plus grand qui tienne (ou le plus
    /// grand de cette forme si le cadre couvre toute l'image). Proportions encore
    /// inconnues : le cadre sera calculé au premier dessin.
    fn refit(&mut self) {
        if let Some(aspect) = self.aspect {
            self.rect = if self.rect == Rect::FULL {
                Rect::fit(self.frame(), aspect)
            } else {
                self.rect.with_ratio(self.frame(), aspect)
            };
        }
    }

    /// Les morceaux : `tiles` (colonnes × lignes ; 1 × 1 : le cadre entier). Chacun garde
    /// le rapport choisi, le cadre s'élargit ou se resserre autour de son centre.
    pub fn set_tiles(&mut self, tiles: Tiles) {
        let tiles = Tiles::from([tiles.columns, tiles.rows]);
        if tiles == self.tiles {
            return;
        }
        self.tiles = tiles;
        self.refit();
        self.rename();
    }

    /// Une colonne (`rows` : une ligne) de plus (`forward`) ou de moins, en boucle de 1 à
    /// [`Tiles::MAX`].
    pub fn step_tiles(&mut self, rows: bool, forward: bool) {
        let step = |n: u32| {
            if forward {
                n % Tiles::MAX + 1
            } else if n <= 1 {
                Tiles::MAX
            } else {
                n - 1
            }
        };
        let mut tiles = self.tiles;
        if rows {
            tiles.rows = step(tiles.rows);
        } else {
            tiles.columns = step(tiles.columns);
        }
        self.set_tiles(tiles);
    }

    /// Rapport suivant (`forward`) ou précédent de la liste (avec `added`), en boucle ;
    /// l'orientation choisie est gardée.
    pub fn step_ratio(&mut self, forward: bool, added: &[Ratio]) {
        let list = ratio_list(added);
        let n = list.len();
        let next = match list.iter().position(|r| r.same_shape(self.ratio)) {
            Some(i) if forward => (i + 1) % n,
            Some(i) => (i + n - 1) % n,
            None if forward => 0,
            None => n - 1,
        };
        self.set_ratio(list[next]);
    }

    /// Préréglage d'export suivant (`forward`) ou précédent parmi `profiles`, « aucun »
    /// compris, en boucle.
    pub fn step_profile(&mut self, forward: bool, profiles: &[String]) {
        let mut all: Vec<Option<String>> = vec![None];
        all.extend(profiles.iter().cloned().map(Some));
        let n = all.len();
        let at = all.iter().position(|p| *p == self.profile).unwrap_or(0);
        let next = if forward {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        };
        self.profile = all[next].clone();
    }

    /// Donne le clavier à un champ de la barre au prochain dessin.
    pub fn focus(&mut self, field: Focus) {
        self.focus = Some(field);
    }

    /// Le cadre d'origine : l'image entière, à son rapport (idée de l'utilisateur,
    /// 07/11/2026 : `0`, ou Ctrl+clic sur « Original »).
    pub fn reset_frame(&mut self) {
        self.ratio = Ratio::Original { flipped: false };
        self.tiles = Tiles::ONE;
        self.rect = Rect::FULL;
        self.rename();
    }

    /// Portrait ↔ paysage.
    pub fn rotate(&mut self) {
        self.ratio = self.ratio.rotated();
        if let Some(aspect) = self.aspect {
            self.rect = self.rect.with_ratio(self.frame(), aspect);
        }
        self.rename();
    }

    /// Déplace le cadre de `(dx, dy)` (proportions de l'image).
    pub fn nudge(&mut self, dx: f64, dy: f64) {
        if let Some(aspect) = self.aspect {
            self.rect = self.rect.drag(Handle::Move, dx, dy, self.frame(), aspect);
        }
    }

    /// Tire le côté droit (`dx`) ou le bas (`dy`) du cadre.
    pub fn grow(&mut self, dx: f64, dy: f64) {
        let handle = if dx != 0.0 {
            Handle::Right
        } else {
            Handle::Bottom
        };
        if let Some(aspect) = self.aspect {
            self.rect = self.rect.drag(handle, dx, dy, self.frame(), aspect);
        }
    }

    /// Cadre à l'écran, pour l'image dessinée en `image`.
    pub fn screen_rect(&self, image: ScreenRect) -> ScreenRect {
        let r = self.rect;
        ScreenRect::from_min_size(
            image.min + egui::vec2(r.x as f32 * image.width(), r.y as f32 * image.height()),
            egui::vec2(r.w as f32 * image.width(), r.h as f32 * image.height()),
        )
    }

    /// Gestes de la souris (`response` : celle de la vue) et dessin : voile, cadre et
    /// poignées, sur l'image dessinée en `image` (vue ajustée). Renvoie le cadre à
    /// l'écran (pour y dessiner la superposition choisie).
    pub fn show(
        &mut self,
        painter: &egui::Painter,
        response: &Response,
        image: ScreenRect,
        settings: &CropSettings,
    ) -> ScreenRect {
        if image.width() <= 0.0 || image.height() <= 0.0 {
            return image;
        }
        let aspect = f64::from(image.width() / image.height());
        // Proportions réelles de l'image affichée : connues maintenant, ou différentes de
        // celles supposées (image chargée entre-temps) ; le cadre d'une nouvelle version
        // suit (celui d'une version reprise est gardé tel quel).
        let known = self.aspect;
        if known.is_none_or(|a| (a - aspect).abs() > a * 0.005) {
            self.aspect = Some(aspect);
            if self.editing.is_none() {
                self.rect = match known {
                    None => Rect::fit(self.frame(), aspect),
                    Some(_) => self.rect.with_ratio(self.frame(), aspect),
                };
            }
        }
        let frame = self.screen_rect(image);
        let grab = settings.handle_grab;
        let hit = |pos: Pos2| hit(frame, grab, pos);
        // Jugé au point d'appui : egui ne déclare le glisser qu'après quelques pixels,
        // assez pour sortir de la bande d'une poignée prise par l'intérieur (issue n° 1).
        if response.drag_started()
            && let Some(pos) = response
                .ctx
                .input(|i| i.pointer.press_origin())
                .or_else(|| response.interact_pointer_pos())
            && let Some(handle) = hit(pos)
        {
            self.drag = Some((handle, self.rect, pos));
        }
        if let Some((handle, start, from)) = self.drag {
            if let Some(pos) = response.interact_pointer_pos() {
                let d = pos - from;
                self.rect = start.drag(
                    handle,
                    f64::from(d.x / image.width()),
                    f64::from(d.y / image.height()),
                    self.frame(),
                    aspect,
                );
            }
            if !response.dragged() {
                self.drag = None;
            }
        }
        let shown = self
            .drag
            .map(|(h, _, _)| h)
            .or_else(|| response.hover_pos().and_then(hit));
        if let Some(handle) = shown {
            response.ctx.set_cursor_icon(cursor(handle));
        }
        // Voile autour du cadre.
        let frame = self.screen_rect(image);
        let shade = Color32::from_rgba_unmultiplied(
            settings.shade.0[0],
            settings.shade.0[1],
            settings.shade.0[2],
            (settings.shade_opacity.clamp(0.0, 1.0) * 255.0) as u8,
        );
        for r in [
            ScreenRect::from_min_max(image.min, egui::pos2(image.max.x, frame.min.y)),
            ScreenRect::from_min_max(egui::pos2(image.min.x, frame.max.y), image.max),
            ScreenRect::from_min_max(
                egui::pos2(image.min.x, frame.min.y),
                egui::pos2(frame.min.x, frame.max.y),
            ),
            ScreenRect::from_min_max(
                egui::pos2(frame.max.x, frame.min.y),
                egui::pos2(image.max.x, frame.max.y),
            ),
        ] {
            if r.width() > 0.0 && r.height() > 0.0 {
                painter.rect_filled(r, 0.0, shade);
            }
        }
        // Cadre et poignées (liseré sombre dessous : lisible sur tout fond).
        let line = settings.line_width;
        painter.rect_stroke(
            frame,
            0.0,
            Stroke::new(line + 2.0, Color32::from_black_alpha(120)),
            egui::StrokeKind::Middle,
        );
        painter.rect_stroke(
            frame,
            0.0,
            Stroke::new(line, Color32::WHITE),
            egui::StrokeKind::Middle,
        );
        // Les coupes entre les morceaux : même trait, plus fin.
        let cut = (line * 0.5).max(1.0);
        for r in self.tiles.cells(Rect::FULL).iter().skip(1) {
            let at = |v: f64, a: f32, b: f32| a + v as f32 * (b - a);
            let mut lines = Vec::new();
            if r.x > 0.0 && r.y == 0.0 {
                let x = at(r.x, frame.left(), frame.right());
                lines.push([egui::pos2(x, frame.top()), egui::pos2(x, frame.bottom())]);
            }
            if r.y > 0.0 && r.x == 0.0 {
                let y = at(r.y, frame.top(), frame.bottom());
                lines.push([egui::pos2(frame.left(), y), egui::pos2(frame.right(), y)]);
            }
            for l in lines {
                painter.line_segment(l, Stroke::new(cut + 2.0, Color32::from_black_alpha(120)));
                painter.line_segment(l, Stroke::new(cut, Color32::WHITE));
            }
        }
        let size = settings.handle_size;
        for p in [
            frame.left_top(),
            frame.right_top(),
            frame.left_bottom(),
            frame.right_bottom(),
            frame.center_top(),
            frame.center_bottom(),
            frame.left_center(),
            frame.right_center(),
        ] {
            let r = ScreenRect::from_center_size(p, egui::vec2(size, size));
            painter.rect_filled(r, 1.0, Color32::WHITE);
            painter.rect_stroke(
                r,
                1.0,
                Stroke::new(1.0, Color32::from_black_alpha(160)),
                egui::StrokeKind::Outside,
            );
        }
        frame
    }

    /// Barre du recadrage (sous l'image) : préréglage, rapports, orientation, nom, profil
    /// d'export, enregistrer / annuler. `profiles` et `presets` : (identifiant, nom affiché).
    pub fn bar(
        &mut self,
        ui: &mut Ui,
        profiles: &[(String, String)],
        presets: &[(String, String)],
        ratios: &[Ratio],
        label_scale: f32,
    ) -> Option<BarAction> {
        let mut action = None;
        // Enregistrer et Annuler à droite, à part (retour du 20/10/2026) ; le reste à gauche,
        // dans la place qui reste.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(t!("cancel")).clicked() {
                action = Some(BarAction::Cancel);
            }
            let ok = self.name_ok();
            if ui
                .add_enabled(ok, egui::Button::new(t!("crop-save")))
                .clicked()
            {
                action = Some(BarAction::Save);
            }
            ui.add_space(ui.spacing().item_spacing.x * 4.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.horizontal_wrapped(|ui| {
                    self.bar_left(ui, profiles, presets, ratios, label_scale, &mut action)
                });
            });
        });
        self.fresh = false;
        action
    }

    /// Partie gauche de la barre, en parties titrées d'un libellé à l'accent (retour du
    /// 07/11/2026 : « un peu le souk ») : Version (préréglage), Recadrage (rapports,
    /// orientation), Export, Nom, puis la disquette (enregistrer comme préréglage).
    fn bar_left(
        &mut self,
        ui: &mut Ui,
        profiles: &[(String, String)],
        presets: &[(String, String)],
        ratios: &[Ratio],
        label_scale: f32,
        action: &mut Option<BarAction>,
    ) {
        // De l'air entre deux parties, plus qu'entre deux boutons.
        let gap = ui.spacing().item_spacing.x * 4.0;
        // Le libellé d'une partie reste avec ce qui le suit (à la ligne ensemble).
        // À l'accent, en majuscules, comme les libellés des autres barres (retour du
        // 07/11/2026).
        let section = |ui: &mut Ui, label: &str, first: f32| {
            keep_together(ui, label, first);
            crate::app::bar_label(ui, label, label_scale);
        };
        // Version : le préréglage.
        if !presets.is_empty() {
            section(ui, &t!("crop-bar-title"), ui.spacing().combo_width);
            let none = t!("crop-preset-none");
            let current = self
                .preset
                .as_ref()
                .and_then(|id| presets.iter().find(|(p, _)| p == id))
                .map_or(none, |(_, name)| name.clone());
            egui::ComboBox::from_id_salt("crop_preset")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (i, (id, name)) in presets.iter().enumerate() {
                        let on = self.preset.as_deref() == Some(id.as_str());
                        if ui.selectable_label(on, name).clicked() {
                            *action = Some(BarAction::Preset(i));
                        }
                    }
                })
                .response
                .on_hover_text(&self.preset_tip);
            ui.add_space(gap);
        }
        // Recadrage : le rapport dans une liste (retour du 07/11/2026 : plus lisible que
        // sept boutons), puis l'orientation.
        section(ui, &t!("crop-section-frame"), ui.spacing().combo_width);
        let mut chosen = None;
        let mut reset = false;
        // `:` : la liste s'ouvre, son champ prend le clavier.
        let focus_custom = self.focus == Some(Focus::CustomRatio);
        if focus_custom && let Some(id) = self.ratio_list_id {
            egui::Popup::open_id(ui.ctx(), id.with("popup"));
        }
        let list = egui::ComboBox::from_id_salt("crop_ratio")
            .selected_text(ratio_name(self.ratio))
            // Ouverte tant qu'on tape un rapport à soi : fermée par un choix ou un clic
            // dehors (sinon, cliquer dans le champ la fermait).
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                // Ceux fournis, puis ceux ajoutés (`[versions] ratios`), sans doublon.
                for preset in ratio_list(ratios) {
                    let on = preset.same_shape(self.ratio);
                    // Dans l'orientation choisie (4:5 plutôt que 5:4) pour celui en cours.
                    let shown = if on { self.ratio } else { preset };
                    let mut item = ui.selectable_label(on, ratio_name(shown));
                    let original = matches!(preset, Ratio::Original { .. });
                    if original {
                        item = item.on_hover_text(t!("crop-ratio-original-tip"));
                    }
                    if item.clicked() {
                        // Ctrl / Cmd+clic sur « Original » : tout le cadre d'origine.
                        if original && ui.input(|i| i.modifiers.command) {
                            reset = true;
                        } else {
                            chosen = Some(preset);
                        }
                        ui.close();
                    }
                }
                // Un rapport à soi (idée de l'utilisateur, 07/11/2026) : « 17:23 », « 1.91:1 »,
                // Entrée l'applique ; « + » l'ajoute à la liste, gardé dans la configuration.
                // Le mot « Personnalisé » en gris dans le champ (un titre à côté était
                // illisible).
                ui.separator();
                ui.horizontal(|ui| {
                    let ok = Ratio::parse_custom(&self.custom);
                    // Largeur fixe (07/11/2026 : celle « disponible » grandissait avec la
                    // liste, qui s'élargissait à chaque image).
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut self.custom)
                            .desired_width(ui.spacing().combo_width)
                            .hint_text(t!("crop-ratio-custom")),
                    );
                    if focus_custom {
                        field.request_focus();
                        self.focus = None;
                    }
                    let field = field.on_hover_text(t!("crop-ratio-custom-bad"));
                    if ok.is_some()
                        && field.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        // Appliqué : la liste le montre, le champ se vide.
                        chosen = ok;
                        self.custom.clear();
                        ui.close();
                    }
                    let add = ui
                        .add_enabled(ok.is_some(), egui::Button::new("+"))
                        .on_hover_text(t!("crop-ratio-add-tip"));
                    if add.clicked()
                        && let Some(r) = ok
                    {
                        chosen = Some(r);
                        *action = Some(BarAction::AddRatio(r));
                        self.custom.clear();
                        ui.close();
                    }
                });
            });
        self.ratio_list_id = Some(list.response.id);
        if let Some(preset) = chosen {
            self.set_ratio(preset);
        }
        if reset {
            self.reset_frame();
        }
        // Orientation : une icône, le rectangle du cadre actuel (retour du 07/11/2026 :
        // « Portrait » sur un cadre paysage, pour dire celle d'arrivée, déroutait) ; la
        // bulle dit celle d'arrivée.
        let value = self.ratio.value(self.aspect.unwrap_or(1.0));
        let portrait = value.is_some_and(|r| r < 1.0);
        let square = matches!(self.ratio, Ratio::Fixed(1, 1));
        let can_turn = !matches!(self.ratio, Ratio::Free) && !square;
        let size = ui.spacing().interact_size.y;
        let turn = ui
            .add_enabled(
                can_turn,
                egui::Button::new("").min_size(egui::vec2(size * 1.4, size)),
            )
            .on_hover_text(if portrait {
                t!("crop-turn-landscape")
            } else {
                t!("crop-turn-portrait")
            })
            .on_disabled_hover_text(t!("crop-turn-none"));
        orientation_icon(ui, &turn, portrait, square);
        if turn.clicked() {
            self.rotate();
        }
        ui.add_space(gap);
        // Morceaux : colonnes × lignes (13/11/2026) ; 3 × 1 une bande horizontale (carrousel
        // Instagram), 1 × 3 une bande verticale, 3 × 3 une grille.
        let width = ui.spacing().interact_size.y * 2.2;
        section(ui, &t!("crop-tiles-title"), width * 2.0);
        let mut tiles = self.tiles;
        for (id, n, tip) in [
            ("crop_tiles", &mut tiles.columns, &self.tiles_tip),
            ("crop_tile_rows", &mut tiles.rows, &self.rows_tip),
        ] {
            if id == "crop_tile_rows" {
                ui.label("×");
            }
            egui::ComboBox::from_id_salt(id)
                .width(width)
                .selected_text(n.to_string())
                .show_ui(ui, |ui| {
                    for k in 1..=Tiles::MAX {
                        ui.selectable_value(n, k, k.to_string());
                    }
                })
                .response
                .on_hover_text(tip);
        }
        self.set_tiles(tiles);
        ui.add_space(gap);
        // Export : le préréglage d'export, facultatif.
        section(ui, &t!("crop-profile"), ui.spacing().combo_width);
        let none = t!("crop-profile-none");
        let current = self
            .profile
            .as_ref()
            .and_then(|id| profiles.iter().find(|(p, _)| p == id))
            .map_or(none.clone(), |(_, name)| name.clone());
        egui::ComboBox::from_id_salt("crop_profile")
            .selected_text(current)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.profile, None, none);
                for (id, name) in profiles {
                    ui.selectable_value(&mut self.profile, Some(id.clone()), name);
                }
            });
        ui.add_space(gap);
        // Nom.
        let name_width = 140.0;
        section(ui, &t!("crop-name"), name_width);
        let taken = self.name_taken();
        // Ctrl+Échap (ou Cmd+Échap) rend le clavier sans quitter la version, comme
        // sur les listes (retour du 07/11/2026 : on ne sortait pas du nom au clavier). Pris
        // avant le champ, qui ne le voit donc pas ; Échap seul quitte la version.
        let name_id = ui.make_persistent_id("crop_name");
        if ui.memory(|m| m.has_focus(name_id)) {
            let escape = ui.input_mut(|i| {
                let before = i.events.len();
                i.events.retain(|e| {
                    !matches!(
                        e,
                        egui::Event::Key {
                            key: egui::Key::Escape,
                            pressed: true,
                            modifiers,
                            ..
                        } if modifiers.ctrl || modifiers.command
                    )
                });
                i.events.len() != before
            });
            if escape {
                ui.memory_mut(|m| m.surrender_focus(name_id));
            }
        }
        let mut edit = egui::TextEdit::singleline(&mut self.name)
            .id(name_id)
            .desired_width(name_width);
        if taken {
            edit = edit.text_color(ui.visuals().error_fg_color);
        }
        let mut field = ui.add(edit);
        // F2 : le nom prend le clavier.
        if self.focus == Some(Focus::Name) {
            field.request_focus();
            self.focus = None;
        }
        if taken {
            field = field.on_hover_text(t!("crop-name-taken"));
        }
        if field.changed() {
            self.name_edited = true;
        }
        // Enregistrer comme préréglage (rapport, nom, export) : la disquette, collée au nom
        // (l'un ne va pas sans l'autre), grisée sans nom (retour du 07/11/2026).
        let named = !self.name.trim().is_empty();
        ui.add_enabled_ui(named, |ui| {
            let size = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
            let (rect, response) = crate::marks::icon_button(
                ui,
                size,
                &t!("crop-preset-save"),
                &t!("crop-preset-save-tip"),
            );
            let color = ui.style().interact(&response).fg_stroke.color;
            crate::marks::floppy(ui.painter(), rect.shrink(size * 0.22), color);
            if response.clicked() {
                *action = Some(BarAction::SavePreset);
            }
        });
    }
}

/// Dans une ligne qui passe à la ligne : passe à la suivante si le libellé `label` et un
/// champ de largeur `width` qui le suit ne tiennent pas ensemble sur celle-ci.
fn keep_together(ui: &mut Ui, label: &str, width: f32) {
    let text = egui::WidgetText::from(label).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Body,
    );
    let need = text.size().x + width + 3.0 * ui.spacing().item_spacing.x;
    if ui.available_size_before_wrap().x < need {
        ui.end_row();
    }
}

/// Pointeur d'une poignée.
/// Poignée sous le pointeur `pos`, pour un cadre à l'écran `frame` et une bande de prise
/// de `grab` pixels de part et d'autre de chaque bord ; `Move` à l'intérieur.
fn hit(frame: ScreenRect, grab: f32, pos: Pos2) -> Option<Handle> {
    let near = |a: f32, b: f32| (a - b).abs() <= grab;
    let (l, r, t, b) = (frame.left(), frame.right(), frame.top(), frame.bottom());
    let inside_x = pos.x > l - grab && pos.x < r + grab;
    let inside_y = pos.y > t - grab && pos.y < b + grab;
    let handle = match (
        near(pos.x, l) && inside_y,
        near(pos.x, r) && inside_y,
        near(pos.y, t) && inside_x,
        near(pos.y, b) && inside_x,
    ) {
        (true, _, true, _) => Handle::TopLeft,
        (_, true, true, _) => Handle::TopRight,
        (true, _, _, true) => Handle::BottomLeft,
        (_, true, _, true) => Handle::BottomRight,
        (true, _, _, _) => Handle::Left,
        (_, true, _, _) => Handle::Right,
        (_, _, true, _) => Handle::Top,
        (_, _, _, true) => Handle::Bottom,
        _ if frame.contains(pos) => Handle::Move,
        _ => return None,
    };
    Some(handle)
}

fn cursor(handle: Handle) -> CursorIcon {
    match handle {
        Handle::Move => CursorIcon::Grab,
        Handle::Left | Handle::Right => CursorIcon::ResizeHorizontal,
        Handle::Top | Handle::Bottom => CursorIcon::ResizeVertical,
        Handle::TopLeft | Handle::BottomRight => CursorIcon::ResizeNwSe,
        Handle::TopRight | Handle::BottomLeft => CursorIcon::ResizeNeSw,
    }
}

/// Icône du bouton d'orientation : un rectangle debout (portrait), couché (paysage) ou
/// carré, dans la couleur du texte du bouton.
fn orientation_icon(ui: &egui::Ui, response: &egui::Response, portrait: bool, square: bool) {
    let rect = response.rect;
    let long = rect.height() * 0.62;
    let short = if square { long } else { long * 0.66 };
    let size = if portrait {
        egui::vec2(short, long)
    } else {
        egui::vec2(long, short)
    };
    let color = ui.style().interact(response).fg_stroke.color;
    let color = if response.enabled() {
        color
    } else {
        ui.visuals().weak_text_color()
    };
    // Calé sur les pixels, largeur et hauteur arrondies une fois : sinon l'anticrénelage
    // d'un bord à cheval sur deux pixels amincissait le rectangle couché (retour du
    // 07/11/2026), debout et couché doivent avoir exactement les mêmes côtés.
    let ppp = ui.ctx().pixels_per_point();
    let px = |v: f32| (v * ppp).round() / ppp;
    let size = egui::vec2(px(size.x), px(size.y));
    let min = rect.center() - size / 2.0;
    let icon = egui::Rect::from_min_size(egui::pos2(px(min.x), px(min.y)), size);
    ui.painter().rect_stroke(
        icon,
        1.0,
        egui::Stroke::new(1.5, color),
        egui::StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_are_grabbed_on_both_sides_of_the_edge() {
        let frame = ScreenRect::from_min_max(egui::pos2(100.0, 100.0), egui::pos2(300.0, 200.0));
        let at = |x, y| hit(frame, 10.0, egui::pos2(x, y));
        // Bord gauche, par l'extérieur et par l'intérieur (issue n° 1 : la prise se juge
        // au point d'appui, à l'intérieur de la bande).
        assert_eq!(at(92.0, 150.0), Some(Handle::Left));
        assert_eq!(at(108.0, 150.0), Some(Handle::Left));
        assert_eq!(at(115.0, 150.0), Some(Handle::Move));
        assert_eq!(at(295.0, 195.0), Some(Handle::BottomRight));
        assert_eq!(at(200.0, 105.0), Some(Handle::Top));
        assert_eq!(at(80.0, 150.0), None);
    }
}
