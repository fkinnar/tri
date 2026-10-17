//! Actions sur les fichiers : renommer, supprimer (corbeille), afficher dans le gestionnaire
//! de fichiers, ouvrir dans un éditeur externe.
//!
//! Une image ne se déplace jamais seule : ses **sidecars XMP** la suivent (sinon ses notes
//! seraient perdues ou attribuées à une autre image), et, par défaut, l'autre image de sa
//! **paire RAW+JPEG** aussi (`[files] pairs`), comme pour les notes. Chaque action se
//! prépare d'abord (`plan_*`) : la liste des fichiers touchés est montrée avant d'agir, et
//! les erreurs prévisibles (nom déjà pris) arrivent avant la première modification. Voir
//! `doc/files.md`.

use std::collections::HashSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::media;
use crate::xmp::SidecarIndex;

/// Section `[files]` du fichier de configuration.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FileSettings {
    /// L'autre image d'une paire RAW+JPEG (même nom, autre extension) suit l'image
    /// renommée ou supprimée.
    pub pairs: bool,
    /// Supprimer = mettre à la corbeille (récupérable) ; `false` : suppression définitive.
    pub trash: bool,
    /// Éditeur externe : programme puis arguments ; `{files}` est remplacé par les
    /// fichiers (ajoutés à la fin sans `{files}`). Vide : application par défaut du
    /// système (`open`, `xdg-open`, `start`). Ex. `["darktable"]`, ou sur macOS
    /// `["open", "-a", "darktable"]`.
    pub editor: Vec<String>,
}

impl Default for FileSettings {
    fn default() -> Self {
        FileSettings {
            pairs: true,
            trash: true,
            editor: Vec::new(),
        }
    }
}

/// Erreurs des actions sur les fichiers.
#[derive(Debug)]
pub enum FileError {
    /// Nouveau nom vide, avec un séparateur de dossier, ou « . » / « .. ».
    InvalidName(String),
    /// Le nouveau nom est déjà pris.
    Exists(PathBuf),
    /// Erreur d'entrée-sortie sur un fichier.
    Io(PathBuf, io::Error),
    /// Mise à la corbeille impossible.
    Trash(String),
    /// Lancement d'un programme impossible (gestionnaire de fichiers, éditeur).
    Launch(String, io::Error),
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(n) => write!(f, "nom invalide : « {n} »"),
            Self::Exists(p) => write!(f, "{} existe déjà", p.display()),
            Self::Io(p, e) => write!(f, "{} : {e}", p.display()),
            Self::Trash(e) => write!(f, "mise à la corbeille impossible : {e}"),
            Self::Launch(p, e) => write!(f, "lancement de « {p} » impossible : {e}"),
        }
    }
}

impl std::error::Error for FileError {}

/// `Result` spécialisé pour le module files.
pub type Result<T> = std::result::Result<T, FileError>;

/// Fichiers touchés par une action sur des images.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    /// Les images, paires comprises (`[files] pairs`), dans l'ordre de la demande.
    pub images: Vec<PathBuf>,
    /// Leurs sidecars XMP qui ne servent qu'à elles.
    pub sidecars: Vec<PathBuf>,
    /// Sidecars partagés avec une image qui reste (`IMAGE.xmp` d'une paire dont une seule
    /// image est touchée) : laissés en place.
    pub shared: Vec<PathBuf>,
}

impl Group {
    /// Tous les fichiers à déplacer ou supprimer : images puis sidecars.
    pub fn files(&self) -> impl Iterator<Item = &PathBuf> {
        self.images.iter().chain(&self.sidecars)
    }
}

/// Nom de fichier sans extension, en minuscules (comparaison des paires).
fn stem_key(path: &Path) -> Option<String> {
    path.file_stem().map(|s| s.to_string_lossy().to_lowercase())
}

/// Images du dossier de `image` de même nom à l'extension près (`image` comprise).
fn same_stem(image: &Path) -> Vec<PathBuf> {
    let dir = image.parent().unwrap_or(Path::new("."));
    let wanted = stem_key(image);
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| media::kind_of(p).is_some() && stem_key(p) == wanted)
        .collect();
    found.sort();
    found
}

