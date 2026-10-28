//! Partages réseau (SMB) qu'on rejoint par leur adresse : `\\nas\photo` (Windows) ou
//! `smb://nas/photo` (partout), comme « Se connecter au serveur » du Finder (demandé le
//! 28/10/2026 : sous Windows, rien ne permettait d'atteindre un partage pas attribué à une
//! lettre de lecteur). Le montage est laissé au système, qui demande lui-même le mot de
//! passe : Windows ouvre le chemin UNC directement, macOS monte par `mount volume`
//! (trousseau, boîte du système), Linux par `gio mount` (GVFS, comme Nemo ou Nautilus).
//! Les partages rejoints sont mémorisés (fichier `servers` du dossier de données) et
//! proposés dans la section « Réseau » du panneau des dossiers. Voir `doc/en/servers.md`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::places::{Place, PlaceKind};

/// Un partage SMB : serveur, partage et, éventuellement, un dossier dedans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Server {
    /// Utilisateur donné dans l'adresse (`smb://moi@nas/photo`), proposé par le système.
    pub user: Option<String>,
    /// Nom ou adresse du serveur (`nas`, `nas.local`, `192.168.1.10`).
    pub host: String,
    /// Nom du partage (`photo`).
    pub share: String,
    /// Dossiers sous le partage (`2026/séance`), à ouvrir une fois connecté.
    pub folders: Vec<String>,
}

impl Server {
    /// Lit une adresse tapée : `\\serveur\partage[\dossiers]` (barres obliques acceptées
    /// aussi : `\\nas/photo`) ou `smb://[utilisateur@]serveur/partage[/dossiers]`
    /// (caractères encodés `%20` décodés). `None` si ce n'est pas une adresse de partage
    /// (il faut au moins un serveur et un partage).
    pub fn parse(text: &str) -> Option<Server> {
        let text = text.trim();
        let (rest, url) = if let Some(rest) = text.strip_prefix(r"\\") {
            (rest, false)
        } else if text.len() > 6 && text[..6].eq_ignore_ascii_case("smb://") {
            (&text[6..], true)
        } else {
            return None;
        };
        let mut parts = rest.split(['\\', '/']).filter(|p| !p.is_empty()).map(|p| {
            if url {
                percent_decode(p)
            } else {
                p.to_string()
            }
        });
        let host = parts.next()?;
        let (user, host) = match host.rsplit_once('@') {
            Some((user, host)) if url && !user.is_empty() => (Some(user.to_string()), host),
            _ => (None, host.as_str()),
        };
        let host = host.to_string();
        let share = parts.next()?;
        if host.is_empty() || host.contains('@') {
            return None;
        }
        Some(Server {
            user,
            host,
            share,
            folders: parts.collect(),
        })
    }

    /// Le partage seul, sans les dossiers dedans (ce qu'on mémorise).
    pub fn root(&self) -> Server {
        Server {
            folders: Vec::new(),
            ..self.clone()
        }
    }

    /// Adresse `smb://[utilisateur@]serveur/partage[/dossiers]`, caractères spéciaux
    /// encodés.
    pub fn address(&self) -> String {
        let user = self
            .user
            .as_ref()
            .map(|u| format!("{}@", percent_encode(u)))
            .unwrap_or_default();
        let mut address = format!(
            "smb://{user}{}/{}",
            percent_encode(&self.host),
            percent_encode(&self.share)
        );
        for folder in &self.folders {
            address.push('/');
            address.push_str(&percent_encode(folder));
        }
        address
    }

    /// Chemin UNC du partage (`\\nas\photo`), sans les dossiers dedans.
    pub fn unc(&self) -> PathBuf {
        PathBuf::from(format!(r"\\{}\{}", self.host, self.share))
    }

    /// Nom affiché : « partage (serveur) », comme les partages montés par GVFS.
    pub fn name(&self) -> String {
        format!("{} ({})", self.share, self.host)
    }

    /// Vrai si `other` désigne le même partage (casse ignorée, comme SMB ; le serveur
    /// comparé à son premier nom : `nas` = `nas.local` = `nas._smb._tcp.local`).
    pub fn same_share(&self, other: &Server) -> bool {
        same_host(&self.host, &other.host) && self.share.eq_ignore_ascii_case(&other.share)
    }

    /// L'emplacement du panneau des dossiers pour ce partage mémorisé, sous le système
    /// `os` : sous Windows, son chemin UNC, un dossier comme un autre ; ailleurs, un
    /// partage à rejoindre ([`PlaceKind::Server`], chemin = son adresse), tant qu'il n'est
    /// pas monté.
    pub fn place(&self, os: &str) -> Place {
        let root = self.root();
        if os == "windows" {
            Place {
                name: root.name(),
                path: root.unc(),
                kind: PlaceKind::Network,
            }
        } else {
            Place {
                name: root.name(),
                path: PathBuf::from(root.address()),
                kind: PlaceKind::Server,
            }
        }
    }

