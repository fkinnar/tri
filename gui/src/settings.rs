//! Réglages de l'interface : le fichier de configuration entier, sections du moteur
//! comprises, plus la section `[gui]`.
//!
//! Même fichier que le moteur (voir `tri::settings`) ; toutes les valeurs par défaut sont
//! dans `doc/config.example.toml`, qu'un test compare au code.

use std::collections::BTreeMap;
use std::time::Duration;

use eframe::egui::{self, Color32};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use tri::scheduler::RawPreviews;
use tri::settings::{
    CacheSection, EngineSettings, PreviewSection, RenderSection, SchedulerSection, XmpSection,
};

use crate::actions::{Keymap, Profile};

/// Le fichier de configuration entier, vu par l'interface. Toute section ou clé inconnue
/// est signalée.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppSettings {
    /// `[preview]` (moteur).
    pub preview: PreviewSection,
    /// `[render]` (moteur).
    pub render: RenderSection,
    /// `[cache]` (moteur).
    pub cache: CacheSection,
    /// `[scheduler]` (moteur).
    pub scheduler: SchedulerSection,
    /// `[xmp]` (moteur).
    pub xmp: XmpSection,
    /// `[sidecars]` (moteur) : création des sidecars des logiciels quand on note.
    pub sidecars: tri::sidecars::SidecarSettings,
    /// `[places]` (moteur).
    pub places: tri::places::PlaceSettings,
    /// `[watch]` (moteur) : surveillance du dossier ouvert.
    pub watch: tri::watch::WatchSettings,
    /// `[sort]` (moteur) : ordre des couleurs du tri par couleur.
    pub sort: tri::sort::SortSettings,
    /// `[develop]` (moteur) : rendu par darktable.
    pub develop: tri::develop::DevelopSettings,
    /// `[export]` (moteur) : profils d'export, destination.
    pub export: tri::export::ExportSettings,
    /// `[versions]` (moteur) : préréglages de recadrage.
    pub versions: tri::versions::VersionSettings,
    /// `[exposure]` (moteur).
    pub exposure: tri::exposure::ExposureSettings,
    /// `[offload]` (moteur).
    pub offload: tri::offload::OffloadSettings,
    /// `[data]` (moteur).
    pub data: tri::settings::DataSection,
    /// `[files]` (moteur).
    pub files: tri::files::FileSettings,
    /// `[sharpness]` (moteur).
    pub sharpness: tri::sharpness::SharpnessSettings,
    /// `[similarity]` (moteur) : regroupement des images semblables.
    pub similarity: tri::similarity::SimilaritySettings,
    /// `[i18n]` (moteur) : langue des textes.
    pub i18n: tri::i18n::I18nSettings,
    /// `[update]` (moteur) : recherche d'une nouvelle version.
    pub update: tri::update::UpdateSettings,
    /// `[remote]`.
    pub remote: tri::remote::RemoteSettings,
    /// `[neighbourhood]`.
    pub neighbourhood: tri::neighbourhood::NeighbourhoodSettings,
    /// `[gui]`.
    pub gui: GuiSettings,
    /// `[keys]` : raccourcis par identifiant d'action (`"core.flag.accept" = ["Maj+P"]`),
    /// qui remplacent ceux des profils pour les actions citées.
    pub keys: BTreeMap<String, Vec<String>>,
}

impl AppSettings {
    /// Sections du moteur.
    pub fn engine(&self) -> EngineSettings {
        EngineSettings {
            preview: self.preview,
            render: self.render,
            cache: self.cache.clone(),
            scheduler: self.scheduler,
            xmp: self.xmp,
            sidecars: self.sidecars,
            places: self.places.clone(),
            watch: self.watch,
            sort: self.sort.clone(),
            develop: self.develop.clone(),
            export: self.export.clone(),
            versions: self.versions.clone(),
            exposure: self.exposure,
            offload: self.offload.clone(),
            data: self.data.clone(),
            files: self.files.clone(),
            sharpness: self.sharpness,
            similarity: self.similarity,
            i18n: self.i18n,
            update: self.update.clone(),
            remote: self.remote.clone(),
            neighbourhood: self.neighbourhood.clone(),
        }
    }

    /// Table des raccourcis : profils de `[gui] keys`, puis section `[keys]` ; avec les
    /// problèmes à signaler (touche inconnue, conflit...).
    pub fn keymap(&self) -> (Keymap, Vec<String>) {
        let defaults = crate::actions::Defaults::get();
        // Problèmes de `keys.toml` (raccourcis par défaut) d'abord.
        let mut warnings: Vec<String> = crate::actions::Defaults::warnings().to_vec();
        if self.gui.profile == Profile::Both {
            warnings.push(t!("keys-profile-both"));
        }
        // Préfixe des panneaux : celui de `[gui] leader`, sinon celui de `keys.toml`.
        // Refusé s'il est déjà le début d'un raccourci (revue du 06/11/2026 : un leader
        // sur Espace aurait pris la sélection) ; on est strict, plutôt que de faire
        // attendre cette action.
        let (plain, _) = self.gui.keymap().with_overrides(&self.keys);
        let leader_actions: Vec<_> = defaults.leader_actions().collect();
        let taken = |spec: &str| -> Result<(), String> {
            let chord = crate::actions::parse_keys(spec)?;
            match (chord.as_slice(), plain.first_use(chord[0], &leader_actions)) {
                ([_], None) => Ok(()),
                ([_], Some(action)) => {
                    Err(t!("keys-leader-taken", key = spec, action = action.label()))
                }
                _ => Err(t!("keys-unknown-key", text = spec)),
            }
        };
        let mut leader = Some(defaults.prefix());
        let mine = self.gui.leader.trim();
        if !mine.is_empty() {
            match taken(mine) {
                Ok(()) => leader = Some(mine),
                Err(e) => warnings.push(t!("keys-error", id = "gui.leader", error = e)),
            }
        }
        if leader == Some(defaults.prefix())
            && let Err(e) = taken(defaults.prefix())
        {
            warnings.push(t!("keys-error", id = "leader", error = e));
            leader = None;
        }
        let keymap = match leader {
            Some(l) => self
                .gui
                .keymap()
                .with_leader(l)
                .unwrap_or_else(|_| self.gui.keymap()),
            None => self.gui.keymap(),
        };
        let (keymap, more) = keymap.with_overrides(&self.keys);
        warnings.extend(more);
        // Touche de loupe : celle de `[gui.zoom] loupe_key`, sinon celle de `keys.toml` ;
        // refusée si elle est déjà liée à une action (les deux se gêneraient).
        let free = |loupe: LoupeKey| match loupe {
            LoupeKey::Key(key) if keymap.binds(key) => {
                Err(t!("loupe-key-taken", key = crate::actions::key_label(key)))
            }
            _ => Ok(loupe),
        };
        let mut loupe = defaults.loupe();
        if self.gui.zoom.loupe_key != LoupeKey::Default {
            match free(self.gui.zoom.loupe_key) {
                Ok(mine) => loupe = mine,
                Err(e) => warnings.push(e),
            }
        }
        let loupe = match free(loupe) {
            Ok(l) => l,
            Err(e) => {
                warnings.push(e);
                LoupeKey::None
            }
        };
        (keymap.with_loupe(loupe), warnings)
    }
}

