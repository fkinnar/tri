//! Rendu par un logiciel de développement (idée de l'utilisateur, 19/10/2026) : voir dans
//! `tri` l'image telle qu'on l'a développée ailleurs, darktable d'abord.
//!
//! Plugins compilés avec `tri` (un trait, une liste, comme les autres plugins), qui pilotent
//! un **programme externe** en processus séparé : `darktable-cli` produit l'image avec tout
//! l'historique de développement du sidecar XMP. Le programme externe ne peut pas faire
//! tomber `tri`, et sa licence reste la sienne. RawTherapee (`rawtherapee-cli`, profils
//! `.pp3`) et ART suivraient le même modèle. Voir `doc/en/develop.md`.
//!
//! Rien n'est écrit chez l'utilisateur : `darktable-cli` reçoit sa propre configuration
//! (dans le cache de `tri`) et une bibliothèque en mémoire, le sidecar n'est que lu.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Logiciel de développement dont `tri` sait afficher le rendu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Engine {
    /// darktable (`darktable-cli`, historique du sidecar `IMAGE.RAF.xmp`).
    Darktable,
    /// RapidRAW (`rapidraw export`, réglages du sidecar `IMAGE.RAF.rrdata`).
    RapidRaw,
    /// RawTherapee (`rawtherapee-cli`, profil `IMAGE.RAF.pp3` ; 31/10/2026).
    RawTherapee,
    /// ART, dérivé de RawTherapee (`ART-cli`, profil `IMAGE.RAF.arp` ; 31/10/2026).
    Art,
}

impl Engine {
    /// Tous les logiciels, dans l'ordre des listes.
    pub const ALL: [Engine; 4] = [
        Engine::Darktable,
        Engine::RapidRaw,
        Engine::RawTherapee,
        Engine::Art,
    ];

    /// Identifiant (configuration, `--level`, clés de cache).
    pub fn id(self) -> &'static str {
        match self {
            Engine::Darktable => "darktable",
            Engine::RapidRaw => "rapidraw",
            Engine::RawTherapee => "rawtherapee",
            Engine::Art => "art",
        }
    }

    /// Nom affiché (« Darktable » avec une majuscule : choix de l'utilisateur, 26/10/2026).
    pub fn name(self) -> &'static str {
        match self {
            Engine::Darktable => "Darktable",
            Engine::RapidRaw => "RapidRAW",
            Engine::RawTherapee => "RawTherapee",
            Engine::Art => "ART",
        }
    }

    /// Sigle, pour la liste repliée du filtre quand plusieurs logiciels sont cochés.
    pub fn short(self) -> &'static str {
        match self {
            Engine::Darktable => "DT",
            Engine::RapidRaw => "RR",
            Engine::RawTherapee => "RT",
            Engine::Art => "ART",
        }
    }

    /// Extension du profil de RawTherapee / ART (`"pp3"`, `"arp"`), pour eux seuls.
    pub fn profile_extension(self) -> Option<&'static str> {
        match self {
            Engine::RawTherapee => Some("pp3"),
            Engine::Art => Some("arp"),
            Engine::Darktable | Engine::RapidRaw => None,
        }
    }

    /// Vrai si les notes d'un développement de ce logiciel vivent dans son propre sidecar
    /// (RapidRAW, RawTherapee, ART) plutôt que dans un XMP partagé avec la photo.
    pub fn notes_in_own_sidecar(self) -> bool {
        !matches!(self, Engine::Darktable)
    }

    /// Logiciel d'identifiant `id`.
    pub fn from_id(id: &str) -> Option<Engine> {
        Engine::ALL.into_iter().find(|e| e.id() == id)
    }

    /// Sidecar de `image` qui porte un vrai développement de ce logiciel, s'il existe. Pour
    /// un développement ([`crate::developments`]) : son sidecar, s'il est de ce logiciel.
    pub fn sidecar(self, image: &Path) -> Option<PathBuf> {
        if let Some(development) = crate::developments::get(image) {
            return (development.engine == self).then_some(development.sidecar);
        }
        // Une photo qui a des développements de ce logiciel : son sidecar est le leur
        // (une image à part dans la grille), elle-même reste le rendu neutre (LibRaw).
        if crate::developments::has(image, self) {
            return None;
        }
        match self {
            Engine::Darktable => darktable_sidecar(image),
            Engine::RapidRaw => rapidraw_sidecar(image),
            Engine::RawTherapee => profile_sidecar(image, "pp3"),
            Engine::Art => profile_sidecar(image, "arp"),
        }
    }
}

/// Logiciel préféré pour le rendu développé d'une image (`[develop] default`) : `"auto"`
/// (le développement le plus récent), ou un logiciel, gardé tant qu'il a développé
/// l'image (choix de l'utilisateur, 26/10/2026 : on utilise darktable mais on essaie
/// RapidRAW, sans que le plus récent passe devant).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preference {
    /// Le sidecar modifié en dernier.
    #[default]
    Auto,
    /// Ce logiciel s'il a développé l'image, sinon un autre qui l'a fait.
    Engine(Engine),
}

impl Preference {
    /// Identifiant dans la configuration (`"auto"`, `"darktable"`…).
    pub fn id(self) -> &'static str {
        match self {
            Preference::Auto => "auto",
            Preference::Engine(e) => e.id(),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Preference {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        match text.trim() {
            "auto" => Ok(Preference::Auto),
            id => Engine::from_id(id).map(Preference::Engine).ok_or_else(|| {
                serde::de::Error::custom(crate::t!("develop-unknown-engine", value = id))
            }),
        }
    }
}

/// Logiciel dont le rendu développé de `image` s'affiche, parmi ceux de `available`
/// (installés) : celui de `preference` s'il a développé l'image, sinon le développement
/// le plus récent. `None` : aucun développement (l'image garde le rendu LibRaw).
pub fn resolve(image: &Path, preference: Preference, available: &[Engine]) -> Option<Engine> {
    let developed: Vec<(Engine, u128)> = available
        .iter()
        .filter_map(|&e| {
            let sidecar = e.sidecar(image)?;
            let (_, modified) = crate::cache::stamp(&sidecar)?;
            Some((e, modified))
        })
        .collect();
    if let Preference::Engine(wanted) = preference
        && developed.iter().any(|(e, _)| *e == wanted)
    {
        return Some(wanted);
    }
    developed
        .into_iter()
        .max_by_key(|(_, t)| *t)
        .map(|(e, _)| e)
}

/// Filtre « développées » (idée de l'utilisateur, 27/10/2026) : les images qu'un des
/// logiciels cochés a développées ; aucun coché, les images sans développement (montrées
/// dans leur rendu LibRaw).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DevelopedRule {
    /// Logiciels cochés ; vide : aucun développement.
    pub engines: Vec<Engine>,
}

impl DevelopedRule {
    /// Les logiciels cochés parmi `available` (installés, comme pour l'affichage).
    pub fn wanted(&self, available: &[Engine]) -> Vec<Engine> {
        available
            .iter()
            .copied()
            .filter(|e| self.engines.contains(e))
            .collect()
    }

    /// Vrai si `image` vérifie le filtre : un développement d'un logiciel coché, ou,
    /// aucun coché, aucun développement des logiciels de `available`.
    pub fn accepts(&self, image: &Path, available: &[Engine]) -> bool {
        if self.engines.is_empty() {
            return !available.iter().any(|e| e.sidecar(image).is_some());
        }
        self.wanted(available)
            .iter()
            .any(|e| e.sidecar(image).is_some())
    }

    /// Logiciel du développement à montrer pour `image` : le plus récent des logiciels
    /// cochés ; `None` : rendu LibRaw.
    pub fn engine_for(&self, image: &Path, available: &[Engine]) -> Option<Engine> {
        resolve(image, Preference::Auto, &self.wanted(available))
    }
}

/// Filtre sur les développements : un [`DevelopedRule`] appliqué avec les logiciels
/// installés. Lit les sidecars ; les images plates n'ont pas de développement.
#[derive(Clone, Copy, Debug)]
pub struct DevelopedFilter<'a> {
    /// Les logiciels cochés.
    pub rule: &'a DevelopedRule,
    /// Logiciels installés.
    pub available: &'a [Engine],
}

impl crate::filter::Filter for DevelopedFilter<'_> {
    fn apply(&self, groups: crate::filter::Groups) -> crate::filter::Groups {
        crate::filter::retain(groups, |f| {
            f.kind == crate::media::MediaKind::Raw && self.rule.accepts(&f.path, self.available)
        })
    }
}

/// Un logiciel de développement prêt à rendre (plugin : darktable, puis RapidRAW,
/// RawTherapee…). Le programme externe tourne dans un processus à part.
pub trait Developer: Send + Sync + std::fmt::Debug {
    /// Le logiciel.
    fn engine(&self) -> Engine;

    /// Développe `image` d'après son `sidecar`, dans `out` (JPEG), le côté long ramené à
    /// `side` ; `cancel` arrête le programme. Le sidecar n'est jamais écrit.
    fn render(
        &self,
        image: &Path,
        sidecar: &Path,
        out: &Path,
        side: u32,
        cancel: &AtomicBool,
    ) -> Result<(), DevelopError>;

    /// Commande qui ouvre le dossier `dir` dans l'interface du logiciel, si elle existe
    /// (darktable : oui ; RapidRAW n'ouvre qu'une image).
    fn open_folder(&self, _dir: &Path) -> Option<Command> {
        None
    }

    /// Vrai si le logiciel ne peut pas recevoir un dossier maintenant : déjà ouvert, sans
    /// moyen de le passer à l'instance ouverte (une seconde instance plante).
    fn open_blocked(&self) -> bool {
        false
    }
}

/// Les logiciels de développement trouvés sur la machine, et la préférence.
#[derive(Clone, Debug, Default)]
pub struct Developers {
    tools: Vec<Arc<dyn Developer>>,
    /// Logiciel préféré (`[develop] default`).
    pub preference: Preference,
    /// Dossier des fichiers de travail (rendus temporaires), dans le cache de `tri`.
    pub work_dir: PathBuf,
}

