//! Le curseur de toute l'interface (retours du 10/11/2026) :
//!
//! - **glisser** avance par **grands crans** (`coarse`) ; un clic sur le rail, loin de la
//!   poignée, y place d'abord la valeur ;
//! - **Maj + glisser** règle **finement** : le déplacement devient relatif et
//!   démultiplié (`[gui] slider_fine_speed`, 1/10 par défaut), par petits crans (`fine`).
//!   Ctrl+clic est le clic droit sur macOS, d'où Maj (comme les champs de valeur d'egui) ;
//! - double-clic : à l'appelant de rendre la valeur par défaut (voir
//!   `SliderDoubleClick`), comme avant.
//!
//! Le passage de l'un à l'autre en cours de geste repart de la valeur atteinte : la poignée
//! ne saute jamais sous le pointeur.

use std::ops::RangeInclusive;

use eframe::egui::{self, Response, Sense, Ui, Widget, vec2};

/// Clé de la vitesse du réglage fin dans la mémoire d'egui (posée par l'application à
/// chaque image, d'après la configuration).
pub fn speed_id() -> egui::Id {
    egui::Id::unique("fine_slider_speed")
}

/// Vitesse du réglage fin par défaut, sans configuration.
const DEFAULT_FINE_SPEED: f32 = 0.1;

/// Un curseur de `range`, sur `value`.
pub struct FineSlider<'a> {
    value: &'a mut f32,
    range: RangeInclusive<f32>,
    coarse: f32,
    fine: f32,
    logarithmic: bool,
    width: Option<f32>,
    center_mark: bool,
}

/// Point de départ d'un glisser (mémoire d'egui, le temps du geste).
#[derive(Clone, Copy)]
struct Anchor {
    /// Position sur le rail (0..1) au départ.
    t: f32,
    /// Abscisse du pointeur au départ.
    x: f32,
    /// Réglage fin (Maj tenue).
    fine: bool,
}

impl<'a> FineSlider<'a> {
    /// Curseur sans cran (valeurs continues).
    pub fn new(value: &'a mut f32, range: RangeInclusive<f32>) -> Self {
        FineSlider {
            value,
            range,
            coarse: 0.0,
            fine: 0.0,
            logarithmic: false,
            width: None,
            center_mark: false,
        }
    }

    /// Crans : grand (glisser) et petit (Maj + glisser) ; 0 : continu.
    pub fn steps(mut self, coarse: f32, fine: f32) -> Self {
        self.coarse = coarse;
        self.fine = fine;
        self
    }

    /// Échelle logarithmique (bornes positives).
    pub fn logarithmic(mut self, on: bool) -> Self {
        self.logarithmic = on && *self.range.start() > 0.0;
        self
    }

    /// Largeur (par défaut : `slider_width` du style).
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Repère au milieu du rail (curseurs centrés : sévérité, similarité).
    pub fn center_mark(mut self, on: bool) -> Self {
        self.center_mark = on;
        self
    }

    fn to_t(&self, v: f32) -> f32 {
        let (lo, hi) = (*self.range.start(), *self.range.end());
        if hi <= lo {
            return 0.0;
        }
        let t = if self.logarithmic {
            (v.max(lo).ln() - lo.ln()) / (hi.ln() - lo.ln())
        } else {
            (v - lo) / (hi - lo)
        };
        t.clamp(0.0, 1.0)
    }

    fn value_at(&self, t: f32) -> f32 {
        let (lo, hi) = (*self.range.start(), *self.range.end());
        let t = t.clamp(0.0, 1.0);
        if self.logarithmic {
            (lo.ln() + t * (hi.ln() - lo.ln())).exp()
        } else {
            lo + t * (hi - lo)
        }
    }

    fn snap(&self, v: f32, step: f32) -> f32 {
        let v = if step > 0.0 {
            (v / step).round() * step
        } else {
            v
        };
        v.clamp(*self.range.start(), *self.range.end())
    }
}