/// Section `[gui]`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GuiSettings {
    /// Thème de l'interface.
    pub theme: Theme,
    /// Jeu de raccourcis : classique (flèches), vim (hjkl), ou les deux.
    pub profile: Profile,
    /// Leader du changement de panneau, comme le préfixe de tmux : puis `h` / `l` (ou les
    /// flèches) pour le panneau de gauche / de droite, `o` pour le suivant. Vide : celui
    /// de `keys.toml` (« Ctrl+b »).
    pub leader: String,
    /// Délai maximal entre deux touches d'une séquence (`g g`), en ms, comme le
    /// `timeoutlen` de vim.
    pub key_sequence_timeout_ms: u64,
    /// Touche qui est un raccourci et le début d'un autre (`y` et `y w`) : délai d'attente
    /// de la suite (ms) avant d'agir seule.
    pub key_ambiguous_timeout_ms: u64,
    /// Menu des suites après le début d'une séquence (comme which-key dans nvim).
    pub key_menu: bool,
    /// Délai avant d'afficher le menu des suites (ms).
    pub key_menu_delay_ms: u64,
    /// Trace (`TRI_GUI_TRACE`) : une image de l'interface plus lente que ce délai (ms)
    /// est tracée avec la durée de ses étapes.
    pub trace_slow_ms: u64,
    /// Flèches des noms de touches (←, →…) : vide, une police du système qui les a ;
    /// un chemin, cette police ; `"none"`, en toutes lettres.
    pub arrow_font: String,
    /// Taille de ces flèches, en plus de l'échelle propre à chaque police du système.
    pub arrow_font_scale: f32,
    /// Taille initiale de la fenêtre (points).
    pub window: [f32; 2],
    /// Rendus RAW : à la demande (image affichée et voisines, en mode RAW) ou tout le
    /// dossier en fond.
    pub raw_previews: RawPreviews,
    /// Temps maximal passé à charger des vignettes, et à intégrer les résultats du moteur,
    /// par image affichée (ms) : au-delà, on reprend à l'image suivante pour ne pas figer
    /// l'interface.
    pub load_budget_ms: u64,
    /// Largeur des barres de progression de la barre d'état (points).
    pub progress_width: f32,
    /// Masquer les barres de progression terminées (la barre d'état reste courte).
    pub progress_hide_done: bool,
    /// Hauteur de la fine barre des tâches de fond repliée (points), grise et discrète
    /// (retour du 25/10/2026) ; le détail lisible s'ouvre d'un clic.
    pub tasks_folded_height: f32,
    /// Délai avant d'afficher les barres de progression après un refiltrage (ms) : le
    /// cache répond souvent tout de suite, une barre montrée 1/10 s n'est pas lisible
    /// (retour du 21/10/2026).
    pub progress_delay_ms: u64,
    /// Filtres sur l'analyse (netteté, exposition) pendant son calcul : la grille est
    /// refiltrée au plus toutes les `analysis_refilter_ms` (ms) avec les analyses déjà
    /// arrivées (issue n° 73), puis à la fin ; 0 : à la fin seulement.
    pub analysis_refilter_ms: u64,
    /// Côté des boutons à icône du bandeau de notation (avance automatique, image
    /// précédente / suivante), en points.
    pub bar_icon_size: f32,
    /// Barre de filtres affichée au démarrage (Maj+F l'affiche ou la masque).
    pub filter_bar: bool,
    /// Sans dossier en argument, rouvrir le dernier dossier ouvert.
    pub reopen_last: bool,
    /// Une image en argument (`tri-gui <image>`) : ouverte en mode concentration (F11),
    /// comme dans une visionneuse (demandé le 31/10/2026) ; `--focus` le force toujours.
    pub image_focus: bool,
    /// Ordre des images au premier lancement (`f s` le change, retenu ensuite) : par nom,
    /// date de prise de vue ou de modification.
    pub sort: tri::sort::SortKey,
    /// Ordre décroissant (`f r`).
    pub sort_descending: bool,
    /// Garder d'un lancement à l'autre ce qu'on affiche ou masque d'une touche (collimateur,
    /// carte de netteté, zones, histogramme, panneaux, barres, avance automatique) ; les
    /// valeurs de la configuration ne servent qu'au premier lancement. La fenêtre aussi
    /// (taille, position, agrandie ; issue n° 58) : `window` ne sert alors qu'au premier.
    pub remember_display: bool,
    /// Garder les filtres et le tri de chaque dossier d'un lancement à l'autre (demandé le
    /// 30/10/2026) ; `false` : chaque dossier s'ouvre sans filtre, dans l'ordre retenu
    /// pour tous.
    pub remember_folder_view: bool,
    /// Fenêtre déplacée ou redimensionnée : retenue une fois immobile depuis ce délai (ms),
    /// pas à chaque image du mouvement (et toujours à la fermeture).
    pub window_save_ms: u64,
    /// Bandeau de notation à la souris affiché au démarrage (Maj+N l'affiche ou le masque).
    pub rating_bar: bool,
    /// Barre du haut : icône et nom de l'application à gauche (comme darktable).
    pub title_bar: bool,
    /// Barre du haut : taille du nom (points ; l'icône un peu plus grande).
    pub title_bar_size: f32,
    /// Mode concentration : durée de l'indication « F11 pour en sortir » (ms).
    pub focus_mode_hint_ms: u64,
    /// Après une note au clavier ou au bandeau, passer à l'image suivante (Maj+A l'active
    /// ou le désactive).
    pub auto_advance: bool,
    /// Annuler : nombre de pas gardés dans l'historique de la session (les plus anciens
    /// sont oubliés).
    pub history: usize,
    /// `[gui.grid]`.
    pub grid: GridSettings,
    /// `[gui.view]`.
    pub view: ViewSettings,
    /// `[gui.zoom]`.
    pub zoom: ZoomSettings,
    /// `[gui.compare]`.
    pub compare: CompareSettings,
    /// `[gui.position]`.
    pub position: PositionSettings,
    /// `[gui.notes]`.
    pub notes: NotesSettings,
    /// `[gui.end]`.
    pub end: EndSettings,
    /// `[gui.dialogs]`.
    pub dialogs: DialogSettings,
    /// `[gui.colors]`.
    pub colors: Colors,
    /// `[gui.folders]`.
    pub folders: FolderSettings,
    /// `[gui.histogram]`.
    pub histogram: HistogramSettings,
    /// `[gui.metadata]`.
    pub metadata: MetadataSettings,
    /// `[gui.sharpness]`.
    pub sharpness: SharpnessDisplay,
    /// `[gui.crop]`.
    pub crop: CropSettings,
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            // Fond sombre et neutre : la norme pour juger des photos.
            theme: Theme::Dark,
            profile: Profile::Classic,
            leader: String::new(),
            key_sequence_timeout_ms: 1000,
            key_ambiguous_timeout_ms: 300,
            key_menu: true,
            key_menu_delay_ms: 400,
            trace_slow_ms: 100,
            arrow_font: String::new(),
            arrow_font_scale: 1.0,
            window: [1400.0, 900.0],
            // En mode JPEG boîtier, la machine reste libre.
            raw_previews: RawPreviews::OnDemand,
            load_budget_ms: 8,
            progress_width: 160.0,
            progress_hide_done: true,
            tasks_folded_height: 4.0,
            progress_delay_ms: 400,
            analysis_refilter_ms: 5000,
            bar_icon_size: 24.0,
            filter_bar: true,
            reopen_last: true,
            image_focus: true,
            sort: tri::sort::SortKey::Name,
            sort_descending: false,
            remember_display: true,
            remember_folder_view: true,
            window_save_ms: 1000,
            rating_bar: true,
            title_bar: true,
            title_bar_size: 26.0,
            focus_mode_hint_ms: 3000,
            auto_advance: false,
            history: 200,
            grid: GridSettings::default(),
            view: ViewSettings::default(),
            zoom: ZoomSettings::default(),
            compare: CompareSettings::default(),
            position: PositionSettings::default(),
            notes: NotesSettings::default(),
            end: EndSettings::default(),
            dialogs: DialogSettings::default(),
            colors: Colors::default(),
            folders: FolderSettings::default(),
            histogram: HistogramSettings::default(),
            metadata: MetadataSettings::default(),
            sharpness: SharpnessDisplay::default(),
            crop: CropSettings::default(),
        }
    }
}

impl GuiSettings {
    /// Couleur des titres : `[gui.colors] title`, ou l'ancienne
    /// `[gui.dialogs] title_color` d'un fichier qui ne règle que celle-là.
    pub fn title_color(&self) -> Color {
        match self.dialogs.legacy_title_color {
            Some(old) if self.colors.title == Colors::default().title => old,
            _ => self.colors.title,
        }
    }

    /// `[gui.dialogs]` tel que les boîtes le lisent (couleur des titres comprise).
    pub fn dialogs(&self) -> DialogSettings {
        DialogSettings {
            title_color: self.title_color(),
            ..self.dialogs
        }
    }

    /// Table des raccourcis des profils choisis.
    pub fn keymap(&self) -> Keymap {
        Keymap::from_profiles(&[self.profile])
    }

    /// Budget de chargement des vignettes par image affichée.
    pub fn load_budget(&self) -> Duration {
        Duration::from_millis(self.load_budget_ms)
    }

    /// Délai maximal entre deux touches d'une séquence.
    pub fn key_sequence_timeout(&self) -> Duration {
        Duration::from_millis(self.key_sequence_timeout_ms)
    }

    /// Séquenceur des raccourcis, avec ses deux délais.
    pub fn sequencer(&self) -> crate::actions::Sequencer {
        crate::actions::Sequencer::new(self.key_sequence_timeout())
            .with_ambiguous(Duration::from_millis(self.key_ambiguous_timeout_ms))
    }
}

/// Thème de l'interface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Sombre.
    #[default]
    Dark,
    /// Clair.
    Light,
    /// Celui du système.
    System,
    /// Gris neutre (idée de l'utilisateur, 19/10/2026, façon Lightroom) : panneaux gris
    /// foncé, fond central gris neutre, qui ne fausse pas le jugement des images comme un
    /// fond noir (plus claires et plus contrastées qu'elles ne sont). Couleurs :
    /// `[gui.colors] neutral_*`.
    Neutral,
}

impl Theme {
    /// Applique le thème à `ctx` : le gris neutre retouche les fonds du thème sombre
    /// d'egui ; les autres thèmes les remettent d'origine. Les messages d'erreur d'egui
    /// prennent `[gui.colors] error`, dans les deux thèmes (revue du 28/10/2026 : certains
    /// gardaient un rouge codé en dur).
    pub fn apply(self, ctx: &egui::Context, colors: &Colors) {
        ctx.set_theme(self);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            ctx.style_mut_of(theme, |style| {
                style.visuals.error_fg_color = colors.error.into();
            });
        }
        for (theme, accent) in [
            (egui::Theme::Dark, colors.accent),
            (egui::Theme::Light, colors.accent_light),
        ] {
            ctx.style_mut_of(theme, |style| {
                style.visuals.selection = accent_selection(accent.into());
            });
        }
        let dark = egui::Visuals::dark();
        ctx.style_mut_of(egui::Theme::Dark, |style| {
            let v = &mut style.visuals;
            if self == Theme::Neutral {
                v.panel_fill = colors.neutral_panel.into();
                v.window_fill = colors.neutral_panel.into();
                v.extreme_bg_color = colors.neutral_field.into();
            } else {
                v.panel_fill = dark.panel_fill;
                v.window_fill = dark.window_fill;
                v.extreme_bg_color = dark.extreme_bg_color;
            }
        });
    }

    /// Fond central (grille, plein écran) : gris neutre pour ce thème, sinon celui du
    /// thème.
    pub fn center(self, colors: &Colors) -> Option<Color32> {
        (self == Theme::Neutral).then(|| colors.neutral_center.into())
    }

    /// Fond d'une cellule de la grille : `neutral_cell` pour ce thème, sinon le fond des
    /// champs du thème (comme avant).
    pub fn cell(self, colors: &Colors, visuals: &egui::Visuals) -> Color32 {
        if self == Theme::Neutral {
            colors.neutral_cell.into()
        } else {
            visuals.extreme_bg_color
        }
    }

    /// Couleur des marques de développement (pastille « n dév. », icône « replier », cadre
    /// d'une pile dépliée) : `development_mark`, un peu plus sombre en thème clair (retour
    /// du 31/10/2026 : le bleu clair distrayait, un gris foncé adapté au thème).
    pub fn development_mark(self, colors: &Colors, visuals: &egui::Visuals) -> Color32 {
        let mark = Color32::from(colors.development_mark);
        if visuals.dark_mode {
            mark
        } else {
            mark.lerp_to_gamma(Color32::BLACK, 0.2)
        }
    }

    /// Couleur de sélection (fond de la cellule, cadre de l'image active en comparaison) :
    /// `selection` si elle est donnée, sinon `neutral_selection` pour ce thème, sinon
    /// celle du thème.
    pub fn selection(self, colors: &Colors, visuals: &egui::Visuals) -> Color32 {
        match (colors.selection, self) {
            (Some(c), _) => c.into(),
            (None, Theme::Neutral) => colors.neutral_selection.into(),
            (None, _) => visuals.selection.bg_fill,
        }
    }

    /// Nom d'une image sur une cellule sélectionnée : sombre sur le gris clair de ce
    /// thème (sans `selection` donnée), sinon `text`, à moins qu'il ne se perde sur le fond
    /// (accent ou `selection` réglés clairs dans un thème sombre, ou l'inverse) : alors un
    /// gris très foncé ou très clair.
    pub fn selected_text(self, colors: &Colors, visuals: &egui::Visuals, text: Color32) -> Color32 {
        if self == Theme::Neutral && colors.selection.is_none() {
            return colors.neutral_cell_text.into();
        }
        let fill = self.selection(colors, visuals);
        match (is_light(fill), is_light(text)) {
            (true, true) => Color32::from_gray(24),
            (false, false) => Color32::from_gray(235),
            _ => text,
        }
    }

    /// Nom d'une image sur une cellule non sélectionnée : `neutral_cell_text` pour ce
    /// thème, sinon `text` (la couleur de texte du thème).
    pub fn cell_text(self, colors: &Colors, text: Color32) -> Color32 {
        if self == Theme::Neutral {
            colors.neutral_cell_text.into()
        } else {
            text
        }
    }

    /// Fond du plein écran, de la comparaison et de la loupe : le gris central pour ce
    /// thème, sinon `view_background`.
    pub fn view_background(self, colors: &Colors) -> Color32 {
        self.center(colors).unwrap_or(colors.view_background.into())
    }

    /// Fonds des piles (un groupe sur deux, l'autre) : accordés au thème.
    pub fn stack_bands(self, colors: &Colors) -> (Color32, Color32) {
        if self == Theme::Neutral {
            (
                colors.neutral_stack_band.into(),
                colors.neutral_stack_band_alt.into(),
            )
        } else {
            (colors.stack_band.into(), colors.stack_band_alt.into())
        }
    }
}

