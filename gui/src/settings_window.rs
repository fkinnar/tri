//! Fenêtre de réglages (`Ctrl+,`) : tous les réglages du fichier de configuration, rangés
//! par section, avec une recherche floue comme l'aide.
//!
//! La liste vient du fichier d'exemple (`doc/config.example.toml`, garanti égal aux
//! valeurs du code), lu par le moteur ([`tri::config_edit::Schema`]) ; le fichier de
//! l'utilisateur est modifié sans perdre ses commentaires ni ses autres clés
//! ([`tri::config_edit::ConfigFile`]). Chaque modification est validée (le fichier entier
//! doit rester lisible) avant d'être gardée ; les réglages de l'interface s'appliquent
//! tout de suite, ceux du moteur au prochain lancement.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use eframe::egui::{self, Color32, Key, Modifiers, RichText, ScrollArea, Ui, vec2};
use tri::config_edit::{ConfigFile, Entry, Schema, Value, parse_color};

use crate::settings::AppSettings;

/// Le fichier d'exemple, embarqué : la liste des réglages.
const EXAMPLE: &str = include_str!("../../doc/config.example.toml");

fn schema() -> &'static Schema {
    static SCHEMA: OnceLock<Schema> = OnceLock::new();
    SCHEMA.get_or_init(|| Schema::parse(EXAMPLE).unwrap_or_default())
}

/// Vrai si `section`.`key` n'est relu qu'au lancement : le moteur (aperçus, cache, calculs,
/// emplacements) tourne déjà avec ses réglages ; la taille de la fenêtre ne sert qu'à
/// l'ouverture.
pub fn needs_restart(section: &str, key: &str) -> bool {
    matches!(
        section,
        "preview" | "render" | "cache" | "scheduler" | "exposure" | "sharpness" | "places" | "data"
    ) || (section, key) == ("gui", "window")
}

/// Identifiant du champ de recherche.
pub fn search_id() -> egui::Id {
    egui::Id::unique("settings_search")
}

/// Fenêtre ouverte.
pub struct SettingsWindow {
    file: ConfigFile,
    /// Section affichée (sans recherche).
    section: usize,
    query: String,
    /// Première image : la recherche prend le focus.
    fresh: bool,
    /// Textes en cours de saisie (texte, liste TOML), écrits à la validation (Entrée ou
    /// focus perdu), par (section, clé).
    drafts: HashMap<(String, String), String>,
    /// Dernière erreur de validation, par (section, clé).
    errors: HashMap<(String, String), String>,
    /// Modifié depuis le dernier enregistrement (écrit quand on lâche la souris : pas à
    /// chaque pas d'un glissé).
    dirty: bool,
    /// Erreur d'enregistrement du fichier.
    save_error: Option<String>,
}

/// Ce que la fenêtre demande à l'application.
pub enum Outcome {
    /// Rien de neuf.
    Open,
    /// Fermée.
    Closed,
    /// Réglages modifiés et valides : à appliquer.
    Changed(Box<AppSettings>),
}

impl SettingsWindow {
    /// Ouvre la fenêtre sur le fichier `path` (créé au premier changement s'il n'existe
    /// pas).
    pub fn open(path: &Path) -> Result<SettingsWindow, String> {
        Ok(SettingsWindow {
            file: ConfigFile::open(path)?,
            section: 0,
            query: String::new(),
            fresh: true,
            drafts: HashMap::new(),
            errors: HashMap::new(),
            dirty: false,
            save_error: None,
        })
    }

    /// Valeur actuelle : celle du fichier, sinon la valeur par défaut.
    fn current(&self, section: &str, entry: &Entry) -> Value {
        self.file
            .get(section, &entry.key)
            .unwrap_or_else(|| entry.default.clone())
    }

