//! Fichier de configuration : sections du moteur, emplacement, lecture.
//!
//! Format TOML. Toute clé absente prend sa valeur par défaut, celle du code : un fichier
//! vide (ou absent) équivaut aux réglages par défaut. Dans une section, une clé inconnue
//! est une erreur : une faute de frappe est signalée au lieu d'être ignorée en silence.
//! Les sections inconnues du moteur sont laissées à l'interface (`[gui]`), qui lit le même
//! fichier avec son propre type (voir [`load`]).
//!
//! Exemple commenté, avec toutes les valeurs par défaut : `doc/config.example.toml`.
//! Voir `doc/en/settings.md`.

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::{self, DeserializeOwned, Deserializer, Visitor};

use crate::cache::{self, Cache};
use crate::preview::{self, Sizes};
use crate::raw::RenderOptions;
use crate::scheduler::{self, Analysis, RawPreviews};

/// Valeur déterminée à l'exécution (`"auto"` dans le fichier) ou fixée (un entier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Auto<T> {
    /// Déterminée à l'exécution (taille de l'écran, nombre de cœurs...).
    #[default]
    Auto,
    /// Valeur fixée dans le fichier.
    Fixed(T),
}

impl<T> Auto<T> {
    /// La valeur fixée, ou celle que calcule `auto`.
    pub fn resolve(self, auto: impl FnOnce() -> T) -> T {
        match self {
            Auto::Auto => auto(),
            Auto::Fixed(v) => v,
        }
    }
}

/// Accepte la chaîne `"auto"` ou un entier positif qui tient dans `T`, avec un message
/// d'erreur clair dans les autres cas.
impl<'de, T: TryFrom<u64>> Deserialize<'de> for Auto<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AutoVisitor<T>(PhantomData<T>);

        impl<T: TryFrom<u64>> Visitor<'_> for AutoVisitor<T> {
            type Value = Auto<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(&crate::t!("settings-auto-or-integer"))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                if v == "auto" {
                    Ok(Auto::Auto)
                } else {
                    Err(E::invalid_value(de::Unexpected::Str(v), &self))
                }
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                T::try_from(v)
                    .map(Auto::Fixed)
                    .map_err(|_| E::invalid_value(de::Unexpected::Unsigned(v), &self))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                match u64::try_from(v) {
                    Ok(v) => self.visit_u64(v),
                    Err(_) => Err(E::invalid_value(de::Unexpected::Signed(v), &self)),
                }
            }
        }

        deserializer.deserialize_any(AutoVisitor(PhantomData))
    }
}

/// Réglages du moteur lus dans le fichier : sections `[preview]`, `[render]`, `[cache]`,
/// `[scheduler]`, `[xmp]`. Les autres sections (`[gui]`) sont ignorées ici.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct EngineSettings {
    /// Tailles et seuils des aperçus.
    pub preview: PreviewSection,
    /// Développement RAW.
    pub render: RenderSection,
    /// Cache des aperçus.
    pub cache: CacheSection,
    /// Travail en tâche de fond.
    pub scheduler: SchedulerSection,
    /// Analyses de tout le dossier : en fond, à la demande, ou retirées (#564).
    pub analysis: AnalysisSection,
    /// Notes dans les sidecars XMP.
    pub xmp: XmpSection,
    /// Sidecars des logiciels de développement (`.rrdata`) : création quand on note.
    pub sidecars: crate::sidecars::SidecarSettings,
    /// Emplacements proposés pour choisir un dossier.
    pub places: crate::places::PlaceSettings,
    /// Surveillance du dossier ouvert.
    pub watch: crate::watch::WatchSettings,
    /// Mode léger des dossiers sur le réseau ou choisis (#597).
    pub light: crate::light::LightSettings,
    /// Tri des images (ordre des couleurs).
    pub sort: crate::sort::SortSettings,
    /// Rendu par un logiciel de développement (darktable).
    pub develop: crate::develop::DevelopSettings,
    /// Export (profils, destination).
    pub export: crate::export::ExportSettings,
    /// Métadonnées de création à poser dans les images (#277).
    pub iptc: crate::iptc::IptcSettings,
    /// Versions (préréglages de recadrage).
    pub versions: crate::versions::VersionSettings,
    /// Calcul de l'exposition (histogrammes, saturés / bouchés).
    pub exposure: crate::exposure::ExposureSettings,
    /// Déchargement des cartes.
    pub offload: crate::offload::OffloadSettings,
    /// Actions sur les fichiers (renommer, supprimer, éditeur externe).
    pub files: crate::files::FileSettings,
    /// Carte de netteté (focus peaking).
    pub sharpness: crate::sharpness::SharpnessSettings,
    /// Regroupement des images semblables.
    pub similarity: crate::similarity::SimilaritySettings,
    /// Palettes de couleurs (#492), section `[colours]` (renommée le 23/11/2026, comme
    /// « Couleurs » dans l'interface).
    #[serde(rename = "colours")]
    pub palette: crate::palette::PaletteSettings,
    /// Index de recherche des palettes de tous les dossiers (#561).
    pub index: crate::index::IndexSettings,
    /// Langue des textes.
    pub i18n: crate::i18n::I18nSettings,
    /// Recherche d'une nouvelle version (sur demande).
    pub update: crate::update::UpdateSettings,
    /// Copie en local d'un serveur que le système ne sait pas monter.
    pub remote: crate::remote::RemoteSettings,
    /// Voisinage réseau (serveurs annoncés par mDNS).
    pub neighbourhood: crate::neighbourhood::NeighbourhoodSettings,
    /// Dossier des données.
    pub data: DataSection,
    /// Journal de l'application (`tri.log`).
    pub log: crate::log::LogSettings,
    /// Réglages de visualisation.
    pub adjust: crate::adjust::AdjustSettings,
}

