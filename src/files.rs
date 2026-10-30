//! Actions sur les fichiers : renommer, supprimer (corbeille), copier / déplacer vers un
//! dossier, afficher dans le gestionnaire de fichiers, ouvrir dans un éditeur externe.
//!
//! Une image ne se déplace jamais seule : ses **sidecars XMP** la suivent (sinon ses notes
//! seraient perdues ou attribuées à une autre image), et, par défaut, l'autre image de sa
//! **paire RAW+JPEG** aussi (`[files] pairs`), comme pour les notes. Chaque action se
//! prépare d'abord (`plan_*`) : la liste des fichiers touchés est montrée avant d'agir, et
//! les erreurs prévisibles (nom déjà pris) arrivent avant la première modification. Voir
//! `doc/en/files.md`.

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
    /// Copier / déplacer vers… : nombre de destinations récentes proposées.
    pub recent: usize,
    /// L'interface peut renommer, copier, déplacer et supprimer images et dossiers
    /// (choix de l'utilisateur, 30/10/2026 : interdit par défaut). Un logiciel de
    /// développement (darktable, Lightroom, Capture One…) attend ses images à leur place :
    /// les déplacer ou les supprimer d'ailleurs casse sa bibliothèque. Autorisé, chaque
    /// boîte le rappelle. Le déchargement d'une carte n'est pas concerné.
    pub allow_actions: bool,
}