impl Widget for FineSlider<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let width = self.width.unwrap_or(ui.spacing().slider_width).max(24.0);
        let height = ui.spacing().interact_size.y;
        let (rect, mut response) =
            ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
        let radius = (height / 2.5).max(4.0);
        let rail = rect.shrink2(vec2(radius, 0.0));
        let id = response.id;
        let old = *self.value;
        let mut value = old;
        let shift = ui.input(|i| i.modifiers.shift);
        let speed = ui
            .ctx()
            .data(|d| d.get_temp::<f32>(speed_id()))
            .unwrap_or(DEFAULT_FINE_SPEED)
            .clamp(0.001, 1.0);
        let along = |x: f32| (x - rail.left()) / rail.width().max(1.0);
        if let Some(pos) = response.interact_pointer_pos() {
            if response.drag_started() || response.dragged() {
                let current = self.to_t(value);
                let mut anchor = ui.ctx().data(|d| d.get_temp::<Anchor>(id));
                if response.drag_started() || anchor.is_none() {
                    // Loin de la poignée : la valeur va d'abord sous le pointeur.
                    let handle = rail.left() + current * rail.width();
                    let t = if (pos.x - handle).abs() > radius * 1.5 && !shift {
                        along(pos.x)
                    } else {
                        current
                    };
                    anchor = Some(Anchor {
                        t,
                        x: pos.x,
                        fine: shift,
                    });
                }
                let mut a = anchor.unwrap_or(Anchor {
                    t: current,
                    x: pos.x,
                    fine: shift,
                });
                let rate = |fine: bool| if fine { speed } else { 1.0 };
                let mut t = a.t + along(pos.x) - along(a.x);
                t = a.t + (t - a.t) * rate(a.fine);
                if a.fine != shift {
                    // Maj prise ou lâchée en cours de geste : on repart d'ici.
                    a = Anchor {
                        t,
                        x: pos.x,
                        fine: shift,
                    };
                }
                ui.ctx().data_mut(|d| d.insert_temp(id, a));
                let step = if shift { self.fine } else { self.coarse };
                value = self.snap(self.value_at(t), step);
            } else if response.clicked() {
                // Un clic sur le rail (pas sur la poignée : rien ne bouge).
                let handle = rail.left() + self.to_t(value) * rail.width();
                if (pos.x - handle).abs() > radius * 1.5 {
                    value = self.snap(self.value_at(along(pos.x)), self.coarse);
                }
            }
        }
        if response.drag_stopped() {
            ui.ctx().data_mut(|d| d.remove::<Anchor>(id));
        }
        if value != old {
            *self.value = value;
            response.mark_changed();
        }
        let shown = *self.value;
        response.widget_info(|| {
            egui::WidgetInfo::slider(ui.is_enabled(), f64::from(shown), String::new())
        });
        if ui.is_rect_visible(rect) {
            let visuals = *ui.style().interact(&response);
            let rail_h = ui.spacing().slider_rail_height.max(2.0);
            let rail_rect = egui::Rect::from_center_size(rect.center(), vec2(rect.width(), rail_h));
            let painter = ui.painter();
            painter.rect_filled(
                rail_rect,
                rail_h / 2.0,
                ui.visuals().widgets.inactive.bg_fill,
            );
            if self.center_mark {
                let x = rail.center().x;
                painter.line_segment(
                    [
                        egui::pos2(x, rect.top() + 1.0),
                        egui::pos2(x, rect.top() + rect.height() * 0.25),
                    ],
                    egui::Stroke::new(1.0, ui.visuals().weak_text_color()),
                );
            }
            let center = egui::pos2(
                rail.left() + self.to_t(shown) * rail.width(),
                rect.center().y,
            );
            painter.circle(
                center,
                radius + visuals.expansion,
                visuals.bg_fill,
                visuals.fg_stroke,
            );
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_snap_and_log_scales_map_both_ways() {
        let mut v = 0.0;
        let s = FineSlider::new(&mut v, -3.0..=3.0).steps(0.1, 0.01);
        assert!((s.snap(0.437, 0.1) - 0.4).abs() < 1e-6);
        assert!((s.snap(0.437, 0.01) - 0.44).abs() < 1e-6);
        assert_eq!(s.snap(9.0, 0.1), 3.0, "clamped");
        let mut k = 5500.0;
        let s = FineSlider::new(&mut k, 2000.0..=12000.0).logarithmic(true);
        for v in [2000.0f32, 3200.0, 6500.0, 12000.0] {
            assert!((s.value_at(s.to_t(v)) - v).abs() < 0.5, "{v}");
        }
        assert!(
            s.to_t(4899.0) > 0.45 && s.to_t(4899.0) < 0.55,
            "log midpoint"
        );
    }
}