/// Section `[xmp]` : notes (étoiles, couleurs, accepté / rejeté) dans les sidecars XMP. Voir
/// [`crate::xmp`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct XmpSection {
    /// Nom d'un **nouveau** sidecar : `"darktable"` (`IMAGE.RAF.xmp`, défaut) ou `"adobe"`
    /// (`IMAGE.xmp`, Lightroom, Capture One). Un sidecar existant est toujours réutilisé (le
    /// plus récent si les deux existent).
    pub sidecar: crate::xmp::Convention,
    /// Plusieurs couleurs par image (comme darktable) ; sinon une seule, une nouvelle
    /// couleur remplaçant la précédente (comme Lightroom).
    pub multiple_colors: bool,
    /// Inscrire aussi les notes dans les images (JPEG, PNG, WebP, TIFF, HEIC) à chaque
    /// note, et y propager une fusion venue d'un autre logiciel : jamais (défaut), sur les
    /// disques internes, ou partout ; un dossier peut avoir son propre réglage. Voir
    /// [`crate::embedded`], [`crate::reconcile`].
    pub embed: crate::embedded::Auto,
    /// Une seule couleur : ordre de lecture des champs qui en portent une (la première
    /// trouvée l'emporte). Voir [`crate::xmp::ColorSources`].
    pub color_sources: crate::xmp::ColorSources,
    /// Champs écrits, par logiciel (`[xmp.fields]`, tous par défaut) : lus et écrits.
    pub fields: crate::xmp::Fields,
}

impl XmpSection {
    /// Règles à donner à [`crate::xmp::set_rules`].
    pub fn rules(&self) -> crate::xmp::Rules {
        crate::xmp::Rules {
            multiple: self.multiple_colors,
            sources: self.color_sources,
            fields: self.fields,
        }
    }
}

/// Section `[preview]` : voir [`preview::Settings`].
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PreviewSection {
    /// Côté long des vignettes (pixels).
    pub thumbnail: u32,
    /// Côté long de l'aperçu standard (pixels) ; « auto » : plus grand côté de l'écran
    /// principal.
    pub standard: Auto<u32>,
    /// Taille « auto » quand l'écran est inconnu (ligne de commande).
    pub auto_fallback: u32,
    /// Voir [`preview::Settings::half_size_margin`].
    pub half_size_margin: f32,
    /// Voir [`preview::Settings::embedded_min_ratio`].
    pub embedded_min_ratio: f32,
    /// Voir [`preview::Settings::min_embedded_full`].
    pub min_embedded_full: u32,
}

impl Default for PreviewSection {
    fn default() -> Self {
        let d = preview::Settings::default();
        Self {
            thumbnail: d.sizes.thumbnail,
            standard: Auto::Auto,
            auto_fallback: d.sizes.standard,
            half_size_margin: d.half_size_margin,
            embedded_min_ratio: d.embedded_min_ratio,
            min_embedded_full: d.min_embedded_full,
        }
    }
}