impl Default for FileSettings {
    fn default() -> Self {
        FileSettings {
            pairs: true,
            trash: true,
            editor: Vec::new(),
            recent: 9,
            allow_actions: false,
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
    /// Destination absente, ou pas un dossier.
    NotADirectory(PathBuf),
    /// Destination = dossier d'origine d'un fichier.
    SameFolder(PathBuf),
    /// Copie relue différente de l'original.
    Mismatch(PathBuf),
    /// Dossier copié ou déplacé dans lui-même (ou un de ses sous-dossiers).
    IntoItself(PathBuf),
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(n) => write!(f, "{}", crate::t!("files-invalid-name", name = n)),
            Self::Exists(p) => write!(
                f,
                "{}",
                crate::t!("files-exists", path = p.display().to_string())
            ),
            Self::Io(p, e) => write!(f, "{} : {e}", p.display()),
            Self::Trash(e) => write!(f, "{}", crate::t!("files-trash-failed", error = e)),
            Self::Launch(p, e) => write!(
                f,
                "{}",
                crate::t!("files-launch-failed", program = p, error = e.to_string())
            ),
            Self::NotADirectory(p) => write!(
                f,
                "{}",
                crate::t!("files-not-a-directory", path = p.display().to_string())
            ),
            Self::SameFolder(p) => write!(
                f,
                "{}",
                crate::t!("files-same-folder", path = p.display().to_string())
            ),
            Self::Mismatch(p) => write!(
                f,
                "{}",
                crate::t!("files-mismatch", path = p.display().to_string())
            ),
            Self::IntoItself(p) => write!(
                f,
                "{}",
                crate::t!("files-into-itself", path = p.display().to_string())
            ),
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
    /// Leurs sidecars XMP, clones darktable et réglages d'autres logiciels qui ne servent
    /// qu'à elles ([`SidecarIndex::companions`]).
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

/// Vrai si les paires RAW+JPEG ajoutent des fichiers à `images` (l'autre image d'une
/// paire n'est pas déjà choisie) : la boîte propose alors de les laisser (retour du
/// 21/10/2026 : supprimer tous les JPEG emportait les RAW).
pub fn adds_pairs(images: &[PathBuf]) -> bool {
    let chosen: HashSet<&PathBuf> = images.iter().collect();
    images
        .iter()
        .any(|image| same_stem(image).iter().any(|m| !chosen.contains(m)))
}

/// Fichiers touchés par une action sur `images` : les images, leurs paires si `pairs`, et
/// les sidecars qui ne servent qu'à elles.
pub fn group(images: &[PathBuf], pairs: bool) -> Group {
    let mut group = Group::default();
    let mut seen = HashSet::new();
    // Un clone darktable ne se sépare pas de son image (choix de l'utilisateur,
    // 30/10/2026) : l'image d'origine, ses clones avec elle.
    let images: Vec<PathBuf> = images
        .iter()
        .map(|i| media::source(i).into_owned())
        .collect();
    for image in &images {
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
        for sidecar in index.companions(image) {
            if !sidecars_seen.insert(sidecar.clone()) {
                continue;
            }
            // Propriétaires du sidecar : les images de même nom qui le reconnaissent.
            let owners: Vec<PathBuf> = same_stem(image)
                .into_iter()
                .filter(|other| index.companions(other).contains(&sidecar))
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
    check_name(new_stem)?;
    // Un clone darktable : son image d'origine est renommée, avec tous ses clones.
    let image = media::source(image);
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

/// Erreur si `name` ne peut pas être un nom de fichier ou de dossier : vide, « . », « .. »,
/// ou avec un séparateur.
fn check_name(name: &str) -> Result<()> {
    let invalid = name.trim().is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\'])
        || name.contains(std::path::MAIN_SEPARATOR);
    if invalid {
        Err(FileError::InvalidName(name.into()))
    } else {
        Ok(())
    }
}

/// Crée le dossier `name` dans `parent` ; erreur si le nom est invalide ou pris.
pub fn create_dir(parent: &Path, name: &str) -> Result<PathBuf> {
    check_name(name)?;
    let path = parent.join(name.trim());
    if path.exists() {
        return Err(FileError::Exists(path));
    }
    std::fs::create_dir(&path).map_err(|e| FileError::Io(path.clone(), e))?;
    Ok(path)
}

/// Renomme le dossier `path` en `name` (même dossier parent) ; changer seulement la casse
/// est permis. Renvoie le nouveau chemin.
pub fn rename_dir(path: &Path, name: &str) -> Result<PathBuf> {
    check_name(name)?;
    let to = path.with_file_name(name.trim());
    if to.exists() && !same(path, &to) {
        return Err(FileError::Exists(to));
    }
    std::fs::rename(path, &to).map_err(|e| FileError::Io(path.to_path_buf(), e))?;
    Ok(to)
}

/// Contenu d'un dossier, sous-dossiers compris : (fichiers, dossiers), pour la
/// confirmation d'une suppression.
pub fn dir_contents(path: &Path) -> (usize, usize) {
    walkdir::WalkDir::new(path)
        .min_depth(1)
        .into_iter()
        .flatten()
        .fold((0, 0), |(files, dirs), e| {
            if e.file_type().is_dir() {
                (files, dirs + 1)
            } else {
                (files + 1, dirs)
            }
        })
}

/// Supprime le dossier `path` et tout son contenu : à la corbeille si `trash` (renvoie de
/// quoi l'en ressortir, voir [`untrash`]), sinon définitivement.
pub fn delete_dir(path: &Path, trash: bool) -> Result<Vec<Trashed>> {
    if trash {
        return to_trash(&[&path.to_path_buf()], &mut |_, _| {});
    }
    std::fs::remove_dir_all(path).map_err(|e| FileError::Io(path.to_path_buf(), e))?;
    Ok(Vec::new())
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
    // Supprimer un clone darktable, c'est supprimer son sidecar (comme darktable) ;
    // son image d'origine reste.
    let (clones, images): (Vec<PathBuf>, Vec<PathBuf>) =
        images.iter().cloned().partition(|i| media::is_clone(i));
    let mut group = group(&images, settings.pairs);
    for clone in clones {
        if let Some(sidecar) = crate::develop::darktable_sidecar_path(&clone)
            && !group.sidecars.contains(&sidecar)
        {
            group.sidecars.push(sidecar);
        }
    }
    group
}

/// Un fichier mis à la corbeille, et de quoi l'en ressortir ([`untrash`]).
#[derive(Clone, Debug)]
pub struct Trashed {
    /// Emplacement d'origine.
    pub original: PathBuf,
    /// macOS : emplacement dans la corbeille, donné par le système.
    #[cfg(target_os = "macos")]
    pub location: Option<PathBuf>,
    /// Linux, Windows : l'élément de la corbeille (le crate `trash` sait l'y retrouver).
    #[cfg(any(
        target_os = "windows",
        all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        )
    ))]
    pub item: Option<trash::TrashItem>,
}

/// Supprime les fichiers de `group` : à la corbeille si `trash`, sinon définitivement.
/// Renvoie les fichiers mis à la corbeille, pour pouvoir les remettre en place
/// ([`untrash`] ; vide pour une suppression définitive).
pub fn delete(group: &Group, trash: bool) -> Result<Vec<Trashed>> {
    delete_with_progress(group, trash, |_, _| {})
}

/// Comme [`delete`] ; `progress` reçoit (faits, total) après chaque fichier (une
/// centaine d'images à la corbeille prend du temps : à lancer hors de l'interface).
pub fn delete_with_progress(
    group: &Group,
    trash: bool,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<Trashed>> {
    let files: Vec<&PathBuf> = group.files().collect();
    if trash {
        return to_trash(&files, &mut progress);
    }
    let total = files.len();
    for (i, file) in files.into_iter().enumerate() {
        std::fs::remove_file(file).map_err(|e| FileError::Io(file.clone(), e))?;
        progress(i + 1, total);
    }
    Ok(Vec::new())
}

/// macOS : le crate `trash` ne dit pas où il met les fichiers ; on demande au système
/// (`NSFileManager`), qui renvoie l'emplacement exact dans la corbeille.
#[cfg(target_os = "macos")]
fn to_trash(files: &[&PathBuf], progress: &mut dyn FnMut(usize, usize)) -> Result<Vec<Trashed>> {
    use objc2_foundation::{NSFileManager, NSString, NSURL};
    let manager = NSFileManager::defaultManager();
    let mut trashed = Vec::new();
    for file in files {
        let absolute = std::path::absolute(file).map_err(|e| FileError::Io((*file).clone(), e))?;
        let url = NSURL::fileURLWithPath(&NSString::from_str(&absolute.to_string_lossy()));
        let mut resulting = None;
        manager
            .trashItemAtURL_resultingItemURL_error(&url, Some(&mut resulting))
            .map_err(|e| FileError::Trash(format!("{} : {e}", file.display())))?;
        let location = resulting
            .and_then(|u| u.path())
            .map(|p| PathBuf::from(p.to_string()));
        trashed.push(Trashed {
            original: absolute,
            location,
        });
        progress(trashed.len(), files.len());
    }
    Ok(trashed)
}

#[cfg(any(
    target_os = "windows",
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
))]
fn to_trash(files: &[&PathBuf], progress: &mut dyn FnMut(usize, usize)) -> Result<Vec<Trashed>> {
    let absolute: Vec<PathBuf> = files
        .iter()
        .map(|f| std::path::absolute(f).map_err(|e| FileError::Io((*f).clone(), e)))
        .collect::<Result<_>>()?;
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64 - 2)
        .unwrap_or(0);
    // Un par un, pour la progression.
    for (i, file) in absolute.iter().enumerate() {
        trash::delete(file).map_err(|e| FileError::Trash(e.to_string()))?;
        progress(i + 1, absolute.len());
    }
    // Retrouvés dans la corbeille par leur emplacement d'origine (les plus récents) ; la
    // corbeille entière est lue une fois, indexée pour ne pas la parcourir par fichier.
    let mut latest: std::collections::HashMap<PathBuf, trash::TrashItem> =
        std::collections::HashMap::new();
    for item in trash::os_limited::list().unwrap_or_default() {
        if item.time_deleted < since {
            continue;
        }
        let original = item.original_path();
        if latest
            .get(&original)
            .is_none_or(|known| known.time_deleted < item.time_deleted)
        {
            latest.insert(original, item);
        }
    }
    Ok(absolute
        .into_iter()
        .map(|original| {
            let item = latest.remove(&original);
            Trashed { original, item }
        })
        .collect())
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(unix, not(target_os = "ios"), not(target_os = "android"))
)))]
fn to_trash(files: &[&PathBuf], progress: &mut dyn FnMut(usize, usize)) -> Result<Vec<Trashed>> {
    trash::delete_all(files).map_err(|e| FileError::Trash(e.to_string()))?;
    progress(files.len(), files.len());
    Ok(Vec::new())
}

/// Remet en place des fichiers mis à la corbeille par [`delete`]. Erreur (rien n'est
/// fait pour ce fichier) si l'emplacement d'origine est repris, ou si le fichier n'est
/// plus dans la corbeille (vidée).
pub fn untrash(items: &[Trashed]) -> Result<()> {
    for t in items {
        if t.original.exists() {
            return Err(FileError::Exists(t.original.clone()));
        }
    }
    for t in items {
        restore_one(t)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn restore_one(t: &Trashed) -> Result<()> {
    let gone = || {
        FileError::Trash(crate::t!(
            "files-not-in-trash",
            path = t.original.display().to_string()
        ))
    };
    let location = t
        .location
        .as_ref()
        .filter(|l| l.exists())
        .ok_or_else(gone)?;
    match std::fs::rename(location, &t.original) {
        Ok(()) => Ok(()),
        // Corbeille d'un autre volume que l'origine : copie vérifiée, puis suppression.
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            copy_verified(location, &t.original)?;
            std::fs::remove_file(location).map_err(|e| FileError::Io(location.clone(), e))
        }
        Err(e) => Err(FileError::Io(location.clone(), e)),
    }
}

#[cfg(any(
    target_os = "windows",
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
))]
fn restore_one(t: &Trashed) -> Result<()> {
    let gone = || {
        FileError::Trash(crate::t!(
            "files-not-in-trash",
            path = t.original.display().to_string()
        ))
    };
    let item = t.item.clone().ok_or_else(gone)?;
    // Encore dans la corbeille ?
    let present = trash::os_limited::list()
        .unwrap_or_default()
        .into_iter()
        .any(|i| i.id == item.id);
    if !present {
        return Err(gone());
    }
    trash::os_limited::restore_all([item]).map_err(|e| FileError::Trash(e.to_string()))
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(unix, not(target_os = "ios"), not(target_os = "android"))
)))]
fn restore_one(t: &Trashed) -> Result<()> {
    Err(FileError::Trash(crate::t!(
        "files-not-in-trash",
        path = t.original.display().to_string()
    )))
}

