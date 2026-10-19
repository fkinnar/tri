//! Fenêtre de réglages (`Ctrl+,`) : tous les réglages du fichier de configuration, rangés
//! par section, avec une recherche floue comme l'aide.
//!
//! La liste vient du fichier d'exemple (`doc/config.example.toml`, garanti égal aux
//! valeurs du code), lu par le moteur ([`tri::config_edit::Schema`]) ; le fichier de
//! l'utilisateur est modifié sans perdre ses commentaires ni ses autres clés
//! ([`tri::config_edit::ConfigFile`]). Chaque modification est validée (le fichier entier
//! doit rester lisible) avant d'être gardée ; les réglages de l'interface s'appliquent
//! tout de suite, ceux du moteur au prochain lancement.
//!
//! La section `[keys]` (raccourcis) n'a pas de clés dans l'exemple : elle a son propre
//! volet, une ligne par action, ses raccourcis en pastilles (retirer, enregistrer la
//! prochaine touche tapée, taper une séquence), conflits signalés.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Event, Key, Modifiers, RichText, ScrollArea, Ui, vec2};

use crate::actions::{Action, Chord, Group, Keymap, keys_spec, parse_keys, spec_label};
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
        "preview"
            | "render"
            | "cache"
            | "scheduler"
            | "exposure"
            | "sharpness"
            | "similarity"
            | "places"
            | "data"
    ) || (section, key) == ("gui", "window")
}

/// Libellé lisible d'une section (« Couleurs » pour `gui.colors`), traduit ; à défaut,
/// son nom.
pub fn section_label(name: &str) -> String {
    let id = format!("settings-section-{}", name.replace('.', "-"));
    if tri::i18n::has(tri::i18n::language(), &id) {
        tri::i18n::translate(&id, None)
    } else {
        name.to_string()
    }
}

/// « Libellé [section] » : le libellé, et le nom technique entre crochets dans un gris
/// plus discret (celui du fichier de configuration).
fn section_title(ui: &Ui, name: &str, strong: bool) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let style = ui.style();
    let mut label = RichText::new(section_label(name)).color(ui.visuals().text_color());
    if strong {
        label = label.strong();
    }
    label.append_to(
        &mut job,
        style,
        egui::FontSelection::Default,
        egui::Align::Center,
    );
    RichText::new(format!("  [{name}]"))
        .color(ui.visuals().weak_text_color())
        .monospace()
        .append_to(
            &mut job,
            style,
            egui::FontSelection::Default,
            egui::Align::Center,
        );
    job
}

/// Vrai si la clé désigne un dossier : saisie avec complétion par Tab.
/// Valeurs possibles d'un réglage à choix (liste déroulante plutôt que texte libre). Un
/// test vérifie que chacune est acceptée.
fn choices(section: &str, key: &str) -> Option<&'static [&'static str]> {
    Some(match (section, key) {
        ("gui", "profile") => &["classic", "vim", "both"],
        ("gui", "theme") => &["dark", "light", "system"],
        ("gui", "raw_previews") => &["on-demand", "background"],
        ("xmp", "sidecar") => &["darktable", "adobe"],
        ("i18n", "language") => &["auto", "fr", "en"],
        ("gui.view", "focus_marker") => &["square", "cross", "circle", "heart"],
        ("gui.histogram", "scale") => &["ev", "linear"],
        _ => return None,
    })
}

