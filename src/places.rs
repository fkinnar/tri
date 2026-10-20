//! Emplacements à proposer pour choisir un dossier : volumes montés (cartes SD, disques,
//! NAS), dossier personnel, racine ; et lecture des sous-dossiers d'un dossier.
//!
//! Rien n'est parcouru d'avance : l'interface lit un dossier quand on le déplie, ce qui
//! reste instantané même sur un NAS. Voir `doc/en/places.md`.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Nature d'un emplacement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceKind {
    /// Volume monté : carte SD, disque externe ou interne, appareil photo, téléphone.
    Volume,
    /// Partage réseau monté (SMB, NFS, SSHFS, WebDAV…).
    Network,
    /// Dossier personnel, ou un de ses dossiers habituels (Bureau, Documents,
    /// Téléchargements, Images).
    Home,
    /// Racine du système de fichiers (arborescence complète).
    Root,
    /// Dossier favori, choisi par l'utilisateur (voir [`favorites`]).
    Favorite,
}

/// Section du panneau des dossiers, dans l'ordre d'affichage (demandé le 20/10/2026) :
/// disques amovibles, dossier personnel, favoris, puis seulement la racine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    /// Volumes montés (cartes, disques, appareils).
    Drives,
    /// Partages réseau montés.
    Network,
    /// Dossier personnel et ses dossiers habituels.
    Personal,
    /// Favoris de l'utilisateur.
    Favorites,
    /// Racine du système de fichiers.
    System,
}

impl PlaceKind {
    /// Section où se range un emplacement de cette nature.
    pub fn section(self) -> Section {
        match self {
            PlaceKind::Volume => Section::Drives,
            PlaceKind::Network => Section::Network,
            PlaceKind::Home => Section::Personal,
            PlaceKind::Favorite => Section::Favorites,
            PlaceKind::Root => Section::System,
        }
    }
}

/// Un emplacement : point de départ d'une arborescence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    /// Nom affiché (nom du volume, « Dossier personnel »...).
    pub name: String,
    /// Dossier.
    pub path: PathBuf,
    /// Nature.
    pub kind: PlaceKind,
}

/// Réglages des emplacements (section `[places]` du fichier de configuration) : lesquels
/// masquer, et la surveillance des volumes.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlaceSettings {
    /// Masquer les volumes de sauvegarde Time Machine, reconnus à leur contenu (pas à leur
    /// nom, choisi par chacun) : `backup_manifest.plist` à la racine (disque APFS) ou
    /// `Backups.backupdb` (disque HFS, instantanés locaux).
    pub hide_backups: bool,
    /// Motifs d'emplacements à masquer, comparés sans tenir compte de la casse au nom
    /// (`"Backup"`) ou au chemin (`"/mnt/*"`) ; `*` : n'importe quelle suite de caractères,
    /// `?` : un caractère.
    pub exclude: Vec<String>,
    /// Volumes **fixés** : disques externes toujours branchés (disque de travail, de
    /// sauvegarde), qu'on ne propose pas d'éjecter, pour éviter un accident (idée de
    /// l'utilisateur, 17/10/2026). Mêmes motifs que `exclude`.
    pub fixed: Vec<String>,
    /// Intervalle de surveillance des volumes (ms) : un disque branché ou débranché
    /// apparaît ou disparaît de lui-même (voir [`Monitor`]). 0 : pas de surveillance.
    pub watch_ms: u64,
}

impl Default for PlaceSettings {
    fn default() -> Self {
        Self {
            hide_backups: true,
            exclude: Vec::new(),
            fixed: Vec::new(),
            watch_ms: 2000,
        }
    }
}

impl PlaceSettings {
    /// Vrai si `place` doit être masqué.
    pub fn hides(&self, place: &Place) -> bool {
        matches_any(&self.exclude, place)
            || (self.hide_backups && place.kind == PlaceKind::Volume && is_backup(&place.path))
    }

    /// Vrai si `place` est un volume qu'on peut proposer d'éjecter : un volume, pas fixé.
    pub fn ejectable(&self, place: &Place) -> bool {
        place.kind == PlaceKind::Volume && !matches_any(&self.fixed, place)
    }
}

/// Nom affiché d'un dossier : son dernier composant, sinon le chemin entier (`/`, `C:\\`).
fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// `place` correspond-il à l'un des `patterns` (par son nom ou son chemin) ?
fn matches_any(patterns: &[String], place: &Place) -> bool {
    let path = place.path.to_string_lossy();
    patterns
        .iter()
        .any(|pattern| wildcard(pattern, &place.name) || wildcard(pattern, &path))
}

/// Vrai si `dir` est un volume de sauvegarde Time Machine (voir [`PlaceSettings`]).
pub fn is_backup(dir: &Path) -> bool {
    dir.join("backup_manifest.plist").is_file() || dir.join("Backups.backupdb").is_dir()
}

/// `text` correspond-il à `pattern` (`*` : n'importe quelle suite, `?` : un caractère),
/// sans tenir compte de la casse ?
fn wildcard(pattern: &str, text: &str) -> bool {
    fn go(p: &[char], t: &[char]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some('*'), _) => go(&p[1..], t) || (!t.is_empty() && go(p, &t[1..])),
            (Some('?'), Some(_)) => go(&p[1..], &t[1..]),
            (Some(a), Some(b)) => a == b && go(&p[1..], &t[1..]),
            _ => false,
        }
    }
    let lower = |s: &str| s.to_lowercase().chars().collect::<Vec<char>>();
    go(&lower(pattern), &lower(text))
}

/// Emplacements de cette machine, sauf ceux que `filter` masque.
pub fn places_filtered(filter: &PlaceSettings) -> Vec<Place> {
    places().into_iter().filter(|p| !filter.hides(p)).collect()
}

/// Emplacements de cette machine : volumes montés, partages réseau, dossier personnel et
/// ses dossiers habituels, racine(s).
pub fn places() -> Vec<Place> {
    let os = std::env::consts::OS;
    let env = |name: &str| std::env::var_os(name);
    let mut places = volumes_in(&volume_dirs(os, env));
    if os == "windows" {
        places.extend(windows_drives());
    }
    // Partages réseau : ceux montés avec les volumes changent de section, les autres
    // (montés ailleurs, `/etc/fstab`, GVFS) s'ajoutent.
    let mut network = network_places(os);
    for place in &mut places {
        if let Some(i) = network.iter().position(|n| n.path == place.path) {
            place.kind = PlaceKind::Network;
            network.remove(i);
        }
    }
    places.extend(network);
    if let Some(home) = home_dir(os, env) {
        let user_dirs = user_dirs(
            os,
            &home,
            &fs::read_to_string(home.join(".config/user-dirs.dirs")).unwrap_or_default(),
        );
        places.push(Place {
            name: crate::t!("places-home"),
            path: home,
            kind: PlaceKind::Home,
        });
        places.extend(user_dirs.into_iter().filter(|p| p.path.is_dir()));
    }
    if std::env::consts::OS != "windows" {
        places.push(Place {
            name: crate::t!("places-root"),
            path: PathBuf::from("/"),
            kind: PlaceKind::Root,
        });
    }
    places
}