/// Copier ou déplacer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferMode {
    /// Les originaux restent.
    Copy,
    /// Les originaux partent.
    Move,
}

/// Copie ou déplacement prévu vers un dossier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transfer {
    /// Copier ou déplacer.
    pub mode: TransferMode,
    /// Dossier de destination.
    pub dest: PathBuf,
    /// (source, cible) : images (paires comprises) puis sidecars.
    pub files: Vec<(PathBuf, PathBuf)>,
    /// Sidecars partagés avec une image qui reste (`IMAGE.xmp` d'une paire dont une seule
    /// image part) : toujours **copiés**, les deux images gardent leurs notes.
    pub shared: Vec<(PathBuf, PathBuf)>,
}

impl Transfer {
    /// Nombre de fichiers écrits dans la destination.
    pub fn len(&self) -> usize {
        self.files.len() + self.shared.len()
    }

    /// Vrai si rien n'est à copier.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Les images (pas les sidecars) et leur cible : pour suivre une image déplacée.
    pub fn images(&self) -> impl Iterator<Item = &(PathBuf, PathBuf)> {
        self.files
            .iter()
            .filter(|(from, _)| media::kind_of(from).is_some())
    }
}

/// Prépare la copie ou le déplacement de `images` dans `dest` : paires (`[files] pairs`)
/// et sidecars suivent, sous le même nom. Erreur si `dest` n'est pas un dossier, est le
/// dossier d'une image, ou contient déjà un des noms (jamais d'écrasement).
pub fn plan_transfer(
    images: &[PathBuf],
    dest: &Path,
    mode: TransferMode,
    settings: &FileSettings,
) -> Result<Transfer> {
    if !dest.is_dir() {
        return Err(FileError::NotADirectory(dest.to_path_buf()));
    }
    let group = group(images, settings.pairs);
    let target = |from: &PathBuf| dest.join(name_in_folder(from));
    let mut plan = Transfer {
        mode,
        dest: dest.to_path_buf(),
        files: group.files().map(|f| (f.clone(), target(f))).collect(),
        shared: group
            .shared
            .iter()
            .map(|f| (f.clone(), target(f)))
            .collect(),
    };
    let mut targets = HashSet::new();
    for (from, to) in plan.files.iter().chain(&plan.shared) {
        let folder = from.parent().unwrap_or(Path::new("."));
        if same(folder, dest) {
            return Err(FileError::SameFolder(dest.to_path_buf()));
        }
        if to.exists() || !targets.insert(to.clone()) {
            return Err(FileError::Exists(to.clone()));
        }
    }
    plan.files.dedup();
    Ok(plan)
}

/// Nom de `file` sous le dossier de son image : son nom, ou `CaptureOne/Settings…/nom`
/// pour les réglages de Capture One (gardés à leur place dans la destination).
fn name_in_folder(file: &Path) -> PathBuf {
    let name = PathBuf::from(file.file_name().unwrap_or_default());
    let settings = file.parent();
    let capture_one = settings.and_then(Path::parent);
    match (
        settings.and_then(Path::file_name),
        capture_one.and_then(Path::file_name),
    ) {
        (Some(settings), Some(dir)) if dir.eq_ignore_ascii_case("CaptureOne") => {
            PathBuf::from(dir).join(settings).join(name)
        }
        _ => name,
    }
}

/// Crée les dossiers manquants au-dessus de `to` (réglages de Capture One), ajoutés à
/// `made` pour les retirer si la copie est défaite.
fn make_parents(to: &Path, made: &mut Vec<PathBuf>) -> Result<()> {
    let mut missing = Vec::new();
    let mut dir = to.parent();
    while let Some(d) = dir.filter(|d| !d.exists()) {
        missing.push(d.to_path_buf());
        dir = d.parent();
    }
    for d in missing.into_iter().rev() {
        std::fs::create_dir(&d).map_err(|e| FileError::Io(d.clone(), e))?;
        made.push(d);
    }
    Ok(())
}

