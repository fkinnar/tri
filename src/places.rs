//! Emplacements à proposer pour choisir un dossier : volumes montés (cartes SD, disques,
//! NAS), dossier personnel, racine ; et lecture des sous-dossiers d'un dossier.
//!
//! Rien n'est parcouru d'avance : l'interface lit un dossier quand on le déplie, ce qui
//! reste instantané même sur un NAS. Voir `doc/en/places.md`.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

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
    /// Support amovible branché mais pas monté (Linux : [`unmounted_devices`]) ; son
    /// chemin est celui du périphérique (`/dev/sdb1`), à monter ([`mount`]) avant d'en
    /// lire les dossiers.
    Device,
    /// Partage réseau mémorisé pas monté (macOS, Linux : [`crate::servers`]) ; son chemin
    /// est son adresse (`smb://nas/photo`), à rejoindre ([`crate::servers::Server::connect`])
    /// avant d'en lire les dossiers.
    Server,
    /// Serveur annoncé sur le réseau local ([`crate::neighbourhood`]) ; son chemin est
    /// son adresse (`smb://nas.local/`), ouverte dans la recherche des dossiers pour
    /// choisir un partage.
    Neighbour,
}

/// Section du panneau des dossiers, dans l'ordre d'affichage (demandé le 20/10/2026) :
/// périphériques, disques, dossier personnel, favoris, puis seulement la racine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Section {
    /// Supports amovibles (cartes, clés, disques externes, appareils), montés ou non
    /// (#113, comme le gestionnaire de fichiers).
    Devices,
    /// Disques fixés ([`PlaceSettings::fixed`]) : toujours branchés, jamais éjectés.
    Drives,
    /// Partages réseau montés, et ceux mémorisés ([`crate::servers`]).
    Network,
    /// Dossier personnel et ses dossiers habituels.
    Personal,
    /// Favoris de l'utilisateur.
    Favorites,
    /// Racine du système de fichiers.
    System,
}

impl Section {
    /// Toutes les sections, dans l'ordre d'affichage.
    pub const ALL: [Section; 6] = [
        Section::Devices,
        Section::Drives,
        Section::Network,
        Section::Personal,
        Section::Favorites,
        Section::System,
    ];

    /// Identifiant stable de la section (repliée ou non, retenu d'une session à l'autre).
    pub fn key(self) -> &'static str {
        match self {
            Section::Devices => "devices",
            Section::Drives => "drives",
            Section::Network => "network",
            Section::Personal => "personal",
            Section::Favorites => "favorites",
            Section::System => "system",
        }
    }
}