/// Fichiers touchés par une action sur `images` : les images, leurs paires si `pairs`, et
/// les sidecars qui ne servent qu'à elles.
pub fn group(images: &[PathBuf], pairs: bool) -> Group {
    let mut group = Group::default();
    let mut seen = HashSet::new();
    for image in images {
        let members = if pairs {
            same_stem(image)
        } else {
            vec![image.clone()]
        };
        // L'image demandée d'abord, puis sa paire.
        for member in std::iter::once(image.clone()).chain(members) {
            if seen.insert(member.clone()) {
                group.images.push(member);
            }
        }
    }
    let chosen: HashSet<&PathBuf> = group.images.iter().collect();
    let mut sidecars_seen = HashSet::new();
    for image in &group.images {
        let index = SidecarIndex::new(image.parent().unwrap_or(Path::new(".")));
        for sidecar in index.sidecars(image) {
            if !sidecars_seen.insert(sidecar.clone()) {
                continue;
            }
            // Propriétaires du sidecar : les images de même nom qui le reconnaissent.
            let owners: Vec<PathBuf> = same_stem(image)
                .into_iter()
                .filter(|other| index.sidecars(other).contains(&sidecar))
                .collect();
            if owners.iter().all(|o| chosen.contains(o)) {
                group.sidecars.push(sidecar);
            } else {
                group.shared.push(sidecar);
            }
        }
    }
    group
}

/// Renommage prévu : chaque fichier et son nouveau chemin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rename {
    /// (ancien, nouveau), images puis sidecars.
    pub moves: Vec<(PathBuf, PathBuf)>,
    /// Sidecars partagés avec une image qui garde son nom : copiés sous le nouveau nom
    /// (les deux images gardent leurs notes).
    pub copies: Vec<(PathBuf, PathBuf)>,
}

/// Prépare le renommage de `image` en `new_stem` (nom sans extension : l'extension est
/// gardée). La paire et les sidecars suivent : `IMG.RAF`, `IMG.JPG`, `IMG.RAF.xmp`,
/// `IMG.xmp` deviennent `NOUVEAU.RAF`, `NOUVEAU.JPG`… Erreur si le nom est invalide ou
/// déjà pris (changer seulement la casse est permis).
pub fn plan_rename(image: &Path, new_stem: &str, settings: &FileSettings) -> Result<Rename> {
    let invalid = new_stem.trim().is_empty()
        || new_stem == "."
        || new_stem == ".."
        || new_stem.contains(['/', '\\'])
        || new_stem.contains(std::path::MAIN_SEPARATOR);
    if invalid {
        return Err(FileError::InvalidName(new_stem.into()));
    }
    let group = group(&[image.to_path_buf()], settings.pairs);
    let old_len = image
        .file_stem()
        .map(|s| s.to_string_lossy().chars().count())
        .unwrap_or(0);
    let renamed = |path: &Path| -> PathBuf {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let rest: String = name.chars().skip(old_len).collect();
        path.with_file_name(format!("{new_stem}{rest}"))
    };
    let mut plan = Rename::default();
    for file in group.files() {
        plan.moves.push((file.clone(), renamed(file)));
    }
    for sidecar in &group.shared {
        plan.copies.push((sidecar.clone(), renamed(sidecar)));
    }
    let sources: HashSet<&PathBuf> = plan.moves.iter().map(|(from, _)| from).collect();
    let mut targets = HashSet::new();
    for (from, to) in plan.moves.iter().chain(&plan.copies) {
        // Même fichier sous une autre casse (macOS, Windows) : pas un conflit.
        let same_file = to.exists() && same(from, to);
        if (!same_file && to.exists() && !sources.contains(to)) || !targets.insert(to.clone()) {
            return Err(FileError::Exists(to.clone()));
        }
    }
    plan.moves.retain(|(from, to)| from != to);
    Ok(plan)
}

