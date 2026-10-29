//! Copie en local d'un dossier d'un serveur que le système ne sait pas monter (FTP et
//! SFTP sous Windows, SFTP sous macOS : voir [`crate::servers::Protocol::mountable`]).
//! Choix de l'utilisateur (29/10/2026) : copier en local, mais annoncer d'abord le nombre
//! d'images et la taille, « pour éventuellement changer d'avis avant de télécharger 60 To
//! de données ». Seules les images et leurs sidecars XMP sont copiés ; les notes restent
//! sur la copie locale.
//!
//! Aucune dépendance : les outils livrés avec Windows 10+ et macOS, `curl` pour FTP et le
//! client OpenSSH `sftp` pour SFTP (clés SSH de l'utilisateur, sans rien demander :
//! `BatchMode`). Voir `doc/en/remote.md`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;

use crate::servers::{ConnectError, Login, Protocol, Server};

/// Section `[remote]` : copie en local d'un serveur que le système ne sait pas monter.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RemoteSettings {
    /// Dossier où les copies sont rangées (`~` accepté), sous `serveur/dossier`.
    pub destination: String,
    /// Profondeur des sous-dossiers parcourus sous le dossier demandé.
    pub depth: usize,
    /// Fichiers demandés par lancement de `sftp` (une session SSH chacun) : plus grand,
    /// moins de connexions ; plus petit, une progression plus fine et une annulation plus
    /// rapide.
    pub batch: usize,
}

impl Default for RemoteSettings {
    fn default() -> Self {
        Self {
            destination: "~/Pictures".into(),
            depth: 8,
            batch: 20,
        }
    }
}

/// Vrai si `tri` sait copier en local depuis un serveur de ce protocole.
pub fn supported(protocol: Protocol) -> bool {
    matches!(protocol, Protocol::Ftp | Protocol::Sftp)
}

/// Un fichier à copier : chemin sous le dossier demandé, taille.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Chemin relatif, élément par élément (`["2026", "DSCF0001.RAF"]`).
    pub path: Vec<String>,
    /// Taille annoncée par le serveur (octets).
    pub size: u64,
}

/// Ce que contient le dossier demandé : les fichiers à copier.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    /// Images et sidecars XMP.
    pub files: Vec<Entry>,
}

impl Listing {
    /// Nombre d'images (sidecars non comptés).
    pub fn images(&self) -> usize {
        self.files.iter().filter(|e| !is_sidecar(e.name())).count()
    }

    /// Taille totale (octets).
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|e| e.size).sum()
    }
}

impl Entry {
    fn name(&self) -> &str {
        self.path.last().map_or("", String::as_str)
    }
}

fn is_sidecar(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xmp"))
}

/// Une image ou un sidecar : ce qu'on copie.
fn wanted(name: &str) -> bool {
    is_sidecar(name) || crate::media::kind_of(Path::new(name)).is_some()
}

/// Le dossier local proposé pour la copie de `server` : `destination/serveur/dossier`.
pub fn default_destination(settings: &RemoteSettings, server: &Server) -> PathBuf {
    let root = crate::settings::expand_home(&settings.destination);
    let last = server.folders.last().cloned().unwrap_or_default();
    let host = root.join(&server.host);
    if last.is_empty() {
        host
    } else {
        host.join(last)
    }
}

/// Une ligne de liste de dossier : dossier ou fichier, taille, nom.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Line {
    dir: bool,
    size: u64,
    name: String,
}

