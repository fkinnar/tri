//! Groupes d'un menu contextuel, en sous-menu ou dépliés au premier niveau (issue n° 406,
//! idée de l'utilisateur, 10/11/2026).
//!
//! Un groupe est par défaut un sous-menu. Un petit bouton, en haut à droite du sous-menu,
//! l'installe au premier niveau, à sa place, entre deux traits ; un titre discret (petites
//! capitales, gris) sur le trait du haut dit d'où il vient, et l'icône à sa droite le
//! replie. Le choix est un réglage du logiciel (la liste des groupes dépliés), le même
//! pour tous les dossiers : chacun se compose son menu.

use eframe::egui::{self, Color32, Painter, Rect, Sense, Stroke, Ui, vec2};

use crate::marks;

/// Côté des petites icônes de groupe (points).
const ICON: f32 = 14.0;

/// Les groupes d'une partie d'un menu, posés l'un après l'autre.
pub struct MenuGroups<'a> {
    /// Identifiants des groupes dépliés (réglage).
    inline: &'a [String],
    /// Groupe dont l'utilisateur vient de changer l'état.
    toggled: Option<&'static str>,
    /// Aucun groupe posé encore.
    first: bool,
    /// Le groupe précédent était déplié.
    after_inline: bool,
    /// Bulles des deux icônes : déplier, replier.
    tips: (String, String),
}

impl<'a> MenuGroups<'a> {
    /// Une partie de menu ; `tips` = bulles des icônes « au premier niveau » et
    /// « en sous-menu ».
    pub fn new(inline: &'a [String], tips: (String, String)) -> Self {
        MenuGroups {
            inline,
            toggled: None,
            first: true,
            after_inline: false,
            tips,
        }
    }

    /// Le groupe `id` (`title` à l'écran), son contenu ajouté par `add`.
    pub fn group(&mut self, ui: &mut Ui, id: &'static str, title: &str, add: impl FnOnce(&mut Ui)) {
        if self.inline.iter().any(|g| g == id) {
            // Le titre sur un trait sert de séparateur au-dessus.
            if header(ui, title, Some((false, &self.tips.1))) {
                self.toggled = Some(id);
            }
            add(ui);
            self.after_inline = true;
        } else {
            if self.first || self.after_inline {
                ui.separator();
            }
            let tip = self.tips.0.clone();
            let mut toggled = false;
            ui.menu_button(title, |ui| {
                toggled = pin_row(ui, &tip);
                add(ui);
            });
            if toggled {
                self.toggled = Some(id);
            }
            self.after_inline = false;
        }
        self.first = false;
    }

    /// Ferme la partie (un trait sous le dernier groupe) ; rend le groupe à basculer,
    /// s'il y en a un.
    pub fn finish(self, ui: &mut Ui) -> Option<&'static str> {
        if !self.first {
            ui.separator();
        }
        self.toggled
    }
}

/// Liste `inline` après la bascule du groupe `id`.
pub fn toggle(inline: &[String], id: &str) -> Vec<String> {
    if inline.iter().any(|g| g == id) {
        inline.iter().filter(|g| *g != id).cloned().collect()
    } else {
        let mut out = inline.to_vec();
        out.push(id.to_string());
        out
    }
}

/// Sous-titre à l'intérieur d'un groupe (« Réglages auto »…) : petites capitales grises
/// sur un trait, comme le titre d'un groupe déplié, sans icône.
pub fn subtitle(ui: &mut Ui, title: &str) {
    header(ui, title, None);
}

/// Titre sur un trait, avec l'icône de bascule à droite (`Some((déplier, bulle))`) ;
/// vrai si l'icône est cliquée.
fn header(ui: &mut Ui, title: &str, icon: Option<(bool, &str)>) -> bool {
    let color = ui.visuals().weak_text_color();
    let font = egui::FontId::proportional(ui.style().text_styles[&egui::TextStyle::Small].size);
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_uppercase(), font, color);
    let gap = ui.spacing().item_spacing.x;
    let height = galley.size().y.max(ICON) + 2.0;
    let wanted = galley.size().x + gap * 3.0 + if icon.is_some() { ICON } else { 0.0 };
    // Passe de mesure : juste le titre ; ensuite toute la largeur du menu (comme un trait).
    let width = if ui.is_sizing_pass() {
        wanted
    } else {
        ui.available_size_before_wrap().x.max(wanted)
    };
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let y = rect.center().y;
    let text_pos = egui::pos2(rect.left(), y - galley.size().y / 2.0);
    let text_right = rect.left() + galley.size().x;
    let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
    let mut clicked = false;
    let line_end = if let Some((pin, tip)) = icon {
        let icon_rect =
            Rect::from_center_size(egui::pos2(rect.right() - ICON / 2.0, y), vec2(ICON, ICON));
        let response = icon_response(ui, icon_rect, tip);
        clicked = response.clicked();
        let hot = response.hovered();
        paint_icon(ui, icon_rect, pin, hot);
        icon_rect.left() - gap
    } else {
        rect.right()
    };
    if ui.is_rect_visible(rect) {
        ui.painter().galley(text_pos, galley, color);
        if line_end > text_right + gap {
            ui.painter().hline(
                (text_right + gap)..=line_end,
                y,
                Stroke::new(stroke.width, stroke.color),
            );
        }
    }
    clicked
}

/// Ligne du haut d'un sous-menu : l'icône « au premier niveau », à droite ; vrai si
/// cliquée.
fn pin_row(ui: &mut Ui, tip: &str) -> bool {
    let width = if ui.is_sizing_pass() {
        ICON
    } else {
        ui.available_size_before_wrap().x.max(ICON)
    };
    let (rect, _) = ui.allocate_exact_size(vec2(width, ICON), Sense::hover());
    let icon_rect = Rect::from_min_size(
        egui::pos2(rect.right() - ICON, rect.top()),
        vec2(ICON, ICON),
    );
    let response = icon_response(ui, icon_rect, tip);
    let hot = response.hovered();
    paint_icon(ui, icon_rect, true, hot);
    response.clicked()
}

fn icon_response(ui: &mut Ui, rect: Rect, tip: &str) -> egui::Response {
    let id = ui.next_auto_id();
    ui.skip_ahead_auto_ids(1);
    let response = ui.interact(rect, id, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tip));
    response
        .on_hover_text(tip)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn paint_icon(ui: &Ui, rect: Rect, pin: bool, hot: bool) {
    let color = if hot {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    if hot {
        let v = ui.visuals().widgets.hovered;
        ui.painter()
            .rect_filled(rect, v.corner_radius, v.weak_bg_fill);
    }
    draw(ui.painter(), rect.shrink(1.0), pin, color);
}

fn draw(painter: &Painter, rect: Rect, pin: bool, color: Color32) {
    if pin {
        marks::menu_inline(painter, rect, color);
    } else {
        marks::menu_fold(painter, rect, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_adds_or_removes_a_group() {
        let inline = vec!["files".to_string()];
        assert_eq!(toggle(&inline, "notes"), ["files", "notes"]);
        assert_eq!(toggle(&inline, "files"), Vec::<String>::new());
    }
}
