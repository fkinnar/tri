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
use std::io;
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
    /// ni à ses autres développements : le sidecar d'un clone ou d'une copie virtuelle, et
    /// le `.rrdata` du développement principal RapidRAW (la photo garde ses notes dans son
    /// XMP). Le développement principal darktable partage son XMP avec la photo (notes) :
    /// `None`, voir [`Development::removal`].
    pub fn own_file(&self) -> Option<&Path> {
        (self.copy != Copy::Main || self.notes_in_rrdata()).then_some(self.sidecar.as_path())
    }

    /// Ce que supprimer ce développement touche.
    pub fn removal(&self) -> Removal<'_> {
        match self.own_file() {
            Some(file) => Removal::File(file),
            None => Removal::Strip(&self.sidecar),
        }
    }
}

/// Ce que supprimer un développement touche (décidé le 30/10/2026) : le fichier qui n'est
/// qu'à lui (clone darktable, copie virtuelle ou développement principal RapidRAW), ou,
/// pour le développement principal darktable, son XMP **réécrit sans l'historique**
/// ([`crate::xmp::strip_darktable`]) : les notes, partagées avec la photo, restent, et la
/// photo redevient une image sans développement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal<'a> {
    /// Fichier à supprimer (corbeille).
    File(&'a Path),
    /// XMP à réécrire sans le développement.
    Strip(&'a Path),
}

/// Un sidecar réécrit sur place, et de quoi revenir en arrière ([`Rewrite::revert`], pour
/// [`crate::history`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rewrite {
    /// La photo du développement.
    pub photo: PathBuf,
    /// Le sidecar réécrit.
    pub sidecar: PathBuf,
    /// Son contenu avant.
    pub before: String,
    /// Son contenu après.
    pub after: String,
}

impl Rewrite {
    /// Prépare le retrait du développement principal darktable `development` : son XMP
    /// tel qu'il est, et sans l'historique. Rien n'est écrit ([`Rewrite::apply`]).
    pub fn strip(development: &Development) -> io::Result<Rewrite> {
        let before = std::fs::read_to_string(&development.sidecar)?;
        let after = crate::xmp::strip_darktable(&before)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        Ok(Rewrite {
            photo: development.photo.clone(),
            sidecar: development.sidecar.clone(),
            before,
            after,
        })
    }

    /// Écrit le contenu d'après. Faux si le fichier n'a plus celui d'avant (modifié
    /// entre-temps, par darktable par exemple) : rien n'est écrit.
    pub fn apply(&self) -> io::Result<bool> {
        rewrite(&self.sidecar, &self.before, &self.after)
    }

    /// Remet le contenu d'avant. Faux si le fichier n'a plus celui d'après.
    pub fn revert(&self) -> io::Result<bool> {
        rewrite(&self.sidecar, &self.after, &self.before)
    }
}

/// Écrit `new` dans `path` si son contenu est encore `expected` (relu juste avant, comme
/// pour les notes) ; le cache oublie sa date.
fn rewrite(path: &Path, expected: &str, new: &str) -> io::Result<bool> {
    if std::fs::read_to_string(path)? != expected {
        return Ok(false);
    }
    std::fs::write(path, new)?;
    crate::cache::forget_stamps([path]);
    Ok(true)
}

/// Développements connus (vus au parcours d'un dossier).
#[derive(Default)]
struct Registry {
    /// Chemin propre → développement.
    known: HashMap<PathBuf, Development>,
    /// Photo → chemins propres de ses développements, dans l'ordre de [`find`].
    by_photo: HashMap<PathBuf, Vec<PathBuf>>,
}

static REGISTRY: LazyLock<RwLock<Registry>> = LazyLock::new(Default::default);

/// Retient `development` : son chemin propre désigne désormais lui.
pub fn register(development: &Development) {
    let Ok(mut registry) = REGISTRY.write() else {
        return;
    };
    let path = development.path();
    let list = registry
        .by_photo
        .entry(development.photo.clone())
        .or_default();
    if !list.contains(&path) {
        list.push(path.clone());
    }
    registry.known.insert(path, development.clone());
}

