//! Serveurs réseau qu'on rejoint par leur adresse, comme « Se connecter au serveur » du
//! Finder : partages SMB (`\\nas\photo`, `smb://nas/photo`, demandé le 28/10/2026 : sous
//! Windows, rien ne permettait d'atteindre un partage pas attribué à une lettre de
//! lecteur), et les autres protocoles courants (29/10/2026) : AFP, FTP, SFTP, WebDAV, NFS.
//! Le montage est laissé au système, qui demande lui-même le mot de passe : Windows ouvre
//! le chemin UNC directement (SMB, WebDAV), macOS monte par `mount volume` (trousseau,
//! boîte du système), Linux par `gio mount` (GVFS, comme Nemo ou Nautilus, tous les
//! protocoles). Les serveurs rejoints sont mémorisés (fichier `servers` du dossier de
//! données) et proposés dans la section « Réseau » du panneau des dossiers. Voir
//! `doc/en/servers.md`.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::places::{Place, PlaceKind};

/// Protocole d'un serveur réseau.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Protocol {
    /// Partages Windows, Samba (`smb://`, `\\serveur\partage`).
    Smb,
    /// Apple Filing Protocol (`afp://`), encore annoncé par bien des NAS.
    Afp,
    /// `ftp://`.
    Ftp,
    /// Fichiers par SSH (`sftp://`).
    Sftp,
    /// WebDAV (`dav://`, ou `webdav://`).
    Dav,
    /// WebDAV chiffré (`davs://`, `webdavs://`).
    Davs,
    /// `nfs://serveur/export`.
    Nfs,
}

impl Protocol {
    /// Tous les protocoles, dans l'ordre où le format attendu les cite.
    pub const ALL: [Protocol; 7] = [
        Protocol::Smb,
        Protocol::Afp,
        Protocol::Ftp,
        Protocol::Sftp,
        Protocol::Dav,
        Protocol::Davs,
        Protocol::Nfs,
    ];

    /// Préfixe des adresses (`smb`, sans `://`).
    pub fn scheme(self) -> &'static str {
        match self {
            Protocol::Smb => "smb",
            Protocol::Afp => "afp",
            Protocol::Ftp => "ftp",
            Protocol::Sftp => "sftp",
            Protocol::Dav => "dav",
            Protocol::Davs => "davs",
            Protocol::Nfs => "nfs",
        }
    }

    /// Nom affiché (« SMB », « WebDAV »).
    pub fn label(self) -> &'static str {
        match self {
            Protocol::Smb => "SMB",
            Protocol::Afp => "AFP",
            Protocol::Ftp => "FTP",
            Protocol::Sftp => "SFTP",
            Protocol::Dav | Protocol::Davs => "WebDAV",
            Protocol::Nfs => "NFS",
        }
    }

    fn from_scheme(scheme: &str) -> Option<Protocol> {
        let scheme = scheme.to_ascii_lowercase();
        match scheme.as_str() {
            "webdav" => Some(Protocol::Dav),
            "webdavs" => Some(Protocol::Davs),
            _ => Protocol::ALL.into_iter().find(|p| p.scheme() == scheme),
        }
    }

    /// Le premier dossier de l'adresse est un partage, ce qu'on monte (SMB, AFP), dont le
    /// serveur sait donner la liste.
    pub fn has_shares(self) -> bool {
        matches!(self, Protocol::Smb | Protocol::Afp)
    }

    /// Ce qu'on monte, après le serveur : le partage (SMB, AFP), rien (FTP, SFTP : tout le
    /// serveur), ou tout le chemin tapé (WebDAV, NFS : un export, un dossier du serveur).
    fn root_len(self, parts: usize) -> usize {
        match self {
            Protocol::Smb | Protocol::Afp => 1.min(parts),
            Protocol::Ftp | Protocol::Sftp => 0,
            Protocol::Dav | Protocol::Davs | Protocol::Nfs => parts,
        }
    }

    /// Même famille : un partage SMB et le même monté en AFP (Nemo choisit AFP quand le
    /// NAS annonce les deux, retour du 29/10/2026) ; WebDAV chiffré ou pas.
    fn family(self) -> Protocol {
        match self {
            Protocol::Afp => Protocol::Smb,
            Protocol::Davs => Protocol::Dav,
            p => p,
        }
    }

    /// Le système `os` sait le monter lui-même : tout sous Linux (GVFS) ; tout sauf SFTP
    /// sous macOS ; SMB et WebDAV (chemins UNC) sous Windows.
    pub fn mountable(self, os: &str) -> bool {
        match os {
            "windows" => matches!(self, Protocol::Smb | Protocol::Dav | Protocol::Davs),
            "macos" => self != Protocol::Sftp,
            _ => true,
        }
    }
}

/// Un dossier d'un serveur réseau : protocole, serveur, ce qu'on monte (`share`) et,
/// éventuellement, des dossiers dedans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Server {
    /// Protocole.
    pub protocol: Protocol,
    /// Utilisateur donné dans l'adresse (`smb://moi@nas/photo`), proposé par le système.
    pub user: Option<String>,
    /// Nom ou adresse du serveur (`nas`, `nas.local`, `192.168.1.10`).
    pub host: String,
    /// Port donné dans l'adresse (`ftp://nas:2121`).
    pub port: Option<u16>,
    /// Ce qu'on monte, après le serveur ([`Protocol::root_len`]) : le partage (`photo`),
    /// rien (FTP, SFTP) ou un chemin (`volume1/photo`, WebDAV, NFS).
    pub share: String,
    /// Dossiers dedans (`2026/séance`), à ouvrir une fois connecté.
    pub folders: Vec<String>,
}

impl Server {
    /// Lit une adresse tapée : `\\serveur\partage[\dossiers]` (SMB ; barres obliques
    /// acceptées aussi : `\\nas/photo`) ou `protocole://[utilisateur@]serveur[:port]/…`
    /// (`smb`, `afp`, `ftp`, `sftp`, `dav`, `davs`, `nfs` ; caractères encodés `%20`
    /// décodés). `None` si ce n'est pas une adresse (il faut au moins un serveur, et un
    /// partage pour SMB et AFP).
    pub fn parse(text: &str) -> Option<Server> {
        let address = split_address(text)?;
        let root = address.protocol.root_len(address.parts.len());
        if address.protocol.has_shares() && root == 0 {
            return None;
        }
        let mut parts = address.parts;
        let folders = parts.split_off(root);
        Some(Server {
            protocol: address.protocol,
            user: address.user,
            host: address.host,
            port: address.port,
            share: parts.join("/"),
            folders,
        })
    }

    /// Le serveur de ce partage.
    pub fn server(&self) -> Host {
        Host {
            protocol: self.protocol,
            user: self.user.clone(),
            host: self.host.clone(),
            port: self.port,
        }
    }

    /// Ce qu'on monte, sans les dossiers dedans (ce qu'on mémorise).
    pub fn root(&self) -> Server {
        Server {
            folders: Vec::new(),
            ..self.clone()
        }
    }

    /// Le chemin sous le serveur, élément par élément : ce qu'on monte, puis les dossiers.
    fn segments(&self) -> Vec<String> {
        self.share
            .split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .chain(self.folders.iter().cloned())
            .collect()
    }

