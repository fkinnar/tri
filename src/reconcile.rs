//! Notes venues de plusieurs sources : fusion et conflits (issue n° 368, 07/11/2026).
//!
//! Une photo peut porter ses notes à plusieurs endroits : `.xmp` (darktable,
//! Lightroom…), `.rrdata` (RapidRAW), `.pp3` / `.arp` (RawTherapee, ART), l'image elle-même
//! ([`crate::embedded`]). Chaque logiciel n'en modifie qu'un. Pour ne perdre aucun
//! changement, `tri` retient, par image et **par source**, ce qu'il y a lu la dernière
//! fois (avec la date et la taille du fichier), et les notes qu'il en a retenues
//! ([`Memory`], fichier `.tri/notes.json` du dossier).
//!
//! À la lecture ([`reconcile`]) :
//!
//! - une source dont la date et la taille n'ont pas bougé n'a rien changé ;
//! - sinon, ses champs qui diffèrent de ce qu'on y avait lu sont **ses** changements (un
//!   JPEG retouché ailleurs sans toucher aux notes ne change donc rien) ;
//! - fusion **champ par champ** (étoiles, drapeau, chaque couleur) : changé par une source,
//!   ou par plusieurs de la même façon → adopté ; différemment → **conflit** ;
//! - une fusion de couleurs qu'aucune source ne porte (deux couleurs ajoutées chacune
//!   d'un côté) → conflit, pour ne pas en inventer en mode une seule couleur ;
//! - une source **inconnue** (première fois, sidecar créé par un autre logiciel) qui ne
//!   s'accorde pas avec les notes retenues → conflit : on demande, sans deviner d'après les
//!   dates (cartes en FAT à 2 s près, horloges des NAS).

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::xmp::{Color, Flag, Notes};

/// Date de modification (nanosecondes depuis 1970) et taille d'un fichier : s'ils n'ont
/// pas bougé, le fichier n'a pas changé.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    /// Date de modification, en nanosecondes depuis 1970.
    pub modified: u64,
    /// Taille, en octets.
    pub size: u64,
}

impl Stamp {
    /// Le tampon actuel de `path`.
    pub fn of(path: &Path) -> io::Result<Stamp> {
        let meta = std::fs::metadata(path)?;
        let modified = meta
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map(|d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        Ok(Stamp {
            modified,
            size: meta.len(),
        })
    }
}

/// Un champ des notes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    /// Les étoiles.
    Stars,
    /// Accepté / rejeté.
    Flag,
    /// Les couleurs.
    Colors,
}

/// Toutes les couleurs, dans l'ordre.
const COLORS: [Color; 5] = [
    Color::Red,
    Color::Yellow,
    Color::Green,
    Color::Blue,
    Color::Purple,
];

/// Une source telle qu'elle est maintenant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// Nom du fichier (dans le dossier de l'image).
    pub name: String,
    /// Son tampon.
    pub stamp: Stamp,
    /// Ses notes.
    pub notes: Notes,
    /// Ne porte aucune note (une image sans XMP ni note EXIF) : `notes` vides. Elle ne
    /// compte pas dans les accords, mais une note qui y apparaît ensuite est un changement
    /// connu.
    pub silent: bool,
}

/// Notes, sous la forme gardée dans `.tri/notes.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Stored {
    #[serde(default)]
    stars: u8,
    #[serde(default)]
    flag: Flag,
    #[serde(default)]
    colors: Vec<Color>,
}

impl From<&Notes> for Stored {
    fn from(n: &Notes) -> Self {
        Stored {
            stars: n.stars,
            flag: n.flag,
            colors: n.colors.clone(),
        }
    }
}

impl From<&Stored> for Notes {
    fn from(s: &Stored) -> Self {
        let mut n = Notes::default();
        n.set_stars(s.stars);
        n.flag = s.flag;
        n.colors = s.colors.clone();
        n.colors.sort();
        n.colors.dedup();
        n
    }
}

/// Ce que `tri` a lu dans une source la dernière fois.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Seen {
    #[serde(flatten)]
    stamp: Stamp,
    notes: Stored,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    silent: bool,
}