impl Developers {
    /// Aucun logiciel (rendus développés désactivés).
    pub fn none() -> Developers {
        Developers::default()
    }

    /// Les logiciels trouvés d'après `settings`, leurs fichiers de travail sous `work_dir`
    /// (dans le cache de `tri` : jamais la configuration de l'utilisateur).
    pub fn find(settings: &DevelopSettings, work_dir: &Path, high_quality: bool) -> Developers {
        let mut tools: Vec<Arc<dyn Developer>> = Vec::new();
        if let Some(cli) = find_darktable(&settings.darktable) {
            tools.push(Arc::new(Darktable {
                cli,
                config_dir: work_dir.join("darktable"),
                high_quality,
            }));
        }
        if let Some(cli) = find_rapidraw(&settings.rapidraw) {
            tools.push(Arc::new(RapidRaw {
                cli,
                work_dir: work_dir.join("rapidraw"),
            }));
        }
        for (engine, setting) in [
            (Engine::RawTherapee, &settings.rawtherapee),
            (Engine::Art, &settings.art),
        ] {
            if let Some(cli) = find_profile_cli(engine, setting) {
                tools.push(Arc::new(ProfileTool {
                    engine,
                    cli,
                    work_dir: work_dir.join(engine.id()),
                }));
            }
        }
        Developers {
            tools,
            preference: settings.default,
            work_dir: work_dir.to_path_buf(),
        }
    }

    /// Ajoute un logiciel (tests, logiciels choisis).
    pub fn with(mut self, tool: Arc<dyn Developer>) -> Developers {
        self.tools.push(tool);
        self
    }

    /// Logiciels installés, dans l'ordre de [`Engine::ALL`].
    pub fn engines(&self) -> Vec<Engine> {
        let mut engines: Vec<Engine> = self.tools.iter().map(|t| t.engine()).collect();
        engines.sort();
        engines
    }

    /// Vrai si aucun logiciel n'est installé.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Le logiciel `engine`, s'il est installé.
    pub fn get(&self, engine: Engine) -> Option<&Arc<dyn Developer>> {
        self.tools.iter().find(|t| t.engine() == engine)
    }

    /// Logiciels installés qui savent ouvrir un dossier ([`Developer::open_folder`]).
    pub fn folder_openers(&self) -> Vec<Engine> {
        let probe = Path::new(".");
        let mut engines: Vec<Engine> = self
            .tools
            .iter()
            .filter(|t| t.open_folder(probe).is_some())
            .map(|t| t.engine())
            .collect();
        engines.sort();
        engines
    }

    /// Vrai si `engine` ne peut pas recevoir un dossier maintenant
    /// ([`Developer::open_blocked`]).
    pub fn open_blocked(&self, engine: Engine) -> bool {
        self.get(engine).is_some_and(|t| t.open_blocked())
    }

    /// Commande qui ouvre `dir` dans `engine`, s'il est installé et le sait.
    pub fn open_folder(&self, engine: Engine, dir: &Path) -> Option<Command> {
        self.get(engine)?.open_folder(dir)
    }

    /// Logiciel du rendu développé de `image` (voir [`resolve`]).
    pub fn resolve(&self, image: &Path) -> Option<Engine> {
        resolve(image, self.preference, &self.engines())
    }
}

/// Réglages (section `[develop]` du fichier de configuration).
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DevelopSettings {
    /// `darktable-cli` : `"auto"` (cherché dans le `PATH`, aux emplacements habituels,
    /// puis sous Linux en AppImage et en Flatpak), un chemin (`darktable-cli` ou une
    /// AppImage de darktable), `"flatpak"` (celui du Flatpak `org.darktable.Darktable`),
    /// ou `""` (pas de rendu darktable).
    pub darktable: String,
    /// Rendu darktable en haute qualité (`--hq`) : un peu plus fidèle, un peu plus lent.
    pub high_quality: bool,
    /// RapidRAW (≥ 1.6, commande `export`) : `"auto"` (cherché dans le `PATH`, aux
    /// emplacements habituels, puis sous Linux en AppImage et en Flatpak), un chemin
    /// (l'exécutable de RapidRAW ou son AppImage), `"flatpak"`, ou `""` (pas de rendu
    /// RapidRAW).
    pub rapidraw: String,
    /// `rawtherapee-cli` (31/10/2026) : `"auto"` (cherché dans le `PATH`, aux emplacements
    /// habituels, puis sous Linux en AppImage et en Flatpak), un chemin, `"flatpak"`
    /// (`com.rawtherapee.RawTherapee`), ou `""` (pas de rendu RawTherapee).
    pub rawtherapee: String,
    /// `ART-cli` (31/10/2026) : `"auto"` (`PATH`, emplacements habituels, AppImage sous
    /// Linux), un chemin, ou `""` (pas de rendu ART).
    pub art: String,
    /// Logiciel préféré quand une image a été développée dans plusieurs : `"auto"` (le
    /// développement le plus récent) ou un logiciel (`"darktable"`).
    pub default: Preference,
}

impl Default for DevelopSettings {
    fn default() -> Self {
        Self {
            darktable: "auto".into(),
            high_quality: false,
            rapidraw: "auto".into(),
            rawtherapee: "auto".into(),
            art: "auto".into(),
            default: Preference::Auto,
        }
    }
}

/// Emplacements habituels de `darktable-cli`, selon le système.
fn darktable_defaults(os: &str) -> Vec<PathBuf> {
    match os {
        "macos" => vec![PathBuf::from(
            "/Applications/darktable.app/Contents/MacOS/darktable-cli",
        )],
        "windows" => vec![PathBuf::from(
            r"C:\Program Files\darktable\bin\darktable-cli.exe",
        )],
        // Le lanceur Flatpak exporté (`/var/lib/flatpak/exports/bin/…`) ouvre l'interface
        // de darktable, pas `darktable-cli` : le Flatpak passe par `flatpak()`.
        _ => vec![
            PathBuf::from("/usr/bin/darktable-cli"),
            PathBuf::from("/usr/local/bin/darktable-cli"),
        ],
    }
}

/// Comment lancer `darktable-cli` : un programme, et des arguments à mettre devant les
/// nôtres (Flatpak : `flatpak run --command=darktable-cli org.darktable.Darktable`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cli {
    /// Programme lancé.
    pub program: PathBuf,
    /// Arguments placés avant ceux de `tri`.
    pub args: Vec<String>,
}

impl Cli {
    /// `darktable-cli` installé normalement (paquet, `.app`, installeur).
    pub fn native(path: PathBuf) -> Cli {
        Cli {
            program: path,
            args: Vec::new(),
        }
    }

    /// La commande, prête à recevoir les arguments de `tri`.
    pub fn command(&self) -> Command {
        let mut cmd = crate::process::command(&self.program);
        cmd.args(&self.args);
        cmd
    }
}

/// Identifiant Flatpak de darktable (Flathub).
pub const FLATPAK_APP: &str = "org.darktable.Darktable";

/// darktable en Flatpak (installation système ou utilisateur), s'il est installé : le
/// Gestionnaire de logiciels de Linux Mint l'installe ainsi (constaté le 19/10/2026), sans
/// `darktable-cli` dans le `PATH`.
fn flatpak() -> Option<Cli> {
    flatpak_app(FLATPAK_APP, Some("darktable-cli"))
}

/// L'application Flatpak `app`, si elle est installée, lancée telle quelle ou par sa
/// commande `command`.
fn flatpak_app(app: &str, command: Option<&str>) -> Option<Cli> {
    let installed = crate::process::command("flatpak")
        .args(["info", app])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    let mut args = vec!["run".to_string()];
    if let Some(command) = command {
        args.push(format!("--command={command}"));
    }
    args.push(app.into());
    installed.then(|| Cli {
        program: PathBuf::from("flatpak"),
        args,
    })
}

/// `darktable-cli` selon le réglage `setting` (voir [`DevelopSettings::darktable`]) :
/// `None` s'il est désactivé ou introuvable.
pub fn find_darktable(setting: &str) -> Option<Cli> {
    let setting = setting.trim();
    match setting {
        "" => return None,
        "flatpak" => return flatpak(),
        "auto" => {}
        path => {
            let path = crate::settings::expand_home(path);
            return path.is_file().then(|| cli_for(path));
        }
    }
    let exe = if cfg!(windows) {
        "darktable-cli.exe"
    } else {
        "darktable-cli"
    };
    let native = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|dir| dir.join(exe))
        .chain(darktable_defaults(std::env::consts::OS))
        .find(|p| p.is_file())
        .map(Cli::native);
    if native.is_some() || !cfg!(target_os = "linux") {
        return native;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    appimage(home.as_deref(), "darktable")
        .map(cli_for)
        .or_else(flatpak)
}

/// L'interface de darktable, d'après `darktable-cli` : le programme `darktable` à côté
/// (paquet, installeur Windows), l'AppImage sans commande (elle lance l'interface par
/// défaut), ou le Flatpak sans `--command`. `None` si le programme manque. Sous macOS,
/// l'application par `open -a darktable.app` : le système passe le dossier à l'instance
/// déjà ouverte (darktable le reçoit, `_osx_openfile_callback`) ; lancer le programme du
/// `.app` en démarrait une seconde, qui plantait.
pub fn darktable_gui(cli: &Cli) -> Option<Cli> {
    darktable_gui_on(cli, std::env::consts::OS)
}

/// [`darktable_gui`] pour le système `os`.
fn darktable_gui_on(cli: &Cli, os: &str) -> Option<Cli> {
    if cli.program == Path::new("flatpak") {
        let args = cli
            .args
            .iter()
            .filter(|a| !a.starts_with("--command="))
            .cloned()
            .collect();
        return Some(Cli {
            program: cli.program.clone(),
            args,
        });
    }
    if is_appimage(&cli.program) {
        return Some(Cli::native(cli.program.clone()));
    }
    let exe = if cli
        .program
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    {
        "darktable.exe"
    } else {
        "darktable"
    };
    let gui = cli.program.with_file_name(exe);
    if !gui.is_file() {
        return None;
    }
    // …/darktable.app/Contents/MacOS/darktable : l'application elle-même.
    let bundle = gui
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|app| app.extension().is_some_and(|e| e == "app"));
    match bundle {
        Some(app) if os == "macos" => Some(Cli {
            program: PathBuf::from("open"),
            args: vec!["-a".into(), app.to_string_lossy().into_owned()],
        }),
        _ => Some(Cli::native(gui)),
    }
}