/// Retient `developments` comme les développements de `photo`, à la place de ceux connus
/// avant (un clone supprimé depuis le dernier parcours n'est plus connu).
pub fn set_for(photo: &Path, developments: &[Development]) {
    let Ok(mut registry) = REGISTRY.write() else {
        return;
    };
    if let Some(old) = registry.by_photo.remove(photo) {
        for path in old {
            registry.known.remove(&path);
        }
    }
    let paths: Vec<PathBuf> = developments
        .iter()
        .map(|d| {
            let path = d.path();
            registry.known.insert(path.clone(), d.clone());
            path
        })
        .collect();
    if !paths.is_empty() {
        registry.by_photo.insert(photo.to_path_buf(), paths);
    }
}

/// Le développement de chemin propre `path`, s'il est connu.
pub fn get(path: &Path) -> Option<Development> {
    REGISTRY.read().ok()?.known.get(path).cloned()
}

/// Les développements connus de `photo`, dans l'ordre de [`find`] ; aucun pour un
/// développement ou une photo sans développement.
pub fn of(photo: &Path) -> Vec<Development> {
    let Ok(registry) = REGISTRY.read() else {
        return Vec::new();
    };
    registry
        .by_photo
        .get(photo)
        .map(|paths| {
            paths
                .iter()
                .filter_map(|p| registry.known.get(p).cloned())
                .collect()
        })
        .unwrap_or_default()
}

/// Rang du développement de chemin propre `path` parmi ceux de sa photo (ordre de
/// [`find`] : principal darktable, clones, principal RapidRAW, copies), pour les trier.
pub fn order(path: &Path) -> Option<usize> {
    let registry = REGISTRY.read().ok()?;
    let photo = &registry.known.get(path)?.photo;
    registry.by_photo.get(photo)?.iter().position(|p| p == path)
}

/// Vrai si `photo` a un développement connu de `engine` : son sidecar de ce logiciel est
/// celui du développement, la photo elle-même reste le rendu neutre (LibRaw).
pub fn has(photo: &Path, engine: Engine) -> bool {
    REGISTRY.read().is_ok_and(|registry| {
        registry.by_photo.get(photo).is_some_and(|paths| {
            paths
                .iter()
                .any(|p| registry.known.get(p).is_some_and(|d| d.engine == engine))
        })
    })
}

/// Le plus récent de `developments`, d'après la date de modification de son sidecar ;
/// `None` sans développement ou si aucun sidecar n'est lisible.
pub fn latest(developments: &[Development]) -> Option<&Development> {
    developments
        .iter()
        .filter_map(|d| crate::cache::stamp(&d.sidecar).map(|(_, modified)| (modified, d)))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, d)| d)
}

/// Place d'un développement parmi ceux de sa photo, pour son étiquette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rank {
    /// Le développement principal du logiciel.
    Main,
    /// Clone darktable, par son numéro (`IMG_01.RAF.xmp` : 1).
    Clone(u32),
    /// Copie virtuelle RapidRAW : son rang parmi les copies de la photo (la première : 1),
    /// l'identifiant du fichier ne disant rien.
    Copy(u32),
}

impl Development {
    /// Place de ce développement parmi `siblings`, les développements de sa photo
    /// (ordre de [`find`]).
    pub fn rank(&self, siblings: &[Development]) -> Rank {
        match &self.copy {
            Copy::Main => Rank::Main,
            Copy::Clone(n) => Rank::Clone(*n),
            Copy::Virtual(_) => {
                let position = siblings
                    .iter()
                    .filter(|d| d.engine == self.engine && matches!(d.copy, Copy::Virtual(_)))
                    .position(|d| d == self)
                    .unwrap_or(0);
                Rank::Copy(position as u32 + 1)
            }
        }
    }
}

/// Fichier des développements préférés (couverture de la pile d'une photo), dans le
/// dossier de données.
const PREFERRED: &str = "preferred-developments.txt";