/// Dossiers habituels du dossier personnel `home`, comme la barre latérale du Finder
/// (demandé le 20/10/2026) : Bureau, Documents, Téléchargements, Images. Sous Linux,
/// ceux de `user-dirs.dirs` (`xdg-user-dirs` : noms de la langue du système, « Bureau »,
/// « Images »…), contenu de ce fichier passé en `xdg` ; sinon les noms anglais. Le
/// dossier personnel lui-même n'est jamais repris. Existence non vérifiée.
fn user_dirs(os: &str, home: &Path, xdg: &str) -> Vec<Place> {
    const DIRS: [(&str, &str, &str); 4] = [
        ("DESKTOP", "Desktop", "places-desktop"),
        ("DOCUMENTS", "Documents", "places-documents"),
        ("DOWNLOAD", "Downloads", "places-downloads"),
        ("PICTURES", "Pictures", "places-pictures"),
    ];
    // XDG_PICTURES_DIR="$HOME/Images" ; un chemin absolu est permis.
    let from_xdg = |key: &str| {
        xdg.lines().find_map(|line| {
            let value = line.trim().strip_prefix(&format!("XDG_{key}_DIR="))?;
            let value = value.trim().trim_matches('"');
            Some(match value.strip_prefix("$HOME") {
                Some(rest) => home.join(rest.trim_start_matches('/')),
                None => PathBuf::from(value),
            })
        })
    };
    DIRS.iter()
        .filter_map(|(key, english, label)| {
            let path = match os {
                "macos" | "windows" => home.join(english),
                _ => from_xdg(key).unwrap_or_else(|| home.join(english)),
            };
            (path != home && path.is_absolute()).then(|| Place {
                name: crate::t!(*label),
                path,
                kind: PlaceKind::Home,
            })
        })
        .collect()
}

/// Types de systèmes de fichiers réseau, tels que les écrivent `mount` (macOS) et
/// `/proc/mounts` (Linux).
const NETWORK_FS: [&str; 13] = [
    "smbfs",
    "afpfs",
    "nfs",
    "nfs4",
    "webdav",
    "cifs",
    "smb3",
    "fuse.sshfs",
    "sshfs",
    "fuse.rclone",
    "davfs",
    "fuse.davfs2",
    "ftp",
];

/// Partages réseau montés : points de montage d'un système de fichiers réseau (sortie de
/// `mount` sur macOS, `/proc/mounts` sous Linux), et sous Linux les montages GVFS du
/// gestionnaire de fichiers (`/run/user/<uid>/gvfs` ; un appareil photo ou un téléphone
/// y est un volume). Rien sous Windows (lecteurs réseau : lettres de lecteur).
fn network_places(os: &str) -> Vec<Place> {
    let mut places: Vec<Place> = match os {
        "macos" => std::process::Command::new("/sbin/mount")
            .output()
            .map(|o| network_mounts_macos(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default(),
        "windows" => Vec::new(),
        _ => network_mounts_linux(&fs::read_to_string("/proc/mounts").unwrap_or_default()),
    }
    .into_iter()
    .map(|path| Place {
        name: folder_name(&path),
        path,
        kind: PlaceKind::Network,
    })
    .collect();
    #[cfg(unix)]
    if os != "macos" {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = fs::metadata("/proc/self") {
            let gvfs = PathBuf::from(format!("/run/user/{}/gvfs", meta.uid()));
            places.extend(gvfs_places(&gvfs));
        }
    }
    places.sort_by_key(|p| p.name.to_lowercase());
    places
}

/// Points de montage réseau dans la sortie de `mount` (macOS) :
/// `//moi@nas/photo on /Volumes/photo (smbfs, nodev, nosuid, mounted by moi)`.
fn network_mounts_macos(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|line| {
            let (rest, options) = line.rsplit_once(" (")?;
            let fs_type = options.split([',', ')']).next()?.trim();
            let (_, point) = rest.split_once(" on ")?;
            NETWORK_FS.contains(&fs_type).then(|| PathBuf::from(point))
        })
        .collect()
}

/// Points de montage réseau dans `/proc/mounts` (Linux ; espaces écrits `\040`).
fn network_mounts_linux(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let (_, point, fs_type) = (fields.next()?, fields.next()?, fields.next()?);
            NETWORK_FS
                .contains(&fs_type)
                .then(|| PathBuf::from(point.replace("\\040", " ")))
        })
        .collect()
}

/// Montages GVFS (Nautilus, Nemo…) dans `dir` : `smb-share:server=nas,share=photo` devient
/// le partage réseau « photo (nas) » ; `mtp:host=…` (téléphone) et `gphoto2:host=…`
/// (appareil photo) sont des volumes. Linux et autres Unix seulement (testé partout).
#[cfg_attr(not(unix), allow(dead_code))]
fn gvfs_places(dir: &Path) -> Vec<Place> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let raw = e.file_name().to_string_lossy().into_owned();
            let (scheme, params) = raw.split_once(':')?;
            let value = |key: &str| {
                params
                    .split(',')
                    .find_map(|p| p.strip_prefix(key)?.strip_prefix('='))
                    .map(|v| v.replace("%20", " "))
            };
            let host = value("server")
                .or_else(|| value("host"))
                .unwrap_or_default();
            let (name, kind) = match (scheme, value("share")) {
                ("mtp" | "gphoto2", _) => (host, PlaceKind::Volume),
                (_, Some(share)) => (format!("{share} ({host})"), PlaceKind::Network),
                _ => (host, PlaceKind::Network),
            };
            Some(Place {
                name: if name.is_empty() { raw.clone() } else { name },
                path: e.path(),
                kind,
            })
        })
        .collect()
}

/// Dossier personnel (`HOME`, ou `USERPROFILE` sous Windows), s'il est absolu.
fn home_dir(os: &str, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let name = if os == "windows" {
        "USERPROFILE"
    } else {
        "HOME"
    };
    env(name).map(PathBuf::from).filter(|p| p.is_absolute())
}