/// Ce que `tri` retient d'une image : ses notes, et ce qu'il a lu dans chaque source.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remembered {
    notes: Stored,
    #[serde(default)]
    sources: BTreeMap<String, Seen>,
}

impl Remembered {
    /// Les notes retenues.
    pub fn notes(&self) -> Notes {
        Notes::from(&self.notes)
    }

    /// Ce qu'on retient après une lecture sans conflit : `notes`, et chaque source telle
    /// quelle.
    pub fn agreed(notes: &Notes, sources: &[Source]) -> Remembered {
        Remembered {
            notes: Stored::from(notes),
            sources: sources
                .iter()
                .map(|s| {
                    let seen = Seen {
                        stamp: s.stamp,
                        notes: Stored::from(&s.notes),
                        silent: s.silent,
                    };
                    (s.name.clone(), seen)
                })
                .collect(),
        }
    }
}

/// Un conflit : des sources qui ne s'accordent pas, à trancher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// Champs en désaccord.
    pub fields: Vec<Field>,
    /// Chaque source et ses notes.
    pub sources: Vec<(String, Notes)>,
    /// Date de modification de chaque source, dans l'ordre de `sources` (nanosecondes
    /// depuis 1970 ; 0 : inconnue) : un indice pour choisir (08/11/2026, idée de
    /// l'utilisateur : un sidecar de 2012 face à un de 2026).
    pub modified: Vec<u64>,
    /// Les notes retenues la dernière fois, s'il y en a.
    pub before: Option<Notes>,
    /// Les champs qui ne sont pas en désaccord, déjà fusionnés (ceux en désaccord : les
    /// notes d'avant, ou les plus récentes) : la base du choix champ par champ.
    pub merged: Notes,
}

impl Field {
    /// Tous les champs, dans l'ordre.
    pub const ALL: [Field; 3] = [Field::Stars, Field::Flag, Field::Colors];

    /// Recopie ce champ de `from` dans `into`.
    pub fn copy(self, from: &Notes, into: &mut Notes) {
        match self {
            Field::Stars => into.set_stars(from.stars),
            Field::Flag => into.flag = from.flag,
            Field::Colors => into.colors = from.colors.clone(),
        }
    }

    /// Vrai si `a` et `b` ont la même valeur pour ce champ.
    pub fn same(self, a: &Notes, b: &Notes) -> bool {
        match self {
            Field::Stars => a.stars == b.stars,
            Field::Flag => a.flag == b.flag,
            Field::Colors => a.colors == b.colors,
        }
    }
}

/// Résultat d'une lecture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Rien de neuf : les notes retenues.
    Same,
    /// Des sources ont changé, sans se contredire : notes fusionnées, et les sources d'où
    /// viennent les changements.
    Merged {
        /// Sources qui ont apporté un changement.
        from: Vec<String>,
    },
    /// Des sources se contredisent.
    Conflict(Conflict),
}

/// Valeur d'un champ, pour comparer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
    Stars(u8),
    Flag(Flag),
    Color(bool),
}

/// Les champs élémentaires : étoiles, drapeau, et chaque couleur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Atom {
    Stars,
    Flag,
    Color(Color),
}

impl Atom {
    fn all() -> impl Iterator<Item = Atom> {
        [Atom::Stars, Atom::Flag]
            .into_iter()
            .chain(COLORS.into_iter().map(Atom::Color))
    }
    fn of(self, n: &Notes) -> Value {
        match self {
            Atom::Stars => Value::Stars(n.stars),
            Atom::Flag => Value::Flag(n.flag),
            Atom::Color(c) => Value::Color(n.colors.contains(&c)),
        }
    }
    fn set(self, n: &mut Notes, v: Value) {
        match v {
            Value::Stars(s) => n.set_stars(s),
            Value::Flag(f) => n.flag = f,
            Value::Color(on) => {
                let Atom::Color(c) = self else { return };
                n.colors.retain(|&x| x != c);
                if on {
                    n.colors.push(c);
                    n.colors.sort();
                }
            }
        }
    }
    fn field(self) -> Field {
        match self {
            Atom::Stars => Field::Stars,
            Atom::Flag => Field::Flag,
            Atom::Color(_) => Field::Colors,
        }
    }
}