/// Une ligne `ls -l` (serveurs Unix, `sftp`) : `drwxr-xr-x 2 moi moi 4096 Oct 29 12:00
/// nom avec espaces` ; ou au format DOS (IIS) : `10-29-26  12:00PM  <DIR>  nom`,
/// `10-29-26  12:00PM  1234 nom`. Liens, `.` et `..` écartés.
fn parse_line(line: &str) -> Option<Line> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let first = fields.first()?;
    let (dir, size, rest) = if first.len() == 10 && first.starts_with(['d', '-']) {
        // ls -l : droits, liens, propriétaire, groupe, taille, mois, jour, heure ou année.
        if fields.len() < 9 {
            return None;
        }
        (first.starts_with('d'), fields[4].parse().ok()?, 8)
    } else if first.contains('-') && fields.len() >= 4 && fields[0].bytes().next()?.is_ascii_digit()
    {
        let dir = fields[2].eq_ignore_ascii_case("<DIR>");
        let size = if dir { 0 } else { fields[2].parse().ok()? };
        (dir, size, 3)
    } else {
        return None;
    };
    let name = after_fields(line, rest)?;
    (name != "." && name != "..").then(|| Line {
        dir,
        size,
        name: name.to_string(),
    })
}

/// Ce qui suit les `n` premiers champs de `line` (le nom, espaces compris).
fn after_fields(line: &str, n: usize) -> Option<&str> {
    let mut rest = line.trim_start();
    for _ in 0..n {
        let end = rest.find(char::is_whitespace)?;
        rest = rest[end..].trim_start();
    }
    (!rest.is_empty()).then_some(rest.trim_end())
}

/// Chemin distant (`/photo/2026`) des éléments `segments`.
fn remote_path(segments: &[String]) -> String {
    format!("/{}", segments.join("/"))
}

/// Lit la liste du dossier `dir` (éléments sous la racine du serveur).
fn list_dir(
    server: &Server,
    login: Option<&Login>,
    dir: &[String],
) -> Result<Vec<Line>, ConnectError> {
    let text = match server.protocol {
        Protocol::Ftp => curl(server, login, &[ftp_url(server, dir, true).into()])?,
        _ => sftp(server, &format!("ls -l \"{}\"\n", quote(&remote_path(dir))))?,
    };
    Ok(text
        .lines()
        .filter(|l| !l.starts_with("sftp>"))
        .filter_map(parse_line)
        .collect())
}

/// Liste, sous-dossiers compris (jusqu'à `depth`), les images et sidecars du dossier de
/// `server`, avec le compte `login` (FTP ; SFTP : clés SSH seulement). Annulable
/// (`cancel`) entre deux dossiers. Un serveur FTP qui demande un compte donne
/// [`ConnectError::NeedsLogin`]. Peut être long sur un gros serveur : à appeler dans un
/// fil à part.
pub fn scan(
    server: &Server,
    login: Option<&Login>,
    depth: usize,
    cancel: &AtomicBool,
) -> Result<Listing, ConnectError> {
    if !supported(server.protocol) {
        return Err(ConnectError::Failed(crate::t!(
            "remote-unsupported",
            protocol = server.protocol.label()
        )));
    }
    let root = server.folders.clone();
    let mut files = Vec::new();
    let mut pending = vec![(Vec::<String>::new(), 0)];
    while let Some((relative, level)) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err(ConnectError::Failed(crate::t!("remote-cancelled")));
        }
        let dir: Vec<String> = root.iter().chain(&relative).cloned().collect();
        for line in list_dir(server, login, &dir)? {
            let mut path = relative.clone();
            path.push(line.name.clone());
            if line.dir {
                if level < depth && !line.name.starts_with('.') {
                    pending.push((path, level + 1));
                }
            } else if wanted(&line.name) {
                files.push(Entry {
                    path,
                    size: line.size,
                });
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Listing { files })
}