/// Dossiers dont chaque sous-dossier est un volume monté, selon l'OS : `/Volumes` (macOS) ;
/// `/media/<utilisateur>`, `/run/media/<utilisateur>` (montage automatique des
/// distributions) et `/mnt` (Linux et autres Unix). Aucun sous Windows (lettres de
/// lecteur, voir [`windows_drives`]).
fn volume_dirs(os: &str, env: impl Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    match os {
        "windows" => Vec::new(),
        "macos" => vec![PathBuf::from("/Volumes")],
        _ => {
            let mut dirs = Vec::new();
            if let Some(user) = env("USER").filter(|u| !u.is_empty()) {
                dirs.push(Path::new("/media").join(&user));
                dirs.push(Path::new("/run/media").join(&user));
            }
            dirs.push(PathBuf::from("/mnt"));
            dirs
        }
    }
}

/// Volumes : sous-dossiers de `dirs`, triés par nom. Les liens symboliques sont écartés
/// (sur macOS, `/Volumes/Macintosh HD` est un lien vers `/`, déjà proposé comme racine) ;
/// un dossier absent ou illisible ne donne rien.
fn volumes_in(dirs: &[PathBuf]) -> Vec<Place> {
    let mut volumes = Vec::new();
    for dir in dirs {
        let mut found: Vec<Place> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| Place {
                name: e.file_name().to_string_lossy().into_owned(),
                path: e.path(),
                kind: PlaceKind::Volume,
            })
            .filter(|p| !is_hidden(&p.path))
            .collect();
        found.sort_by_key(|p| p.name.to_lowercase());
        volumes.extend(found);
    }
    volumes
}

/// Lettres de lecteur présentes (`C:\`...), sous Windows.
fn windows_drives() -> Vec<Place> {
    (b'A'..=b'Z')
        .map(|letter| format!("{}:\\", letter as char))
        .filter(|root| Path::new(root).is_dir())
        .map(|root| Place {
            name: root.clone(),
            path: PathBuf::from(root),
            kind: PlaceKind::Volume,
        })
        .collect()
}

/// Fichier des favoris, dans le dossier de données : un chemin par ligne, dans l'ordre
/// d'affichage.
const FAVORITES_FILE: &str = "favorites";

/// Dossiers favoris, dans l'ordre où ils ont été ajoutés, absents compris (carte retirée,
/// disque débranché : le favori revient quand le volume est remonté).
pub fn favorites(data: &Path) -> Vec<PathBuf> {
    fs::read_to_string(data.join(FAVORITES_FILE))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

fn write_favorites(data: &Path, list: &[PathBuf]) -> io::Result<()> {
    let text: String = list
        .iter()
        .map(|p| p.to_string_lossy().into_owned() + "\n")
        .collect();
    fs::create_dir_all(data)?;
    fs::write(data.join(FAVORITES_FILE), text)
}

/// Ajoute `dir` aux favoris (à la fin), ou l'en retire si `on` est faux. Renvoie la
/// nouvelle liste.
pub fn set_favorite(data: &Path, dir: &Path, on: bool) -> io::Result<Vec<PathBuf>> {
    let mut list = favorites(data);
    list.retain(|p| p != dir);
    if on {
        list.push(dir.to_path_buf());
    }
    write_favorites(data, &list)?;
    Ok(list)
}

/// Dossier `from` renommé ou déplacé en `to` : les favoris qui sont `from` ou dedans
/// suivent. Renvoie la nouvelle liste (inchangée si aucun favori n'est concerné).
pub fn rename_favorites(data: &Path, from: &Path, to: &Path) -> io::Result<Vec<PathBuf>> {
    let list = favorites(data);
    let moved: Vec<PathBuf> = list
        .iter()
        .map(|p| {
            p.strip_prefix(from)
                .map_or_else(|_| p.clone(), |rest| to.join(rest))
        })
        .map(|p| p.components().collect())
        .collect();
    if moved != list {
        write_favorites(data, &moved)?;
    }
    Ok(moved)
}

/// Vrai si le nom de `path` commence par un point (dossier caché sous Unix).
pub fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
}

/// Sous-dossiers de `dir`, triés par nom sans tenir compte de la casse ; les dossiers
/// cachés seulement si `hidden`. Un lien symbolique vers un dossier est gardé (il est
/// suivi). Les entrées illisibles sont ignorées ; erreur si `dir` lui-même est illisible.
pub fn subfolders(dir: &Path, hidden: bool) -> io::Result<Vec<PathBuf>> {
    let mut folders: Vec<PathBuf> = fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && (hidden || !is_hidden(p)))
        .collect();
    folders.sort_by_key(|p| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    });
    Ok(folders)
}

/// Dossiers de `path` à déplier pour l'atteindre depuis `root` (de `root` exclu à `path`
/// inclus), ou `None` si `path` n'est pas sous `root`.
pub fn ancestors_from(root: &Path, path: &Path) -> Option<Vec<PathBuf>> {
    let rest = path.strip_prefix(root).ok()?;
    let mut current = root.to_path_buf();
    let mut chain = Vec::new();
    for part in rest.components() {
        current.push(part);
        chain.push(current.clone());
    }
    Some(chain)
}

/// Surveillance des volumes, dans un fil d'exécution à part : toutes les `watch_ms`, la
/// liste des emplacements est relue, et `on_change` est appelé quand elle change (disque
/// branché ou débranché). Relire `/Volumes` ne coûte presque rien ; le fil à part protège
/// l'appelant d'un volume réseau qui ne répond plus. Pas de notifications du système :
/// une dépendance et trois comportements différents (macOS, Linux, Windows) pour un gain
/// de deux secondes au plus. Arrêtée quand elle est détruite.
#[derive(Debug)]
pub struct Monitor {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Monitor {
    /// Démarre la surveillance (rien si `settings.watch_ms` vaut 0). La liste de départ est
    /// celle du moment : seuls les changements suivants sont signalés.
    pub fn start(
        settings: PlaceSettings,
        on_change: impl Fn(Vec<Place>) + Send + 'static,
    ) -> Monitor {
        let interval = std::time::Duration::from_millis(settings.watch_ms);
        Monitor::with_source(interval, move || places_filtered(&settings), on_change)
    }