/// Vrai pour une couleur claire (luminance perçue au-dessus du milieu) : un texte posé
/// dessus doit être sombre, et sur fond blanc elle se voit mal.
pub fn is_light(color: Color32) -> bool {
    let [r, g, b, _] = color.to_array();
    0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b) > 128.0
}

/// Sélection d'egui pour une couleur d'accent. Le texte posé dessus est l'accent très
/// éclairci sur un accent sombre, très assombri sur un accent clair : lisible sur un jaune
/// vif, et de la même teinte (les défauts restent proches du bleu pâle et du bleu foncé
/// d'egui). Ailleurs, cette variante sert d'accent visible sur le fond
/// (`marks::accent_bright`).
fn accent_selection(accent: Color32) -> egui::style::Selection {
    let text = if is_light(accent) {
        accent.lerp_to_gamma(Color32::BLACK, 0.6)
    } else {
        accent.lerp_to_gamma(Color32::WHITE, 0.75)
    };
    egui::style::Selection {
        bg_fill: accent,
        stroke: egui::Stroke::new(1.0, text),
    }
}

impl From<Theme> for egui::ThemePreference {
    fn from(theme: Theme) -> Self {
        match theme {
            Theme::Dark => egui::ThemePreference::Dark,
            Theme::Light => egui::ThemePreference::Light,
            Theme::System => egui::ThemePreference::System,
            Theme::Neutral => egui::ThemePreference::Dark,
        }
    }
}

/// Section `[gui.grid]` : dimensions en points.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GridSettings {
    /// Côté d'une cellule au premier lancement et au double-clic sur le curseur de taille
    /// (la taille choisie est ensuite retenue, `state.rs`).
    pub cell: f32,
    /// Bornes du curseur de taille des vignettes.
    pub cell_min: f32,
    pub cell_max: f32,
    /// Côté de cellule (points) sous lequel les infos des vignettes (nom, format, notes)
    /// sont masquées d'elles-mêmes, sans toucher au réglage : trop petites pour être
    /// lisibles (demandé le 31/10/2026), elles reviennent en agrandissant.
    pub info_min_cell: f32,
    /// Espace entre les cellules.
    pub gap: f32,
    /// Infos sur les vignettes (nom, format, notes, badges) au démarrage (Maj+T).
    pub info: bool,
    /// Hauteur réservée au nom du fichier sous la vignette.
    pub label: f32,
    /// Marge intérieure de la cellule.
    pub padding: f32,
    /// Longueur maximale du nom affiché (caractères, « … » au-delà).
    pub name_chars: usize,
    /// Vignettes gardées en mémoire graphique au plus.
    pub textures: usize,
    /// Taille des notes (étoiles, couleurs, drapeau) sur les vignettes.
    pub marks: f32,
    /// Marques vides dessinées en gris clair là où rien n'est posé (une étoile, une
    /// pastille de couleur, le cercle du drapeau neutre) : chaque vignette montre toutes
    /// ses informations et où cliquer (demandé le 31/10/2026).
    pub empty_marks: bool,
    /// Opacité de la vignette d'une photo rejetée (0 : invisible, 1 : normale).
    pub rejected_opacity: f32,
    /// Épaisseur du cadre de la vignette survolée par la souris.
    pub hover_frame: f32,
    /// Épaisseur du cadre des vignettes sélectionnées (sélection multiple).
    pub marked_frame: f32,
    /// Goutte sous les vignettes des photos jugées floues (score de netteté).
    pub blur_badge: bool,
    /// Largeur du curseur de sévérité du flou de la barre du bas du plein écran, avec la
    /// carte de netteté (points). Dans le panneau de droite, les curseurs prennent sa
    /// largeur.
    pub blur_slider: f32,
    /// Largeur du curseur de taille des vignettes, en bas de la grille (points).
    pub size_slider: f32,
    /// Libellés « NOTE » et « FILTRE » des barres du bas et du haut : taille par rapport
    /// au texte.
    pub bar_label_scale: f32,
    /// Bulle du nombre d'images au milieu de l'écran après un changement de filtre :
    /// durée (millisecondes ; 0 : jamais).
    pub filter_toast_ms: u64,
    /// Bandeau « n développements à refaire » (développés modifiés ailleurs, rendus de
    /// nouveau en fond) : durée à l'écran au départ, puis à la fin (millisecondes ; 0 :
    /// jamais). La progression reste dans la barre d'état.
    pub develop_toast_ms: u64,
    /// Piles affichées au démarrage (sinon chaque image seule ; Ctrl+S bascule). Les
    /// empreintes se calculent en fond quand même (`[similarity] background`) : la
    /// bascule est instantanée.
    pub stacks: bool,
    /// Champ de recherche par nom de la barre des filtres (`/`, Ctrl+F) : largeur (points).
    pub search_width: f32,
    /// Largeur minimale de ce champ quand la barre manque de place (le tri et le nombre
    /// d'images passent alors dans un menu « … », issue n° 71).
    pub search_min_width: f32,
    /// Rendu des vignettes (#114) : JPEG boîtier, dernier développement, un logiciel, ou
    /// LibRaw ; un développement absent retombe sur le rendu LibRaw. Changé à la volée
    /// dans la barre des filtres (`f d`), retenu d'une session à l'autre.
    pub render: crate::grid_render::GridRender,
    /// Développements que la grille montre mais pas encore rendus : en fond (`auto`), ou
    /// seulement sur demande (`manual`, Ctrl+Maj+B).
    pub develop: crate::grid_render::GridDevelop,
    /// Rendu d'une vignette attendu (développement en cours) : relecture du cache au plus
    /// une fois par cet intervalle (ms).
    pub render_poll_ms: u64,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            cell: 200.0,
            cell_min: 60.0,
            cell_max: 600.0,
            info_min_cell: 110.0,
            gap: 8.0,
            info: true,
            label: 18.0,
            padding: 6.0,
            name_chars: 24,
            textures: 600,
            marks: 12.0,
            empty_marks: true,
            rejected_opacity: 0.35,
            hover_frame: 2.0,
            marked_frame: 3.0,
            blur_badge: true,
            blur_slider: 90.0,
            size_slider: 90.0,
            bar_label_scale: 0.85,
            filter_toast_ms: 1500,
            develop_toast_ms: 6000,
            stacks: false,
            search_width: 200.0,
            search_min_width: 80.0,
            render: crate::grid_render::GridRender::Embedded,
            develop: crate::grid_render::GridDevelop::Auto,
            render_poll_ms: 1000,
        }
    }
}

