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

use eframe::egui::{self, Event, Key, Modifiers, RichText, ScrollArea, Ui, vec2};

use crate::actions::{Action, Chord, Group, Keymap, keys_spec, parse_keys};
use tri::config_edit::{ConfigFile, Entry, Schema, Value, parse_color};

use crate::settings::{AppSettings, Colors};

/// Le fichier d'exemple, embarqué : la liste des réglages, décrits en français.
const EXAMPLE: &str = include_str!("../../doc/config.example.toml");
/// Le même, décrit en anglais (21/10/2026) : mêmes clés et valeurs (test du moteur).
const EXAMPLE_EN: &str = include_str!("../../doc/config.example.en.toml");

/// Liste des réglages, décrits dans la langue de l'interface.
fn schema() -> &'static Schema {
    static FR: OnceLock<Schema> = OnceLock::new();
    static EN: OnceLock<Schema> = OnceLock::new();
    match tri::i18n::language() {
        tri::i18n::Language::Fr => FR.get_or_init(|| Schema::parse(EXAMPLE).unwrap_or_default()),
        tri::i18n::Language::En => EN.get_or_init(|| Schema::parse(EXAMPLE_EN).unwrap_or_default()),
    }
}

/// Les logiciels de développement que `tri` sait utiliser, à la suite (« darktable,
/// RapidRAW, … »).
fn supported_software() -> String {
    tri::develop::Engine::ALL.map(|e| e.name()).join(", ")
}

/// Choix d'un logiciel de développement dans la fenêtre de réglages ; dans le fichier :
/// `"auto"`, un chemin (ou `"flatpak"`), `""`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeveloperMode {
    Auto,
    Manual,
    Off,
}

impl DeveloperMode {
    const ALL: [DeveloperMode; 3] = [
        DeveloperMode::Auto,
        DeveloperMode::Manual,
        DeveloperMode::Off,
    ];

    /// D'après la valeur du fichier ; `manual` : passé en Manuel, chemin pas encore donné.
    fn of(value: &str, manual: bool) -> DeveloperMode {
        match value.trim() {
            "auto" => DeveloperMode::Auto,
            "" if !manual => DeveloperMode::Off,
            _ => DeveloperMode::Manual,
        }
    }

    fn label(self) -> String {
        match self {
            DeveloperMode::Auto => t!("settings-developer-auto"),
            DeveloperMode::Manual => t!("settings-developer-manual"),
            DeveloperMode::Off => t!("settings-developer-off"),
        }
    }
}