/// Lecture des notes d'une image à partir de ses `sources`, et de ce qu'on en retenait
/// (`memory`). Renvoie les notes à montrer, le résultat, et ce qu'il faut retenir
/// désormais (`None` : garder l'ancien, en cas de conflit).
pub fn reconcile(
    memory: Option<&Remembered>,
    sources: &[Source],
) -> (Notes, Outcome, Option<Remembered>) {
    // Les sources qui disent quelque chose.
    let speaking = || sources.iter().filter(|s| !s.silent);
    let Some(memory) = memory else {
        // Rien de retenu : d'accord, ou on demande. Que des sources muettes : rien à
        // retenir (pas de `.tri` pour un dossier de JPEG jamais notés).
        let first = speaking()
            .next()
            .map(|s| s.notes.clone())
            .unwrap_or_default();
        if speaking().all(|s| s.notes == first) {
            return (
                first.clone(),
                Outcome::Same,
                speaking()
                    .next()
                    .is_some()
                    .then(|| Remembered::agreed(&first, sources)),
            );
        }
        // Les plus récentes ; à égalité (dates grossières de Windows, de la FAT), la
        // première source : un sidecar avant l'image (bug de la CI Windows, 07/11/2026).
        let newest = speaking()
            .rev()
            .max_by_key(|s| s.stamp.modified)
            .map(|s| s.notes.clone())
            .unwrap_or_default();
        let fields = disagreeing(speaking().map(|s| &s.notes));
        let conflict = Conflict {
            fields,
            sources: named(sources),
            modified: modified(sources),
            before: None,
            merged: newest.clone(),
        };
        return (newest, Outcome::Conflict(conflict), None);
    };
    let before = memory.notes();
    let mut merged = before.clone();
    let mut conflicts: Vec<Field> = Vec::new();
    let mut from: Vec<String> = Vec::new();
    // Changements de chaque source connue ; désaccords des sources inconnues.
    for atom in Atom::all() {
        let mut changed: Vec<(Value, &str)> = Vec::new();
        let mut unknown = false;
        for source in sources {
            let now = atom.of(&source.notes);
            match memory.sources.get(&source.name) {
                Some(seen) if seen.stamp == source.stamp => {}
                Some(seen) => {
                    if now != atom.of(&Notes::from(&seen.notes)) {
                        changed.push((now, &source.name));
                    }
                }
                None => {
                    if !source.silent && now != atom.of(&before) {
                        unknown = true;
                    }
                }
            }
        }
        let value = match changed.first() {
            _ if unknown => None,
            None => Some(atom.of(&before)),
            Some(&(v, _)) if changed.iter().all(|&(w, _)| w == v) => Some(v),
            Some(_) => None,
        };
        match value {
            Some(v) => {
                if v != atom.of(&before) {
                    atom.set(&mut merged, v);
                    for &(_, name) in &changed {
                        if !from.iter().any(|f| f == name) {
                            from.push(name.to_string());
                        }
                    }
                }
            }
            None => {
                if !conflicts.contains(&atom.field()) {
                    conflicts.push(atom.field());
                }
            }
        }
    }
    // Couleurs fusionnées qu'aucune source ne porte : on ne les invente pas.
    if !conflicts.contains(&Field::Colors)
        && merged.colors != before.colors
        && !sources.iter().any(|s| s.notes.colors == merged.colors)
    {
        conflicts.push(Field::Colors);
    }
    if !conflicts.is_empty() {
        conflicts.sort();
        let conflict = Conflict {
            fields: conflicts,
            sources: named(sources),
            modified: modified(sources),
            before: Some(before.clone()),
            merged,
        };
        return (before, Outcome::Conflict(conflict), None);
    }
    let remembered = Remembered::agreed(&merged, sources);
    let outcome = if merged == before {
        Outcome::Same
    } else {
        from.sort();
        Outcome::Merged { from }
    };
    let keep = (remembered != *memory).then_some(remembered);
    (merged, outcome, keep)
}