    /// Le dossier où ce partage est monté (`/Volumes/photo`, GVFS, CIFS), d'après les
    /// montages `mounts` ; ses dossiers ajoutés.
    pub fn mounted_in(&self, mounts: &Mounts) -> Option<PathBuf> {
        let point = mounts
            .shares
            .iter()
            .find(|(share, _)| share.same_share(self))
            .map(|(_, point)| point.clone())?;
        Some(self.folders.iter().fold(point, |p, f| p.join(f)))
    }

    /// Rejoint le partage sous le système `os` et renvoie son dossier (dossiers compris).
    /// Peut prendre plusieurs secondes (serveur lent, boîte du mot de passe) : à appeler
    /// dans un fil à part.
    pub fn connect(&self, os: &str) -> Result<PathBuf, String> {
        if os == "windows" {
            let root = self.unc();
            fs::read_dir(&root)
                .map_err(|e| crate::t!("servers-unreachable", error = e.to_string()))?;
            return Ok(self.folders.iter().fold(root, |p, f| p.join(f)));
        }
        if let Some(dir) = self.mounted_in(&Mounts::read(os)) {
            return Ok(dir);
        }
        let address = self.root().address();
        let mut command = if os == "macos" {
            // Boîte du système (utilisateur, mot de passe, trousseau) ; l'adresse est
            // encodée, sans guillemet qui couperait la chaîne AppleScript.
            let mut c = crate::process::command("osascript");
            c.args(["-e", &format!("mount volume \"{address}\"")]);
            c
        } else {
            let mut c = crate::process::command("gio");
            c.args(["mount", &address]);
            c
        };
        let output = command
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| crate::t!("servers-tool-missing", error = e.to_string()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::t!(
                "servers-refused",
                error = stderr.trim().to_string()
            ));
        }
        self.mounted_in(&Mounts::read(os))
            .ok_or_else(|| crate::t!("servers-not-found", name = self.name()))
    }
}

/// Vrai si `text` commence comme une adresse de partage (`\\…`, `smb://…`), complète ou
/// pas encore.
pub fn looks_like_address(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with(r"\\")
        || text
            .get(..6)
            .is_some_and(|start| start.eq_ignore_ascii_case("smb://"))
}

/// Même serveur : premiers noms égaux, casse ignorée (`NAS` = `nas.local`). Une adresse
/// IP est comparée entière.
fn same_host(a: &str, b: &str) -> bool {
    let first = |h: &str| {
        if h.parse::<std::net::IpAddr>().is_ok() {
            h.to_lowercase()
        } else {
            h.split('.').next().unwrap_or(h).to_lowercase()
        }
    };
    first(a) == first(b)
}

/// `%20` → espace ; un `%` mal formé reste tel quel.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Encode tout sauf les lettres, chiffres et `-._~` (RFC 3986).
fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Partages SMB montés sur cette machine, avec leur dossier.
#[derive(Clone, Debug, Default)]
pub struct Mounts {
    shares: Vec<(Server, PathBuf)>,
}

impl Mounts {
    /// Lit les montages du système `os` : sortie de `mount` (macOS) ; `/proc/mounts`
    /// (CIFS) et les montages GVFS (Linux). Rien sous Windows (chemins UNC).
    pub fn read(os: &str) -> Mounts {
        match os {
            "macos" => crate::process::command("/sbin/mount")
                .output()
                .map(|o| Mounts::from_mount_output(&String::from_utf8_lossy(&o.stdout)))
                .unwrap_or_default(),
            "windows" => Mounts::default(),
            _ => {
                let mut mounts = Mounts::from_proc_mounts(
                    &fs::read_to_string("/proc/mounts").unwrap_or_default(),
                );
                if let Some(gvfs) = gvfs_dir() {
                    mounts.shares.extend(Mounts::from_gvfs(&gvfs).shares);
                }
                mounts
            }
        }
    }

    /// Sortie de `mount` (macOS) : `//moi@nas._smb._tcp.local/photo on /Volumes/photo
    /// (smbfs, …)`.
    pub fn from_mount_output(text: &str) -> Mounts {
        let shares = text
            .lines()
            .filter(|l| l.contains("(smbfs"))
            .filter_map(|line| {
                let (source, rest) = line.split_once(" on ")?;
                let (point, _) = rest.rsplit_once(" (")?;
                let server = Server::parse(&format!("smb:{source}"))?;
                Some((server, PathBuf::from(point)))
            })
            .collect();
        Mounts { shares }
    }