/// La commande de `[files] editor` lue dans le fichier (liste TOML de chaînes).
fn editor_list(value: &Value) -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct Wrap {
        v: Vec<String>,
    }
    match value {
        Value::Strings(list) => list.clone(),
        Value::Toml(text) => toml::from_str::<Wrap>(&format!("v = {text}"))
            .map(|w| w.v)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Le programme de l'éditeur existe : un chemin (ou une application macOS), ou un nom
/// trouvé dans le `PATH`.
fn program_exists(program: &str) -> bool {
    let path = tri::settings::expand_home(program);
    if path.components().count() > 1 {
        return path.exists();
    }
    std::env::var_os("PATH").is_some_and(|dirs| {
        std::env::split_paths(&dirs)
            .any(|d| d.join(program).is_file() || d.join(format!("{program}.exe")).is_file())
    })
}

/// Dossier où « Parcourir… » s'ouvre pour l'éditeur : celui du programme choisi, sinon
/// celui des applications du système.
fn program_start(program: &str) -> std::path::PathBuf {
    let given = tri::settings::expand_home(program);
    if let Some(dir) = given
        .parent()
        .filter(|d| d.is_dir() && !d.as_os_str().is_empty())
    {
        return dir.to_path_buf();
    }
    let usual = if cfg!(target_os = "macos") {
        "/Applications"
    } else if cfg!(windows) {
        r"C:\Program Files"
    } else {
        "/usr/bin"
    };
    let usual = std::path::PathBuf::from(usual);
    if usual.is_dir() {
        usual
    } else {
        std::path::PathBuf::from(std::path::MAIN_SEPARATOR_STR)
    }
}

/// Dossier où « Parcourir… » s'ouvre : celui du chemin donné, sinon celui du logiciel
/// trouvé, sinon la racine.
fn browse_start(value: &str, found: Option<&tri::develop::Found>) -> std::path::PathBuf {
    let given = (!value.is_empty() && value != "flatpak" && value != "auto")
        .then(|| tri::settings::expand_home(value));
    given
        .into_iter()
        .chain(found.and_then(|f| f.path.clone()))
        .filter_map(|p| p.parent().map(Path::to_path_buf))
        .find(|dir| dir.is_dir())
        .unwrap_or_else(|| std::path::PathBuf::from(std::path::MAIN_SEPARATOR_STR))
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

/// Sections que l'utilisateur peut multiplier depuis la fenêtre (demandé le 19/10/2026) :
/// (section parente, section d'exemple qui sert de modèle, libellé d'une section ajoutée).
const FAMILIES: [(&str, &str, &str); 2] = [
    (
        "export.profiles",
        "export.profiles.web",
        "settings-section-export-profile-user",
    ),
    (
        "versions.presets",
        "versions.presets.story",
        "settings-section-versions-preset-user",
    ),
];

/// Famille de `section` (`export.profiles.tirage`) : (parente, modèle, libellé) et
/// l'identifiant (`tirage`).
fn family(section: &str) -> Option<((&'static str, &'static str, &'static str), &str)> {
    FAMILIES.iter().find_map(|f| {
        section
            .strip_prefix(f.0)
            .and_then(|rest| rest.strip_prefix('.'))
            .filter(|id| !id.is_empty() && !id.contains('.'))
            .map(|id| (*f, id))
    })
}

/// Réglages sans objet sans aucun logiciel de développement installé (voir
/// `SettingsWindow::hidden`).
const NEED_A_DEVELOPER: [(&str, &str); 5] = [
    ("develop", "default"),
    ("gui.grid", "develop"),
    ("gui.grid", "develop_toast_ms"),
    ("gui.view", "review_developments"),
    ("gui.colors", "development_mark"),
];

/// Réglages de darktable seul.
const NEED_DARKTABLE: [(&str, &str); 1] = [("develop", "high_quality")];

/// Le nom d'un profil ou préréglage : montré dans la liste de l'en-tête et changé par
/// « Renommer… », pas parmi les réglages (issue n° 18).
fn renamed_in_header(section: &str, key: &str) -> bool {
    key == "name" && family(section).is_some()
}

/// Profils et préréglages fournis par `tri` : modifiables, pas supprimables.
fn builtin(section: &str) -> bool {
    match family(section) {
        Some((("export.profiles", _, _), id)) => {
            tri::export::Profile::defaults().iter().any(|p| p.id == id)
        }
        Some((_, id)) => tri::versions::Preset::defaults().iter().any(|p| p.id == id),
        None => false,
    }
}

/// Nom affiché d'un profil ou préréglage : celui du fichier, sinon celui d'un fourni
/// (traduit), sinon son identifiant.
fn member_name(file: &ConfigFile, section: &str) -> String {
    let Some(((parent, _, _), id)) = family(section) else {
        return section.to_string();
    };
    if let Some(Value::Str(name)) = file.get(section, "name")
        && !name.trim().is_empty()
    {
        return name;
    }
    let key = if parent == "export.profiles" {
        format!("export-profile-{id}")
    } else {
        format!("preset-{id}")
    };
    if builtin(section) && tri::i18n::has(tri::i18n::language(), &key) {
        tri::i18n::translate(&key, None)
    } else {
        id.to_string()
    }
}

/// Sections de la liste de gauche : une seule entrée par famille (son premier membre),
/// le profil voulu se choisit à droite (retour du 20/10/2026 : une entrée par profil
/// encombrait la liste).
fn listed(schema: &Schema) -> Vec<usize> {
    let mut seen = Vec::new();
    (0..schema.sections.len())
        .filter(|&i| match family(&schema.sections[i].name) {
            Some((f, _)) if seen.contains(&f.0) => false,
            Some((f, _)) => {
                seen.push(f.0);
                true
            }
            None => true,
        })
        .collect()
}

/// Entrée de la liste de gauche qui montre la section `i` (le premier membre de sa
/// famille).
fn listed_of(schema: &Schema, i: usize) -> usize {
    let parent = schema
        .sections
        .get(i)
        .and_then(|s| family(&s.name))
        .map(|(f, _)| f.0);
    match parent {
        Some(parent) => schema
            .sections
            .iter()
            .position(|s| family(&s.name).is_some_and(|(f, _)| f.0 == parent))
            .unwrap_or(i),
        None => i,
    }
}

/// Réglage masqué faute du logiciel de développement installé qu'il concerne (voir
/// [`SettingsWindow::hidden`]), `engines` : ceux qui sont installés.
fn hidden_for(engines: &[tri::develop::Engine], section: &str, key: &str) -> bool {
    use tri::develop::Engine;
    let missing = |e: Engine| !engines.contains(&e);
    if NEED_A_DEVELOPER.contains(&(section, key)) {
        return engines.is_empty();
    }
    if NEED_DARKTABLE.contains(&(section, key)) {
        return missing(Engine::Darktable);
    }
    section
        .strip_prefix("sidecars.")
        .and_then(Engine::from_id)
        .is_some_and(missing)
}

/// Un réglage, pour la palette de commandes : sa section, sa clé et ce qui le décrit.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingRef {
    /// Section (`gui.zoom`).
    pub section: String,
    /// Clé (`max`).
    pub key: String,
    /// Libellé de la section, traduit (« Zoom »).
    pub section_label: String,
    /// Description (commentaire du fichier d'exemple, dans la langue de l'interface).
    pub doc: String,
}

/// Les réglages de la fenêtre de réglages (fichier d'exemple), sans ceux qu'elle masque
/// (`engines` : logiciels de développement installés), pour la palette de commandes.
pub fn settings_list(engines: &[tri::develop::Engine]) -> Vec<SettingRef> {
    let mut list = Vec::new();
    for section in &schema().sections {
        for group in &section.groups {
            for entry in &group.entries {
                if renamed_in_header(&section.name, &entry.key)
                    || hidden_for(engines, &section.name, &entry.key)
                {
                    continue;
                }
                list.push(SettingRef {
                    section: section.name.clone(),
                    key: entry.key.clone(),
                    section_label: section_label(&section.name),
                    doc: group.doc.clone(),
                });
            }
        }
    }
    list
}

/// Libellé lisible d'une section (« Couleurs » pour `gui.colors`), traduit ; à défaut,
/// son nom (profil ou préréglage ajouté : « Profil d'export « tirage » »).
pub fn section_label(name: &str) -> String {
    let id = format!("settings-section-{}", name.replace('.', "-"));
    if tri::i18n::has(tri::i18n::language(), &id) {
        tri::i18n::translate(&id, None)
    } else if let Some(((_, _, label), id)) = family(name) {
        t!(label, id = id)
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

/// `"#rrggbb"`.
fn hex_color([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Couleur qui suit l'accent ou fixe : case « Couleur d'accent » ; décochée, sélecteur de
/// couleur, parti de l'accent du moment (on ajuste à partir de ce qu'on voyait).
fn accent_color_editor(ui: &mut Ui, value: &Value) -> (egui::Response, Option<Value>) {
    let s = match value {
        Value::Str(s) => s.as_str(),
        _ => "accent",
    };
    let fixed = parse_color(s).filter(|_| !s.trim().eq_ignore_ascii_case("accent"));
    let mut new = None;
    let response = ui
        .horizontal(|ui| {
            let mut on = fixed.is_none();
            let r = ui
                .checkbox(&mut on, t!("settings-accent"))
                .on_hover_text(t!("settings-accent-tip"));
            if r.changed() {
                new = Some(Value::Str(if on {
                    "accent".into()
                } else {
                    let [r, g, b, _] = crate::marks::accent_mark(ui.ctx()).to_array();
                    hex_color([r, g, b])
                }));
            }
            if let Some(mut rgb) = fixed {
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    new = Some(Value::Str(hex_color(rgb)));
                }
                ui.weak(s);
            }
            r
        })
        .inner;
    (response, new)
}

/// Vrai si la clé désigne un dossier : saisie avec complétion par Tab.
/// Valeurs possibles d'un réglage à choix (liste déroulante plutôt que texte libre). Un
/// test vérifie que chacune est acceptée.
fn choices(section: &str, key: &str) -> Option<&'static [&'static str]> {
    // Profils d'export (`[export.profiles.<id>]`) : espaces couleur selon la profondeur du
    // profil fourni (ProPhoto et les linéaires seulement en 16 bits), affinés à
    // l'affichage selon les valeurs du fichier (`fits`).
    // Préréglages de recadrage : rapports proposés, dans les deux orientations.
    if section.starts_with("versions.presets.") && key == "ratio" {
        return Some(&[
            "free", "original", "1:1", "4:5", "5:4", "2:3", "3:2", "9:16", "16:9", "3:4", "4:3",
        ]);
    }
    if let Some(id) = section.strip_prefix("export.profiles.") {
        let deep = tri::export::Profiles::default()
            .get(id)
            .is_some_and(|p| p.bits >= 16);
        return match key {
            "format" => Some(&["jpeg", "tiff", "png"]),
            "color" if deep => Some(&tri::export::COLORS),
            "color" => Some(&tri::export::COLORS_8BIT),
            "intent" => Some(&tri::export::INTENTS),
            "applies_to" => Some(&["all", "raw", "flat"]),
            "sharpen" => Some(&["none", "screen", "print"]),
            _ => None,
        };
    }
    Some(match (section, key) {
        ("gui", "profile") => &["classic", "vim"],
        ("gui", "theme") => &["dark", "light", "system", "neutral"],
        ("gui", "sort") => &["name", "captured", "modified", "stars", "color", "flag"],
        ("gui", "raw_previews") => &["on-demand", "background"],
        ("xmp", "sidecar") => &["darktable", "adobe"],
        ("xmp", "embed") => &["never", "internal", "everywhere"],
        ("sidecars.rapidraw", "create") => &["auto", "always", "never"],
        ("i18n", "language") => &["auto", "fr", "en"],
        ("gui.view", "focus_marker") => &["square", "cross", "circle", "heart"],
        ("gui.view", "stack_enter") => &["cover", "first"],
        ("gui.position", "place") | ("gui.notes", "place") | ("gui.end", "place") => &[
            "center",
            "top-left",
            "top-right",
            "bottom-right",
            "bottom-left",
        ],
        ("cache", "location") => &["user", "disk", "folder"],
        // Tous les logiciels connus ; la liste n'affiche que ceux installés (`fits`).
        ("develop", "default") => &["auto", "darktable", "rapidraw", "rawtherapee", "art"],
        ("gui.grid", "render") => &[
            "embedded",
            "cover",
            "darktable",
            "rapidraw",
            "rawtherapee",
            "art",
            "libraw",
        ],
        ("gui.grid", "develop") => &["auto", "manual"],
        ("gui.histogram", "scale") => &["ev", "linear"],
        _ => return None,
    })
}

fn is_folder_key(key: &str) -> bool {
    key == "dir" || key.ends_with("_dir") || key == "destination" || key == "folder"
}

/// Identifiant du champ de recherche.
pub fn search_id() -> egui::Id {
    egui::Id::unique("settings_search")
}

/// Fenêtre ouverte.
pub struct SettingsWindow {
    file: ConfigFile,
    /// Logiciels de développement installés, pour la liste de `[develop] default`.
    engines: Vec<tri::develop::Engine>,
    /// Logiciels de développement trouvés au lancement, et où.
    found: Vec<tri::develop::Found>,
    /// Couleurs du thème, pour les pastilles de `[sort] color_order`.
    colors: Colors,
    /// Section affichée (sans recherche).
    section: usize,
    query: String,
    /// Première image : la recherche prend le focus.
    fresh: bool,
    /// Textes en cours de saisie (texte, liste TOML), écrits à la validation (Entrée ou
    /// focus perdu), par (section, clé).
    drafts: HashMap<(String, String), String>,
    /// Logiciels passés en Manuel sans chemin encore choisi (le fichier garde `""`) ;
    /// `"editor"` : éditeur externe passé en « Autre application », pas encore choisi.
    manual: std::collections::HashSet<String>,
    /// Application du système par type d'image du dossier ouvert (« RAF », « darktable »),
    /// pour information sous l'éditeur externe.
    default_apps: Vec<(String, String)>,
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
    /// Nom d'un profil ou préréglage en cours de saisie (« Nouveau… » ou « Renommer… »).
    naming: Option<Naming>,
    naming_focus: bool,
}

/// Nom en cours de saisie dans l'en-tête d'une famille.
enum Naming {
    /// Nouveau membre, copie de celui montré.
    New(String),
    /// Nouveau nom de celui montré (issue n° 18 : plus de champ « name » modifiable parmi
    /// les réglages, validé d'un Entrée sans contrôle).
    Rename(String),
}

/// Ce qu'une recherche trouve dans une section.
#[derive(Default)]
struct Found {
    /// Réglages trouvés (clés).
    keys: Vec<String>,
    /// Actions trouvées (section `[keys]`).
    actions: Vec<Action>,
    /// Meilleure correspondance de la section.
    best: Option<tri::fuzzy::Match>,
}

/// Un résultat de recherche : un réglage ou une action.
enum Hit {
    Key(String),
    Action(Action),
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
    /// Logiciels cherchés à nouveau (réglage `[develop]` changé) : listes et réglages
    /// montrés ou masqués à jour.
    pub fn set_developers(
        &mut self,
        engines: Vec<tri::develop::Engine>,
        found: Vec<tri::develop::Found>,
    ) {
        self.engines = engines;
        self.found = found;
    }

    /// Logiciels trouvés au lancement ([`tri::develop::Developers::found`]) : où chacun l'a
    /// été, sous son réglage de `[develop]` en mode Auto.
    pub fn with_found(mut self, found: Vec<tri::develop::Found>) -> SettingsWindow {
        self.found = found;
        self
    }

    /// Application du système par type d'image du dossier ouvert, montrée sous l'éditeur
    /// externe en mode « Application du système ».
    pub fn with_default_apps(mut self, apps: Vec<(String, String)>) -> SettingsWindow {
        self.default_apps = apps;
        self
    }

    /// Logiciels de développement installés (ceux que propose `[develop] default`).
    pub fn with_engines(mut self, engines: Vec<tri::develop::Engine>) -> SettingsWindow {
        self.engines = engines;
        self
    }

    /// Couleurs du thème (les pastilles de `[sort] color_order`).
    pub fn with_colors(mut self, colors: Colors) -> SettingsWindow {
        self.colors = colors;
        self
    }

    /// Ouvre la fenêtre sur le fichier `path` (créé au premier changement s'il n'existe
    /// pas).
    pub fn open(path: &Path) -> Result<SettingsWindow, String> {
        let file = ConfigFile::open(path)?;
        let settings = tri::settings::parse::<AppSettings>(&file.text()).unwrap_or_default();
        let (keymap, key_warnings) = settings.keymap();
        Ok(SettingsWindow {
            engines: Vec::new(),
            found: Vec::new(),
            colors: Colors::default(),
            keymap,
            key_warnings,
            recording: None,
            section_moved: false,
            focus_first: false,
            #[cfg(test)]
            sections_offset: 0.0,
            naming: None,
            naming_focus: false,
            sequence_timeout: settings.gui.key_sequence_timeout(),
            file,
            section: 0,
            query: String::new(),
            fresh: true,
            drafts: HashMap::new(),
            manual: Default::default(),
            default_apps: Vec::new(),
            paths: HashMap::new(),
            errors: HashMap::new(),
            dirty: false,
            save_error: None,
        })
    }

    /// Recherche `text` (script de capture : `search:vim`).
    pub fn search(&mut self, text: &str) {
        self.query = text.to_string();
    }

    /// Affiche la section `name` (`keys`, `gui.zoom`), si elle existe.
    pub fn show_section(&mut self, name: &str) {
        if let Some(i) = self.schema().sections.iter().position(|s| s.name == name) {
            self.section = i;
            self.section_moved = true;
        }
    }

    /// Les sections montrées : celles de l'exemple, et les profils d'export et
    /// préréglages ajoutés par l'utilisateur (sur le modèle d'un profil fourni), à la suite
    /// de ceux de leur famille.
    fn schema(&self) -> Schema {
        let mut schema = schema().clone();
        for (parent, model, _) in FAMILIES {
            let Some(template) = schema.sections.iter().find(|s| s.name == model).cloned() else {
                continue;
            };
            let prefix = format!("{parent}.");
            let mut at = schema
                .sections
                .iter()
                .rposition(|s| s.name.starts_with(&prefix))
                .map_or(schema.sections.len(), |i| i + 1);
            for id in self.file.subtables(parent) {
                let name = format!("{parent}.{id}");
                if schema.sections.iter().any(|s| s.name == name) {
                    continue;
                }
                let mut section = template.clone();
                section.name = name;
                section.doc = String::new();
                schema.sections.insert(at, section);
                at += 1;
            }
        }
        schema
    }

    /// Nouveau profil ou préréglage nommé `name`, copie de `section` (toutes ses valeurs
    /// écrites : il ne dépend plus des valeurs par défaut d'un autre), puis montré.
    fn create_from(&mut self, section: &str, name: &str) -> Option<AppSettings> {
        let ((parent, _, _), id) = family(section)?;
        let settings = tri::settings::parse::<AppSettings>(&self.file.text()).ok()?;
        let (ids, entries): (Vec<String>, Vec<_>) = if parent == "export.profiles" {
            let all = &settings.export.profiles.0;
            (
                all.iter().map(|p| p.id.clone()).collect(),
                all.iter().find(|p| p.id == id)?.entries(),
            )
        } else {
            let all = &settings.versions.presets.0;
            (
                all.iter().map(|p| p.id.clone()).collect(),
                all.iter().find(|p| p.id == id)?.entries(),
            )
        };
        let new = tri::versions::new_id(name, |id| ids.iter().any(|i| i == id));
        let target = format!("{parent}.{new}");
        for (key, value) in entries {
            let value = if key == "name" {
                Value::Str(name.trim().to_string())
            } else {
                value
            };
            if let Err(e) = self.file.set(&target, key, &value) {
                self.errors.insert((target.clone(), key.to_string()), e);
                return None;
            }
        }
        let settings = tri::settings::parse::<AppSettings>(&self.file.text()).ok()?;
        self.dirty = true;
        self.naming = None;
        self.show_section(&target);
        Some(settings)
    }

    /// Renomme un profil ou préréglage (son nom affiché ; l'identifiant, qui est la clé du
    /// fichier, ne change pas).
    fn rename_section(&mut self, section: &str, name: &str) -> Option<AppSettings> {
        family(section)?;
        if let Err(e) = self
            .file
            .set(section, "name", &Value::Str(name.trim().to_string()))
        {
            self.errors.insert((section.to_string(), "name".into()), e);
            return None;
        }
        let settings = tri::settings::parse::<AppSettings>(&self.file.text()).ok()?;
        self.dirty = true;
        Some(settings)
    }

    /// Supprime un profil ou préréglage ajouté par l'utilisateur.
    fn delete_section(&mut self, section: &str) -> Option<AppSettings> {
        if builtin(section) || family(section).is_none() {
            return None;
        }
        self.file.remove_table(section);
        let settings = tri::settings::parse::<AppSettings>(&self.file.text()).ok()?;
        self.dirty = true;
        self.section = self.section.saturating_sub(1);
        self.section_moved = true;
        Some(settings)
    }

    /// Vrai pendant l'enregistrement d'un raccourci (les touches lui reviennent).
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    /// Réglage sans objet tant qu'aucun logiciel de développement (darktable, RapidRAW,
    /// RawTherapee, ART) qui le concerne n'est installé (retour du 04/11/2026 : la plupart
    /// des utilisateurs n'en ont aucun) : masqué dans la fenêtre, gardé dans le fichier.
    /// Les chemins des logiciels (`[develop] darktable`…) restent : c'est là qu'on en
    /// indique un installé ailleurs.
    fn hidden(&self, section: &str, key: &str) -> bool {
        hidden_for(&self.engines, section, key)
    }

    /// Section dont tous les réglages sont masqués ([`SettingsWindow::hidden`]) : absente de
    /// la liste.
    fn section_hidden(&self, section: &tri::config_edit::Section) -> bool {
        let mut entries = section.groups.iter().flat_map(|g| &g.entries).peekable();
        entries.peek().is_some() && entries.all(|e| self.hidden(&section.name, &e.key))
    }

    /// Sections de la liste de gauche ([`listed`]), sans celles dont tout est masqué.
    fn shown_sections(&self, schema: &Schema) -> Vec<usize> {
        let mut list = listed(schema);
        list.retain(|&i| !self.section_hidden(&schema.sections[i]));
        list
    }

    /// Valeur actuelle : celle du fichier, sinon la valeur par défaut.
    /// Choix cohérent avec les autres valeurs de la section ? Profils d'export : pas
    /// d'espace « 16 bits » (ProPhoto…) en JPEG ou en 8 bits, pas de JPEG avec un tel
    /// espace (idée de l'utilisateur, 19/10/2026). Le moteur refuse de toute façon.
    fn fits(&self, section: &str, key: &str, choice: &str) -> bool {
        // Logiciel de développement préféré : « auto » et ceux installés, plus la valeur
        // en place (un logiciel désinstallé depuis reste visible).
        if (section, key) == ("develop", "default") || (section, key) == ("gui.grid", "render") {
            let current = matches!(self.file.get(section, key), Some(Value::Str(v)) if v == choice);
            let engine = tri::develop::Engine::from_id(choice);
            return engine.is_none_or(|e| self.engines.contains(&e)) || current;
        }
        let Some(id) = section.strip_prefix("export.profiles.") else {
            return true;
        };
        let base = tri::export::Profiles::default().get(id).cloned();
        let text = |k: &str, default: String| match self.file.get(section, k) {
            Some(Value::Str(v)) => v,
            _ => default,
        };
        let format = text(
            "format",
            base.as_ref()
                .map_or("jpeg", |p| p.format.extension())
                .into(),
        );
        let bits = match self.file.get(section, "bits") {
            Some(Value::Int(b)) => b as u8,
            _ if format == "jpeg" || format == "jpg" => 8,
            _ => base.as_ref().map_or(8, |p| p.bits),
        };
        let color = text("color", base.map_or_else(String::new, |p| p.color));
        match key {
            "color" => tri::export::color_fits(choice, bits),
            "format" => choice != "jpeg" || tri::export::color_fits(&color, 8),
            _ => true,
        }
    }

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
        let checked =
            written.and_then(|()| tri::settings::parse_message::<AppSettings>(&self.file.text()));
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
        let schema = self.schema();
        let schema = &schema;
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
            let listed = self.shown_sections(schema);
            let at = listed_of(schema, self.section);
            let pos = listed.iter().position(|&i| i == at).unwrap_or(0);
            if up {
                self.section = listed[pos.saturating_sub(1)];
            }
            if down {
                self.section = listed[(pos + 1).min(listed.len() - 1)];
            }
            if up || down {
                self.naming = None;
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
                            self.section_list(ui, schema, size.y, None);
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
                        // Les sections restent à gauche (22/10/2026), avec le nombre de
                        // résultats de chacune.
                        let hits = self.hits(schema);
                        ui.horizontal_top(|ui| {
                            self.section_list(ui, schema, size.y, Some(&hits));
                            ui.separator();
                            ScrollArea::vertical()
                                .id_salt("settings_found")
                                .max_height(size.y)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.vertical(|ui| changed = self.found(ui, schema, &hits))
                                });
                        });
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.weak(t!(
                        "settings-file",
                        path = self.file.path.display().to_string()
                    ));
                    ui.weak(t!("settings-bold"));
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
        if modal.should_close() {
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

    /// Liste des sections, à gauche ; pendant une recherche (`hits`), le nombre de
    /// résultats de chacune, les autres grisées ; un clic quitte la recherche et ouvre la
    /// section.
    fn section_list(
        &mut self,
        ui: &mut Ui,
        schema: &Schema,
        height: f32,
        hits: Option<&[(usize, Found)]>,
    ) {
        let mut open = None;
        let _output = ScrollArea::vertical()
            .id_salt("settings_sections")
            .max_height(height)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    let at = listed_of(schema, self.section);
                    let mut order = self.shown_sections(schema);
                    // Recherche : les sections trouvées en tête, dans l'ordre des résultats,
                    // puis les autres, grisées, après un espace (un trait prendrait toute la
                    // largeur, la liste avec).
                    let mut first_other = None;
                    if let Some(hits) = hits {
                        let mut found: Vec<usize> = Vec::new();
                        for (s, _) in hits {
                            let l = listed_of(schema, *s);
                            if !found.contains(&l) {
                                found.push(l);
                            }
                        }
                        order.retain(|i| !found.contains(i));
                        first_other = order.first().copied().filter(|_| !found.is_empty());
                        found.append(&mut order);
                        order = found;
                    }
                    for i in order {
                        if first_other == Some(i) {
                            ui.add_space(14.0);
                        }
                        let name = &schema.sections[i].name;
                        // Une famille : son nom (« Profils d'export [export.profiles] »).
                        let name = family(name).map_or(name.as_str(), |(f, _)| f.0);
                        if let Some(hits) = hits {
                            let count: usize = hits
                                .iter()
                                .filter(|(s, _)| listed_of(schema, *s) == i)
                                .map(|(_, f)| f.keys.len() + f.actions.len())
                                .sum();
                            let mut job = section_title(ui, name, false);
                            if count > 0 {
                                RichText::new(format!("  {count}"))
                                    .color(crate::marks::accent_bright(ui.visuals()))
                                    .strong()
                                    .append_to(
                                        &mut job,
                                        ui.style(),
                                        egui::FontSelection::Default,
                                        egui::Align::Center,
                                    );
                            }
                            let label =
                                ui.add_enabled(count > 0, egui::Button::selectable(false, job));
                            // Un clic ouvre la section comme sans recherche (issue n° 24 :
                            // il amenait seulement ses résultats en vue, et une famille
                            // montrait alors tous ses membres en entier, l'un après l'autre).
                            // Pour une famille, le membre le mieux trouvé.
                            if label.clicked() {
                                open = hits
                                    .iter()
                                    .map(|(s, _)| *s)
                                    .find(|&s| listed_of(schema, s) == i)
                                    .or(Some(i));
                            }
                            continue;
                        }
                        let mut job = section_title(ui, name, false);
                        if at == i {
                            // Texte de la sélection sur le fond d'accent : les gris du thème
                            // disparaissaient sur un accent clair (accent réglable, 05/11/2026).
                            let on = ui.visuals().selection.stroke.color;
                            for (n, part) in job.sections.iter_mut().enumerate() {
                                part.format.color =
                                    if n == 0 { on } else { on.gamma_multiply(0.7) };
                            }
                        }
                        let label = ui.selectable_label(at == i, job);
                        if at == i && self.section_moved {
                            label.scroll_to_me(None);
                        }
                        if label.clicked() && at != i {
                            self.section = i;
                            self.section_moved = true;
                            self.naming = None;
                        }
                    }
                });
            });
        #[cfg(test)]
        {
            self.sections_offset = _output.state.clamped_offset().y;
        }
        if let Some(i) = open {
            self.query.clear();
            self.section = i;
            self.section_moved = true;
            self.naming = None;
        }
    }

    /// En-tête d'un profil d'export ou d'un préréglage : la famille (titre et explication),
    /// la liste de ses membres, « Nouveau… » (copie de celui montré) et « Supprimer » (ceux
    /// de l'utilisateur seulement).
    fn family_header(&mut self, ui: &mut Ui, section: &str) -> Option<AppSettings> {
        let ((parent, _, _), _) = family(section)?;
        ui.label(section_title(ui, parent, true));
        // L'explication de la famille est sur son premier membre dans l'exemple.
        if let Some(doc) = schema()
            .sections
            .iter()
            .find(|s| family(&s.name).is_some_and(|(f, _)| f.0 == parent))
            .map(|s| &s.doc)
            .filter(|d| !d.is_empty())
        {
            ui.weak(doc);
        }
        ui.add_space(6.0);
        let schema = self.schema();
        let members: Vec<(usize, String)> = schema
            .sections
            .iter()
            .enumerate()
            .filter(|(_, s)| family(&s.name).is_some_and(|(f, _)| f.0 == parent))
            .map(|(i, s)| (i, s.name.clone()))
            .collect();
        let mut pick = None;
        let mut delete = false;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(("settings_member", parent))
                .selected_text(member_name(&self.file, section))
                .show_ui(ui, |ui| {
                    for (i, name) in &members {
                        let label = member_name(&self.file, name);
                        if ui.selectable_label(name == section, label).clicked() {
                            pick = Some(*i);
                        }
                    }
                });
            if ui.button(t!("settings-member-new")).clicked() {
                self.naming = Some(Naming::New(String::new()));
                self.naming_focus = true;
            }
            if ui.button(t!("settings-member-rename")).clicked() {
                self.naming = Some(Naming::Rename(member_name(&self.file, section)));
                self.naming_focus = true;
            }
            let own = !builtin(section);
            delete = ui
                .add_enabled(own, egui::Button::new(t!("settings-member-delete")))
                .on_disabled_hover_text(t!("settings-member-builtin"))
                .clicked();
        });
        if let Some(i) = pick {
            self.section = i;
            self.section_moved = true;
            self.naming = None;
        }
        // « Nouveau… » (copie de celui montré) ou « Renommer… » : un nom, ni vide ni déjà
        // pris dans la famille (issue n° 17), casse et espaces mis à part.
        let mut confirm = false;
        if let Some(mut naming) = self.naming.take() {
            let renaming = matches!(naming, Naming::Rename(_));
            let (Naming::New(name) | Naming::Rename(name)) = &mut naming;
            let taken = members
                .iter()
                .filter(|(_, m)| !(renaming && m == section))
                .any(|(_, m)| tri::versions::same_name(&member_name(&self.file, m), name));
            let ok = !name.trim().is_empty() && !taken;
            let mut keep = true;
            ui.horizontal(|ui| {
                ui.label(t!("settings-member-name"));
                let mut edit = egui::TextEdit::singleline(name).desired_width(160.0);
                if taken {
                    edit = edit.text_color(ui.visuals().error_fg_color);
                }
                let field = ui.add(edit);
                // Le champ vient d'apparaître : il prend le clavier.
                if std::mem::take(&mut self.naming_focus) {
                    field.request_focus();
                }
                let (label, tip) = if renaming {
                    (
                        t!("settings-member-rename-ok"),
                        t!("settings-member-rename-tip"),
                    )
                } else {
                    (
                        t!("settings-member-create"),
                        t!("settings-member-create-tip"),
                    )
                };
                confirm = ui
                    .add_enabled(ok, egui::Button::new(label))
                    .on_hover_text(tip)
                    .clicked()
                    || (ok && field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
                keep = !ui.button(t!("cancel")).clicked();
                if taken {
                    ui.colored_label(ui.visuals().error_fg_color, t!("settings-member-taken"));
                }
            });
            if keep {
                self.naming = Some(naming);
            }
        }
        ui.add_space(6.0);
        if confirm {
            match self.naming.take() {
                Some(Naming::New(name)) => {
                    if let Some(s) = self.create_from(section, &name) {
                        return Some(s);
                    }
                }
                Some(Naming::Rename(name)) => return self.rename_section(section, &name),
                None => {}
            }
        }
        if delete {
            return self.delete_section(section);
        }
        None
    }

    /// Les réglages d'une section ; `only` : les seules clés à montrer (recherche).
    fn section_entries(
        &mut self,
        ui: &mut Ui,
        section: &tri::config_edit::Section,
        only: Option<&[&str]>,
    ) -> Option<AppSettings> {
        let mut changed = None;
        // Profil d'export ou préréglage : en-tête de sa famille.
        if only.is_none() && family(&section.name).is_some() {
            let header = self.family_header(ui, &section.name);
            if header.is_some() {
                return header;
            }
        }
        if only.is_none() && family(&section.name).is_none() {
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
            // Aucun logiciel trouvé : dit, avec ce que `tri` sait utiliser.
            if section.name == "develop" && self.found.is_empty() {
                ui.label(t!("settings-found-none", list = supported_software()));
            }
        }
        // Toujours en tableau, aussi dans les résultats d'une recherche (08/11/2026 : la
        // recherche « xmp » montrait une case par ligne).
        if section.name == "xmp.fields" {
            return self.fields_table(ui, section);
        }
        for group in &section.groups {
            let entries: Vec<&Entry> = group
                .entries
                .iter()
                .filter(|e| only.is_none_or(|keys| keys.contains(&e.key.as_str())))
                .filter(|e| !renamed_in_header(&section.name, &e.key))
                .filter(|e| !self.hidden(&section.name, &e.key))
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

    /// Ce que trouve la recherche : par section (indice dans le schéma), les réglages ou
    /// les actions trouvés, sections rangées de la mieux trouvée à la moins bien. Champs
    /// cherchés : libellé et nom de la section (trouver une section la montre en entier),
    /// clé, description, mots-clés cachés (`actions::keywords`) ; les exactes seulement
    /// s'il y en a (22/10/2026).
    fn hits(&self, schema: &Schema) -> Vec<(usize, Found)> {
        let query = self.query.trim();
        let mut found: Vec<((usize, Hit), tri::fuzzy::Match)> = Vec::new();
        for (i, section) in schema.sections.iter().enumerate() {
            let label = section_label(&section.name);
            let section_kw = crate::actions::keywords(&section.name);
            if section.name == "keys" {
                for action in self.matching_actions(None) {
                    let keys = self.keymap.keys_for(action).join(" ");
                    let fields = [
                        label.as_str(),
                        "keys",
                        &section_kw,
                        &action.label(),
                        action.id(),
                        &keys,
                        &action.group().label(),
                        &crate::actions::keywords(action.id()),
                    ];
                    if let Some(m) = tri::fuzzy::find(query, &fields) {
                        found.push(((i, Hit::Action(action)), m));
                    }
                }
                continue;
            }
            for group in &section.groups {
                for entry in &group.entries {
                    if renamed_in_header(&section.name, &entry.key)
                        || self.hidden(&section.name, &entry.key)
                    {
                        continue;
                    }
                    let id = format!("{}.{}", section.name, entry.key);
                    let fields = [
                        label.as_str(),
                        &id,
                        &section_kw,
                        &group.doc,
                        &crate::actions::keywords(&id),
                    ];
                    if let Some(m) = tri::fuzzy::find(query, &fields) {
                        found.push(((i, Hit::Key(entry.key.clone())), m));
                    }
                }
            }
        }
        let found = tri::fuzzy::relevant(found);
        let mut sections: Vec<(usize, Found)> = Vec::new();
        for ((i, hit), m) in found {
            let at = match sections.iter().position(|(s, _)| *s == i) {
                Some(at) => at,
                None => {
                    sections.push((i, Found::default()));
                    sections.len() - 1
                }
            };
            let entry = &mut sections[at].1;
            entry.best = entry.best.max(Some(m));
            match hit {
                Hit::Key(k) => entry.keys.push(k),
                Hit::Action(a) => entry.actions.push(a),
            }
        }
        // Stable : à égalité, l'ordre du fichier.
        sections.sort_by_key(|(_, f)| std::cmp::Reverse(f.best));
        sections
    }

    /// Résultats d'une recherche : par section, les réglages trouvés, sous le titre de
    /// leur section.
    fn found(
        &mut self,
        ui: &mut Ui,
        schema: &Schema,
        hits: &[(usize, Found)],
    ) -> Option<AppSettings> {
        let mut changed = None;
        for (i, found) in hits {
            let section = &schema.sections[*i];
            ui.label(section_title(ui, &section.name, true));
            let result = if section.name == "keys" {
                self.keys_entries(ui, Some(&found.actions))
            } else {
                let keys: Vec<&str> = found.keys.iter().map(String::as_str).collect();
                self.section_entries(ui, section, Some(&keys))
            };
            if result.is_some() {
                changed = result;
            }
            ui.add_space(8.0);
        }
        if hits.is_empty() {
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
        ctx.request_repaint_after(crate::app::poll_interval(ctx));
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

    /// Actions du volet des raccourcis, dans l'ordre de l'aide ; `only` : celles-là
    /// seulement (recherche).
    fn matching_actions<'a>(
        &'a self,
        only: Option<&'a [Action]>,
    ) -> impl Iterator<Item = Action> + 'a {
        Group::ALL.into_iter().flat_map(move |group| {
            Action::ALL.into_iter().filter(move |a| {
                a.group() == group
                    && a.available()
                    && a.bindable()
                    && only.is_none_or(|o| o.contains(a))
            })
        })
    }

    /// Volet des raccourcis : problèmes en tête, puis une ligne par action, par thème ;
    /// `only` : les actions trouvées par une recherche.
    fn keys_entries(&mut self, ui: &mut Ui, only: Option<&[Action]>) -> Option<AppSettings> {
        if only.is_none() {
            ui.weak(t!("settings-keys-doc"));
            ui.add_space(4.0);
            for warning in &self.key_warnings {
                ui.colored_label(ui.visuals().error_fg_color, warning);
            }
        }
        let actions: Vec<Action> = self.matching_actions(only).collect();
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
                let chip =
                    egui::Button::new(format!("{}  ×", self.keymap.spec_label(spec))).small();
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
            ui.colored_label(ui.visuals().error_fg_color, e);
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
                    // En gras : présent dans le fichier (issue n° 6 : on le prenait pour
                    // une erreur d'affichage), dit au survol.
                    let hover = format!("{section}.{}", entry.key);
                    let hover = if in_file {
                        format!("{hover}\n{}", t!("settings-in-file"))
                    } else {
                        hover
                    };
                    ui.add(egui::Label::new(name).truncate())
                        .on_hover_text(hover);
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
                    // Appliquée à Entrée ou en quittant le champ, pas à chaque chiffre
                    // tapé (07/11/2026 : « 800 » passait par « 8 », refusé sous 100 %, et
                    // la valeur ne changeait jamais). Le glisser reste en direct.
                    let r = ui.add(egui::DragValue::new(&mut v).update_while_editing(false));
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
                        .max_decimals(6)
                        .update_while_editing(false);
                    let r = ui.add(drag);
                    if r.changed() {
                        new = Some(Value::Float(v));
                    }
                    widget = Some(r);
                }
                // Préréglage d'export d'un préréglage de recadrage : la liste des
                // préréglages d'export (retour du 21/10/2026 : un champ texte laissait
                // passer une faute de frappe), « aucun » en tête.
                Value::Str(s)
                    if section.starts_with("versions.presets.") && entry.key == "profile" =>
                {
                    let mut v = s.clone();
                    let profiles: Vec<(String, String)> = self
                        .schema()
                        .sections
                        .iter()
                        .filter_map(|sec| match family(&sec.name) {
                            Some((("export.profiles", _, _), id)) => {
                                Some((id.to_string(), member_name(&self.file, &sec.name)))
                            }
                            _ => None,
                        })
                        .collect();
                    let none = t!("settings-profile-none");
                    let shown = if v.is_empty() {
                        none.clone()
                    } else {
                        profiles
                            .iter()
                            .find(|(id, _)| *id == v)
                            .map_or_else(|| v.clone(), |(_, name)| name.clone())
                    };
                    let combo = egui::ComboBox::from_id_salt(("settings_profile", &id))
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut v, String::new(), none);
                            for (pid, name) in &profiles {
                                ui.selectable_value(&mut v, pid.clone(), name)
                                    .on_hover_text(pid);
                            }
                        });
                    widget = Some(combo.response);
                    if v != *s {
                        new = Some(Value::Str(v));
                    }
                }
                // Éditeur externe (04/11/2026, idée de l'utilisateur : une liste TOML à
                // taper, trop technique) : application du système, ou une autre choisie.
                value if section == "files" && entry.key == "editor" => {
                    let (r, changed) = self.editor_editor(ui, value);
                    widget = Some(r);
                    new = changed;
                }
                // Logiciel de développement (04/11/2026, idée de l'utilisateur) : Auto
                // (et où il a été trouvé), Manuel (un chemin, Parcourir…) ou Désactivé.
                Value::Str(s)
                    if section == "develop"
                        && tri::develop::Engine::from_id(&entry.key).is_some() =>
                {
                    let (r, changed) = self.developer_editor(ui, &id, s);
                    widget = Some(r);
                    new = changed;
                }
                Value::Str(s) if choices(section, &entry.key).is_some() => {
                    let mut v = s.clone();
                    let combo = egui::ComboBox::from_id_salt(("settings_choice", &id))
                        .selected_text(&v)
                        .show_ui(ui, |ui| {
                            for choice in choices(section, &entry.key).unwrap_or_default() {
                                if self.fits(section, &entry.key, choice) {
                                    ui.selectable_value(&mut v, choice.to_string(), *choice);
                                }
                            }
                        });
                    widget = Some(combo.response);
                    if v != *s {
                        new = Some(Value::Str(v));
                    }
                }
                // Ordre des couleurs du tri (idée de l'utilisateur, 31/10/2026) : des
                // pastilles à glisser (ou cliquer) plutôt qu'une liste à taper.
                value if section == "sort" && entry.key == "color_order" => {
                    let (r, changed) = self.color_order_editor(ui, value);
                    widget = Some(r);
                    new = changed;
                }
                // Ordre de lecture des couleurs (08/11/2026) : les champs dans l'ordre, en
                // colonne, un bouton entre deux voisins pour les échanger.
                value if section == "xmp" && entry.key == "color_sources" => {
                    let managed = self.managed_color_sources();
                    let (r, changed) = color_sources_editor(ui, value, &managed);
                    widget = Some(r);
                    new = changed;
                }
                // Couleur qui peut suivre l'accent (`"accent"` par défaut : liseré des
                // boîtes, étoiles) : une case plutôt que le mot à taper (retour du
                // 05/11/2026), décochée = sélecteur de couleur comme les autres.
                value if matches!(&entry.default, Value::Str(d) if d == "accent") => {
                    let (r, changed) = accent_color_editor(ui, value);
                    widget = Some(r);
                    new = changed;
                }
                Value::Str(s) if parse_color(s).is_some() => {
                    let mut rgb = parse_color(s).unwrap_or_default();
                    let r = ui.color_edit_button_srgb(&mut rgb);
                    if r.changed() {
                        new = Some(Value::Str(hex_color(rgb)));
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
                    let browsed = self.paths.get_mut(&id).is_some_and(|p| p.take_browsed());
                    let done = (field.lost_focus() || browsed) && *draft != shown;
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
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        if reset {
            self.drafts.remove(&id);
            return self.commit(section, &entry.key, None);
        }
        let value = new?;
        self.drafts.remove(&id);
        self.commit(section, &entry.key, Some(value))
    }

    /// `[files] editor` (04/11/2026, idée de l'utilisateur) : « Application du système »
    /// (liste vide ; dessous, celle que le système choisit pour les types d'images du
    /// dossier) ou « Autre application » : un programme (Parcourir…) et ses arguments,
    /// `{files}` par défaut. Rend la réponse de la liste (focus) et la nouvelle valeur.
    fn editor_editor(&mut self, ui: &mut Ui, value: &Value) -> (egui::Response, Option<Value>) {
        let list = editor_list(value);
        let key = "editor".to_string();
        let system = list.is_empty() && !self.manual.contains(&key);
        let mut other = !system;
        let mut new: Option<Vec<String>> = None;
        let program_id = ("files".to_string(), "editor/program".to_string());
        let args_id = ("files".to_string(), "editor/args".to_string());
        let response = ui
            .vertical(|ui| {
                let label = |other: bool| {
                    if other {
                        t!("settings-editor-other")
                    } else {
                        t!("settings-editor-system")
                    }
                };
                let combo = egui::ComboBox::from_id_salt("settings_editor")
                    .selected_text(label(other))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut other, false, label(false));
                        ui.selectable_value(&mut other, true, label(true));
                    });
                if other == system {
                    self.drafts.remove(&program_id);
                    self.drafts.remove(&args_id);
                    if other {
                        self.manual.insert(key.clone());
                    } else {
                        self.manual.remove(&key);
                        new = Some(Vec::new());
                    }
                }
                if !other {
                    let text = if self.default_apps.is_empty() {
                        t!("settings-editor-system-tip")
                    } else {
                        let apps: Vec<String> = self
                            .default_apps
                            .iter()
                            .map(|(kind, app)| format!("{kind} : {app}"))
                            .collect();
                        t!("settings-editor-system-apps", apps = apps.join(" · "))
                    };
                    ui.weak(text);
                    return combo.response;
                }
                let (program, args) = match list.split_first() {
                    Some((p, a)) => (p.clone(), a.to_vec()),
                    None => (String::new(), Vec::new()),
                };
                let args = if args.is_empty() {
                    vec!["{files}".to_string()]
                } else {
                    args
                };
                let shown_args = tri::files::join_args(&args);
                ui.horizontal(|ui| {
                    ui.label(t!("settings-editor-program"));
                    let draft = self
                        .drafts
                        .entry(program_id.clone())
                        .or_insert(program.clone());
                    let field = ui.add(egui::TextEdit::singleline(draft).desired_width(300.0));
                    if field.lost_focus() && *draft != program && !draft.trim().is_empty() {
                        new = Some(
                            std::iter::once(draft.trim().to_string())
                                .chain(args.iter().cloned())
                                .collect(),
                        );
                    }
                    if !field.has_focus() && new.is_none() {
                        *draft = program.clone();
                    }
                    if ui.button(t!("settings-developer-browse")).clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .set_directory(program_start(&program))
                            .pick_file()
                    {
                        new = Some(
                            std::iter::once(path.to_string_lossy().into_owned())
                                .chain(args.iter().cloned())
                                .collect(),
                        );
                    }
                });
                if !program.is_empty() && !program_exists(&program) {
                    ui.colored_label(ui.visuals().error_fg_color, t!("settings-editor-missing"));
                }
                ui.horizontal(|ui| {
                    ui.label(t!("settings-editor-args"));
                    let draft = self
                        .drafts
                        .entry(args_id.clone())
                        .or_insert(shown_args.clone());
                    let field = ui
                        .add(egui::TextEdit::singleline(draft).desired_width(300.0))
                        .on_hover_text(t!("settings-editor-args-tip"));
                    if field.lost_focus() && *draft != shown_args && !program.is_empty() {
                        new = Some(
                            std::iter::once(program.clone())
                                .chain(tri::files::split_args(draft))
                                .collect(),
                        );
                    }
                    if !field.has_focus() && new.is_none() {
                        *draft = shown_args.clone();
                    }
                });
                ui.weak(t!("settings-editor-args-tip"));
                combo.response
            })
            .inner;
        if new.as_ref().is_some_and(|l| !l.is_empty()) {
            self.manual.remove(&key);
        }
        if new.is_some() {
            self.drafts.remove(&program_id);
            self.drafts.remove(&args_id);
        }
        (response, new.map(Value::Strings))
    }

    /// Réglage d'un logiciel de développement (`[develop] darktable`…, 04/11/2026, idée
    /// de l'utilisateur) : une liste Auto / Manuel / Désactivé, et dessous, selon le choix,
    /// l'endroit où il a été trouvé, un chemin à taper ou choisir
    /// (« Parcourir… », ouvert au dossier du logiciel trouvé, sinon à la racine), ou « …
    /// désactivé ». Rend la réponse de la liste (focus) et la nouvelle valeur.
    fn developer_editor(
        &mut self,
        ui: &mut Ui,
        id: &(String, String),
        value: &str,
    ) -> (egui::Response, Option<Value>) {
        let key = id.1.clone();
        let Some(engine) = tri::develop::Engine::from_id(&key) else {
            return (ui.label(value), None);
        };
        let found = self.found.iter().find(|f| f.engine == engine).cloned();
        let before = DeveloperMode::of(value, self.manual.contains(&key));
        let mut mode = before;
        let mut new: Option<String> = None;
        let response = ui
            .vertical(|ui| {
                let combo = egui::ComboBox::from_id_salt(("settings_developer", &key))
                    .selected_text(mode.label())
                    .show_ui(ui, |ui| {
                        for m in DeveloperMode::ALL {
                            ui.selectable_value(&mut mode, m, m.label());
                        }
                    });
                if mode != before {
                    self.manual.remove(&key);
                    self.drafts.remove(id);
                    new = match mode {
                        DeveloperMode::Auto => Some("auto".into()),
                        DeveloperMode::Off => Some(String::new()),
                        // Le logiciel trouvé, pour partir de là ; sinon un chemin à donner.
                        DeveloperMode::Manual => match &found {
                            Some(f) => Some(f.path.as_ref().map_or_else(
                                || "flatpak".to_string(),
                                |p| p.to_string_lossy().into_owned(),
                            )),
                            None => {
                                self.manual.insert(key.clone());
                                None
                            }
                        },
                    };
                }
                let value = new.clone().unwrap_or_else(|| value.to_string());
                let value = value.as_str();
                match mode {
                    DeveloperMode::Auto => {
                        let text = found.as_ref().map_or_else(
                            || t!("settings-developer-not-found"),
                            |f| f.location.clone(),
                        );
                        ui.add(egui::Label::new(RichText::new(text).weak().monospace()).truncate())
                            .on_hover_text(t!("settings-developer-auto-tip"));
                    }
                    DeveloperMode::Off => {
                        ui.weak(t!("settings-developer-disabled", name = engine.name()));
                    }
                    DeveloperMode::Manual => {
                        ui.horizontal(|ui| {
                            let draft = self.drafts.entry(id.clone()).or_insert(value.into());
                            let field =
                                ui.add(egui::TextEdit::singleline(draft).desired_width(360.0));
                            if field.lost_focus() && draft != value {
                                new = Some(draft.clone());
                            }
                            if !field.has_focus() && new.is_none() {
                                *draft = value.into();
                            }
                            if ui.button(t!("settings-developer-browse")).clicked() {
                                let start = browse_start(value, found.as_ref());
                                if let Some(path) =
                                    rfd::FileDialog::new().set_directory(start).pick_file()
                                {
                                    new = Some(path.to_string_lossy().into_owned());
                                }
                            }
                        });
                        let value = new.clone().unwrap_or_else(|| value.to_string());
                        let missing = !value.is_empty()
                            && value != "flatpak"
                            && !tri::settings::expand_home(&value).is_file();
                        if missing {
                            ui.colored_label(
                                ui.visuals().error_fg_color,
                                t!("settings-developer-missing"),
                            );
                        }
                    }
                }
                combo.response
            })
            .inner;
        if new.is_some() {
            self.drafts.remove(id);
        }
        (response, new.map(Value::Str))
    }

    /// `[xmp.fields]` en tableau (08/11/2026, idée de l'utilisateur : « bien plus net »
    /// qu'une case par ligne) : une ligne par logiciel, ses champs XMP et ce qu'ils
    /// portent, une case « géré » ; les étoiles (`xmp:Rating`), toujours gérées, en
    /// dernière ligne pour mémoire.
    fn fields_table(
        &mut self,
        ui: &mut Ui,
        section: &tri::config_edit::Section,
    ) -> Option<AppSettings> {
        let rows: [(&str, &str, &str, &str); 4] = [
            (
                "label",
                "settings-fields-label",
                "xmp:Label",
                "settings-fields-for-color",
            ),
            (
                "darktable",
                "settings-fields-darktable",
                "darktable:colorlabels",
                "settings-fields-for-colors",
            ),
            (
                "digikam",
                "settings-fields-digikam",
                "digiKam:ColorLabel, digiKam:PickLabel",
                "settings-fields-for-color-flag",
            ),
            (
                "lightroom",
                "settings-fields-lightroom",
                "xmpDM:pick, xmpDM:good",
                "settings-fields-for-flag",
            ),
        ];
        let mut changed = None;
        let mut set: Option<(String, bool)> = None;
        egui::Grid::new("xmp_fields_table")
            .striped(true)
            .num_columns(4)
            .spacing(vec2(18.0, 6.0))
            .show(ui, |ui| {
                for head in [
                    "settings-fields-head-managed",
                    "settings-fields-head-programs",
                    "settings-fields-head-fields",
                    "settings-fields-head-for",
                ] {
                    ui.label(RichText::new(t!(head)).strong());
                }
                ui.end_row();
                for (key, programs, fields, what) in rows {
                    let entry = section
                        .groups
                        .iter()
                        .flat_map(|g| &g.entries)
                        .find(|e| e.key == key);
                    let mut on = entry
                        .map(|e| self.current(&section.name, e))
                        .is_none_or(|v| v != Value::Bool(false));
                    let in_file = self.file.get(&section.name, key).is_some();
                    let r = ui
                        .checkbox(&mut on, "")
                        .on_hover_text(format!("{}.{key}", section.name));
                    if r.changed() {
                        set = Some((key.to_string(), on));
                    }
                    let name = RichText::new(t!(programs));
                    ui.label(if in_file { name.strong() } else { name });
                    ui.label(RichText::new(fields).monospace());
                    ui.weak(t!(what));
                    ui.end_row();
                }
                let mut always = true;
                ui.add_enabled(false, egui::Checkbox::new(&mut always, ""));
                ui.weak(t!("settings-fields-all"));
                ui.label(RichText::new("xmp:Rating").monospace());
                ui.weak(t!("settings-fields-for-stars"));
                ui.end_row();
            });
        if let Some((key, on)) = set {
            changed = self.commit(&section.name, &key, Some(Value::Bool(on)));
        }
        for key in ["label", "darktable", "digikam", "lightroom"] {
            if let Some(e) = self.errors.get(&(section.name.clone(), key.to_string())) {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
        }
        changed
    }

    /// Champs de couleur gérés d'après le fichier (`[xmp.fields]`, cochés par défaut),
    /// pour l'éditeur de `[xmp] color_sources`.
    fn managed_color_sources(&self) -> Vec<tri::xmp::ColorSource> {
        tri::xmp::ColorSource::ALL
            .into_iter()
            .filter(|s| {
                !matches!(
                    self.file.get("xmp.fields", s.id()),
                    Some(Value::Bool(false))
                )
            })
            .collect()
    }

    /// Éditeur de `[sort] color_order` (idée de l'utilisateur, 31/10/2026) : à gauche,
    /// sous « Tri », cinq emplacements (pastilles vides) qui reçoivent les couleurs dans
    /// l'ordre voulu ; à droite, sous « Disponibles », les couleurs restantes. On glisse
    /// une couleur d'un côté à l'autre, ou dans l'ordre à gauche ; un clic fait de même
    /// (ajouter à la fin, retirer). Rend la réponse du premier emplacement (focus) et la
    /// nouvelle valeur si elle a changé.
    fn color_order_editor(&self, ui: &mut Ui, value: &Value) -> (egui::Response, Option<Value>) {
        use eframe::egui::{Sense, Stroke};
        use tri::xmp::Color;
        let mut chosen = color_order_of(value);
        let before = chosen.clone();
        let size = 18.0;
        let gap = 4.0;
        let faded = ui.visuals().weak_text_color();
        let mut first: Option<egui::Response> = None;
        // Une pastille cliquable et glissable (`dnd_set_drag_payload` plutôt que
        // `dnd_drag_source`, qui avalait le clic) ; vide : un cercle gris.
        let dot = |ui: &mut Ui, color: Option<Color>, drag: Option<ColorDrag>| -> egui::Response {
            let (rect, response) =
                ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
            match color {
                Some(c) => {
                    let dragging = egui::DragAndDrop::payload::<ColorDrag>(ui.ctx())
                        .is_some_and(|d| Some(*d) == drag);
                    crate::marks::color_dot(
                        ui.painter(),
                        rect.center(),
                        size,
                        crate::marks::color_of(&self.colors, c),
                        !dragging,
                    );
                    let name = t!(&format!("color-{}", c.id()));
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::accesskit::Role::Button, true, &name)
                    });
                    if let Some(payload) = drag {
                        response.dnd_set_drag_payload(payload);
                    }
                    response.on_hover_cursor(egui::CursorIcon::Grab)
                }
                None => {
                    ui.painter()
                        .circle_stroke(rect.center(), size * 0.4, Stroke::new(1.2, faded));
                    response
                }
            }
        };
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                // « Tri » : les emplacements, dans l'ordre.
                ui.vertical(|ui| {
                    ui.weak(t!("settings-color-order-sorted"));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for k in 0..Color::ALL.len() {
                            let color = chosen.get(k).copied();
                            let response = dot(ui, color, color.map(|_| ColorDrag::Sorted(k)));
                            if response.dnd_hover_payload::<ColorDrag>().is_some() {
                                ui.painter().rect_stroke(
                                    response.rect.expand(1.0),
                                    3.0,
                                    Stroke::new(1.5, crate::marks::accent_bright(ui.visuals())),
                                    egui::StrokeKind::Outside,
                                );
                            }
                            if let Some(d) = response.dnd_release_payload::<ColorDrag>() {
                                match *d {
                                    ColorDrag::Sorted(from) if from != k => {
                                        let c = chosen.remove(from);
                                        let at = k.min(chosen.len());
                                        chosen.insert(at, c);
                                    }
                                    ColorDrag::Sorted(_) => {}
                                    ColorDrag::Available(c) => {
                                        let at = k.min(chosen.len());
                                        chosen.insert(at, c);
                                    }
                                }
                            } else if response.clicked() && color.is_some() {
                                chosen.remove(k);
                            }
                            if first.is_none() {
                                first = Some(response);
                            }
                        }
                    });
                });
                ui.add_space(12.0);
                // « Disponibles » : ce qui reste, dans l'ordre habituel.
                ui.vertical(|ui| {
                    ui.weak(t!("settings-color-order-available"));
                    let zone = ui
                        .horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            let mut clicked = None;
                            for c in Color::ALL {
                                if chosen.contains(&c) {
                                    continue;
                                }
                                let response = dot(ui, Some(c), Some(ColorDrag::Available(c)));
                                if response.clicked() {
                                    clicked = Some(c);
                                }
                            }
                            // Au moins la place d'une pastille : zone de dépôt visible.
                            ui.allocate_exact_size(vec2(size, size), Sense::hover());
                            clicked
                        })
                        .inner;
                    if let Some(c) = zone {
                        chosen.push(c);
                    }
                    let zone_rect = ui.min_rect();
                    let hovering = ui.input(|i| {
                        i.pointer
                            .latest_pos()
                            .is_some_and(|p| zone_rect.contains(p))
                    });
                    let released = ui.input(|i| i.pointer.any_released());
                    if let Some(d) = egui::DragAndDrop::payload::<ColorDrag>(ui.ctx())
                        && hovering
                    {
                        ui.painter().rect_stroke(
                            zone_rect.expand(2.0),
                            3.0,
                            Stroke::new(1.5, crate::marks::accent_bright(ui.visuals())),
                            egui::StrokeKind::Outside,
                        );
                        if released && let ColorDrag::Sorted(k) = *d {
                            chosen.remove(k);
                            egui::DragAndDrop::clear_payload(ui.ctx());
                        }
                    }
                });
            });
        });
        // La pastille glissée suit le pointeur (`dnd_set_drag_payload` ne la dessine pas).
        if let Some(d) = egui::DragAndDrop::payload::<ColorDrag>(ui.ctx())
            && let Some(pos) = ui.ctx().pointer_latest_pos()
        {
            let color = match *d {
                ColorDrag::Sorted(k) => before.get(k).copied(),
                ColorDrag::Available(c) => Some(c),
            };
            if let Some(c) = color {
                let painter = ui.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::unique("color_order_drag"),
                ));
                crate::marks::color_dot(
                    &painter,
                    pos,
                    size,
                    crate::marks::color_of(&self.colors, c),
                    true,
                );
            }
        }
        let response = first.expect("cinq emplacements");
        let changed = (chosen != before).then(|| {
            Value::Toml(format!(
                "[{}]",
                chosen
                    .iter()
                    .map(|c| format!("\"{}\"", c.id()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        });
        (response, changed)
    }
}

/// Éditeur de `[xmp] color_sources` (08/11/2026, idée de l'utilisateur : comme l'ordre
/// des couleurs du tri, mais vertical) : les champs gérés (`managed`, cochés dans
/// `[xmp.fields]`) dans l'ordre de lecture, numérotés, une ligne par champ qu'on glisse à
/// sa place ; grisé s'il en reste un seul (rien à ordonner). Les champs non gérés gardent
/// leur place dans la valeur. Rend la réponse de la première ligne (focus) et la nouvelle
/// valeur si elle a changé.
fn color_sources_editor(
    ui: &mut Ui,
    value: &Value,
    managed: &[tri::xmp::ColorSource],
) -> (egui::Response, Option<Value>) {
    use eframe::egui::{Sense, Stroke, pos2};
    let before = color_sources_of(value);
    let mut order = before;
    // Rangs, dans la valeur complète, des champs montrés.
    let shown: Vec<usize> = (0..3).filter(|&i| managed.contains(&before.0[i])).collect();
    let movable = shown.len() > 1;
    let row_size = vec2(300.0, 24.0);
    let response = ui
        .add_enabled_ui(movable, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                if shown.is_empty() {
                    return ui.weak(t!("settings-color-sources-none"));
                }
                let mut first: Option<egui::Response> = None;
                for (n, &i) in shown.iter().enumerate() {
                    let source = before.0[i];
                    let name = t!(&format!("settings-color-source-{}", source.id()));
                    let sense = if movable {
                        Sense::click_and_drag()
                    } else {
                        Sense::hover()
                    };
                    let (rect, response) = ui.allocate_exact_size(row_size, sense);
                    let visuals = ui.visuals();
                    let dragging = egui::DragAndDrop::payload::<SourceDrag>(ui.ctx())
                        .is_some_and(|d| d.0 == n);
                    let fill = if dragging {
                        visuals.extreme_bg_color
                    } else if response.hovered() && movable {
                        visuals.widgets.hovered.weak_bg_fill
                    } else {
                        visuals.widgets.inactive.weak_bg_fill
                    };
                    ui.painter().rect_filled(rect, 4.0, fill);
                    let text_color = if movable {
                        visuals.text_color()
                    } else {
                        visuals.weak_text_color()
                    };
                    // Poignée : trois traits à gauche.
                    if movable {
                        for k in -1..=1 {
                            let y = rect.center().y + k as f32 * 3.5;
                            ui.painter().line_segment(
                                [pos2(rect.left() + 7.0, y), pos2(rect.left() + 15.0, y)],
                                Stroke::new(1.2, visuals.weak_text_color()),
                            );
                        }
                    }
                    ui.painter().text(
                        pos2(rect.left() + 24.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        format!("{}. {name}", n + 1),
                        egui::TextStyle::Body.resolve(ui.style()),
                        text_color,
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::accesskit::Role::ListItem, true, &name)
                    });
                    let response = if movable {
                        response.dnd_set_drag_payload(SourceDrag(n));
                        response.on_hover_cursor(egui::CursorIcon::Grab)
                    } else {
                        response
                    };
                    if response.dnd_hover_payload::<SourceDrag>().is_some() {
                        ui.painter().rect_stroke(
                            rect,
                            4.0,
                            Stroke::new(1.5, crate::marks::accent_bright(ui.visuals())),
                            egui::StrokeKind::Outside,
                        );
                    }
                    if let Some(d) = response.dnd_release_payload::<SourceDrag>() {
                        order = move_source(before, &shown, d.0, n);
                    }
                    first.get_or_insert(response);
                }
                first.expect("au moins un champ")
            })
            .inner
        })
        .inner;
    // La ligne glissée suit le pointeur.
    if let Some(d) = egui::DragAndDrop::payload::<SourceDrag>(ui.ctx())
        && let Some(pos) = ui.ctx().pointer_latest_pos()
        && let Some(&i) = shown.get(d.0)
    {
        let painter = ui.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::unique("color_sources_drag"),
        ));
        let name = t!(&format!("settings-color-source-{}", before.0[i].id()));
        painter.text(
            pos + vec2(12.0, 0.0),
            egui::Align2::LEFT_CENTER,
            name,
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().strong_text_color(),
        );
    }
    let changed = (order != before).then(|| {
        Value::Toml(format!(
            "[{}]",
            order
                .0
                .iter()
                .map(|s| format!("\"{}\"", s.id()))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    });
    (response, changed)
}

/// Ligne glissée dans l'éditeur de `[xmp] color_sources`, par son rang parmi les champs
/// montrés.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceDrag(usize);

/// `order` avec le champ montré au rang `from` déplacé au rang `to` (parmi les champs
/// montrés, `shown` : leurs places dans la valeur complète) ; les champs cachés ne bougent
/// pas.
fn move_source(
    order: tri::xmp::ColorSources,
    shown: &[usize],
    from: usize,
    to: usize,
) -> tri::xmp::ColorSources {
    let mut visible: Vec<tri::xmp::ColorSource> = shown.iter().map(|&i| order.0[i]).collect();
    if from >= visible.len() || to >= visible.len() {
        return order;
    }
    let moved = visible.remove(from);
    visible.insert(to, moved);
    let mut out = order;
    for (&i, s) in shown.iter().zip(visible) {
        out.0[i] = s;
    }
    out
}

/// L'ordre de la valeur TOML de `[xmp] color_sources` (`["label", "darktable"]`) ; une
/// entrée inconnue est ignorée, une oubliée ajoutée à la fin.
fn color_sources_of(value: &Value) -> tri::xmp::ColorSources {
    let text = match value {
        Value::Strings(list) => list.join(","),
        Value::Str(s) | Value::Toml(s) => s.clone(),
        _ => String::new(),
    };
    text.split(|c: char| !c.is_ascii_alphabetic())
        .filter_map(tri::xmp::ColorSource::from_id)
        .collect::<Vec<_>>()
        .into()
}

/// Ce qu'on glisse dans l'éditeur de `[sort] color_order`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorDrag {
    /// Une couleur déjà triée, par son rang.
    Sorted(usize),
    /// Une couleur disponible.
    Available(tri::xmp::Color),
}