/// Section `[gui.dialogs]` : boîtes ouvertes près du clic (notation, éjection,
/// déchargement, renommage, corbeille). Elles se déplacent en glissant leur fond.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DialogSettings {
    /// Marge intérieure (points).
    pub margin: f32,
    /// Espace entre les éléments (points).
    pub spacing: f32,
    /// Marge intérieure des boutons, horizontale (points ; verticale : moitié).
    pub button_padding: f32,
    /// Largeur minimale (points).
    pub min_width: f32,
    /// Largeur maximale (points) : les textes longs passent à la ligne.
    pub max_width: f32,
    /// Taille du texte des titres (points).
    pub title_size: f32,
    /// Couleur des titres, tirée de `[gui.colors] title` à chaque image (pour les boîtes,
    /// qui ne lisent que cette section) ; pas une clé de cette section.
    #[serde(skip)]
    pub title_color: Color,
    /// Ancienne place de `[gui.colors] title` (jusqu'au 05/11/2026), encore lue : un
    /// fichier qui la donne reste valide et garde sa couleur (voir
    /// [`GuiSettings::title_color`]).
    #[serde(rename = "title_color")]
    pub legacy_title_color: Option<Color>,
    /// Fond du panneau qui a le clavier : part de la couleur du texte mêlée au fond des
    /// panneaux (0 : aucune différence, 1 : couleur du texte).
    pub focus_panel_tint: f32,
    /// Ligne pointée au clavier (panneaux, listes) : une bande de la couleur du texte à
    /// cette opacité (0 à 1), sur toute la largeur.
    pub cursor_tint: f32,
    /// Trait en haut du panneau qui a le clavier (#77) : `"accent"` (couleur d'accent du
    /// thème) ou `"#rrggbb"`.
    pub focus_line: ThemeColor,
    /// Épaisseur de ce trait (points) ; 0 : aucun.
    pub focus_line_width: f32,
    /// Côté des boutons à icône (actions sur le fichier de la boîte du clic droit).
    pub icon_size: f32,
    /// Largeur minimale de l'aide (F1), points : ses lignes ne débordent pas.
    pub help_width: f32,
    /// Défilement de l'aide par flèche haut / bas (points).
    pub help_scroll_line: f32,
    /// Aide « ici » (`?`) : taille du texte (points), en grand pour se lire d'un coup d'œil.
    pub help_here_text: f32,
    /// Aide « ici » : largeur d'une colonne de cartes (points).
    pub help_here_column: f32,
    /// Palette de commandes : largeur (points).
    pub palette_width: f32,
    /// Palette de commandes : lignes montrées au plus ; 0 : toutes.
    pub palette_max: usize,
    /// Palette de commandes : préfixe d'une recherche d'actions seulement (`>`, comme
    /// dans VS Code) : un caractère ; vide : aucun.
    #[serde(deserialize_with = "one_char")]
    pub palette_actions_prefix: Option<char>,
    /// Palette de commandes : préfixe d'une recherche de réglages seulement (`?`) : un
    /// caractère ; vide : aucun.
    #[serde(deserialize_with = "one_char")]
    pub palette_settings_prefix: Option<char>,
    /// Pendant une attente (copie, recherche sur le réseau, touche de séquence…),
    /// intervalle entre deux images pour voir où elle en est (ms).
    pub poll_ms: u64,
    /// Fichiers en erreur listés à la fin d'un déchargement.
    pub max_errors: usize,
    /// Dossiers proposés sous un champ de chemin, après Tab.
    pub completions: usize,
}

impl DialogSettings {
    /// Préfixes de la palette de commandes.
    pub fn palette_prefixes(&self) -> crate::palette::Prefixes {
        crate::palette::Prefixes {
            actions: self.palette_actions_prefix,
            settings: self.palette_settings_prefix,
        }
    }
}

/// Un caractère écrit en chaîne (`">"`) ; chaîne vide : aucun.
fn one_char<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<char>, D::Error> {
    let text = String::deserialize(d)?;
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (None, _) => Ok(None),
        (Some(c), None) => Ok(Some(c)),
        _ => Err(serde::de::Error::custom(format!(
            "un seul caractère attendu, pas « {text} »"
        ))),
    }
}

impl Default for DialogSettings {
    fn default() -> Self {
        Self {
            margin: 18.0,
            spacing: 10.0,
            button_padding: 10.0,
            min_width: 340.0,
            max_width: 520.0,
            title_size: 17.0,
            title_color: Colors::default().title,
            legacy_title_color: None,
            focus_panel_tint: 0.06,
            cursor_tint: 0.12,
            focus_line: ThemeColor::Accent,
            focus_line_width: 2.0,
            icon_size: 34.0,
            help_width: 620.0,
            help_scroll_line: 40.0,
            help_here_text: 16.0,
            help_here_column: 420.0,
            palette_width: 640.0,
            palette_max: 0,
            palette_actions_prefix: Some('>'),
            palette_settings_prefix: Some('?'),
            poll_ms: 100,
            max_errors: 5,
            completions: 12,
        }
    }
}

/// Section `[gui.crop]` : recadrage (versions, `r` en plein écran).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CropSettings {
    /// Pas du clavier (déplacer, agrandir), en proportion de l'image.
    pub step: f64,
    /// Couleur du voile autour du cadre.
    pub shade: Color,
    /// Opacité du voile (0 à 1).
    pub shade_opacity: f32,
    /// Épaisseur du cadre (points).
    pub line_width: f32,
    /// Côté des poignées (points).
    pub handle_size: f32,
    /// Distance de prise d'un bord ou d'un angle (points).
    pub handle_grab: f32,
}

impl Default for CropSettings {
    fn default() -> Self {
        Self {
            step: 0.01,
            shade: Color([40, 40, 40]),
            shade_opacity: 0.65,
            line_width: 1.5,
            handle_size: 9.0,
            handle_grab: 10.0,
        }
    }
}

/// Section `[gui.position]` : repère « où on en est » à chaque changement d'image (idée
/// de l'utilisateur, 04/11/2026 : en parcourant les piles, on se perdait), « Photo 12 /
/// 94 » ou « Pile 3 / 20 · 2 / 5 », en grille et en plein écran.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PositionSettings {
    /// Durée d'affichage (secondes) ; 0 : jamais ; négatif (-1) : toujours.
    pub seconds: f32,
    /// Où, dans la grille ou l'image.
    pub place: PositionPlace,
}

impl Default for PositionSettings {
    fn default() -> Self {
        Self {
            seconds: 2.0,
            place: PositionPlace::TopLeft,
        }
    }
}

/// Section `[gui.notes]` : notes posées sur l'image en plein écran, bandeau du bas affiché
/// ou non (04/11/2026, idée de l'utilisateur : réglables comme le repère de position, pour
/// éviter de les avoir au même coin). Même place que le repère : les notes restent,
/// le repère se met à côté.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NotesSettings {
    /// Durée d'affichage (secondes) après un changement d'image ou de notes ; 0 : jamais ;
    /// négatif (-1) : toujours.
    pub seconds: f32,
    /// Où, dans l'image.
    pub place: PositionPlace,
}

impl Default for NotesSettings {
    fn default() -> Self {
        Self {
            seconds: 2.0,
            place: PositionPlace::BottomLeft,
        }
    }
}

/// Section `[gui.end]` : en plein écran, ← / → au bout de la pile ou de la liste le
/// disent un instant (#307 : le défilement s'arrêtait sans rien dire).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EndSettings {
    /// Durée d'affichage (secondes) ; 0 : jamais.
    pub seconds: f32,
    /// Où, dans l'image.
    pub place: PositionPlace,
}

impl Default for EndSettings {
    fn default() -> Self {
        Self {
            seconds: 1.5,
            place: PositionPlace::Center,
        }
    }
}

/// Place du repère de position. Dans le fichier : `"center"`, `"top-left"`, `"top-right"`,
/// `"bottom-right"` ou `"bottom-left"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PositionPlace {
    /// Au centre.
    Center,
    /// En haut à gauche.
    #[default]
    TopLeft,
    /// En haut à droite.
    TopRight,
    /// En bas à droite.
    BottomRight,
    /// En bas à gauche.
    BottomLeft,
}

impl PositionPlace {
    /// Ancrage egui correspondant.
    pub fn align(self) -> egui::Align2 {
        match self {
            PositionPlace::Center => egui::Align2::CENTER_CENTER,
            PositionPlace::TopLeft => egui::Align2::LEFT_TOP,
            PositionPlace::TopRight => egui::Align2::RIGHT_TOP,
            PositionPlace::BottomRight => egui::Align2::RIGHT_BOTTOM,
            PositionPlace::BottomLeft => egui::Align2::LEFT_BOTTOM,
        }
    }
}

/// Section `[gui.compare]` : comparaison côte à côte (`c`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CompareSettings {
    /// Images comparées au plus : au-delà, les premières de la sélection. Chacune garde
    /// un aperçu, et en 1:1 son 1:1 (~100 Mo de mémoire graphique pour 26 Mpx).
    pub max: usize,
    /// Espace entre les images (points).
    pub gap: f32,
    /// Épaisseur du cadre de l'image active (celle que l'on note).
    pub frame: f32,
}

impl Default for CompareSettings {
    fn default() -> Self {
        Self {
            max: 21,
            gap: 4.0,
            frame: 3.0,
        }
    }
}

/// Touche à maintenir pour la loupe : un modificateur, ou une touche ordinaire (retour du
/// 19/10/2026 : avec Maj, chaque raccourci Maj+… faisait apparaître la loupe ; `t` par
/// défaut, libre dans les deux jeux de raccourcis).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoupeKey {
    /// Maj.
    Shift,
    /// Ctrl.
    Ctrl,
    /// Alt (Option sur macOS).
    Alt,
    /// Pas de loupe.
    None,
    /// Une touche (nom comme dans `[keys]` : « m », « Espace », « F3 »).
    Key(egui::Key),
    /// Celle des raccourcis par défaut (`loupe` de `keys.toml`) : `[gui.zoom] loupe_key`
    /// vide.
    #[default]
    Default,
}

impl LoupeKey {
    /// Lit une touche de loupe écrite comme dans la configuration : « shift », « ctrl »,
    /// « alt », « none », un nom de touche, ou vide (celle de `keys.toml`).
    pub fn parse(name: &str) -> Result<LoupeKey, String> {
        Ok(match name.trim().to_lowercase().as_str() {
            "" => LoupeKey::Default,
            "shift" => LoupeKey::Shift,
            "ctrl" => LoupeKey::Ctrl,
            "alt" => LoupeKey::Alt,
            "none" => LoupeKey::None,
            _ => LoupeKey::Key(
                crate::actions::key_named(name.trim())
                    .ok_or_else(|| t!("loupe-key-unknown", key = name))?,
            ),
        })
    }
}

impl<'de> Deserialize<'de> for LoupeKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let name = String::deserialize(d)?;
        LoupeKey::parse(&name).map_err(serde::de::Error::custom)
    }
}