/// Copie `listing` (de [`scan`], même compte `login`) dans `destination`, en gardant les
/// sous-dossiers ; un
/// fichier déjà là, à la même taille, est sauté (reprise d'une copie interrompue).
/// `progress(fichiers faits, octets faits)` après chaque fichier ou lot. Annulable
/// (`cancel`) entre deux fichiers ou lots.
pub fn download(
    server: &Server,
    login: Option<&Login>,
    listing: &Listing,
    destination: &Path,
    batch: usize,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, u64),
) -> Result<(), String> {
    let (mut done, mut bytes) = (0, 0);
    let todo: Vec<&Entry> = listing
        .files
        .iter()
        .filter(|e| {
            let present = fs::metadata(local_path(destination, e)).is_ok_and(|m| m.len() == e.size);
            if present {
                done += 1;
                bytes += e.size;
            }
            !present
        })
        .collect();
    progress(done, bytes);
    for chunk in todo.chunks(if server.protocol == Protocol::Sftp {
        batch.max(1)
    } else {
        1
    }) {
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::t!("remote-cancelled"));
        }
        for entry in chunk {
            let local = local_path(destination, entry);
            if let Some(parent) = local.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        match server.protocol {
            Protocol::Ftp => {
                let entry = chunk[0];
                let remote: Vec<String> =
                    server.folders.iter().chain(&entry.path).cloned().collect();
                curl(
                    server,
                    login,
                    &[
                        "-o".into(),
                        local_path(destination, entry).into(),
                        ftp_url(server, &remote, false).into(),
                    ],
                )
                .map_err(|e| e.to_string())?;
            }
            _ => {
                let script: String = chunk
                    .iter()
                    .map(|entry| {
                        let remote: Vec<String> =
                            server.folders.iter().chain(&entry.path).cloned().collect();
                        format!(
                            "get \"{}\" \"{}\"\n",
                            quote(&remote_path(&remote)),
                            quote(&local_path(destination, entry).to_string_lossy())
                        )
                    })
                    .collect();
                sftp(server, &script).map_err(|e| e.to_string())?;
            }
        }
        done += chunk.len();
        bytes += chunk.iter().map(|e| e.size).sum::<u64>();
        progress(done, bytes);
    }
    Ok(())
}

fn local_path(destination: &Path, entry: &Entry) -> PathBuf {
    entry
        .path
        .iter()
        .fold(destination.to_path_buf(), |p, s| p.join(s))
}

/// Guillemets et barres obliques inverses protégés (commandes de `sftp`).
fn quote(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Lance `curl` (silencieux, erreurs affichées, sans rien demander) avec `args` ; sa
/// sortie. Le compte passe par l'entrée (`-K -`), jamais dans la ligne de commande
/// (visible des autres processus) ; sans compte, l'utilisateur de l'adresse, mot de
/// passe vide.
fn curl(
    server: &Server,
    login: Option<&Login>,
    args: &[std::ffi::OsString],
) -> Result<String, ConnectError> {
    let mut c = crate::process::command("curl");
    c.args(["--silent", "--show-error", "--fail", "-K", "-"])
        .args(args);
    let config = curl_config(server, login);
    let mut child = c
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            ConnectError::Failed(crate::t!("servers-tool-missing", error = e.to_string()))
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(config.as_bytes())
            .map_err(|e| ConnectError::Failed(e.to_string()))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| ConnectError::Failed(e.to_string()))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(curl_error(output.status.code(), stderr, server, login))
}

/// Configuration de `curl` lue sur son entrée : le compte, guillemets protégés.
fn curl_config(server: &Server, login: Option<&Login>) -> String {
    let user = match (login, &server.user) {
        (Some(login), _) => format!("{}:{}", login.user, login.password),
        (None, Some(user)) => format!("{user}:"),
        (None, None) => return String::new(),
    };
    format!("user = \"{}\"\n", quote(&user))
}

/// Une erreur de `curl` (code de sortie `code`) en cause expliquée.
fn curl_error(
    code: Option<i32>,
    stderr: String,
    server: &Server,
    login: Option<&Login>,
) -> ConnectError {
    match code {
        // Connexion refusée par le serveur (compte demandé ou refusé).
        Some(67) => ConnectError::NeedsLogin {
            user: login.map(|l| l.user.clone()).or(server.user.clone()),
            domain: None,
            refused: login.is_some(),
        },
        // Nom introuvable, connexion impossible, délai dépassé.
        Some(6 | 7 | 28) => ConnectError::Failed(crate::t!("servers-host-unreachable")),
        // Dossier ou fichier refusé, introuvable.
        Some(9 | 78) => ConnectError::Failed(crate::t!("remote-missing")),
        _ => ConnectError::Failed(crate::t!("remote-failed", error = stderr)),
    }
}

