//! Développements d'une photo (décidé le 30/10/2026) : tout ce qui la rend, darktable
//! (le développement principal et ses clones) et RapidRAW (le principal et ses copies
//! virtuelles), au même niveau ; le rendu LibRaw neutre est la photo elle-même. Chaque
//! développement a ses notes, là où son logiciel les lit. Voir `doc/en/developments.md`.
//!
//! Un développement n'a pas de fichier image à lui : il a un chemin propre, jamais sur
//! le disque, dans le dossier de la photo, qui garde son extension (`IMG@dt.RAF`,
//! `IMG@dt-01.RAF`, `IMG@rr.RAF`, `IMG@rr-c33b94.RAF`) ; le reste de `tri` (cache, notes,
//! interface) le traite comme une image. Les pixels sont ceux de la photo
//! ([`crate::media::source`]), le rendu et les notes ceux de son sidecar.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, RwLock};

use crate::develop::Engine;

/// Lequel des développements d'un logiciel.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Copy {
    /// Le développement principal (`IMG.RAF.xmp`, `IMG.RAF.rrdata`).
    Main,
    /// Un clone darktable, par numéro (`IMG_01.RAF.xmp`).
    Clone(u32),
    /// Une copie virtuelle RapidRAW, par identifiant (`IMG.RAF.c33b94.rrdata`).
    Virtual(String),
}

/// Un développement d'une photo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Development {
    /// Logiciel.
    pub engine: Engine,
    /// Lequel.
    pub copy: Copy,
    /// La photo (le fichier image).
    pub photo: PathBuf,
    /// Son sidecar : réglages et notes.
    pub sidecar: PathBuf,
}

/// Séparateur du nom propre d'un développement (`IMG@dt.RAF`) : accepté par tous les
/// systèmes, rare dans un nom de photo.
const MARK: char = '@';

impl Development {
    /// Chemin propre du développement (jamais sur le disque).
    pub fn path(&self) -> PathBuf {
        let stem = self
            .photo
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let engine = match self.engine {
            Engine::Darktable => "dt",
            Engine::RapidRaw => "rr",
        };
        let copy = match &self.copy {
            Copy::Main => String::new(),
            Copy::Clone(n) => format!("-{n:02}"),
            Copy::Virtual(id) => format!("-{id}"),
        };
        let mut name = format!("{stem}{MARK}{engine}{copy}");
        if let Some(extension) = self.photo.extension() {
            name.push('.');
            name.push_str(&extension.to_string_lossy());
        }
        self.photo.with_file_name(name)
    }

    /// Vrai si les notes sont dans le `.rrdata` de RapidRAW (sinon dans un XMP).
    pub fn notes_in_rrdata(&self) -> bool {
        self.engine == Engine::RapidRaw
    }

    /// Le fichier propre à ce développement, qu'on peut supprimer sans toucher à la photo
    /// ni à ses autres développements : le sidecar d'un clone ou d'une copie virtuelle.
    /// Le développement principal partage le sien avec la photo (notes) : `None`.
    pub fn own_file(&self) -> Option<&Path> {
        (self.copy != Copy::Main).then_some(self.sidecar.as_path())
    }
}

/// Développements connus (vus au parcours d'un dossier) : chemin propre → développement.
static KNOWN: LazyLock<RwLock<HashMap<PathBuf, Development>>> = LazyLock::new(Default::default);

/// Retient `development` : son chemin propre désigne désormais lui.
pub fn register(development: &Development) {
    if let Ok(mut known) = KNOWN.write() {
        known.insert(development.path(), development.clone());
    }
}

/// Le développement de chemin propre `path`, s'il est connu.
pub fn get(path: &Path) -> Option<Development> {
    KNOWN.read().ok()?.get(path).cloned()
}

/// Vrai si le XMP de contenu `text` est de darktable : écrit par darktable
/// (`darktable:xmp_version`), ou avec un historique. Un XMP écrit par `tri` seul (notes)
/// ne l'est pas.
pub fn is_darktable(text: &str) -> bool {
    text.contains("darktable:xmp_version") || text.contains("darktable:history_end=\"")
}

/// Vrai si le XMP `path` est de darktable ([`is_darktable`]) : seul son début est lu
/// (darktable écrit ses attributs en tête).
pub fn file_is_darktable(path: &Path) -> bool {
    use std::io::Read;
    let mut head = vec![0; 16 * 1024];
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let n = file.read(&mut head).unwrap_or(0);
    is_darktable(&String::from_utf8_lossy(&head[..n]))
}