    /// Adresse `protocole://[utilisateur@]serveur[:port]/…`, caractères spéciaux encodés.
    pub fn address(&self) -> String {
        let mut address = self.server().address();
        address.push_str(
            &self
                .segments()
                .iter()
                .map(|s| percent_encode(s))
                .collect::<Vec<_>>()
                .join("/"),
        );
        address
    }

    /// Chemin UNC de ce qu'on monte, sans les dossiers dedans : `\\nas\photo` (SMB),
    /// `\\nas@SSL@8443\DavWWWRoot\dossier` (WebDAV, service WebClient de Windows).
    pub fn unc(&self) -> PathBuf {
        let path = self.share.replace('/', r"\");
        match self.protocol {
            Protocol::Dav | Protocol::Davs => {
                let ssl = if self.protocol == Protocol::Davs {
                    "@SSL"
                } else {
                    ""
                };
                let port = self.port.map(|p| format!("@{p}")).unwrap_or_default();
                let path = if path.is_empty() {
                    "DavWWWRoot".to_string()
                } else {
                    path
                };
                PathBuf::from(format!(r"\\{}{ssl}{port}\{path}", self.host))
            }
            _ => PathBuf::from(format!(r"\\{}\{path}", self.host)),
        }
    }

    /// Nom affiché : « partage (serveur) », comme les partages montés par GVFS ;
    /// « serveur (FTP) » pour un serveur entier.
    pub fn name(&self) -> String {
        match self.share.rsplit('/').find(|s| !s.is_empty()) {
            Some(last) => format!("{last} ({})", self.host),
            None => format!("{} ({})", self.host, self.protocol.label()),
        }
    }

    /// Vrai si `other` désigne le même endroit monté (même famille de protocole, même
    /// serveur comparé à son premier nom : `nas` = `nas.local` = `nas._smb._tcp.local`,
    /// même `share`, casse ignorée pour SMB et AFP).
    pub fn same_share(&self, other: &Server) -> bool {
        self.protocol.family() == other.protocol.family()
            && same_host(&self.host, &other.host)
            && (self.port.is_none() || other.port.is_none() || self.port == other.port)
            && same_path(self.protocol, &self.share, &other.share)
    }

    /// L'emplacement du panneau des dossiers pour ce serveur mémorisé, sous le système
    /// `os` : sous Windows, son chemin UNC (SMB, WebDAV), un dossier comme un autre ;
    /// ailleurs, un serveur à rejoindre ([`PlaceKind::Server`], chemin = son adresse), tant
    /// qu'il n'est pas monté.
    pub fn place(&self, os: &str) -> Place {
        let root = self.root();
        if os == "windows" && self.protocol.mountable(os) {
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

    /// Le dossier de ce serveur d'après les montages `mounts` (`/Volumes/photo`, GVFS,
    /// CIFS…) : un montage du même serveur dont le chemin est le début du nôtre, le reste
    /// ajouté (`sftp://nas/home/moi/photo` dans un montage de `sftp://nas/`).
    pub fn mounted_in(&self, mounts: &Mounts) -> Option<PathBuf> {
        let wanted = self.segments();
        mounts.shares.iter().find_map(|(mount, point)| {
            let host = Server {
                share: String::new(),
                ..mount.clone()
            };
            let probe = Server {
                share: String::new(),
                folders: Vec::new(),
                ..self.clone()
            };
            if !probe.same_share(&host) {
                return None;
            }
            let root = mount.segments();
            if root.len() > wanted.len()
                || !root
                    .iter()
                    .zip(&wanted)
                    .all(|(a, b)| same_path(self.protocol, a, b))
            {
                return None;
            }
            Some(
                wanted[root.len()..]
                    .iter()
                    .fold(point.clone(), |p, f| p.join(f)),
            )
        })
    }

    /// Rejoint le serveur sous le système `os` et renvoie son dossier (dossiers compris).
    /// Sans `login`, un serveur qui demande un compte donne
    /// [`ConnectError::NeedsLogin`] : l'interface ouvre sa boîte de connexion, puis
    /// rappelle avec le compte saisi. macOS demande lui-même (boîte du système,
    /// trousseau). Peut prendre plusieurs secondes (serveur lent) : à appeler dans un fil
    /// à part.
    pub fn connect(&self, os: &str, login: Option<&Login>) -> Result<PathBuf, ConnectError> {
        if !self.protocol.mountable(os) {
            return Err(ConnectError::Failed(crate::t!(
                "servers-unsupported",
                protocol = self.protocol.label(),
                system = os_name(os)
            )));
        }
        if os == "windows" {
            let root = self.unc();
            if let Some(login) = login {
                windows_connect(&root, login).map_err(|code| {
                    windows_error(Some(code), String::new(), self.user.clone(), Some(login))
                })?;
            }
            return match fs::read_dir(&root) {
                Ok(_) => Ok(self.folders.iter().fold(root, |p, f| p.join(f))),
                Err(e) => Err(windows_error(
                    e.raw_os_error(),
                    e.to_string(),
                    self.user.clone(),
                    login,
                )),
            };
        }
        if let Some(dir) = self.mounted_in(&Mounts::read(os)) {
            return Ok(dir);
        }
        let address = self.root().address();
        if os == "macos" {
            // Boîte du système (utilisateur, mot de passe, trousseau) ; l'adresse est
            // encodée, sans guillemet qui couperait la chaîne AppleScript. WebDAV : le
            // Finder attend `http(s)://`.
            let address = match self.protocol {
                Protocol::Dav => address.replacen("dav://", "http://", 1),
                Protocol::Davs => address.replacen("davs://", "https://", 1),
                _ => address,
            };
            let output = crate::process::command("osascript")
                .args(["-e", &format!("mount volume \"{address}\"")])
                .stdin(std::process::Stdio::null())
                .output()
                .map_err(|e| {
                    ConnectError::Failed(crate::t!("servers-tool-missing", error = e.to_string()))
                })?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(ConnectError::Failed(crate::t!(
                    "servers-refused",
                    error = stderr.trim().to_string()
                )));
            }
        } else {
            gio_mount(&address, self.user.as_deref(), login)?;
        }
        self.mounted_in(&Mounts::read(os))
            .ok_or_else(|| ConnectError::Failed(crate::t!("servers-not-found", name = self.name())))
    }

    /// Vrai si la boîte de connexion peut proposer de retenir le mot de passe sous `os` :
    /// gestionnaire d'identifiants de Windows, trousseau du bureau sous Linux
    /// (`secret-tool`, lu par GVFS).
    pub fn can_save_login(&self, os: &str) -> bool {
        self.protocol.mountable(os) && matches!(os, "windows" | "linux")
    }

    /// Retient le compte `login` pour ce serveur, une fois la connexion réussie : la
    /// prochaine fois, le système ne demande plus rien. Windows : gestionnaire
    /// d'identifiants (`CredWrite`, comme `cmdkey /add`). Linux : trousseau du bureau
    /// par `secret-tool`, sous les attributs que GVFS cherche (sans garantie : à vérifier
    /// selon le bureau).
    pub fn save_login(&self, os: &str, login: &Login) -> Result<(), String> {
        match os {
            "windows" => windows_save_login(&self.host, login),
            "linux" => {
                let mut args = vec![
                    "store".to_string(),
                    "--label".to_string(),
                    format!("tri: {}", self.root().address()),
                    "xdg:schema".to_string(),
                    "org.gnome.keyring.NetworkPassword".to_string(),
                    "user".to_string(),
                    login.user.clone(),
                    "server".to_string(),
                    self.host.clone(),
                    "protocol".to_string(),
                    self.protocol.scheme().to_string(),
                ];
                if !login.domain.is_empty() {
                    args.extend(["domain".to_string(), login.domain.clone()]);
                }
                if let Some(port) = self.port {
                    args.extend(["port".to_string(), port.to_string()]);
                }
                let mut child = crate::process::command("secret-tool")
                    .args(&args)
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| crate::t!("servers-tool-missing", error = e.to_string()))?;
                if let Some(mut stdin) = child.stdin.take() {
                    stdin
                        .write_all(login.password.as_bytes())
                        .map_err(|e| e.to_string())?;
                }
                let output = child.wait_with_output().map_err(|e| e.to_string())?;
                if output.status.success() {
                    Ok(())
                } else {
                    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
                }
            }
            _ => Ok(()),
        }
    }
}

/// Compte saisi dans la boîte de connexion.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Login {
    /// Utilisateur (vide : celui que le système propose).
    pub user: String,
    /// Domaine ou groupe de travail (SMB ; vide : celui que le système propose).
    pub domain: String,
    /// Mot de passe.
    pub password: String,
    /// Retenir le mot de passe une fois connecté ([`Server::save_login`]).
    pub remember: bool,
}