/// Vrai si `a` et `b` désignent le même fichier (casse différente sur un système de
/// fichiers insensible à la casse).
fn same(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Exécute un renommage préparé par [`plan_rename`]. Sur une erreur, ce qui a déjà été
/// renommé est remis en place.
pub fn rename(plan: &Rename) -> Result<()> {
    let mut done: Vec<(&PathBuf, &PathBuf)> = Vec::new();
    let mut copied: Vec<&PathBuf> = Vec::new();
    let undo = |done: &[(&PathBuf, &PathBuf)], copied: &[&PathBuf]| {
        for (from, to) in done.iter().rev() {
            let _ = std::fs::rename(to, from);
        }
        for to in copied {
            let _ = std::fs::remove_file(to);
        }
    };
    for (from, to) in &plan.copies {
        if let Err(e) = std::fs::copy(from, to) {
            undo(&done, &copied);
            return Err(FileError::Io(from.clone(), e));
        }
        copied.push(to);
    }
    for (from, to) in &plan.moves {
        // Pas d'écrasement : le plan l'a vérifié, mais un fichier a pu apparaître depuis.
        if to.exists() && !same(from, to) {
            undo(&done, &copied);
            return Err(FileError::Exists(to.clone()));
        }
        if let Err(e) = std::fs::rename(from, to) {
            undo(&done, &copied);
            return Err(FileError::Io(from.clone(), e));
        }
        done.push((from, to));
    }
    Ok(())
}

/// Prépare la suppression de `images` : les fichiers à supprimer (paires et sidecars
/// compris), à montrer avant de confirmer.
pub fn plan_delete(images: &[PathBuf], settings: &FileSettings) -> Group {
    group(images, settings.pairs)
}

/// Supprime les fichiers de `group` : à la corbeille si `trash`, sinon définitivement.
pub fn delete(group: &Group, trash: bool) -> Result<()> {
    let files: Vec<&PathBuf> = group.files().collect();
    if trash {
        return trash::delete_all(&files).map_err(|e| FileError::Trash(e.to_string()));
    }
    for file in files {
        std::fs::remove_file(file).map_err(|e| FileError::Io(file.clone(), e))?;
    }
    Ok(())
}

/// Nom du gestionnaire de fichiers du système, pour les libellés.
pub fn file_manager_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "le Finder"
    } else if cfg!(windows) {
        "l'Explorateur"
    } else {
        "le gestionnaire de fichiers"
    }
}

/// Montre `path` dans le gestionnaire de fichiers, sélectionné quand le système le permet
/// (macOS, Windows, gestionnaires Linux qui exposent `org.freedesktop.FileManager1` :
/// Nautilus, Nemo, Dolphin…) ; sinon ouvre son dossier.
pub fn reveal(path: &Path) -> Result<()> {
    let path = std::path::absolute(path).map_err(|e| FileError::Io(path.into(), e))?;
    if cfg!(target_os = "macos") {
        return spawn(Command::new("open").arg("-R").arg(&path), "open");
    }
    if cfg!(windows) {
        let mut select = std::ffi::OsString::from("/select,");
        select.push(path.as_os_str());
        return spawn(Command::new("explorer").arg(select), "explorer");
    }
    let uri = file_uri(&path);
    let shown = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.FileManager1",
            "--object-path",
            "/org/freedesktop/FileManager1",
            "--method",
            "org.freedesktop.FileManager1.ShowItems",
            &format!("['{uri}']"),
            "",
        ])
        .output()
        .is_ok_and(|o| o.status.success());
    if shown {
        return Ok(());
    }
    let dir = path.parent().unwrap_or(Path::new("/"));
    spawn(Command::new("xdg-open").arg(dir), "xdg-open")
}

/// URI `file://` de `path`, caractères spéciaux encodés (pour D-Bus).
fn file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                uri.push(byte as char)
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