impl LoupeKey {
    /// Vrai si la touche est maintenue dans `input`.
    pub fn held(self, input: &egui::InputState) -> bool {
        match self {
            LoupeKey::Shift => input.modifiers.shift,
            LoupeKey::Ctrl => input.modifiers.ctrl,
            LoupeKey::Alt => input.modifiers.alt,
            LoupeKey::None | LoupeKey::Default => false,
            LoupeKey::Key(key) => input.key_down(key),
        }
    }
}

/// Section `[gui.zoom]` : zoom continu et loupe (décidé le 18/10/2026).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZoomSettings {
    /// Paliers de `+` / `-` (1,0 = 100 % : un pixel de l'image par pixel de l'écran).
    pub steps: Vec<f32>,
    /// Zoom maximal (4,0 = 400 %).
    pub max: f32,
    /// Zoom minimal (0,1 = 10 %) : sous l'ajusté, l'image plus petite que l'écran.
    pub min: f32,
    /// Loupe : touche à maintenir (en vue ajustée, sous le pointeur).
    pub loupe_key: LoupeKey,
    /// Loupe : délai avant d'apparaître (ms), pour ne pas clignoter quand la touche sert à
    /// un raccourci (Maj+P).
    pub loupe_delay_ms: u64,
    /// Loupe : côté (points).
    pub loupe_size: f32,
    /// Loupe : grossissement (1,0 = 100 %).
    pub loupe_scale: f32,
    /// Loupe au clavier : pas d'un déplacement (touches de navigation), en points de la
    /// loupe (l'image à son échelle).
    pub loupe_step: f32,
    /// Loupe au clavier : grand pas (touches de défilement, avec Maj), en points de la
    /// loupe.
    pub loupe_step_large: f32,
    /// Grossissement de la loupe tapé (`<leader> m 2`) : pause après le dernier chiffre
    /// avant d'appliquer un ou deux chiffres (ms).
    pub loupe_digits_ms: u64,
}

impl Default for ZoomSettings {
    fn default() -> Self {
        Self {
            steps: vec![0.125, 0.25, 0.5, 1.0, 2.0, 4.0],
            max: 4.0,
            min: 0.1,
            loupe_key: LoupeKey::default(),
            loupe_delay_ms: 150,
            loupe_size: 320.0,
            loupe_scale: 1.0,
            loupe_step: 80.0,
            loupe_step_large: 320.0,
            loupe_digits_ms: 600,
        }
    }
}

/// Section `[gui.view]` : plein écran.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ViewSettings {
    /// Temps à passer sur une image avant de lancer son 100 % RAW (ms ; ~4,5 s de calcul,
    /// que LibRaw ne sait pas interrompre) : en enchaînant vite les flèches, les images
    /// seulement traversées ne lancent pas un calcul qui bloquerait ensuite celui de
    /// l'image regardée.
    pub full_raw_dwell_ms: u64,
    /// Grandes textures (aperçus, 1:1) gardées en mémoire : l'image affichée, ses voisines
    /// et quelques images récentes. Un 1:1 de 26 Mpx occupe ~100 Mo de mémoire graphique.
    pub textures: usize,
    /// Déplacement d'une image zoomée par appui sur une touche (Maj + flèches, Maj + hjkl),
    /// en fraction de la largeur (ou hauteur) de la vue.
    pub pan_step: f32,
    /// Taille des notes (étoiles, couleurs, drapeau) en plein écran.
    pub marks: f32,
    /// Collimateur de mise au point affiché au démarrage (`m` l'affiche ou le masque ;
    /// plein écran et comparaison).
    pub focus_point: bool,
    /// Forme du repère du collimateur.
    pub focus_marker: FocusMarker,
    /// Taille du repère du collimateur (points).
    pub focus_marker_size: f32,
    /// Épaisseur du trait du repère (points).
    pub focus_marker_width: f32,
    /// Aide-mémoire des touches en haut à droite du plein écran (F1 ouvre l'aide).
    pub key_hints: bool,
    /// Mode concentration : infos et notes dessinées sur l'image (ailleurs, la barre d'état
    /// et le bandeau les montrent) ; `Ctrl+I` les masque.
    pub image_info: bool,
    /// 1:1 au clavier ou au bouton centré sur le collimateur quand il est connu (sinon au
    /// milieu de l'image).
    pub zoom_on_focus_point: bool,
    /// Mode pile (plein écran) : au bout de la pile, ← / → repartent de l'autre bout
    /// (sinon on s'arrête).
    pub stack_wrap: bool,
    /// Plein écran : ← / → passent par chaque développement d'une photo ; `false` (défaut,
    /// retour du 04/11/2026) : de photo en photo, chacune dans le rendu de la grille (les
    /// autres développements : Maj+W / Maj+Q). La case de la section « Vue » le change
    /// (retenu).
    pub review_developments: bool,
    /// Mode pile (plein écran), avance automatique après une note : au bout de la pile,
    /// passer à la première photo de la pile suivante (sinon on reste ; prime sur
    /// `stack_wrap` pour l'avance automatique).
    pub stack_advance_next: bool,
    /// Mode pile (plein écran) : en entrant dans une pile (↑ / ↓ de pile en pile), sa
    /// couverture ou sa première image (issue n° 62 ; bouton du bandeau, retenu).
    pub stack_enter: StackEnter,
    /// Superpositions (o) : épaisseur des traits (points).
    pub overlay_line_width: f32,
    /// Grilles intégrées (tiers, nombre d'or, diagonales) : opacité des lignes.
    pub overlay_opacity: f32,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            full_raw_dwell_ms: 400,
            textures: 8,
            pan_step: 0.2,
            marks: 18.0,
            focus_point: true,
            focus_marker: FocusMarker::default(),
            focus_marker_size: 28.0,
            focus_marker_width: 2.0,
            key_hints: false,
            image_info: true,
            zoom_on_focus_point: true,
            stack_wrap: false,
            review_developments: false,
            stack_advance_next: false,
            stack_enter: StackEnter::Cover,
            overlay_line_width: 1.0,
            overlay_opacity: 0.6,
        }
    }
}

/// Forme du repère du collimateur. Dans le fichier : `"square"`, `"cross"`, `"circle"` ou
/// `"heart"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FocusMarker {
    /// Carré, comme dans le viseur.
    #[default]
    Square,
    /// Croix.
    Cross,
    /// Cercle.
    Circle,
    /// Cœur (proposé par l'utilisateur, le 17/10/2026).
    Heart,
}

impl ViewSettings {
    /// Attente avant un 100 % RAW.
    pub fn full_raw_dwell(&self) -> Duration {
        Duration::from_millis(self.full_raw_dwell_ms)
    }
}

/// Section `[gui.folders]` : panneau des dossiers, à gauche de la grille.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FolderSettings {
    /// Panneau affiché au démarrage (Maj+D l'affiche ou le masque).
    pub visible: bool,
    /// Largeur initiale (points) ; le bord se tire à la souris.
    pub width: f32,
    /// Retrait par niveau de l'arborescence (points).
    pub indent: f32,
    /// Montrer les dossiers cachés (`.xxx`).
    pub hidden: bool,
    /// Recherche de dossiers (`/`) : niveaux de sous-dossiers parcourus au plus.
    pub search_depth: usize,
    /// Recherche de dossiers : nombre de résultats au plus.
    pub search_results: usize,
    /// Case « sous-dossiers » du panneau : niveaux de sous-dossiers dont les images
    /// s'affichent aussi dans la grille, au plus (en plus du dossier ouvert).
    pub subfolders_depth: usize,
    /// Titres des sections (« Disques amovibles », « Favoris »… et celles du panneau de
    /// droite) : taille par rapport au texte des dossiers.
    pub section_scale: f32,
    /// Écart au-dessus du titre de chaque section des panneaux latéraux (points).
    pub section_gap: f32,
    /// « Ouvrir dans Darktable » : délai (ms) pendant lequel on se fie à la dernière
    /// vérification « déjà ouvert » (Windows ; le menu se redessine à chaque image).
    pub running_check_ms: u64,
    /// Ouverture d'un dossier : délai (ms) pendant lequel l'interface attend sa liste
    /// d'images ; au-delà (NAS, disque lent), elle la lit en fond, avec la progression et
    /// de quoi annuler.
    pub open_wait_ms: u64,
    /// Lecture d'un dossier : une demande du type d'une entrée plus lente que ce délai
    /// (ms ; partage monté par GVFS sous Linux) fait se fier aux noms pour la suite.
    pub type_check_ms: u64,
}

impl Default for FolderSettings {
    fn default() -> Self {
        Self {
            visible: true,
            width: 240.0,
            indent: 14.0,
            hidden: false,
            search_depth: 4,
            search_results: 200,
            subfolders_depth: 3,
            section_scale: 1.25,
            section_gap: 6.0,
            running_check_ms: 1000,
            open_wait_ms: 150,
            type_check_ms: 20,
        }
    }
}

/// Section `[gui.sharpness]` : affichage de la carte de netteté (calcul : `[sharpness]`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SharpnessDisplay {
    /// Carte affichée au démarrage (`p` l'affiche ou la masque ; en 1:1 seulement).
    pub visible: bool,
    /// Couleurs proposées, dans l'ordre de la rotation (`Ctrl+P`) : selon l'image, l'une
    /// ressort mieux que l'autre (une couleur proche de celle du sujet se perd).
    pub colors: Vec<Color>,
    /// Opacité des contours (0 à 1).
    pub opacity: f32,
    /// Pas de la sévérité au clavier (Ctrl+J / Ctrl+K), sur une échelle de -1 à 1.
    pub step: f32,
}

impl Default for SharpnessDisplay {
    fn default() -> Self {
        Self {
            visible: false,
            colors: vec![
                Color([255, 40, 40]),
                Color([40, 255, 80]),
                Color([40, 180, 255]),
            ],
            opacity: 1.0,
            step: 0.1,
        }
    }
}