/// Section `[render]` : voir [`RenderOptions`] (`half_size` n'en fait pas partie, il est
/// choisi par niveau).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RenderSection {
    /// Balance des blancs du boîtier (sinon « lumière du jour »).
    pub camera_wb: bool,
    /// Luminosité (1.0 = défaut LibRaw).
    pub bright: f32,
    /// Luminosité automatique de LibRaw (masque une sous-exposition : désactivée).
    pub auto_bright: bool,
}

impl RenderSection {
    /// Options de rendu LibRaw à pleine résolution (aperçus, 1:1, export).
    pub fn options(&self) -> RenderOptions {
        RenderOptions {
            camera_wb: self.camera_wb,
            half_size: false,
            bright: self.bright,
            auto_bright: self.auto_bright,
            ..RenderOptions::default()
        }
    }
}

impl Default for RenderSection {
    fn default() -> Self {
        let d = RenderOptions::default();
        Self {
            camera_wb: d.camera_wb,
            bright: d.bright,
            auto_bright: d.auto_bright,
        }
    }
}

/// Section `[cache]`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CacheSection {
    /// Dossier du cache ; absent : emplacement par défaut du système
    /// ([`cache::default_dir`]).
    pub dir: Option<PathBuf>,
    /// Qualité JPEG des entrées (1 à 100).
    pub jpeg_quality: u8,
    /// Taille maximale de chaque niveau 1:1 (`Full`, `EmbeddedFull`), en Mo : au-delà,
    /// les entrées les moins récemment utilisées sont supprimées (un 1:1 de 26 Mpx pèse
    /// ~5 Mo). 0 : pas de limite. Les autres niveaux, bien plus légers, ne sont pas
    /// limités.
    pub zoom_max_mb: u64,
    /// Où garder le cache d'un dossier ouvert qui n'en a pas encore : chez l'utilisateur,
    /// sur le disque amovible, dans le dossier ([`cache::Location`]).
    pub location: cache::Location,
}

impl Default for CacheSection {
    fn default() -> Self {
        Self {
            dir: None,
            jpeg_quality: cache::DEFAULT_JPEG_QUALITY,
            zoom_max_mb: 2000,
            location: cache::Location::User,
        }
    }
}

/// Section `[data]` : dossier des données de `tri` (historique des déchargements, dernier
/// dossier ouvert), ni réglages ni cache.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DataSection {
    /// Dossier ; absent : celui du système ([`data_dir`]).
    pub dir: Option<PathBuf>,
}

/// Section `[scheduler]` : voir [`scheduler::Config`].
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SchedulerSection {
    /// Workers légers (vignettes, images plates) ; « auto » : la moitié des cœurs.
    pub light_workers: Auto<usize>,
    /// Vignettes d'abord, puis aperçus boîtier, puis rendus RAW.
    pub thumbnails_first: bool,
    /// Ancienne clé (avant #564) : `false` vaut `[analysis] exposure = "on-demand"`.
    pub exposure: Option<bool>,
}

/// Section `[analysis]` (#564) : voir [`scheduler::Analyses`].
pub type AnalysisSection = scheduler::Analyses;

impl Default for SchedulerSection {
    fn default() -> Self {
        Self {
            light_workers: Auto::Auto,
            thumbnails_first: scheduler::Config::default().thumbnails_first,
            exposure: None,
        }
    }
}

impl EngineSettings {
    /// Réglages de calcul des aperçus. `screen_side` : plus grand côté de l'écran principal
    /// (pixels), pour la taille « auto » ; `None` : `auto_fallback`.
    pub fn preview(&self, screen_side: Option<u32>) -> preview::Settings {
        let p = &self.preview;
        let r = &self.render;
        preview::Settings {
            sizes: Sizes {
                thumbnail: p.thumbnail,
                standard: p
                    .standard
                    .resolve(|| screen_side.unwrap_or(p.auto_fallback)),
            },
            render: r.options(),
            half_size_margin: p.half_size_margin,
            embedded_min_ratio: p.embedded_min_ratio,
            min_embedded_full: p.min_embedded_full,
            exposure: self.exposure,
            sharpness: self.sharpness,
            similarity: self.similarity,
            palette: self.palette,
        }
    }