/// Un exécutable donné : AppImage de darktable (le binaire `darktable-cli` qu'elle
/// contient est choisi par son nom en premier argument, documenté par darktable :
/// « invocation per OS »), sinon `darktable-cli` lui-même.
fn cli_for(path: PathBuf) -> Cli {
    if is_appimage(&path) {
        Cli {
            program: path,
            args: vec!["darktable-cli".into()],
        }
    } else {
        Cli::native(path)
    }
}

fn is_appimage(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("appimage"))
}

/// AppImage dont le nom contient `name` (« darktable »…) aux emplacements habituels
/// (`~/AppImages` de Gear Lever ou AppImageLauncher, `~/Applications`, `~/.local/bin`,
/// `~/bin`, téléchargements, `/opt`) : la plus récente. Constaté le 19/10/2026 sur la
/// machine Linux Mint de l'utilisateur (`~/AppImages/darktable.appimage`), format que
/// darktable propose lui-même pour Linux.
fn appimage(home: Option<&Path>, name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = [
        "AppImages",
        "Applications",
        ".local/bin",
        "bin",
        "Downloads",
        "Téléchargements",
    ]
    .iter()
    .filter_map(|d| home.map(|h| h.join(d)))
    .collect();
    dirs.push(PathBuf::from("/opt"));
    dirs.iter()
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten())
        .map(|e| e.path())
        .filter(|p| {
            is_appimage(p)
                && p.is_file()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().to_lowercase().contains(name))
        })
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
}

/// Identifiant Flatpak de RapidRAW (Flathub).
pub const RAPIDRAW_FLATPAK: &str = "io.github.CyberTimon.RapidRAW";

/// Emplacements habituels de RapidRAW, selon le système.
fn rapidraw_defaults(os: &str) -> Vec<PathBuf> {
    match os {
        "macos" => vec![PathBuf::from(
            "/Applications/RapidRAW.app/Contents/MacOS/RapidRAW",
        )],
        "windows" => vec![PathBuf::from(r"C:\Program Files\RapidRAW\RapidRAW.exe")],
        _ => ["/usr/bin", "/usr/local/bin"]
            .iter()
            .flat_map(|d| ["rapidraw", "RapidRAW"].map(|n| Path::new(d).join(n)))
            .collect(),
    }
}

/// RapidRAW selon le réglage `setting` (voir [`DevelopSettings::rapidraw`]) : `None`
/// s'il est désactivé ou introuvable. Son exécutable fait aussi la commande `export`
/// (depuis la version 1.6), AppImage et Flatpak compris.
pub fn find_rapidraw(setting: &str) -> Option<Cli> {
    match setting.trim() {
        "" => return None,
        "flatpak" => return flatpak_app(RAPIDRAW_FLATPAK, None),
        "auto" => {}
        path => {
            let path = crate::settings::expand_home(path);
            return path.is_file().then(|| Cli::native(path));
        }
    }
    let names: &[&str] = if cfg!(windows) {
        &["RapidRAW.exe", "rapidraw.exe"]
    } else {
        &["rapidraw", "RapidRAW"]
    };
    let native = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .chain(rapidraw_defaults(std::env::consts::OS))
        .find(|p| p.is_file())
        .map(Cli::native);
    if native.is_some() || !cfg!(target_os = "linux") {
        return native;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    appimage(home.as_deref(), "rapidraw")
        .map(Cli::native)
        .or_else(|| flatpak_app(RAPIDRAW_FLATPAK, None))
}

/// Sidecar RapidRAW de `image` (`IMAGE.RAF.rrdata`, à côté de l'image), s'il existe :
/// RapidRAW l'écrit dès qu'on règle l'image. Ses copies virtuelles
/// (`IMAGE.RAF.<id>.rrdata`) ne sont pas lues.
pub fn rapidraw_sidecar(image: &Path) -> Option<PathBuf> {
    let mut name = image.file_name()?.to_os_string();
    name.push(".rrdata");
    let sidecar = image.with_file_name(name);
    // Un `.rrdata` qui ne porte qu'une note (écrit par `tri`, voir `sidecars`) n'est pas un
    // développement : rien à rendre (31/10/2026, retour de l'utilisateur : « Développer
    // avec RapidRAW » donnait un rendu orphelin, hors de la pile).
    (crate::cache::stamp(&sidecar).is_some() && crate::rrdata::file_is_development(&sidecar))
        .then_some(sidecar)
}

/// Profil de RawTherapee ou d'ART de `image` (`IMAGE.RAF.pp3` / `.arp`, à côté de
/// l'image), s'il existe et développe l'image : un profil réduit aux notes (image notée
/// dans leur explorateur, ou par `tri`) n'en est pas un (31/10/2026).
pub fn profile_sidecar(image: &Path, extension: &str) -> Option<PathBuf> {
    let sidecar = crate::pp3::path(image, extension);
    (crate::cache::stamp(&sidecar).is_some() && crate::pp3::file_is_development(&sidecar))
        .then_some(sidecar)
}

/// Identifiant Flatpak de RawTherapee (Flathub).
pub const RAWTHERAPEE_FLATPAK: &str = "com.rawtherapee.RawTherapee";

/// Noms de la ligne de commande de RawTherapee / ART, et de leur interface.
fn profile_cli_names(engine: Engine) -> (&'static str, &'static str) {
    match engine {
        Engine::RawTherapee => ("rawtherapee-cli", "rawtherapee"),
        Engine::Art => ("ART-cli", "ART"),
        Engine::Darktable | Engine::RapidRaw => ("", ""),
    }
}

/// Emplacements habituels de `rawtherapee-cli` / `ART-cli`, selon le système.
fn profile_cli_defaults(engine: Engine, os: &str) -> Vec<PathBuf> {
    let (cli, app) = profile_cli_names(engine);
    match os {
        "macos" => vec![PathBuf::from(format!(
            "/Applications/{app}.app/Contents/MacOS/{cli}"
        ))],
        "windows" => {
            // RawTherapee s'installe dans un dossier versionné (`RawTherapee\5.11\`).
            let root = PathBuf::from(format!(r"C:\Program Files\{app}"));
            let mut found: Vec<PathBuf> = std::fs::read_dir(&root)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path().join(format!("{cli}.exe")))
                .collect();
            found.push(root.join(format!("{cli}.exe")));
            found
        }
        _ => ["/usr/bin", "/usr/local/bin"]
            .iter()
            .map(|d| Path::new(d).join(cli))
            .collect(),
    }
}

/// `rawtherapee-cli` / `ART-cli` selon le réglage `setting` (voir
/// [`DevelopSettings::rawtherapee`]) : `None` s'il est désactivé ou introuvable. Une
/// AppImage (format de distribution officiel d'ART) fait aussi la ligne de commande,
/// lancée avec le nom de la commande en premier argument.
pub fn find_profile_cli(engine: Engine, setting: &str) -> Option<Cli> {
    let (cli, _) = profile_cli_names(engine);
    let flatpak = || {
        (engine == Engine::RawTherapee)
            .then(|| flatpak_app(RAWTHERAPEE_FLATPAK, Some(cli)))
            .flatten()
    };
    match setting.trim() {
        "" => return None,
        "flatpak" => return flatpak(),
        "auto" => {}
        path => {
            let path = crate::settings::expand_home(path);
            return path.is_file().then(|| {
                if is_appimage(&path) {
                    Cli {
                        program: path,
                        args: vec![cli.into()],
                    }
                } else {
                    Cli::native(path)
                }
            });
        }
    }
    let name = if cfg!(windows) {
        format!("{cli}.exe")
    } else {
        cli.to_string()
    };
    let native = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|dir| dir.join(&name))
        .chain(profile_cli_defaults(engine, std::env::consts::OS))
        .find(|p| p.is_file())
        .map(Cli::native);
    if native.is_some() || !cfg!(target_os = "linux") {
        return native;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    appimage(home.as_deref(), engine.id())
        .map(|path| Cli {
            program: path,
            args: vec![cli.into()],
        })
        .or_else(flatpak)
}

/// Sidecar darktable de `image` (`IMAGE.RAF.xmp`), s'il est de darktable : écrit par
/// darktable (`darktable:xmp_version`), même sans historique, ou avec un historique non
/// vide (`darktable:history_end` > 0). Retour du 30/10/2026 : une image importée dans
/// darktable mais pas retouchée (un clone vierge) a son rendu darktable par défaut, qui
/// n'est pas celui de LibRaw. Un sidecar écrit par `tri` seul (notes) ou par un autre
/// logiciel n'en a pas.
pub fn darktable_sidecar(image: &Path) -> Option<PathBuf> {
    use std::sync::{Mutex, OnceLock};
    type Memo = std::collections::HashMap<PathBuf, ((u64, u128), bool)>;
    static MEMO: OnceLock<Mutex<Memo>> = OnceLock::new();
    let sidecar = darktable_sidecar_path(image)?;
    // Relu seulement s'il a changé : demandé à chaque image affichée (NAS : un accès
    // réseau à chaque fois, 29/10/2026).
    let now = crate::cache::stamp(&sidecar)?;
    let memo = MEMO.get_or_init(Default::default);
    if let Some((known, developed)) = memo.lock().ok().and_then(|m| m.get(&sidecar).copied())
        && known == now
    {
        return developed.then_some(sidecar);
    }
    // Pendant le dessin de la grille : lu en fond, pas encore développé en attendant.
    if crate::cache::deferring() {
        let image = image.to_path_buf();
        crate::cache::defer_job(format!("darktable:{}", sidecar.display()), move || {
            darktable_sidecar(&image);
        });
        return None;
    }
    let developed = std::fs::read_to_string(&sidecar).is_ok_and(|text| {
        text.contains("darktable:xmp_version") || history_end(&text).is_some_and(|end| end > 0)
    });
    if let Ok(mut m) = memo.lock() {
        m.insert(sidecar.clone(), (now, developed));
    }
    developed.then_some(sidecar)
}