/// Section `[gui.metadata]` : panneau des métadonnées, à droite.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetadataSettings {
    /// Panneau affiché au démarrage (Maj+M l'affiche ou le masque).
    pub visible: bool,
    /// Largeur initiale (points) ; le bord se tire à la souris.
    pub width: f32,
    /// Histogramme de l'image sélectionnée en haut du panneau, en grille comme en plein
    /// écran (panneau masqué : en surimpression sur l'image en plein écran).
    pub histogram: bool,
    /// Hauteur de cet histogramme (points).
    pub histogram_height: f32,
    /// Pas d'un curseur de sévérité du panneau au clavier (flèches gauche / droite, h /
    /// l), sur sa course de -1 à 1.
    pub slider_step: f32,
    /// Air sous chaque groupe de réglages du panneau, avant le sous-titre du suivant, en
    /// hauteurs de ligne (0,5 : un interligne et demi).
    pub group_gap: f32,
}

impl Default for MetadataSettings {
    fn default() -> Self {
        Self {
            visible: true,
            width: 280.0,
            histogram: true,
            histogram_height: 120.0,
            slider_step: 0.1,
            group_gap: 0.5,
        }
    }
}

/// Image où l'on arrive en entrant dans une pile. Dans le fichier : `"cover"` ou `"first"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StackEnter {
    /// Sa couverture : la préférée, sinon l'acceptée, la mieux notée, la plus nette.
    #[default]
    Cover,
    /// Sa première image (dans l'ordre de la grille).
    First,
}

/// Échelle horizontale de l'histogramme. Dans le fichier : `"ev"` ou `"linear"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistogramScale {
    /// En IL sous la saturation (0 = blanc du capteur, -1, -2…) : la marge avant de
    /// brûler se lit directement, les ombres ne sont pas écrasées à gauche.
    #[default]
    Ev,
    /// Valeurs brutes, du noir au blanc.
    Linear,
}

impl HistogramScale {
    /// L'autre échelle.
    pub fn toggled(self) -> Self {
        match self {
            Self::Ev => Self::Linear,
            Self::Linear => Self::Ev,
        }
    }
}

/// Section `[gui.histogram]` : histogramme et alertes d'exposition.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistogramSettings {
    /// Histogramme affiché au démarrage, en haut du panneau des métadonnées (touche `i`).
    pub visible: bool,
    /// Échelle au démarrage (`Maj+I` bascule).
    pub scale: HistogramScale,
    /// Grand histogramme (`Maj+E`) : part de la largeur de la fenêtre (0,2 à 1).
    pub large: f32,
    /// IL montrés sous la saturation, en échelle IL : 0 (défaut) = plage utile de
    /// l'image (~13,9 IL pour un RAW 14 bits, ~11,7 pour un JPEG) ; positif : plafond.
    pub ev_shown: f32,
    /// Zones saturées / bouchées superposées à l'image au démarrage (touche `o`).
    pub clipping: bool,
    /// Opacité des zones superposées (0 à 1).
    pub clipping_opacity: f32,
    /// Zones saturées colorées par canal (`clip_red`, `clip_green`, `clip_blue`,
    /// mélangées si deux canaux, `clip_over` si les trois) ; sinon `clip_over` pour toute
    /// saturation.
    pub clipping_channels: bool,
    /// Badge « brûlée » sur une vignette au-delà de cette part de pixels saturés (canal
    /// le plus touché, 0 à 1).
    pub badge_over: f64,
    /// Badge « bouchée » au-delà de cette part de pixels bouchés.
    pub badge_under: f64,
    /// Curseurs « brûlé » / « bouché » de la barre des filtres : à fond à droite, seuils
    /// des badges divisés par cette valeur ; vers la gauche, multipliés (tout à gauche, ce
    /// côté n'exclut plus rien).
    pub strictness_span: f64,
    /// Pastilles saturés / bouchés de l'histogramme : allumées au-delà de cette part des
    /// pixels (canal le plus touché, 0 à 1), pas pour quelques pixels morts.
    pub indicator_min: f64,
    /// Nombre de classes de la courbe (sur la largeur de l'histogramme).
    pub classes: usize,
    /// Lissage de la courbe : écart type, en classes (0 : sans lissage).
    pub smoothing: f32,
    /// Intensité des surfaces de la courbe (0 à 1), mélangées en additif : rouge + vert =
    /// jaune, les trois = blanc.
    pub fill_strength: f32,
    /// Statistiques du grand histogramme : part de pixels laissée de chaque côté pour la
    /// plage des ombres aux hautes lumières (0,01 : centiles 1 % et 99 %).
    pub stats_tail: f32,
    /// Grand histogramme : diamètre des pastilles de choix du canal (points).
    pub channel_dot: f32,
    /// Grand histogramme : photo d'analyse (zones saturées / bouchées sur l'image en
    /// gris) affichée à l'ouverture, à droite de l'histogramme et à sa hauteur (`p`
    /// bascule).
    pub analysis: bool,
    /// Fond du bloc de l'image d'analyse : fond de la boîte éclairci de cette part vers
    /// le blanc (0 à 1).
    pub analysis_lighten: f32,
    /// RAW : courbe du JPEG boîtier en trait fin par-dessus celle des données du capteur
    /// (couleur `[gui.colors] hist_camera`), pour voir ce que le boîtier a fait des
    /// hautes lumières et des ombres. Valeur au premier lancement : ensuite, `j` ou le
    /// bouton du grand histogramme la montre ou la masque, et c'est retenu.
    pub camera_jpeg: bool,
    /// Épaisseur de ce trait (points).
    pub camera_width: f32,
}

impl Default for HistogramSettings {
    fn default() -> Self {
        Self {
            visible: true,
            scale: HistogramScale::Ev,
            large: 0.7,
            ev_shown: 0.0,
            clipping: false,
            clipping_opacity: 0.7,
            clipping_channels: true,
            badge_over: 0.005,
            badge_under: 0.05,
            strictness_span: 10.0,
            indicator_min: 0.001,
            classes: 80,
            smoothing: 1.5,
            fill_strength: 0.55,
            stats_tail: 0.01,
            channel_dot: 11.0,
            analysis: false,
            analysis_lighten: 0.06,
            camera_jpeg: true,
            camera_width: 1.5,
        }
    }
}