/// Champs sur lesquels `notes` ne s'accordent pas.
fn disagreeing<'a>(notes: impl Iterator<Item = &'a Notes> + Clone) -> Vec<Field> {
    let mut fields = Vec::new();
    for atom in Atom::all() {
        let mut values = notes.clone().map(|n| atom.of(n));
        if let Some(first) = values.next()
            && values.any(|v| v != first)
            && !fields.contains(&atom.field())
        {
            fields.push(atom.field());
        }
    }
    fields
}

/// Dates de modification des sources qui parlent, dans l'ordre de [`named`].
fn modified(sources: &[Source]) -> Vec<u64> {
    sources
        .iter()
        .filter(|s| !s.silent)
        .map(|s| s.stamp.modified)
        .collect()
}

fn named(sources: &[Source]) -> Vec<(String, Notes)> {
    sources
        .iter()
        .filter(|s| !s.silent)
        .map(|s| (s.name.clone(), s.notes.clone()))
        .collect()
}

// --- Mémoire d'un dossier ------------------------------------------------------------------

/// Dossier caché de `tri` dans un dossier de photos.
pub const DIR: &str = crate::store::DIR;
/// Fichier de la mémoire des notes, dans [`DIR`].
pub const FILE: &str = crate::store::NOTES;

/// Contenu de `.tri/notes.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Document {
    version: u32,
    /// Réglage propre au dossier : notes inscrites dans les images ou non, prioritaire
    /// sur `[xmp] embed` ([`Memory::embed`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    embed: Option<bool>,
    #[serde(default)]
    images: BTreeMap<String, Remembered>,
}

/// Ce que `tri` retient des notes des images d'un dossier : un fichier des données du
/// dossier ([`crate::store`] : son `.tri/` si l'interrupteur le couvre, sinon le dossier de
/// données de `tri`).
#[derive(Debug)]
pub struct Memory {
    dir: PathBuf,
    images: BTreeMap<String, Remembered>,
    embed: Option<bool>,
    data: Option<PathBuf>,
    changed: bool,
}

/// Dossier de données de `tri`, donné une fois au démarrage (voir
/// [`crate::store::set_data_dir`]).
pub fn set_data_dir(dir: Option<PathBuf>) {
    crate::store::set_data_dir(dir);
}

impl Memory {
    /// Mémoire du dossier `dir`, avec le dossier de données de [`set_data_dir`].
    pub fn open(dir: &Path) -> Memory {
        Memory::load(dir, crate::store::data_dir().as_deref())
    }

    /// Mémoire du dossier `dir` ; `data` : dossier de données de `tri`, pour le repli.
    /// Absente ou illisible : vide (rien n'est retenu ; les sources qui ne s'accordent pas
    /// seront signalées).
    pub fn load(dir: &Path, data: Option<&Path>) -> Memory {
        let document = crate::store::read(data, dir, FILE)
            .and_then(|bytes| serde_json::from_slice::<Document>(&bytes).ok())
            .unwrap_or_default();
        Memory {
            dir: dir.to_path_buf(),
            images: document.images,
            embed: document.embed,
            data: data.map(Path::to_path_buf),
            changed: false,
        }
    }

    /// Réglage propre au dossier (décidé le 07/11/2026 : « ce support amovible-là,
    /// oui ») : `Some(true)` toujours inscrire les notes dans les images, `Some(false)`
    /// jamais, `None` selon `[xmp] embed`. Le dossier seul, pas ses sous-dossiers.
    pub fn embed(&self) -> Option<bool> {
        self.embed
    }

    /// Change le réglage propre au dossier ([`Memory::embed`]).
    pub fn set_embed(&mut self, embed: Option<bool>) {
        if self.embed != embed {
            self.embed = embed;
            self.changed = true;
        }
    }

    /// Ce qu'on retient de l'image `name`.
    pub fn get(&self, name: &str) -> Option<&Remembered> {
        self.images.get(name)
    }

    /// Retient `remembered` pour l'image `name` (`None` : l'oublie).
    pub fn set(&mut self, name: &str, remembered: Option<Remembered>) {
        let changed = match remembered {
            Some(r) if self.images.get(name) == Some(&r) => false,
            Some(r) => {
                self.images.insert(name.to_string(), r);
                true
            }
            None => self.images.remove(name).is_some(),
        };
        self.changed |= changed;
    }