/// Copie `from` en `to` sans écraser, vérifiée : écrite sous un nom temporaire, relue et
/// comparée à l'original (BLAKE3), date de modification gardée (elle départage les
/// sidecars, voir `xmp`), puis renommée.
fn copy_verified(from: &Path, to: &Path) -> Result<()> {
    fn io_err(p: &Path) -> impl FnOnce(io::Error) -> FileError + '_ {
        move |e| FileError::Io(p.to_path_buf(), e)
    }
    let tmp = to.with_file_name(format!(
        ".{}.tri-tmp",
        to.file_name().unwrap_or_default().to_string_lossy()
    ));
    let result = (|| {
        std::fs::copy(from, &tmp).map_err(io_err(from))?;
        let modified = std::fs::metadata(from)
            .and_then(|m| m.modified())
            .map_err(io_err(from))?;
        std::fs::File::options()
            .write(true)
            .open(&tmp)
            .and_then(|f| f.set_modified(modified))
            .map_err(io_err(&tmp))?;
        if hash(from).map_err(io_err(from))? != hash(&tmp).map_err(io_err(&tmp))? {
            return Err(FileError::Mismatch(from.to_path_buf()));
        }
        if to.exists() {
            return Err(FileError::Exists(to.to_path_buf()));
        }
        std::fs::rename(&tmp, to).map_err(io_err(to))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn hash(path: &Path) -> io::Result<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(std::fs::File::open(path)?)?;
    Ok(hasher.finalize())
}

/// Exécute une copie ou un déplacement préparé par [`plan_transfer`] ; `progress` reçoit
/// (fichiers faits, total) après chaque fichier.
///
/// Copie : copies vérifiées. Déplacement : renommage sur le même volume ; sinon (autre
/// disque, carte, NAS) copies vérifiées de **tous** les fichiers, puis suppression des
/// originaux, jamais avant. Sur une erreur, ce qui a été fait est défait (copies
/// supprimées, renommages remis en place).
pub fn transfer(plan: &Transfer, mut progress: impl FnMut(usize, usize)) -> Result<()> {
    let total = plan.len();
    let mut done = 0;
    let mut created: Vec<&PathBuf> = Vec::new();
    let mut renamed: Vec<(&PathBuf, &PathBuf)> = Vec::new();
    // Dossiers des réglages de Capture One, créés dans la destination.
    let mut made = Vec::new();
    for (_, to) in plan.shared.iter().chain(&plan.files) {
        make_parents(to, &mut made)?;
    }
    let undo = |created: &[&PathBuf], renamed: &[(&PathBuf, &PathBuf)]| {
        for to in created {
            let _ = std::fs::remove_file(to);
        }
        for (from, to) in renamed.iter().rev() {
            let _ = std::fs::rename(to, from);
        }
        for dir in made.iter().rev() {
            let _ = std::fs::remove_dir(dir);
        }
    };
    let mut step = |result: Result<()>,
                    created: &[&PathBuf],
                    renamed: &[(&PathBuf, &PathBuf)]|
     -> Result<()> {
        if result.is_err() {
            undo(created, renamed);
        }
        result?;
        done += 1;
        progress(done, total);
        Ok(())
    };
    // Sidecars partagés : copiés dans tous les cas.
    for (from, to) in &plan.shared {
        let result = copy_verified(from, to);
        if result.is_ok() {
            created.push(to);
        }
        step(result, &created, &renamed)?;
    }
    // Déplacement : renommage tant que le volume est le même.
    let mut copy_rest = plan.mode == TransferMode::Copy;
    let mut sources_to_remove: Vec<&PathBuf> = Vec::new();
    for (from, to) in &plan.files {
        if !copy_rest {
            if to.exists() {
                undo(&created, &renamed);
                return Err(FileError::Exists(to.clone()));
            }
            match std::fs::rename(from, to) {
                Ok(()) => {
                    renamed.push((from, to));
                    step(Ok(()), &created, &renamed)?;
                    continue;
                }
                // Autre volume : copier ce fichier et les suivants, supprimer à la fin.
                Err(e) if e.kind() == io::ErrorKind::CrossesDevices => copy_rest = true,
                Err(e) => return step(Err(FileError::Io(from.clone(), e)), &created, &renamed),
            }
        }
        let result = copy_verified(from, to);
        if result.is_ok() {
            created.push(to);
            if plan.mode == TransferMode::Move {
                sources_to_remove.push(from);
            }
        }
        step(result, &created, &renamed)?;
    }
    // Tout est copié et vérifié : les originaux peuvent partir.
    for from in sources_to_remove {
        std::fs::remove_file(from).map_err(|e| FileError::Io(from.clone(), e))?;
    }
    Ok(())
}

/// Copie ou déplacement prévu d'un dossier entier (sous-dossiers compris) dans un autre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirTransfer {
    /// Copier ou déplacer.
    pub mode: TransferMode,
    /// Dossier copié ou déplacé.
    pub from: PathBuf,
    /// Ce qu'il devient : `dest` / son nom.
    pub to: PathBuf,
    /// (source, cible) de chaque fichier, sous-dossiers compris.
    pub files: Vec<(PathBuf, PathBuf)>,
    /// Sous-dossiers à créer dans la cible (parents d'abord), `to` compris.
    pub dirs: Vec<PathBuf>,
}

impl DirTransfer {
    /// Nombre de fichiers.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Vrai si le dossier ne contient aucun fichier.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Prépare la copie ou le déplacement du dossier `dir` dans `dest` (il y garde son nom).
/// Erreur si `dest` n'est pas un dossier, est déjà le dossier parent, est `dir` lui-même
/// ou un de ses sous-dossiers, ou contient déjà ce nom (jamais d'écrasement).
pub fn plan_transfer_dir(dir: &Path, dest: &Path, mode: TransferMode) -> Result<DirTransfer> {
    if !dir.is_dir() {
        return Err(FileError::NotADirectory(dir.to_path_buf()));
    }
    if !dest.is_dir() {
        return Err(FileError::NotADirectory(dest.to_path_buf()));
    }
    let (from, into) = match (dir.canonicalize(), dest.canonicalize()) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) => return Err(FileError::Io(dir.to_path_buf(), e)),
        (_, Err(e)) => return Err(FileError::Io(dest.to_path_buf(), e)),
    };
    if into.starts_with(&from) {
        return Err(FileError::IntoItself(dir.to_path_buf()));
    }
    if from.parent() == Some(into.as_path()) {
        return Err(FileError::SameFolder(dest.to_path_buf()));
    }
    let to = dest.join(dir.file_name().unwrap_or_default());
    if to.exists() {
        return Err(FileError::Exists(to));
    }
    let mut plan = DirTransfer {
        mode,
        from: dir.to_path_buf(),
        to: to.clone(),
        files: Vec::new(),
        dirs: vec![to.clone()],
    };
    // Le cache de `tri` gardé dans le dossier (`[cache] location = "folder"`, #55) ne
    // suit pas : rangé par chemin, il ne servirait à rien ailleurs, et ses fichiers
    // temporaires vont et viennent pendant la copie (#157 : « No such file »).
    let walk = walkdir::WalkDir::new(dir)
        .min_depth(1)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| e.file_name() != crate::cache::DISK_DIR);
    for entry in walk {
        let entry = entry.map_err(|e| {
            let path = e.path().unwrap_or(dir).to_path_buf();
            FileError::Io(path, e.into())
        })?;
        let rel = entry.path().strip_prefix(dir).unwrap_or(entry.path());
        let target = to.join(rel);
        if entry.file_type().is_dir() {
            plan.dirs.push(target);
        } else {
            plan.files.push((entry.path().to_path_buf(), target));
        }
    }
    Ok(plan)
}

/// Exécute une copie ou un déplacement de dossier préparé par [`plan_transfer_dir`] ;
/// `progress` reçoit (fichiers faits, total).
///
/// Déplacement sur le même volume : un seul renommage du dossier. Sinon (autre disque,
/// carte, NAS) et pour une copie : sous-dossiers créés, copies vérifiées de **tous** les
/// fichiers ; pour un déplacement, les originaux ne sont supprimés qu'ensuite (et les
/// sous-dossiers vidés, s'il n'y reste rien d'autre). Sur une erreur, ce qui a été écrit
/// est retiré.
pub fn transfer_dir(plan: &DirTransfer, progress: impl FnMut(usize, usize)) -> Result<()> {
    transfer_dir_with(plan, true, progress)
}