/// Section `[gui.colors]` : couleurs `"#rrggbb"`. Sélection absente : couleur du thème.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Colors {
    /// Couleur d'accent du thème sombre (et du gris neutre) : sélections, curseurs,
    /// boutons actifs, et tout ce qui « prend l'accent » (étoiles, liseré des
    /// boîtes…). Demandée le 04/11/2026 ; défaut = celle d'egui, rien ne change.
    pub accent: Color,
    /// Couleur d'accent du thème clair (défaut : celle d'egui, plus claire).
    pub accent_light: Color,
    /// Titres (boîtes, sections des panneaux) : turquoise, à part de l'accent pour qui
    /// bricole un thème. Était `[gui.dialogs] title_color` jusqu'au 05/11/2026.
    pub title: Color,
    /// Fond de la cellule sélectionnée (au clic ou au clavier).
    pub selection: Option<Color>,
    /// Barres de progression de la barre d'état ; absent : gris du thème (discret, pas la
    /// couleur d'accent).
    pub progress: Option<Color>,
    /// Cadre de la cellule survolée par la souris (le survol ne sélectionne pas).
    pub hover: Color,
    /// Cadre des images sélectionnées (sélection multiple : Espace, v, Maj+clic…).
    pub marked: Color,
    /// Repère du collimateur de mise au point.
    pub focus: Color,
    /// Grilles intégrées des superpositions (tiers, nombre d'or, diagonales).
    pub overlay: Color,
    /// Goutte des filtres de netteté (barre de filtres).
    pub blur_drop: Color,
    /// Piles actives : fond des images d'un groupe sur deux (gris clair), photo isolée
    /// comprise (un groupe d'une image) ; les autres sur `stack_band_alt` (gris foncé).
    pub stack_band: Color,
    /// Fond de l'autre pile sur deux (voir `stack_band`).
    pub stack_band_alt: Color,
    /// Thème gris neutre : panneaux et boîtes.
    pub neutral_panel: Color,
    /// Thème gris neutre : fond central (grille, plein écran).
    pub neutral_center: Color,
    /// Thème gris neutre : cellules de la grille (un peu plus claires que le fond).
    pub neutral_cell: Color,
    /// Thème gris neutre : champs de saisie (recherche, réglages).
    pub neutral_field: Color,
    /// Thème gris neutre : cellule sélectionnée (gris clair, comme darktable), sauf si
    /// `selection` est donnée.
    pub neutral_selection: Color,
    /// Thème gris neutre : nom des images sur les cellules (sombre : lisible sur un gris
    /// moyen, contrairement au gris du thème sombre).
    pub neutral_cell_text: Color,
    /// Thème gris neutre : fonds des piles, un groupe sur deux et l'autre.
    pub neutral_stack_band: Color,
    /// Voir `neutral_stack_band`.
    pub neutral_stack_band_alt: Color,
    /// Piles repliées : feuilles derrière la couverture et pastille du nombre d'images.
    pub stack_mark: Color,
    /// Développements d'une photo : pastille du nombre (pile repliée) et cadre fin des
    /// vignettes d'une pile dépliée.
    pub development_mark: Color,
    /// Marques vides des vignettes (`[gui.grid] empty_marks`) : contour de l'étoile, de
    /// la pastille et du cercle du neutre quand rien n'est posé.
    pub empty_mark: Color,
    /// Badge « RAW » des cellules ; absent : couleur du nom de l'image.
    pub badge: Option<Color>,
    /// Icône d'une vignette impossible à calculer.
    pub warning: Color,
    /// Messages d'erreur.
    pub error: Color,
    /// Fond du plein écran.
    pub view_background: Color,
    /// Fonds proposés tour à tour (`Ctrl+L`) pour la grille et le plein écran, après celui
    /// du thème ; une couleur égale à celle du thème ou déjà vue est sautée.
    pub backgrounds: Vec<Color>,
    /// Étoiles ; `"accent"` (défaut) : couleur d'accent du thème, comme le reste de
    /// l'interface (retour du 21/10/2026, à la place du jaune).
    pub stars: ThemeColor,
    /// Pastille « acceptée ».
    pub accepted: Color,
    /// Pastille « rejetée ».
    pub rejected: Color,
    /// Couleurs de notation, telles qu'affichées.
    pub red: Color,
    /// Jaune.
    pub yellow: Color,
    /// Vert.
    pub green: Color,
    /// Bleu.
    pub blue: Color,
    /// Violet.
    pub purple: Color,
    /// Histogramme : canal rouge.
    pub hist_red: Color,
    /// Histogramme : canal vert.
    pub hist_green: Color,
    /// Histogramme : canal bleu.
    pub hist_blue: Color,
    /// Histogramme : luminance (grand histogramme, `l`).
    pub hist_luminance: Color,
    /// Histogramme d'un RAW : courbe du JPEG boîtier (`[gui.histogram] camera_jpeg`).
    pub hist_camera: Color,
    /// Zones et badges : saturé dans les trois canaux (brûlé), ou dans un canal
    /// quelconque sans couleur par canal.
    pub clip_over: Color,
    /// Zones : saturé dans le seul canal rouge.
    pub clip_red: Color,
    /// Zones : saturé dans le seul canal vert.
    pub clip_green: Color,
    /// Zones : saturé dans le seul canal bleu.
    pub clip_blue: Color,
    /// Zones et badges : bouché.
    pub clip_under: Color,
    /// Image d'analyse du grand histogramme (en gris) : saturé.
    pub analysis_over: Color,
    /// Image d'analyse du grand histogramme : bouché.
    pub analysis_under: Color,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            accent: Color([0, 92, 128]),
            accent_light: Color([144, 209, 255]),
            title: Color([80, 190, 220]),
            selection: None,
            progress: None,
            hover: Color([180, 180, 180]),
            marked: Color([255, 170, 40]),
            focus: Color([60, 255, 110]),
            overlay: Color([255, 255, 255]),
            blur_drop: Color([110, 170, 230]),
            stack_band: Color([56, 56, 56]),
            stack_band_alt: Color([24, 24, 24]),
            // Relevés sur une capture de darktable (thème gris, 19/10/2026).
            neutral_panel: Color([42, 42, 42]),
            neutral_center: Color([94, 94, 94]),
            neutral_cell: Color([118, 118, 118]),
            neutral_field: Color([62, 62, 62]),
            neutral_cell_text: Color([30, 30, 30]),
            neutral_selection: Color([179, 179, 179]),
            neutral_stack_band: Color([132, 132, 132]),
            neutral_stack_band_alt: Color([106, 106, 106]),
            stack_mark: Color([140, 140, 140]),
            development_mark: Color([88, 88, 88]),
            empty_mark: Color([60, 60, 60]),
            badge: None,
            warning: Color([230, 160, 60]),
            error: Color([230, 90, 90]),
            view_background: Color([12, 12, 12]),
            backgrounds: vec![
                Color([48, 48, 48]),
                Color([0, 0, 0]),
                Color([255, 255, 255]),
            ],
            stars: ThemeColor::Accent,
            accepted: Color([70, 170, 90]),
            rejected: Color([215, 70, 70]),
            red: Color([225, 65, 65]),
            yellow: Color([230, 200, 50]),
            green: Color([70, 180, 80]),
            blue: Color([65, 130, 225]),
            purple: Color([165, 85, 210]),
            hist_red: Color([235, 70, 70]),
            hist_green: Color([80, 200, 90]),
            hist_blue: Color([80, 130, 240]),
            hist_luminance: Color([200, 200, 200]),
            hist_camera: Color([128, 128, 128]),
            clip_over: Color([255, 40, 255]),
            clip_red: Color([255, 50, 50]),
            clip_green: Color([50, 230, 50]),
            clip_blue: Color([50, 120, 255]),
            clip_under: Color([0, 210, 255]),
            analysis_over: Color([255, 220, 0]),
            analysis_under: Color([40, 110, 255]),
        }
    }
}

/// Couleur opaque, écrite `"#rrggbb"` dans le fichier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub [u8; 3]);

impl From<Color> for Color32 {
    fn from(Color([r, g, b]): Color) -> Self {
        Color32::from_rgb(r, g, b)
    }
}

impl std::str::FromStr for Color {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let hex = s
            .strip_prefix('#')
            .filter(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| t!("settings-bad-color", value = s))?;
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).expect("hexadécimal vérifié");
        Ok(Color([byte(0), byte(2), byte(4)]))
    }
}

impl Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let [r, g, b] = self.0;
        serializer.serialize_str(&format!("#{r:02x}{g:02x}{b:02x}"))
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

/// Couleur d'accent du thème (`"accent"`, qui suit le thème) ou couleur fixe
/// (`"#rrggbb"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeColor {
    /// Couleur d'accent du thème.
    Accent,
    /// Couleur fixe.
    Rgb(Color),
}

impl ThemeColor {
    /// La couleur à peindre, `accent` étant celle du thème en cours.
    pub fn resolve(self, accent: Color32) -> Color32 {
        match self {
            ThemeColor::Accent => accent,
            ThemeColor::Rgb(c) => c.into(),
        }
    }
}

impl std::str::FromStr for ThemeColor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        if s.trim().eq_ignore_ascii_case("accent") {
            Ok(ThemeColor::Accent)
        } else {
            s.parse().map(ThemeColor::Rgb)
        }
    }
}