    /// Réglages du scheduler. La politique des rendus RAW n'est pas dans cette section :
    /// elle dépend de l'usage (l'interface les calcule à la demande, la démo en ligne de
    /// commande en fond), d'où le paramètre.
    pub fn scheduler(&self, raw_previews: RawPreviews) -> scheduler::Config {
        let default = scheduler::Config::default();
        scheduler::Config {
            light_workers: self
                .scheduler
                .light_workers
                .resolve(|| default.light_workers)
                .max(1),
            thumbnails_first: self.scheduler.thumbnails_first,
            raw_previews,
            exposure: self.analysis().exposure,
            sharpness: self.analysis().sharpness,
            similarity: self.analysis().stacks,
            palettes: self.analysis().palettes,
            // Choisie dans l'interface des palettes, ou en ligne de commande.
            palette: None,
        }
    }

    /// Analyses de tout le dossier (`[analysis]`), avec les anciennes clés : `[scheduler]
    /// exposure`, `[sharpness] background`, `[similarity] background` à `false` valent « à
    /// la demande » (avant #564, l'analyse n'était plus faite en fond).
    pub fn analysis(&self) -> AnalysisSection {
        let legacy = |mode: Analysis, old: Option<bool>| match (mode, old) {
            (Analysis::Background, Some(false)) => Analysis::OnDemand,
            _ => mode,
        };
        let a = self.analysis;
        AnalysisSection {
            exposure: legacy(a.exposure, self.scheduler.exposure),
            sharpness: legacy(a.sharpness, self.sharpness.background),
            stacks: legacy(a.stacks, self.similarity.background),
            palettes: a.palettes,
        }
    }

    /// Dossier des données : celui de `[data] dir`, sinon celui du système.
    pub fn data_dir(&self) -> Option<PathBuf> {
        self.data.dir.clone().or_else(data_dir)
    }

    /// Applique les limites de taille du cache (`zoom_max_mb`) : supprime les 1:1 les
    /// moins récemment utilisés. Parcourt les dossiers du cache : à appeler hors du fil de
    /// l'interface. Renvoie l'espace libéré.
    pub fn trim_cache(&self, cache: &Cache) -> io::Result<cache::Usage> {
        let mut freed = cache::Usage::default();
        let max = self.cache.zoom_max_mb;
        if max == 0 {
            return Ok(freed);
        }
        for level in [cache::Level::Full, cache::Level::EmbeddedFull] {
            let u = cache.evict(level, max * 1_000_000)?;
            freed.bytes += u.bytes;
            freed.entries += u.entries;
        }
        Ok(freed)
    }

    /// Ouvre le cache : dossier configuré (sinon celui du système), qualité configurée.
    pub fn open_cache(&self) -> io::Result<Cache> {
        let cache = match &self.cache.dir {
            Some(dir) => Cache::open(dir)?,
            None => Cache::open_default()?,
        };
        Ok(cache.with_quality(self.cache.jpeg_quality))
    }
}

/// Emplacement par défaut du fichier de configuration, selon le système :
/// - Linux et autres Unix : `$XDG_CONFIG_HOME/tri/config.toml` (si absolu), sinon
///   `~/.config/tri/config.toml` ;
/// - macOS : `~/Library/Application Support/tri/config.toml` ;
/// - Windows : `%APPDATA%\tri\config.toml`.
///
/// `None` si les variables d'environnement nécessaires sont absentes.
pub fn default_path() -> Option<PathBuf> {
    default_path_for(std::env::consts::OS, |name| std::env::var_os(name))
}

/// Logique de [`default_path`], avec l'OS et l'environnement injectés (pour les tests).
fn default_path_for(os: &str, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |name: &str| env(name).map(PathBuf::from).filter(|p| p.is_absolute());
    let base = match os {
        "windows" => absolute("APPDATA")?,
        "macos" => absolute("HOME")?
            .join("Library")
            .join("Application Support"),
        _ => absolute("XDG_CONFIG_HOME").or_else(|| Some(absolute("HOME")?.join(".config")))?,
    };
    Some(base.join("tri").join("config.toml"))
}

/// Dossier des données de `tri` (ce qui n'est ni réglage ni cache : historique des
/// déchargements de cartes…), selon le système :
/// - Linux et autres Unix : `$XDG_DATA_HOME/tri` (si absolu), sinon `~/.local/share/tri` ;
/// - macOS : `~/Library/Application Support/tri` ;
/// - Windows : `%APPDATA%\tri`.
///
/// Pas le cache : on peut le vider sans rien perdre, pas ces données.
pub fn data_dir() -> Option<PathBuf> {
    data_dir_for(std::env::consts::OS, |name| std::env::var_os(name))
}