/// Jamais le mot de passe dans une trace.
impl std::fmt::Debug for Login {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Login")
            .field("user", &self.user)
            .field("domain", &self.domain)
            .field("remember", &self.remember)
            .finish_non_exhaustive()
    }
}

/// Échec d'une connexion, par cause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectError {
    /// Le serveur demande un compte : ouvrir la boîte de connexion. `user` et `domain` :
    /// ce que proposer ; `refused` : un compte vient d'être refusé.
    NeedsLogin {
        /// Utilisateur à proposer.
        user: Option<String>,
        /// Domaine à proposer.
        domain: Option<String>,
        /// Un compte vient d'être refusé.
        refused: bool,
    },
    /// Autre échec, message expliqué (serveur injoignable, partage inconnu…).
    Failed(String),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::NeedsLogin { refused: true, .. } => {
                f.write_str(&crate::t!("servers-login-refused"))
            }
            ConnectError::NeedsLogin { .. } => f.write_str(&crate::t!("servers-login-needed")),
            ConnectError::Failed(message) => f.write_str(message),
        }
    }
}

/// Une erreur Windows (`code`, texte `text`) en cause expliquée. `user` : utilisateur de
/// l'adresse ; `login` : compte essayé (refusé si l'accès l'est encore).
fn windows_error(
    code: Option<i32>,
    text: String,
    user: Option<String>,
    login: Option<&Login>,
) -> ConnectError {
    match code {
        // Accès refusé, mot de passe faux, échec d'ouverture de session, non authentifié.
        Some(5 | 86 | 1326 | 1244) => ConnectError::NeedsLogin {
            user: login.map(|l| l.user.clone()).or(user),
            domain: login.map(|l| l.domain.clone()),
            refused: login.is_some(),
        },
        // Chemin réseau introuvable, emplacement injoignable.
        Some(53 | 1231 | 1232) => ConnectError::Failed(crate::t!("servers-host-unreachable")),
        // Nom de partage inconnu.
        Some(67) => ConnectError::Failed(crate::t!("servers-share-missing")),
        // Déjà connecté à ce serveur sous un autre compte.
        Some(1219) => ConnectError::Failed(crate::t!("servers-other-account")),
        _ => {
            let text = if text.is_empty() {
                code.map(|c| io::Error::from_raw_os_error(c).to_string())
                    .unwrap_or_default()
            } else {
                text
            };
            ConnectError::Failed(crate::t!("servers-unreachable", error = text))
        }
    }
}

/// Connecte le partage `unc` avec le compte `login` (`WNetAddConnection2`, sans lettre de
/// lecteur) ; le code d'erreur de Windows sinon.
#[cfg(windows)]
fn windows_connect(unc: &Path, login: &Login) -> Result<(), i32> {
    use windows_sys::Win32::NetworkManagement::WNet::{
        NETRESOURCEW, RESOURCETYPE_DISK, WNetAddConnection2W,
    };
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let mut remote = wide(&unc.to_string_lossy());
    let user = wide(&windows_user(login));
    let password = wide(&login.password);
    let resource = NETRESOURCEW {
        dwType: RESOURCETYPE_DISK,
        lpRemoteName: remote.as_mut_ptr(),
        ..Default::default()
    };
    // SAFETY : chaînes terminées par un zéro, vivantes pendant l'appel.
    let status = unsafe { WNetAddConnection2W(&resource, password.as_ptr(), user.as_ptr(), 0) };
    if status == 0 {
        Ok(())
    } else {
        Err(status as i32)
    }
}

#[cfg(not(windows))]
fn windows_connect(_unc: &Path, _login: &Login) -> Result<(), i32> {
    Ok(())
}

/// Utilisateur pour Windows : `domaine\utilisateur` si un domaine est donné.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_user(login: &Login) -> String {
    if login.domain.is_empty() {
        login.user.clone()
    } else {
        format!(r"{}\{}", login.domain, login.user)
    }
}

/// Retient `login` pour `host` dans le gestionnaire d'identifiants (`CredWrite`).
#[cfg(windows)]
fn windows_save_login(host: &str, login: &Login) -> Result<(), String> {
    use windows_sys::Win32::Security::Credentials::{
        CRED_PERSIST_ENTERPRISE, CRED_TYPE_DOMAIN_PASSWORD, CREDENTIALW, CredWriteW,
    };
    let wide = |s: &str| -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() };
    let mut target = wide(host);
    let mut user = wide(&windows_user(login));
    let mut blob: Vec<u8> = login
        .password
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_DOMAIN_PASSWORD,
        TargetName: target.as_mut_ptr(),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_ENTERPRISE,
        UserName: user.as_mut_ptr(),
        ..Default::default()
    };
    // SAFETY : la structure et ses chaînes vivent pendant l'appel.
    if unsafe { CredWriteW(&credential, 0) } != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error().to_string())
    }
}

#[cfg(not(windows))]
fn windows_save_login(_host: &str, _login: &Login) -> Result<(), String> {
    Ok(())
}

/// Une question de `gio mount` (fin de sa sortie, sans retour à la ligne).
#[derive(Debug, PartialEq, Eq)]
enum GioPrompt {
    /// `User [proposé]: `
    User(Option<String>),
    /// `Domain [proposé]: `
    Domain(Option<String>),
    /// `Password: `
    Password,
    /// Question à choix (`[0] Log In Anyway`, `Choice: `), avec son texte.
    Choice(String),
}

