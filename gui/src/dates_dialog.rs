//! Boîte « Trier par date de prise de vue ? » (26/11/2026, idée de l'utilisateur) : en mode
//! léger (NAS, partage réseau), trier par date de prise de vue fait lire la date de chaque
//! photo qui ne l'a pas encore ; sur un disque lent, c'est long. À partir de
//! `[gui] sort_dates_ask` dates à lire, la boîte prévient avant ; « Ne plus demander » met
//! ce réglage à 0. La boîte ne fait que demander.

use eframe::egui;

/// Ce que la boîte demande à l'application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// Trier quand même ; `always` : ne plus demander.
    Sort { always: bool },
    /// Fermée sans trier : l'ordre reste ce qu'il était.
    Close,
}

/// La boîte ouverte.
pub struct DatesDialog {
    /// Le tri demandé, appliqué si l'on confirme.
    pub(crate) order: tri::sort::Order,
    /// Dates à lire.
    count: usize,
    pub(crate) always: bool,
}

impl DatesDialog {
    /// Ouvre la boîte pour `order`, qui demande de lire `count` dates.
    pub fn new(order: tri::sort::Order, count: usize) -> Self {
        DatesDialog {
            order,
            count,
            always: false,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<Request> {
        let mut sort = false;
        let mut cancel = false;
        let modal = crate::app::modal_at("dates_dialog", crate::app::CENTRED)
            .title(t!("sort-dates-ask-title"))
            .show(ctx, |ui| {
                ui.label(t!("sort-dates-ask-text", n = self.count));
                ui.add_space(4.0);
                ui.weak(t!("sort-dates-ask-detail"));
                ui.add_space(6.0);
                ui.checkbox(&mut self.always, t!("sort-dates-ask-always"));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    sort = ui.button(t!("sort-dates-ask-sort")).clicked();
                    cancel = ui.button(t!("cancel")).clicked();
                });
            });
        if sort {
            return Some(Request::Sort {
                always: self.always,
            });
        }
        (cancel || modal.should_close()).then_some(Request::Close)
    }
}