    /// Écrit `value` (ou retire la clé si `None`), puis valide le fichier entier ; refusé,
    /// l'ancienne valeur revient et l'erreur est gardée pour la ligne.
    fn commit(&mut self, section: &str, key: &str, value: Option<Value>) -> Option<AppSettings> {
        let id = (section.to_string(), key.to_string());
        let previous = self.file.get(section, key);
        let written = match &value {
            Some(v) => self.file.set(section, key, v),
            None => {
                self.file.remove(section, key);
                Ok(())
            }
        };
        let checked = written.and_then(|()| tri::settings::parse::<AppSettings>(&self.file.text()));
        match checked {
            Ok(settings) => {
                self.errors.remove(&id);
                self.dirty = true;
                Some(settings)
            }
            Err(e) => {
                match previous {
                    Some(v) => {
                        let _ = self.file.set(section, key, &v);
                    }
                    None => self.file.remove(section, key),
                }
                self.errors.insert(id, e);
                None
            }
        }
    }

    /// Affiche la fenêtre.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let schema = schema();
        let id = search_id();
        // Recherche vide et focus dessus : flèches haut / bas pour changer de section.
        if ctx.memory(|m| m.has_focus(id)) && self.query.trim().is_empty() {
            let (up, down) = ctx.input_mut(|i| {
                (
                    i.consume_key(Modifiers::NONE, Key::ArrowUp),
                    i.consume_key(Modifiers::NONE, Key::ArrowDown),
                )
            });
            let last = schema.sections.len().saturating_sub(1);
            if up {
                self.section = self.section.saturating_sub(1);
            }
            if down {
                self.section = (self.section + 1).min(last);
            }
        }
        let screen = ctx.content_rect().size();
        // Place des marges et du titre de la boîte comprise : elle tient dans la fenêtre.
        let size = vec2(screen.x * 0.8 - 60.0, screen.y * 0.8 - 160.0);
        let at = ctx.content_rect().center() - size / 2.0 - vec2(20.0, 30.0);
        let mut changed = None;
        let mut close = false;
        let title = t!("settings-title");
        let modal = crate::app::modal_at("settings_window", at)
            .title(title)
            .free_width()
            .show(ctx, |ui| {
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .id(id)
                        .hint_text(t!("settings-search-hint"))
                        .desired_width(size.x),
                );
                if std::mem::take(&mut self.fresh) {
                    search.request_focus();
                }
                ui.add_space(6.0);
                ui.allocate_ui(size, |ui| {
                    ui.set_min_size(size);
                    if self.query.trim().is_empty() {
                        ui.horizontal_top(|ui| {
                            self.section_list(ui, schema, size.y);
                            ui.separator();
                            ScrollArea::vertical()
                                .id_salt("settings_entries")
                                .max_height(size.y)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        if let Some(section) = schema.sections.get(self.section) {
                                            changed = self.section_entries(ui, section, None);
                                        }
                                    });
                                });
                        });
                    } else {
                        ScrollArea::vertical()
                            .id_salt("settings_found")
                            .max_height(size.y)
                            .auto_shrink([false, false])
                            .show(ui, |ui| ui.vertical(|ui| changed = self.found(ui, schema)));
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    close = ui.button(t!("close")).clicked();
                    ui.weak(t!("or-escape"));
                    ui.weak(t!(
                        "settings-file",
                        path = self.file.path.display().to_string()
                    ));
                    if let Some(e) = &self.save_error {
                        ui.colored_label(ui.visuals().error_fg_color, e);
                    }
                });
            });
        // Enregistré quand plus rien n'est en cours de glissé.
        if self.dirty && !ctx.input(|i| i.pointer.any_down()) {
            self.dirty = false;
            self.save_error = self.file.save().err().map(|e| {
                t!(
                    "settings-save-failed",
                    path = self.file.path.display().to_string(),
                    error = e.to_string()
                )
            });
        }
        if close || modal.should_close() {
            if self.dirty {
                let _ = self.file.save();
            }
            return Outcome::Closed;
        }
        match changed {
            Some(settings) => Outcome::Changed(Box::new(settings)),
            None => Outcome::Open,
        }
    }

    fn section_list(&mut self, ui: &mut Ui, schema: &Schema, height: f32) {
        ScrollArea::vertical()
            .id_salt("settings_sections")
            .max_height(height)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    for (i, section) in schema.sections.iter().enumerate() {
                        if ui
                            .selectable_label(self.section == i, &section.name)
                            .clicked()
                        {
                            self.section = i;
                        }
                    }
                });
            });
    }

    /// Les réglages d'une section ; `only` : les seules clés à montrer (recherche).
    fn section_entries(
        &mut self,
        ui: &mut Ui,
        section: &tri::config_edit::Section,
        only: Option<&[&str]>,
    ) -> Option<AppSettings> {
        let mut changed = None;
        if only.is_none() {
            ui.label(
                RichText::new(format!("[{}]", section.name))
                    .strong()
                    .monospace(),
            );
            if !section.doc.is_empty() {
                ui.weak(&section.doc);
            }
            if needs_restart(&section.name, "") {
                ui.label(t!("settings-section-restart"));
            }
            ui.add_space(6.0);
        }
        for group in &section.groups {
            let entries: Vec<&Entry> = group
                .entries
                .iter()
                .filter(|e| only.is_none_or(|keys| keys.contains(&e.key.as_str())))
                .collect();
            if entries.is_empty() {
                continue;
            }
            if !group.doc.is_empty() {
                ui.add_space(4.0);
                ui.weak(&group.doc);
            }
            for entry in entries {
                if let Some(s) = self.entry(ui, &section.name, entry) {
                    changed = Some(s);
                }
            }
        }
        changed
    }

    /// Résultats d'une recherche : par section, les clés trouvées.
    fn found(&mut self, ui: &mut Ui, schema: &Schema) -> Option<AppSettings> {
        let query = self.query.trim().to_string();
        let mut changed = None;
        let mut any = false;
        for section in &schema.sections {
            let keys: Vec<&str> = section
                .groups
                .iter()
                .flat_map(|g| g.entries.iter().map(move |e| (g, e)))
                .filter(|(g, e)| {
                    let text = format!("{}.{} {}", section.name, e.key, g.doc);
                    crate::actions::fuzzy_score(&query, &text).is_some()
                })
                .map(|(_, e)| e.key.as_str())
                .collect();
            if keys.is_empty() {
                continue;
            }
            any = true;
            ui.label(
                RichText::new(format!("[{}]", section.name))
                    .strong()
                    .monospace(),
            );
            if let Some(s) = self.section_entries(ui, section, Some(&keys)) {
                changed = Some(s);
            }
            ui.add_space(8.0);
        }
        if !any {
            ui.weak(t!("help-no-match"));
        }
        changed
    }

    /// Une ligne : nom, champ selon le type, retour à la valeur par défaut.
    fn entry(&mut self, ui: &mut Ui, section: &str, entry: &Entry) -> Option<AppSettings> {
        let id = (section.to_string(), entry.key.clone());
        let current = self.current(section, entry);
        let in_file = self.file.get(section, &entry.key).is_some();
        let mut new = None;
        let mut reset = false;
        ui.horizontal(|ui| {
            let name = RichText::new(&entry.key).monospace();
            let name = if in_file { name.strong() } else { name };
            ui.allocate_ui_with_layout(
                vec2(220.0, 18.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_width(220.0);
                    ui.add(egui::Label::new(name).truncate())
                        .on_hover_text(format!("{section}.{}", entry.key));
                },
            );
            match &current {
                Value::Bool(b) => {
                    let mut v = *b;
                    if ui.checkbox(&mut v, "").changed() {
                        new = Some(Value::Bool(v));
                    }
                }
                Value::Int(i) => {
                    let mut v = *i;
                    if ui.add(egui::DragValue::new(&mut v)).changed() {
                        new = Some(Value::Int(v));
                    }
                }
                Value::Float(x) => {
                    let mut v = *x;
                    let scale = match &entry.default {
                        Value::Float(d) if *d != 0.0 => d.abs(),
                        _ => 1.0,
                    };
                    let drag = egui::DragValue::new(&mut v)
                        .speed(scale * 0.01)
                        .max_decimals(6);
                    if ui.add(drag).changed() {
                        new = Some(Value::Float(v));
                    }
                }
                Value::Str(s) if parse_color(s).is_some() => {
                    let mut rgb = parse_color(s).unwrap_or_default();
                    if ui.color_edit_button_srgb(&mut rgb).changed() {
                        new = Some(Value::Str(format!(
                            "#{:02x}{:02x}{:02x}",
                            rgb[0], rgb[1], rgb[2]
                        )));
                    }
                    ui.weak(s);
                }
                other => {
                    // Texte libre ou liste TOML : écrit à la validation.
                    let shown = match other {
                        Value::Str(s) => s.clone(),
                        v => v.to_string(),
                    };
                    let draft = self.drafts.entry(id.clone()).or_insert(shown.clone());
                    let field = ui.add(egui::TextEdit::singleline(draft).desired_width(260.0));
                    let done = field.lost_focus() && *draft != shown;
                    if done {
                        new = Some(match other {
                            Value::Str(_) => Value::Str(draft.clone()),
                            _ => Value::Toml(draft.clone()),
                        });
                    }
                    if !field.has_focus() && !done {
                        *draft = shown;
                    }
                }
            }
            if in_file
                && ui
                    .small_button(t!("settings-reset"))
                    .on_hover_text(t!("settings-default", value = entry.default.to_string()))
                    .clicked()
            {
                reset = true;
            }
            // Toute la section au prochain lancement : dit une fois, en tête.
            if needs_restart(section, &entry.key) && !needs_restart(section, "") {
                ui.weak(t!("settings-restart"));
            }
        });
        if let Some(e) = self.errors.get(&id) {
            ui.colored_label(Color32::from_rgb(230, 90, 90), e);
        }
        if reset {
            self.drafts.remove(&id);
            return self.commit(section, &entry.key, None);
        }
        let value = new?;
        self.drafts.remove(&id);
        self.commit(section, &entry.key, Some(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_of_the_example_is_listed() {
        let names: Vec<&str> = schema().sections.iter().map(|s| s.name.as_str()).collect();
        for wanted in ["preview", "files", "gui", "gui.colors", "gui.histogram"] {
            assert!(names.contains(&wanted), "{wanted} : {names:?}");
        }
        assert!(needs_restart("cache", "dir") && !needs_restart("gui.colors", "hover"));
        assert!(needs_restart("gui", "window") && !needs_restart("gui", "theme"));
    }

    #[test]
    fn an_invalid_value_is_refused_and_a_valid_one_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut w = SettingsWindow::open(&path).unwrap();
        // Une chaîne là où le moteur attend un nombre : refusée, rien d'écrit.
        assert!(
            w.commit("preview", "thumbnail", Some(Value::Str("grand".into())))
                .is_none()
        );
        assert!(
            w.errors
                .contains_key(&("preview".into(), "thumbnail".into()))
        );
        assert_eq!(w.file.get("preview", "thumbnail"), None, "ancienne valeur");
        let s = w
            .commit("preview", "thumbnail", Some(Value::Int(500)))
            .expect("valide");
        assert_eq!(s.preview.thumbnail, 500);
        assert!(w.errors.is_empty());
        // Retour à la valeur par défaut : la clé disparaît du fichier.
        let s = w.commit("preview", "thumbnail", None).unwrap();
        assert_eq!(
            s.preview.thumbnail,
            AppSettings::default().preview.thumbnail
        );
    }
}