/// Les développements de `photo` d'après les noms des fichiers de son dossier : `names`
/// (noms réels, à côté de la photo). darktable : `IMG.RAF.xmp` s'il est de darktable
/// (`is_darktable` lit son début), ses clones `IMG_NN.RAF.xmp` (sans image `IMG_NN.RAF`,
/// voir `images`) ; RapidRAW : `IMG.RAF.rrdata` et `IMG.RAF.<id>.rrdata`. Dans cet ordre,
/// clones et copies par numéro et identifiant.
pub fn find(
    photo: &Path,
    names: &[String],
    images: &dyn Fn(&str) -> bool,
    is_darktable: &dyn Fn(&Path) -> bool,
) -> Vec<Development> {
    let Some(name) = photo.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return Vec::new();
    };
    let lower = name.to_lowercase();
    let (stem, extension) = lower.rsplit_once('.').unwrap_or((lower.as_str(), ""));
    let sidecar = |file: &str| photo.with_file_name(file);
    let new = |engine, copy, file: &str| Development {
        engine,
        copy,
        photo: photo.to_path_buf(),
        sidecar: sidecar(file),
    };
    let mut found = Vec::new();
    let mut clones = Vec::new();
    let mut rapidraw = Vec::new();
    let mut copies = Vec::new();
    for file in names {
        let l = file.to_lowercase();
        if l == format!("{lower}.xmp") {
            if is_darktable(&sidecar(file)) {
                found.push(new(Engine::Darktable, Copy::Main, file));
            }
        } else if let Some(image) = l.strip_suffix(".xmp")
            && let Some(number) = image
                .strip_suffix(&format!(".{extension}"))
                .and_then(|s| s.strip_prefix(&format!("{stem}_")))
                .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|n| n.parse::<u32>().ok())
            && !images(image)
        {
            clones.push(new(Engine::Darktable, Copy::Clone(number), file));
        } else if l == format!("{lower}.rrdata") {
            rapidraw.push(new(Engine::RapidRaw, Copy::Main, file));
        } else if let Some(id) = l
            .strip_prefix(&format!("{lower}."))
            .and_then(|r| r.strip_suffix(".rrdata"))
            .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric()))
        {
            // L'identifiant tel qu'écrit sur le disque, si la casse n'a pas changé les
            // longueurs (noms ASCII).
            let id = file
                .get(name.len() + 1..file.len() - ".rrdata".len())
                .filter(|_| file.len() == l.len())
                .unwrap_or(id)
                .to_string();
            copies.push(new(Engine::RapidRaw, Copy::Virtual(id), file));
        }
    }
    clones.sort_by(|a, b| a.copy.cmp(&b.copy));
    copies.sort_by(|a, b| a.copy.cmp(&b.copy));
    found.extend(clones);
    found.extend(rapidraw);
    found.extend(copies);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn developments_are_found_by_name_and_have_their_own_path() {
        let photo = Path::new("/p/IMG.RAF");
        let files = names(&[
            "IMG.RAF",
            "IMG.RAF.xmp",
            "IMG_02.RAF.xmp",
            "IMG_01.RAF.xmp",
            "IMG.RAF.rrdata",
            "IMG.RAF.c33b94.rrdata",
            // Une vraie image : son sidecar n'est pas un clone.
            "IMG_03.RAF.xmp",
            "AUTRE.RAF.xmp",
        ]);
        let found = find(photo, &files, &|n| n == "img_03.raf", &|_| true);
        let paths: Vec<String> = found
            .iter()
            .map(|d| d.path().file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            paths,
            [
                "IMG@dt.RAF",
                "IMG@dt-01.RAF",
                "IMG@dt-02.RAF",
                "IMG@rr.RAF",
                "IMG@rr-c33b94.RAF"
            ]
        );
        assert_eq!(found[1].sidecar, Path::new("/p/IMG_01.RAF.xmp"));
        assert_eq!(found[4].sidecar, Path::new("/p/IMG.RAF.c33b94.rrdata"));
        assert!(found[3].notes_in_rrdata() && !found[0].notes_in_rrdata());
        assert_eq!(found[0].own_file(), None, "le principal partage son XMP");
        assert_eq!(found[1].own_file(), Some(Path::new("/p/IMG_01.RAF.xmp")));
        // Un XMP écrit par tri seul (notes) : pas de développement darktable.
        let none = find(photo, &files, &|_| false, &|_| false);
        assert_eq!(none[0].copy, Copy::Clone(1));
    }

    #[test]
    fn darktable_xmp_are_told_from_notes_written_by_tri() {
        assert!(is_darktable(r#"<x darktable:xmp_version="5"/>"#));
        assert!(is_darktable(r#"<x darktable:history_end="0"/>"#));
        assert!(!is_darktable(
            r#"<x xmp:Rating="3" darktable:colorlabels="1"/>"#
        ));
    }

    #[test]
    fn a_registered_development_is_found_from_its_path() {
        let d = Development {
            engine: Engine::RapidRaw,
            copy: Copy::Virtual("ab12".into()),
            photo: "/q/DSCF0001.RAF".into(),
            sidecar: "/q/DSCF0001.RAF.ab12.rrdata".into(),
        };
        register(&d);
        assert_eq!(get(Path::new("/q/DSCF0001@rr-ab12.RAF")), Some(d));
        assert_eq!(get(Path::new("/q/DSCF0001.RAF")), None);
    }
}