/// Logique de [`data_dir`], avec l'OS et l'environnement injectés (pour les tests).
fn data_dir_for(os: &str, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |name: &str| env(name).map(PathBuf::from).filter(|p| p.is_absolute());
    let base = match os {
        "windows" => absolute("APPDATA")?,
        "macos" => absolute("HOME")?
            .join("Library")
            .join("Application Support"),
        _ => absolute("XDG_DATA_HOME")
            .or_else(|| Some(absolute("HOME")?.join(".local").join("share")))?,
    };
    Some(base.join("tri"))
}

/// Remplace un `~` initial par le dossier personnel (`HOME`, ou `USERPROFILE` sous
/// Windows) : les chemins du fichier de configuration s'écrivent `"~/Pictures"`.
pub fn expand_home(path: &str) -> PathBuf {
    expand_home_with(path, |name| std::env::var_os(name))
}

fn expand_home_with(path: &str, env: impl Fn(&str) -> Option<OsString>) -> PathBuf {
    let home = || {
        env("HOME")
            .or_else(|| env("USERPROFILE"))
            .map(PathBuf::from)
    };
    match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\') => {
            match home() {
                Some(home) => home.join(rest.trim_start_matches(['/', '\\'])),
                None => PathBuf::from(path),
            }
        }
        _ => PathBuf::from(path),
    }
}

/// Réglages lus par [`load`].
#[derive(Clone, Debug)]
pub struct Loaded<T> {
    /// Réglages du fichier, ou ceux par défaut si le fichier est absent ou invalide.
    pub settings: T,
    /// Fichier lu ; `None` si aucun (pas de fichier à l'emplacement par défaut).
    pub path: Option<PathBuf>,
    /// Problème à signaler à l'utilisateur (fichier invalide, introuvable...). Les
    /// réglages par défaut sont alors utilisés : une faute de frappe ne doit pas empêcher
    /// de trier ses photos.
    pub warning: Option<String>,
}

/// Lit les réglages dans `path`, ou à l'emplacement par défaut ([`default_path`]) si
/// `None`. `T` est le type du fichier entier : [`EngineSettings`] pour le moteur seul,
/// ou un type de l'interface qui reprend ses sections et y ajoute les siennes.
///
/// Un fichier absent à l'emplacement par défaut n'est pas une erreur (réglages par
/// défaut, sans avertissement) ; un fichier demandé explicitement mais introuvable, ou
/// invalide, l'est (réglages par défaut, avec avertissement).
pub fn load<T: DeserializeOwned + Default>(path: Option<&Path>) -> Loaded<T> {
    let explicit = path.is_some();
    let Some(path) = path.map(Path::to_path_buf).or_else(default_path) else {
        return Loaded {
            settings: T::default(),
            path: None,
            warning: None,
        };
    };
    let failed = |warning: String, path: Option<PathBuf>| Loaded {
        settings: T::default(),
        path,
        warning: Some(crate::t!("settings-defaults-used", warning = warning)),
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound && !explicit => {
            return Loaded {
                settings: T::default(),
                path: None,
                warning: None,
            };
        }
        Err(e) => {
            let message = crate::t!(
                "settings-file-error",
                path = path.display().to_string(),
                error = e.to_string()
            );
            return failed(message, None);
        }
    };
    match parse(&text) {
        Ok(settings) => Loaded {
            settings,
            path: Some(path),
            warning: None,
        },
        Err(e) => failed(
            crate::t!(
                "settings-file-error",
                path = path.display().to_string(),
                error = e
            ),
            Some(path),
        ),
    }
}

/// Lit des réglages depuis un texte TOML. Le message d'erreur indique la ligne et la clé
/// en cause.
pub fn parse<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    toml::from_str(text).map_err(|e| {
        let full = e.to_string().trim_end().to_string();
        let raw = e.message().trim_end();
        let Some(place) = full.strip_suffix(raw) else {
            return full;
        };
        // « TOML parse error at line 2, column 13 » : la position, dans la langue choisie.
        let place = match place.split_once('\n') {
            Some((head, rest)) => match position(head) {
                Some((line, column)) => {
                    let head = crate::t!("settings-error-at", line = line, column = column);
                    format!("{head}\n{rest}")
                }
                None => place.to_string(),
            },
            None => place.to_string(),
        };
        format!("{place}{}", readable(raw))
    })
}

/// Comme [`parse`], mais l'erreur sans sa position ni l'extrait du fichier : pour un
/// message montré sous la valeur en cause (fenêtre de réglages, retour du 07/11/2026 :
/// « TOML parse error at line 22 » et ses `^^^^` n'y disaient rien de plus).
pub fn parse_message<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    toml::from_str(text).map_err(|e| readable(e.message().trim_end()))
}