    /// Surveillance d'une liste d'emplacements quelconque, relue par `source` toutes les
    /// `interval` (rien si `interval` est nul). Sert aux tests, et à surveiller autre chose
    /// que les volumes de la machine.
    pub fn with_source(
        interval: std::time::Duration,
        source: impl Fn() -> Vec<Place> + Send + 'static,
        on_change: impl Fn(Vec<Place>) + Send + 'static,
    ) -> Monitor {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        if interval.is_zero() {
            return Monitor { stop, thread: None };
        }
        let flag = stop.clone();
        // Liste de départ lue ici, avant le fil : un volume branché juste après l'appel est
        // bien signalé comme un changement.
        let mut last = source();
        let thread = std::thread::Builder::new()
            .name("tri-volumes".into())
            .spawn(move || {
                loop {
                    std::thread::park_timeout(interval);
                    if flag.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    let now = source();
                    if now != last {
                        on_change(now.clone());
                        last = now;
                    }
                }
            })
            .ok();
        Monitor { stop, thread }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

/// Éjecte le volume monté en `path` (carte SD, disque externe), avec l'outil du système :
/// `diskutil eject` (macOS) ; `gio mount -e` (Linux, l'outil de l'environnement de
/// bureau, comme le gestionnaire de fichiers), sinon `udisksctl unmount` puis
/// `power-off` ; Windows : verbe « Éjecter » de l'explorateur, via PowerShell (non testé).
/// Bloquant (une à quelques secondes) : à appeler hors du fil de l'interface. En cas de
/// refus (disque interne, fichier ouvert sur le volume), le message du système est
/// renvoyé.
pub fn eject(path: &Path) -> Result<(), String> {
    match std::env::consts::OS {
        "macos" => run("diskutil", &["eject".as_ref(), path.as_os_str()]),
        "windows" => {
            let drive = path.to_string_lossy();
            let drive = drive.trim_end_matches('\\').replace('\'', "''");
            let script = format!(
                "(New-Object -comObject Shell.Application).Namespace(17).ParseName('{drive}').InvokeVerb('Eject')"
            );
            run(
                "powershell",
                &["-NoProfile".as_ref(), "-Command".as_ref(), script.as_ref()],
            )
        }
        _ => {
            let gio = run("gio", &["mount".as_ref(), "-e".as_ref(), path.as_os_str()]);
            if gio.is_ok() {
                return gio;
            }
            let mounts = fs::read_to_string("/proc/mounts").unwrap_or_default();
            let Some(device) = device_of(&mounts, path) else {
                return gio;
            };
            run(
                "udisksctl",
                &["unmount".as_ref(), "-b".as_ref(), device.as_ref()],
            )?;
            // Couper l'alimentation : le support peut alors être retiré sans risque. Un
            // échec ici (lecteur de cartes intégré) n'empêche pas le retrait.
            let _ = run(
                "udisksctl",
                &["power-off".as_ref(), "-b".as_ref(), device.as_ref()],
            );
            Ok(())
        }
    }
}

/// Lance `program` avec `args` ; en cas d'échec, sa sortie d'erreur (ou standard).
fn run(program: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{program} : {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).trim().to_string();
    let message = Some(text(&output.stderr))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| text(&output.stdout));
    Err(format!("{program} : {message}"))
}

/// Périphérique monté en `path` d'après le contenu de `/proc/mounts` (espaces des chemins
/// écrits `\040`).
fn device_of(mounts: &str, path: &Path) -> Option<String> {
    let wanted = path.to_string_lossy();
    mounts.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let device = fields.next()?;
        let point = fields.next()?.replace("\\040", " ");
        (point == wanted).then(|| device.to_string())
    })
}

/// Recherche de dossiers (panneau des dossiers, `/`, retour du 19/10/2026) : d'où elle
/// part et ce qu'elle cherche.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirQuery {
    /// Dossier de départ : ses sous-dossiers sont parcourus.
    pub base: PathBuf,
    /// Motif cherché en flou ([`crate::fuzzy`]) sur le chemin relatif à `base`.
    pub pattern: String,
}

/// Lit ce qui est tapé : `~/…` part du dossier personnel `home`, `/…` de la racine ou du
/// plus long chemin existant tapé en tête (`/Volumes/Work/dehors` : de `/Volumes/Work`),
/// sinon du dossier `current` (celui pointé dans le panneau).
pub fn parse_dir_query(text: &str, current: &Path, home: Option<&Path>) -> DirQuery {
    let text = text.trim_start();
    if let (Some(home), Some(rest)) = (
        home,
        text.strip_prefix("~/").or((text == "~").then_some("")),
    ) {
        return DirQuery {
            base: home.to_path_buf(),
            pattern: rest.to_string(),
        };
    }
    if Path::new(text).is_absolute() {
        // Le plus long préfixe, coupé à un séparateur, qui soit un dossier existant.
        let cuts = text
            .char_indices()
            .filter(|(_, c)| std::path::is_separator(*c))
            .map(|(i, _)| i + 1)
            .collect::<Vec<_>>();
        for &cut in cuts.iter().rev() {
            let base = Path::new(&text[..cut]);
            if base.is_dir() {
                return DirQuery {
                    base: base.to_path_buf(),
                    pattern: text[cut..].to_string(),
                };
            }
        }
    }
    DirQuery {
        base: current.to_path_buf(),
        pattern: text.to_string(),
    }
}

/// Dossier personnel de l'utilisateur (`HOME`, `USERPROFILE` sous Windows).
pub fn home() -> Option<PathBuf> {
    home_dir(std::env::consts::OS, |name| std::env::var_os(name))
}

/// Recherche de dossiers en fond ([`walk_dirs`] et [`crate::fuzzy`]) : les dossiers dont
/// le chemin relatif à la base correspond au motif arrivent au fil de l'eau, avec leur
/// score. S'arrête après `limit` correspondances, ou quand on la lâche (nouvelle lettre).
pub struct DirSearch {
    query: DirQuery,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    results: std::sync::mpsc::Receiver<(PathBuf, i32)>,
}

impl DirSearch {
    /// Lance la recherche ; `notify` est appelé à chaque résultat et à la fin (redessiner).
    pub fn start(
        query: DirQuery,
        depth: usize,
        hidden: bool,
        limit: usize,
        notify: impl Fn() + Send + 'static,
    ) -> DirSearch {
        use std::sync::atomic::{AtomicBool, Ordering};
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let done = std::sync::Arc::new(AtomicBool::new(false));
        let (tx, results) = std::sync::mpsc::channel();
        {
            let (cancel, done, q) = (cancel.clone(), done.clone(), query.clone());
            std::thread::spawn(move || {
                let mut count = 0;
                walk_dirs(&q.base, depth, hidden, &cancel, |dir| {
                    let relative = dir.strip_prefix(&q.base).unwrap_or(dir);
                    if let Some(score) =
                        crate::fuzzy::score(&q.pattern, &relative.to_string_lossy())
                    {
                        count += 1;
                        if tx.send((dir.to_path_buf(), score)).is_err() {
                            return false;
                        }
                        notify();
                    }
                    count < limit
                });
                done.store(true, Ordering::Relaxed);
                notify();
            });
        }
        DirSearch {
            query,
            cancel,
            done,
            results,
        }
    }

    /// La requête lue.
    pub fn query(&self) -> &DirQuery {
        &self.query
    }

