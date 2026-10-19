//! Rendu par un logiciel de développement (idée de l'utilisateur, 19/10/2026) : voir dans
//! `tri` l'image telle qu'on l'a développée ailleurs, darktable d'abord.
//!
//! Plugins compilés avec `tri` (un trait, une liste, comme les autres plugins), qui pilotent
//! un **programme externe** en processus séparé : `darktable-cli` produit l'image avec tout
//! l'historique de développement du sidecar XMP. Le programme externe ne peut pas faire
//! tomber `tri`, et sa licence reste la sienne. RawTherapee (`rawtherapee-cli`, profils
//! `.pp3`) et ART suivraient le même modèle. Voir `doc/develop.md`.
//!
//! Rien n'est écrit chez l'utilisateur : `darktable-cli` reçoit sa propre configuration
//! (dans le cache de `tri`) et une bibliothèque en mémoire, le sidecar n'est que lu.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

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
}

impl Default for DevelopSettings {
    fn default() -> Self {
        Self {
            darktable: "auto".into(),
            high_quality: false,
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
        let mut cmd = Command::new(&self.program);
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
    let installed = Command::new("flatpak")
        .args(["info", FLATPAK_APP])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    installed.then(|| Cli {
        program: PathBuf::from("flatpak"),
        args: vec![
            "run".into(),
            "--command=darktable-cli".into(),
            FLATPAK_APP.into(),
        ],
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
    appimage(home.as_deref()).map(cli_for).or_else(flatpak)
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

/// AppImage de darktable aux emplacements habituels (`~/AppImages` de Gear Lever ou
/// AppImageLauncher, `~/Applications`, `~/.local/bin`, `~/bin`, téléchargements, `/opt`) :
/// la plus récente. Constaté le 19/10/2026 sur la machine Linux Mint de l'utilisateur
/// (`~/AppImages/darktable.appimage`), format que darktable propose lui-même pour Linux.
fn appimage(home: Option<&Path>) -> Option<PathBuf> {
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
                    .is_some_and(|n| n.to_string_lossy().to_lowercase().contains("darktable"))
        })
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
}

/// Sidecar darktable de `image` (`IMAGE.RAF.xmp`), s'il porte un vrai développement : un
/// historique non vide (`darktable:history_end` > 0). Un sidecar écrit par `tri` seul
/// (notes) ou par un autre logiciel n'en a pas.
pub fn darktable_sidecar(image: &Path) -> Option<PathBuf> {
    let mut name = image.file_name()?.to_os_string();
    name.push(".xmp");
    let sidecar = image.with_file_name(name);
    let text = std::fs::read_to_string(&sidecar).ok()?;
    (history_end(&text)? > 0).then_some(sidecar)
}

/// Valeur de `darktable:history_end`, en attribut ou en élément.
fn history_end(xmp: &str) -> Option<u32> {
    let tag = "darktable:history_end";
    let at = xmp.find(tag)? + tag.len();
    let rest = xmp[at..].trim_start_matches(['=', '"', '>', ' ']);
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Empreinte d'un sidecar (taille, date) : dans la clé de cache, pour qu'un développement
/// modifié dans darktable refasse le rendu.
pub fn stamp(sidecar: &Path) -> String {
    match std::fs::metadata(sidecar) {
        Ok(meta) => {
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            format!("{}:{modified}", meta.len())
        }
        Err(_) => "absent".into(),
    }
}

/// Erreur d'un rendu.
#[derive(Debug)]
pub enum DevelopError {
    /// Pas de développement darktable pour cette image (sidecar absent ou sans historique).
    NotDeveloped,
    /// `darktable-cli` introuvable ou désactivé.
    Unavailable,
    /// Rendu annulé (l'image n'est plus affichée).
    Cancelled,
    /// Échec du programme (dernière ligne de son message d'erreur).
    Failed(String),
    /// Échec hors de darktable (export par LibRaw ou du JPEG boîtier), message tel quel.
    Other(String),
}

impl std::fmt::Display for DevelopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DevelopError::NotDeveloped => write!(f, "{}", crate::t!("develop-none")),
            DevelopError::Unavailable => write!(f, "{}", crate::t!("develop-unavailable")),
            DevelopError::Cancelled => write!(f, "annulé"),
            DevelopError::Failed(e) => {
                write!(f, "{}", crate::t!("develop-failed", error = e.as_str()))
            }
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

impl Darktable {
    /// Développe `image` d'après son `sidecar`, dans `out` (JPEG), le côté long ramené à
    /// `side`. Bibliothèque en mémoire, sidecars jamais écrits ; `cancel` arrête le
    /// programme.
    pub fn render(
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
    std::fs::create_dir_all(&dt.config_dir).map_err(|e| DevelopError::Failed(e.to_string()))?;
    let _ = std::fs::remove_file(out);
    let side = side.to_string();
    let mut child = dt
        .cli
        .command()
        .arg(image)
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
        .map_err(|e| DevelopError::Failed(e.to_string()))?;
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
                    last.unwrap_or("darktable-cli").trim().to_string(),
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(DevelopError::Failed(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_sidecars_with_a_history_count() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("DSCF0001.RAF");
        std::fs::write(&image, b"raw").unwrap();
        assert_eq!(darktable_sidecar(&image), None, "pas de sidecar");
        let sidecar = dir.path().join("DSCF0001.RAF.xmp");
        // Écrit par tri seul (notes) : pas de développement.
        std::fs::write(&sidecar, r#"<x:xmpmeta xmp:Rating="3"/>"#).unwrap();
        assert_eq!(darktable_sidecar(&image), None);
        std::fs::write(&sidecar, r#"<rdf:Description darktable:history_end="0"/>"#).unwrap();
        assert_eq!(darktable_sidecar(&image), None, "historique vide");
        std::fs::write(&sidecar, r#"<rdf:Description darktable:history_end="27"/>"#).unwrap();
        assert_eq!(darktable_sidecar(&image), Some(sidecar.clone()));
        std::fs::write(&sidecar, "<darktable:history_end>4</darktable:history_end>").unwrap();
        assert_eq!(
            darktable_sidecar(&image),
            Some(sidecar.clone()),
            "en élément"
        );
        let before = stamp(&sidecar);
        std::thread::sleep(Duration::from_millis(10));
        std::fs::write(&sidecar, r#"<rdf:Description darktable:history_end="28"/>"#).unwrap();
        assert_ne!(stamp(&sidecar), before, "modifié : autre empreinte");
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
        assert_eq!(appimage(Some(dir.path())), None);
        std::fs::write(apps.join("autre.AppImage"), b"").unwrap();
        std::fs::write(apps.join("darktable.appimage"), b"").unwrap();
        let found = appimage(Some(dir.path())).unwrap();
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
        dt.render(&raw, &sidecar, &out, 800, &AtomicBool::new(false))
            .unwrap();
        assert_eq!(std::fs::read(&sidecar).unwrap(), before, "sidecar intact");
        let (w, h) = image::image_dimensions(&out).unwrap();
        assert_eq!(w.max(h), 800);
    }
}