/// Commande qui ouvre `files` dans l'éditeur `editor` (voir [`FileSettings::editor`]) :
/// programme et arguments. Vide : application par défaut du système. Séparée de
/// [`open_with`] pour être testée sans rien lancer.
pub fn editor_command(editor: &[String], files: &[PathBuf]) -> Vec<Command> {
    let Some((program, args)) = editor.split_first() else {
        return files.iter().map(|f| default_opener(f)).collect();
    };
    let mut command = Command::new(program);
    let mut placed = false;
    for arg in args {
        if arg == "{files}" {
            command.args(files);
            placed = true;
        } else {
            command.arg(arg);
        }
    }
    if !placed {
        command.args(files);
    }
    vec![command]
}

fn default_opener(file: &Path) -> Command {
    if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(file);
        c
    } else if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]).arg(file);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(file);
        c
    }
}

/// Ouvre `files` dans l'éditeur externe (sans attendre qu'il se ferme).
pub fn open_with(editor: &[String], files: &[PathBuf]) -> Result<()> {
    for mut command in editor_command(editor, files) {
        let name = command.get_program().to_string_lossy().into_owned();
        spawn(&mut command, &name)?;
    }
    Ok(())
}

/// Lance `command` sans l'attendre ; un fil récupère sa fin (pas de processus zombie).
fn spawn(command: &mut Command, name: &str) -> Result<()> {
    let mut child = command
        .spawn()
        .map_err(|e| FileError::Launch(name.into(), e))?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, names: &[&str]) {
        for name in names {
            std::fs::write(dir.join(name), name).unwrap();
        }
    }

    fn names(paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_group_takes_the_pair_and_its_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        touch(
            d,
            &["IMG.RAF", "IMG.JPG", "IMG.RAF.xmp", "IMG.xmp", "AUTRE.RAF"],
        );
        let g = group(&[d.join("IMG.RAF")], true);
        assert_eq!(names(&g.images), ["IMG.RAF", "IMG.JPG"]);
        let mut sidecars = names(&g.sidecars);
        sidecars.sort();
        assert_eq!(sidecars, ["IMG.RAF.xmp", "IMG.xmp"]);
        assert!(g.shared.is_empty());
    }

    #[test]
    fn without_pairs_a_shared_sidecar_stays() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        touch(d, &["IMG.RAF", "IMG.JPG", "IMG.RAF.xmp", "IMG.xmp"]);
        let g = group(&[d.join("IMG.RAF")], false);
        assert_eq!(names(&g.images), ["IMG.RAF"]);
        assert_eq!(names(&g.sidecars), ["IMG.RAF.xmp"]);
        assert_eq!(names(&g.shared), ["IMG.xmp"], "le JPG en a encore besoin");
    }

    #[test]
    fn invalid_names_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), &["IMG.RAF"]);
        let image = dir.path().join("IMG.RAF");
        for name in ["", "  ", ".", "..", "a/b"] {
            assert!(
                matches!(
                    plan_rename(&image, name, &FileSettings::default()),
                    Err(FileError::InvalidName(_))
                ),
                "{name:?}"
            );
        }
    }

    #[test]
    fn editor_command_places_the_files() {
        let files = [PathBuf::from("a.RAF"), PathBuf::from("b.RAF")];
        let args = |c: &Command| -> Vec<String> {
            c.get_args().map(|a| a.to_string_lossy().into()).collect()
        };
        let editor = vec!["open".to_string(), "-a".into(), "darktable".into()];
        let c = editor_command(&editor, &files);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].get_program(), "open");
        assert_eq!(args(&c[0]), ["-a", "darktable", "a.RAF", "b.RAF"]);
        let editor = vec!["ed".to_string(), "{files}".into(), "--fin".into()];
        assert_eq!(
            args(&editor_command(&editor, &files)[0]),
            ["a.RAF", "b.RAF", "--fin"]
        );
        // Sans éditeur : l'application par défaut, une commande par fichier.
        assert_eq!(editor_command(&[], &files).len(), 2);
    }

    #[test]
    fn uri_is_encoded() {
        assert_eq!(
            file_uri(Path::new("/a b/é'.RAF")),
            "file:///a%20b/%C3%A9%27.RAF"
        );
    }
}