impl Serialize for ThemeColor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ThemeColor::Accent => serializer.serialize_str("accent"),
            ThemeColor::Rgb(c) => c.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le fichier d'exemple documente exactement les valeurs par défaut du code.
    #[test]
    fn example_file_matches_the_defaults() {
        let text = include_str!("../../doc/config.example.toml");
        let parsed: AppSettings = tri::settings::parse(text).unwrap();
        assert_eq!(parsed, AppSettings::default());
    }

    #[test]
    fn empty_file_gives_defaults() {
        let parsed: AppSettings = tri::settings::parse("").unwrap();
        assert_eq!(parsed, AppSettings::default());
    }

    #[test]
    fn a_theme_color_is_accent_or_a_fixed_colour() {
        // #77 : le trait du panneau actif suit l'accent du thème, ou une couleur donnée.
        let parsed: AppSettings = tri::settings::parse(
            r##"
            [gui.dialogs]
            focus_line = "#ff8000"
            "##,
        )
        .unwrap();
        assert_eq!(
            parsed.gui.dialogs.focus_line,
            ThemeColor::Rgb(Color([255, 128, 0]))
        );
        assert_eq!("Accent".parse::<ThemeColor>(), Ok(ThemeColor::Accent));
        assert!("bleu".parse::<ThemeColor>().is_err());
        let accent = Color32::from_rgb(1, 2, 3);
        assert_eq!(ThemeColor::Accent.resolve(accent), accent);
    }

    #[test]
    fn gui_section_overrides_and_engine_sections_pass_through() {
        let parsed: AppSettings = tri::settings::parse(
            r##"
            [preview]
            standard = 3840
            [gui]
            theme = "light"
            profile = "vim"
            raw_previews = "background"
            [gui.view]
            full_raw_dwell_ms = 250
            [gui.colors]
            selection = "#1E6F7F"
            "##,
        )
        .unwrap();
        assert_eq!(parsed.gui.theme, Theme::Light);
        assert_eq!(parsed.gui.profile, Profile::Vim);
        assert_eq!(parsed.gui.raw_previews, RawPreviews::Background);
        assert_eq!(parsed.gui.view.full_raw_dwell(), Duration::from_millis(250));
        assert_eq!(parsed.gui.colors.selection, Some(Color([0x1e, 0x6f, 0x7f])));
        assert_eq!(parsed.gui.grid, GridSettings::default());
        assert_eq!(
            parsed.engine().preview(Some(1000)).sizes.standard,
            3840,
            "section du moteur transmise"
        );
    }

    #[test]
    fn unknown_sections_and_keys_are_reported() {
        for (text, culprit) in [
            ("[previw]\nthumbnail = 300", "previw"),
            ("[gui]\nthme = \"dark\"", "thme"),
            ("[gui.grid]\ncells = 3", "cells"),
        ] {
            let err = tri::settings::parse::<AppSettings>(text).unwrap_err();
            assert!(err.contains(culprit), "{text} : {err}");
        }
    }

    #[test]
    fn bad_values_are_reported() {
        for (text, culprit) in [
            ("[gui]\ntheme = \"sepia\"", "sepia"),
            ("[gui]\nprofile = \"emacs\"", "emacs"),
            ("[gui.colors]\nbadge = \"bleu\"", "#rrggbb"),
            ("[gui.colors]\nbadge = \"#12345\"", "#rrggbb"),
            ("[gui.colors]\nbadge = \"#12345g\"", "#rrggbb"),
        ] {
            let err = tri::settings::parse::<AppSettings>(text).unwrap_err();
            assert!(err.contains(culprit), "{text} : {err}");
        }
    }

    #[test]
    fn keys_section_overrides_profile_keys() {
        let parsed: AppSettings = tri::settings::parse(
            r#"
            [keys]
            "core.flag.accept" = ["g"]
            "core.nope" = ["z"]
            "#,
        )
        .unwrap();
        let (keymap, warnings) = parsed.keymap();
        assert_eq!(
            keymap.action(egui::Key::G, egui::Modifiers::NONE),
            Some(crate::actions::Action::Flag(tri::xmp::Flag::Accepted))
        );
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            AppSettings::default().keymap().1.is_empty(),
            "défauts sans conflit"
        );
    }

    #[test]
    fn error_messages_take_the_configured_colour_in_every_theme() {
        // Revue du 28/10/2026 : des messages d'erreur gardaient un rouge codé en dur.
        let ctx = egui::Context::default();
        let colors = Colors {
            error: Color([1, 2, 3]),
            ..Colors::default()
        };
        Theme::Dark.apply(&ctx, &colors);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            let style = ctx.style_of(theme);
            assert_eq!(style.visuals.error_fg_color, Color32::from_rgb(1, 2, 3));
        }
    }

    #[test]
    fn the_title_colour_moved_to_the_colours_and_the_old_key_still_works() {
        // Réorganisation du 05/11/2026 : [gui.dialogs] title_color → [gui.colors] title.
        let read = |text: &str| -> AppSettings { tri::settings::parse(text).unwrap() };
        let default = read("");
        assert_eq!(default.gui.title_color(), Colors::default().title);
        let new = read("[gui.colors]\ntitle = \"#010203\"\n");
        assert_eq!(new.gui.title_color(), Color([1, 2, 3]));
        assert_eq!(new.gui.dialogs().title_color, Color([1, 2, 3]));
        let old = read("[gui.dialogs]\ntitle_color = \"#040506\"\n");
        assert_eq!(old.gui.title_color(), Color([4, 5, 6]));
        let both =
            read("[gui.dialogs]\ntitle_color = \"#040506\"\n[gui.colors]\ntitle = \"#010203\"\n");
        assert_eq!(both.gui.title_color(), Color([1, 2, 3]));
    }

    #[test]
    fn the_accent_colour_is_the_theme_selection_with_readable_text() {
        // Demandée le 04/11/2026 : l'accent n'était pas réglable.
        let ctx = egui::Context::default();
        let defaults = Colors::default();
        Theme::Dark.apply(&ctx, &defaults);
        // Par défaut, les accents d'egui.
        for (theme, egui) in [
            (egui::Theme::Dark, egui::Visuals::dark()),
            (egui::Theme::Light, egui::Visuals::light()),
        ] {
            let selection = ctx.style_of(theme).visuals.selection;
            assert_eq!(selection.bg_fill, egui.selection.bg_fill);
            assert_eq!(
                is_light(selection.stroke.color),
                is_light(egui.selection.stroke.color)
            );
        }
        let colors = Colors {
            accent: Color([240, 200, 40]),
            accent_light: Color([20, 60, 20]),
            ..Colors::default()
        };
        Theme::Neutral.apply(&ctx, &colors);
        let dark = ctx.style_of(egui::Theme::Dark).visuals.selection;
        assert_eq!(dark.bg_fill, Color32::from_rgb(240, 200, 40));
        assert!(!is_light(dark.stroke.color), "texte sombre sur un jaune");
        let light = ctx.style_of(egui::Theme::Light).visuals.selection;
        assert_eq!(light.bg_fill, Color32::from_rgb(20, 60, 20));
        assert!(
            is_light(light.stroke.color),
            "texte clair sur un vert foncé"
        );
    }

    #[test]
    fn the_neutral_theme_has_its_own_greys() {
        let c = Colors::default();
        assert_eq!(Theme::Dark.center(&c), None);
        assert_eq!(Theme::Neutral.center(&c), Some(c.neutral_center.into()));
        assert_eq!(
            Theme::Neutral.view_background(&c),
            Color32::from(c.neutral_center)
        );
        assert_eq!(
            Theme::Dark.view_background(&c),
            Color32::from(c.view_background)
        );
        assert_eq!(
            Theme::Neutral.stack_bands(&c),
            (c.neutral_stack_band.into(), c.neutral_stack_band_alt.into())
        );
        // Cellules et champs séparés ; noms sombres sur les cellules.
        let v = egui::Visuals::dark();
        assert_eq!(Theme::Neutral.cell(&c, &v), Color32::from(c.neutral_cell));
        assert_eq!(Theme::Dark.cell(&c, &v), v.extreme_bg_color);
        assert_eq!(
            Theme::Neutral.cell_text(&c, Color32::WHITE),
            Color32::from(c.neutral_cell_text)
        );
        assert_eq!(Theme::Dark.cell_text(&c, Color32::WHITE), Color32::WHITE);
        // Sélection gris clair (texte sombre), sauf couleur donnée.
        assert_eq!(
            Theme::Neutral.selection(&c, &v),
            Color32::from(c.neutral_selection)
        );
        assert_eq!(Theme::Dark.selection(&c, &v), v.selection.bg_fill);
        let mut chosen = c.clone();
        chosen.selection = Some(Color([1, 2, 3]));
        assert_eq!(
            Theme::Neutral.selection(&chosen, &v),
            Color32::from_rgb(1, 2, 3)
        );
        assert_eq!(
            Theme::Neutral.selected_text(&chosen, &v, Color32::WHITE),
            Color32::WHITE
        );
        // Un accent clair dans le thème sombre : nom en sombre (05/11/2026).
        let mut yellow = v.clone();
        yellow.selection.bg_fill = Color32::from_rgb(240, 208, 32);
        assert!(!is_light(Theme::Dark.selected_text(
            &c,
            &yellow,
            Color32::WHITE
        )));
        assert_eq!(
            Theme::Dark.selected_text(&c, &v, Color32::WHITE),
            Color32::WHITE
        );
        let parsed: AppSettings = tri::settings::parse("[gui]\ntheme = \"neutral\"\n").unwrap();
        assert_eq!(parsed.gui.theme, Theme::Neutral);
        assert_eq!(
            egui::ThemePreference::from(Theme::Neutral),
            egui::ThemePreference::Dark
        );
    }

    #[test]
    fn the_loupe_key_is_a_modifier_or_any_key() {
        let parse = |v: &str| {
            tri::settings::parse::<AppSettings>(&format!("[gui.zoom]\nloupe_key = \"{v}\"\n"))
                .map(|s| s.gui.zoom.loupe_key)
        };
        assert_eq!(parse("t"), Ok(LoupeKey::Key(egui::Key::T)));
        assert_eq!(
            parse(""),
            Ok(LoupeKey::Default),
            "vide : celle de keys.toml"
        );
        assert_eq!(parse("F3"), Ok(LoupeKey::Key(egui::Key::F3)));
        assert_eq!(parse("shift"), Ok(LoupeKey::Shift));
        assert_eq!(parse("none"), Ok(LoupeKey::None));
        assert!(
            parse("touche-magique")
                .unwrap_err()
                .contains("touche-magique")
        );
        assert_eq!(ZoomSettings::default().loupe_key, LoupeKey::Default);
        // Celle de keys.toml (m), libre ; une touche déjà prise est refusée, et le dit :
        // celle de keys.toml reste.
        let (keymap, warnings) = AppSettings::default().keymap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(keymap.loupe(), LoupeKey::Key(egui::Key::M));
        let mut taken = AppSettings::default();
        taken.gui.zoom.loupe_key = LoupeKey::Key(egui::Key::O);
        let (keymap, warnings) = taken.keymap();
        assert_eq!(warnings.len(), 1, "o : superpositions");
        assert_eq!(keymap.loupe(), LoupeKey::Key(egui::Key::M));
        taken.gui.zoom.loupe_key = LoupeKey::Key(egui::Key::F3);
        assert_eq!(taken.keymap().0.loupe(), LoupeKey::Key(egui::Key::F3));
    }

    #[test]
    fn a_leader_already_bound_to_an_action_is_refused() {
        // Revue du 06/11/2026 : un leader sur Espace aurait pris la sélection. On est
        // strict : refusé, celui de keys.toml reste, et c'est dit.
        use crate::actions::{Action, Chord};
        let mut space = AppSettings::default();
        space.gui.leader = "Space".into();
        let (keymap, warnings) = space.keymap();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("Space") || warnings[0].contains("Espace"));
        assert_eq!(
            keymap.action(egui::Key::Space, egui::Modifiers::NONE),
            Some(Action::SelectToggle),
            "Espace sélectionne toujours, sans attendre"
        );
        // Une fois la sélection (et « actionner » du panneau de droite) déplacées dans
        // [keys], Espace est accepté.
        for id in ["core.select.toggle", "core.metadata.activate"] {
            space.keys.insert(id.into(), vec!["Insert".into()]);
        }
        let (keymap, warnings) = space.keymap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut seq = crate::actions::Sequencer::new(std::time::Duration::from_secs(1));
        let now = std::time::Instant::now();
        let key = |k| Chord::from_event(k, egui::Modifiers::NONE).unwrap();
        assert_eq!(seq.feed_one(&keymap, key(egui::Key::Space), now), None);
        assert_eq!(
            seq.feed_one(&keymap, key(egui::Key::Space), now),
            Some(Action::ToggleAutoAdvance),
            "<leader> Espace : l'avance automatique"
        );
    }

    #[test]
    fn each_profile_has_only_its_own_keys() {
        let keymap = |profile| {
            GuiSettings {
                profile,
                ..GuiSettings::default()
            }
            .keymap()
        };
        let next = |km: &Keymap, k| km.action(k, egui::Modifiers::NONE);
        let (classic, vim, both) = (
            keymap(Profile::Classic),
            keymap(Profile::Vim),
            keymap(Profile::Both),
        );
        // « both » n'existe plus : lu comme le classique (et signalé, voir `keymap`).
        use crate::actions::Action;
        use egui::Key::{ArrowRight, L, X, Y};
        assert_eq!(next(&classic, ArrowRight), Some(Action::NavNext));
        assert_eq!(next(&classic, L), None, "pas de hjkl en classique");
        assert_eq!(next(&classic, X), None);
        assert_eq!(next(&vim, L), Some(Action::NavNext));
        assert_eq!(next(&vim, ArrowRight), None, "pas de flèches en vim");
        assert_eq!(next(&vim, Y), Some(Action::FileCopy));
        assert_eq!(next(&both, ArrowRight), Some(Action::NavNext));
        assert_eq!(next(&both, L), None);
        let mut old = AppSettings::default();
        old.gui.profile = Profile::Both;
        assert_eq!(old.keymap().1.len(), 1, "« both » signalé");
        // Communs : la notation, dans les deux.
        for km in [&classic, &vim] {
            assert_eq!(next(km, egui::Key::Num3), Some(Action::Stars(3)));
        }
        assert_eq!(GuiSettings::default().profile, Profile::Classic);
    }
}