    /// Contenu de `/proc/mounts` (Linux) : `//nas/photo /mnt/nas\040photo cifs …`.
    pub fn from_proc_mounts(text: &str) -> Mounts {
        let shares = text
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let source = fields.next()?.replace("\\040", " ");
                let point = fields.next()?.replace("\\040", " ");
                let kind = fields.next()?;
                if !matches!(kind, "cifs" | "smb3" | "smbfs") {
                    return None;
                }
                let server = Server::parse(&format!("smb:{}", source.replace('\\', "/")))?;
                Some((server, PathBuf::from(point)))
            })
            .collect();
        Mounts { shares }
    }

    /// Montages GVFS dans `dir` (`/run/user/<uid>/gvfs`) :
    /// `smb-share:server=nas.local,share=photo[,user=moi]`.
    pub fn from_gvfs(dir: &Path) -> Mounts {
        let shares = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let fields = name.strip_prefix("smb-share:")?;
                let field = |key: &str| {
                    fields
                        .split(',')
                        .find_map(|f| f.strip_prefix(&format!("{key}=")))
                        .map(percent_decode)
                };
                let server = Server {
                    user: field("user"),
                    host: field("server")?,
                    share: field("share")?,
                    folders: Vec::new(),
                };
                Some((server, entry.path()))
            })
            .collect();
        Mounts { shares }
    }
}

/// Dossier des montages GVFS de l'utilisateur (`/run/user/<uid>/gvfs`).
fn gvfs_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let uid = fs::metadata("/proc/self").ok()?.uid();
        Some(PathBuf::from(format!("/run/user/{uid}/gvfs")))
    }
    #[cfg(not(unix))]
    None
}

/// Fichier des partages mémorisés, dans le dossier de données : une adresse `smb://` par
/// ligne, dans l'ordre où ils ont été rejoints.
const SERVERS_FILE: &str = "servers";

/// Partages mémorisés (dossier de données `data`).
pub fn remembered(data: &Path) -> Vec<Server> {
    fs::read_to_string(data.join(SERVERS_FILE))
        .unwrap_or_default()
        .lines()
        .filter_map(Server::parse)
        .collect()
}

fn write(data: &Path, list: &[Server]) -> io::Result<()> {
    let text: String = list.iter().map(|s| s.address() + "\n").collect();
    fs::create_dir_all(data)?;
    fs::write(data.join(SERVERS_FILE), text)
}

/// Mémorise le partage de `server` (sans ses dossiers), s'il ne l'est pas déjà. Renvoie la
/// liste.
pub fn remember(data: &Path, server: &Server) -> io::Result<Vec<Server>> {
    let mut list = remembered(data);
    if !list.iter().any(|s| s.same_share(server)) {
        list.push(server.root());
        write(data, &list)?;
    }
    Ok(list)
}

/// Oublie le partage de `server`. Renvoie la liste.
pub fn forget(data: &Path, server: &Server) -> io::Result<Vec<Server>> {
    let mut list = remembered(data);
    list.retain(|s| !s.same_share(server));
    write(data, &list)?;
    Ok(list)
}