impl GioPrompt {
    /// La question qui termine `text` (sortie de `gio mount` depuis la dernière réponse),
    /// s'il y en a une.
    fn read(text: &str) -> Option<GioPrompt> {
        let line = text.rsplit('\n').next()?;
        let question = line.strip_suffix(": ")?;
        let (name, default) = match question.split_once(" [") {
            Some((name, rest)) => (name, rest.strip_suffix(']').map(str::to_string)),
            None => (question, None),
        };
        match name {
            "User" => Some(GioPrompt::User(default)),
            "Domain" => Some(GioPrompt::Domain(default)),
            "Password" => Some(GioPrompt::Password),
            "Choice" => {
                let message = text[..text.len() - line.len()].trim();
                Some(GioPrompt::Choice(message.to_string()))
            }
            _ => None,
        }
    }
}

/// Réponses aux questions de `gio mount`, dans l'ordre où elles viennent.
struct GioAnswers<'a> {
    login: Option<&'a Login>,
    /// Utilisateur de l'adresse, proposé dans la boîte.
    user: Option<String>,
    /// Mot de passe déjà donné : toute nouvelle question est un refus.
    answered: bool,
}

impl GioAnswers<'_> {
    /// Ligne à envoyer, ou la fin de la connexion : sans compte (ou compte refusé), la
    /// boîte de connexion.
    fn answer(&mut self, prompt: GioPrompt) -> Result<String, ConnectError> {
        if let GioPrompt::Choice(message) = prompt {
            return Err(ConnectError::Failed(crate::t!(
                "servers-question",
                question = message
            )));
        }
        match self.login {
            Some(login) if !self.answered => Ok(match prompt {
                GioPrompt::User(_) => login.user.clone(),
                GioPrompt::Domain(_) => login.domain.clone(),
                _ => {
                    self.answered = true;
                    login.password.clone()
                }
            }),
            login => {
                let offer = match prompt {
                    GioPrompt::User(offer) => offer,
                    _ => None,
                };
                let given = |f: fn(&Login) -> &String| {
                    login.map(|l| f(l).clone()).filter(|s| !s.is_empty())
                };
                Err(ConnectError::NeedsLogin {
                    user: given(|l| &l.user).or(self.user.clone()).or(offer),
                    domain: given(|l| &l.domain),
                    refused: login.is_some(),
                })
            }
        }
    }
}

/// Monte `address` par `gio mount`, en répondant à ses questions (utilisateur, domaine,
/// mot de passe) avec `login` ; `user` : utilisateur de l'adresse. Sortie lue au fil de
/// l'eau : les questions ne finissent pas par un retour à la ligne.
fn gio_mount(address: &str, user: Option<&str>, login: Option<&Login>) -> Result<(), ConnectError> {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = crate::process::command("gio")
        .args(["mount", address])
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            ConnectError::Failed(crate::t!("servers-tool-missing", error = e.to_string()))
        })?;
    let mut stdin = child.stdin.take();
    let mut stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut stderr) = stderr {
            let _ = stderr.read_to_string(&mut text);
        }
        text
    });
    let mut answers = GioAnswers {
        login,
        user: user.map(str::to_string),
        answered: false,
    };
    let (mut pending, mut buffer, mut stop) = (Vec::new(), [0u8; 4096], None);
    while let Some(out) = stdout.as_mut() {
        let n = match out.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        pending.extend_from_slice(&buffer[..n]);
        let Some(prompt) = GioPrompt::read(&String::from_utf8_lossy(&pending)) else {
            continue;
        };
        pending.clear();
        match answers.answer(prompt) {
            Ok(line) => {
                if let Some(input) = stdin.as_mut() {
                    let _ = input.write_all(format!("{line}\n").as_bytes());
                    let _ = input.flush();
                }
            }
            Err(e) => {
                stop = Some(e);
                let _ = child.kill();
                break;
            }
        }
    }
    drop(stdin);
    drop(stdout);
    let status = child.wait();
    let stderr = errors.join().unwrap_or_default();
    if let Some(e) = stop {
        return Err(e);
    }
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => Err(gio_error(&stderr, user, login)),
    }
}

/// Une erreur de `gio mount` (en anglais : `LC_ALL=C`) en cause expliquée.
fn gio_error(stderr: &str, user: Option<&str>, login: Option<&Login>) -> ConnectError {
    let has = |words: &[&str]| words.iter().any(|w| stderr.contains(w));
    if has(&[
        "Permission denied",
        "Access denied",
        "Login failed",
        "Authentication failed",
    ]) {
        return ConnectError::NeedsLogin {
            user: login.map(|l| l.user.clone()).or(user.map(str::to_string)),
            domain: login.map(|l| l.domain.clone()),
            refused: login.is_some(),
        };
    }
    if has(&[
        "Connection refused",
        "timed out",
        "resolve",
        "Name or service not known",
        "No route to host",
        "Network is unreachable",
        "Host is down",
    ]) {
        return ConnectError::Failed(crate::t!("servers-host-unreachable"));
    }
    if has(&["No such file or directory", "not found", "does not exist"]) {
        return ConnectError::Failed(crate::t!("servers-share-missing"));
    }
    // « gio: smb://nas/photo/: Failed to mount… » : le message seul.
    let message = stderr.trim();
    let message = message
        .strip_prefix("gio: ")
        .and_then(|m| m.split_once(": ").map(|(_, rest)| rest))
        .unwrap_or(message);
    ConnectError::Failed(crate::t!("servers-refused", error = message.to_string()))
}

/// Nom du système `os` pour les messages.
fn os_name(os: &str) -> &str {
    match os {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    }
}

/// Deux chemins égaux, élément par élément ; casse ignorée pour SMB et AFP.
fn same_path(protocol: Protocol, a: &str, b: &str) -> bool {
    let parts = |s: &str| {
        s.split('/')
            .filter(|p| !p.is_empty())
            .map(|p| {
                if protocol.has_shares() {
                    p.to_lowercase()
                } else {
                    p.to_string()
                }
            })
            .collect::<Vec<_>>()
    };
    parts(a) == parts(b)
}

/// Une adresse découpée par [`split_address`].
struct Address {
    protocol: Protocol,
    user: Option<String>,
    host: String,
    port: Option<u16>,
    /// Éléments du chemin, décodés pour une adresse `protocole://`.
    parts: Vec<String>,
    /// Une barre suit le serveur.
    separated: bool,
}

/// Découpe une adresse `\\serveur\partage\…` (SMB) ou
/// `protocole://[utilisateur@]serveur[:port]/…`.
fn split_address(text: &str) -> Option<Address> {
    let text = text.trim();
    let (protocol, rest, url) = if let Some(rest) = text.strip_prefix(r"\\") {
        (Protocol::Smb, rest, false)
    } else {
        let (scheme, rest) = text.split_once("://")?;
        (Protocol::from_scheme(scheme)?, rest, true)
    };
    let separated = rest.contains(['\\', '/']);
    let mut parts = rest.split(['\\', '/']).filter(|p| !p.is_empty());
    let authority = parts.next()?;
    let (user, host) = match authority.rsplit_once('@') {
        Some((user, host)) if url && !user.is_empty() => (Some(percent_decode(user)), host),
        _ => (None, authority),
    };
    // Port : `nas:2121`, `[::1]:21` ; une adresse IPv6 garde ses deux-points.
    let (host, port) = match host.rsplit_once(':') {
        Some((h, p)) if url && !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => {
            (h, Some(p.parse().ok()?))
        }
        _ => (host, None),
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let host = if url {
        percent_decode(host)
    } else {
        host.to_string()
    };
    if host.is_empty() || host.contains('@') {
        return None;
    }
    let parts = parts
        .map(|p| {
            if url {
                percent_decode(p)
            } else {
                p.to_string()
            }
        })
        .collect();
    Some(Address {
        protocol,
        user,
        host,
        port,
        parts,
        separated,
    })
}

/// Un serveur, sans chemin : celui dont on liste les partages (`smb://nas/`, `\\nas\`),
/// comme « Partages Windows sur nas » de Nemo (retour du 29/10/2026 : `smb://nas.local/`
/// suivi d'Entrée ne faisait rien).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Host {
    /// Protocole.
    pub protocol: Protocol,
    /// Utilisateur donné dans l'adresse (`smb://moi@nas/`).
    pub user: Option<String>,
    /// Nom ou adresse du serveur.
    pub host: String,
    /// Port donné dans l'adresse.
    pub port: Option<u16>,
}