/// Ligne et colonne de « TOML parse error at line 2, column 13 ».
fn position(head: &str) -> Option<(u64, u64)> {
    let rest = head.strip_prefix("TOML parse error at line ")?;
    let (line, column) = rest.split_once(", column ")?;
    Some((line.trim().parse().ok()?, column.trim().parse().ok()?))
}

/// Message d'erreur de serde / TOML dit avec des mots de tous les jours, dans la langue
/// de l'interface (issue n° 353 : « expected u64 » ne dit rien à personne). Les formes
/// courantes sont traduites (type attendu, choix inconnu, clé inconnue avec la plus
/// proche, clé en double) ; les autres messages, dont les nôtres, passent tels quels.
pub fn readable(message: &str) -> String {
    readable_parts(message).unwrap_or_else(|| message.to_string())
}

fn readable_parts(message: &str) -> Option<String> {
    if let Some(rest) = message
        .strip_prefix("invalid value: ")
        .or_else(|| message.strip_prefix("invalid type: "))
    {
        let (given, expected) = rest.rsplit_once(", expected ")?;
        return Some(crate::t!(
            "settings-error-expected",
            expected = expected_kind(expected)?,
            given = given_value(given)
        ));
    }
    if let Some(rest) = message.strip_prefix("unknown variant ") {
        let (value, choices) = rest.split_once(", expected ")?;
        return Some(crate::t!(
            "settings-error-choice",
            value = unquote(value),
            choices = quoted(choices).join(", ")
        ));
    }
    if let Some(rest) = message.strip_prefix("unknown field ") {
        let (name, known) = rest.split_once(", expected ").unwrap_or((rest, ""));
        let name = unquote(name);
        let closest = closest(&name, quoted(known).into_iter());
        return Some(match closest {
            Some(closest) => crate::t!(
                "settings-error-unknown-near",
                name = name,
                closest = closest
            ),
            None => crate::t!("settings-error-unknown", name = name),
        });
    }
    if let Some(rest) = message.strip_prefix("missing field ") {
        return Some(crate::t!("settings-error-missing", name = unquote(rest)));
    }
    if message == "duplicate key" {
        return Some(crate::t!("settings-error-duplicate"));
    }
    None
}

/// Ce que serde attend (« u64 », « a boolean »…), en mots ; `None` si inconnu.
fn expected_kind(expected: &str) -> Option<String> {
    let id = match expected {
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => "settings-kind-unsigned",
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => "settings-kind-integer",
        "f32" | "f64" => "settings-kind-number",
        "a boolean" => "settings-kind-boolean",
        "a string" | "a character" | "a borrowed string" => "settings-kind-text",
        "a sequence" => "settings-kind-list",
        "a map" => "settings-kind-section",
        _ if expected.starts_with("struct ") || expected.starts_with("a table") => {
            "settings-kind-section"
        }
        _ => return None,
    };
    Some(crate::t!(id))
}

/// La valeur donnée telle que serde la décrit (« integer `-2` », « string "a" »,
/// « sequence ») : la valeur seule, ou son genre en mots.
fn given_value(given: &str) -> String {
    for prefix in [
        "integer ",
        "floating point ",
        "boolean ",
        "string ",
        "char ",
    ] {
        if let Some(value) = given.strip_prefix(prefix) {
            let value = value.trim_matches('`');
            return if prefix == "string " || prefix == "char " {
                format!("\"{}\"", value.trim_matches('"').trim_matches('\''))
            } else {
                value.to_string()
            };
        }
    }
    match given {
        "sequence" => crate::t!("settings-kind-list"),
        "map" => crate::t!("settings-kind-section"),
        _ => given.to_string(),
    }
}

/// Les noms entre accents graves de `text` (« `a`, `b` », « `a` or `b` », « one of … »).
fn quoted(text: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

fn unquote(text: &str) -> String {
    text.trim().trim_matches('`').to_string()
}

/// Le nom de `known` le plus proche de `name` (faute de frappe), s'il l'est assez.
fn closest(name: &str, known: impl Iterator<Item = String>) -> Option<String> {
    let limit = (name.chars().count() / 3).clamp(1, 3);
    known
        .filter(|k| !k.is_empty())
        .map(|k| (edit_distance(name, &k), k))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, k)| k)
}