/// Le partage mémorisé dont `path` est l'emplacement ([`Server::place`]) sous le système
/// `os` : son adresse (ailleurs que sous Windows) ou son chemin UNC.
pub fn of_place<'a>(servers: &'a [Server], path: &Path, os: &str) -> Option<&'a Server> {
    servers.iter().find(|s| s.place(os).path == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(host: &str, share: &str, folders: &[&str]) -> Server {
        Server {
            user: None,
            host: host.into(),
            share: share.into(),
            folders: folders.iter().map(|f| f.to_string()).collect(),
        }
    }

    #[test]
    fn unc_paths_and_smb_addresses_are_read() {
        assert_eq!(
            Server::parse(r"\\truc\machin"),
            Some(server("truc", "machin", &[]))
        );
        assert_eq!(
            Server::parse(r"  \\nas\photo\2026\séance\ "),
            Some(server("nas", "photo", &["2026", "séance"]))
        );
        assert_eq!(
            Server::parse("\\\\nas/photo"),
            Some(server("nas", "photo", &[]))
        );
        assert_eq!(
            Server::parse("SMB://moi@nas.local/Mes%20photos/2026"),
            Some(Server {
                user: Some("moi".into()),
                ..server("nas.local", "Mes photos", &["2026"])
            })
        );
        assert!(looks_like_address("smb://") && looks_like_address(r" \\na"));
        assert!(looks_like_address("SMB://nas") && !looks_like_address("/smb://"));
        assert!(!looks_like_address("sm") && !looks_like_address(r"\a"));
        for not in [
            r"\\nas",
            r"\\",
            "smb://nas",
            "smb://",
            "/Volumes/photo",
            "C:\\photo",
            "nas/photo",
        ] {
            assert_eq!(Server::parse(not), None, "{not}");
        }
    }

    #[test]
    fn an_address_is_encoded_and_reads_back() {
        let s = Server {
            user: Some("moi".into()),
            ..server("nas", "Mes photos", &["été 2026"])
        };
        assert_eq!(
            s.address(),
            "smb://moi@nas/Mes%20photos/%C3%A9t%C3%A9%202026"
        );
        assert_eq!(Server::parse(&s.address()), Some(s.clone()));
        assert_eq!(s.root().address(), "smb://moi@nas/Mes%20photos");
        assert_eq!(s.unc(), PathBuf::from(r"\\nas\Mes photos"));
        assert_eq!(s.name(), "Mes photos (nas)");
        // Pas de guillemet dans l'adresse (AppleScript).
        assert!(!server("a\"b", "c\"d", &[]).address().contains('"'));
    }

    #[test]
    fn the_same_share_ignores_case_and_the_domain() {
        let a = server("NAS", "Photo", &[]);
        assert!(a.same_share(&server("nas._smb._tcp.local", "photo", &["x"])));
        assert!(!a.same_share(&server("nas", "musique", &[])));
        assert!(!a.same_share(&server("autre", "photo", &[])));
        assert!(server("192.168.1.10", "p", &[]).same_share(&server("192.168.1.10", "P", &[])));
        assert!(!server("192.168.1.10", "p", &[]).same_share(&server("192.168.1.11", "p", &[])));
    }

    #[test]
    fn mounted_shares_are_found_on_each_system() {
        let mac = "/dev/disk3s1 on / (apfs, local, journaled)\n\
                   //moi@nas._smb._tcp.local/Mes%20photos on /Volumes/Mes photos (smbfs, nodev, nosuid, mounted by moi)\n";
        let wanted = server("nas", "mes photos", &["2026"]);
        assert_eq!(
            wanted.mounted_in(&Mounts::from_mount_output(mac)),
            Some(PathBuf::from("/Volumes/Mes photos/2026"))
        );
        let proc = "/dev/sda1 / ext4 rw 0 0\n\
                    //nas/photo /mnt/nas\\040photo cifs rw,relatime 0 0\n\
                    nas:/export /mnt/nfs nfs4 rw 0 0\n";
        assert_eq!(
            server("nas", "photo", &[]).mounted_in(&Mounts::from_proc_mounts(proc)),
            Some(PathBuf::from("/mnt/nas photo"))
        );
        // GVFS : Linux seulement (« : » interdit dans un nom de fichier sous Windows).
        #[cfg(unix)]
        {
            let gvfs = tempfile::tempdir().unwrap();
            for d in [
                "smb-share:server=nas.local,share=photo,user=moi",
                "mtp:host=telephone",
            ] {
                fs::create_dir(gvfs.path().join(d)).unwrap();
            }
            assert_eq!(
                server("NAS", "Photo", &[]).mounted_in(&Mounts::from_gvfs(gvfs.path())),
                Some(
                    gvfs.path()
                        .join("smb-share:server=nas.local,share=photo,user=moi")
                )
            );
            assert_eq!(
                server("nas", "autre", &[]).mounted_in(&Mounts::from_gvfs(gvfs.path())),
                None
            );
        }
    }

    #[test]
    fn shares_are_remembered_once_and_forgotten() {
        let data = tempfile::tempdir().unwrap();
        assert!(remembered(data.path()).is_empty());
        remember(data.path(), &server("nas", "photo", &["2026"])).unwrap();
        let list = remember(data.path(), &server("NAS", "Photo", &[])).unwrap();
        assert_eq!(
            list,
            vec![server("nas", "photo", &[])],
            "une fois, sans ses dossiers"
        );
        remember(data.path(), &server("nas", "musique", &[])).unwrap();
        let list = forget(data.path(), &server("nas", "photo", &[])).unwrap();
        assert_eq!(list, vec![server("nas", "musique", &[])]);
        assert_eq!(remembered(data.path()), list);
    }

    #[test]
    fn a_remembered_share_is_a_folder_on_windows_and_a_share_to_join_elsewhere() {
        let s = server("nas", "photo", &["2026"]);
        let windows = s.place("windows");
        assert_eq!(windows.path, PathBuf::from(r"\\nas\photo"));
        assert_eq!(windows.kind, PlaceKind::Network);
        let mac = s.place("macos");
        assert_eq!(mac.path, PathBuf::from("smb://nas/photo"));
        assert_eq!(mac.kind, PlaceKind::Server);
        assert_eq!(mac.name, "photo (nas)");
        let list = vec![s.root()];
        assert_eq!(of_place(&list, &mac.path, "macos"), Some(&list[0]));
        assert_eq!(of_place(&list, &windows.path, "windows"), Some(&list[0]));
        assert_eq!(of_place(&list, Path::new("/ailleurs"), "macos"), None);
    }
}