    /// Résultats arrivés depuis le dernier appel.
    pub fn take(&self) -> Vec<(PathBuf, i32)> {
        self.results.try_iter().collect()
    }

    /// Vrai quand le parcours est fini (ou arrêté).
    pub fn is_done(&self) -> bool {
        self.done.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Drop for DirSearch {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Parcourt les sous-dossiers de `base` en largeur (les plus proches d'abord), sur
/// `depth` niveaux au plus, sans les dossiers cachés (sauf `hidden`) ni les sauvegardes
/// Time Machine. `found` reçoit chaque dossier et renvoie faux pour arrêter ; `cancel`
/// interrompt (une nouvelle lettre tapée).
pub fn walk_dirs(
    base: &Path,
    depth: usize,
    hidden: bool,
    cancel: &std::sync::atomic::AtomicBool,
    mut found: impl FnMut(&Path) -> bool,
) {
    use std::sync::atomic::Ordering;
    let mut level = vec![base.to_path_buf()];
    for _ in 0..depth {
        let mut next = Vec::new();
        for dir in &level {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            for child in subfolders(dir, hidden).unwrap_or_default() {
                if is_backup(&child) {
                    continue;
                }
                if !found(&child) {
                    return;
                }
                next.push(child);
            }
        }
        if next.is_empty() {
            return;
        }
        level = next;
    }
}

/// Une ligne de l'arborescence affichée : un emplacement (profondeur 0) ou un dossier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Dossier.
    pub path: PathBuf,
    /// Nom affiché.
    pub name: String,
    /// Profondeur (0 : emplacement).
    pub depth: usize,
    /// Déplié.
    pub expanded: bool,
    /// Dossier illisible (droits...), une fois déplié.
    pub error: Option<String>,
    /// Nature de l'emplacement, pour une ligne de profondeur 0 (un volume peut être
    /// éjecté).
    pub kind: Option<PlaceKind>,
    /// Emplacement qu'on peut éjecter (volume non fixé, voir [`PlaceSettings::fixed`]).
    pub ejectable: bool,
    /// Dossier favori (dans la section des favoris, ou ailleurs dans l'arborescence).
    pub favorite: bool,
    /// Section du panneau où est la ligne (celle de son emplacement).
    pub section: Section,
}

/// Arborescence des dossiers à partir des emplacements, indépendante de l'interface : ce
/// qui est déplié, les sous-dossiers déjà lus (lecture seulement au dépliage), et les
/// lignes visibles à plat ([`Tree::rows`]), qu'une interface graphique ou un TUI n'ont
/// plus qu'à dessiner. Un dossier présent sous deux emplacements (`/Volumes/SD` sous
/// « Volumes » et sous la racine) est déplié aux deux endroits.
#[derive(Clone, Debug)]
pub struct Tree {
    places: Vec<Place>,
    filter: PlaceSettings,
    hidden: bool,
    expanded: std::collections::HashSet<PathBuf>,
    children: std::collections::HashMap<PathBuf, Result<Vec<PathBuf>, String>>,
    /// Ligne pointée au clavier : son rang dans [`Tree::rows`] et son dossier (un dossier
    /// peut paraître deux fois ; le rang départage, le dossier retrouve la ligne quand
    /// d'autres apparaissent au-dessus).
    cursor: Option<(usize, PathBuf)>,
    /// Favoris, dans l'ordre ([`favorites`]), et ceux qui existent (montrés en tête).
    favorites: Vec<PathBuf>,
    present: Vec<PathBuf>,
}

impl Tree {
    /// Arborescence des emplacements de cette machine, sauf ceux que `filter` masque ;
    /// dossiers cachés seulement si `hidden`.
    pub fn new(filter: PlaceSettings, hidden: bool) -> Tree {
        let mut tree = Tree::with_places(places_filtered(&filter), hidden);
        tree.filter = filter;
        tree
    }

    /// Arborescence à partir d'emplacements donnés (tests, emplacements choisis).
    pub fn with_places(places: Vec<Place>, hidden: bool) -> Tree {
        Tree {
            places,
            filter: PlaceSettings::default(),
            hidden,
            expanded: Default::default(),
            children: Default::default(),
            cursor: None,
            favorites: Vec::new(),
            present: Vec::new(),
        }
    }

    /// Remplace les favoris, montrés en tête, avant les emplacements ; ceux qui n'existent
    /// pas (volume démonté) sont omis jusqu'au prochain [`Tree::set_places`] ou
    /// [`Tree::refresh`].
    pub fn set_favorites(&mut self, favorites: Vec<PathBuf>) {
        self.favorites = favorites;
        self.check_favorites();
    }

    /// Favoris (tous, absents compris).
    pub fn favorites(&self) -> &[PathBuf] {
        &self.favorites
    }

    /// Vrai si `path` est un favori.
    pub fn is_favorite(&self, path: &Path) -> bool {
        self.favorites.iter().any(|p| p == path)
    }

    /// Favoris présents : vérifié une fois, pas à chaque dessin (un NAS peut être lent).
    fn check_favorites(&mut self) {
        self.present = self
            .favorites
            .iter()
            .filter(|p| p.is_dir())
            .cloned()
            .collect();
    }

    /// Emplacements.
    pub fn places(&self) -> &[Place] {
        &self.places
    }

    /// Vrai si `path` est déplié.
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    /// Déplie `path` (ses sous-dossiers sont lus la première fois).
    pub fn expand(&mut self, path: &Path) {
        if !self.children.contains_key(path) {
            let read = subfolders(path, self.hidden).map_err(|e| e.to_string());
            self.children.insert(path.to_path_buf(), read);
        }
        self.expanded.insert(path.to_path_buf());
    }

    /// Replie `path`.
    pub fn collapse(&mut self, path: &Path) {
        self.expanded.remove(path);
    }

    /// Déplie ou replie `path`.
    pub fn toggle(&mut self, path: &Path) {
        if self.is_expanded(path) {
            self.collapse(path);
        } else {
            self.expand(path);
        }
    }

    /// Déplie ce qu'il faut pour montrer `path`, sous le premier emplacement qui le
    /// contient (le plus précis : un volume avant la racine). Vrai si un emplacement le
    /// contient.
    pub fn reveal(&mut self, path: &Path) -> bool {
        let best = self
            .places
            .iter()
            .filter(|p| path.starts_with(&p.path))
            .max_by_key(|p| p.path.components().count())
            .map(|p| p.path.clone());
        let Some(root) = best else {
            return false;
        };
        let chain = ancestors_from(&root, path).unwrap_or_default();
        self.expand(&root);
        // Tous sauf le dossier lui-même : on le montre, sans forcément le déplier.
        for dir in chain.iter().take(chain.len().saturating_sub(1)) {
            self.expand(dir);
        }
        true
    }

    /// Vrai si `path` est un emplacement qu'on peut éjecter (volume affiché, non fixé) : à
    /// vérifier avant [`eject`].
    pub fn can_eject(&self, path: &Path) -> bool {
        self.places
            .iter()
            .any(|p| p.path == path && self.filter.ejectable(p))
    }

    /// Remplace les emplacements (liste fournie par un [`Monitor`]) ; ce qui était déplié
    /// le reste.
    pub fn set_places(&mut self, places: Vec<Place>) {
        self.places = places;
        self.check_favorites();
    }

    /// Relit les emplacements (carte SD insérée entre-temps) et les dossiers dépliés.
    pub fn refresh(&mut self) {
        self.places = places_filtered(&self.filter);
        self.check_favorites();
        self.reload();
    }

    /// Relit les dossiers dépliés (dossier créé ou supprimé entre-temps).
    pub fn reload(&mut self) {
        self.children.clear();
        let expanded: Vec<PathBuf> = self.expanded.iter().cloned().collect();
        for path in expanded {
            self.expand(&path);
        }
    }

    /// Rang de la ligne pointée au clavier dans [`Tree::rows`] (`None` : aucune).
    pub fn cursor(&self) -> Option<usize> {
        let (index, path) = self.cursor.as_ref()?;
        let rows = self.rows();
        if rows.get(*index).is_some_and(|r| r.path == *path) {
            return Some(*index);
        }
        // Des lignes ont paru ou disparu au-dessus : la plus proche de ce dossier.
        rows.iter()
            .enumerate()
            .filter(|(_, r)| r.path == *path)
            .min_by_key(|(i, _)| i.abs_diff(*index))
            .map(|(i, _)| i)
            .or_else(|| (!rows.is_empty()).then(|| (*index).min(rows.len() - 1)))
    }

    /// Dossier de la ligne pointée.
    pub fn cursor_path(&self) -> Option<PathBuf> {
        self.cursor().map(|i| self.rows()[i].path.clone())
    }

    /// Pointe la ligne `index` (bornée aux lignes visibles).
    pub fn set_cursor(&mut self, index: usize) {
        let rows = self.rows();
        if let Some(row) = rows.get(index.min(rows.len().saturating_sub(1))) {
            self.cursor = Some((index.min(rows.len() - 1), row.path.clone()));
        }
    }

    /// Pointe la ligne de `path` (la première), si elle est visible.
    pub fn point_at(&mut self, path: &Path) {
        if let Some(i) = self.rows().iter().position(|r| r.path == path) {
            self.set_cursor(i);
        }
    }

    /// Descend (`delta` > 0) ou monte de `delta` lignes ; sans ligne pointée, la première.
    pub fn move_cursor(&mut self, delta: isize) {
        let next = match self.cursor() {
            Some(i) => i.saturating_add_signed(delta),
            None => 0,
        };
        self.set_cursor(next);
    }

    /// Première (`first`) ou dernière ligne.
    pub fn cursor_to_end(&mut self, first: bool) {
        let last = self.rows().len().saturating_sub(1);
        self.set_cursor(if first { 0 } else { last });
    }

    /// Droite : déplie la ligne pointée ; déjà dépliée, descend sur son premier
    /// sous-dossier.
    pub fn cursor_right(&mut self) {
        let Some(i) = self.cursor() else {
            return self.set_cursor(0);
        };
        let row = self.rows()[i].clone();
        if row.expanded {
            let rows = self.rows();
            if rows.get(i + 1).is_some_and(|next| next.depth > row.depth) {
                self.set_cursor(i + 1);
            }
        } else {
            self.expand(&row.path);
            self.set_cursor(i);
        }
    }

    /// Gauche : replie la ligne pointée ; déjà repliée, remonte à son dossier parent.
    pub fn cursor_left(&mut self) {
        let Some(i) = self.cursor() else {
            return self.set_cursor(0);
        };
        let rows = self.rows();
        let row = &rows[i];
        if row.expanded {
            let path = row.path.clone();
            self.collapse(&path);
            self.set_cursor(i);
        } else if let Some(parent) = (0..i).rev().find(|&j| rows[j].depth < row.depth) {
            self.set_cursor(parent);
        }
    }

    /// Lignes visibles, dans l'ordre d'affichage, section par section ([`Section`]) :
    /// volumes, dossier personnel, favoris présents, racine ; sous chaque emplacement,
    /// s'il est déplié, ses sous-dossiers, récursivement.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut places: Vec<&Place> = self.places.iter().collect();
        // Tri stable : l'ordre des volumes entre eux est gardé.
        places.sort_by_key(|p| p.kind.section());
        let (before, after): (Vec<&Place>, Vec<&Place>) = places
            .into_iter()
            .partition(|p| p.kind.section() < Section::Favorites);
        for place in before {
            self.push_place(&mut rows, place);
        }
        for path in &self.present {
            let first = rows.len();
            self.push_rows(&mut rows, path, folder_name(path), 0, Section::Favorites);
            rows[first].kind = Some(PlaceKind::Favorite);
        }
        for place in after {
            self.push_place(&mut rows, place);
        }
        rows
    }

    fn push_place(&self, rows: &mut Vec<Row>, place: &Place) {
        let first = rows.len();
        let section = place.kind.section();
        self.push_rows(rows, &place.path, place.name.clone(), 0, section);
        rows[first].kind = Some(place.kind);
        rows[first].ejectable = self.filter.ejectable(place);
    }

    fn push_rows(
        &self,
        rows: &mut Vec<Row>,
        path: &Path,
        name: String,
        depth: usize,
        section: Section,
    ) {
        let expanded = self.is_expanded(path);
        let children = self.children.get(path).filter(|_| expanded);
        rows.push(Row {
            path: path.to_path_buf(),
            name,
            depth,
            expanded,
            error: match children {
                Some(Err(e)) => Some(e.clone()),
                _ => None,
            },
            kind: None,
            ejectable: false,
            favorite: self.is_favorite(path),
            section,
        });
        if let Some(Ok(children)) = children {
            for child in children {
                self.push_rows(rows, child, folder_name(child), depth + 1, section);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| OsString::from(v))
        }
    }

    #[test]
    fn a_background_search_finds_matching_folders() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["2025/Plage", "2026/2026-10-17_Self_Dehors", "2026/Studio"] {
            std::fs::create_dir_all(dir.path().join(d)).unwrap();
        }
        let query = parse_dir_query("2026 dehors", dir.path(), None);
        let search = DirSearch::start(query, 4, false, 100, || {});
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !search.is_done() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let found: Vec<PathBuf> = search.take().into_iter().map(|(p, _)| p).collect();
        assert_eq!(found, [dir.path().join("2026/2026-10-17_Self_Dehors")]);
    }

    #[test]
    fn folder_queries_start_where_asked() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("Work/2026/Dehors")).unwrap();
        let current = Path::new("/ici");
        let home = root.join("Work");
        let q = |t: &str| parse_dir_query(t, current, Some(&home));
        assert_eq!(q("dehors").base, current);
        assert_eq!(q("dehors").pattern, "dehors");
        assert_eq!(q("~/deh").base, home);
        assert_eq!(q("~/deh").pattern, "deh");
        assert_eq!(q("~").base, home);
        let typed = format!("{}/Work/2026/deh", root.display());
        assert_eq!(
            q(&typed).base,
            root.join("Work/2026"),
            "plus long chemin existant"
        );
        assert_eq!(q(&typed).pattern, "deh");
        // Sous Windows, « /zzz » n'est pas un chemin absolu (pas de lecteur).
        #[cfg(not(windows))]
        assert_eq!(q("/zzz").base, Path::new("/"), "racine");
    }

    #[test]
    fn folders_are_walked_breadth_first_within_depth() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for d in ["a/b/c/d", "e", ".cachée/x"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut seen = Vec::new();
        walk_dirs(root, 3, false, &cancel, |p| {
            seen.push(p.strip_prefix(root).unwrap().to_path_buf());
            true
        });
        let expected: Vec<PathBuf> = ["a", "e", "a/b", "a/b/c"]
            .iter()
            .map(|p| p.split('/').collect())
            .collect();
        assert_eq!(seen, expected, "en largeur, 3 niveaux, sans les cachés");
        let mut count = 0;
        walk_dirs(root, 3, true, &cancel, |_| {
            count += 1;
            count < 2
        });
        assert_eq!(count, 2, "arrêt demandé");
    }

    #[test]
    fn the_panel_lists_drives_network_home_favorites_then_root() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["home", "card", "nas", "fav"] {
            std::fs::create_dir(dir.path().join(d)).unwrap();
        }
        let place = |name: &str, path: PathBuf, kind| Place {
            name: name.into(),
            path,
            kind,
        };
        // Dans le désordre : la racine d'abord, comme le donnerait une autre plateforme.
        let mut tree = Tree::with_places(
            vec![
                place("Racine", PathBuf::from("/"), PlaceKind::Root),
                place("Perso", dir.path().join("home"), PlaceKind::Home),
                place("SD", dir.path().join("card"), PlaceKind::Volume),
                place("NAS", dir.path().join("nas"), PlaceKind::Network),
            ],
            false,
        );
        tree.set_favorites(vec![dir.path().join("fav")]);
        let rows = tree.rows();
        let got: Vec<(&str, Section)> = rows.iter().map(|r| (r.name.as_str(), r.section)).collect();
        assert_eq!(
            got,
            [
                ("SD", Section::Drives),
                ("NAS", Section::Network),
                ("Perso", Section::Personal),
                ("fav", Section::Favorites),
                ("Racine", Section::System),
            ]
        );
        assert!(!rows[4].expanded, "racine repliée par défaut");
    }

    #[test]
    fn personal_folders_follow_xdg_and_network_mounts_are_found() {
        let home = Path::new("/home/fab");
        let xdg = "# commentaire\nXDG_DESKTOP_DIR=\"$HOME/Bureau\"\n\
                   XDG_DOWNLOAD_DIR=\"$HOME/\"\nXDG_PICTURES_DIR=\"/data/Photos\"\n";
        let paths: Vec<PathBuf> = user_dirs("linux", home, xdg)
            .into_iter()
            .map(|p| p.path)
            .collect();
        assert_eq!(
            paths,
            [
                home.join("Bureau"),
                home.join("Documents"),
                PathBuf::from("/data/Photos")
            ],
            "Téléchargements = le dossier personnel : omis ; absent du fichier : nom anglais"
        );
        let mac: Vec<PathBuf> = user_dirs("macos", home, xdg)
            .into_iter()
            .map(|p| p.path)
            .collect();
        assert_eq!(mac[3], home.join("Pictures"));
        let mount = "/dev/disk3s1s1 on / (apfs, sealed, local, read-only, journaled)\n\
                     //fab@nas._smb._tcp.local/photo on /Volumes/photo (smbfs, nodev, nosuid, mounted by fab)\n\
                     /dev/disk5s1 on /Volumes/Work (apfs, local, nodev, nosuid, journaled)\n";
        assert_eq!(
            network_mounts_macos(mount),
            [PathBuf::from("/Volumes/photo")]
        );
        let proc = "/dev/sda1 / ext4 rw 0 0\n\
                    //nas/photo /mnt/nas\\040photo cifs rw,relatime 0 0\n\
                    fab@srv:/ /home/fab/srv fuse.sshfs rw 0 0\n\
                    gvfsd-fuse /run/user/1000/gvfs fuse.gvfsd-fuse rw 0 0\n";
        assert_eq!(
            network_mounts_linux(proc),
            [
                PathBuf::from("/mnt/nas photo"),
                PathBuf::from("/home/fab/srv")
            ]
        );
        let gvfs = tempfile::tempdir().unwrap();
        for d in [
            "smb-share:server=nas.local,share=photo",
            "mtp:host=Google_Pixel_7",
            "sftp:host=srv",
        ] {
            std::fs::create_dir(gvfs.path().join(d)).unwrap();
        }
        let mut found: Vec<(String, PlaceKind)> = gvfs_places(gvfs.path())
            .into_iter()
            .map(|p| (p.name, p.kind))
            .collect();
        found.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            found,
            [
                ("Google_Pixel_7".to_string(), PlaceKind::Volume),
                ("photo (nas.local)".to_string(), PlaceKind::Network),
                ("srv".to_string(), PlaceKind::Network),
            ]
        );
    }

    #[test]
    fn favorites_have_their_section_and_follow_renames() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let root = dir.path().join("photos");
        for d in ["2026/Séance", "2026/Autre"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let seance = root.join("2026/Séance");
        assert!(favorites(&data).is_empty());
        set_favorite(&data, &seance, true).unwrap();
        set_favorite(&data, &root.join("absent"), true).unwrap();
        let list = set_favorite(&data, &root.join("2026"), true).unwrap();
        assert_eq!(list.len(), 3);
        let place = Place {
            name: "Test".into(),
            path: root.clone(),
            kind: PlaceKind::Home,
        };
        let mut tree = Tree::with_places(vec![place], false);
        tree.set_favorites(list);
        let rows = tree.rows();
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            ["Test", "Séance", "2026"],
            "dossier personnel, puis les favoris (absent : omis)"
        );
        assert_eq!(rows[1].kind, Some(PlaceKind::Favorite));
        assert_eq!(rows[0].section, Section::Personal);
        assert_eq!(rows[1].section, Section::Favorites);
        assert!(rows[1].favorite && !rows[0].favorite);
        // Le même dossier dans l'arborescence est marqué favori aussi.
        tree.expand(&root);
        assert!(tree.rows().iter().any(|r| r.depth == 1 && r.favorite));
        // Renommer « 2026 » : les deux favoris suivent.
        std::fs::rename(root.join("2026"), root.join("2027")).unwrap();
        let list = rename_favorites(&data, &root.join("2026"), &root.join("2027")).unwrap();
        assert_eq!(list[0], root.join("2027/Séance"));
        assert_eq!(list[2], root.join("2027"));
        assert_eq!(favorites(&data), list, "écrit");
        let list = set_favorite(&data, &root.join("2027"), false).unwrap();
        assert_eq!(list, [root.join("2027/Séance"), root.join("absent")]);
    }

    #[test]
    fn the_keyboard_cursor_walks_the_tree() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["A/A1", "A/A2", "B"] {
            std::fs::create_dir_all(dir.path().join(d)).unwrap();
        }
        let place = Place {
            name: "Test".into(),
            path: dir.path().to_path_buf(),
            kind: PlaceKind::Home,
        };
        let mut tree = Tree::with_places(vec![place], false);
        assert_eq!(tree.cursor(), None);
        tree.move_cursor(1);
        assert_eq!(tree.cursor(), Some(0), "sans curseur : la première ligne");
        tree.cursor_right();
        assert!(tree.is_expanded(dir.path()), "droite déplie");
        tree.cursor_right();
        assert_eq!(
            tree.cursor_path(),
            Some(dir.path().join("A")),
            "puis descend"
        );
        tree.cursor_right();
        tree.move_cursor(2);
        assert_eq!(tree.cursor_path(), Some(dir.path().join("A/A2")));
        tree.cursor_left();
        assert_eq!(
            tree.cursor_path(),
            Some(dir.path().join("A")),
            "gauche remonte"
        );
        tree.cursor_left();
        assert!(!tree.is_expanded(&dir.path().join("A")), "puis replie");
        tree.cursor_to_end(false);
        assert_eq!(tree.cursor_path(), Some(dir.path().join("B")));
        tree.move_cursor(10);
        assert_eq!(tree.cursor_path(), Some(dir.path().join("B")), "borné");
        tree.cursor_to_end(true);
        assert_eq!(tree.cursor(), Some(0));
        tree.point_at(&dir.path().join("B"));
        assert_eq!(tree.cursor_path(), Some(dir.path().join("B")));
    }

    #[test]
    fn volume_dirs_per_os() {
        let user = [("USER", "fab")];
        assert_eq!(
            volume_dirs("macos", env(&user)),
            [PathBuf::from("/Volumes")]
        );
        assert_eq!(
            volume_dirs("linux", env(&user)),
            [
                PathBuf::from("/media/fab"),
                PathBuf::from("/run/media/fab"),
                PathBuf::from("/mnt")
            ]
        );
        assert_eq!(volume_dirs("linux", env(&[])), [PathBuf::from("/mnt")]);
        assert!(volume_dirs("windows", env(&user)).is_empty());
    }

    // Chemins Unix simulés : sous Windows, « /home/fab » n'est pas absolu.
    #[cfg(not(windows))]
    #[test]
    fn home_per_os() {
        let vars = [("HOME", "/home/fab"), ("USERPROFILE", "C:\\Users\\fab")];
        assert_eq!(
            home_dir("linux", env(&vars)),
            Some(PathBuf::from("/home/fab"))
        );
        assert_eq!(home_dir("linux", env(&[("HOME", "relatif")])), None);
    }

    #[test]
    fn ancestors_to_unfold() {
        assert_eq!(
            ancestors_from(Path::new("/Volumes"), Path::new("/Volumes/SD/DCIM")),
            Some(vec![
                PathBuf::from("/Volumes/SD"),
                PathBuf::from("/Volumes/SD/DCIM")
            ])
        );
        assert_eq!(
            ancestors_from(Path::new("/Volumes"), Path::new("/Volumes")),
            Some(vec![])
        );
        assert_eq!(
            ancestors_from(Path::new("/home"), Path::new("/Volumes/SD")),
            None
        );
    }

    #[test]
    fn the_monitor_reports_a_plugged_volume() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        let monitor = Monitor::with_source(
            std::time::Duration::from_millis(20),
            move || volumes_in(std::slice::from_ref(&root)),
            move |places| {
                let _ = tx.send(places);
            },
        );
        std::fs::create_dir(dir.path().join("SD_CARD")).unwrap();
        let places = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("changement signalé");
        assert_eq!(places.len(), 1);
        assert_eq!(places[0].name, "SD_CARD");
        // Rien de neuf : pas de nouveau message.
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(100))
                .is_err()
        );
        std::fs::remove_dir(dir.path().join("SD_CARD")).unwrap();
        let places = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert!(places.is_empty(), "débranché");
        drop(monitor); // s'arrête sans attendre l'intervalle
    }

    #[test]
    fn a_zero_interval_watches_nothing() {
        let monitor = Monitor::start(
            PlaceSettings {
                watch_ms: 0,
                ..PlaceSettings::default()
            },
            |_| panic!("pas de surveillance"),
        );
        assert!(monitor.thread.is_none());
    }

    #[test]
    fn device_of_a_mount_point() {
        let mounts = "/dev/sda2 / ext4 rw 0 0\n\
                      /dev/sdb1 /media/fab/CARTE\\040SD vfat rw,nosuid 0 0\n";
        assert_eq!(
            device_of(mounts, Path::new("/media/fab/CARTE SD")),
            Some("/dev/sdb1".into())
        );
        assert_eq!(device_of(mounts, Path::new("/media/fab/AUTRE")), None);
    }

    #[test]
    fn wildcards() {
        assert!(wildcard("Backup", "backup"));
        assert!(wildcard("/mnt/*", "/mnt/nas"));
        assert!(wildcard(
            "com.apple.TimeMachine.*",
            "com.apple.TimeMachine.localsnapshots"
        ));
        assert!(wildcard("SD-??", "sd-01"));
        assert!(!wildcard("SD-??", "sd-1"));
        assert!(!wildcard("Work", "Workspace"));
        assert!(wildcard("*", ""));
    }

    #[test]
    fn hidden_names() {
        assert!(is_hidden(Path::new("/a/.git")));
        assert!(!is_hidden(Path::new("/a/DCIM")));
    }
}