/// Chemin du sidecar darktable de `image` (`IMAGE.RAF.xmp`), qu'il existe ou non.
pub fn darktable_sidecar_path(image: &Path) -> Option<PathBuf> {
    let mut name = image.file_name()?.to_os_string();
    name.push(".xmp");
    Some(image.with_file_name(name))
}

/// Valeur de `darktable:history_end`, en attribut ou en élément.
fn history_end(xmp: &str) -> Option<u32> {
    let tag = "darktable:history_end";
    let at = xmp.find(tag)? + tag.len();
    let rest = xmp[at..].trim_start_matches(['=', '"', '>', ' ']);
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Modules darktable qui changent la taille de l'image autrement qu'un recadrage simple
/// (redressement et perspective, ancien recadrage, pixels pivotés ou mis à l'échelle de
/// certains capteurs) : avec l'un d'eux actif, la taille du développé reste inconnue.
const RESIZING: [&str; 4] = ["ashift", "clipping", "rotatepixels", "scalepixels"];

/// Taille du développé darktable décrit par le sidecar `xmp` (son contenu), pour une
/// image de `size` pixels (orientation EXIF appliquée), à pleine résolution : le cadre du
/// module `crop` (le dernier de l'historique actif, avant `history_end`), dans
/// l'orientation du module `flip`. `None` si un module de [`RESIZING`] est actif ou si un
/// paramètre est illisible : rien plutôt qu'une taille fausse (demandé le 20/10/2026).
pub fn developed_size(xmp: &str, size: (u32, u32)) -> Option<(u32, u32)> {
    let end = history_end(xmp)?;
    // Dernier état de chaque module (instance principale), dans l'ordre de l'historique.
    let mut last: std::collections::HashMap<String, (u32, bool, String)> =
        std::collections::HashMap::new();
    let mut reader = quick_xml::Reader::from_str(xmp);
    loop {
        let event = reader.read_event().ok()?;
        let element = match &event {
            quick_xml::events::Event::Start(e) | quick_xml::events::Event::Empty(e) => e,
            quick_xml::events::Event::Eof => break,
            _ => continue,
        };
        if element.name().as_ref() != "rdf:li" {
            continue;
        }
        let mut entry: (Option<u32>, Option<String>, bool, String, u32) =
            (None, None, false, String::new(), 0);
        for attr in element.attributes().flatten() {
            let value = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()?
                .into_owned();
            match attr.key.as_ref() {
                "darktable:num" => entry.0 = value.parse().ok(),
                "darktable:operation" => entry.1 = Some(value),
                "darktable:enabled" => entry.2 = value == "1",
                "darktable:params" => entry.3 = value,
                "darktable:multi_priority" => entry.4 = value.parse().unwrap_or(0),
                _ => {}
            }
        }
        let (Some(num), Some(operation)) = (entry.0, entry.1) else {
            continue;
        };
        if num >= end || entry.4 != 0 {
            continue;
        }
        if last.get(&operation).is_none_or(|(n, ..)| *n <= num) {
            last.insert(operation, (num, entry.2, entry.3));
        }
    }
    let active = |op: &str| last.get(op).filter(|(_, on, _)| *on).map(|(.., p)| p);
    if RESIZING.iter().any(|op| active(op).is_some()) {
        return None;
    }
    // `flip` : -1 (ou absent, ou éteint) = orientation EXIF ; sinon une orientation de
    // darktable, échange largeur / hauteur si le bit 4 est mis, sur le capteur (paysage).
    let (w, h) = match active("flip").map(|p| floats(p, 1).map(|v| v[0].to_bits() as i32)) {
        Some(Some(o)) if o >= 0 => {
            let (long, short) = (size.0.max(size.1), size.0.min(size.1));
            if o & 4 != 0 {
                (short, long)
            } else {
                (long, short)
            }
        }
        Some(None) => return None,
        _ => size,
    };
    // `crop` : bords gauche, haut, droit, bas en fractions de l'image.
    let Some(params) = active("crop") else {
        return Some((w, h));
    };
    let v = floats(params, 4)?;
    let (cw, ch) = (v[2] - v[0], v[3] - v[1]);
    if !(cw > 0.0 && cw <= 1.0 && ch > 0.0 && ch <= 1.0) {
        return None;
    }
    Some((
        (w as f32 * cw).round() as u32,
        (h as f32 * ch).round() as u32,
    ))
}

/// Les `n` premiers nombres 32 bits (petit-boutiste) des paramètres d'un module, écrits en
/// hexadécimal par darktable. Paramètres compressés (`gz…`, gros modules) : `None`.
fn floats(params: &str, n: usize) -> Option<Vec<f32>> {
    let bytes = params.as_bytes();
    if bytes.len() < n * 8 || params.starts_with("gz") {
        return None;
    }
    (0..n)
        .map(|i| {
            let hex = std::str::from_utf8(&bytes[i * 8..i * 8 + 8]).ok()?;
            let word = u32::from_str_radix(hex, 16).ok()?;
            Some(f32::from_bits(word.swap_bytes()))
        })
        .collect()
}

/// Empreinte d'un sidecar (taille, date) : dans la clé de cache, pour qu'un développement
/// modifié dans darktable refasse le rendu.
pub fn stamp(sidecar: &Path) -> String {
    match crate::cache::lookup_stamp(sidecar) {
        Some(Some((len, modified))) => format!("{len}:{modified}"),
        Some(None) => "absent".into(),
        // Relevé demandé en fond (fil de l'interface) : rien à lire pour l'instant.
        None => PENDING.into(),
    }
}

/// Empreinte d'un sidecar pas encore relevé ([`stamp`]).
const PENDING: &str = "pending";

/// Empreinte du **développement** décrit par un sidecar, pour la clé de cache du rendu
/// développé : sous `xmp`, seulement les champs de darktable (`darktable:…`), sans
/// `darktable:colorlabels` ; autre format (`.rrdata`), tout le contenu. Avant (revue du
/// 28/10/2026), la taille et la date du fichier : noter une image dans `tri`, qui écrit
/// dans le même `IMAGE.RAF.xmp` (convention darktable), refaisait le rendu (~10 s).
/// Mémorisée par fichier, taille et date : le sidecar n'est relu que s'il a changé.
pub fn render_stamp(sidecar: &Path) -> String {
    use std::sync::{Mutex, OnceLock};
    type Memo = std::collections::HashMap<PathBuf, (String, String)>;
    static MEMO: OnceLock<Mutex<Memo>> = OnceLock::new();
    let quick = stamp(sidecar);
    if quick == PENDING {
        return quick;
    }
    let memo = MEMO.get_or_init(Default::default);
    if let Some((known, fingerprint)) = memo.lock().ok().and_then(|m| m.get(sidecar).cloned())
        && known == quick
    {
        return fingerprint;
    }
    // Pendant le dessin de la grille : lu en fond.
    if crate::cache::deferring() {
        let path = sidecar.to_path_buf();
        crate::cache::defer_job(format!("render:{}", path.display()), move || {
            render_stamp(&path);
        });
        return PENDING.into();
    }
    let Ok(bytes) = std::fs::read(sidecar) else {
        return "absent".into();
    };
    let extension = |e: &str| {
        sidecar
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case(e))
    };
    let content = if extension("xmp") {
        String::from_utf8(bytes.clone())
            .ok()
            .and_then(|text| darktable_content(&text))
            .unwrap_or(bytes)
    } else if extension("pp3") || extension("arp") {
        // Profil de RawTherapee / ART : sans `[General]` (les notes, écrites par `tri`).
        String::from_utf8(bytes.clone())
            .ok()
            .map(|text| crate::pp3::development_content(&text).into_bytes())
            .unwrap_or(bytes)
    } else {
        bytes
    };
    let fingerprint = blake3::hash(&content).to_hex()[..16].to_string();
    if let Ok(mut m) = memo.lock() {
        m.insert(sidecar.to_path_buf(), (quick, fingerprint.clone()));
    }
    fingerprint
}