/// Les couleurs de la valeur TOML de `[sort] color_order` (`["red", "green"]`), dans
/// l'ordre, sans doublon ; une entrée inconnue est ignorée.
fn color_order_of(value: &Value) -> Vec<tri::xmp::Color> {
    let text = match value {
        Value::Strings(list) => list.join(","),
        Value::Str(s) | Value::Toml(s) => s.clone(),
        _ => String::new(),
    };
    let mut colors = Vec::new();
    for word in text.split(|c: char| !c.is_ascii_alphabetic()) {
        if let Some(c) = tri::xmp::Color::from_id(word)
            && !colors.contains(&c)
        {
            colors.push(c);
        }
    }
    colors
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
    fn every_search_keyword_names_a_setting_a_section_or_an_action() {
        crate::i18n::ensure();
        let mut known: Vec<String> = Action::ALL.iter().map(|a| a.id().to_string()).collect();
        known.extend(
            crate::palette::CONTROL_KEYWORDS
                .iter()
                .map(|id| id.to_string()),
        );
        for section in &schema().sections {
            known.push(section.name.clone());
            for group in &section.groups {
                for entry in &group.entries {
                    known.push(format!("{}.{}", section.name, entry.key));
                }
            }
        }
        let mut known: Vec<String> = known
            .iter()
            .map(|id| format!("kw-{}", id.replace(['.', '_'], "-")))
            .collect();
        // Mots-clés d'un thème de l'aide (`kw-group-rating`).
        known.extend(
            crate::actions::Group::ALL
                .iter()
                .map(|g| format!("kw-group-{g:?}").to_lowercase()),
        );
        for source in [
            include_str!("../locales/fr.ftl"),
            include_str!("../locales/en.ftl"),
        ] {
            for id in tri::i18n::ids(source)
                .iter()
                .filter(|i| i.starts_with("kw-"))
            {
                assert!(known.contains(id), "{id} ne désigne rien");
            }
        }
        assert!(crate::actions::keywords("gui.profile").contains("vim"));
        assert!(crate::actions::keywords("gui.nothing").is_empty());
    }

    #[test]
    fn search_keeps_sections_and_finds_titles_and_keywords() {
        crate::i18n::ensure();
        let dir = tempfile::tempdir().unwrap();
        let mut window = SettingsWindow::open(&dir.path().join("config.toml")).unwrap();
        let schema = window.schema();
        let name = |hits: &[(usize, Found)], at: usize| schema.sections[hits[at].0].name.clone();
        // « vim » : le jeu de raccourcis en tête, peu de résultats (exacts seulement).
        window.query = "vim".into();
        let hits = window.hits(&schema);
        assert_eq!(name(&hits, 0), "gui");
        assert!(
            hits[0].1.keys.contains(&"profile".to_string()),
            "{:?}",
            hits[0].1.keys
        );
        let total: usize = hits
            .iter()
            .map(|(_, f)| f.keys.len() + f.actions.len())
            .sum();
        assert!(total < 15, "{total}");
        // Le titre d'une section la trouve en entier.
        window.query = section_label("gui.zoom");
        let hits = window.hits(&schema);
        let zoom = hits
            .iter()
            .find(|(i, _)| schema.sections[*i].name == "gui.zoom")
            .unwrap();
        let all = schema.sections[zoom.0]
            .groups
            .iter()
            .map(|g| g.entries.len())
            .sum::<usize>();
        assert_eq!(zoom.1.keys.len(), all);
    }

    #[test]
    fn settings_without_purpose_are_hidden_without_a_development_software() {
        // Retour du 04/11/2026 : sans darktable, RapidRAW, RawTherapee ni ART, leurs
        // réglages n'ont pas de sens ; les chemins des logiciels restent.
        use tri::develop::Engine;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut w = SettingsWindow::open(&path).unwrap();
        let schema = schema();
        let section = |name: &str| schema.sections.iter().find(|s| s.name == name).unwrap();
        assert!(w.hidden("gui.view", "review_developments"));
        assert!(w.hidden("develop", "default"));
        assert!(!w.hidden("develop", "darktable"), "le chemin reste");
        assert!(w.section_hidden(section("sidecars.rapidraw")));
        assert!(!w.section_hidden(section("develop")));
        w.engines = vec![Engine::RapidRaw];
        assert!(!w.hidden("gui.view", "review_developments"));
        assert!(!w.section_hidden(section("sidecars.rapidraw")));
        assert!(
            w.section_hidden(section("sidecars.art")),
            "ART pas installé"
        );
        assert!(w.hidden("develop", "high_quality"), "darktable seulement");
        // Chaque clé masquée est un vrai réglage (un renommage ne la laisse pas filer).
        for (name, key) in NEED_A_DEVELOPER.iter().chain(&NEED_DARKTABLE) {
            assert!(
                section(name)
                    .groups
                    .iter()
                    .flat_map(|g| &g.entries)
                    .any(|e| e.key == *key),
                "{name}.{key}"
            );
        }
        for engine in Engine::ALL {
            assert!(
                schema
                    .sections
                    .iter()
                    .any(|s| s.name == format!("sidecars.{}", engine.id()))
                    || engine == Engine::Darktable,
                "[sidecars.{}]",
                engine.id()
            );
        }
    }

    #[test]
    fn each_development_software_is_auto_manual_or_disabled() {
        // Idée de l'utilisateur (04/11/2026) : une liste par logiciel ; Auto dit où il a été
        // trouvé, Manuel donne un chemin (Parcourir…), Désactivé le dit.
        use egui_kittest::kittest::Queryable;
        use tri::develop::{Engine, Found};
        crate::i18n::ensure();
        assert_eq!(DeveloperMode::of("auto", false), DeveloperMode::Auto);
        assert_eq!(DeveloperMode::of("", false), DeveloperMode::Off);
        assert_eq!(DeveloperMode::of("", true), DeveloperMode::Manual);
        assert_eq!(DeveloperMode::of("flatpak", false), DeveloperMode::Manual);
        assert_eq!(
            DeveloperMode::of("~/AppImages/x.AppImage", false),
            DeveloperMode::Manual
        );
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("darktable-cli");
        std::fs::write(&program, b"").unwrap();
        let found = Found {
            engine: Engine::Darktable,
            location: program.display().to_string(),
            path: Some(program.clone()),
        };
        assert_eq!(browse_start("", Some(&found)), dir.path());
        assert_eq!(
            browse_start("", None),
            std::path::PathBuf::from(std::path::MAIN_SEPARATOR_STR),
            "rien de trouvé : la racine"
        );
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[develop]\nrapidraw = \"\"\nart = \"/nulle/part/ART-cli\"\n",
        )
        .unwrap();
        let mut w = SettingsWindow::open(&path)
            .unwrap()
            .with_engines(vec![Engine::Darktable])
            .with_found(vec![found]);
        w.show_section("develop");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 1400.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, w: &mut SettingsWindow| {
                    w.show(ui.ctx());
                },
                w,
            );
        h.run_steps(2);
        let place = program.display().to_string();
        assert!(
            h.query_by_label(&place).is_some(),
            "darktable : où il a été trouvé"
        );
        assert!(h.query_by_label("RapidRAW désactivé").is_some());
        assert!(h.query_by_label("Non trouvé.").is_some(), "RawTherapee");
        assert!(
            h.query_by_label("Fichier introuvable.").is_some(),
            "ART : chemin faux"
        );
        assert_eq!(
            h.query_all_by_label("Parcourir…").count(),
            1,
            "ART seulement"
        );
        // Manuel : le chemin trouvé écrit dans le fichier.
        assert!(!h.state().file.text().contains("darktable ="));
        let read = h
            .state_mut()
            .commit("develop", "darktable", Some(Value::Str(place.clone())))
            .expect("chemin accepté");
        assert_eq!(read.develop.darktable, place);
        h.run_steps(2);
        assert_eq!(
            h.query_all_by_label("Parcourir…").count(),
            2,
            "darktable aussi"
        );
    }

    #[test]
    fn a_number_is_typed_whole_before_being_checked() {
        // Retour du 07/11/2026 : `max` (zoom, 100 % au moins) ne se changeait pas au
        // clavier : « 800 » passait par « 8 », refusé.
        use egui_kittest::kittest::Queryable;
        crate::i18n::ensure();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut w = SettingsWindow::open(&path).unwrap();
        w.show_section("gui.zoom");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 1400.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, w: &mut SettingsWindow| {
                    w.show(ui.ctx());
                },
                w,
            );
        h.run_steps(2);
        h.get_by_value("400").click();
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        for c in ["8", "0", "0"] {
            h.event(egui::Event::Text(c.into()));
            h.run_steps(1);
            assert!(
                h.state().errors.is_empty(),
                "pas vérifié en cours de frappe"
            );
        }
        h.key_press(egui::Key::Enter);
        h.run_steps(2);
        assert!(h.state().errors.is_empty(), "{:?}", h.state().errors);
        assert!(
            h.state().file.text().contains("max = 800"),
            "{}",
            h.state().file.text()
        );
    }

    #[test]
    fn a_colour_that_follows_the_accent_is_a_box_then_a_picker() {
        // Retour du 05/11/2026 : « accent » s'écrivait dans un champ de texte.
        use egui_kittest::kittest::Queryable;
        crate::i18n::ensure();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut w = SettingsWindow::open(&path).unwrap();
        w.show_section("gui.dialogs");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 1400.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, w: &mut SettingsWindow| {
                    w.show(ui.ctx());
                },
                w,
            );
        h.run_steps(2);
        assert!(
            !h.state()
                .drafts
                .contains_key(&("gui.dialogs".to_string(), "focus_line".to_string())),
            "pas de champ de texte"
        );
        h.get_by_label("Couleur d'accent").click();
        h.run_steps(2);
        let written = std::fs::read_to_string(&path).unwrap();
        let line = written
            .lines()
            .find(|l| l.starts_with("focus_line"))
            .expect("écrit");
        let value = line.split('"').nth(1).unwrap();
        assert!(parse_color(value).is_some(), "une couleur fixe : {line}");
        assert!(
            h.query_by_label(value).is_some(),
            "sélecteur et valeur montrés"
        );
        h.get_by_label("Couleur d'accent").click();
        h.run_steps(2);
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("focus_line = \"accent\""), "{written}");
    }

    #[test]
    fn the_external_editor_is_the_system_one_or_another_chosen() {
        // Idée de l'utilisateur (04/11/2026) : plus de liste TOML à taper ; l'application
        // du système par défaut (et laquelle, pour information), ou une autre choisie par
        // Parcourir…, avec `{files}` déjà en argument.
        use egui_kittest::kittest::Queryable;
        crate::i18n::ensure();
        assert_eq!(editor_list(&Value::Toml("[]".into())), Vec::<String>::new());
        assert_eq!(
            editor_list(&Value::Toml(r#"["gimp", "{files}"]"#.into())),
            ["gimp", "{files}"]
        );
        assert!(!program_exists("/nulle/part/gimp"));
        assert!(!program_exists("pas-un-programme-connu"));
        assert!(program_start("").is_dir());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut w = SettingsWindow::open(&path)
            .unwrap()
            .with_default_apps(vec![("RAF".into(), "darktable".into())]);
        w.show_section("files");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 1400.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, w: &mut SettingsWindow| {
                    w.show(ui.ctx());
                },
                w,
            );
        h.run_steps(2);
        assert!(h.query_by_label_contains("RAF : darktable").is_some());
        assert!(
            h.query_by_label("Programme").is_none(),
            "système : rien à taper"
        );
        // « Autre application » : le programme à choisir, `{files}` déjà là.
        h.state_mut().manual.insert("editor".into());
        h.run_steps(2);
        assert!(h.query_by_label("Programme").is_some());
        assert_eq!(
            h.state()
                .drafts
                .get(&("files".to_string(), "editor/args".to_string()))
                .map(String::as_str),
            Some("{files}"),
            "argument par défaut"
        );
        let read = h
            .state_mut()
            .commit(
                "files",
                "editor",
                Some(Value::Strings(vec![
                    "/nulle/part/gimp".into(),
                    "{files}".into(),
                ])),
            )
            .expect("accepté");
        assert_eq!(read.files.editor, ["/nulle/part/gimp", "{files}"]);
        h.run_steps(2);
        assert!(h.query_by_label("Programme introuvable.").is_some());
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
                    let mut text = format!("[{}]\n{} = \"{choice}\"\n", section.name, entry.key);
                    // JPEG d'un profil d'export en ProPhoto : masqué tant que l'espace n'a pas
                    // changé (`fits`) ; accepté avec un espace 8 bits.
                    if section.name.starts_with("export.profiles.") && *choice == "jpeg" {
                        text.push_str("color = \"srgb\"\n");
                    }
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
            ("xmp", "embed"),
            ("sidecars.rapidraw", "create"),
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
        // Jusqu'à la dernière section : la liste défile pour la montrer (sections masquées
        // sautées).
        let shown = h.state().shown_sections(schema());
        let last = *shown.last().unwrap();
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
    fn export_profiles_only_offer_coherent_choices() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = SettingsWindow::open(&dir.path().join("config.toml")).unwrap();
        let (edit, web) = ("export.profiles.edit", "export.profiles.web");
        // Retouche (TIFF 16 bits, ProPhoto) : tous les espaces, mais pas de JPEG.
        assert!(w.fits(edit, "color", "prophoto"));
        assert!(!w.fits(edit, "format", "jpeg"));
        // Web (JPEG) : ni ProPhoto ni espaces linéaires.
        assert!(!w.fits(web, "color", "prophoto") && !w.fits(web, "color", "rec2020"));
        assert!(w.fits(web, "color", "adobergb"));
        // Retouche passée en sRGB : le JPEG devient possible, et le moteur l'accepte.
        w.commit(edit, "color", Some(Value::Str("srgb".into())))
            .expect("valide");
        assert!(w.fits(edit, "format", "jpeg"));
        w.commit(edit, "format", Some(Value::Str("jpeg".into())))
            .expect("valide");
        // Puis ProPhoto en JPEG : refusé, avec la raison.
        assert!(
            w.commit(edit, "color", Some(Value::Str("prophoto".into())))
                .is_none()
        );
        let reason = &w.errors[&(edit.to_string(), "color".to_string())];
        assert!(reason.contains("16"), "{reason}");
    }

    #[test]
    fn profiles_and_presets_are_created_from_another_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut w = SettingsWindow::open(&path).unwrap();
        // Nouveau profil « Tirage A4 » à partir de Web : toutes ses valeurs écrites.
        let s = w.create_from("export.profiles.web", "Tirage A4").unwrap();
        let tirage = s.export.profiles.get("tirage-a4").expect("créé");
        assert_eq!(tirage.name, "Tirage A4");
        assert_eq!(tirage.max_side, 2048, "valeurs de Web");
        assert_eq!(
            w.file.get("export.profiles.tirage-a4", "format"),
            Some(Value::Str("jpeg".into()))
        );
        let schema = w.schema();
        let names: Vec<&str> = schema.sections.iter().map(|s| s.name.as_str()).collect();
        let at = names
            .iter()
            .position(|n| *n == "export.profiles.tirage-a4")
            .unwrap();
        assert_eq!(
            names[at - 1],
            "export.profiles.web",
            "à la suite des profils"
        );
        assert_eq!(w.section, at, "montré");
        // Une seule entrée par famille à gauche, qui montre le profil choisi.
        let shown: Vec<&str> = listed(&schema)
            .into_iter()
            .map(|i| schema.sections[i].name.as_str())
            .filter(|n| n.starts_with("export.profiles."))
            .collect();
        assert_eq!(shown, ["export.profiles.edit"]);
        assert_eq!(names[listed_of(&schema, at)], "export.profiles.edit");
        assert_eq!(
            member_name(&w.file, "export.profiles.tirage-a4"),
            "Tirage A4"
        );
        assert_eq!(
            section_label("export.profiles.tirage-a4"),
            "Préréglage d'export « tirage-a4 »"
        );
        // Modifiable comme les autres.
        let s = w
            .commit(
                "export.profiles.tirage-a4",
                "max_side",
                Some(Value::Int(4000)),
            )
            .unwrap();
        assert_eq!(s.export.profiles.get("tirage-a4").unwrap().max_side, 4000);
        // Supprimable ; un profil fourni, non.
        assert!(w.delete_section("export.profiles.web").is_none());
        let s = w.delete_section("export.profiles.tirage-a4").unwrap();
        assert!(s.export.profiles.get("tirage-a4").is_none());
        assert!(
            !w.schema()
                .sections
                .iter()
                .any(|s| s.name == "export.profiles.tirage-a4")
        );
        // Préréglage de recadrage : de même.
        let s = w.create_from("versions.presets.story", "Reel").unwrap();
        let reel = s
            .versions
            .presets
            .0
            .iter()
            .find(|p| p.id == "reel")
            .unwrap();
        assert_eq!(reel.ratio, tri::versions::Ratio::Fixed(9, 16));
        assert!(w.delete_section("versions.presets.story").is_none());
    }

    #[test]
    fn members_are_renamed_from_the_header_without_duplicate_names() {
        use egui_kittest::kittest::Queryable;
        crate::i18n::ensure();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut w = SettingsWindow::open(&path).unwrap();
        w.create_from("export.profiles.web", "Tirage").unwrap();
        w.show_section("export.profiles.tirage");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 1000.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, w: &mut SettingsWindow| {
                    w.show(ui.ctx());
                },
                w,
            );
        h.run_steps(2);
        // Issue n° 18 : le nom n'est plus un réglage modifiable, mais « Renommer… ».
        assert!(h.query_by_label("max_side").is_some());
        assert!(h.query_by_label("name").is_none());
        h.get_by_label("Renommer…").click();
        h.run_steps(2);
        let naming = match &h.state().naming {
            Some(Naming::Rename(n)) => format!("R {n}"),
            Some(Naming::New(n)) => format!("N {n}"),
            None => "-".into(),
        };
        assert_eq!(naming, "R Tirage");
        // Issue n° 17 : un nom déjà pris dans la famille (casse mise à part) est refusé.
        h.state_mut().naming = Some(Naming::Rename(" web".into()));
        h.run_steps(2);
        assert!(h.query_by_label("Ce nom est déjà pris").is_some());
        h.get_by_label("Renommer").click();
        h.run_steps(2);
        assert_eq!(
            member_name(&h.state().file, "export.profiles.tirage"),
            "Tirage"
        );
        h.state_mut().naming = Some(Naming::New("TIRAGE".into()));
        h.run_steps(2);
        assert!(h.query_by_label("Ce nom est déjà pris").is_some());
        // Son propre nom, ou un nom libre : accepté.
        h.state_mut().naming = Some(Naming::Rename("Tirage A3".into()));
        h.run_steps(2);
        assert!(h.query_by_label("Ce nom est déjà pris").is_none());
        h.get_by_label("Renommer").click();
        h.run_steps(2);
        assert!(h.state().naming.is_none());
        assert_eq!(
            member_name(&h.state().file, "export.profiles.tirage"),
            "Tirage A3"
        );
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("name = \"Tirage A3\""),
            "écrit"
        );
    }

    #[test]
    fn a_section_clicked_during_a_search_opens_like_without_search() {
        use egui_kittest::kittest::Queryable;
        crate::i18n::ensure();
        let dir = tempfile::tempdir().unwrap();
        let mut w = SettingsWindow::open(&dir.path().join("config.toml")).unwrap();
        w.search("export");
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1400.0, 1000.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, w: &mut SettingsWindow| {
                    w.show(ui.ctx());
                },
                w,
            );
        h.run_steps(2);
        // Issue n° 24 : tous les préréglages d'export s'affichaient l'un après l'autre.
        h.get_by_label_contains("Préréglages d'export").click();
        h.run_steps(2);
        let w = h.state();
        assert!(w.query.is_empty(), "recherche quittée");
        let section = &w.schema().sections[w.section].name;
        assert!(section.starts_with("export.profiles."), "{section}");
        h.run_steps(2);
        assert!(
            h.query_by_label("Renommer…").is_some(),
            "en-tête de la famille"
        );
    }

    #[test]
    fn the_colour_order_is_built_by_clicking_the_dots() {
        // Idée de l'utilisateur (31/10/2026) : pastilles « Tri » / « Disponibles » ; au
        // clic (le glisser-déposer fait de même), une disponible s'ajoute à la fin, une
        // triée se retire ; la liste est écrite à chaque changement.
        crate::i18n::ensure();
        use egui_kittest::kittest::Queryable;
        use tri::xmp::Color;
        assert_eq!(
            color_order_of(&Value::Toml(r#"["blue", "Red", "blue", "mauve"]"#.into())),
            [Color::Blue, Color::Red]
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut w = SettingsWindow::open(&path).unwrap();
        w.show_section("sort");
        let mut h = egui_kittest::Harness::builder()
            .with_size(vec2(1400.0, 1000.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, w: &mut SettingsWindow| {
                    w.show(ui.ctx());
                },
                w,
            );
        h.run_steps(3);
        // Par défaut, les cinq sont triées : « bleu » est une pastille triée ; cliquée,
        // elle part dans les disponibles.
        h.get_by_label("bleu").click();
        h.run_steps(2);
        assert_eq!(
            h.state()
                .file
                .get("sort", "color_order")
                .map(|v| v.to_string()),
            Some(r#"["red", "yellow", "green", "purple"]"#.to_string())
        );
        // Cliquée à nouveau (disponible) : à la fin.
        h.get_by_label("bleu").click();
        h.run_steps(2);
        assert_eq!(
            h.state()
                .file
                .get("sort", "color_order")
                .map(|v| v.to_string()),
            Some(r#"["red", "yellow", "green", "purple", "blue"]"#.to_string())
        );
        // La valeur est bien celle que le moteur relit.
        let loaded = tri::settings::load::<crate::settings::AppSettings>(Some(&path));
        assert!(loaded.warning.is_none(), "{:?}", loaded.warning);
        assert_eq!(
            loaded.settings.sort.color_order,
            [
                Color::Red,
                Color::Yellow,
                Color::Green,
                Color::Purple,
                Color::Blue
            ]
        );
    }

    #[test]
    fn the_colour_sources_are_reordered_by_dragging() {
        // 08/11/2026 : un ordre plutôt qu'un gagnant (darktable, Label, digiKam), qu'on
        // glisse comme l'ordre des couleurs du tri, mais verticalement.
        crate::i18n::ensure();
        use ColorSource::{Darktable, Digikam, Label};
        use egui_kittest::kittest::NodeT;
        use egui_kittest::kittest::Queryable;
        use tri::xmp::{ColorSource, ColorSources};
        assert_eq!(
            color_sources_of(&Value::Toml(r#"["digikam", "nope", "digikam"]"#.into())).0,
            [Digikam, Darktable, Label]
        );
        // Déplacer parmi les champs montrés ; un champ caché garde sa place.
        let all = ColorSources([Darktable, Label, Digikam]);
        assert_eq!(
            move_source(all, &[0, 1, 2], 2, 0).0,
            [Digikam, Darktable, Label]
        );
        assert_eq!(
            move_source(all, &[0, 1, 2], 0, 2).0,
            [Label, Digikam, Darktable]
        );
        assert_eq!(
            move_source(all, &[1, 2], 1, 0).0,
            [Darktable, Digikam, Label]
        );
        assert_eq!(
            move_source(all, &[1, 2], 5, 0),
            all,
            "rang hors liste : rien"
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut w = SettingsWindow::open(&path).unwrap();
        w.show_section("xmp");
        let mut h = egui_kittest::Harness::builder()
            .with_size(vec2(1400.0, 1000.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, w: &mut SettingsWindow| {
                    w.show(ui.ctx());
                },
                w,
            );
        h.run_steps(3);
        let rows = |h: &egui_kittest::Harness<'_, SettingsWindow>| {
            h.query_all_by_role(egui::accesskit::Role::ListItem)
                .map(|n| n.accesskit_node().label().unwrap_or_default())
                .collect::<Vec<_>>()
        };
        assert_eq!(rows(&h).len(), 3, "{:?}", rows(&h));
        // darktable décoché : il disparaît de la liste.
        h.state_mut()
            .file
            .set("xmp.fields", "darktable", &Value::Bool(false))
            .unwrap();
        h.run_steps(2);
        assert_eq!(rows(&h).len(), 2, "{:?}", rows(&h));
        assert!(rows(&h)[0].contains("xmp:Label"), "{:?}", rows(&h));
        // Label seul géré : une ligne, rien à ordonner (grisée).
        h.state_mut()
            .file
            .set("xmp.fields", "digikam", &Value::Bool(false))
            .unwrap();
        h.run_steps(2);
        assert_eq!(rows(&h).len(), 1, "{:?}", rows(&h));
        assert!(
            h.query_all_by_role(egui::accesskit::Role::ListItem)
                .all(|n| n.accesskit_node().is_disabled())
        );
    }

    #[test]
    fn the_handled_fields_are_a_table() {
        // 08/11/2026, idée de l'utilisateur : un tableau plutôt qu'une case par ligne.
        crate::i18n::ensure();
        use egui_kittest::kittest::NodeT;
        use egui_kittest::kittest::Queryable;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut w = SettingsWindow::open(&path).unwrap();
        w.show_section("xmp.fields");
        let mut h = egui_kittest::Harness::builder()
            .with_size(vec2(1400.0, 1000.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, w: &mut SettingsWindow| {
                    w.show(ui.ctx());
                },
                w,
            );
        h.run_steps(3);
        for text in [
            "Géré",
            "Logiciels",
            "Lightroom Classic",
            "darktable:colorlabels",
        ] {
            assert!(h.query_by_label(text).is_some(), "{text}");
        }
        // Quatre cases actives (et celle des étoiles, grisée) ; décocher la deuxième
        // (darktable) l'écrit dans le fichier.
        let boxes: Vec<_> = h
            .query_all_by_role(egui::accesskit::Role::CheckBox)
            .filter(|n| !n.accesskit_node().is_disabled())
            .collect();
        assert_eq!(boxes.len(), 4);
        boxes[1].click();
        h.run_steps(2);
        assert_eq!(
            h.state().file.get("xmp.fields", "darktable"),
            Some(Value::Bool(false))
        );
        let loaded = tri::settings::load::<crate::settings::AppSettings>(Some(&path));
        assert!(loaded.warning.is_none(), "{:?}", loaded.warning);
        assert!(!loaded.settings.xmp.fields.darktable);
        // Aussi en tableau dans les résultats d'une recherche.
        h.state_mut().query = "xmp".into();
        h.run_steps(3);
        assert!(h.query_by_label("Lightroom Classic").is_some());
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
        // Le message seul, sans la ligne du fichier ni ses `^^^^` (retour du 07/11/2026).
        assert!(
            w.commit("gui.zoom", "max", Some(Value::Int(4000)))
                .is_none()
        );
        let message = &w.errors[&("gui.zoom".into(), "max".into())];
        assert!(message.contains("4000 %"), "{message}");
        assert!(
            !message.contains("line") && !message.contains('^'),
            "{message}"
        );
        // Puis des valeurs correctes passent (retour du 07/11/2026 : le message restait).
        for (key, value) in [
            ("max", Value::Int(800)),
            ("max", Value::Float(800.0)),
            ("min", Value::Int(5)),
            ("min", Value::Float(20.0)),
        ] {
            let r = w.commit("gui.zoom", key, Some(value.clone()));
            assert!(r.is_some(), "{key} = {value:?} : {:?}", w.errors);
        }
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