/// Développements préférés (chemins propres), un par photo au plus.
pub fn preferred(data: &Path) -> std::collections::HashSet<PathBuf> {
    std::fs::read_to_string(data.join(PREFERRED))
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Retient (`on`) ou oublie `development` (chemin propre) comme développement préféré de
/// sa photo ; les autres développements connus de la même photo sont oubliés.
pub fn prefer(data: &Path, development: &Path, on: bool) -> std::io::Result<()> {
    let mut chosen = preferred(data);
    if let Some(d) = get(development) {
        for other in of(&d.photo) {
            chosen.remove(&other.path());
        }
    }
    if on {
        chosen.insert(development.to_path_buf());
    } else {
        chosen.remove(development);
    }
    let mut lines: Vec<String> = chosen.iter().map(|p| p.display().to_string()).collect();
    lines.sort();
    std::fs::create_dir_all(data)?;
    std::fs::write(data.join(PREFERRED), lines.join("\n") + "\n")
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
/// voir `images`, et de darktable aussi) ; RapidRAW : `IMG.RAF.rrdata` et `IMG.RAF.<id>.rrdata`. Dans cet ordre,
/// clones et copies par numéro et identifiant.
pub fn find(
    photo: &Path,
    names: &[String],
    images: &dyn Fn(&str) -> bool,
    is_darktable: &dyn Fn(&Path) -> bool,
    is_rapidraw: &dyn Fn(&Path) -> bool,
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
            // Un clone aussi doit être de darktable : un sidecar orphelin d'un autre
            // logiciel (digiKam…) n'en est pas un.
            && is_darktable(&sidecar(file))
        {
            clones.push(new(Engine::Darktable, Copy::Clone(number), file));
        } else if l == format!("{lower}.rrdata") {
            // Un `.rrdata` sans réglages n'est qu'un sidecar de notes (31/10/2026,
            // `rrdata::is_development`) : la photo notée dans RapidRAW, ou par `tri` pour
            // lui, sans développement.
            if is_rapidraw(&sidecar(file)) {
                rapidraw.push(new(Engine::RapidRaw, Copy::Main, file));
            }
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
        let found = find(photo, &files, &|n| n == "img_03.raf", &|_| true, &|_| true);
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
        assert_eq!(
            found[0].removal(),
            Removal::Strip(Path::new("/p/IMG.RAF.xmp"))
        );
        assert_eq!(found[1].own_file(), Some(Path::new("/p/IMG_01.RAF.xmp")));
        assert_eq!(
            found[3].own_file(),
            Some(Path::new("/p/IMG.RAF.rrdata")),
            "le principal RapidRAW n'a que lui dans son .rrdata (la photo note dans son XMP)"
        );
        // Un XMP écrit par tri seul ou par un autre logiciel (notes digiKam…) : pas de
        // développement darktable, clones compris ; RapidRAW reste.
        let none = find(photo, &files, &|_| false, &|_| false, &|_| true);
        assert_eq!(none.len(), 2);
        assert_eq!(none[0].engine, Engine::RapidRaw);
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
        assert_eq!(get(Path::new("/q/DSCF0001@rr-ab12.RAF")), Some(d.clone()));
        assert_eq!(get(Path::new("/q/DSCF0001.RAF")), None);
        assert_eq!(of(Path::new("/q/DSCF0001.RAF")), std::slice::from_ref(&d));
        assert!(has(Path::new("/q/DSCF0001.RAF"), Engine::RapidRaw));
        assert!(!has(Path::new("/q/DSCF0001.RAF"), Engine::Darktable));
        assert!(of(&d.path()).is_empty(), "un développement n'en a pas");
    }

    #[test]
    fn a_photo_s_developments_are_replaced_at_each_scan() {
        let photo = Path::new("/r/A.RAF");
        let clone = |n| Development {
            engine: Engine::Darktable,
            copy: Copy::Clone(n),
            photo: photo.to_path_buf(),
            sidecar: photo.with_file_name(format!("A_{n:02}.RAF.xmp")),
        };
        set_for(photo, &[clone(1), clone(2)]);
        assert_eq!(of(photo).len(), 2);
        assert!(get(&clone(2).path()).is_some());
        // Le clone 2 supprimé : plus connu.
        set_for(photo, &[clone(1)]);
        assert_eq!(of(photo), [clone(1)]);
        assert!(get(&clone(2).path()).is_none());
        set_for(photo, &[]);
        assert!(of(photo).is_empty() && !has(photo, Engine::Darktable));
    }

    #[test]
    fn ranks_number_virtual_copies_in_order() {
        let photo = Path::new("/s/B.RAF");
        let new = |engine, copy, file: &str| Development {
            engine,
            copy,
            photo: photo.to_path_buf(),
            sidecar: photo.with_file_name(file),
        };
        let list = [
            new(Engine::Darktable, Copy::Main, "B.RAF.xmp"),
            new(Engine::Darktable, Copy::Clone(3), "B_03.RAF.xmp"),
            new(Engine::RapidRaw, Copy::Main, "B.RAF.rrdata"),
            new(
                Engine::RapidRaw,
                Copy::Virtual("aa".into()),
                "B.RAF.aa.rrdata",
            ),
            new(
                Engine::RapidRaw,
                Copy::Virtual("bb".into()),
                "B.RAF.bb.rrdata",
            ),
        ];
        let ranks: Vec<Rank> = list.iter().map(|d| d.rank(&list)).collect();
        assert_eq!(
            ranks,
            [
                Rank::Main,
                Rank::Clone(3),
                Rank::Main,
                Rank::Copy(1),
                Rank::Copy(2)
            ]
        );
    }

    #[test]
    fn the_preferred_development_is_one_per_photo() {
        let dir = tempfile::tempdir().unwrap();
        let photo = Path::new("/t/C.RAF");
        let dt = Development {
            engine: Engine::Darktable,
            copy: Copy::Main,
            photo: photo.to_path_buf(),
            sidecar: photo.with_file_name("C.RAF.xmp"),
        };
        let rr = Development {
            engine: Engine::RapidRaw,
            copy: Copy::Main,
            photo: photo.to_path_buf(),
            sidecar: photo.with_file_name("C.RAF.rrdata"),
        };
        set_for(photo, &[dt.clone(), rr.clone()]);
        prefer(dir.path(), &dt.path(), true).unwrap();
        assert_eq!(
            preferred(dir.path()).into_iter().collect::<Vec<_>>(),
            [dt.path()]
        );
        // Une autre préférée : la première est oubliée.
        prefer(dir.path(), &rr.path(), true).unwrap();
        assert_eq!(
            preferred(dir.path()).into_iter().collect::<Vec<_>>(),
            [rr.path()]
        );
        prefer(dir.path(), &rr.path(), false).unwrap();
        assert!(preferred(dir.path()).is_empty());
    }

    #[test]
    fn stripping_a_main_development_is_prepared_then_applied_and_reverted() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("E.RAF");
        let sidecar = dir.path().join("E.RAF.xmp");
        let xml = concat!(
            r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">"#,
            r#"<rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:darktable="http://darktable.sf.net/" "#,
            r#"xmp:Rating="4" darktable:xmp_version="5" darktable:history_end="1"><darktable:history><rdf:Seq><rdf:li darktable:operation="exposure"/></rdf:Seq></darktable:history>"#,
            r#"</rdf:Description></rdf:RDF></x:xmpmeta>"#
        );
        std::fs::write(&sidecar, xml).unwrap();
        let main = Development {
            engine: Engine::Darktable,
            copy: Copy::Main,
            photo: photo.clone(),
            sidecar: sidecar.clone(),
        };
        let rewrite = Rewrite::strip(&main).unwrap();
        assert_eq!(rewrite.before, xml);
        assert!(!is_darktable(&rewrite.after));
        assert_eq!(
            std::fs::read_to_string(&sidecar).unwrap(),
            xml,
            "rien d'écrit"
        );
        assert!(rewrite.apply().unwrap());
        assert!(!file_is_darktable(&sidecar));
        assert_eq!(crate::xmp::read_sidecar(&sidecar).unwrap().stars, 4);
        // Déjà appliqué : le contenu n'est plus celui d'avant.
        assert!(!rewrite.apply().unwrap());
        assert!(rewrite.revert().unwrap());
        assert_eq!(std::fs::read_to_string(&sidecar).unwrap(), xml);
        // Modifié ailleurs entre-temps : refusé, rien d'écrit.
        std::fs::write(&sidecar, "<x/>").unwrap();
        assert!(!rewrite.apply().unwrap());
        assert_eq!(std::fs::read_to_string(&sidecar).unwrap(), "<x/>");
    }

    #[test]
    fn the_latest_development_is_the_one_with_the_newest_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("D.RAF");
        std::fs::write(&photo, b"x").unwrap();
        let old = photo.with_file_name("D.RAF.xmp");
        let new = photo.with_file_name("D.RAF.rrdata");
        std::fs::write(&old, b"x").unwrap();
        let earlier = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
        // Ouvert en écriture : sous Windows, changer la date d'un fichier ouvert en
        // lecture seule est refusé.
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(earlier)
            .unwrap();
        std::fs::write(&new, b"x").unwrap();
        let list = [
            Development {
                engine: Engine::Darktable,
                copy: Copy::Main,
                photo: photo.clone(),
                sidecar: old,
            },
            Development {
                engine: Engine::RapidRaw,
                copy: Copy::Main,
                photo: photo.clone(),
                sidecar: new,
            },
        ];
        assert_eq!(latest(&list), Some(&list[1]));
        assert_eq!(latest(&[]), None);
    }
}