impl Host {
    /// Le serveur à partages (SMB, AFP) d'une adresse tapée, dès qu'une barre suit son
    /// nom (`smb://nas/`, `\\nas\`, `smb://nas/pho`) : avant, le nom est sans doute encore
    /// en cours de frappe, inutile d'interroger le réseau.
    pub fn parse(text: &str) -> Option<Host> {
        Host::parse_typed_any(text).filter(|_| split_address(text).is_some_and(|a| a.separated))
    }

    /// Le serveur à partages d'une adresse tapée sans partage, barre ou pas après son nom
    /// (`\\host.lan`) : pour proposer de lister ses partages (retour du 29/10/2026,
    /// WinBoat : `\\host.lan` suivi d'Entrée ne faisait rien).
    pub fn parse_typed(text: &str) -> Option<Host> {
        Host::parse_typed_any(text)
            .filter(|_| split_address(text).is_some_and(|a| a.parts.is_empty()))
    }

    fn parse_typed_any(text: &str) -> Option<Host> {
        let a = split_address(text)?;
        a.protocol.has_shares().then_some(Host {
            protocol: a.protocol,
            user: a.user,
            host: a.host,
            port: a.port,
        })
    }

    /// Adresse `protocole://[utilisateur@]serveur[:port]/`.
    pub fn address(&self) -> String {
        let user = self
            .user
            .as_ref()
            .map(|u| format!("{}@", percent_encode(u)))
            .unwrap_or_default();
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            percent_encode(&self.host)
        };
        let port = self.port.map(|p| format!(":{p}")).unwrap_or_default();
        format!("{}://{user}{host}{port}/", self.protocol.scheme())
    }

    /// Le partage `share` de ce serveur.
    pub fn share(&self, share: &str) -> Server {
        Server {
            protocol: self.protocol,
            user: self.user.clone(),
            host: self.host.clone(),
            port: self.port,
            share: share.to_string(),
            folders: Vec::new(),
        }
    }

    /// Les partages de dossiers de ce serveur, par ordre alphabétique (partages cachés
    /// `…$` et imprimantes écartés), demandés au système `os` : `NetShareEnum` sous
    /// Windows, `smbutil view` sous macOS, `gio list` sous Linux (GVFS, comme Nemo ; AFP
    /// aussi). Peut prendre plusieurs secondes : à appeler dans un fil à part.
    pub fn shares(&self, os: &str) -> Result<Vec<String>, String> {
        let mut shares = match os {
            _ if os != "linux" && os != "macos" && os != "windows" => Vec::new(),
            "linux" => {
                // Le voisinage d'un serveur se monte d'abord (anonyme, sans rien demander)
                // ; déjà monté, l'échec est sans importance.
                let address = self.address();
                let _ = run(&["gio", "mount", &address]);
                shares_from_gio_list(&run(&["gio", "list", &address])?)
            }
            _ if self.protocol != Protocol::Smb => {
                return Err(crate::t!(
                    "servers-no-listing",
                    protocol = self.protocol.label(),
                    system = os_name(os)
                ));
            }
            "windows" => windows_shares(&self.host)?,
            _ => {
                let address = format!("//{}", self.address().trim_start_matches("smb://"));
                let address = address.trim_end_matches('/');
                // Identifiants du trousseau d'abord (`-N` : sans rien demander), puis en
                // invité.
                let text = run(&["/usr/bin/smbutil", "view", "-N", address])
                    .or_else(|_| run(&["/usr/bin/smbutil", "view", "-g", address]))?;
                shares_from_smbutil(&text)
            }
        };
        shares.retain(|s| !s.ends_with('$'));
        shares.sort_by_key(|s| s.to_lowercase());
        shares.dedup();
        Ok(shares)
    }
}

/// Lance `args` sans entrée (aucune question possible) ; sa sortie, ou son erreur.
fn run(args: &[&str]) -> Result<String, String> {
    let output = crate::process::command(args[0])
        .args(&args[1..])
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| crate::t!("servers-tool-missing", error = e.to_string()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.trim().to_string())
    }
}