    /// Écrit la mémoire si elle a changé ([`crate::store::write`]). Vide : le fichier (et
    /// `.tri/` s'il est vide) disparaît.
    pub fn save(&mut self) -> io::Result<()> {
        if !self.changed {
            return Ok(());
        }
        let data = self.data.as_deref();
        if self.images.is_empty() && self.embed.is_none() {
            crate::store::write(data, &self.dir, FILE, None)?;
            self.changed = false;
            return Ok(());
        }
        let document = Document {
            version: 1,
            embed: self.embed,
            images: std::mem::take(&mut self.images),
        };
        let json = serde_json::to_vec_pretty(&document).map_err(io::Error::other);
        self.images = document.images;
        crate::store::write(data, &self.dir, FILE, Some(&json?))?;
        self.changed = false;
        Ok(())
    }
}

/// Fichiers renommés, déplacés ou copiés (`files`, (ancien, nouveau), images et
/// sidecars) : ce qu'on retenait de chaque image suit, sous son nouveau nom et dans son
/// nouveau dossier, ses sources renommées comme elles ; `copy` : l'original le garde
/// aussi. Au mieux : une mémoire illisible ou impossible à écrire est laissée.
pub fn carry(files: &[(PathBuf, PathBuf)], copy: bool) {
    let renamed: std::collections::HashMap<&Path, &Path> = files
        .iter()
        .map(|(a, b)| (a.as_path(), b.as_path()))
        .collect();
    // Une mémoire par dossier touché, ouverte une fois.
    let mut memories: BTreeMap<PathBuf, Memory> = BTreeMap::new();
    let dir = |p: &Path| p.parent().unwrap_or(Path::new(".")).to_path_buf();
    let name = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    for (from, to) in files {
        let (from_dir, to_dir) = (dir(from), dir(to));
        let from_memory = memories
            .entry(from_dir.clone())
            .or_insert_with(|| Memory::open(&from_dir));
        let Some(entry) = from_memory.get(&name(from)).cloned() else {
            continue;
        };
        if !copy {
            from_memory.set(&name(from), None);
        }
        let mut moved = entry;
        moved.sources = std::mem::take(&mut moved.sources)
            .into_iter()
            .map(|(source, seen)| {
                let old = from_dir.join(&source);
                let new = renamed.get(old.as_path()).map_or(source, |n| name(n));
                (new, seen)
            })
            .collect();
        memories
            .entry(to_dir.clone())
            .or_insert_with(|| Memory::open(&to_dir))
            .set(&name(to), Some(moved));
    }
    for memory in memories.values_mut() {
        let _ = memory.save();
    }
}