/// [`transfer_dir`] ; `rename` faux : comme vers un autre volume (copies puis
/// suppression), pour les tests.
fn transfer_dir_with(
    plan: &DirTransfer,
    rename: bool,
    mut progress: impl FnMut(usize, usize),
) -> Result<()> {
    let total = plan.len();
    if plan.to.exists() {
        return Err(FileError::Exists(plan.to.clone()));
    }
    if plan.mode == TransferMode::Move && rename {
        match std::fs::rename(&plan.from, &plan.to) {
            Ok(()) => {
                progress(total, total);
                return Ok(());
            }
            Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {}
            Err(e) => return Err(FileError::Io(plan.from.clone(), e)),
        }
    }
    let mut created_dirs: Vec<&PathBuf> = Vec::new();
    let mut created: Vec<&PathBuf> = Vec::new();
    let undo = |created: &[&PathBuf], dirs: &[&PathBuf]| {
        for f in created {
            let _ = std::fs::remove_file(f);
        }
        for d in dirs.iter().rev() {
            let _ = std::fs::remove_dir(d);
        }
    };
    for dir in &plan.dirs {
        if let Err(e) = std::fs::create_dir(dir) {
            undo(&created, &created_dirs);
            return Err(FileError::Io(dir.clone(), e));
        }
        created_dirs.push(dir);
    }
    for (i, (from, to)) in plan.files.iter().enumerate() {
        if let Err(e) = copy_verified(from, to) {
            undo(&created, &created_dirs);
            return Err(e);
        }
        created.push(to);
        progress(i + 1, total);
    }
    if plan.mode == TransferMode::Move {
        // Tout est copié et vérifié : les originaux peuvent partir. Un fichier apparu
        // entre-temps dans la source y reste, avec ses dossiers.
        for (from, _) in &plan.files {
            std::fs::remove_file(from).map_err(|e| FileError::Io(from.clone(), e))?;
        }
        let mut dirs: Vec<PathBuf> = plan
            .dirs
            .iter()
            .filter_map(|d| d.strip_prefix(&plan.to).ok())
            .map(|rel| plan.from.join(rel))
            .collect();
        dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
        for d in dirs {
            // Le cache resté dans la source (non copié) ne vaut plus rien : il part aussi.
            let _ = std::fs::remove_dir_all(d.join(crate::cache::DISK_DIR));
            let _ = std::fs::remove_dir(d);
        }
    }
    if total == 0 {
        progress(0, 0);
    }
    Ok(())
}

/// Fichier des destinations récentes, dans le dossier de données.
const RECENT_FILE: &str = "destinations";

/// Destinations récentes de « copier / déplacer vers… », la plus récente d'abord (celles
/// qui n'existent plus sont omises : carte retirée, dossier supprimé).
pub fn recent_destinations(data: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(data.join(RECENT_FILE))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| Path::new(l).components().collect::<PathBuf>())
        .filter(|p| p.is_dir())
        .fold(Vec::new(), |mut list, p| {
            // Fichier écrit avant le 20/10/2026 : doublons possibles (`~/Images/`).
            if !list.contains(&p) {
                list.push(p);
            }
            list
        })
}

/// Met `dest` en tête des destinations récentes, `max` au plus.
pub fn remember_destination(data: &Path, dest: &Path, max: usize) -> io::Result<()> {
    let path = data.join(RECENT_FILE);
    // Comparées en chemins : `~/Images/` (complété par Tab) et `~/Images` sont la même
    // (doublons vus le 20/10/2026) ; gardées sans séparateur final.
    let clean = |p: &Path| p.components().collect::<PathBuf>();
    let mut list: Vec<PathBuf> = vec![clean(dest)];
    for line in std::fs::read_to_string(&path).unwrap_or_default().lines() {
        let p = clean(Path::new(line));
        if !line.trim().is_empty() && !list.contains(&p) {
            list.push(p);
        }
    }
    let mut list: Vec<String> = list
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    list.truncate(max.max(1));
    std::fs::create_dir_all(data)?;
    std::fs::write(path, list.join("\n") + "\n")
}

/// Complétion d'un chemin de dossier en cours de saisie (`~` accepté) : les
/// sous-dossiers du dossier tapé dont le nom commence par la fin de la saisie (casse
/// ignorée, dossiers cachés seulement si la saisie commence par « . »), triés, terminés
/// par un séparateur. Pour la touche Tab de « copier / déplacer vers… ».
pub fn complete_dir(input: &str) -> Vec<String> {
    let sep = std::path::MAIN_SEPARATOR;
    let ends_with_sep = input.ends_with(['/', sep]);
    let (parent, prefix) = if ends_with_sep || input.is_empty() {
        (input, "")
    } else {
        match input.rfind(['/', sep]) {
            Some(i) => (&input[..=i], &input[i + 1..]),
            None => ("", input),
        }
    };
    let dir = if parent.is_empty() {
        PathBuf::from(".")
    } else {
        crate::settings::expand_home(parent)
    };
    let wanted = prefix.to_lowercase();
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_lowercase().starts_with(&wanted))
        .filter(|n| !n.starts_with('.') || prefix.starts_with('.'))
        .map(|n| format!("{parent}{n}{sep}"))
        .collect();
    found.sort_by_key(|n| n.to_lowercase());
    found
}

/// Début commun de `candidates` (casse respectée), ce que Tab complète d'abord.
pub fn common_prefix(candidates: &[String]) -> String {
    let Some(first) = candidates.first() else {
        return String::new();
    };
    let mut end = first.len();
    for other in &candidates[1..] {
        end = first
            .char_indices()
            .zip(other.chars())
            .take_while(|((_, a), b)| a == b)
            .last()
            .map(|((i, a), _)| i + a.len_utf8())
            .unwrap_or(0)
            .min(end);
    }
    first[..end].to_string()
}