/// Sortie de `gio list smb://nas/` : un partage par ligne.
fn shares_from_gio_list(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Sortie de `smbutil view` : un tableau « Share / Type / Comments » sous une ligne de
/// tirets, jusqu'à « N shares listed » ; seuls les partages `Disk` comptent.
fn shares_from_smbutil(text: &str) -> Vec<String> {
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let Some(column) = header.find("Type") else {
        return Vec::new();
    };
    lines
        .skip_while(|l| !l.trim_start().starts_with('-'))
        .skip(1)
        .take_while(|l| !l.contains("shares listed"))
        .filter_map(|line| {
            let (name, kind) = line.split_at_checked(column)?;
            (kind.split_whitespace().next() == Some("Disk")).then(|| name.trim().to_string())
        })
        .filter(|n| !n.is_empty())
        .collect()
}

/// Partages de dossiers du serveur `host`, par `NetShareEnum` (niveau 1) : pas de sortie
/// de `net view` à lire, traduite selon la langue de Windows.
#[cfg(windows)]
fn windows_shares(host: &str) -> Result<Vec<String>, String> {
    use windows_sys::Win32::NetworkManagement::NetManagement::{
        MAX_PREFERRED_LENGTH, NetApiBufferFree,
    };
    use windows_sys::Win32::Storage::FileSystem::{NetShareEnum, SHARE_INFO_1, STYPE_DISKTREE};
    let server: Vec<u16> = format!(r"\\{host}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut buffer: *mut u8 = std::ptr::null_mut();
    let (mut read, mut total) = (0u32, 0u32);
    // SAFETY : `server` finit par un zéro ; le tampon rendu est libéré plus bas.
    let status = unsafe {
        NetShareEnum(
            server.as_ptr(),
            1,
            &mut buffer,
            MAX_PREFERRED_LENGTH,
            &mut read,
            &mut total,
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32).to_string());
    }
    let mut shares = Vec::new();
    if !buffer.is_null() {
        // SAFETY : `read` entrées `SHARE_INFO_1` à la suite, noms terminés par un zéro.
        let infos =
            unsafe { std::slice::from_raw_parts(buffer as *const SHARE_INFO_1, read as usize) };
        for info in infos {
            if info.shi1_type & 0xff != STYPE_DISKTREE || info.shi1_netname.is_null() {
                continue;
            }
            let name = unsafe {
                let len = (0..)
                    .take_while(|&i| *info.shi1_netname.add(i) != 0)
                    .count();
                String::from_utf16_lossy(std::slice::from_raw_parts(info.shi1_netname, len))
            };
            shares.push(name);
        }
        unsafe { NetApiBufferFree(buffer as _) };
    }
    Ok(shares)
}

#[cfg(not(windows))]
fn windows_shares(_host: &str) -> Result<Vec<String>, String> {
    Ok(Vec::new())
}

/// Vrai si `text` commence comme une adresse de serveur (`\\…`, `smb://…`, `ftp://…`),
/// complète ou pas encore.
pub fn looks_like_address(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with(r"\\")
        || text
            .split_once("://")
            .is_some_and(|(scheme, _)| Protocol::from_scheme(scheme).is_some())
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
/// Chemin `a/b c` encodé élément par élément (`a/b%20c`), pour une adresse.
pub(crate) fn percent_encode_path(path: &str) -> String {
    path.split('/')
        .map(percent_encode)
        .collect::<Vec<_>>()
        .join("/")
}

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

/// Serveurs réseau montés sur cette machine, avec leur dossier.
#[derive(Clone, Debug, Default)]
pub struct Mounts {
    shares: Vec<(Server, PathBuf)>,
}

/// Un serveur monté, `share` = le chemin monté.
fn mounted(
    protocol: Protocol,
    user: Option<String>,
    host: String,
    port: Option<u16>,
    path: &str,
) -> Server {
    Server {
        protocol,
        user,
        host,
        port,
        share: path.trim_matches('/').to_string(),
        folders: Vec::new(),
    }
}

/// `serveur:/chemin` (NFS, sshfs `utilisateur@serveur:/chemin`).
fn host_colon_path(protocol: Protocol, source: &str) -> Option<Server> {
    let (authority, path) = source.split_once(":/")?;
    let (user, host) = match authority.rsplit_once('@') {
        Some((u, h)) => (Some(u.to_string()), h),
        None => (None, authority),
    };
    Some(mounted(protocol, user, host.to_string(), None, path))
}

/// Une URL montée (`ftp://nas/`, `https://nas/dav`) ; `http(s)` = WebDAV.
fn mounted_url(source: &str) -> Option<Server> {
    let source = if let Some(rest) = source.strip_prefix("https://") {
        format!("davs://{rest}")
    } else if let Some(rest) = source.strip_prefix("http://") {
        format!("dav://{rest}")
    } else {
        source.to_string()
    };
    let a = split_address(&source)?;
    Some(mounted(
        a.protocol,
        a.user,
        a.host,
        a.port,
        &a.parts.join("/"),
    ))
}

impl Mounts {
    /// Lit les montages du système `os` : sortie de `mount` (macOS) ; `/proc/mounts`
    /// (CIFS, NFS, sshfs, davfs) et les montages GVFS (Linux). Rien sous Windows (chemins
    /// UNC).
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
    /// (smbfs, …)` ; de même `afpfs`, `ftp` et `webdav` (une URL), `nfs`
    /// (`nas:/volume1/photo`).
    pub fn from_mount_output(text: &str) -> Mounts {
        let shares = text
            .lines()
            .filter_map(|line| {
                let (source, rest) = line.split_once(" on ")?;
                let (point, options) = rest.rsplit_once(" (")?;
                let kind = options.split([',', ')']).next()?.trim();
                let server = match kind {
                    "smbfs" => mounted_url(&format!("smb:{source}"))?,
                    "afpfs" => mounted_url(&format!("afp:{source}"))?,
                    "ftp" | "webdav" => mounted_url(source)?,
                    "nfs" => host_colon_path(Protocol::Nfs, source)?,
                    _ => return None,
                };
                Some((server, PathBuf::from(point)))
            })
            .collect();
        Mounts { shares }
    }

    /// Contenu de `/proc/mounts` (Linux) : `//nas/photo /mnt/nas\040photo cifs …` ; de même
    /// `nfs` / `nfs4` (`nas:/export`), `fuse.sshfs` (`moi@nas:/home/moi`), `davfs` (une
    /// URL).
    pub fn from_proc_mounts(text: &str) -> Mounts {
        let shares = text
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let source = fields.next()?.replace("\\040", " ");
                let point = fields.next()?.replace("\\040", " ");
                let server = match fields.next()? {
                    "cifs" | "smb3" | "smbfs" => {
                        mounted_url(&format!("smb:{}", source.replace('\\', "/")))?
                    }
                    "nfs" | "nfs4" => host_colon_path(Protocol::Nfs, &source)?,
                    "fuse.sshfs" => host_colon_path(Protocol::Sftp, &source)?,
                    "davfs" | "fuse.davfs2" => mounted_url(&source)?,
                    _ => return None,
                };
                Some((server, PathBuf::from(point)))
            })
            .collect();
        Mounts { shares }
    }

    /// Montages GVFS dans `dir` (`/run/user/<uid>/gvfs`) :
    /// `smb-share:server=nas.local,share=photo[,user=moi]`, le même partage monté en AFP,
    /// `afp-volume:host=Nas.local,user=moi,volume=photo` (retour du 29/10/2026 : Nemo
    /// choisit AFP quand le NAS annonce les deux, et le partage n'était pas reconnu),
    /// `ftp:host=…[,port=…]`, `sftp:host=…[,user=…]`, `dav:host=…,ssl=true,prefix=%2Fdav`,
    /// `nfs:host=…,prefix=%2Fexport`.
    pub fn from_gvfs(dir: &Path) -> Mounts {
        let shares = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let (kind, fields) = name.split_once(':')?;
                let field = |key: &str| {
                    fields
                        .split(',')
                        .find_map(|f| f.strip_prefix(&format!("{key}=")))
                        .map(percent_decode)
                };
                let port = field("port").and_then(|p| p.parse().ok());
                let (protocol, host, path) = match kind {
                    "smb-share" => (Protocol::Smb, field("server")?, field("share")?),
                    "afp-volume" => (Protocol::Afp, field("host")?, field("volume")?),
                    "ftp" => (Protocol::Ftp, field("host")?, String::new()),
                    "sftp" => (Protocol::Sftp, field("host")?, String::new()),
                    "dav" => {
                        let ssl = field("ssl").is_some_and(|s| s == "true");
                        let protocol = if ssl { Protocol::Davs } else { Protocol::Dav };
                        (
                            protocol,
                            field("host")?,
                            field("prefix").unwrap_or_default(),
                        )
                    }
                    "nfs" => (
                        Protocol::Nfs,
                        field("host")?,
                        field("prefix").unwrap_or_default(),
                    ),
                    _ => return None,
                };
                Some((
                    mounted(protocol, field("user"), host, port, &path),
                    entry.path(),
                ))
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
            protocol: Protocol::Smb,
            user: None,
            host: host.into(),
            port: None,
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
                "afp-volume:host=Nas.local,user=Fabrice,volume=kphoto",
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
            // Le même partage monté en AFP par Nemo (29/10/2026).
            assert_eq!(
                server("nas.local", "kphoto", &["2026"])
                    .mounted_in(&Mounts::from_gvfs(gvfs.path())),
                Some(
                    gvfs.path()
                        .join("afp-volume:host=Nas.local,user=Fabrice,volume=kphoto/2026")
                )
            );
            assert_eq!(
                server("nas", "autre", &[]).mounted_in(&Mounts::from_gvfs(gvfs.path())),
                None
            );
        }
        let afp = "//Fabrice@Nas._afpovertcp._tcp.local/kphoto on /Volumes/kphoto (afpfs, nodev, nosuid, mounted by fabrice)\n";
        assert_eq!(
            server("nas.local", "kphoto", &[]).mounted_in(&Mounts::from_mount_output(afp)),
            Some(PathBuf::from("/Volumes/kphoto"))
        );
    }

    #[test]
    fn a_server_is_read_once_a_separator_follows_its_name() {
        let nas = Host {
            protocol: Protocol::Smb,
            user: None,
            host: "nas.local".into(),
            port: None,
        };
        assert_eq!(Host::parse("smb://nas.local/"), Some(nas.clone()));
        assert_eq!(Host::parse(r"\\nas.local\"), Some(nas.clone()));
        assert_eq!(Host::parse("smb://nas.local/kph"), Some(nas.clone()));
        assert_eq!(Host::parse("smb://nas.loc"), None, "nom en cours de frappe");
        assert_eq!(Host::parse(r"\\nas"), None);
        assert_eq!(
            Host::parse_typed(r"\\host.lan").map(|h| h.host),
            Some("host.lan".into())
        );
        assert_eq!(Host::parse_typed(r"\\nas\photo"), None, "partage tapé");
        assert_eq!(Host::parse("smb://"), None);
        assert_eq!(Host::parse("/home/moi"), None);
        let moi = Host::parse("smb://moi@nas/").unwrap();
        assert_eq!(moi.address(), "smb://moi@nas/");
        assert_eq!(moi.share("photo").address(), "smb://moi@nas/photo");
        assert_eq!(
            Server::parse("smb://moi@nas/photo/x").unwrap().server(),
            moi
        );
    }

    #[test]
    fn the_other_protocols_are_read_with_what_they_mount() {
        let ftp = Server::parse("ftp://moi@nas.local:2121/photo/2026").unwrap();
        assert_eq!(ftp.protocol, Protocol::Ftp);
        assert_eq!((ftp.user.as_deref(), ftp.port), (Some("moi"), Some(2121)));
        assert_eq!(ftp.share, "", "FTP : tout le serveur");
        assert_eq!(ftp.folders, ["photo", "2026"]);
        assert_eq!(ftp.address(), "ftp://moi@nas.local:2121/photo/2026");
        assert_eq!(ftp.root().address(), "ftp://moi@nas.local:2121/");
        assert_eq!(ftp.name(), "nas.local (FTP)");
        let sftp = Server::parse("SFTP://nas/home/moi").unwrap();
        assert_eq!((sftp.protocol, sftp.share.as_str()), (Protocol::Sftp, ""));
        let dav = Server::parse("davs://cloud.example.org/remote.php/webdav").unwrap();
        assert_eq!(dav.share, "remote.php/webdav", "WebDAV : tout le chemin");
        assert!(dav.folders.is_empty());
        assert_eq!(dav.name(), "webdav (cloud.example.org)");
        assert_eq!(
            dav.unc(),
            PathBuf::from(r"\\cloud.example.org@SSL\remote.php\webdav")
        );
        assert_eq!(
            Server::parse("webdav://nas:8080/").unwrap().unc(),
            PathBuf::from(r"\\nas@8080\DavWWWRoot")
        );
        let nfs = Server::parse("nfs://nas/volume1/photo").unwrap();
        assert_eq!(
            (nfs.protocol, nfs.share.as_str()),
            (Protocol::Nfs, "volume1/photo")
        );
        assert_eq!(
            Server::parse("afp://nas/").map(|s| s.share),
            None,
            "AFP : un partage"
        );
        assert_eq!(Server::parse("gopher://nas/x"), None);
        assert!(looks_like_address("sftp://"));
        assert!(!looks_like_address("https://example.org"));
        // Ce que chaque système sait monter lui-même.
        assert!(Protocol::ALL.iter().all(|p| p.mountable("linux")));
        assert!(!Protocol::Sftp.mountable("macos") && Protocol::Ftp.mountable("macos"));
        assert!(!Protocol::Ftp.mountable("windows") && Protocol::Davs.mountable("windows"));
        assert!(
            Server::parse("ftp://nas/")
                .unwrap()
                .connect("windows", None)
                .is_err()
        );
        assert_eq!(
            Server::parse("ftp://nas/").unwrap().place("windows").kind,
            PlaceKind::Server
        );
    }

    #[test]
    fn mounts_of_the_other_protocols_are_found() {
        let mac = "ftp://nas.local/ on /Volumes/nas.local (ftp, nodev, nosuid, read-only)\n\
                   https://cloud.example.org/remote.php/webdav on /Volumes/webdav (webdav, nodev)\n\
                   nas:/volume1/photo on /Volumes/photo (nfs, nodev, nosuid)\n";
        let mounts = Mounts::from_mount_output(mac);
        let found = |address: &str| Server::parse(address).unwrap().mounted_in(&mounts);
        assert_eq!(
            found("ftp://nas/photo/2026"),
            Some(PathBuf::from("/Volumes/nas.local/photo/2026"))
        );
        assert_eq!(
            found("davs://cloud.example.org/remote.php/webdav/Photos"),
            Some(PathBuf::from("/Volumes/webdav/Photos")),
            "un dossier sous le chemin monté"
        );
        assert_eq!(found("dav://cloud.example.org/autre"), None);
        assert_eq!(
            found("nfs://nas/volume1/photo"),
            Some(PathBuf::from("/Volumes/photo"))
        );
        assert_eq!(found("smb://nas/photo"), None, "un autre protocole");
        let proc = "nas:/volume1/photo /mnt/photo nfs4 rw 0 0\n\
                    moi@nas:/home/moi /mnt/ssh fuse.sshfs rw 0 0\n";
        let mounts = Mounts::from_proc_mounts(proc);
        assert_eq!(
            Server::parse("sftp://nas/home/moi/photo")
                .unwrap()
                .mounted_in(&mounts),
            Some(PathBuf::from("/mnt/ssh/photo"))
        );
        assert_eq!(
            Server::parse("nfs://nas/volume1/photo/2026")
                .unwrap()
                .mounted_in(&mounts),
            Some(PathBuf::from("/mnt/photo/2026"))
        );
        #[cfg(unix)]
        {
            let gvfs = tempfile::tempdir().unwrap();
            for d in [
                "ftp:host=nas.local",
                "sftp:host=nas,user=moi",
                "dav:host=cloud.example.org,ssl=true,prefix=%2Fremote.php%2Fwebdav",
            ] {
                fs::create_dir(gvfs.path().join(d)).unwrap();
            }
            let mounts = Mounts::from_gvfs(gvfs.path());
            let found = |address: &str| Server::parse(address).unwrap().mounted_in(&mounts);
            assert_eq!(
                found("ftp://nas/photo"),
                Some(gvfs.path().join("ftp:host=nas.local/photo"))
            );
            assert_eq!(
                found("sftp://nas/home"),
                Some(gvfs.path().join("sftp:host=nas,user=moi/home"))
            );
            assert_eq!(
                found("davs://cloud.example.org/remote.php/webdav/2026"),
                Some(gvfs.path().join(
                    "dav:host=cloud.example.org,ssl=true,prefix=%2Fremote.php%2Fwebdav/2026"
                ))
            );
            assert_eq!(
                found("dav://cloud.example.org/remote.php/webdav"),
                Some(
                    gvfs.path()
                        .join("dav:host=cloud.example.org,ssl=true,prefix=%2Fremote.php%2Fwebdav")
                ),
                "WebDAV chiffré ou pas : le même serveur"
            );
        }
    }

    #[test]
    fn only_servers_with_shares_are_listed() {
        assert_eq!(
            Host::parse("afp://nas/").map(|h| h.protocol),
            Some(Protocol::Afp)
        );
        assert_eq!(
            Host::parse("ftp://nas/"),
            None,
            "FTP : le serveur se rejoint"
        );
        let afp = Host::parse("afp://nas/").unwrap();
        assert!(
            afp.shares("windows").is_err(),
            "pas de liste AFP sous Windows"
        );
        assert_eq!(afp.address(), "afp://nas/");
        assert_eq!(
            Host::parse("smb://[fe80::1]:445/").unwrap().address(),
            "smb://[fe80::1]:445/"
        );
    }

    #[test]
    fn shares_are_read_from_the_system_tools() {
        assert_eq!(
            shares_from_gio_list("3d\nhome\nkphoto\n\nweb_packages\n"),
            ["3d", "home", "kphoto", "web_packages"]
        );
        let smbutil = "Share                                           Type    Comments\n\
                       -------------------------------\n\
                       kphoto                                          Disk    \n\
                       Mes photos                                      Disk    Photos de famille\n\
                       IPC$                                            Pipe    IPC Service\n\
                       imprimante                                      Printer \n\
                       \n\
                       4 shares listed\n";
        assert_eq!(shares_from_smbutil(smbutil), ["kphoto", "Mes photos"]);
        assert!(shares_from_smbutil("").is_empty());
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

    fn login(user: &str, password: &str) -> Login {
        Login {
            user: user.into(),
            password: password.into(),
            ..Login::default()
        }
    }

    #[test]
    fn the_questions_of_gio_mount_are_read_at_the_end_of_its_output() {
        let asked = "Password required for share photo on nas\nUser [fabrice]: ";
        assert_eq!(
            GioPrompt::read(asked),
            Some(GioPrompt::User(Some("fabrice".into())))
        );
        assert_eq!(
            GioPrompt::read("Domain [WORKGROUP]: "),
            Some(GioPrompt::Domain(Some("WORKGROUP".into())))
        );
        assert_eq!(GioPrompt::read("User: "), Some(GioPrompt::User(None)));
        assert_eq!(GioPrompt::read("Password: "), Some(GioPrompt::Password));
        assert_eq!(
            GioPrompt::read(
                "Can't verify the identity of nas.\n[0] Log In Anyway\n[1] Cancel\nChoice: "
            ),
            Some(GioPrompt::Choice(
                "Can't verify the identity of nas.\n[0] Log In Anyway\n[1] Cancel".into()
            ))
        );
        // Question pas encore finie, ou simple message.
        assert_eq!(
            GioPrompt::read("Password required for share photo\nUs"),
            None
        );
        assert_eq!(GioPrompt::read("Mounted smb://nas/photo/\n"), None);
    }

    #[test]
    fn gio_mount_is_answered_with_the_account_then_a_new_question_is_a_refusal() {
        // Sans compte : la boîte, avec l'utilisateur proposé par GVFS.
        let mut none = GioAnswers {
            login: None,
            user: None,
            answered: false,
        };
        assert_eq!(
            none.answer(GioPrompt::User(Some("fabrice".into()))),
            Err(ConnectError::NeedsLogin {
                user: Some("fabrice".into()),
                domain: None,
                refused: false
            })
        );
        // Avec un compte : utilisateur, domaine (vide = celui proposé), mot de passe.
        let account = Login {
            domain: String::new(),
            ..login("moi", "secret")
        };
        let mut answers = GioAnswers {
            login: Some(&account),
            user: None,
            answered: false,
        };
        assert_eq!(answers.answer(GioPrompt::User(None)), Ok("moi".into()));
        assert_eq!(
            answers.answer(GioPrompt::Domain(Some("WORKGROUP".into()))),
            Ok(String::new())
        );
        assert_eq!(answers.answer(GioPrompt::Password), Ok("secret".into()));
        // Redemandé : refusé, la boîte revient avec l'utilisateur tapé.
        assert_eq!(
            answers.answer(GioPrompt::User(None)),
            Err(ConnectError::NeedsLogin {
                user: Some("moi".into()),
                domain: None,
                refused: true
            })
        );
        let mut question = GioAnswers {
            login: Some(&account),
            user: None,
            answered: false,
        };
        assert!(matches!(
            question.answer(GioPrompt::Choice("?".into())),
            Err(ConnectError::Failed(_))
        ));
    }

    #[test]
    fn connection_errors_are_explained_by_cause() {
        let account = login("moi", "faux");
        // Windows : accès refusé sans compte = la boîte ; avec, un refus.
        assert_eq!(
            windows_error(Some(5), String::new(), Some("fab".into()), None),
            ConnectError::NeedsLogin {
                user: Some("fab".into()),
                domain: None,
                refused: false
            }
        );
        assert!(matches!(
            windows_error(Some(1326), String::new(), None, Some(&account)),
            ConnectError::NeedsLogin { refused: true, .. }
        ));
        let failed =
            |e: ConnectError, key: &str| assert_eq!(e, ConnectError::Failed(crate::t!(key)));
        failed(
            windows_error(Some(53), String::new(), None, None),
            "servers-host-unreachable",
        );
        failed(
            windows_error(Some(67), String::new(), None, None),
            "servers-share-missing",
        );
        failed(
            windows_error(Some(1219), String::new(), None, None),
            "servers-other-account",
        );
        // GVFS.
        assert!(matches!(
            gio_error("gio: afp://nas/: Permission denied", None, Some(&account)),
            ConnectError::NeedsLogin { refused: true, .. }
        ));
        failed(
            gio_error(
                "gio: smb://nax/photo/: Failed to mount Windows share: Connection timed out",
                None,
                None,
            ),
            "servers-host-unreachable",
        );
        failed(
            gio_error(
                "gio: smb://nas/fotos/: Failed to mount Windows share: No such file or directory",
                None,
                None,
            ),
            "servers-share-missing",
        );
        assert_eq!(
            gio_error("gio: ftp://nas/: Location is not mountable", None, None),
            ConnectError::Failed(crate::t!(
                "servers-refused",
                error = "Location is not mountable"
            ))
        );
        // Jamais le mot de passe dans une trace.
        assert!(!format!("{account:?}").contains("faux"));
        assert_eq!(
            windows_user(&Login {
                domain: "MAISON".into(),
                ..account
            }),
            r"MAISON\moi"
        );
    }
}