/// Champs de darktable d'un sidecar XMP, à la suite (attributs, éléments et leur texte),
/// sans `darktable:colorlabels` (écrit par `tri` avec les notes). `None` : XML illisible.
fn darktable_content(xmp: &str) -> Option<Vec<u8>> {
    use quick_xml::events::Event;
    let ours = |name: &str| name.starts_with("darktable:") && name != "darktable:colorlabels";
    let mut reader = quick_xml::Reader::from_str(xmp);
    let mut out = Vec::new();
    // Profondeur dans un élément de darktable (`darktable:history`…) : tout son contenu
    // compte, attributs des `rdf:li` compris.
    let mut inside = 0usize;
    loop {
        let (element, start) = match reader.read_event().ok()? {
            Event::Start(e) => (e, true),
            Event::Empty(e) => (e, false),
            Event::End(_) => {
                if inside > 0 {
                    inside -= 1;
                    out.push(b'>');
                }
                continue;
            }
            Event::Text(t) => {
                if inside > 0 {
                    out.extend_from_slice(t[..].as_bytes());
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let counted = inside > 0 || ours(element.name().as_ref());
        if counted {
            out.extend_from_slice(element.name().as_ref().as_bytes());
            out.push(b'<');
        }
        for attr in element.attributes().flatten() {
            if counted || ours(attr.key.as_ref()) {
                out.extend_from_slice(attr.key.as_ref().as_bytes());
                out.push(b'=');
                out.extend_from_slice(attr.value.as_bytes());
                out.push(b';');
            }
        }
        if start && counted {
            inside += 1;
        }
    }
    Some(out)
}

/// Erreur d'un rendu.
#[derive(Debug)]
pub enum DevelopError {
    /// Pas de développement darktable pour cette image (sidecar absent ou sans historique).
    NotDeveloped,
    /// Logiciel introuvable ou désactivé.
    Unavailable(Engine),
    /// Rendu annulé (l'image n'est plus affichée).
    Cancelled,
    /// Échec du logiciel (dernière ligne de son message d'erreur, ou la raison).
    Failed(Engine, String),
    /// Échec hors de darktable (export par LibRaw ou du JPEG boîtier), message tel quel.
    Other(String),
}

impl std::fmt::Display for DevelopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DevelopError::NotDeveloped => write!(f, "{}", crate::t!("develop-none")),
            DevelopError::Unavailable(engine) => write!(
                f,
                "{}",
                crate::t!(
                    "develop-unavailable",
                    engine = engine.name(),
                    id = engine.id()
                )
            ),
            DevelopError::Cancelled => write!(f, "annulé"),
            DevelopError::Failed(engine, e) => write!(
                f,
                "{}",
                crate::t!("develop-failed", engine = engine.name(), error = e.as_str())
            ),
            DevelopError::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DevelopError {}

/// darktable, prêt à développer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Darktable {
    /// `darktable-cli` ([`find_darktable`]).
    pub cli: Cli,
    /// Configuration propre à `tri` (dans son cache) : jamais celle de l'utilisateur.
    pub config_dir: PathBuf,
    /// Haute qualité (`--hq`).
    pub high_quality: bool,
}

impl Developer for Darktable {
    fn engine(&self) -> Engine {
        Engine::Darktable
    }

    /// L'interface de darktable sur `dir` : la table lumineuse s'ouvre avec ce dossier
    /// comme collection (importé au besoin) ; une instance déjà ouverte le reçoit, par
    /// D-Bus sous Linux, par le système sous macOS (`open -a`, voir [`darktable_gui`]).
    /// Avec la configuration de l'utilisateur, pas celle de `tri`.
    fn open_folder(&self, dir: &Path) -> Option<Command> {
        let mut command = darktable_gui(&self.cli)?.command();
        command.arg(dir);
        Some(command)
    }

    /// Sous Windows seulement (ni D-Bus ni `open -a`) : darktable déjà ouvert. Sous
    /// macOS, le lancer directement alors le faisait planter (28/10/2026) ; `open -a`
    /// passe le dossier à l'instance ouverte. Les rendus de `tri` passent par
    /// `darktable-cli`, qui ne compte pas.
    fn open_blocked(&self) -> bool {
        cfg!(windows) && crate::process::running("darktable")
    }

    /// Bibliothèque en mémoire, sidecars jamais écrits.
    fn render(
        &self,
        image: &Path,
        sidecar: &Path,
        out: &Path,
        side: u32,
        cancel: &AtomicBool,
    ) -> Result<(), DevelopError> {
        render_darktable(self, image, sidecar, out, side, cancel)
    }
}

fn render_darktable(
    dt: &Darktable,
    image: &Path,
    sidecar: &Path,
    out: &Path,
    side: u32,
    cancel: &AtomicBool,
) -> Result<(), DevelopError> {
    std::fs::create_dir_all(&dt.config_dir)
        .map_err(|e| DevelopError::Failed(Engine::Darktable, e.to_string()))?;
    let _ = std::fs::remove_file(out);
    let side = side.to_string();
    let mut child = dt
        .cli
        .command()
        // Clone darktable : l'image d'origine, avec le sidecar du clone.
        .arg(crate::media::source(image).as_ref())
        .arg(sidecar)
        .arg(out)
        .args(["--width", &side, "--height", &side])
        .args(["--hq", if dt.high_quality { "true" } else { "false" }])
        .arg("--core")
        .arg("--configdir")
        .arg(&dt.config_dir)
        .args(["--library", ":memory:"])
        // Jamais de sidecar écrit, quelle que soit la configuration de darktable.
        .args(["--conf", "write_sidecar_files=never"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| DevelopError::Failed(Engine::Darktable, e.to_string()))?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(DevelopError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() && out.is_file() {
                    return Ok(());
                }
                let mut message = String::new();
                if let Some(mut err) = child.stderr.take() {
                    use std::io::Read;
                    let _ = err.read_to_string(&mut message);
                }
                let last = message.lines().rev().find(|l| !l.trim().is_empty());
                return Err(DevelopError::Failed(
                    Engine::Darktable,
                    last.unwrap_or("darktable-cli").trim().to_string(),
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(DevelopError::Failed(Engine::Darktable, e.to_string())),
        }
    }
}

/// RapidRAW, prêt à développer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RapidRaw {
    /// Exécutable de RapidRAW ([`find_rapidraw`]).
    pub cli: Cli,
    /// Fichiers de travail (export temporaire, messages), dans le cache de `tri`.
    pub work_dir: PathBuf,
}

impl Developer for RapidRaw {
    fn engine(&self) -> Engine {
        Engine::RapidRaw
    }

    /// `rapidraw export` (RapidRAW ≥ 1.6, sans interface) applique de lui-même le
    /// sidecar `.rrdata` posé à côté de l'image ; il exporte à pleine taille, réduit
    /// ensuite à `side`. Interface de RapidRAW ouverte, il a rendu une image toute noire
    /// sans signaler d'erreur (essai du 26/10/2026) : refusée, avec un message clair.
    fn render(
        &self,
        image: &Path,
        _sidecar: &Path,
        out: &Path,
        side: u32,
        cancel: &AtomicBool,
    ) -> Result<(), DevelopError> {
        let failed =
            |e: &dyn std::fmt::Display| DevelopError::Failed(Engine::RapidRaw, e.to_string());
        std::fs::create_dir_all(&self.work_dir).map_err(|e| failed(&e))?;
        let id = format!("{}-{:?}", std::process::id(), std::thread::current().id());
        let export = self.work_dir.join(format!("export-{id}.jpg"));
        let log = self.work_dir.join(format!("export-{id}.log"));
        let _ = std::fs::remove_file(&export);
        // Messages dans un fichier plutôt qu'un tube : un programme bavard ne bloque pas.
        let messages = std::fs::File::create(&log).map_err(|e| failed(&e))?;
        // Une copie virtuelle : RapidRAW n'exporte que la photo ; ses réglages à elle lui
        // sont passés (`--adjustments`, vu dans son code le 30/10/2026).
        let development = crate::developments::get(image);
        let photo = development.as_ref().map_or(image, |d| d.photo.as_path());
        let adjustments = match &development {
            Some(d) if d.copy != crate::developments::Copy::Main => {
                let text = std::fs::read_to_string(&d.sidecar).map_err(|e| failed(&e))?;
                let json = crate::rrdata::adjustments(&text).map_err(|e| failed(&e))?;
                let file = self.work_dir.join(format!("adjustments-{id}.json"));
                std::fs::write(&file, json.to_string()).map_err(|e| failed(&e))?;
                Some(file)
            }
            _ => None,
        };
        let mut command = self.cli.command();
        command.arg("export").arg(photo);
        if let Some(file) = &adjustments {
            command.arg("--adjustments").arg(file);
        }
        let child = command
            .arg("--output")
            .arg(&export)
            .args(["--format", "jpeg", "--quality", "95"])
            .stdin(Stdio::null())
            .stdout(messages.try_clone().map_err(|e| failed(&e))?)
            .stderr(messages)
            .spawn()
            .map_err(|e| failed(&e))?;
        let status = wait(child, Engine::RapidRaw, cancel);
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        let _ = std::fs::remove_file(&log);
        let result = match status {
            Ok(status) if status.success() && export.is_file() => {
                shrink(&export, out, side).map_err(|e| failed(&e))
            }
            Ok(_) if text.contains("already running") => {
                Err(failed(&crate::t!("develop-rapidraw-open")))
            }
            Ok(_) => {
                let last = text.lines().rev().find(|l| !l.trim().is_empty());
                Err(DevelopError::Failed(
                    Engine::RapidRaw,
                    last.unwrap_or("rapidraw export").trim().to_string(),
                ))
            }
            Err(e) => Err(e),
        };
        let _ = std::fs::remove_file(&export);
        if let Some(file) = adjustments {
            let _ = std::fs::remove_file(file);
        }
        result
    }
}

/// RawTherapee ou ART (31/10/2026) : `rawtherapee-cli` / `ART-cli -o <dossier> -p <profil>
/// -j95 -Y -c <image>` applique le profil (`.pp3` / `.arp`) à l'image et écrit
/// `<dossier>/<nom>.jpg` ; pas d'option de taille, `tri` réduit ensuite à `side`.
#[derive(Debug)]
pub struct ProfileTool {
    /// RawTherapee ou ART.
    pub engine: Engine,
    /// Sa ligne de commande ([`find_profile_cli`]).
    pub cli: Cli,
    /// Fichiers de travail (export temporaire, messages), dans le cache de `tri`.
    pub work_dir: PathBuf,
}

impl Developer for ProfileTool {
    fn engine(&self) -> Engine {
        self.engine
    }

    fn render(
        &self,
        image: &Path,
        sidecar: &Path,
        out: &Path,
        side: u32,
        cancel: &AtomicBool,
    ) -> Result<(), DevelopError> {
        let engine = self.engine;
        let failed = |e: &dyn std::fmt::Display| DevelopError::Failed(engine, e.to_string());
        let id = format!("{}-{:?}", std::process::id(), std::thread::current().id());
        // Un développement (`IMG@rt.RAF`, chemin propre, jamais sur le disque) : le
        // logiciel reçoit la photo (vérifié sur RawTherapee 5.13 le 31/10/2026 : le lot
        // rendait l'entrée du développement et échouait).
        let photo =
            crate::developments::get(image).map_or_else(|| image.to_path_buf(), |d| d.photo);
        // Un dossier de sortie par rendu : le logiciel nomme lui-même le fichier
        // (`<nom sans extension>.jpg`).
        let export_dir = self.work_dir.join(format!("export-{id}"));
        std::fs::create_dir_all(&export_dir).map_err(|e| failed(&e))?;
        let log = self.work_dir.join(format!("export-{id}.log"));
        let messages = std::fs::File::create(&log).map_err(|e| failed(&e))?;
        let child = self
            .cli
            .command()
            .arg("-o")
            .arg(&export_dir)
            .arg("-p")
            .arg(sidecar)
            .args(["-j95", "-Y", "-c"])
            .arg(&photo)
            .stdin(Stdio::null())
            .stdout(messages.try_clone().map_err(|e| failed(&e))?)
            .stderr(messages)
            .spawn()
            .map_err(|e| failed(&e))?;
        let status = wait(child, engine, cancel);
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        let _ = std::fs::remove_file(&log);
        let produced = std::fs::read_dir(&export_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jpg")));
        let result = match (status, produced) {
            (Ok(status), Some(export)) if status.success() => {
                shrink(&export, out, side).map_err(|e| failed(&e))
            }
            (Ok(_), _) => {
                let last = text.lines().rev().find(|l| !l.trim().is_empty());
                Err(DevelopError::Failed(
                    engine,
                    last.unwrap_or(profile_cli_names(engine).0)
                        .trim()
                        .to_string(),
                ))
            }
            (Err(e), _) => Err(e),
        };
        let _ = std::fs::remove_dir_all(&export_dir);
        result
    }

    /// `rawtherapee <dossier>` / `ART <dossier>` : l'interface à côté de la ligne de
    /// commande (même dossier, nom sans `-cli`), si elle est là ; une AppImage ou un
    /// Flatpak ouvre l'interface par le programme lui-même.
    fn open_folder(&self, dir: &Path) -> Option<Command> {
        let (_, app) = profile_cli_names(self.engine);
        if self.cli.args.is_empty() {
            let name = if cfg!(windows) {
                format!("{app}.exe")
            } else {
                app.to_string()
            };
            let gui = self.cli.program.with_file_name(name);
            if !gui.is_file() {
                return None;
            }
            let mut command = crate::process::command(&gui);
            command.arg(dir);
            return Some(command);
        }
        let mut command = crate::process::command(&self.cli.program);
        command.args(
            self.cli
                .args
                .iter()
                .filter(|a| !a.starts_with("--command=") && !a.ends_with("-cli")),
        );
        command.arg(dir);
        Some(command)
    }
}

/// Attend la fin de `child` ; `cancel` l'arrête.
pub(crate) fn wait(
    mut child: std::process::Child,
    engine: Engine,
    cancel: &AtomicBool,
) -> Result<std::process::ExitStatus, DevelopError> {
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(DevelopError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(DevelopError::Failed(engine, e.to_string())),
        }
    }
}

/// Le JPEG `from`, réduit au côté long `side`, écrit dans `to` (qualité du cache).
/// Une image toute noire (aucun pixel au-dessus de 0) est refusée : c'est ce que rend
/// RapidRAW quand son interface est ouverte, jamais une vraie photo (le bruit du capteur
/// suffit à dépasser 0).
fn shrink(from: &Path, to: &Path, side: u32) -> Result<(), String> {
    let bytes = std::fs::read(from).map_err(|e| e.to_string())?;
    let bitmap = crate::jpeg::decode(&bytes, side)
        .map_err(|e| e.to_string())?
        .fit(side);
    if bitmap.data.iter().all(|&v| v == 0) {
        return Err(crate::t!("develop-rapidraw-black"));
    }
    let jpeg = crate::jpeg::encode(
        &bitmap,
        crate::cache::DEFAULT_JPEG_QUALITY,
        crate::jpeg::Chroma::Half,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(to, jpeg).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_darktable_interface_is_found_next_to_its_cli() {
        let dir = tempfile::tempdir().unwrap();
        let cli = dir.path().join("darktable-cli");
        std::fs::write(&cli, "").unwrap();
        assert_eq!(
            darktable_gui(&Cli::native(cli.clone())),
            None,
            "pas d'interface"
        );
        std::fs::write(dir.path().join("darktable"), "").unwrap();
        let gui = darktable_gui(&Cli::native(cli)).unwrap();
        assert_eq!(gui.program, dir.path().join("darktable"));
        // macOS : l'application par `open -a`, qui passe le dossier à l'instance ouverte.
        let macos = dir.path().join("darktable.app/Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        for name in ["darktable-cli", "darktable"] {
            std::fs::write(macos.join(name), "").unwrap();
        }
        let app = Cli::native(macos.join("darktable-cli"));
        let gui = darktable_gui_on(&app, "macos").unwrap();
        let bundle = dir.path().join("darktable.app");
        assert_eq!(gui.program, PathBuf::from("open"));
        assert_eq!(gui.args, ["-a".to_string(), bundle.display().to_string()]);
        assert_eq!(
            darktable_gui_on(&app, "linux").unwrap().program,
            macos.join("darktable"),
            "hors de macOS, le programme"
        );
        // AppImage : elle-même, sans la commande `darktable-cli`.
        let image = PathBuf::from("/home/u/AppImages/darktable.appimage");
        let gui = darktable_gui(&cli_for(image.clone())).unwrap();
        assert_eq!((gui.program, gui.args), (image, Vec::<String>::new()));
        // Flatpak : l'application, sans `--command`.
        let flatpak = Cli {
            program: PathBuf::from("flatpak"),
            args: vec![
                "run".into(),
                "--command=darktable-cli".into(),
                FLATPAK_APP.into(),
            ],
        };
        assert_eq!(
            darktable_gui(&flatpak).unwrap().args,
            ["run".to_string(), FLATPAK_APP.into()]
        );
    }

    #[test]
    fn darktable_opens_a_folder_and_rapidraw_does_not() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["darktable-cli", "darktable", "rapidraw"] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        let developers = Developers::none()
            .with(Arc::new(Darktable {
                cli: Cli::native(dir.path().join("darktable-cli")),
                config_dir: dir.path().join("config"),
                high_quality: false,
            }))
            .with(Arc::new(RapidRaw {
                cli: Cli::native(dir.path().join("rapidraw")),
                work_dir: dir.path().join("work"),
            }));
        assert_eq!(developers.folder_openers(), [Engine::Darktable]);
        let session = Path::new("/photos/2026-10-28_Jade");
        let command = developers.open_folder(Engine::Darktable, session).unwrap();
        assert_eq!(command.get_program(), dir.path().join("darktable"));
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            [session.as_os_str()],
            "la configuration de l'utilisateur"
        );
        assert!(developers.open_folder(Engine::RapidRaw, session).is_none());
    }

    /// Sidecar darktable minimal : `history_end`, puis les entrées `(num, opération,
    /// active, paramètres)`.
    fn sidecar(end: u32, entries: &[(u32, &str, bool, &str)]) -> String {
        let items: String = entries
            .iter()
            .map(|(num, op, on, params)| {
                format!(
                    r#"<rdf:li darktable:num="{num}" darktable:operation="{op}" darktable:enabled="{}" darktable:params="{params}" darktable:multi_priority="0"/>"#,
                    u8::from(*on)
                )
            })
            .collect();
        format!(
            r#"<rdf:Description darktable:history_end="{end}"><darktable:history><rdf:Seq>{items}</rdf:Seq></darktable:history></rdf:Description>"#
        )
    }

    #[test]
    fn the_developed_size_follows_the_darktable_crop() {
        // Paramètres relevés dans de vrais sidecars de l'utilisateur (darktable 5, crop v3).
        let tight = "8bd7213f08dd8b3e0000803fea56243f0000000000000000";
        let wide = "9d75823dbfa34b3d4af07f3fe8f77f3f0200000003000000";
        let auto = "ffffffff";
        let size = (6240, 4160);
        let xmp = sidecar(5, &[(0, "flip", true, auto), (1, "crop", true, tight)]);
        assert_eq!(developed_size(&xmp, size), Some((2295, 1534)));
        // Le dernier recadrage de l'historique actif ; ceux après history_end ne comptent pas.
        let xmp = sidecar(
            3,
            &[
                (1, "crop", true, tight),
                (2, "crop", true, wide),
                (3, "crop", true, tight),
            ],
        );
        assert_eq!(developed_size(&xmp, size), Some((5841, 3953)));
        // Recadrage éteint, ou pas de recadrage : l'image entière.
        let xmp = sidecar(3, &[(1, "crop", true, tight), (2, "crop", false, tight)]);
        assert_eq!(developed_size(&xmp, size), Some(size));
        assert_eq!(
            developed_size(&sidecar(1, &[(0, "exposure", true, "00")]), size),
            Some(size)
        );
        // Rotation choisie dans darktable (ici 90° : bit 4) : portrait.
        let xmp = sidecar(
            5,
            &[(0, "flip", true, "06000000"), (1, "crop", true, tight)],
        );
        assert_eq!(developed_size(&xmp, size), Some((1530, 2301)));
        // Redressement, perspective : taille inconnue plutôt que fausse.
        let xmp = sidecar(5, &[(0, "ashift", true, "00"), (1, "crop", true, tight)]);
        assert_eq!(developed_size(&xmp, size), None);
        // Paramètres compressés ou illisibles : inconnue aussi.
        let xmp = sidecar(5, &[(1, "crop", true, "gz12eJxj")]);
        assert_eq!(developed_size(&xmp, size), None);
    }

    /// Écrit un fichier et oublie son relevé, comme la surveillance du dossier quand un
    /// sidecar change ailleurs ([`crate::cache::forget_stamps`]).
    fn written(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> std::io::Result<()> {
        std::fs::write(path.as_ref(), contents)?;
        crate::cache::forget_stamps([path.as_ref()]);
        Ok(())
    }

    #[test]
    fn a_photo_with_developments_has_no_developed_render_of_its_own() {
        // Décidé le 30/10/2026 : le sidecar d'une photo appartient à son développement
        // (une image de la grille) ; la photo reste le rendu neutre.
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("DSCF0002.RAF");
        written(&image, b"raw").unwrap();
        let sidecar = dir.path().join("DSCF0002.RAF.xmp");
        written(&sidecar, r#"<rdf:Description darktable:xmp_version="5"/>"#).unwrap();
        assert_eq!(Engine::Darktable.sidecar(&image), Some(sidecar.clone()));
        let development = crate::developments::Development {
            engine: Engine::Darktable,
            copy: crate::developments::Copy::Main,
            photo: image.clone(),
            sidecar: sidecar.clone(),
        };
        crate::developments::set_for(&image, std::slice::from_ref(&development));
        assert_eq!(
            Engine::Darktable.sidecar(&image),
            None,
            "le sidecar est au développement"
        );
        assert_eq!(
            Engine::Darktable.sidecar(&development.path()),
            Some(sidecar)
        );
        assert_eq!(Engine::RapidRaw.sidecar(&development.path()), None);
        crate::developments::set_for(&image, &[]);
    }

    #[test]
    fn only_sidecars_written_by_darktable_or_with_a_history_count() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("DSCF0001.RAF");
        written(&image, b"raw").unwrap();
        assert_eq!(darktable_sidecar(&image), None, "pas de sidecar");
        let sidecar = dir.path().join("DSCF0001.RAF.xmp");
        // Écrit par tri seul (notes) : pas de développement.
        written(&sidecar, r#"<x:xmpmeta xmp:Rating="3"/>"#).unwrap();
        assert_eq!(darktable_sidecar(&image), None);
        written(&sidecar, r#"<rdf:Description darktable:history_end="0"/>"#).unwrap();
        assert_eq!(darktable_sidecar(&image), None, "historique vide");
        // Écrit par darktable, sans historique (importée, clone vierge) : son rendu par
        // défaut, différent de LibRaw (retour du 30/10/2026).
        written(
            &sidecar,
            r#"<rdf:Description darktable:xmp_version="5" darktable:history_end="0"/>"#,
        )
        .unwrap();
        assert_eq!(
            darktable_sidecar(&image),
            Some(sidecar.clone()),
            "par défaut"
        );
        written(&sidecar, r#"<rdf:Description darktable:history_end="27"/>"#).unwrap();
        assert_eq!(darktable_sidecar(&image), Some(sidecar.clone()));
        written(&sidecar, "<darktable:history_end>4</darktable:history_end>").unwrap();
        assert_eq!(
            darktable_sidecar(&image),
            Some(sidecar.clone()),
            "en élément"
        );
        let before = stamp(&sidecar);
        std::thread::sleep(Duration::from_millis(10));
        written(&sidecar, r#"<rdf:Description darktable:history_end="28"/>"#).unwrap();
        assert_ne!(stamp(&sidecar), before, "modifié : autre empreinte");
    }

    #[test]
    fn rating_an_image_does_not_redo_its_darktable_render() {
        // Revue du 28/10/2026 : `tri` écrit les notes dans le sidecar de darktable ; seuls
        // les champs de darktable comptent pour le rendu, pas les notes ni les couleurs.
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("DSCF0001.RAF.xmp");
        let xmp = |rating: u8, label: u8, exposure: &str| {
            format!(
                r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF><rdf:Description
 xmp:Rating="{rating}" darktable:history_end="2" darktable:xmp_version="5">
 <darktable:colorlabels><rdf:Seq><rdf:li>{label}</rdf:li></rdf:Seq></darktable:colorlabels>
 <darktable:history><rdf:Seq>
  <rdf:li darktable:num="0" darktable:operation="exposure" darktable:params="{exposure}"/>
 </rdf:Seq></darktable:history></rdf:Description></rdf:RDF></x:xmpmeta>"#
            )
        };
        let write = |text: String| {
            written(&sidecar, text).unwrap();
            // Une autre date : la mémoire de l'empreinte ne doit pas suffire.
            std::thread::sleep(Duration::from_millis(10));
        };
        write(xmp(0, 0, "aa"));
        let developed = render_stamp(&sidecar);
        write(xmp(4, 2, "aa"));
        assert_eq!(
            render_stamp(&sidecar),
            developed,
            "étoiles et couleur : même rendu"
        );
        write(xmp(4, 2, "bb"));
        assert_ne!(
            render_stamp(&sidecar),
            developed,
            "exposition changée : autre rendu"
        );
        // Un autre format (RapidRAW) : tout le contenu.
        let rr = dir.path().join("DSCF0001.RAF.rrdata");
        written(&rr, "{\"exposure\": 1}").unwrap();
        let before = render_stamp(&rr);
        std::thread::sleep(Duration::from_millis(10));
        written(&rr, "{\"exposure\": 2}").unwrap();
        assert_ne!(render_stamp(&rr), before);
    }

    #[test]
    fn the_developed_engine_follows_the_preference_then_the_latest() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("DSCF0001.RAF");
        written(&image, b"raw").unwrap();
        let all = [Engine::Darktable];
        // Sans développement : rien (l'image garde le rendu LibRaw).
        assert_eq!(resolve(&image, Preference::Auto, &all), None);
        written(
            dir.path().join("DSCF0001.RAF.xmp"),
            r#"<rdf:Description darktable:history_end="3"/>"#,
        )
        .unwrap();
        assert_eq!(
            resolve(&image, Preference::Auto, &all),
            Some(Engine::Darktable)
        );
        assert_eq!(
            resolve(&image, Preference::Engine(Engine::Darktable), &all),
            Some(Engine::Darktable)
        );
        // Logiciel pas installé : son développement ne compte pas.
        assert_eq!(resolve(&image, Preference::Auto, &[]), None);
    }

    #[test]
    fn the_preference_is_read_from_the_configuration() {
        let read = |text: &str| crate::settings::parse::<DevelopSettings>(text).map(|s| s.default);
        assert_eq!(read("").unwrap(), Preference::Auto);
        assert_eq!(
            read("default = \"darktable\"").unwrap(),
            Preference::Engine(Engine::Darktable)
        );
        assert!(read("default = \"photoshop\"").is_err());
        assert_eq!(Preference::Engine(Engine::Darktable).id(), "darktable");
        assert_eq!(Engine::from_id("darktable"), Some(Engine::Darktable));
    }

    #[test]
    fn rapidraw_sidecars_and_executable() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("DSCF0001.RAF");
        written(&image, b"raw").unwrap();
        assert_eq!(rapidraw_sidecar(&image), None);
        // Copie virtuelle seule : pas lue.
        written(dir.path().join("DSCF0001.RAF.a1b2.rrdata"), b"{}").unwrap();
        assert_eq!(rapidraw_sidecar(&image), None);
        let sidecar = dir.path().join("DSCF0001.RAF.rrdata");
        // Une note seule (sidecar écrit par `tri`) : pas un développement, rien à rendre.
        written(&sidecar, br#"{"rating": 3, "adjustments": null}"#).unwrap();
        assert_eq!(rapidraw_sidecar(&image), None, "note seule");
        written(
            &sidecar,
            br#"{"rating": 3, "adjustments": {"exposure": 0.5}}"#,
        )
        .unwrap();
        crate::cache::forget_stamps([sidecar.as_path()]);
        assert_eq!(rapidraw_sidecar(&image), Some(sidecar));
        assert_eq!(Engine::RapidRaw.sidecar(&image), rapidraw_sidecar(&image));
        assert_eq!(find_rapidraw(""), None, "désactivé");
        assert_eq!(find_rapidraw("/nulle/part/rapidraw"), None);
        let _ = find_rapidraw("auto"); // trouvé seulement s'il est installé
        let found = dir.path().join("RapidRAW.AppImage");
        written(&found, b"").unwrap();
        let cli = find_rapidraw(found.to_str().unwrap()).unwrap();
        assert!(
            cli.args.is_empty(),
            "l'AppImage reçoit `export` directement"
        );
    }

    #[test]
    fn with_two_developments_the_preference_wins_then_the_latest() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("DSCF0001.RAF");
        std::fs::write(&image, b"raw").unwrap();
        let both = [Engine::Darktable, Engine::RapidRaw];
        std::fs::write(
            dir.path().join("DSCF0001.RAF.xmp"),
            r#"<rdf:Description darktable:history_end="3"/>"#,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(
            dir.path().join("DSCF0001.RAF.rrdata"),
            br#"{"adjustments": {"exposure": 1}}"#,
        )
        .unwrap();
        assert_eq!(
            resolve(&image, Preference::Auto, &both),
            Some(Engine::RapidRaw),
            "le plus récent"
        );
        assert_eq!(
            resolve(&image, Preference::Engine(Engine::Darktable), &both),
            Some(Engine::Darktable),
            "on travaille dans darktable, on essaie RapidRAW"
        );
        assert_eq!(
            resolve(
                &image,
                Preference::Engine(Engine::RapidRaw),
                &[Engine::Darktable]
            ),
            Some(Engine::Darktable),
            "RapidRAW pas installé"
        );
    }

    /// Un faux RapidRAW (script) : il copie `jpeg` vers `--output`, ou écrit `message` et
    /// échoue.
    #[cfg(unix)]
    fn fake_rapidraw(dir: &Path, jpeg: Option<&Path>, message: &str) -> RapidRaw {
        use std::os::unix::fs::PermissionsExt;
        let script = dir.join("rapidraw");
        let body = match jpeg {
            Some(jpeg) => format!(
                "#!/bin/sh\n[ \"$1\" = export ] || exit 2\ncp '{}' \"$4\"\n",
                jpeg.display()
            ),
            None => format!("#!/bin/sh\necho '{message}' >&2\nexit 1\n"),
        };
        std::fs::write(&script, body).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        RapidRaw {
            cli: Cli::native(script),
            work_dir: dir.join("work"),
        }
    }

    /// Un faux `rawtherapee-cli` / `ART-cli` (script) : il copie `jpeg` dans le dossier
    /// `-o` sous le nom que le vrai donnerait (`<nom sans extension>.jpg`).
    #[cfg(unix)]
    fn fake_profile_tool(dir: &Path, engine: Engine, jpeg: &Path) -> ProfileTool {
        use std::os::unix::fs::PermissionsExt;
        let script = dir.join(profile_cli_names(engine).0);
        // `-c` reçoit un fichier qui existe (la photo, jamais le chemin virtuel d'un
        // développement : 31/10/2026).
        let body = format!(
            "#!/bin/sh\n[ \"$1\" = -o ] || exit 2\n[ \"$3\" = -p ] || exit 3\n[ -f \"$4\" ] || exit 4\n[ \"$7\" = -c ] || exit 7\n[ -f \"$8\" ] || exit 8\ncp '{}' \"$2/photo.jpg\"\n",
            jpeg.display()
        );
        std::fs::write(&script, body).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        ProfileTool {
            engine,
            cli: Cli::native(script),
            work_dir: dir.join("work"),
        }
    }

    #[test]
    #[cfg(unix)]
    fn rawtherapee_and_art_export_into_a_folder_then_tri_shrinks() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.jpg");
        let bitmap = crate::bitmap::Bitmap {
            width: 400,
            height: 300,
            data: vec![128; 400 * 300 * 3],
        };
        let bytes = crate::jpeg::encode(&bitmap, 90, crate::jpeg::Chroma::Half).unwrap();
        std::fs::write(&big, bytes).unwrap();
        let image = dir.path().join("photo.RAF");
        written(&image, b"raw").unwrap();
        let cancel = AtomicBool::new(false);
        for engine in [Engine::RawTherapee, Engine::Art] {
            let profile = crate::pp3::path(&image, engine.profile_extension().unwrap());
            std::fs::write(&profile, "[Exposure]\nCompensation=1\n").unwrap();
            let out = dir.path().join(format!("{}.jpg", engine.id()));
            let tool = fake_profile_tool(dir.path(), engine, &big);
            tool.render(&image, &profile, &out, 100, &cancel).unwrap();
            assert_eq!(image::image_dimensions(&out).unwrap(), (100, 75));
            // L'entrée du développement (chemin virtuel) : rendue depuis la photo.
            let development = crate::developments::Development {
                engine,
                copy: crate::developments::Copy::Main,
                photo: image.clone(),
                sidecar: profile.clone(),
            };
            crate::developments::set_for(&image, std::slice::from_ref(&development));
            let virtual_out = dir.path().join(format!("{}-dev.jpg", engine.id()));
            tool.render(&development.path(), &profile, &virtual_out, 100, &cancel)
                .unwrap_or_else(|e| panic!("rendu du développement : {e}"));
            assert!(virtual_out.exists());
            crate::developments::set_for(&image, &[]);
            assert!(
                tool.open_folder(dir.path()).is_none(),
                "pas d'interface à côté du script"
            );
            // Le profil : un développement ; réduit aux notes, non.
            assert_eq!(engine.sidecar(&image), Some(profile.clone()));
            std::fs::write(&profile, "[General]\nRank=3\n").unwrap();
            crate::cache::forget_stamps([profile.as_path()]);
            assert_eq!(engine.sidecar(&image), None, "notes seules : rien à rendre");
        }
    }

    #[test]
    #[cfg(unix)]
    fn rapidraw_exports_then_tri_shrinks() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.jpg");
        let bitmap = crate::bitmap::Bitmap {
            width: 400,
            height: 300,
            data: vec![128; 400 * 300 * 3],
        };
        let bytes = crate::jpeg::encode(&bitmap, 90, crate::jpeg::Chroma::Half).unwrap();
        std::fs::write(&big, bytes).unwrap();
        let out = dir.path().join("out.jpg");
        let cancel = AtomicBool::new(false);
        let image = dir.path().join("a.RAF");
        let rr = fake_rapidraw(dir.path(), Some(&big), "");
        rr.render(&image, &image, &out, 100, &cancel).unwrap();
        assert_eq!(image::image_dimensions(&out).unwrap(), (100, 75));
        // Image toute noire (interface de RapidRAW ouverte) : refusée, rien d'écrit.
        let black = dir.path().join("black.jpg");
        let bitmap = crate::bitmap::Bitmap {
            width: 40,
            height: 30,
            data: vec![0; 40 * 30 * 3],
        };
        let bytes = crate::jpeg::encode(&bitmap, 90, crate::jpeg::Chroma::Half).unwrap();
        std::fs::write(&black, bytes).unwrap();
        let dark = dir.path().join("dark.jpg");
        let rr = fake_rapidraw(dir.path(), Some(&black), "");
        let err = rr.render(&image, &image, &dark, 100, &cancel).unwrap_err();
        assert!(err.to_string().contains("RapidRAW"), "{err}");
        assert!(!dark.exists());
        // Interface de RapidRAW ouverte : la raison, en clair.
        let rr = fake_rapidraw(
            dir.path(),
            None,
            "Error: Headless export cannot be attached to an already running GUI instance.",
        );
        let err = rr.render(&image, &image, &out, 100, &cancel).unwrap_err();
        assert!(err.to_string().contains("RapidRAW"), "{err}");
        let rr = fake_rapidraw(dir.path(), None, "boum");
        let err = rr.render(&image, &image, &out, 100, &cancel).unwrap_err();
        assert!(err.to_string().contains("boum"), "{err}");
    }

    #[test]
    fn darktable_is_found_or_cleanly_absent() {
        assert_eq!(find_darktable(""), None, "désactivé");
        assert_eq!(find_darktable("/nulle/part/darktable-cli"), None);
        // « auto » : trouvé seulement s'il est installé ; jamais d'erreur.
        if let Some(cli) = find_darktable("auto") {
            assert!(cli.args.is_empty() == cli.program.is_file(), "{cli:?}");
        }
        // Flatpak : arguments devant ceux de tri.
        let flatpak = Cli {
            program: "flatpak".into(),
            args: vec![
                "run".into(),
                "--command=darktable-cli".into(),
                FLATPAK_APP.into(),
            ],
        };
        // AppImage : le binaire voulu en premier argument.
        let dir = tempfile::tempdir().unwrap();
        let apps = dir.path().join("AppImages");
        std::fs::create_dir(&apps).unwrap();
        assert_eq!(appimage(Some(dir.path()), "darktable"), None);
        std::fs::write(apps.join("autre.AppImage"), b"").unwrap();
        std::fs::write(apps.join("darktable.appimage"), b"").unwrap();
        let found = appimage(Some(dir.path()), "darktable").unwrap();
        assert_eq!(found, apps.join("darktable.appimage"));
        assert_eq!(cli_for(found).args, ["darktable-cli"]);
        assert!(
            cli_for(PathBuf::from("/usr/bin/darktable-cli"))
                .args
                .is_empty()
        );
        let cmd = flatpak.command();
        let args: Vec<_> = cmd.get_args().collect();
        assert_eq!(args, ["run", "--command=darktable-cli", FLATPAK_APP]);
    }

    /// Rendu réel : seulement si darktable est installé et `TRI_TEST_DARKTABLE_RAW` désigne
    /// un RAW développé (avec son sidecar), hors du dépôt.
    #[test]
    #[ignore]
    fn darktable_renders_a_developed_raw() {
        let cli = find_darktable("auto").expect("darktable-cli installé");
        let original = PathBuf::from(std::env::var("TRI_TEST_DARKTABLE_RAW").expect("RAW de test"));
        // Sur une copie : jamais les fichiers de l'utilisateur.
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join(original.file_name().unwrap());
        std::fs::copy(&original, &raw).unwrap();
        let mut name = original.file_name().unwrap().to_os_string();
        name.push(".xmp");
        std::fs::copy(original.with_file_name(&name), raw.with_file_name(&name)).unwrap();
        let sidecar = darktable_sidecar(&raw).expect("développé dans darktable");
        let before = std::fs::read(&sidecar).unwrap();
        let out = dir.path().join("out.jpg");
        let dt = Darktable {
            cli,
            config_dir: dir.path().join("cfg"),
            high_quality: false,
        };
        Developer::render(&dt, &raw, &sidecar, &out, 800, &AtomicBool::new(false)).unwrap();
        assert_eq!(std::fs::read(&sidecar).unwrap(), before, "sidecar intact");
        let (w, h) = image::image_dimensions(&out).unwrap();
        assert_eq!(w.max(h), 800);
    }

    #[test]
    fn the_developed_filter_keeps_what_the_checked_software_developed() {
        use crate::filter::{Filter, ungrouped};
        use crate::media::{MediaFile, MediaKind};
        let dir = tempfile::tempdir().unwrap();
        let file = |name: &str, kind| {
            let path = dir.path().join(name);
            std::fs::write(&path, b"x").unwrap();
            MediaFile { path, kind }
        };
        // a : darktable ; b : RapidRAW puis darktable ; c : un sidecar de notes seulement ;
        // d : JPEG.
        let a = file("a.RAF", MediaKind::Raw);
        let b = file("b.RAF", MediaKind::Raw);
        let c = file("c.RAF", MediaKind::Raw);
        let d = file("d.JPG", MediaKind::Flat);
        std::fs::write(dir.path().join("a.RAF.xmp"), sidecar(2, &[])).unwrap();
        std::fs::write(
            dir.path().join("b.RAF.rrdata"),
            br#"{"adjustments": {"exposure": 1}}"#,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(dir.path().join("b.RAF.xmp"), sidecar(2, &[])).unwrap();
        std::fs::write(dir.path().join("c.RAF.xmp"), sidecar(0, &[])).unwrap();
        let all = ungrouped(vec![a.clone(), b.clone(), c.clone(), d]);
        let both = [Engine::Darktable, Engine::RapidRaw];
        let rule = |engines: &[Engine]| DevelopedRule {
            engines: engines.to_vec(),
        };
        let kept = |rule: &DevelopedRule, available: &[Engine]| -> Vec<MediaFile> {
            DevelopedFilter { rule, available }
                .apply(all.clone())
                .concat()
        };
        let dt_rr = rule(&both);
        assert_eq!(kept(&dt_rr, &both), [a.clone(), b.clone()]);
        assert_eq!(
            kept(&rule(&[Engine::Darktable]), &both),
            [a.clone(), b.clone()]
        );
        assert_eq!(
            kept(&rule(&[Engine::RapidRaw]), &both),
            std::slice::from_ref(&b)
        );
        // Aucun coché : les RAW sans développement (rendu LibRaw).
        assert_eq!(kept(&rule(&[]), &both), [c]);
        // Logiciel pas installé : ses développements ne comptent pas, comme à l'affichage.
        assert!(kept(&rule(&[Engine::RapidRaw]), &[Engine::Darktable]).is_empty());
        // Montré : le plus récent des cochés.
        assert_eq!(dt_rr.engine_for(&b.path, &both), Some(Engine::Darktable));
        assert_eq!(
            rule(&[Engine::RapidRaw]).engine_for(&b.path, &both),
            Some(Engine::RapidRaw)
        );
        assert_eq!(rule(&[]).engine_for(&a.path, &both), None);
    }
}