/// Montre `path` dans le gestionnaire de fichiers, sélectionné quand le système le permet
/// (macOS, Windows, gestionnaires Linux qui exposent `org.freedesktop.FileManager1` :
/// Nautilus, Nemo, Dolphin…) ; sinon ouvre son dossier.
pub fn reveal(path: &Path) -> Result<()> {
    // Un clone darktable : son image d'origine.
    let path = media::source(path);
    let path = std::path::absolute(&path).map_err(|e| FileError::Io(path.into(), e))?;
    if cfg!(target_os = "macos") {
        return spawn(crate::process::command("open").arg("-R").arg(&path), "open");
    }
    if cfg!(windows) {
        let mut select = std::ffi::OsString::from("/select,");
        select.push(path.as_os_str());
        return spawn(crate::process::command("explorer").arg(select), "explorer");
    }
    let uri = file_uri(&path);
    let shown = crate::process::command("gdbus")
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
    spawn(crate::process::command("xdg-open").arg(dir), "xdg-open")
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
    // Clones darktable : leur image d'origine, une fois.
    let mut sources: Vec<PathBuf> = Vec::new();
    for file in files {
        let source = media::source(file).into_owned();
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    let files = &sources;
    let Some((program, args)) = editor.split_first() else {
        return files.iter().map(|f| default_opener(f)).collect();
    };
    let mut command = crate::process::command(program);
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
        let mut c = crate::process::command("open");
        c.arg(file);
        c
    } else if cfg!(windows) {
        let mut c = crate::process::command("cmd");
        c.args(["/C", "start", ""]).arg(file);
        c
    } else {
        let mut c = crate::process::command("xdg-open");
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

/// Lance `command` (un logiciel ouvert sur un dossier…) sans l'attendre, entrée et
/// sorties fermées.
pub fn launch(mut command: Command) -> Result<()> {
    let name = command.get_program().to_string_lossy().into_owned();
    spawn(&mut command, &name)
}

/// Lance `command` sans l'attendre ; un fil récupère sa fin (pas de processus zombie).
/// Entrée et sorties fermées : le programme lancé (éditeur, visionneuse Qt…) écrivait ses
/// traces dans le terminal de `tri` (retour du 24/10/2026, sous Mint).
fn spawn(command: &mut Command, name: &str) -> Result<()> {
    use std::process::Stdio;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
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

    /// Une séance avec des clones darktable et les réglages d'autres logiciels.
    fn developed(d: &Path) {
        touch(
            d,
            &[
                "IMG.RAF",
                "IMG.RAF.xmp",
                "IMG_01.RAF.xmp",
                "IMG_12.RAF.xmp",
                "IMG.RAF.pp3",
                "IMG.RAF.arp",
                // RapidRAW : le développement principal et une copie virtuelle.
                "IMG.RAF.rrdata",
                "IMG.RAF.c33b94.rrdata",
                // Une vraie image nommée comme un clone : son sidecar est à elle.
                "IMG_02.RAF",
                "IMG_02.RAF.xmp",
                // Pas des clones : autre extension, suffixe pas numérique.
                "IMG_01.JPG.xmp",
                "IMG_ab.RAF.xmp",
            ],
        );
        let settings = d.join("CaptureOne").join("Settings153");
        std::fs::create_dir_all(&settings).unwrap();
        touch(&settings, &["IMG.RAF.cos", "IMG_02.RAF.cos"]);
    }

    #[test]
    fn clones_and_the_settings_of_other_software_follow_the_image() {
        // Retour du 29/10/2026 : un clone darktable (`IMG_01.RAF.xmp`) restait derrière
        // l'image déplacée ; de même les réglages de RawTherapee, ART et Capture One.
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        developed(d);
        let g = group(&[d.join("IMG.RAF")], true);
        assert_eq!(names(&g.images), ["IMG.RAF"]);
        let mut sidecars = names(&g.sidecars);
        sidecars.sort();
        assert_eq!(
            sidecars,
            [
                "IMG.RAF.arp",
                "IMG.RAF.c33b94.rrdata",
                "IMG.RAF.cos",
                "IMG.RAF.pp3",
                "IMG.RAF.rrdata",
                "IMG.RAF.xmp",
                "IMG_01.RAF.xmp",
                "IMG_12.RAF.xmp"
            ]
        );
        let index = SidecarIndex::new(d);
        assert_eq!(
            index
                .clones(&d.join("IMG.RAF"))
                .iter()
                .map(|(n, _)| *n)
                .collect::<Vec<_>>(),
            [1, 12]
        );
        assert!(index.clones(&d.join("IMG_02.RAF")).is_empty());
        let other = group(&[d.join("IMG_02.RAF")], true);
        let mut sidecars = names(&other.sidecars);
        sidecars.sort();
        assert_eq!(sidecars, ["IMG_02.RAF.cos", "IMG_02.RAF.xmp"]);
        // Les notes, elles, ne lisent toujours que les sidecars de l'image.
        assert_eq!(names(&index.sidecars(&d.join("IMG.RAF"))), ["IMG.RAF.xmp"]);
    }

    #[test]
    fn a_renamed_image_takes_its_clones_and_settings_along() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        developed(d);
        let plan = plan_rename(&d.join("IMG.RAF"), "Mer", &FileSettings::default()).unwrap();
        rename(&plan).unwrap();
        for name in [
            "Mer.RAF",
            "Mer.RAF.xmp",
            "Mer_01.RAF.xmp",
            "Mer_12.RAF.xmp",
            "Mer.RAF.pp3",
            "Mer.RAF.arp",
            "Mer.RAF.rrdata",
            "Mer.RAF.c33b94.rrdata",
        ] {
            assert!(d.join(name).exists(), "{name}");
        }
        let settings = d.join("CaptureOne").join("Settings153");
        assert!(
            settings.join("Mer.RAF.cos").exists(),
            "Capture One, à sa place"
        );
        assert!(!d.join("IMG_01.RAF.xmp").exists());
        assert!(
            d.join("IMG_02.RAF.xmp").exists(),
            "l'autre image n'a pas bougé"
        );
    }

    #[test]
    fn a_clone_is_deleted_alone_but_never_moved_or_renamed_alone() {
        // Choix de l'utilisateur (30/10/2026) : supprimer un clone, c'est supprimer son
        // sidecar ; le copier, le déplacer ou le renommer emporte l'image et tous ses
        // clones (« tri ne peut pas scinder les versions »).
        let dir = tempfile::tempdir().unwrap();
        let (d, dest) = (dir.path().join("src"), dir.path().join("dest"));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        developed(&d);
        crate::media::scan(&d, 1).unwrap();
        let clone = d.join("IMG_01.RAF");
        assert!(crate::media::is_clone(&clone));
        let settings = FileSettings::default();
        let deleted = plan_delete(std::slice::from_ref(&clone), &settings);
        assert!(deleted.images.is_empty(), "l'image d'origine reste");
        assert_eq!(names(&deleted.sidecars), ["IMG_01.RAF.xmp"]);
        let moved = plan_transfer(
            std::slice::from_ref(&clone),
            &dest,
            TransferMode::Move,
            &settings,
        )
        .unwrap();
        let mut moved: Vec<String> = moved
            .files
            .iter()
            .map(|(from, _)| from.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        moved.sort();
        assert_eq!(
            moved,
            [
                "IMG.RAF",
                "IMG.RAF.arp",
                "IMG.RAF.c33b94.rrdata",
                "IMG.RAF.cos",
                "IMG.RAF.pp3",
                "IMG.RAF.rrdata",
                "IMG.RAF.xmp",
                "IMG_01.RAF.xmp",
                "IMG_12.RAF.xmp"
            ]
        );
        let plan = plan_rename(&clone, "Mer", &settings).unwrap();
        rename(&plan).unwrap();
        for name in ["Mer.RAF", "Mer_01.RAF.xmp", "Mer_12.RAF.xmp"] {
            assert!(d.join(name).exists(), "{name}");
        }
    }

    #[test]
    fn a_copied_image_keeps_the_capture_one_folders_of_its_settings() {
        let dir = tempfile::tempdir().unwrap();
        let (src, dest) = (dir.path().join("src"), dir.path().join("dest"));
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        developed(&src);
        let s = FileSettings::default();
        let plan = plan_transfer(&[src.join("IMG.RAF")], &dest, TransferMode::Move, &s).unwrap();
        assert_eq!(plan.len(), 9);
        transfer(&plan, |_, _| {}).unwrap();
        for name in [
            "IMG.RAF.c33b94.rrdata",
            "IMG.RAF",
            "IMG_01.RAF.xmp",
            "IMG_12.RAF.xmp",
            "IMG.RAF.pp3",
            "IMG.RAF.arp",
        ] {
            assert!(
                dest.join(name).exists() && !src.join(name).exists(),
                "{name}"
            );
        }
        let cos = Path::new("CaptureOne")
            .join("Settings153")
            .join("IMG.RAF.cos");
        assert!(dest.join(&cos).exists() && !src.join(&cos).exists());
        assert!(
            src.join("CaptureOne/Settings153/IMG_02.RAF.cos").exists(),
            "ceux des autres images restent"
        );
        // Annulé à mi-chemin (nom déjà pris) : rien de créé, dossiers compris.
        let other = dir.path().join("autre");
        std::fs::create_dir_all(&other).unwrap();
        touch(&other, &["IMG_02.RAF.xmp"]);
        assert!(plan_transfer(&[src.join("IMG_02.RAF")], &other, TransferMode::Copy, &s).is_err());
        assert!(!other.join("CaptureOne").exists());
    }

    #[test]
    fn pairs_add_files_only_when_the_other_image_is_not_chosen() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        touch(d, &["IMG.RAF", "IMG.JPG", "SEUL.JPG"]);
        assert!(adds_pairs(&[d.join("IMG.JPG")]));
        assert!(!adds_pairs(&[d.join("IMG.JPG"), d.join("IMG.RAF")]));
        assert!(!adds_pairs(&[d.join("SEUL.JPG")]));
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
    fn copy_and_move_take_pairs_and_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let (src, dest) = (dir.path().join("src"), dir.path().join("dest"));
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        touch(
            &src,
            &["IMG.RAF", "IMG.JPG", "IMG.RAF.xmp", "IMG.xmp", "B.RAF"],
        );
        let s = FileSettings::default();
        let image = [src.join("IMG.RAF")];
        let plan = plan_transfer(&image, &dest, TransferMode::Copy, &s).unwrap();
        assert_eq!(plan.len(), 4);
        assert_eq!(plan.images().count(), 2);
        let mut steps = Vec::new();
        transfer(&plan, |done, total| steps.push((done, total))).unwrap();
        assert_eq!(steps.last(), Some(&(4, 4)));
        for name in ["IMG.RAF", "IMG.JPG", "IMG.RAF.xmp", "IMG.xmp"] {
            assert!(
                src.join(name).exists() && dest.join(name).exists(),
                "{name}"
            );
            assert_eq!(std::fs::read(dest.join(name)).unwrap(), name.as_bytes());
        }
        // Jamais d'écrasement : tout est refusé avant la première copie.
        assert!(matches!(
            plan_transfer(&image, &dest, TransferMode::Copy, &s),
            Err(FileError::Exists(_))
        ));
        // Déplacement de B vers dest.
        let b = [src.join("B.RAF")];
        let plan = plan_transfer(&b, &dest, TransferMode::Move, &s).unwrap();
        transfer(&plan, |_, _| {}).unwrap();
        assert!(!src.join("B.RAF").exists() && dest.join("B.RAF").exists());
    }

    #[test]
    fn moving_one_image_of_a_pair_copies_the_shared_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let (src, dest) = (dir.path().join("src"), dir.path().join("dest"));
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        touch(&src, &["IMG.RAF", "IMG.JPG", "IMG.xmp"]);
        let s = FileSettings {
            pairs: false,
            ..FileSettings::default()
        };
        let plan = plan_transfer(&[src.join("IMG.RAF")], &dest, TransferMode::Move, &s).unwrap();
        transfer(&plan, |_, _| {}).unwrap();
        assert!(dest.join("IMG.RAF").exists() && !src.join("IMG.RAF").exists());
        assert!(src.join("IMG.xmp").exists(), "le JPG garde ses notes");
        assert!(dest.join("IMG.xmp").exists(), "le RAF aussi");
    }

    #[test]
    fn bad_destinations_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), &["IMG.RAF"]);
        let image = [dir.path().join("IMG.RAF")];
        let s = FileSettings::default();
        assert!(matches!(
            plan_transfer(&image, dir.path(), TransferMode::Move, &s),
            Err(FileError::SameFolder(_))
        ));
        assert!(matches!(
            plan_transfer(&image, &dir.path().join("absent"), TransferMode::Copy, &s),
            Err(FileError::NotADirectory(_))
        ));
    }

    #[test]
    fn a_verified_copy_keeps_the_modification_date() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("a.xmp");
        std::fs::write(&from, "x").unwrap();
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        std::fs::File::options()
            .write(true)
            .open(&from)
            .unwrap()
            .set_modified(old)
            .unwrap();
        let to = dir.path().join("b.xmp");
        copy_verified(&from, &to).unwrap();
        assert_eq!(std::fs::metadata(&to).unwrap().modified().unwrap(), old);
        assert!(matches!(
            copy_verified(&from, &to),
            Err(FileError::Exists(_))
        ));
    }

    #[test]
    fn recent_destinations_are_ordered_and_capped() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let (a, b, c) = (
            dir.path().join("a"),
            dir.path().join("b"),
            dir.path().join("c"),
        );
        for d in [&a, &b, &c] {
            std::fs::create_dir(d).unwrap();
        }
        assert!(recent_destinations(&data).is_empty());
        for d in [&a, &b, &a, &c] {
            remember_destination(&data, d, 2).unwrap();
        }
        assert_eq!(recent_destinations(&data), [c.clone(), a.clone()]);
        // Avec le séparateur final de la complétion : la même destination.
        let typed = PathBuf::from(format!("{}{}", a.display(), std::path::MAIN_SEPARATOR));
        remember_destination(&data, &typed, 2).unwrap();
        assert_eq!(recent_destinations(&data), [a.clone(), c.clone()]);
        std::fs::remove_dir(&a).unwrap();
        assert_eq!(recent_destinations(&data), [c], "disparue : omise");
    }

    #[test]
    fn a_folder_is_copied_or_moved_whole() {
        let dir = tempfile::tempdir().unwrap();
        let seance = dir.path().join("Séance");
        let dest = dir.path().join("Archive");
        std::fs::create_dir_all(seance.join("JPEG/vide")).unwrap();
        std::fs::create_dir(&dest).unwrap();
        touch(&seance, &["A.RAF", "A.RAF.xmp"]);
        touch(&seance.join("JPEG"), &["A.JPG"]);
        // Refusés : dans lui-même, là où il est déjà, par-dessus un dossier du même nom.
        assert!(matches!(
            plan_transfer_dir(&seance, &seance.join("JPEG"), TransferMode::Move),
            Err(FileError::IntoItself(_))
        ));
        assert!(matches!(
            plan_transfer_dir(&seance, &seance, TransferMode::Move),
            Err(FileError::IntoItself(_))
        ));
        assert!(matches!(
            plan_transfer_dir(&seance, dir.path(), TransferMode::Move),
            Err(FileError::SameFolder(_))
        ));
        assert!(matches!(
            plan_transfer_dir(&seance, &dir.path().join("absent"), TransferMode::Move),
            Err(FileError::NotADirectory(_))
        ));
        let plan = plan_transfer_dir(&seance, &dest, TransferMode::Copy).unwrap();
        assert_eq!(plan.len(), 3);
        assert_eq!(plan.to, dest.join("Séance"));
        assert_eq!(plan.dirs.len(), 3, "le dossier, JPEG et JPEG/vide");
        let mut steps = Vec::new();
        transfer_dir(&plan, |done, total| steps.push((done, total))).unwrap();
        assert_eq!(steps, [(1, 3), (2, 3), (3, 3)]);
        let copy = dest.join("Séance");
        assert!(copy.join("JPEG/A.JPG").is_file() && copy.join("JPEG/vide").is_dir());
        assert!(seance.join("A.RAF").exists(), "copie : l'original reste");
        assert!(matches!(
            plan_transfer_dir(&seance, &dest, TransferMode::Move),
            Err(FileError::Exists(_))
        ));
        // Déplacement comme vers un autre volume : copies, puis originaux supprimés ;
        // un fichier apparu entre-temps dans la source y reste, avec ses dossiers.
        std::fs::remove_dir_all(&copy).unwrap();
        let plan = plan_transfer_dir(&seance, &dest, TransferMode::Move).unwrap();
        touch(&seance.join("JPEG"), &["B.JPG"]);
        transfer_dir_with(&plan, false, |_, _| {}).unwrap();
        assert!(copy.join("JPEG/A.JPG").is_file() && copy.join("JPEG/vide").is_dir());
        assert!(!seance.join("A.RAF").exists() && !seance.join("JPEG/vide").exists());
        assert!(
            seance.join("JPEG/B.JPG").exists(),
            "apparu entre-temps : gardé"
        );
        std::fs::remove_file(seance.join("JPEG/B.JPG")).unwrap();
        // Retour (même volume : un renommage).
        let back = plan_transfer_dir(&copy, dir.path(), TransferMode::Move);
        assert!(
            matches!(back, Err(FileError::Exists(_))),
            "Séance existe encore (vide)"
        );
        std::fs::remove_dir_all(&seance).unwrap();
        let back = plan_transfer_dir(&copy, dir.path(), TransferMode::Move).unwrap();
        transfer_dir(&back, |_, _| {}).unwrap();
        assert!(!copy.exists() && seance.join("A.RAF.xmp").exists());
    }

    #[test]
    fn a_folder_s_own_cache_does_not_follow() {
        // #157 : le cache gardé dans le dossier (`[cache] location = "folder"`) ne suit
        // pas : ses fichiers temporaires disparaissaient pendant la copie.
        let dir = tempfile::tempdir().unwrap();
        let seance = dir.path().join("Séance");
        let dest = dir.path().join("Archive");
        let cache = seance.join(crate::cache::DISK_DIR).join("v4/embedded");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::create_dir(&dest).unwrap();
        touch(&seance, &["A.RAF"]);
        touch(&cache, &["x.jpg"]);
        let plan = plan_transfer_dir(&seance, &dest, TransferMode::Copy).unwrap();
        assert_eq!(plan.len(), 1, "le RAW seul");
        assert_eq!(plan.dirs.len(), 1);
        // Déplacement vers un autre volume : le cache resté dans la source part aussi.
        let plan = plan_transfer_dir(&seance, &dest, TransferMode::Move).unwrap();
        transfer_dir_with(&plan, false, |_, _| {}).unwrap();
        assert!(dest.join("Séance/A.RAF").is_file());
        assert!(!dest.join("Séance").join(crate::cache::DISK_DIR).exists());
        assert!(!seance.exists(), "plus rien dans la source");
    }

    #[test]
    fn directories_are_completed() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["Photos", "photos-2026", "Autre", ".cache"] {
            std::fs::create_dir(dir.path().join(d)).unwrap();
        }
        touch(dir.path(), &["Photo.txt"]);
        let sep = std::path::MAIN_SEPARATOR;
        let base = format!("{}{sep}", dir.path().display());
        let found = complete_dir(&format!("{base}pho"));
        assert_eq!(
            found,
            [
                format!("{base}photos-2026{sep}"),
                format!("{base}Photos{sep}")
            ]
        );
        assert_eq!(
            common_prefix(&found),
            base,
            "casse différente : rien de commun"
        );
        assert_eq!(complete_dir(&base).len(), 3, "sans les dossiers cachés");
        assert_eq!(complete_dir(&format!("{base}.c")).len(), 1);
        assert_eq!(
            common_prefix(&[format!("{base}Autre{sep}")]),
            format!("{base}Autre{sep}")
        );
    }

    #[test]
    fn folders_are_created_renamed_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let made = create_dir(dir.path(), "Séance").unwrap();
        assert!(made.is_dir());
        assert!(matches!(
            create_dir(dir.path(), "Séance"),
            Err(FileError::Exists(_))
        ));
        assert!(matches!(
            create_dir(dir.path(), "a/b"),
            Err(FileError::InvalidName(_))
        ));
        touch(&made, &["A.RAF", "A.RAF.xmp"]);
        std::fs::create_dir(made.join("sous")).unwrap();
        assert_eq!(dir_contents(&made), (2, 1));
        let renamed = rename_dir(&made, "Séance du jour").unwrap();
        assert!(renamed.join("A.RAF").exists() && !made.exists());
        create_dir(dir.path(), "Autre").unwrap();
        assert!(matches!(
            rename_dir(&renamed, "Autre"),
            Err(FileError::Exists(_))
        ));
        // Définitivement (les tests ne touchent jamais à la corbeille).
        delete_dir(&renamed, false).unwrap();
        assert!(!renamed.exists());
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