fn is_folder_key(key: &str) -> bool {
    key == "dir" || key.ends_with("_dir") || key == "destination"
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
    /// Champs de chemin (complétion par Tab), par (section, clé).
    paths: HashMap<(String, String), crate::path_field::PathField>,
    /// Dernière erreur de validation, par (section, clé).
    errors: HashMap<(String, String), String>,
    /// Modifié depuis le dernier enregistrement (écrit quand on lâche la souris : pas à
    /// chaque pas d'un glissé).
    dirty: bool,
    /// Erreur d'enregistrement du fichier.
    save_error: Option<String>,
    /// Raccourcis en vigueur d'après le fichier (profils, leader, `[keys]`), et les
    /// problèmes signalés (conflits…).
    keymap: Keymap,
    key_warnings: Vec<String>,
    /// Raccourci en cours d'enregistrement : l'action, les combinaisons déjà tapées, et
    /// l'instant de la dernière (fini après le délai des séquences sans nouvelle touche).
    recording: Option<(Action, Vec<Chord>, Instant)>,
    sequence_timeout: Duration,
    /// Section changée au clavier : la liste défile jusqu'à elle, les réglages repartent
    /// du haut (retour du 18/10/2026 : la section choisie sortait de la vue).
    section_moved: bool,
    /// Entrer dans la partie droite au clavier (→, Entrée ou Tab depuis la recherche) :
    /// le premier réglage montré prend le focus.
    focus_first: bool,
    /// Défilement de la liste des sections (tests).
    #[cfg(test)]
    sections_offset: f32,
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
        let file = ConfigFile::open(path)?;
        let settings = tri::settings::parse::<AppSettings>(&file.text()).unwrap_or_default();
        let (keymap, key_warnings) = settings.keymap();
        Ok(SettingsWindow {
            keymap,
            key_warnings,
            recording: None,
            section_moved: false,
            focus_first: false,
            #[cfg(test)]
            sections_offset: 0.0,
            sequence_timeout: settings.gui.key_sequence_timeout(),
            file,
            section: 0,
            query: String::new(),
            fresh: true,
            drafts: HashMap::new(),
            paths: HashMap::new(),
            errors: HashMap::new(),
            dirty: false,
            save_error: None,
        })
    }

    /// Affiche la section `name` (`keys`, `gui.zoom`), si elle existe.
    pub fn show_section(&mut self, name: &str) {
        if let Some(i) = schema().sections.iter().position(|s| s.name == name) {
            self.section = i;
        }
    }

    /// Vrai pendant l'enregistrement d'un raccourci (les touches lui reviennent).
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
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
                (self.keymap, self.key_warnings) = settings.keymap();
                self.sequence_timeout = settings.gui.key_sequence_timeout();
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
        // Raccourci en cours d'enregistrement : les touches lui reviennent (Échap compris).
        let mut recorded = self.record(ctx);
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
            self.section_moved |= up || down;
        }
        // Depuis la recherche : →, Entrée ou Tab entrent dans les réglages (retour du
        // 18/10/2026 : la partie droite n'était pas joignable au clavier, Tab passant
        // d'abord par toutes les sections).
        if ctx.memory(|m| m.has_focus(id)) {
            let empty = self.query.trim().is_empty();
            let enter = ctx.input_mut(|i| {
                i.consume_key(Modifiers::NONE, Key::Enter)
                    | i.consume_key(Modifiers::NONE, Key::Tab)
                    | (empty && i.consume_key(Modifiers::NONE, Key::ArrowRight))
            });
            self.focus_first |= enter;
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
                            let mut entries = ScrollArea::vertical();
                            if self.section_moved {
                                entries = entries.vertical_scroll_offset(0.0);
                            }
                            entries
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
                            self.section_moved = false;
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
        match changed.or(recorded.take()) {
            Some(settings) => Outcome::Changed(Box::new(settings)),
            None => Outcome::Open,
        }
    }

    fn section_list(&mut self, ui: &mut Ui, schema: &Schema, height: f32) {
        let _output = ScrollArea::vertical()
            .id_salt("settings_sections")
            .max_height(height)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    for (i, section) in schema.sections.iter().enumerate() {
                        let label = ui.selectable_label(
                            self.section == i,
                            section_title(ui, &section.name, false),
                        );
                        if self.section == i && self.section_moved {
                            label.scroll_to_me(None);
                        }
                        if label.clicked() && self.section != i {
                            self.section = i;
                            self.section_moved = true;
                        }
                    }
                });
            });
        #[cfg(test)]
        {
            self.sections_offset = _output.state.clamped_offset().y;
        }
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
            ui.label(section_title(ui, &section.name, true));
            if !section.doc.is_empty() {
                ui.weak(&section.doc);
            }
            if needs_restart(&section.name, "") {
                ui.label(t!("settings-section-restart"));
            }
            ui.add_space(6.0);
            if section.name == "keys" {
                return self.keys_entries(ui, None);
            }
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
            if section.name == "keys" {
                if self.matching_actions(Some(&query)).next().is_some() {
                    any = true;
                    ui.label(section_title(ui, &section.name, true));
                    if let Some(s) = self.keys_entries(ui, Some(&query)) {
                        changed = Some(s);
                    }
                    ui.add_space(8.0);
                }
                continue;
            }
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
            ui.label(section_title(ui, &section.name, true));
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

    /// Enregistrement d'un raccourci : prend les touches tapées (retirées des événements,
    /// pour que ni la fenêtre ni l'application ne les voient) ; fini après le délai des
    /// séquences sans nouvelle touche, il est ajouté aux raccourcis de l'action.
    fn record(&mut self, ctx: &egui::Context) -> Option<AppSettings> {
        let (action, chords, last) = self.recording.as_mut()?;
        let events = ctx.input_mut(|i| {
            let events = i.events.clone();
            i.events
                .retain(|e| !matches!(e, Event::Key { .. } | Event::Text(_)));
            events
        });
        // Un caractère sans touche nommée (« ° », « + » selon le clavier) : le caractère,
        // comme les raccourcis des profils ; sinon la touche.
        let typed: Vec<char> = events
            .iter()
            .filter_map(|e| match e {
                Event::Text(t) => {
                    let mut chars = t.chars();
                    let c = chars.next()?;
                    (chars.next().is_none() && !c.is_ascii_alphanumeric() && c != ' ').then_some(c)
                }
                _ => None,
            })
            .collect();
        let now = Instant::now();
        for event in &events {
            if let Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
                && let Some(chord) = Chord::from_event(*key, *modifiers)
            {
                let chord = match typed.first() {
                    Some(&c) if !chord.ctrl => Chord::char(c),
                    _ => chord,
                };
                chords.push(chord);
                *last = now;
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        if chords.is_empty() || now.duration_since(*last) < self.sequence_timeout {
            return None;
        }
        let (action, chords) = (*action, std::mem::take(chords));
        self.recording = None;
        self.add_key(action, keys_spec(&chords))
    }

    /// Ajoute `spec` aux raccourcis de `action` (écrits dans `[keys]`).
    fn add_key(&mut self, action: Action, spec: String) -> Option<AppSettings> {
        let mut specs = self.keymap.specs_for(action);
        if specs.contains(&spec) {
            return None;
        }
        specs.push(spec);
        self.commit("keys", action.id(), Some(Value::Strings(specs)))
    }

    /// Actions du volet des raccourcis, dans l'ordre de l'aide ; `query` : celles qui
    /// correspondent (libellé, identifiant, raccourcis).
    fn matching_actions<'a>(&'a self, query: Option<&'a str>) -> impl Iterator<Item = Action> + 'a {
        Group::ALL.into_iter().flat_map(move |group| {
            Action::ALL.into_iter().filter(move |a| {
                a.group() == group
                    && a.available()
                    && query.is_none_or(|q| {
                        let keys = self.keymap.keys_for(*a).join(" ");
                        let text = format!("{} {} {keys}", a.label(), a.id());
                        crate::actions::fuzzy_score(q, &text).is_some()
                    })
            })
        })
    }

    /// Volet des raccourcis : problèmes en tête, puis une ligne par action, par thème.
    fn keys_entries(&mut self, ui: &mut Ui, query: Option<&str>) -> Option<AppSettings> {
        if query.is_none() {
            ui.weak(t!("settings-keys-doc"));
            ui.add_space(4.0);
            for warning in &self.key_warnings {
                ui.colored_label(ui.visuals().error_fg_color, warning);
            }
        }
        let actions: Vec<Action> = self.matching_actions(query).collect();
        let mut changed = None;
        let mut group = None;
        for action in actions {
            if group != Some(action.group()) {
                group = Some(action.group());
                ui.add_space(6.0);
                ui.strong(action.group().label());
            }
            if let Some(s) = self.key_row(ui, action) {
                changed = Some(s);
            }
        }
        changed
    }

    /// Une action : son libellé, ses raccourcis en pastilles (clic : retiré), enregistrer,
    /// taper un raccourci, retour aux raccourcis du profil.
    fn key_row(&mut self, ui: &mut Ui, action: Action) -> Option<AppSettings> {
        let id = ("keys".to_string(), action.id().to_string());
        let in_file = self.file.get("keys", action.id()).is_some();
        let specs = self.keymap.specs_for(action);
        let mut new: Option<Vec<String>> = None;
        let mut typed = None;
        let mut reset = false;
        ui.horizontal(|ui| {
            let name = RichText::new(action.label());
            let name = if in_file { name.strong() } else { name };
            ui.allocate_ui_with_layout(
                vec2(240.0, 18.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_width(240.0);
                    ui.add(egui::Label::new(name).truncate())
                        .on_hover_text(action.id());
                },
            );
            if specs.is_empty() {
                ui.weak(t!("settings-keys-none"));
            }
            for (i, spec) in specs.iter().enumerate() {
                let chip = egui::Button::new(format!("{}  ×", spec_label(spec))).small();
                if ui
                    .add(chip)
                    .on_hover_text(t!("settings-keys-remove", keys = spec.as_str()))
                    .clicked()
                {
                    let mut list = specs.clone();
                    list.remove(i);
                    new = Some(list);
                }
            }
            match &self.recording {
                Some((a, chords, _)) if *a == action => {
                    let so_far = keys_spec(chords);
                    ui.label(
                        RichText::new(t!("settings-keys-recording", keys = so_far))
                            .color(ui.visuals().warn_fg_color),
                    );
                    if ui.small_button(t!("cancel")).clicked() {
                        self.recording = None;
                    }
                }
                _ => {
                    let record = ui
                        .small_button(t!("settings-keys-record"))
                        .on_hover_text(t!("settings-keys-record-tip"));
                    if self.focus_first {
                        record.request_focus();
                        self.focus_first = false;
                    }
                    if record.clicked() {
                        self.recording = Some((action, Vec::new(), Instant::now()));
                    }
                    let draft = self.drafts.entry(id.clone()).or_default();
                    let field = ui.add(
                        egui::TextEdit::singleline(draft)
                            .hint_text(t!("settings-keys-type-hint"))
                            .desired_width(90.0),
                    );
                    if field.lost_focus()
                        && ui.input(|i| i.key_pressed(Key::Enter))
                        && !draft.trim().is_empty()
                    {
                        typed = Some(draft.trim().to_string());
                    }
                }
            }
            if in_file
                && ui
                    .small_button(t!("settings-reset"))
                    .on_hover_text(t!("settings-keys-default"))
                    .clicked()
            {
                reset = true;
            }
        });
        for warning in self.key_warnings.iter().filter(|w| w.contains(action.id())) {
            ui.colored_label(ui.visuals().warn_fg_color, warning);
        }
        if let Some(e) = self.errors.get(&id) {
            ui.colored_label(Color32::from_rgb(230, 90, 90), e);
        }
        if reset {
            return self.commit("keys", action.id(), None);
        }
        if let Some(spec) = typed {
            match parse_keys(&spec) {
                Ok(keys) => {
                    self.drafts.remove(&id);
                    self.errors.remove(&id);
                    return self.add_key(action, keys_spec(&keys));
                }
                Err(e) => {
                    self.errors.insert(id, e);
                    return None;
                }
            }
        }
        let list = new?;
        self.commit("keys", action.id(), Some(Value::Strings(list)))
    }

    /// Une ligne : nom, champ selon le type, retour à la valeur par défaut.
    fn entry(&mut self, ui: &mut Ui, section: &str, entry: &Entry) -> Option<AppSettings> {
        let id = (section.to_string(), entry.key.clone());
        let current = self.current(section, entry);
        let in_file = self.file.get(section, &entry.key).is_some();
        let mut new = None;
        let mut reset = false;
        let mut widget: Option<egui::Response> = None;
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
                    let r = ui.checkbox(&mut v, "");
                    if r.changed() {
                        new = Some(Value::Bool(v));
                    }
                    widget = Some(r);
                }
                Value::Int(i) => {
                    let mut v = *i;
                    let r = ui.add(egui::DragValue::new(&mut v));
                    if r.changed() {
                        new = Some(Value::Int(v));
                    }
                    widget = Some(r);
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
                    let r = ui.add(drag);
                    if r.changed() {
                        new = Some(Value::Float(v));
                    }
                    widget = Some(r);
                }
                Value::Str(s) if choices(section, &entry.key).is_some() => {
                    let mut v = s.clone();
                    let combo = egui::ComboBox::from_id_salt(("settings_choice", &id))
                        .selected_text(&v)
                        .show_ui(ui, |ui| {
                            for choice in choices(section, &entry.key).unwrap_or_default() {
                                ui.selectable_value(&mut v, choice.to_string(), *choice);
                            }
                        });
                    widget = Some(combo.response);
                    if v != *s {
                        new = Some(Value::Str(v));
                    }
                }
                Value::Str(s) if parse_color(s).is_some() => {
                    let mut rgb = parse_color(s).unwrap_or_default();
                    let r = ui.color_edit_button_srgb(&mut rgb);
                    if r.changed() {
                        new = Some(Value::Str(format!(
                            "#{:02x}{:02x}{:02x}",
                            rgb[0], rgb[1], rgb[2]
                        )));
                    }
                    widget = Some(r);
                    ui.weak(s);
                }
                other => {
                    // Texte libre ou liste TOML : écrit à la validation.
                    let shown = match other {
                        Value::Str(s) => s.clone(),
                        v => v.to_string(),
                    };
                    let draft = self.drafts.entry(id.clone()).or_insert(shown.clone());
                    let field = if is_folder_key(&entry.key) {
                        let path = self.paths.entry(id.clone()).or_default();
                        let field_id = egui::Id::unique("settings_path").with(&id);
                        ui.vertical(|ui| path.show(ui, field_id, draft, 260.0))
                            .inner
                    } else {
                        ui.add(egui::TextEdit::singleline(draft).desired_width(260.0))
                    };
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
                    widget = Some(field);
                }
            }
            if let Some(w) = widget.as_ref().filter(|_| self.focus_first) {
                w.request_focus();
                self.focus_first = false;
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
    fn every_section_has_a_label_in_both_languages() {
        crate::i18n::ensure();
        for section in &schema().sections {
            let id = format!("settings-section-{}", section.name.replace('.', "-"));
            for lang in [tri::i18n::Language::Fr, tri::i18n::Language::En] {
                assert!(tri::i18n::has(lang, &id), "{id} ({lang:?})");
            }
        }
        assert!(is_folder_key("destination") && is_folder_key("dir") && !is_folder_key("history"));
    }

    #[test]
    fn every_choice_is_accepted_and_names_a_real_setting() {
        let schema = schema();
        for section in &schema.sections {
            for entry in section.groups.iter().flat_map(|g| &g.entries) {
                let Some(list) = choices(&section.name, &entry.key) else {
                    continue;
                };
                assert!(
                    list.iter()
                        .any(|c| entry.default == Value::Str(c.to_string())),
                    "{}.{} : défaut hors de la liste",
                    section.name,
                    entry.key
                );
                for choice in list {
                    let text = format!("[{}]\n{} = \"{choice}\"\n", section.name, entry.key);
                    assert!(tri::settings::parse::<AppSettings>(&text).is_ok(), "{text}");
                }
            }
        }
        // Chaque réglage de la table existe bien.
        for (section, key) in [
            ("gui", "profile"),
            ("gui", "theme"),
            ("gui", "raw_previews"),
            ("xmp", "sidecar"),
            ("i18n", "language"),
            ("gui.view", "focus_marker"),
            ("gui.histogram", "scale"),
        ] {
            assert!(schema.default_of(section, key).is_some(), "{section}.{key}");
        }
    }

    #[test]
    fn sections_follow_the_keyboard_and_the_entries_are_reachable() {
        crate::i18n::ensure();
        let dir = tempfile::tempdir().unwrap();
        let w = SettingsWindow::open(&dir.path().join("config.toml")).unwrap();
        let mut h = egui_kittest::Harness::builder()
            .with_size(vec2(900.0, 420.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, w: &mut SettingsWindow| {
                    w.show(ui.ctx());
                },
                w,
            );
        h.run_steps(3);
        // Jusqu'à la dernière section : la liste défile pour la montrer.
        let last = schema().sections.len() - 1;
        for _ in 0..last {
            h.key_press(Key::ArrowDown);
            h.run_steps(1);
        }
        h.run_steps(2);
        assert_eq!(h.state().section, last);
        assert!(h.state().sections_offset > 0.0, "liste défilée");
        // Retour à la première, puis → : le premier réglage prend le clavier.
        for _ in 0..last {
            h.key_press(Key::ArrowUp);
            h.run_steps(1);
        }
        h.run_steps(2);
        assert_eq!(h.state().sections_offset, 0.0);
        h.key_press(Key::ArrowRight);
        h.run_steps(3);
        let focused = h.ctx.memory(|m| m.focused());
        assert!(
            focused.is_some() && focused != Some(search_id()),
            "{focused:?}"
        );
    }

    #[test]
    fn shortcuts_are_added_removed_and_reset() {
        crate::i18n::ensure();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut w = SettingsWindow::open(&path).unwrap();
        let accept = Action::from_id("core.flag.accept").unwrap();
        let before = w.keymap.specs_for(accept);
        assert!(!before.is_empty());
        // Ajouté : les raccourcis du profil, plus le nouveau, écrits dans [keys].
        let s = w.add_key(accept, "Maj+X".into()).expect("valide");
        assert_eq!(s.keys["core.flag.accept"].last().unwrap(), "Maj+X");
        let spec = keys_spec(&parse_keys("Maj+X").unwrap());
        assert!(w.keymap.specs_for(accept).contains(&spec));
        assert!(w.add_key(accept, spec).is_none(), "déjà là");
        // Maj+X rejetait : signalé.
        assert!(
            w.key_warnings
                .iter()
                .any(|m| m.contains("core.flag.accept")),
            "{:?}",
            w.key_warnings
        );
        // Plus aucun raccourci (liste vide), puis retour au profil.
        let s = w
            .commit("keys", accept.id(), Some(Value::Strings(Vec::new())))
            .unwrap();
        assert!(s.keys["core.flag.accept"].is_empty());
        assert!(w.keymap.specs_for(accept).is_empty());
        let s = w.commit("keys", accept.id(), None).unwrap();
        assert!(s.keys.is_empty());
        assert_eq!(w.keymap.specs_for(accept), before);
    }

    #[test]
    fn a_shortcut_is_recorded_from_the_keys_typed() {
        crate::i18n::ensure();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[gui]\nkey_sequence_timeout_ms = 2000\n").unwrap();
        let mut w = SettingsWindow::open(&path).unwrap();
        let accept = Action::from_id("core.flag.accept").unwrap();
        w.recording = Some((accept, Vec::new(), Instant::now()));
        let mut h = egui_kittest::Harness::builder()
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, w: &mut SettingsWindow| {
                    w.show(ui.ctx());
                },
                w,
            );
        h.run_steps(2);
        // Une séquence : g puis Échap (qui ne ferme pas la fenêtre).
        h.key_press(Key::G);
        h.run_steps(1);
        h.key_press(Key::Escape);
        h.run_steps(1);
        assert!(h.state().is_recording());
        std::thread::sleep(Duration::from_millis(2100));
        h.run_steps(2);
        assert!(!h.state().is_recording());
        let specs = h.state().keymap.specs_for(accept);
        let last = specs.last().unwrap();
        assert_eq!(parse_keys(last).unwrap().len(), 2, "{last}");
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("\"core.flag.accept\"")
        );
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