impl PlaceKind {
    /// Section où se range un emplacement de cette nature.
    pub fn section(self) -> Section {
        match self {
            PlaceKind::Volume | PlaceKind::Device => Section::Devices,
            PlaceKind::Network | PlaceKind::Server | PlaceKind::Neighbour => Section::Network,
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
    /// Monter de lui-même un support amovible branché mais pas monté par le système
    /// (Linux, #113 ; activé par défaut, choix de l'utilisateur ; sinon, il attend un clic
    /// dans la section des périphériques).
    pub auto_mount: bool,
    /// Windows : attente maximale du verrouillage du volume avant l'éjection (ms), le
    /// temps qu'un programme lâche ses fichiers ; au-delà, l'éjection est refusée avec la
    /// raison (issue n° 192).
    pub eject_lock_ms: u64,
    /// Éjection refusée parce que le volume est occupé : nouveaux essais, le temps que des
    /// calculs encore en cours sur ses fichiers se terminent (14/11/2026 : tri lui-même
    /// tenait une clé USB juste après y avoir déplacé un dossier). 0 : un seul essai.
    pub eject_retries: u32,
    /// Attente entre deux essais (ms).
    pub eject_retry_ms: u64,
    /// Dossier déplié dans le panneau : attente de sa liste (ms) ; au-delà (iCloud, NAS),
    /// elle est lue en fond et l'interface ne se fige pas (retour du 16/11/2026).
    pub read_wait_ms: u64,
}

impl Default for PlaceSettings {
    fn default() -> Self {
        Self {
            hide_backups: true,
            exclude: Vec::new(),
            fixed: Vec::new(),
            watch_ms: 2000,
            auto_mount: true,
            eject_lock_ms: 5000,
            eject_retries: 5,
            eject_retry_ms: 1000,
            read_wait_ms: 150,
        }
    }
}

impl PlaceSettings {
    /// Vrai si `place` doit être masqué.
    pub fn hides(&self, place: &Place) -> bool {
        matches_any(&self.exclude, place)
            || (self.hide_backups && place.kind == PlaceKind::Volume && is_backup(&place.path))
    }

    /// Vrai si `place` est un volume qu'on peut proposer d'éjecter : un volume, pas fixé,
    /// ou un support pas monté (on peut couper son alimentation).
    pub fn ejectable(&self, place: &Place) -> bool {
        match place.kind {
            PlaceKind::Volume => !matches_any(&self.fixed, place),
            PlaceKind::Device => true,
            _ => false,
        }
    }

    /// Section où se range `place` : un volume fixé va dans [`Section::Drives`].
    pub fn section(&self, place: &Place) -> Section {
        if place.kind == PlaceKind::Volume && !self.ejectable(place) {
            Section::Drives
        } else {
            place.kind.section()
        }
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

/// Racines des disques amovibles branchés (volumes qu'on peut éjecter, voir
/// [`PlaceSettings::ejectable`]) : là où `[cache] location = "disk"` range le cache.
pub fn removable_roots(filter: &PlaceSettings) -> Vec<PathBuf> {
    places_filtered(filter)
        .into_iter()
        .filter(|p| filter.ejectable(p))
        .map(|p| p.path)
        .collect()
}

/// Racines hors des « disques internes » : disques amovibles ([`removable_roots`]) et
/// partages réseau montés. Le niveau « internes » de `[xmp] embed` n'y écrit pas
/// d'office (voir [`crate::embedded::Auto`]).
pub fn outside_roots(filter: &PlaceSettings) -> Vec<PathBuf> {
    places_filtered(filter)
        .into_iter()
        .filter(|p| filter.ejectable(p) || p.kind == PlaceKind::Network)
        .map(|p| p.path)
        .collect()
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
    // Linux : seuls les vrais points de montage (retour du 16/11/2026 : un disque éjecté
    // restait dans le panneau, son dossier de montage vide encore dans `/media`).
    if os != "macos" && os != "windows" {
        let mounts = fs::read_to_string("/proc/mounts").unwrap_or_default();
        places = mounted_only(places, &mounts);
    }
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
        places.extend(
            cloud_drives(os, &home)
                .into_iter()
                .filter(|p| p.path.is_dir()),
        );
    }
    places.extend(unmounted_devices().into_iter().map(|d| d.place()));
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
            // `has_root` plutôt qu'`is_absolute` : sous Windows, « /home/… » n'a pas de
            // lettre de lecteur ; seul un chemin relatif est écarté.
            (path != home && path.has_root()).then(|| Place {
                name: crate::t!(*label),
                path,
                kind: PlaceKind::Home,
            })
        })
        .collect()
}

/// **iCloud Drive** du dossier personnel `home`, dans la section Réseau (retour du
/// 20/11/2026 : introuvable sur macOS, caché dans `~/Library` ; « plus logique dans
/// Réseau » que dans les dossiers) : `~/Library/Mobile Documents/com~apple~CloudDocs` sur
/// macOS, `~/iCloudDrive` sous Windows (application iCloud), rien sous Linux. Existence
/// non vérifiée. Comme un partage, hors des disques internes ([`outside_roots`]).
fn cloud_drives(os: &str, home: &Path) -> Vec<Place> {
    let path = match os {
        "macos" => home.join("Library/Mobile Documents/com~apple~CloudDocs"),
        "windows" => home.join("iCloudDrive"),
        _ => return Vec::new(),
    };
    vec![Place {
        name: crate::t!("places-icloud"),
        path,
        kind: PlaceKind::Network,
    }]
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
        "macos" => crate::process::command("/sbin/mount")
            .output()
            .map(|o| network_mounts_macos(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default(),
        "windows" => Vec::new(),
        _ => network_mounts_linux(
            &fs::read_to_string("/proc/mounts").unwrap_or_default(),
            &fs::read_to_string("/etc/fstab").unwrap_or_default(),
        ),
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
///
/// Un montage automatique (`x-systemd.automount` dans `/etc/fstab`) n'y apparaît qu'en
/// `autofs` tant que personne n'est entré dans le dossier : il se rangeait alors dans les
/// périphériques au premier lancement, dans le réseau ensuite (retour du 27/11/2026, NFS).
/// Un point `autofs` compte donc comme réseau quand `fstab` (texte de `/etc/fstab`) lui
/// donne un système de fichiers réseau. Un point monté n'est donné qu'une fois (l'`autofs`
/// reste listé sous le vrai montage).
fn network_mounts_linux(text: &str, fstab: &str) -> Vec<PathBuf> {
    let unescape = |point: &str| PathBuf::from(point.replace("\\040", " "));
    let fields = |line: &str| -> Option<(String, String)> {
        let line = line.trim();
        if line.starts_with('#') {
            return None;
        }
        let mut f = line.split_whitespace();
        let (_, point, fs_type) = (f.next()?, f.next()?, f.next()?);
        Some((point.to_string(), fs_type.to_string()))
    };
    let network_in_fstab: Vec<String> = fstab
        .lines()
        .filter_map(fields)
        .filter(|(_, fs_type)| NETWORK_FS.contains(&fs_type.as_str()))
        .map(|(point, _)| point)
        .collect();
    let mut points: Vec<PathBuf> = Vec::new();
    for (point, fs_type) in text.lines().filter_map(fields) {
        let network = NETWORK_FS.contains(&fs_type.as_str())
            || (fs_type == "autofs" && network_in_fstab.contains(&point));
        let path = unescape(&point);
        if network && !points.contains(&path) {
            points.push(path);
        }
    }
    points
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

/// Lettres de lecteur présentes (`C:\`...), sous Windows ; un lecteur réseau (`Z:`
/// attribué à un partage) va dans la section « Réseau » (28/10/2026).
fn windows_drives() -> Vec<Place> {
    (b'A'..=b'Z')
        .map(|letter| format!("{}:\\", letter as char))
        .filter(|root| Path::new(root).is_dir())
        .map(|root| Place {
            name: root.clone(),
            kind: if is_remote_drive(&root) {
                PlaceKind::Network
            } else {
                PlaceKind::Volume
            },
            path: PathBuf::from(root),
        })
        .collect()
}

/// Vrai si la lettre de lecteur `root` (`Z:\`) est un lecteur réseau (Windows).
#[cfg(windows)]
fn is_remote_drive(root: &str) -> bool {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    // DRIVE_REMOTE (WinBase.h).
    const DRIVE_REMOTE: u32 = 4;
    let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY : chaîne terminée par un zéro, lue seulement pendant l'appel.
    unsafe { GetDriveTypeW(wide.as_ptr()) == DRIVE_REMOTE }
}

#[cfg(not(windows))]
fn is_remote_drive(_root: &str) -> bool {
    false
}

/// Fichier des favoris, dans le dossier de données : un chemin par ligne, dans l'ordre
/// d'affichage, suivi d'une tabulation et de son nom quand l'utilisateur lui en a donné
/// un (22/10/2026 : plusieurs dossiers « Sources » à des endroits différents).
const FAVORITES_FILE: &str = "favorites";

/// Favoris lus : (dossier, nom donné), dans l'ordre.
fn favorite_entries(data: &Path) -> Vec<(PathBuf, Option<String>)> {
    fs::read_to_string(data.join(FAVORITES_FILE))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| match line.split_once('\t') {
            Some((path, name)) if !name.trim().is_empty() => {
                (PathBuf::from(path), Some(name.trim().to_string()))
            }
            Some((path, _)) => (PathBuf::from(path), None),
            None => (PathBuf::from(line), None),
        })
        .collect()
}

fn write_favorites(data: &Path, list: &[(PathBuf, Option<String>)]) -> io::Result<()> {
    let text: String = list
        .iter()
        .map(|(p, name)| {
            let mut line = p.to_string_lossy().into_owned();
            if let Some(name) = name {
                line.push('\t');
                line.push_str(name);
            }
            line + "\n"
        })
        .collect();
    fs::create_dir_all(data)?;
    fs::write(data.join(FAVORITES_FILE), text)
}

/// Dossiers favoris, dans l'ordre où ils ont été ajoutés, absents compris (carte retirée,
/// disque débranché : le favori revient quand le volume est remonté).
pub fn favorites(data: &Path) -> Vec<PathBuf> {
    favorite_entries(data).into_iter().map(|(p, _)| p).collect()
}

/// Noms donnés aux favoris (ceux qui en ont un), affichés à la place du nom du dossier
/// dans la section des favoris ([`Tree::set_favorite_names`]).
pub fn favorite_names(data: &Path) -> HashMap<PathBuf, String> {
    favorite_entries(data)
        .into_iter()
        .filter_map(|(p, name)| Some((p, name?)))
        .collect()
}

/// Donne le nom `name` au favori `dir` ; vide : il reprend le nom du dossier. Sans effet
/// si `dir` n'est pas un favori. Renvoie les noms.
pub fn name_favorite(data: &Path, dir: &Path, name: &str) -> io::Result<HashMap<PathBuf, String>> {
    let mut list = favorite_entries(data);
    let name = name.trim();
    for (path, entry) in &mut list {
        if path == dir {
            // Une tabulation ou un retour à la ligne casserait le fichier.
            let clean: String = name.chars().filter(|c| !c.is_control()).collect();
            *entry = (!clean.is_empty()).then_some(clean);
        }
    }
    write_favorites(data, &list)?;
    Ok(favorite_names(data))
}

/// Ajoute `dir` aux favoris (à la fin), ou l'en retire si `on` est faux (son nom avec).
/// Les autres gardent leur nom. Renvoie la nouvelle liste.
pub fn set_favorite(data: &Path, dir: &Path, on: bool) -> io::Result<Vec<PathBuf>> {
    let mut list = favorite_entries(data);
    let existing = list.iter().position(|(p, _)| p == dir);
    match (existing, on) {
        (Some(i), false) => {
            list.remove(i);
        }
        (None, true) => list.push((dir.to_path_buf(), None)),
        _ => {}
    }
    write_favorites(data, &list)?;
    Ok(list.into_iter().map(|(p, _)| p).collect())
}

/// Dossier `from` renommé ou déplacé en `to` : les favoris qui sont `from` ou dedans
/// suivent, avec leur nom. Renvoie la nouvelle liste (inchangée si aucun favori n'est
/// concerné).
pub fn rename_favorites(data: &Path, from: &Path, to: &Path) -> io::Result<Vec<PathBuf>> {
    let list = favorite_entries(data);
    let moved: Vec<(PathBuf, Option<String>)> = list
        .iter()
        .map(|(p, name)| {
            let p: PathBuf = p
                .strip_prefix(from)
                .map_or_else(|_| p.clone(), |rest| to.join(rest))
                .components()
                .collect();
            (p, name.clone())
        })
        .collect();
    if moved != list {
        write_favorites(data, &moved)?;
    }
    Ok(moved.into_iter().map(|(p, _)| p).collect())
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
        // Supports déjà vus, pour ne monter d'office qu'un support qui arrive : un
        // support démonté exprès ensuite reste démonté.
        let seen = std::sync::Mutex::new(HashSet::new());
        let source = move || {
            let places = places_filtered(&settings);
            if !settings.auto_mount {
                return places;
            }
            let mut seen = seen.lock().unwrap_or_else(|e| e.into_inner());
            seen.retain(|p: &PathBuf| p.exists());
            let arrived: Vec<&Place> = places
                .iter()
                .filter(|p| p.kind == PlaceKind::Device && seen.insert(p.path.clone()))
                .collect();
            if arrived.is_empty() {
                return places;
            }
            for device in arrived {
                let _ = mount(&device.path);
            }
            places_filtered(&settings)
        };
        Monitor::with_source(interval, source, on_change)
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

impl Monitor {
    /// Arrête la surveillance des volumes sans l'attendre (fermeture de l'application).
    pub fn detach(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
        }
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
/// `power-off` ; Windows : l'API du système (verrouiller, démonter, éjecter le support),
/// sinon le verbe « Éjecter » de l'explorateur pour un disque dont le support ne s'éjecte
/// pas. Bloquant (une à quelques secondes) : à appeler hors du fil de l'interface. En cas
/// de refus (disque interne, fichier ouvert sur le volume), le message du système est
/// renvoyé.
pub fn eject(path: &Path, settings: &PlaceSettings) -> Result<(), String> {
    let mut tries = 0;
    loop {
        match eject_once(path, settings) {
            Err(e) if is_busy(&e) && tries < settings.eject_retries => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(settings.eject_retry_ms));
            }
            result => return result,
        }
    }
}

/// Vrai si `path` est un iCloud Drive (dans la section Réseau, mais rien à déconnecter).
pub fn is_cloud_drive(path: &Path) -> bool {
    let os = std::env::consts::OS;
    home().is_some_and(|h| cloud_drives(os, &h).iter().any(|p| p.path == path))
}

/// Déconnecte le partage réseau monté en `path` (comme l'icône d'éjection du Finder) :
/// `diskutil unmount` (macOS) ; `gio mount -u` (Linux, montage GVFS), sinon `umount`
/// (montage NFS ou CIFS de l'utilisateur ; un montage de `/etc/fstab` réservé à
/// l'administrateur est refusé par le système, message renvoyé) ; `net use <lettre>
/// /delete` (Windows). Bloquant : à appeler hors du fil de l'interface.
pub fn disconnect(path: &Path) -> Result<(), String> {
    match std::env::consts::OS {
        "macos" => run("diskutil", &["unmount".as_ref(), path.as_os_str()]),
        "windows" => {
            let letter = path.to_string_lossy();
            let letter = letter.trim_end_matches(['\\', '/']);
            run(
                "net",
                &[
                    "use".as_ref(),
                    letter.as_ref(),
                    "/delete".as_ref(),
                    "/y".as_ref(),
                ],
            )
        }
        _ => run("gio", &["mount".as_ref(), "-u".as_ref(), path.as_os_str()])
            .or_else(|gio| run("umount", &[path.as_os_str()]).map_err(|e| format!("{gio}\n{e}"))),
    }
}

/// Vrai si le refus d'éjecter dit que le volume est occupé (fichiers ouverts) : `udisksctl`
/// (« target is busy », `DeviceBusy`), `umount`, `diskutil` (« dissented »), Windows.
fn is_busy(message: &str) -> bool {
    let m = message.to_lowercase();
    ["busy", "dissent", "in use", "being used"]
        .iter()
        .any(|w| m.contains(w))
}

/// Un essai d'éjection ([`eject`]).
fn eject_once(path: &Path, settings: &PlaceSettings) -> Result<(), String> {
    // Seul Windows a un réglage d'éjection.
    #[cfg(not(windows))]
    let _ = settings;
    // Support pas monté ([`PlaceKind::Device`]) : il n'y a qu'à couper l'alimentation.
    if path.starts_with("/dev") {
        return run(
            "udisksctl",
            &["power-off".as_ref(), "-b".as_ref(), path.as_os_str()],
        );
    }
    match std::env::consts::OS {
        "macos" => run("diskutil", &["eject".as_ref(), path.as_os_str()]),
        #[cfg(windows)]
        "windows" => windows::eject(
            path,
            std::time::Duration::from_millis(settings.eject_lock_ms),
        ),
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

/// Éjection sous Windows (issue n° 192 : le verbe « Éjecter » lancé par PowerShell
/// échouait sans rien dire). Méthode de Microsoft (KB 165721) : ouvrir le volume
/// (`\\.\E:`), le verrouiller (refusé tant qu'un fichier y est ouvert, réessayé pendant
/// `lock_wait`), le démonter, autoriser le retrait puis éjecter le support (carte SD,
/// clé). Un disque dont le support ne s'éjecte pas (disque dur USB) passe par le verbe
/// « Éjecter » de l'explorateur, qui le retire en toute sécurité.
#[cfg(windows)]
mod windows {
    use std::path::Path;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Ioctl::{
        FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME, IOCTL_STORAGE_EJECT_MEDIA,
        IOCTL_STORAGE_MEDIA_REMOVAL, PREVENT_MEDIA_REMOVAL,
    };

    /// Nombre d'essais de verrouillage pendant l'attente (comme l'exemple de Microsoft).
    const LOCK_TRIES: u32 = 20;

    /// Volume ouvert, refermé (et déverrouillé) en sortant.
    struct Volume(HANDLE);

    impl Drop for Volume {
        fn drop(&mut self) {
            // SAFETY : poignée ouverte par `CreateFileW`, refermée une seule fois.
            unsafe { CloseHandle(self.0) };
        }
    }

    impl Volume {
        /// Commande `code` sur le volume, avec `input` ; l'erreur du système sinon.
        fn control<T>(&self, code: u32, input: Option<&T>) -> Result<(), String> {
            let (ptr, size) = match input {
                Some(value) => (
                    value as *const T as *const core::ffi::c_void,
                    size_of::<T>() as u32,
                ),
                None => (std::ptr::null(), 0),
            };
            let mut returned = 0u32;
            // SAFETY : poignée valide ; tampon d'entrée de `size` octets, pas de sortie.
            let ok = unsafe {
                DeviceIoControl(
                    self.0,
                    code,
                    ptr,
                    size,
                    std::ptr::null_mut(),
                    0,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            };
            if ok != 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error().to_string())
            }
        }
    }

    pub fn eject(path: &Path, lock_wait: Duration) -> Result<(), String> {
        let letter = path
            .to_string_lossy()
            .chars()
            .next()
            .filter(|c| c.is_ascii_alphabetic())
            .ok_or_else(|| crate::t!("eject-not-a-drive", path = path.display().to_string()))?;
        let device: Vec<u16> = format!("\\\\.\\{letter}:")
            .encode_utf16()
            .chain([0])
            .collect();
        // SAFETY : nom terminé par un zéro ; pas d'attributs de sécurité ni de modèle.
        let handle = unsafe {
            CreateFileW(
                device.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let volume = Volume(handle);
        let mut locked = volume.control::<()>(FSCTL_LOCK_VOLUME, None);
        for _ in 1..LOCK_TRIES {
            if locked.is_ok() {
                break;
            }
            std::thread::sleep(lock_wait / LOCK_TRIES);
            locked = volume.control::<()>(FSCTL_LOCK_VOLUME, None);
        }
        locked.map_err(|e| crate::t!("eject-busy", error = e))?;
        volume.control::<()>(FSCTL_DISMOUNT_VOLUME, None)?;
        // Certains lecteurs ne gèrent pas l'interdiction de retrait : sans importance.
        let allow = PREVENT_MEDIA_REMOVAL {
            PreventMediaRemoval: false,
        };
        let _ = volume.control(IOCTL_STORAGE_MEDIA_REMOVAL, Some(&allow));
        if volume
            .control::<()>(IOCTL_STORAGE_EJECT_MEDIA, None)
            .is_ok()
        {
            return Ok(());
        }
        // Support qui ne s'éjecte pas (disque dur USB) : le volume, démonté, est refermé,
        // puis l'explorateur retire le disque.
        drop(volume);
        let script = format!(
            "(New-Object -comObject Shell.Application).Namespace(17).ParseName('{letter}:').InvokeVerb('Eject')"
        );
        super::run(
            "powershell",
            &["-NoProfile".as_ref(), "-Command".as_ref(), script.as_ref()],
        )
    }
}

/// Support amovible branché mais pas monté (#113 : un système qui ne monte plus rien de
/// lui-même ; le gestionnaire de fichiers le montre pourtant, sous « Périphériques »).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// Périphérique (`/dev/sdb1`).
    pub path: PathBuf,
    /// Nom du volume, s'il en a un.
    pub label: Option<String>,
    /// Taille (octets).
    pub size: u64,
}

impl Device {
    /// L'emplacement à montrer : son nom, sinon sa taille (« Support de 31,9 Go »).
    pub fn place(&self) -> Place {
        let name = self.label.clone().unwrap_or_else(|| {
            crate::t!(
                "places-device",
                size = crate::i18n::decimal(self.size as f64 / 1e9, 1)
            )
        });
        Place {
            name,
            path: self.path.clone(),
            kind: PlaceKind::Device,
        }
    }
}

/// Supports amovibles branchés mais pas montés, d'après `lsblk` (Linux ; rien ailleurs :
/// macOS et Windows montent tout d'eux-mêmes). `lsblk` absent : rien.
pub fn unmounted_devices() -> Vec<Device> {
    if std::env::consts::OS != "linux" {
        return Vec::new();
    }
    crate::process::command("lsblk")
        .args([
            "-J",
            "-b",
            "-l",
            "-o",
            "PATH,LABEL,SIZE,FSTYPE,MOUNTPOINT,RM,HOTPLUG,TYPE",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| parse_lsblk(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// Systèmes de fichiers qu'on ne monte pas pour y chercher des photos.
const NOT_MOUNTABLE: [&str; 5] = [
    "swap",
    "crypto_LUKS",
    "LVM2_member",
    "linux_raid_member",
    "zfs_member",
];

/// Supports pas montés d'après la sortie JSON de `lsblk` (`-J -b -l`) : partitions, ou
/// disques sans partition (carte formatée d'un bloc), avec un système de fichiers, sur un
/// support amovible (`RM`) ou branché à chaud (`HOTPLUG` : lecteurs de cartes USB, qui
/// se disent souvent fixes). Les anciennes versions écrivent booléens et nombres en
/// texte (`"1"`).
fn parse_lsblk(json: &str) -> Vec<Device> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let text = |d: &serde_json::Value, key: &str| {
        d.get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let flag = |d: &serde_json::Value, key: &str| match d.get(key) {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => s == "1",
        Some(serde_json::Value::Number(n)) => n.as_u64() == Some(1),
        _ => false,
    };
    let size = |d: &serde_json::Value| match d.get("size") {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(serde_json::Value::String(s)) => s.parse().unwrap_or(0),
        _ => 0,
    };
    value
        .get("blockdevices")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter(|d| matches!(text(d, "type").as_deref(), Some("part" | "disk")))
        .filter(|d| text(d, "mountpoint").is_none())
        .filter(|d| flag(d, "rm") || flag(d, "hotplug"))
        .filter(|d| text(d, "fstype").is_some_and(|f| !NOT_MOUNTABLE.contains(&f.as_str())))
        .filter_map(|d| {
            Some(Device {
                path: PathBuf::from(text(d, "path")?),
                label: text(d, "label"),
                size: size(d),
            })
        })
        .collect()
}

/// Monte le support `device` (`/dev/sdb1`) comme le ferait le gestionnaire de fichiers,
/// sans mot de passe pour un support amovible : `udisksctl mount` (Linux), `diskutil
/// mount` (macOS). Renvoie le dossier où il est monté. Bloquant (une seconde ou deux) :
/// hors du fil de l'interface.
pub fn mount(device: &Path) -> Result<PathBuf, String> {
    let program = match std::env::consts::OS {
        "macos" => "diskutil",
        "linux" => "udisksctl",
        _ => return Err(crate::t!("places-mount-unsupported")),
    };
    let mut command = crate::process::command(program);
    command.arg("mount");
    if program == "udisksctl" {
        command.arg("-b");
    }
    let output = command
        .arg(device)
        .output()
        .map_err(|e| format!("{program} : {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = Some(stderr.trim())
            .filter(|t| !t.is_empty())
            .unwrap_or(stdout.trim());
        return Err(format!("{program} : {message}"));
    }
    parse_mounted(&stdout)
        .or_else(|| {
            let mounts = fs::read_to_string("/proc/mounts").unwrap_or_default();
            mount_point_of(&mounts, device)
        })
        .ok_or_else(|| format!("{program} : {}", stdout.trim()))
}

/// Dossier lu dans la réponse de `udisksctl mount` : « Mounted /dev/sdb1 at
/// /media/moi/CARTE » (point final selon les versions).
fn parse_mounted(stdout: &str) -> Option<PathBuf> {
    let (_, at) = stdout.trim().split_once(" at ")?;
    let at = at.strip_suffix('.').unwrap_or(at);
    at.starts_with('/').then(|| PathBuf::from(at))
}

/// Point de montage de `device` d'après le contenu de `/proc/mounts`.
fn mount_point_of(mounts: &str, device: &Path) -> Option<PathBuf> {
    let wanted = device.to_string_lossy();
    mounts.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let dev = fields.next()?;
        let point = fields.next()?.replace("\\040", " ");
        (dev == wanted).then(|| PathBuf::from(point))
    })
}

/// Lance `program` avec `args` ; en cas d'échec, sa sortie d'erreur (ou standard).
fn run(program: &str, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let output = crate::process::command(program)
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

/// Volumes de `places` qui sont des points de montage d'après le contenu de
/// `/proc/mounts` : un dossier de `/media/<utilisateur>` ou de `/mnt` resté après un
/// démontage n'est plus un volume. `/proc/mounts` illisible (autre Unix) : tous gardés.
fn mounted_only(places: Vec<Place>, mounts: &str) -> Vec<Place> {
    let points: HashSet<PathBuf> = mounts
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(|point| PathBuf::from(point.replace("\\040", " ")))
        .collect();
    if points.is_empty() {
        return places;
    }
    places
        .into_iter()
        .filter(|p| points.contains(&p.path))
        .collect()
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
    /// Dossier déplié dont la liste est encore lue en fond (montage lent).
    pub loading: bool,
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

/// Lecteur des sous-dossiers d'un dossier : [`subfolders`], ou un faux disque lent
/// dans les tests ([`Tree::set_reader`]).
pub type Reader = Arc<dyn Fn(&Path, bool) -> io::Result<Vec<PathBuf>> + Send + Sync>;

/// Résultat d'une lecture faite en fond pour l'arborescence.
enum Read {
    /// Sous-dossiers d'un dossier, pour la lecture numéro `u64`.
    Children(PathBuf, u64, Result<Vec<PathBuf>, String>),
    /// Favoris présents, pour la vérification numéro `u64`.
    Present(u64, Vec<PathBuf>),
}

/// Lectures du disque de l'arborescence faites dans des fils : un montage lent (iCloud,
/// NAS) ne doit jamais figer l'interface (retour du 16/11/2026 : fenêtre « ne répond
/// pas », jusqu'à la fermeture). Les résultats sont déposés ici, repris par
/// [`Tree::poll`].
#[derive(Clone)]
struct Reads {
    done: Arc<(Mutex<Vec<Read>>, Condvar)>,
    reader: Reader,
    /// Appelé à l'arrivée d'un résultat (redessiner l'interface).
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Default for Reads {
    fn default() -> Self {
        Reads {
            done: Arc::default(),
            reader: Arc::new(subfolders),
            wake: None,
        }
    }
}

impl std::fmt::Debug for Reads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Reads")
    }
}

/// Arborescence des dossiers à partir des emplacements, indépendante de l'interface : ce
/// qui est déplié, les sous-dossiers déjà lus (lecture seulement au dépliage), et les
/// lignes visibles à plat ([`Tree::rows`]), qu'une interface graphique ou un TUI n'ont
/// plus qu'à dessiner. Un dossier présent sous deux emplacements (`/Volumes/SD` sous
/// « Volumes » et sous la racine) est déplié aux deux endroits ; dans la section des
/// favoris, en revanche, le dépliage est à part (retour du 22/10/2026 : se promener dans
/// un favori dépliait aussi le vrai dossier, et l'arborescence bougeait toute seule).
#[derive(Clone, Debug)]
pub struct Tree {
    places: Vec<Place>,
    filter: PlaceSettings,
    hidden: bool,
    /// Dossiers dépliés : (dans la section des favoris, dossier).
    expanded: HashSet<(bool, PathBuf)>,
    children: HashMap<PathBuf, Result<Vec<PathBuf>, String>>,
    /// Ligne pointée au clavier : son rang dans [`Tree::rows`] et son dossier (un dossier
    /// peut paraître deux fois ; le rang départage, le dossier retrouve la ligne quand
    /// d'autres apparaissent au-dessus).
    cursor: Option<(usize, PathBuf)>,
    /// Dossiers choisis (#606 : supprimer, copier, déplacer plusieurs dossiers d'un coup),
    /// dans l'ordre du choix.
    selection: Vec<PathBuf>,
    /// Départ des plages (Maj+clic) : le dernier dossier choisi un à un.
    anchor: Option<PathBuf>,
    /// Mode visuel (Maj+Espace) : son départ ; les déplacements choisissent la plage.
    visual: Option<PathBuf>,
    /// Favoris, dans l'ordre ([`favorites`]), et ceux qui existent (montrés en tête).
    favorites: Vec<PathBuf>,
    present: Vec<PathBuf>,
    /// Noms donnés aux favoris ([`favorite_names`]).
    names: HashMap<PathBuf, String>,
    /// Sections repliées : leurs lignes ne sont pas dans [`Tree::rows`].
    folded: HashSet<Section>,
    /// Partages réseau mémorisés ([`crate::servers::remembered`]), ajoutés aux
    /// emplacements quand ils n'y sont pas déjà (montés).
    servers: Vec<crate::servers::Server>,
    /// Serveurs annoncés sur le réseau local ([`crate::neighbourhood`]), ajoutés aux
    /// emplacements.
    neighbours: Vec<Place>,
    /// Attente d'une lecture faite en fond avant de rendre la main ; `None` : lectures
    /// faites sur place ([`Tree::with_places`], tests).
    wait: Option<Duration>,
    reads: Reads,
    /// Lectures en cours : dossier et numéro de la dernière demandée (seule retenue).
    pending: HashMap<PathBuf, u64>,
    /// Vérification des favoris en cours (son numéro).
    present_pending: Option<u64>,
    /// Dernier numéro de lecture donné.
    serial: u64,
}

impl Tree {
    /// Arborescence des emplacements de cette machine, sauf ceux que `filter` masque ;
    /// dossiers cachés seulement si `hidden`.
    pub fn new(filter: PlaceSettings, hidden: bool) -> Tree {
        let mut tree = Tree::with_places(places_filtered(&filter), hidden);
        tree.set_filter(filter);
        tree
    }

    /// Remplace les réglages des emplacements (volumes fixés, masqués) ; les lectures du
    /// disque se font désormais en fond, attendues `read_wait_ms`.
    pub fn set_filter(&mut self, filter: PlaceSettings) {
        self.wait = Some(Duration::from_millis(filter.read_wait_ms));
        self.filter = filter;
    }

    /// Fonction appelée à l'arrivée d'une lecture faite en fond (redessiner).
    pub fn set_wake(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        self.reads.wake = Some(Arc::new(wake));
    }

    /// Remplace le lecteur des sous-dossiers (tests : faux disque lent).
    pub fn set_reader(&mut self, reader: Reader) {
        self.reads.reader = reader;
    }

    /// Reprend les lectures arrivées ; vrai si l'arborescence a changé.
    pub fn poll(&mut self) -> bool {
        let reads =
            std::mem::take(&mut *self.reads.done.0.lock().unwrap_or_else(|e| e.into_inner()));
        let mut changed = false;
        for read in reads {
            match read {
                Read::Children(path, serial, result) => {
                    // Une lecture plus récente du même dossier est demandée : celle-ci
                    // est dépassée.
                    if self.pending.get(&path) == Some(&serial) {
                        self.pending.remove(&path);
                        self.children.insert(path, result);
                        changed = true;
                    }
                }
                Read::Present(serial, present) => {
                    if self.present_pending == Some(serial) {
                        self.present_pending = None;
                        self.present = present;
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    /// Vrai si des lectures sont encore en cours.
    pub fn is_loading(&self) -> bool {
        !self.pending.is_empty() || self.present_pending.is_some()
    }

    /// Attend la fin des lectures en cours, `timeout` au plus.
    pub fn settle_for(&mut self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        let done = self.reads.done.clone();
        loop {
            self.poll();
            let now = Instant::now();
            if !self.is_loading() || now >= deadline {
                return;
            }
            let (lock, ready) = &*done;
            let list = lock.lock().unwrap_or_else(|e| e.into_inner());
            if list.is_empty() {
                let _ = ready.wait_timeout(list, deadline - now);
            }
        }
    }

    /// Attend un instant les lectures en cours (disque local : comme sur place).
    fn settle(&mut self) {
        if let Some(wait) = self.wait {
            self.settle_for(wait);
        }
    }

    /// Lance `job` dans un fil ; son résultat est déposé pour [`Tree::poll`].
    fn spawn(&mut self, job: impl FnOnce() -> Read + Send + 'static) -> u64 {
        self.serial += 1;
        let reads = self.reads.clone();
        let _ = std::thread::Builder::new()
            .name("tri-tree".into())
            .spawn(move || {
                let read = job();
                let (lock, ready) = &*reads.done;
                lock.lock().unwrap_or_else(|e| e.into_inner()).push(read);
                // Le réveil avant le signal : qui attend ([`Tree::settle_for`]) trouve
                // l'interface déjà prévenue (sinon, course vue en CI le 18/11/2026).
                if let Some(wake) = &reads.wake {
                    wake();
                }
                ready.notify_all();
            });
        self.serial
    }

    /// Lit les sous-dossiers de `path` : sur place, ou en fond (sans attendre).
    fn read_children(&mut self, path: &Path) {
        if self.wait.is_none() {
            let read = (self.reads.reader)(path, self.hidden).map_err(|e| e.to_string());
            self.children.insert(path.to_path_buf(), read);
            return;
        }
        let (reader, hidden, dir) = (self.reads.reader.clone(), self.hidden, path.to_path_buf());
        // Numéro de cette lecture : celui que `spawn` va donner.
        let serial = self.serial + 1;
        self.pending.insert(path.to_path_buf(), serial);
        self.spawn(move || {
            let read = reader(&dir, hidden).map_err(|e| e.to_string());
            Read::Children(dir, serial, read)
        });
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
            selection: Vec::new(),
            anchor: None,
            visual: None,
            favorites: Vec::new(),
            present: Vec::new(),
            names: HashMap::new(),
            folded: HashSet::new(),
            servers: Vec::new(),
            neighbours: Vec::new(),
            wait: None,
            reads: Reads::default(),
            pending: HashMap::new(),
            present_pending: None,
            serial: 0,
        }
    }

    /// Remplace les partages réseau mémorisés, proposés dans la section « Réseau » : sous
    /// Windows, leur chemin UNC ; ailleurs, à rejoindre tant qu'ils ne sont pas montés.
    pub fn set_servers(&mut self, servers: Vec<crate::servers::Server>) {
        // Les emplacements des partages d'avant : à rejoindre (macOS, Linux) ou chemin UNC
        // (Windows, `PlaceKind::Network` : un partage oublié y restait, CI du 28/10/2026).
        let os = std::env::consts::OS;
        let old: Vec<PathBuf> = self.servers.iter().map(|s| s.place(os).path).collect();
        self.places
            .retain(|p| p.kind != PlaceKind::Server && !old.contains(&p.path));
        self.servers = servers;
        self.add_servers();
        self.add_neighbours();
    }

    /// Remplace les serveurs du voisinage réseau, proposés dans la section « Réseau »
    /// (un clic ouvre leur adresse dans la recherche des dossiers, pour choisir un
    /// partage).
    pub fn set_neighbours(&mut self, neighbours: Vec<Place>) {
        self.places.retain(|p| p.kind != PlaceKind::Neighbour);
        self.neighbours = neighbours;
        self.add_neighbours();
    }

    /// Ajoute aux emplacements les serveurs du voisinage qui n'y sont pas déjà.
    fn add_neighbours(&mut self) {
        for place in &self.neighbours {
            if !self.places.iter().any(|p| p.path == place.path) && !self.filter.hides(place) {
                self.places.push(place.clone());
            }
        }
    }

    /// Partages réseau mémorisés.
    pub fn servers(&self) -> &[crate::servers::Server] {
        &self.servers
    }

    /// Ajoute aux emplacements les partages mémorisés qui n'y sont pas déjà (montés, ou
    /// même chemin UNC).
    fn add_servers(&mut self) {
        if self.servers.is_empty() {
            return;
        }
        let os = std::env::consts::OS;
        let mounts = crate::servers::Mounts::read(os);
        for server in &self.servers {
            if os != "windows" && server.mounted_in(&mounts).is_some() {
                continue;
            }
            let place = server.place(os);
            if !self.places.iter().any(|p| p.path == place.path) && !self.filter.hides(&place) {
                self.places.push(place);
            }
        }
    }

    /// Remplace les favoris, montrés en tête, avant les emplacements ; ceux qui n'existent
    /// pas (volume démonté) sont omis jusqu'au prochain [`Tree::set_places`] ou
    /// [`Tree::refresh`].
    pub fn set_favorites(&mut self, favorites: Vec<PathBuf>) {
        self.favorites = favorites;
        self.check_favorites();
    }

    /// Remplace les noms donnés aux favoris ([`favorite_names`]), montrés dans leur
    /// section à la place du nom du dossier.
    pub fn set_favorite_names(&mut self, names: HashMap<PathBuf, String>) {
        self.names = names;
    }

    /// Nom donné au favori `path`, s'il en a un.
    pub fn favorite_name(&self, path: &Path) -> Option<&str> {
        self.names.get(path).map(String::as_str)
    }

    /// Favoris (tous, absents compris).
    pub fn favorites(&self) -> &[PathBuf] {
        &self.favorites
    }

    /// Vrai si `path` est un favori.
    pub fn is_favorite(&self, path: &Path) -> bool {
        self.favorites.iter().any(|p| p == path)
    }

    /// Favoris présents : vérifié une fois, pas à chaque dessin, et en fond (un NAS, un
    /// montage iCloud peuvent être lents) ; la liste d'avant reste en attendant.
    fn check_favorites(&mut self) {
        let favorites = self.favorites.clone();
        let present =
            move || -> Vec<PathBuf> { favorites.into_iter().filter(|p| p.is_dir()).collect() };
        if self.wait.is_none() {
            self.present = present();
            return;
        }
        let serial = self.serial + 1;
        self.present_pending = Some(serial);
        self.spawn(move || Read::Present(serial, present()));
        self.settle();
    }

    /// Emplacements.
    pub fn places(&self) -> &[Place] {
        &self.places
    }

    /// Vrai si `path` est déplié dans l'arborescence des emplacements (pas dans la section
    /// des favoris : voir [`Tree::toggle_row`]).
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(&(false, path.to_path_buf()))
    }

    /// Déplie `path` dans l'arborescence des emplacements (ses sous-dossiers sont lus la
    /// première fois).
    pub fn expand(&mut self, path: &Path) {
        self.expand_in(false, path);
    }

    /// Replie `path` dans l'arborescence des emplacements.
    pub fn collapse(&mut self, path: &Path) {
        self.expanded.remove(&(false, path.to_path_buf()));
    }

    /// Déplie ou replie `path` dans l'arborescence des emplacements.
    pub fn toggle(&mut self, path: &Path) {
        if self.is_expanded(path) {
            self.collapse(path);
        } else {
            self.expand(path);
        }
    }

    /// Déplie ou replie la ligne `index` de [`Tree::rows`], là où elle est : dans la
    /// section des favoris, sans toucher au même dossier ailleurs, et inversement.
    pub fn toggle_row(&mut self, index: usize) {
        let Some(row) = self.rows().into_iter().nth(index) else {
            return;
        };
        let favorites = row.section == Section::Favorites;
        if row.expanded {
            self.expanded.remove(&(favorites, row.path));
        } else {
            self.expand_in(favorites, &row.path);
        }
    }

    /// Déplie `path` dans la section des favoris (`favorites`) ou ailleurs ; ses
    /// sous-dossiers sont lus la première fois, attendus un instant.
    fn expand_in(&mut self, favorites: bool, path: &Path) {
        self.open_in(favorites, path);
        self.settle();
    }

    /// [`Tree::expand_in`], sans attendre la lecture.
    fn open_in(&mut self, favorites: bool, path: &Path) {
        if !self.children.contains_key(path) && !self.pending.contains_key(path) {
            self.read_children(path);
        }
        self.expanded.insert((favorites, path.to_path_buf()));
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
        self.open_in(false, &root);
        // Tous sauf le dossier lui-même : on le montre, sans forcément le déplier. Lus
        // ensemble, attendus une seule fois.
        for dir in chain.iter().take(chain.len().saturating_sub(1)) {
            self.open_in(false, dir);
        }
        self.settle();
        // Sa section aussi, si elle était repliée (retour du 22/11/2026 : « Montrer dans
        // son dossier » dépliait le chemin dans une section repliée, on ne voyait rien).
        if let Some(section) = self
            .all_rows()
            .iter()
            .find(|r| r.path == root)
            .map(|r| r.section)
        {
            self.folded.remove(&section);
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

    /// Vrai si `path` est un partage réseau monté qu'on peut déconnecter (pas iCloud).
    pub fn can_disconnect(&self, path: &Path) -> bool {
        self.places
            .iter()
            .any(|p| p.path == path && p.kind == PlaceKind::Network)
            && !is_cloud_drive(path)
    }

    /// Remplace les emplacements (liste fournie par un [`Monitor`]) ; ce qui était déplié
    /// le reste.
    pub fn set_places(&mut self, places: Vec<Place>) {
        let before: HashSet<PathBuf> = self.places.iter().map(|p| p.path.clone()).collect();
        self.places = places;
        self.add_servers();
        self.add_neighbours();
        self.check_favorites();
        // Un volume revenu (carte remise) : ce qu'on en avait lu est relu. Déplié quand il
        // a été éjecté, il gardait l'échec de sa lecture d'alors, « illisible » (retour du
        // 26/11/2026).
        let arrived: Vec<PathBuf> = self
            .places
            .iter()
            .filter(|p| !before.contains(&p.path))
            .map(|p| p.path.clone())
            .collect();
        for root in arrived {
            self.children.retain(|p, _| !p.starts_with(&root));
            let unfolded: Vec<PathBuf> = self
                .expanded
                .iter()
                .map(|(_, p)| p.clone())
                .filter(|p| p.starts_with(&root))
                .collect();
            for path in unfolded {
                self.read_children(&path);
            }
        }
    }

    /// Relit les emplacements (carte SD insérée entre-temps) et les dossiers dépliés.
    pub fn refresh(&mut self) {
        self.places = places_filtered(&self.filter);
        self.add_servers();
        self.add_neighbours();
        self.check_favorites();
        self.reload();
    }

    /// Relit les dossiers dépliés (dossier créé ou supprimé entre-temps). En fond, leur
    /// liste d'avant reste affichée jusqu'à l'arrivée de la nouvelle.
    pub fn reload(&mut self) {
        let expanded: HashSet<PathBuf> = self.expanded.iter().map(|(_, p)| p.clone()).collect();
        if self.wait.is_none() {
            self.children.clear();
        } else {
            self.children.retain(|p, _| expanded.contains(p));
        }
        for path in &expanded {
            self.read_children(path);
        }
        self.settle();
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
    /// En mode visuel, la plage parcourue est choisie.
    pub fn move_cursor(&mut self, delta: isize) {
        let next = match self.cursor() {
            Some(i) => i.saturating_add_signed(delta),
            None => 0,
        };
        self.set_cursor(next);
        self.follow_visual();
    }

    // --- Sélection de plusieurs dossiers (#606) -----------------------------------------

    /// Sous-dossiers de `dir` déjà lus (dossier déplié au moins une fois) ; `None` : pas
    /// encore lus, ou illisibles.
    pub fn children_of(&self, dir: &Path) -> Option<Vec<PathBuf>> {
        self.children.get(dir)?.as_ref().ok().cloned()
    }

    /// Dossiers choisis, dans l'ordre du choix (vide : aucun).
    pub fn selection(&self) -> &[PathBuf] {
        &self.selection
    }

    /// Vrai si `path` est choisi.
    pub fn is_selected(&self, path: &Path) -> bool {
        self.selection.iter().any(|p| p == path)
    }

    /// Ajoute la ligne `index` au choix, ou l'en retire (Cmd/Ctrl+clic, Espace) ; elle
    /// devient le départ des plages. Au premier choix, le dossier `here` (celui où l'on
    /// était : le dossier ouvert, surligné) y entre aussi, comme dans le Finder (retour du
    /// 26/11/2026 : un seul des deux dossiers surlignés partait) ; jamais un emplacement.
    pub fn toggle_selected(&mut self, index: usize, here: Option<&Path>) {
        let rows = self.rows();
        // Un emplacement (volume, dossier personnel…) ne se choisit pas (retour du
        // 26/11/2026 : « Images » choisi au clavier écartait ses sous-dossiers choisis).
        let Some(path) = rows
            .get(index)
            .filter(|r| r.depth > 0)
            .map(|r| r.path.clone())
        else {
            return;
        };
        if self.selection.is_empty()
            && let Some(here) = here
            && here != path
            && rows.iter().any(|r| r.path == here && r.depth > 0)
        {
            self.selection.push(here.to_path_buf());
        }
        if let Some(at) = self.selection.iter().position(|p| *p == path) {
            self.selection.remove(at);
        } else {
            self.selection.push(path.clone());
        }
        self.anchor = Some(path);
    }

    /// Choisit les lignes du départ des plages (sinon le dossier `here`, où l'on était,
    /// sinon la ligne pointée) jusqu'à `index` (Maj+clic), à la place du choix d'avant.
    pub fn select_range(&mut self, index: usize, here: Option<&Path>) {
        let rows = self.rows();
        let from = self
            .anchor
            .as_ref()
            .and_then(|a| rows.iter().position(|r| r.path == *a))
            .or_else(|| here.and_then(|h| rows.iter().position(|r| r.path == h)))
            .or_else(|| self.cursor())
            .unwrap_or(index);
        let (lo, hi) = (
            from.min(index),
            from.max(index).min(rows.len().saturating_sub(1)),
        );
        self.selection = rows[lo..=hi]
            .iter()
            .filter(|r| r.depth > 0)
            .map(|r| r.path.clone())
            .collect();
        self.selection.dedup();
        if self.anchor.is_none() {
            self.anchor = rows.get(from).map(|r| r.path.clone());
        }
    }

    /// Entre dans le mode visuel (la ligne pointée choisie) ou en sort (Maj+Espace).
    pub fn toggle_visual(&mut self) {
        if self.visual.take().is_some() {
            return;
        }
        let Some(path) = self.cursor_path() else {
            return;
        };
        self.visual = Some(path.clone());
        self.anchor = Some(path);
        self.follow_visual();
    }

    /// Vrai en mode visuel.
    pub fn in_visual(&self) -> bool {
        self.visual.is_some()
    }

    /// Mode visuel : la plage de son départ à la ligne pointée est choisie.
    fn follow_visual(&mut self) {
        if self.visual.is_some()
            && let Some(i) = self.cursor()
        {
            self.anchor.clone_from(&self.visual);
            self.select_range(i, None);
        }
    }

    /// Vide le choix et sort du mode visuel. Vrai s'il y avait quelque chose à vider.
    pub fn clear_selection(&mut self) -> bool {
        let had = !self.selection.is_empty() || self.visual.is_some();
        self.selection.clear();
        self.visual = None;
        self.anchor = None;
        had
    }

    /// Les dossiers sur lesquels agir (supprimer, copier, déplacer) : le choix s'il y en
    /// a un, sinon la ligne pointée. Jamais un emplacement (volume, dossier personnel,
    /// favori en tête) ; un dossier dont un parent est aussi choisi est laissé de côté
    /// (il part avec lui).
    pub fn acting(&self) -> Vec<PathBuf> {
        let chosen: Vec<PathBuf> = if self.selection.is_empty() {
            self.cursor_path().into_iter().collect()
        } else {
            self.selection.clone()
        };
        let roots: Vec<PathBuf> = self
            .rows()
            .into_iter()
            .filter(|r| r.depth == 0)
            .map(|r| r.path)
            .collect();
        let chosen: Vec<PathBuf> = chosen.into_iter().filter(|p| !roots.contains(p)).collect();
        chosen
            .iter()
            .filter(|p| {
                !chosen
                    .iter()
                    .any(|other| other != *p && p.starts_with(other))
            })
            .cloned()
            .collect()
    }

    /// Première (`first`) ou dernière ligne.
    pub fn cursor_to_end(&mut self, first: bool) {
        let last = self.rows().len().saturating_sub(1);
        self.set_cursor(if first { 0 } else { last });
        self.follow_visual();
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
            self.toggle_row(i);
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
            self.toggle_row(i);
            self.set_cursor(i);
        } else if let Some(parent) = (0..i).rev().find(|&j| rows[j].depth < row.depth) {
            self.set_cursor(parent);
        }
    }

    /// Lignes visibles, dans l'ordre d'affichage, section par section ([`Section`]) :
    /// volumes, dossier personnel, favoris présents, racine ; sous chaque emplacement,
    /// s'il est déplié, ses sous-dossiers, récursivement.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = self.all_rows();
        rows.retain(|r| !self.folded.contains(&r.section));
        rows
    }

    /// Sections qui ont des lignes, repliées comprises, dans l'ordre d'affichage : une
    /// section repliée garde son titre (pour la déplier).
    pub fn sections(&self) -> Vec<Section> {
        let mut sections: Vec<Section> = self.all_rows().iter().map(|r| r.section).collect();
        sections.dedup();
        sections
    }

    /// Replie (`folded`) ou déplie la section `section`.
    pub fn set_folded(&mut self, section: Section, folded: bool) {
        if folded {
            self.folded.insert(section);
        } else {
            self.folded.remove(&section);
        }
    }

    /// Vrai si la section `section` est repliée.
    pub fn is_folded(&self, section: Section) -> bool {
        self.folded.contains(&section)
    }

    /// Toutes les lignes, sections repliées comprises.
    fn all_rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut places: Vec<&Place> = self.places.iter().collect();
        // Tri stable : l'ordre des volumes entre eux est gardé.
        places.sort_by_key(|p| self.filter.section(p));
        let (before, after): (Vec<&Place>, Vec<&Place>) = places
            .into_iter()
            .partition(|p| self.filter.section(p) < Section::Favorites);
        for place in before {
            self.push_place(&mut rows, place);
        }
        for path in &self.present {
            let first = rows.len();
            let name = self
                .favorite_name(path)
                .map_or_else(|| folder_name(path), str::to_string);
            self.push_rows(&mut rows, path, name, 0, Section::Favorites);
            rows[first].kind = Some(PlaceKind::Favorite);
        }
        for place in after {
            self.push_place(&mut rows, place);
        }
        rows
    }

    fn push_place(&self, rows: &mut Vec<Row>, place: &Place) {
        let first = rows.len();
        let section = self.filter.section(place);
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
        let expanded = self
            .expanded
            .contains(&(section == Section::Favorites, path.to_path_buf()));
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
            loading: expanded && !self.children.contains_key(path),
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

    #[test]
    fn a_busy_volume_is_told_apart_from_other_refusals() {
        assert!(is_busy(
            "udisksctl : Error unmounting /dev/sdb1: GDBus.Error:org.freedesktop.UDisks2.Error.DeviceBusy: Error unmounting /dev/sdb1: target is busy"
        ));
        assert!(is_busy(
            "Volume CARTE on disk4s1 failed to unmount: dissented by PID 42"
        ));
        assert!(!is_busy("Error looking up object for device /dev/sdz1"));
    }

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            vars.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| OsString::from(v))
        }
    }

    #[test]
    fn a_leftover_mount_folder_is_not_a_volume() {
        let place = |path: &str| Place {
            name: folder_name(Path::new(path)),
            path: PathBuf::from(path),
            kind: PlaceKind::Volume,
        };
        let places = vec![
            place("/media/moi/disk"),
            place("/media/moi/CARTE SD"),
            place("/mnt/vide"),
        ];
        let mounts = "/dev/sda2 / ext4 rw 0 0\n\
                      /dev/sdc1 /media/moi/CARTE\\040SD vfat rw 0 0\n";
        let kept: Vec<PathBuf> = mounted_only(places.clone(), mounts)
            .into_iter()
            .map(|p| p.path)
            .collect();
        assert_eq!(kept, [PathBuf::from("/media/moi/CARTE SD")]);
        // Rien de lisible : tous gardés.
        assert_eq!(mounted_only(places, "").len(), 3);
    }

    /// Faux disque lent : chaque lecture prend `delay`, puis lit le vrai dossier.
    fn slow_reader(delay: Duration) -> Reader {
        Arc::new(move |dir: &Path, hidden: bool| {
            std::thread::sleep(delay);
            subfolders(dir, hidden)
        })
    }

    #[test]
    fn several_folders_are_chosen_and_acted_upon_together() {
        // #606 : Cmd/Ctrl+clic, Maj+clic, mode visuel ; jamais l'emplacement ; un
        // sous-dossier d'un dossier choisi part avec lui.
        let dir = tempfile::tempdir().unwrap();
        for name in ["a", "b", "c", "d"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        std::fs::create_dir(dir.path().join("b/sous")).unwrap();
        let place = Place {
            name: "Photos".into(),
            path: dir.path().to_path_buf(),
            kind: PlaceKind::Home,
        };
        let mut tree = Tree::with_places(vec![place], false);
        tree.expand(dir.path());
        let at = |tree: &Tree, name: &str| {
            tree.rows()
                .iter()
                .position(|r| r.path == dir.path().join(name))
                .unwrap()
        };
        let names = |paths: Vec<PathBuf>| -> Vec<String> {
            paths
                .iter()
                .map(|p| p.strip_prefix(dir.path()).unwrap().display().to_string())
                .collect()
        };
        // Rien de choisi : la ligne pointée ; l'emplacement, jamais, ni choisi (il
        // écarterait ses sous-dossiers choisis).
        tree.set_cursor(0);
        assert!(tree.acting().is_empty(), "pas l'emplacement");
        tree.toggle_selected(0, None);
        assert!(
            tree.selection().is_empty(),
            "un emplacement ne se choisit pas"
        );
        tree.select_range(at(&tree, "a"), None);
        assert_eq!(
            names(tree.acting()),
            ["a"],
            "plage depuis l'emplacement : sans lui"
        );
        tree.clear_selection();
        tree.set_cursor(at(&tree, "a"));
        assert_eq!(names(tree.acting()), ["a"]);
        // Cmd/Ctrl+clic depuis a (pointée) : a et c ; encore c : retiré.
        tree.toggle_selected(at(&tree, "c"), Some(&dir.path().join("a")));
        assert_eq!(
            names(tree.acting()),
            ["a", "c"],
            "le dossier où l'on était aussi"
        );
        tree.clear_selection();
        tree.toggle_selected(at(&tree, "a"), None);
        tree.toggle_selected(at(&tree, "c"), None);
        assert_eq!(names(tree.acting()), ["a", "c"]);
        tree.toggle_selected(at(&tree, "c"), None);
        assert_eq!(names(tree.acting()), ["a"]);
        // Maj+clic : du dernier dossier cliqué (a) à d.
        tree.clear_selection();
        tree.toggle_selected(at(&tree, "a"), None);
        tree.select_range(at(&tree, "d"), None);
        assert_eq!(names(tree.acting()), ["a", "b", "c", "d"]);
        // Un sous-dossier d'un dossier choisi part avec lui.
        tree.expand(&dir.path().join("b"));
        tree.toggle_selected(at(&tree, "b/sous"), None);
        assert!(tree.is_selected(&dir.path().join("b/sous")));
        assert_eq!(names(tree.acting()), ["a", "b", "c", "d"]);
        // Mode visuel : la plage suit le curseur.
        assert!(tree.clear_selection());
        tree.set_cursor(at(&tree, "a"));
        tree.toggle_visual();
        tree.move_cursor(1);
        assert_eq!(names(tree.acting()), ["a", "b"]);
        tree.cursor_to_end(false);
        assert_eq!(names(tree.acting()), ["a", "b", "c", "d"]);
        tree.toggle_visual();
        assert!(!tree.in_visual());
        assert!(tree.clear_selection());
        assert!(!tree.clear_selection(), "plus rien");
    }

    #[test]
    fn a_slow_folder_is_read_in_the_background() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Photos")).unwrap();
        let place = Place {
            name: "iCloud".into(),
            path: dir.path().to_path_buf(),
            kind: PlaceKind::Network,
        };
        let mut tree = Tree::with_places(vec![place], false);
        tree.set_filter(PlaceSettings {
            read_wait_ms: 20,
            ..PlaceSettings::default()
        });
        tree.set_reader(slow_reader(Duration::from_millis(500)));
        let woken = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = woken.clone();
        tree.set_wake(move || flag.store(true, std::sync::atomic::Ordering::Relaxed));
        // Déplié : la main revient sans attendre la lecture, la ligne le dit.
        let started = Instant::now();
        tree.expand(dir.path());
        assert!(started.elapsed() < Duration::from_millis(400), "pas figé");
        let rows = tree.rows();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].expanded && rows[0].loading && tree.is_loading());
        // La liste arrive en fond.
        tree.settle_for(Duration::from_secs(10));
        assert!(woken.load(std::sync::atomic::Ordering::Relaxed), "réveil");
        let rows = tree.rows();
        assert!(!rows[0].loading && !tree.is_loading());
        assert_eq!(rows[1].path, dir.path().join("Photos"));
        // Relu : la liste d'avant reste affichée en attendant la nouvelle.
        std::fs::create_dir(dir.path().join("Vidéos")).unwrap();
        tree.reload();
        let rows = tree.rows();
        assert_eq!(rows.len(), 2, "liste d'avant");
        assert!(!rows[0].loading && tree.is_loading());
        tree.settle_for(Duration::from_secs(10));
        assert_eq!(tree.rows().len(), 3, "nouvelle liste");
    }

    #[test]
    fn a_read_overtaken_by_a_newer_one_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let place = Place {
            name: "NAS".into(),
            path: dir.path().to_path_buf(),
            kind: PlaceKind::Network,
        };
        let mut tree = Tree::with_places(vec![place], false);
        tree.set_filter(PlaceSettings {
            read_wait_ms: 0,
            ..PlaceSettings::default()
        });
        // Première lecture lente (dossier vide), seconde rapide (un dossier créé).
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = calls.clone();
        tree.set_reader(Arc::new(move |dir: &Path, hidden: bool| {
            if count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                let empty = subfolders(dir, hidden);
                std::thread::sleep(Duration::from_millis(300));
                return empty;
            }
            subfolders(dir, hidden)
        }));
        tree.expand(dir.path());
        std::fs::create_dir(dir.path().join("Nouveau")).unwrap();
        tree.reload();
        tree.settle_for(Duration::from_secs(10));
        // La lecture lente arrive en dernier : ignorée.
        std::thread::sleep(Duration::from_millis(400));
        tree.poll();
        assert_eq!(tree.rows().len(), 2);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn favourites_are_checked_in_the_background() {
        let dir = tempfile::tempdir().unwrap();
        let (here, gone) = (dir.path().join("Là"), dir.path().join("Parti"));
        std::fs::create_dir(&here).unwrap();
        let mut tree = Tree::with_places(Vec::new(), false);
        tree.set_filter(PlaceSettings::default());
        tree.set_favorites(vec![here.clone(), gone]);
        tree.settle_for(Duration::from_secs(10));
        let rows = tree.rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, here);
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
    fn the_panel_lists_devices_drives_network_home_favorites_then_root() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["home", "card", "nas", "fav", "work"] {
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
                place("Work", dir.path().join("work"), PlaceKind::Volume),
                place("Clé", PathBuf::from("/dev/sdb1"), PlaceKind::Device),
            ],
            false,
        );
        // Un disque fixé va dans « Disques » ; cartes, clés montées ou non, dans
        // « Périphériques » (#113).
        tree.set_filter(PlaceSettings {
            fixed: vec!["Work".into()],
            ..PlaceSettings::default()
        });
        tree.set_favorites(vec![dir.path().join("fav")]);
        let rows = tree.rows();
        let got: Vec<(&str, Section)> = rows.iter().map(|r| (r.name.as_str(), r.section)).collect();
        assert_eq!(
            got,
            [
                ("SD", Section::Devices),
                ("Clé", Section::Devices),
                ("Work", Section::Drives),
                ("NAS", Section::Network),
                ("Perso", Section::Personal),
                ("fav", Section::Favorites),
                ("Racine", Section::System),
            ]
        );
        assert!(rows[1].ejectable, "un support pas monté s'éjecte aussi");
        assert!(!rows[2].ejectable);
        assert!(!rows[6].expanded, "racine repliée par défaut");
    }

    #[test]
    fn a_folded_section_keeps_its_title_but_not_its_rows() {
        let dir = tempfile::tempdir().unwrap();
        let place = |name: &str, kind| Place {
            name: name.into(),
            path: dir.path().to_path_buf(),
            kind,
        };
        let mut tree = Tree::with_places(
            vec![
                place("Perso", PlaceKind::Home),
                place("Racine", PlaceKind::Root),
            ],
            false,
        );
        tree.set_cursor(1);
        tree.set_folded(Section::Personal, true);
        assert!(tree.is_folded(Section::Personal));
        let names: Vec<String> = tree.rows().into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["Racine"]);
        assert_eq!(tree.sections(), [Section::Personal, Section::System]);
        assert_eq!(tree.cursor(), Some(0), "le curseur suit sa ligne");
        tree.set_folded(Section::Personal, false);
        assert_eq!(tree.rows().len(), 2);
        assert_eq!(Section::ALL.map(Section::key)[4], "favorites");
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
        assert_eq!(mac.len(), 4, "iCloud Drive : dans Réseau");
        let icloud = cloud_drives("macos", home);
        assert_eq!(
            icloud[0].path,
            home.join("Library/Mobile Documents/com~apple~CloudDocs"),
            "iCloud Drive, caché dans ~/Library"
        );
        assert_eq!(icloud[0].kind.section(), Section::Network);
        assert_eq!(
            cloud_drives("windows", home)[0].path,
            home.join("iCloudDrive")
        );
        assert!(cloud_drives("linux", home).is_empty());
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
            network_mounts_linux(proc, ""),
            [
                PathBuf::from("/mnt/nas photo"),
                PathBuf::from("/home/fab/srv")
            ]
        );
        // Montage automatique pas encore touché : `autofs`, réseau d'après fstab ; une
        // fois monté, une seule fois. Un `autofs` local (/boot/efi) n'est pas du réseau.
        let fstab = "# /etc/fstab\n\
                     UUID=1234 /boot/efi vfat noauto,x-systemd.automount 0 0\n\
                     nas:/volume1/photo /mnt/photo nfs4 noauto,x-systemd.automount,_netdev 0 0\n";
        let waiting = "/dev/sda1 / ext4 rw 0 0\n\
                       systemd-1 /mnt/photo autofs rw,fd=55 0 0\n\
                       systemd-1 /boot/efi autofs rw,fd=56 0 0\n";
        assert_eq!(
            network_mounts_linux(waiting, fstab),
            [PathBuf::from("/mnt/photo")]
        );
        let mounted = format!("{waiting}nas:/volume1/photo /mnt/photo nfs4 rw 0 0\n");
        assert_eq!(
            network_mounts_linux(&mounted, fstab),
            [PathBuf::from("/mnt/photo")]
        );
        assert!(network_mounts_linux(waiting, "").is_empty());
        // GVFS : Linux seulement ; « : » est interdit dans un nom de dossier sous Windows.
        if cfg!(windows) {
            return;
        }
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
    fn a_mounted_share_can_be_disconnected_but_not_icloud() {
        let share = PathBuf::from("/Volumes/photo");
        let mut places = vec![
            Place {
                name: "photo".into(),
                path: share.clone(),
                kind: PlaceKind::Network,
            },
            Place {
                name: "card".into(),
                path: "/Volumes/SD".into(),
                kind: PlaceKind::Volume,
            },
        ];
        if let Some(home) = home() {
            places.extend(cloud_drives(std::env::consts::OS, &home));
        }
        let icloud: Vec<PathBuf> = places
            .iter()
            .filter(|p| p.kind == PlaceKind::Network && p.path != share)
            .map(|p| p.path.clone())
            .collect();
        let tree = Tree::with_places(places, false);
        assert!(tree.can_disconnect(&share));
        assert!(
            !tree.can_disconnect(Path::new("/Volumes/SD")),
            "un disque s'éjecte"
        );
        for path in icloud {
            assert!(!tree.can_disconnect(&path), "iCloud : rien à déconnecter");
        }
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
    fn favorites_have_names_and_their_own_unfolding() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let root = dir.path().join("photos");
        for d in ["Sources/2026", "Autre/Sources"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let sources = root.join("Sources");
        let other = root.join("Autre/Sources");
        set_favorite(&data, &sources, true).unwrap();
        set_favorite(&data, &other, true).unwrap();
        // Un nom au premier ; nettoyé (une tabulation casserait le fichier).
        let names = name_favorite(&data, &sources, "  Sources\tdu NAS ").unwrap();
        assert_eq!(names[&sources], "Sourcesdu NAS");
        let names = name_favorite(&data, &sources, "Sources du NAS").unwrap();
        assert_eq!(names.len(), 1);
        // Ancien fichier (chemins seuls) toujours lu ; nom gardé en ajoutant un favori.
        let list = set_favorite(&data, &root, true).unwrap();
        assert_eq!(list, [sources.clone(), other.clone(), root.clone()]);
        assert_eq!(favorite_names(&data)[&sources], "Sources du NAS");
        let place = Place {
            name: "Test".into(),
            path: root.clone(),
            kind: PlaceKind::Home,
        };
        let mut tree = Tree::with_places(vec![place], false);
        tree.set_favorites(favorites(&data));
        tree.set_favorite_names(favorite_names(&data));
        let names: Vec<String> = tree.rows().iter().map(|r| r.name.clone()).collect();
        assert_eq!(names, ["Test", "Sources du NAS", "Sources", "photos"]);
        // Déplier le favori « Sources du NAS » ne déplie pas le vrai dossier, et inversement.
        tree.toggle_row(1);
        tree.expand(&root);
        let rows = tree.rows();
        let favorite = rows
            .iter()
            .position(|r| r.name == "Sources du NAS")
            .unwrap();
        assert!(rows[favorite].expanded);
        assert_eq!(
            rows[favorite + 1].name,
            "2026",
            "le favori montre son contenu"
        );
        let real = rows
            .iter()
            .position(|r| r.section != Section::Favorites && r.path == sources)
            .unwrap();
        assert!(!rows[real].expanded, "le vrai dossier reste replié");
        assert!(!tree.is_expanded(&sources));
        tree.toggle(&sources);
        // Le dossier personnel vient avant les favoris : ses lignes décalent le favori.
        let favorite = tree
            .rows()
            .iter()
            .position(|r| r.name == "Sources du NAS")
            .unwrap();
        tree.toggle_row(favorite);
        let rows = tree.rows();
        assert!(!rows[favorite].expanded, "favori replié");
        let real = rows
            .iter()
            .position(|r| r.section != Section::Favorites && r.path == sources)
            .unwrap();
        assert!(rows[real].expanded, "le vrai dossier, lui, reste déplié");
        // Le nom suit un renommage, part avec le favori, et un nom vide l'efface.
        std::fs::rename(&sources, root.join("Sources2")).unwrap();
        rename_favorites(&data, &sources, &root.join("Sources2")).unwrap();
        assert_eq!(
            favorite_names(&data)[&root.join("Sources2")],
            "Sources du NAS"
        );
        assert!(
            name_favorite(&data, &root.join("Sources2"), " ")
                .unwrap()
                .is_empty()
        );
        name_favorite(&data, &other, "Autre").unwrap();
        set_favorite(&data, &other, false).unwrap();
        assert!(favorite_names(&data).is_empty(), "retiré avec son favori");
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

    #[test]
    fn lsblk_gives_the_unmounted_removable_devices() {
        // #113 : sortie de `lsblk -J -b -l` (util-linux 2.39) : la carte pas montée
        // seulement ; ni le disque interne, ni la partition montée, ni le swap, ni le
        // disque partitionné lui-même (sans système de fichiers).
        let json = r#"{"blockdevices": [
            {"path":"/dev/nvme0n1p2","label":null,"size":500000000000,"fstype":"ext4","mountpoint":"/","rm":false,"hotplug":false,"type":"part"},
            {"path":"/dev/nvme0n1p3","label":null,"size":8000000000,"fstype":"swap","mountpoint":null,"rm":false,"hotplug":false,"type":"part"},
            {"path":"/dev/sdb","label":null,"size":31914983424,"fstype":null,"mountpoint":null,"rm":true,"hotplug":true,"type":"disk"},
            {"path":"/dev/sdb1","label":"X-T4","size":31910789120,"fstype":"exfat","mountpoint":null,"rm":true,"hotplug":true,"type":"part"},
            {"path":"/dev/sdc1","label":"CLE","size":16000000000,"fstype":"vfat","mountpoint":"/media/fab/CLE","rm":true,"hotplug":true,"type":"part"},
            {"path":"/dev/sdd1","label":"","size":64000000000,"fstype":"exfat","mountpoint":null,"rm":false,"hotplug":true,"type":"part"}
        ]}"#;
        let devices = parse_lsblk(json);
        let paths: Vec<&Path> = devices.iter().map(|d| d.path.as_path()).collect();
        assert_eq!(paths, [Path::new("/dev/sdb1"), Path::new("/dev/sdd1")]);
        assert_eq!(devices[0].label.as_deref(), Some("X-T4"));
        assert_eq!(devices[1].label, None, "nom vide : aucun");
        assert_eq!(devices[1].size, 64_000_000_000);
        // Anciennes versions : booléens et nombres en texte.
        let old = r#"{"blockdevices": [
            {"path":"/dev/sdb1","label":"SD","size":"1000","fstype":"vfat","mountpoint":null,"rm":"1","hotplug":"1","type":"part"}
        ]}"#;
        assert_eq!(parse_lsblk(old)[0].size, 1000);
        assert!(parse_lsblk("pas du JSON").is_empty());
    }

    #[test]
    fn a_device_without_a_name_is_shown_by_its_size() {
        let device = Device {
            path: PathBuf::from("/dev/sdb1"),
            label: None,
            size: 31_910_789_120,
        };
        let place = device.place();
        assert_eq!(place.kind, PlaceKind::Device);
        assert!(place.name.contains("31"), "{}", place.name);
    }

    #[test]
    fn the_mount_point_is_read_from_udisksctl_or_proc_mounts() {
        assert_eq!(
            parse_mounted("Mounted /dev/sdb1 at /media/fab/X-T4\n"),
            Some(PathBuf::from("/media/fab/X-T4"))
        );
        assert_eq!(
            parse_mounted("Mounted /dev/sdb1 at /media/fab/MA CARTE."),
            Some(PathBuf::from("/media/fab/MA CARTE"))
        );
        assert_eq!(parse_mounted("rien"), None);
        let mounts = "/dev/sda1 / ext4 rw 0 0\n/dev/sdb1 /media/fab/MA\\040CARTE exfat rw 0 0\n";
        assert_eq!(
            mount_point_of(mounts, Path::new("/dev/sdb1")),
            Some(PathBuf::from("/media/fab/MA CARTE"))
        );
    }

    #[test]
    fn a_card_put_back_is_read_again() {
        // Retour du 26/11/2026 : dépliée, éjectée puis remise, la carte restait
        // « illisible » (l'échec de sa lecture une fois partie).
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("CARTE");
        std::fs::create_dir_all(card.join("DCIM")).unwrap();
        let place = Place {
            name: "CARTE".into(),
            path: card.clone(),
            kind: PlaceKind::Volume,
        };
        let mut tree = Tree::with_places(vec![place.clone()], false);
        tree.expand(&card);
        let row = |tree: &Tree| tree.rows().into_iter().find(|r| r.path == card).unwrap();
        assert!(row(&tree).error.is_none());
        // Éjectée : lue partie, elle est illisible.
        std::fs::remove_dir_all(&card).unwrap();
        tree.set_places(Vec::new());
        tree.reload();
        // Remise.
        std::fs::create_dir_all(card.join("DCIM")).unwrap();
        tree.set_places(vec![place]);
        assert!(row(&tree).error.is_none(), "relue");
    }
}