/// Distance d'édition (Levenshtein) entre deux noms.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (above + 1)
                .min(row[j] + 1)
                .min(diagonal + usize::from(ca != *cb));
            diagonal = above;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_messages_are_said_in_plain_words() {
        #[derive(Debug, Deserialize)]
        #[serde(deny_unknown_fields)]
        #[allow(dead_code)]
        struct Probe {
            delay_ms: Option<u64>,
            ratio: Option<f32>,
            on: Option<bool>,
            mode: Option<Mode>,
        }
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Mode {
            Fast,
            Slow,
        }
        let message = |text: &str| parse_message::<Probe>(text).unwrap_err();
        assert_eq!(
            message("delay_ms = -2"),
            "attendu : un nombre entier positif ou nul (donné : -2)"
        );
        assert_eq!(
            message("delay_ms = \"a\""),
            "attendu : un nombre entier positif ou nul (donné : \"a\")"
        );
        assert_eq!(
            message("ratio = true"),
            "attendu : un nombre (donné : true)"
        );
        assert_eq!(
            message("on = [1]"),
            "attendu : true ou false (donné : une liste entre crochets [ ])"
        );
        assert_eq!(
            message("mode = \"medium\""),
            "« medium » n'est pas un choix possible : fast, slow"
        );
        assert_eq!(
            message("dlay_ms = 3"),
            "clé inconnue : « dlay_ms » (vouliez-vous dire « delay_ms » ?)"
        );
        assert_eq!(message("zzz = 3"), "clé inconnue : « zzz »");
        assert_eq!(message("on = true\non = false"), "clé donnée deux fois");
        let full = parse::<Probe>("\ndelay_ms = -2").unwrap_err();
        assert!(full.starts_with("ligne 2, colonne 12\n"), "{full}");
        assert!(full.contains("delay_ms = -2"), "extrait gardé : {full}");
        assert!(full.ends_with("(donné : -2)"), "{full}");
        assert_eq!(readable("our own message"), "our own message");
    }

    fn env<'a>(vars: &'a [(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| OsString::from(v))
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn default_path_per_os() {
        let home = [("HOME", "/home/u")];
        assert_eq!(
            default_path_for("linux", env(&home)),
            Some("/home/u/.config/tri/config.toml".into())
        );
        let xdg = [("HOME", "/home/u"), ("XDG_CONFIG_HOME", "/etc/u")];
        assert_eq!(
            default_path_for("linux", env(&xdg)),
            Some("/etc/u/tri/config.toml".into())
        );
        let rel = [("HOME", "/home/u"), ("XDG_CONFIG_HOME", "conf")];
        assert_eq!(
            default_path_for("linux", env(&rel)),
            Some("/home/u/.config/tri/config.toml".into()),
            "XDG relatif ignoré"
        );
        assert_eq!(
            default_path_for("macos", env(&home)),
            Some("/home/u/Library/Application Support/tri/config.toml".into())
        );
        assert_eq!(default_path_for("linux", env(&[])), None);
        assert_eq!(default_path_for("windows", env(&home)), None);
    }

    // Chemins Unix simulés : sous Windows, « /home/u » n'est pas absolu.
    #[cfg(not(windows))]
    #[test]
    fn data_dir_per_os() {
        let home = [("HOME", "/home/u")];
        assert_eq!(
            data_dir_for("linux", env(&home)),
            Some("/home/u/.local/share/tri".into())
        );
        let xdg = [("HOME", "/home/u"), ("XDG_DATA_HOME", "/data/u")];
        assert_eq!(data_dir_for("linux", env(&xdg)), Some("/data/u/tri".into()));
        assert_eq!(
            data_dir_for("macos", env(&home)),
            Some("/home/u/Library/Application Support/tri".into())
        );
        assert_eq!(data_dir_for("linux", env(&[])), None);
    }

    #[test]
    fn home_is_expanded() {
        let home = [("HOME", "/home/u")];
        assert_eq!(
            expand_home_with("~/Pictures", env(&home)),
            PathBuf::from("/home/u/Pictures")
        );
        assert_eq!(expand_home_with("~", env(&home)), PathBuf::from("/home/u"));
        assert_eq!(expand_home_with("/abs", env(&home)), PathBuf::from("/abs"));
        assert_eq!(
            expand_home_with("~autre", env(&home)),
            PathBuf::from("~autre")
        );
    }

    #[test]
    fn empty_file_gives_defaults() {
        assert_eq!(
            parse::<EngineSettings>("").unwrap(),
            EngineSettings::default()
        );
    }

    #[test]
    fn defaults_match_the_engine_types() {
        let s = EngineSettings::default();
        let p = s.preview(None);
        assert_eq!(
            p,
            preview::Settings::default(),
            "« auto » sans écran = 2560"
        );
        assert_eq!(s.preview(Some(3840)).sizes.standard, 3840);
        let c = s.scheduler(RawPreviews::Background);
        assert_eq!(c.light_workers, scheduler::Config::default().light_workers);
        assert_eq!(
            c.thumbnails_first,
            scheduler::Config::default().thumbnails_first
        );
    }

    #[test]
    fn analyses_are_read_with_the_old_keys() {
        let s: EngineSettings =
            parse("[analysis]\ncolours = \"off\"\nstacks = \"on-demand\"").unwrap();
        let config = s.scheduler(RawPreviews::OnDemand);
        assert_eq!(config.palettes, Analysis::Off);
        assert_eq!(config.similarity, Analysis::OnDemand);
        assert_eq!(config.exposure, Analysis::Background);
        // Avant #564 : `false` arrêtait le calcul de fond.
        let s: EngineSettings = parse(
            "[scheduler]\nexposure = false\n[sharpness]\nbackground = false\n\
             [similarity]\nbackground = true",
        )
        .unwrap();
        let a = s.analysis();
        assert_eq!(a.exposure, Analysis::OnDemand);
        assert_eq!(a.sharpness, Analysis::OnDemand);
        assert_eq!(a.stacks, Analysis::Background);
        // La nouvelle clé l'emporte.
        let s: EngineSettings =
            parse("[scheduler]\nexposure = false\n[analysis]\nexposure = \"off\"").unwrap();
        assert_eq!(s.analysis().exposure, Analysis::Off);
    }

    #[test]
    fn partial_file_keeps_other_defaults() {
        let s: EngineSettings = parse(
            "[preview]\nstandard = 3840\n[scheduler]\nlight_workers = 3\n[render]\nauto_bright = true",
        )
        .unwrap();
        assert_eq!(s.preview.standard, Auto::Fixed(3840));
        assert_eq!(
            s.preview(Some(1000)).sizes.standard,
            3840,
            "fixée l'emporte"
        );
        assert_eq!(s.preview.thumbnail, 400);
        assert_eq!(s.scheduler(RawPreviews::OnDemand).light_workers, 3);
        assert!(s.preview(None).render.auto_bright);
        assert!(s.preview(None).render.camera_wb);
    }

    #[test]
    fn auto_accepts_auto_or_positive_integers_only() {
        let parse_workers =
            |v: &str| parse::<EngineSettings>(&format!("[scheduler]\nlight_workers = {v}"));
        assert_eq!(
            parse_workers("\"auto\"").unwrap().scheduler.light_workers,
            Auto::Auto
        );
        assert_eq!(
            parse_workers("2").unwrap().scheduler.light_workers,
            Auto::Fixed(2)
        );
        for bad in ["\"automatique\"", "-1", "1.5", "true"] {
            let err = parse_workers(bad).unwrap_err();
            assert!(err.contains("light_workers"), "{bad} : {err}");
        }
        let err = parse::<EngineSettings>("[preview]\nstandard = 99999999999").unwrap_err();
        assert!(err.contains("\"auto\" ou un entier positif"), "{err}");
    }

    #[test]
    fn zero_workers_means_one() {
        let s: EngineSettings = parse("[scheduler]\nlight_workers = 0").unwrap();
        assert_eq!(s.scheduler(RawPreviews::Background).light_workers, 1);
    }

    #[test]
    fn unknown_key_in_a_section_is_reported_with_its_line() {
        let err =
            parse::<EngineSettings>("[preview]\nthumbnail = 300\nstandart = 2000").unwrap_err();
        assert!(err.contains("standart"), "{err}");
        assert!(err.contains("ligne 3"), "{err}");
    }

    #[test]
    fn other_sections_are_left_to_the_interface() {
        let s: EngineSettings = parse("[gui]\ntheme = \"dark\"").unwrap();
        assert_eq!(s, EngineSettings::default());
    }

    #[test]
    fn wrong_type_is_reported() {
        let err = parse::<EngineSettings>("[cache]\njpeg_quality = \"haute\"").unwrap_err();
        assert!(err.contains("jpeg_quality"), "{err}");
    }
}
