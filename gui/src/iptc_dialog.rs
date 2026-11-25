//! Boîte « Métadonnées de création » (#277) : poser un jeu `[iptc.sets.<id>]` (auteur,
//! copyright…) sur la sélection. Le jeu se choisit dans une liste, ce qu'il pose est
//! rappelé ; un jeu vide renvoie aux réglages. Le travail est fait par le moteur
//! ([`tri::iptc::stamp`]), en fond.

use std::path::PathBuf;

use eframe::egui;
use tri::iptc::{FIELDS, IptcSettings};

/// Ce que la boîte demande à l'application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Poser le jeu `id` sur les images.
    Apply(String),
    /// Ouvrir les réglages sur cette section (remplir le jeu).
    Settings(String),
    /// Fermée.
    Close,
}

/// La boîte ouverte.
pub struct IptcDialog {
    /// Images visées.
    pub(crate) images: Vec<PathBuf>,
    /// Jeu choisi (identifiant).
    pub(crate) chosen: String,
}

/// Nom affiché d'un jeu : le sien, sinon « Mes métadonnées » pour le jeu fourni, sinon
/// l'identifiant.
pub fn set_name(id: &str, settings: &IptcSettings) -> String {
    match settings.sets.0.get(id).map(|s| s.name.trim()) {
        Some(name) if !name.is_empty() => name.to_string(),
        _ if id == tri::iptc::MINE => t!("iptc-set-mine"),
        _ => id.to_string(),
    }
}

impl IptcDialog {
    /// La boîte pour `images`, sur le jeu par défaut.
    pub fn new(images: Vec<PathBuf>, settings: &IptcSettings) -> Self {
        let chosen = settings
            .set("")
            .map(|(id, _)| id.to_string())
            .unwrap_or_default();
        IptcDialog { images, chosen }
    }

    pub fn show(&mut self, ctx: &egui::Context, settings: &IptcSettings) -> Option<Request> {
        let mut request = None;
        let set = settings.set(&self.chosen).map(|(_, s)| s.clone());
        let modal = crate::app::modal_at("iptc_dialog", crate::app::CENTRED)
            .title(t!("iptc-title", n = self.images.len()))
            .show(ctx, |ui| {
                if settings.sets.0.len() > 1 {
                    ui.horizontal(|ui| {
                        ui.label(t!("iptc-set"));
                        egui::ComboBox::from_id_salt("iptc_set")
                            .selected_text(set_name(&self.chosen, settings))
                            .show_ui(ui, |ui| {
                                for id in settings.sets.0.keys() {
                                    ui.selectable_value(
                                        &mut self.chosen,
                                        id.clone(),
                                        set_name(id, settings),
                                    );
                                }
                            });
                    });
                    ui.add_space(4.0);
                }
                let section = format!("iptc.sets.{}", self.chosen);
                match &set {
                    Some(set) if !set.is_empty() => {
                        egui::Grid::new("iptc_fields")
                            .num_columns(2)
                            .show(ui, |ui| {
                                for f in FIELDS {
                                    let value = set.value(f.key).trim();
                                    if value.is_empty() {
                                        continue;
                                    }
                                    ui.weak(t!(&format!("iptc-field-{}", f.key.replace('_', "-"))));
                                    let replaced = set.replace.iter().any(|r| r == f.key);
                                    ui.label(if replaced {
                                        format!("{value}  ({})", t!("iptc-replaced"))
                                    } else {
                                        value.to_string()
                                    });
                                    ui.end_row();
                                }
                            });
                        ui.add_space(4.0);
                        ui.weak(t!("iptc-where"));
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button(t!("iptc-apply")).clicked() {
                                request = Some(Request::Apply(self.chosen.clone()));
                            }
                            if ui.button(t!("cancel")).clicked() {
                                request = Some(Request::Close);
                            }
                            if ui.button(t!("iptc-edit")).clicked() {
                                request = Some(Request::Settings(section.clone()));
                            }
                        });
                    }
                    _ => {
                        ui.label(t!("iptc-empty"));
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button(t!("iptc-edit")).clicked() {
                                request = Some(Request::Settings(section.clone()));
                            }
                            if ui.button(t!("cancel")).clicked() {
                                request = Some(Request::Close);
                            }
                        });
                    }
                }
            });
        if request.is_none() && modal.should_close() {
            request = Some(Request::Close);
        }
        request
    }
}