/// URL FTP du dossier ou fichier `segments` (barre finale pour lister un dossier).
fn ftp_url(server: &Server, segments: &[String], dir: bool) -> String {
    let address = Server {
        user: None,
        share: String::new(),
        folders: segments.to_vec(),
        ..server.clone()
    }
    .address();
    if dir && !address.ends_with('/') {
        address + "/"
    } else {
        address
    }
}

/// Lance `sftp` en mode lot sur `server`, commandes `script` sur son entrée ; sa sortie.
fn sftp(server: &Server, script: &str) -> Result<String, ConnectError> {
    let mut c = crate::process::command("sftp");
    c.args(["-q", "-b", "-", "-o", "BatchMode=yes"]);
    if let Some(port) = server.port {
        c.arg("-P").arg(port.to_string());
    }
    let target = match &server.user {
        Some(user) => format!("{user}@{}", server.host),
        None => server.host.clone(),
    };
    c.arg(target);
    let mut child = c
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            ConnectError::Failed(crate::t!("servers-tool-missing", error = e.to_string()))
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(script.as_bytes())
            .map_err(|e| ConnectError::Failed(e.to_string()))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| ConnectError::Failed(e.to_string()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(sftp_error(&String::from_utf8_lossy(&output.stderr)))
    }
}

/// Une erreur de `sftp` en cause expliquée (`BatchMode` : rien ne se demande).
fn sftp_error(stderr: &str) -> ConnectError {
    let has = |words: &[&str]| words.iter().any(|w| stderr.contains(w));
    if has(&["Host key verification failed"]) {
        ConnectError::Failed(crate::t!("remote-sftp-host-key"))
    } else if has(&["Permission denied"]) {
        ConnectError::Failed(crate::t!("remote-sftp-keys"))
    } else if has(&[
        "Could not resolve",
        "Connection refused",
        "timed out",
        "No route to host",
        "Network is unreachable",
    ]) {
        ConnectError::Failed(crate::t!("servers-host-unreachable"))
    } else if has(&["No such file", "not found"]) {
        ConnectError::Failed(crate::t!("remote-missing"))
    } else {
        ConnectError::Failed(crate::t!(
            "remote-failed",
            error = stderr.trim().to_string()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listings_are_read_in_unix_and_dos_formats() {
        assert_eq!(
            parse_line("drwxr-xr-x    2 moi  moi      4096 Oct 29 12:00 Séance du matin"),
            Some(Line {
                dir: true,
                size: 4096,
                name: "Séance du matin".into()
            })
        );
        assert_eq!(
            parse_line("-rw-r--r--    1 moi  moi  26123456 Oct 29  2025 DSCF0001.RAF"),
            Some(Line {
                dir: false,
                size: 26_123_456,
                name: "DSCF0001.RAF".into()
            })
        );
        assert_eq!(
            parse_line("10-29-26  12:00PM       <DIR>          2026"),
            Some(Line {
                dir: true,
                size: 0,
                name: "2026".into()
            })
        );
        assert_eq!(
            parse_line("10-29-26  09:15AM              123456 IMG 0001.JPG"),
            Some(Line {
                dir: false,
                size: 123_456,
                name: "IMG 0001.JPG".into()
            })
        );
        assert_eq!(
            parse_line("drwxr-xr-x 2 moi moi 4096 Oct 29 12:00 .."),
            None
        );
        assert_eq!(
            parse_line("lrwxrwxrwx 1 moi moi 4 Oct 29 12:00 lien -> cible"),
            None,
            "lien"
        );
        assert_eq!(parse_line("total 12"), None);
        assert_eq!(parse_line(""), None);
    }

    #[test]
    fn only_images_and_sidecars_are_copied_and_counted() {
        assert!(wanted("DSCF0001.RAF") && wanted("a.jpg") && wanted("DSCF0001.RAF.xmp"));
        assert!(!wanted("notes.txt") && !wanted("film.mp4"));
        let listing = Listing {
            files: vec![
                Entry {
                    path: vec!["a.RAF".into()],
                    size: 30,
                },
                Entry {
                    path: vec!["a.RAF.xmp".into()],
                    size: 2,
                },
            ],
        };
        assert_eq!((listing.images(), listing.bytes()), (1, 32));
    }

    #[test]
    fn the_destination_and_the_urls_follow_the_server() {
        let server = Server::parse("ftp://moi@nas:2121/photo/Séance 1").unwrap();
        let settings = RemoteSettings {
            destination: "/tmp/copies".into(),
            ..RemoteSettings::default()
        };
        assert_eq!(
            default_destination(&settings, &server),
            PathBuf::from("/tmp/copies/nas/Séance 1")
        );
        assert_eq!(
            ftp_url(&server, &server.folders, true),
            "ftp://nas:2121/photo/S%C3%A9ance%201/"
        );
        assert_eq!(remote_path(&server.folders), "/photo/Séance 1");
        assert_eq!(quote(r#"a "b" \c"#), r#"a \"b\" \\c"#);
        assert!(supported(Protocol::Sftp) && !supported(Protocol::Smb));
    }

    #[test]
    fn the_ftp_account_goes_through_the_input_of_curl_and_errors_are_explained() {
        let server = Server::parse("ftp://fab@nas/photo").unwrap();
        let login = Login {
            user: "moi".into(),
            password: r#"mot "de" \passe"#.into(),
            ..Login::default()
        };
        assert_eq!(
            curl_config(&server, Some(&login)),
            "user = \"moi:mot \\\"de\\\" \\\\passe\"\n"
        );
        assert_eq!(curl_config(&server, None), "user = \"fab:\"\n");
        assert_eq!(
            curl_config(&Server::parse("ftp://nas/x").unwrap(), None),
            ""
        );
        assert_eq!(
            curl_error(Some(67), String::new(), &server, None),
            ConnectError::NeedsLogin {
                user: Some("fab".into()),
                domain: None,
                refused: false
            }
        );
        assert!(matches!(
            curl_error(Some(67), String::new(), &server, Some(&login)),
            ConnectError::NeedsLogin { refused: true, .. }
        ));
        let failed = |key: &str| ConnectError::Failed(crate::t!(key));
        assert_eq!(
            curl_error(Some(6), String::new(), &server, None),
            failed("servers-host-unreachable")
        );
        assert_eq!(
            curl_error(Some(78), String::new(), &server, None),
            failed("remote-missing")
        );
        assert_eq!(
            sftp_error("moi@nas: Permission denied (publickey)."),
            failed("remote-sftp-keys")
        );
        assert_eq!(
            sftp_error("Host key verification failed."),
            failed("remote-sftp-host-key")
        );
        assert_eq!(
            sftp_error("ssh: Could not resolve hostname nax"),
            failed("servers-host-unreachable")
        );
    }

    #[test]
    fn a_file_already_copied_at_the_same_size_is_skipped() {
        // Rien n'est demandé au réseau : tout est déjà là.
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("2026")).unwrap();
        fs::write(dir.path().join("2026").join("a.jpg"), b"12345").unwrap();
        let listing = Listing {
            files: vec![Entry {
                path: vec!["2026".into(), "a.jpg".into()],
                size: 5,
            }],
        };
        let server = Server::parse("sftp://tri-test-host/photo").unwrap();
        let mut seen = Vec::new();
        download(
            &server,
            None,
            &listing,
            dir.path(),
            20,
            &AtomicBool::new(false),
            |f, b| seen.push((f, b)),
        )
        .unwrap();
        assert_eq!(seen, [(1, 5)]);
    }
}