/// Images supprimées : on oublie ce qu'on en retenait.
pub fn forget(images: &[PathBuf]) {
    let mut memories: BTreeMap<PathBuf, Memory> = BTreeMap::new();
    for image in images {
        let dir = image.parent().unwrap_or(Path::new(".")).to_path_buf();
        let name = image
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        memories
            .entry(dir.clone())
            .or_insert_with(|| Memory::open(&dir))
            .set(&name, None);
    }
    for memory in memories.values_mut() {
        let _ = memory.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notes(stars: u8, flag: Flag, colors: &[Color]) -> Notes {
        let mut n = Notes::default();
        n.set_stars(stars);
        n.flag = flag;
        n.colors = colors.to_vec();
        n
    }

    fn source(name: &str, time: u64, n: &Notes) -> Source {
        Source {
            name: name.into(),
            stamp: Stamp {
                modified: time,
                size: 100,
            },
            notes: n.clone(),
            silent: false,
        }
    }

    /// Mémoire où les deux sources s'accordaient sur `n` (dates 1).
    fn agreed(n: &Notes) -> Remembered {
        Remembered::agreed(n, &[source("a.xmp", 1, n), source("a.rrdata", 1, n)])
    }

    #[test]
    fn nothing_changed_nothing_happens() {
        let n = notes(3, Flag::None, &[Color::Red]);
        let memory = agreed(&n);
        let (shown, outcome, keep) = reconcile(
            Some(&memory),
            &[source("a.xmp", 1, &n), source("a.rrdata", 1, &n)],
        );
        assert_eq!((shown, outcome, keep), (n, Outcome::Same, None));
    }

    #[test]
    fn changes_from_different_sources_are_merged() {
        // darktable change les étoiles, RapidRAW la couleur : les deux sont gardés.
        let base = notes(3, Flag::None, &[Color::Red]);
        let memory = agreed(&base);
        let dt = notes(5, Flag::None, &[Color::Red]);
        let rr = notes(3, Flag::None, &[Color::Blue]);
        let (shown, outcome, keep) = reconcile(
            Some(&memory),
            &[source("a.xmp", 2, &dt), source("a.rrdata", 2, &rr)],
        );
        assert_eq!(shown, notes(5, Flag::None, &[Color::Blue]));
        assert_eq!(
            outcome,
            Outcome::Merged {
                from: vec!["a.rrdata".into(), "a.xmp".into()]
            }
        );
        let keep = keep.unwrap();
        assert_eq!(keep.notes(), shown);
        // Relu tel quel ensuite : plus rien de neuf (les sources en retard le restent).
        let again = reconcile(
            Some(&keep),
            &[source("a.xmp", 2, &dt), source("a.rrdata", 2, &rr)],
        );
        assert_eq!(again, (shown, Outcome::Same, None));
    }

    #[test]
    fn the_same_field_changed_two_ways_is_a_conflict() {
        let base = notes(3, Flag::None, &[]);
        let memory = agreed(&base);
        let (shown, outcome, keep) = reconcile(
            Some(&memory),
            &[
                source("a.xmp", 2, &notes(5, Flag::None, &[])),
                source("a.rrdata", 2, &notes(1, Flag::Accepted, &[])),
            ],
        );
        assert_eq!(shown, base, "les notes d'avant, en attendant");
        assert!(keep.is_none(), "rien n'est retenu");
        let Outcome::Conflict(c) = outcome else {
            panic!("conflit attendu")
        };
        assert_eq!(c.fields, [Field::Stars], "le drapeau, lui, se fusionnerait");
        assert_eq!(
            c.merged.flag,
            Flag::Accepted,
            "déjà fusionné, pour le choix"
        );
        assert_eq!(c.merged.stars, 3, "en désaccord : celles d'avant");
        assert_eq!(c.before, Some(base));
        assert_eq!(c.sources.len(), 2);
        assert_eq!(c.modified.len(), 2, "une date par source");
    }

    #[test]
    fn the_same_change_on_both_sides_is_no_conflict() {
        // C'est ce qui arrive après une note posée dans `tri` (toutes les sources écrites).
        let base = notes(3, Flag::None, &[]);
        let memory = agreed(&base);
        let n = notes(4, Flag::Accepted, &[Color::Green]);
        let (shown, outcome, _) = reconcile(
            Some(&memory),
            &[source("a.xmp", 2, &n), source("a.rrdata", 2, &n)],
        );
        assert_eq!(shown, n);
        assert!(matches!(outcome, Outcome::Merged { .. }));
    }

    #[test]
    fn a_touched_file_with_the_same_notes_changes_nothing() {
        // Un JPEG retouché ailleurs (date changée) sans toucher à ses notes, restées en
        // retard sur le sidecar : il n'apporte rien.
        let old = notes(2, Flag::None, &[]);
        let base = notes(4, Flag::None, &[]);
        let memory = Remembered::agreed(
            &base,
            &[source("a.xmp", 1, &base), source("a.jpg", 1, &old)],
        );
        let mut memory = memory;
        memory.notes = Stored::from(&base);
        let (shown, outcome, _) = reconcile(
            Some(&memory),
            &[source("a.xmp", 1, &base), source("a.jpg", 9, &old)],
        );
        assert_eq!((shown, outcome), (base, Outcome::Same));
    }

    #[test]
    fn unknown_sources_that_disagree_are_asked_about() {
        // Rien de retenu, sources en désaccord : on demande (les plus récentes montrées).
        let (shown, outcome, keep) = reconcile(
            None,
            &[
                source("a.xmp", 1, &notes(2, Flag::None, &[])),
                source("a.rrdata", 5, &notes(4, Flag::None, &[])),
            ],
        );
        assert_eq!(shown.stars, 4);
        assert!(matches!(outcome, Outcome::Conflict(ref c) if c.fields == [Field::Stars]));
        // À égalité de dates : la première source (un sidecar avant l'image).
        let (shown, _, _) = reconcile(
            None,
            &[
                source("a.xmp", 5, &notes(2, Flag::None, &[])),
                source("a.jpg", 5, &notes(4, Flag::None, &[])),
            ],
        );
        assert_eq!(shown.stars, 2);
        assert!(keep.is_none());
        // D'accord : retenues sans rien demander.
        let n = notes(2, Flag::None, &[]);
        let (_, outcome, keep) =
            reconcile(None, &[source("a.xmp", 1, &n), source("a.rrdata", 5, &n)]);
        assert_eq!(outcome, Outcome::Same);
        assert!(keep.is_some());
        // Un nouveau sidecar (créé par un autre logiciel) en désaccord : on demande.
        let base = notes(3, Flag::None, &[]);
        let memory = Remembered::agreed(&base, &[source("a.xmp", 1, &base)]);
        let (_, outcome, _) = reconcile(
            Some(&memory),
            &[
                source("a.xmp", 1, &base),
                source("a.pp3", 7, &notes(0, Flag::None, &[])),
            ],
        );
        assert!(matches!(outcome, Outcome::Conflict(_)));
        // D'accord avec les notes retenues : simplement ajouté.
        let (_, outcome, keep) = reconcile(
            Some(&memory),
            &[source("a.xmp", 1, &base), source("a.pp3", 7, &base)],
        );
        assert_eq!(outcome, Outcome::Same);
        assert!(keep.unwrap().sources.contains_key("a.pp3"));
    }

    #[test]
    fn colours_no_source_carries_are_not_invented() {
        // Chacun ajoute sa couleur : la fusion donnerait rouge et vert, qu'aucune source
        // n'a ; on demande.
        let base = notes(0, Flag::None, &[]);
        let memory = agreed(&base);
        let (_, outcome, _) = reconcile(
            Some(&memory),
            &[
                source("a.xmp", 2, &notes(0, Flag::None, &[Color::Red])),
                source("a.rrdata", 2, &notes(0, Flag::None, &[Color::Green])),
            ],
        );
        assert!(matches!(outcome, Outcome::Conflict(ref c) if c.fields == [Field::Colors]));
        // Une couleur changée d'un côté (rouge → bleu) : fusionnée.
        let base = notes(0, Flag::None, &[Color::Red]);
        let memory = agreed(&base);
        let (shown, _, _) = reconcile(
            Some(&memory),
            &[
                source("a.xmp", 2, &notes(0, Flag::None, &[Color::Blue])),
                source("a.rrdata", 1, &base),
            ],
        );
        assert_eq!(shown.colors, [Color::Blue]);
    }

    #[test]
    fn notes_appearing_in_a_silent_image_are_a_known_change() {
        // Un JPEG sans notes à côté de son sidecar : retenu muet ; noté ensuite ailleurs
        // (5 étoiles) pendant que le sidecar prend du rouge : les deux fusionnés.
        let base = notes(3, Flag::None, &[]);
        let mut image = source("a.jpg", 1, &Notes::default());
        image.silent = true;
        let (_, outcome, keep) = reconcile(None, &[source("a.jpg.xmp", 1, &base), image]);
        assert_eq!(outcome, Outcome::Same, "une image muette ne contredit rien");
        let memory = keep.unwrap();
        let (shown, outcome, _) = reconcile(
            Some(&memory),
            &[
                source("a.jpg.xmp", 2, &notes(3, Flag::None, &[Color::Red])),
                source("a.jpg", 2, &notes(5, Flag::None, &[])),
            ],
        );
        assert_eq!(shown, notes(5, Flag::None, &[Color::Red]));
        assert!(matches!(outcome, Outcome::Merged { .. }));
        // Que des sources muettes : rien à retenir.
        let mut only = source("b.jpg", 1, &Notes::default());
        only.silent = true;
        assert_eq!(reconcile(None, &[only]).2, None);
    }

    #[test]
    fn a_deleted_source_is_forgotten() {
        let n = notes(3, Flag::None, &[]);
        let memory = agreed(&n);
        let (shown, outcome, keep) = reconcile(Some(&memory), &[source("a.xmp", 1, &n)]);
        assert_eq!((shown, outcome), (n, Outcome::Same));
        assert!(!keep.unwrap().sources.contains_key("a.rrdata"));
    }

    #[test]
    fn the_memory_follows_the_switch_and_vanishes_when_empty() {
        // Issue n° 378 : dans le dossier de données par défaut, à côté des photos quand
        // l'interrupteur « garder les données de tri ici » est posé.
        let dir = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let n = notes(3, Flag::Accepted, &[Color::Red]);
        let mut memory = Memory::load(dir.path(), Some(data.path()));
        assert!(memory.get("a.jpg").is_none());
        memory.set(
            "a.jpg",
            Some(Remembered::agreed(&n, &[source("a.jpg.xmp", 1, &n)])),
        );
        memory.save().unwrap();
        let file = dir.path().join(DIR).join(FILE);
        assert!(!file.exists(), "rien à côté des photos par défaut");
        let again = Memory::load(dir.path(), Some(data.path()));
        assert_eq!(again.get("a.jpg").unwrap().notes(), n);
        crate::store::keep(Some(data.path()), dir.path()).unwrap();
        assert!(file.is_file(), "passée dans le dossier");
        let mut again = Memory::load(dir.path(), Some(data.path()));
        assert_eq!(again.get("a.jpg").unwrap().notes(), n);
        again.set("a.jpg", None);
        again.save().unwrap();
        assert!(!file.exists(), "vide : retirée");
        crate::store::release(Some(data.path()), dir.path()).unwrap();
        assert!(!dir.path().join(DIR).exists(), "plus de `.tri/`");
    }

    #[test]
    fn the_memory_follows_renamed_moved_copied_and_deleted_images() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let (s, d) = (src.path(), dest.path());
        let n = notes(3, Flag::None, &[]);
        let mut memory = Memory::load(s, None);
        let sources = [source("A.jpg.xmp", 1, &n), source("A.jpg", 1, &n)];
        memory.set("A.jpg", Some(Remembered::agreed(&n, &sources)));
        memory.save().unwrap();
        // Renommé (sidecar compris) : la clé et les sources suivent.
        carry(
            &[
                (s.join("A.jpg"), s.join("B.jpg")),
                (s.join("A.jpg.xmp"), s.join("B.jpg.xmp")),
            ],
            false,
        );
        let memory = Memory::load(s, None);
        assert!(memory.get("A.jpg").is_none());
        let b = memory.get("B.jpg").unwrap();
        assert!(b.sources.contains_key("B.jpg.xmp") && b.sources.contains_key("B.jpg"));
        // Copié ailleurs : les deux l'ont.
        carry(&[(s.join("B.jpg"), d.join("B.jpg"))], true);
        assert!(Memory::load(s, None).get("B.jpg").is_some());
        assert_eq!(Memory::load(d, None).get("B.jpg").unwrap().notes(), n);
        // Supprimé : oublié (et le fichier, vide, retiré).
        forget(&[s.join("B.jpg")]);
        assert!(!s.join(DIR).exists());
    }

    #[test]
    fn a_folder_setting_is_kept_even_without_images() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = Memory::load(dir.path(), None);
        assert_eq!(memory.embed(), None);
        memory.set_embed(Some(true));
        memory.save().unwrap();
        assert_eq!(Memory::load(dir.path(), None).embed(), Some(true));
        let mut memory = Memory::load(dir.path(), None);
        memory.set_embed(None);
        memory.save().unwrap();
        assert!(!dir.path().join(DIR).exists(), "plus rien : retiré");
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_folder_falls_back_to_the_data_folder() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        let n = notes(2, Flag::None, &[]);
        let mut memory = Memory::load(dir.path(), Some(data.path()));
        memory.set(
            "a.jpg",
            Some(Remembered::agreed(&n, &[source("a.jpg", 1, &n)])),
        );
        let saved = memory.save();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        saved.unwrap();
        assert!(!dir.path().join(DIR).exists());
        assert_eq!(
            Memory::load(dir.path(), Some(data.path()))
                .get("a.jpg")
                .unwrap()
                .notes(),
            n
        );
    }
}
