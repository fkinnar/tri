//! Notes de tri (étoiles, couleurs, accepté / rejeté) dans les sidecars XMP.
//!
//! Principe : ne toucher qu'à nos champs. Le XMP est relu événement par événement et
//! réécrit à l'identique (espaces, commentaires, historique darktable, mots-clés…), sauf
//! les champs de notation. Les champs sont reconnus par leur espace de noms, pas par leur
//! préfixe (Bridge écrit `xap:` au lieu de `xmp:`), sous forme d'attribut
//! (`xmp:Rating="3"`) comme d'élément (`<xmp:Rating>3</xmp:Rating>`). Un sidecar mal formé
//! n'est jamais réécrit. Voir `doc/xmp.md`.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::{Writer, XmlVersion};

/// Espaces de noms utilisés.
pub mod ns {
    /// RDF.
    pub const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
    /// XMP de base : `Rating`, `Label`.
    pub const XMP: &str = "http://ns.adobe.com/xap/1.0/";
    /// Adobe Dynamic Media : `pick`, `good` (drapeau de Lightroom Classic ≥ 13.2).
    pub const XMP_DM: &str = "http://ns.adobe.com/xmp/1.0/DynamicMedia/";
    /// digiKam : `PickLabel`, `ColorLabel`.
    pub const DIGIKAM: &str = "http://www.digikam.org/ns/1.0/";
    /// darktable : `colorlabels`.
    pub const DARKTABLE: &str = "http://darktable.sf.net/";
    /// tri : étoiles d'une photo rejetée (`RatingBeforeReject`).
    pub const TRI: &str = "urn:tri:xmp:1.0";
}

/// Drapeau d'une photo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Flag {
    /// Ni accepté ni rejeté.
    #[default]
    None,
    /// Accepté (gardé).
    Accepted,
    /// Rejeté.
    Rejected,
}

/// Couleur d'une photo (les cinq couleurs communes à Lightroom, darktable et Capture One).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Color {
    /// Rouge.
    Red,
    /// Jaune.
    Yellow,
    /// Vert.
    Green,
    /// Bleu.
    Blue,
    /// Violet.
    Purple,
}

impl Color {
    /// Toutes les couleurs, dans l'ordre de darktable (0 = rouge … 4 = violet).
    pub const ALL: [Color; 5] = [
        Color::Red,
        Color::Yellow,
        Color::Green,
        Color::Blue,
        Color::Purple,
    ];

    /// Nom dans `xmp:Label` (Lightroom, Capture One, darktable en lecture).
    pub fn label(self) -> &'static str {
        match self {
            Color::Red => "Red",
            Color::Yellow => "Yellow",
            Color::Green => "Green",
            Color::Blue => "Blue",
            Color::Purple => "Purple",
        }
    }

    /// Numéro dans `darktable:colorlabels`.
    pub fn darktable(self) -> u8 {
        self as u8
    }

    /// Numéro dans `digiKam:ColorLabel` (1 rouge, 3 jaune, 4 vert, 5 bleu, 6 magenta).
    pub fn digikam(self) -> u8 {
        match self {
            Color::Red => 1,
            Color::Yellow => 3,
            Color::Green => 4,
            Color::Blue => 5,
            Color::Purple => 6,
        }
    }

    fn from_label(s: &str) -> Option<Color> {
        Color::ALL
            .into_iter()
            .find(|c| c.label().eq_ignore_ascii_case(s.trim()))
    }

    fn from_darktable(n: i64) -> Option<Color> {
        Color::ALL
            .into_iter()
            .find(|c| i64::from(c.darktable()) == n)
    }

    fn from_digikam(n: i64) -> Option<Color> {
        Color::ALL.into_iter().find(|c| i64::from(c.digikam()) == n)
    }
}

/// Notes d'une photo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notes {
    /// Étoiles, 0 à 5. Conservées quand la photo est rejetée.
    pub stars: u8,
    /// Accepté, rejeté ou rien.
    pub flag: Flag,
    /// Couleurs, triées, sans doublon (une seule par défaut ; plusieurs si l'interface le
    /// permet).
    pub colors: Vec<Color>,
}

impl Notes {
    /// Notes vides (constante, pour les prêter sans allocation).
    pub const EMPTY: Notes = Notes {
        stars: 0,
        flag: Flag::None,
        colors: Vec::new(),
    };

    /// Change les étoiles (0 à 5, au-delà ramené à 5). Le drapeau ne change pas : une photo
    /// rejetée garde ses étoiles pour le jour où le rejet est annulé.
    pub fn set_stars(&mut self, stars: u8) {
        self.stars = stars.min(5);
    }

    /// Ajoute ou retire `color`. Avec `multiple` faux, une photo n'a qu'une couleur : une
    /// autre couleur remplace la précédente, la même la retire (comme Lightroom).
    pub fn toggle_color(&mut self, color: Color, multiple: bool) {
        if let Some(i) = self.colors.iter().position(|&c| c == color) {
            self.colors.remove(i);
        } else if multiple {
            self.colors.push(color);
            self.colors.sort();
        } else {
            self.colors = vec![color];
        }
    }

    /// Retire toutes les couleurs.
    pub fn clear_colors(&mut self) {
        self.colors.clear();
    }

    /// Vrai si la photo porte la couleur `color`.
    pub fn has_color(&self, color: Color) -> bool {
        self.colors.contains(&color)
    }
}

/// Erreurs du module xmp.
#[derive(Debug)]
pub enum XmpError {
    /// Lecture ou écriture du fichier.
    Io(io::Error),
    /// XML mal formé : le sidecar n'est pas modifié.
    Malformed(String),
    /// Aucun `rdf:Description` : pas un paquet XMP utilisable, le sidecar n'est pas modifié.
    NoDescription,
    /// Erreur déjà décrite, rapportée pour une image précise (notes d'un dossier).
    Image {
        /// Image concernée.
        image: PathBuf,
        /// Description de l'erreur.
        message: String,
    },
}

impl fmt::Display for XmpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "sidecar XMP : {e}"),
            Self::Malformed(e) => write!(f, "sidecar XMP mal formé (non modifié) : {e}"),
            Self::NoDescription => write!(f, "sidecar XMP sans rdf:Description (non modifié)"),
            Self::Image { image, message } => write!(f, "{} : {message}", image.display()),
        }
    }
}

impl std::error::Error for XmpError {}

impl From<io::Error> for XmpError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<quick_xml::Error> for XmpError {
    fn from(e: quick_xml::Error) -> Self {
        Self::Malformed(e.to_string())
    }
}

/// `Result` spécialisé pour le module xmp.
pub type Result<T> = std::result::Result<T, XmpError>;

/// Convention de nommage des sidecars.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Convention {
    /// `IMAGE.RAF.xmp` : darktable, digiKam. Un sidecar par fichier.
    #[default]
    Darktable,
    /// `IMAGE.xmp` : Lightroom, Capture One, Bridge. Partagé par les fichiers de même nom
    /// (le RAF et le JPG d'une paire).
    Adobe,
}

impl Convention {
    /// Chemin du sidecar de `image` selon cette convention.
    pub fn sidecar(self, image: &Path) -> PathBuf {
        match self {
            Convention::Darktable => {
                let mut name = image.as_os_str().to_owned();
                name.push(".xmp");
                PathBuf::from(name)
            }
            Convention::Adobe => image.with_extension("xmp"),
        }
    }

    /// Convention à laquelle correspond le sidecar `sidecar` de `image`.
    pub fn of(image: &Path, sidecar: &Path) -> Convention {
        let darktable = Convention::Darktable.sidecar(image);
        let same = |a: &Path, b: &Path| {
            a.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
        };
        if same(sidecar, &darktable) {
            Convention::Darktable
        } else {
            Convention::Adobe
        }
    }
}

/// Fichiers `.xmp` d'un dossier, indexés par nom en minuscules : une seule lecture du
/// dossier pour tout un dossier d'images, et les vrais noms du disque, quelle que soit la
/// casse (sur macOS, `IMG.XMP` et `IMG.xmp` désignent le même fichier ; sur Linux, deux).
#[derive(Clone, Debug, Default)]
pub struct SidecarIndex {
    by_lowercase: std::collections::HashMap<String, Vec<PathBuf>>,
}

impl SidecarIndex {
    /// Index des sidecars du dossier `dir` (vide si illisible).
    pub fn new(dir: &Path) -> SidecarIndex {
        let mut by_lowercase: std::collections::HashMap<String, Vec<PathBuf>> =
            std::collections::HashMap::new();
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.ends_with(".xmp") {
                by_lowercase.entry(name).or_default().push(entry.path());
            }
        }
        SidecarIndex { by_lowercase }
    }

    /// Sidecars existants de `image` (dans ce dossier), des deux conventions.
    pub fn sidecars(&self, image: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for convention in [Convention::Darktable, Convention::Adobe] {
            let wanted = convention.sidecar(image);
            let Some(name) = wanted.file_name() else {
                continue;
            };
            let key = name.to_string_lossy().to_lowercase();
            for path in self.by_lowercase.get(&key).into_iter().flatten() {
                if !found.contains(path) {
                    found.push(path.clone());
                }
            }
        }
        found
    }

    /// Le sidecar à lire et à écrire pour `image` : le plus récent des existants, sinon
    /// celui de la convention `default`.
    pub fn sidecar_path(&self, image: &Path, default: Convention) -> PathBuf {
        self.sidecars(image)
            .into_iter()
            .max_by_key(|p| {
                fs::metadata(p)
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH)
            })
            .unwrap_or_else(|| default.sidecar(image))
    }
}

fn index_for(image: &Path) -> SidecarIndex {
    SidecarIndex::new(image.parent().unwrap_or(Path::new(".")))
}

/// Sidecars existants de `image`, des deux conventions, quelle que soit la casse de leur
/// nom. Relit le dossier : pour tout un dossier, préférer [`SidecarIndex`].
pub fn existing_sidecars(image: &Path) -> Vec<PathBuf> {
    index_for(image).sidecars(image)
}

/// Le sidecar à lire et à écrire : le plus récent des sidecars existants, sinon celui de la
/// convention `default`. Relit le dossier : pour tout un dossier, préférer
/// [`SidecarIndex`].
pub fn sidecar_path(image: &Path, default: Convention) -> PathBuf {
    index_for(image).sidecar_path(image, default)
}

/// Notes de `image`, lues dans son sidecar le plus récent ; notes vides sans sidecar.
pub fn read(image: &Path) -> Result<Notes> {
    read_sidecar(&sidecar_path(image, Convention::default()))
}

/// Notes lues dans le sidecar `path` ; notes vides s'il n'existe pas.
pub fn read_sidecar(path: &Path) -> Result<Notes> {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Notes::default()),
        Err(e) => Err(e.into()),
    }
}

/// Écrit les notes de `image` dans son sidecar le plus récent (créé selon `default` s'il
/// n'existe pas). Le fichier est relu juste avant, seuls nos champs changent, et il n'est
/// réécrit que si son contenu change (écriture atomique). Renvoie le sidecar utilisé.
pub fn write(image: &Path, notes: &Notes, default: Convention) -> Result<PathBuf> {
    let path = sidecar_path(image, default);
    write_sidecar(&path, notes)?;
    Ok(path)
}

/// Écrit `notes` dans le sidecar `path` (créé s'il n'existe pas), comme [`write()`]. Renvoie
/// vrai si le fichier a été modifié.
pub fn write_sidecar(path: &Path, notes: &Notes) -> Result<bool> {
    let original = match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let base = original.as_deref().unwrap_or(NEW_PACKET);
    let updated = apply(base, notes)?;
    if original.as_deref() == Some(updated.as_str()) {
        return Ok(false);
    }
    let tmp = path.with_extension(format!("xmp.{}.tmp", std::process::id()));
    fs::write(&tmp, &updated)?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(true)
}

/// Bilan d'une conversion de convention ([`convert`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Conversion {
    /// Sidecars écrits (ou à écrire, en simulation).
    pub written: Vec<PathBuf>,
    /// Images déjà à jour dans la convention voulue, ou sans notes ni sidecar.
    pub unchanged: usize,
    /// Images dont les notes diffèrent de celles déjà écrites dans le même sidecar cible
    /// (convention Adobe : `IMG.RAF` et `IMG.JPG` notés différemment) : la première image
    /// (le RAW, dans l'ordre de `media::scan`) l'emporte, celle-ci est ignorée.
    pub conflicts: Vec<PathBuf>,
    /// Images dont le sidecar n'a pas pu être lu ou écrit.
    pub errors: Vec<(PathBuf, String)>,
}

/// Recopie les notes de chaque image, lues dans son sidecar le plus récent, dans le sidecar
/// de la convention `target` (créé s'il le faut ; s'il existe, seuls nos champs changent).
/// **Aucun sidecar n'est supprimé** : celui de darktable garde l'historique de
/// développement. Après conversion, le sidecar cible est le plus récent, donc celui que
/// `tri` lit et écrit. `dry_run` : rien n'est écrit, le bilan dit ce qui le serait.
pub fn convert<'a>(
    images: impl IntoIterator<Item = &'a Path>,
    target: Convention,
    dry_run: bool,
) -> Conversion {
    let mut report = Conversion::default();
    let mut indexes: std::collections::HashMap<PathBuf, SidecarIndex> =
        std::collections::HashMap::new();
    let mut done: std::collections::HashMap<PathBuf, Notes> = std::collections::HashMap::new();
    for image in images {
        let dir = image.parent().unwrap_or(Path::new(".")).to_path_buf();
        let index = indexes
            .entry(dir.clone())
            .or_insert_with(|| SidecarIndex::new(&dir));
        let existing = index.sidecars(image);
        if existing.is_empty() {
            report.unchanged += 1;
            continue;
        }
        let source = index.sidecar_path(image, target);
        let destination = existing
            .iter()
            .find(|p| Convention::of(image, p) == target)
            .cloned()
            .unwrap_or_else(|| target.sidecar(image));
        let notes = match read_sidecar(&source) {
            Ok(notes) => notes,
            Err(e) => {
                report.errors.push((image.to_path_buf(), e.to_string()));
                continue;
            }
        };
        if let Some(previous) = done.get(&destination) {
            if *previous != notes {
                report.conflicts.push(image.to_path_buf());
            } else {
                report.unchanged += 1;
            }
            continue;
        }
        done.insert(destination.clone(), notes.clone());
        let current = if destination.exists() {
            read_sidecar(&destination)
        } else {
            Ok(Notes::default())
        };
        match current {
            Ok(current) if current == notes && (destination.exists() || notes == Notes::EMPTY) => {
                // Déjà à jour, mais il doit devenir le plus récent pour être celui qu'on
                // écrit ensuite : seule sa date change.
                if destination != source && destination.exists() && !dry_run {
                    let touched = fs::File::options()
                        .write(true)
                        .open(&destination)
                        .and_then(|f| f.set_modified(SystemTime::now()));
                    if let Err(e) = touched {
                        report.errors.push((image.to_path_buf(), e.to_string()));
                        continue;
                    }
                }
                report.unchanged += 1;
            }
            Ok(_) if dry_run => report.written.push(destination),
            Ok(_) => match write_sidecar(&destination, &notes) {
                Ok(_) => report.written.push(destination),
                Err(e) => report.errors.push((image.to_path_buf(), e.to_string())),
            },
            Err(e) => report.errors.push((image.to_path_buf(), e.to_string())),
        }
    }
    report
}

/// Paquet XMP minimal d'un nouveau sidecar.
const NEW_PACKET: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="tri">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/">
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
"#;

/// Nos champs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Rating,
    Label,
    Pick,
    Good,
    PickLabel,
    DigikamColor,
    DarktableColors,
    StarsBeforeReject,
}

impl Field {
    const ALL: [Field; 8] = [
        Field::Rating,
        Field::Label,
        Field::Pick,
        Field::Good,
        Field::PickLabel,
        Field::DigikamColor,
        Field::DarktableColors,
        Field::StarsBeforeReject,
    ];

    /// Espace de noms, nom local, préfixe usuel.
    fn name(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Field::Rating => (ns::XMP, "Rating", "xmp"),
            Field::Label => (ns::XMP, "Label", "xmp"),
            Field::Pick => (ns::XMP_DM, "pick", "xmpDM"),
            Field::Good => (ns::XMP_DM, "good", "xmpDM"),
            Field::PickLabel => (ns::DIGIKAM, "PickLabel", "digiKam"),
            Field::DigikamColor => (ns::DIGIKAM, "ColorLabel", "digiKam"),
            Field::DarktableColors => (ns::DARKTABLE, "colorlabels", "darktable"),
            Field::StarsBeforeReject => (ns::TRI, "RatingBeforeReject", "tri"),
        }
    }

    fn find(uri: &str, local: &str) -> Option<Field> {
        Field::ALL.into_iter().find(|f| {
            let (u, l, _) = f.name();
            u == uri && l == local
        })
    }
}

/// Valeurs brutes de nos champs trouvées dans un paquet.
#[derive(Default, Debug)]
struct Raw {
    values: Vec<(Field, String)>,
    darktable_colors: Option<Vec<String>>,
}

impl Raw {
    fn get(&self, field: Field) -> Option<&str> {
        self.values
            .iter()
            .find(|(f, _)| *f == field)
            .map(|(_, v)| v.as_str())
    }

    fn has(&self, field: Field) -> bool {
        match field {
            Field::DarktableColors => self.darktable_colors.is_some(),
            f => self.get(f).is_some(),
        }
    }

    fn int(&self, field: Field) -> Option<i64> {
        // Certains logiciels écrivent « 3.0 ».
        self.get(field)
            .and_then(|v| v.trim().parse::<f64>().ok())
            .map(|v| v.round() as i64)
    }

    fn notes(&self) -> Notes {
        let rating = self.int(Field::Rating).unwrap_or(0);
        let pick = self.int(Field::Pick).unwrap_or(0);
        let pick_label = self.int(Field::PickLabel).unwrap_or(0);
        let flag = if rating == -1 || pick == -1 || pick_label == 1 {
            Flag::Rejected
        } else if pick == 1 || pick_label == 3 {
            Flag::Accepted
        } else {
            Flag::None
        };
        let stars = if rating == -1 {
            self.int(Field::StarsBeforeReject).unwrap_or(0)
        } else {
            rating
        }
        .clamp(0, 5) as u8;
        // Même priorité que darktable : xmp:Label d'abord.
        let mut colors: Vec<Color> =
            if let Some(c) = self.get(Field::Label).and_then(Color::from_label) {
                vec![c]
            } else if let Some(list) = &self.darktable_colors {
                list.iter()
                    .filter_map(|v| v.trim().parse().ok())
                    .filter_map(Color::from_darktable)
                    .collect()
            } else {
                self.int(Field::DigikamColor)
                    .and_then(Color::from_digikam)
                    .into_iter()
                    .collect()
            };
        colors.sort();
        colors.dedup();
        Notes {
            stars,
            flag,
            colors,
        }
    }
}

/// Préfixes d'espaces de noms déclarés (pile de portées).
#[derive(Default)]
struct Scopes(Vec<Vec<(String, String)>>);

impl Scopes {
    fn push(&mut self, start: &BytesStart<'_>) -> Result<()> {
        let mut scope = Vec::new();
        for attr in start.attributes() {
            let attr = attr.map_err(|e| XmpError::Malformed(e.to_string()))?;
            let key = attr.key.as_ref().to_string();
            if let Some(prefix) = key.strip_prefix("xmlns:") {
                let value = attr
                    .normalized_value(XmlVersion::Implicit1_0)
                    .map_err(|e| XmpError::Malformed(e.to_string()))?;
                scope.push((prefix.to_string(), value.into_owned()));
            }
        }
        self.0.push(scope);
        Ok(())
    }

    fn pop(&mut self) {
        self.0.pop();
    }

    fn uri(&self, prefix: &str) -> Option<&str> {
        self.0
            .iter()
            .rev()
            .flat_map(|s| s.iter().rev())
            .find(|(p, _)| p == prefix)
            .map(|(_, u)| u.as_str())
    }

    fn prefix(&self, uri: &str) -> Option<&str> {
        self.0
            .iter()
            .rev()
            .flat_map(|s| s.iter().rev())
            .find(|(p, u)| u == uri && self.uri(p) == Some(uri))
            .map(|(p, _)| p.as_str())
    }

    /// (espace de noms, nom local) d'un nom qualifié `préfixe:local`.
    fn resolve<'a>(&self, qname: &'a str) -> Option<(String, &'a str)> {
        let (prefix, local) = qname.split_once(':')?;
        Some((self.uri(prefix)?.to_string(), local))
    }
}

fn name_of(start: &BytesStart<'_>) -> String {
    start.name().as_ref().to_string()
}

fn is_rdf(scopes: &Scopes, qname: &str, local: &str) -> bool {
    scopes
        .resolve(qname)
        .is_some_and(|(u, l)| u == ns::RDF && l == local)
}

fn new_reader(xml: &str) -> Reader<&[u8]> {
    let mut reader = Reader::from_str(xml);
    let config = reader.config_mut();
    config.trim_text(false);
    config.check_end_names = true;
    reader
}

/// Notes contenues dans un paquet XMP.
pub fn parse(xml: &str) -> Result<Notes> {
    Ok(scan(xml)?.notes())
}

/// Relève nos champs (attributs ou éléments) dans tous les `rdf:Description`.
fn scan(xml: &str) -> Result<Raw> {
    let mut reader = new_reader(xml);
    let mut scopes = Scopes::default();
    let mut raw = Raw::default();
    let mut depth = 0usize;
    // Profondeur des rdf:Description ouverts, champ élément en cours, texte, rdf:li.
    let mut description: Option<usize> = None;
    let mut field: Option<(Field, usize)> = None;
    let mut text = String::new();
    let mut items: Vec<String> = Vec::new();
    let mut found = false;
    loop {
        let event = reader.read_event()?;
        match &event {
            Event::Start(e) | Event::Empty(e) => {
                scopes.push(e)?;
                depth += 1;
                let qname = name_of(e);
                if is_rdf(&scopes, &qname, "Description") {
                    found = true;
                    collect_attributes(e, &scopes, &mut raw)?;
                    if matches!(event, Event::Start(_)) {
                        description = Some(depth);
                    }
                } else if description == Some(depth - 1)
                    && let Some((uri, local)) = scopes.resolve(&qname)
                    && let Some(f) = Field::find(&uri, local)
                {
                    text.clear();
                    items.clear();
                    if matches!(event, Event::Start(_)) {
                        field = Some((f, depth));
                    } else {
                        store(&mut raw, f, String::new(), Vec::new());
                    }
                }
                if matches!(event, Event::Empty(_)) {
                    scopes.pop();
                    depth -= 1;
                }
            }
            Event::Text(t) => {
                if field.is_some() {
                    text.push_str(t);
                }
            }
            Event::GeneralRef(r) => {
                if field.is_some() {
                    if let Some(c) = r.resolve_char_ref().ok().flatten() {
                        text.push(c);
                    } else if let Some(c) = predefined_entity(r) {
                        text.push(c);
                    }
                }
            }
            Event::End(e) => {
                let qname = e.name().as_ref().to_string();
                if let Some((f, d)) = field {
                    if is_rdf(&scopes, &qname, "li") {
                        items.push(std::mem::take(&mut text).trim().to_string());
                    } else if d == depth {
                        store(
                            &mut raw,
                            f,
                            text.trim().to_string(),
                            std::mem::take(&mut items),
                        );
                        field = None;
                    }
                }
                if description == Some(depth) {
                    description = None;
                }
                scopes.pop();
                depth -= 1;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !found {
        return Err(XmpError::NoDescription);
    }
    Ok(raw)
}

/// Les cinq entités prédéfinies de XML (`&amp;` etc.), reçues comme événements séparés.
fn predefined_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => None,
    }
}

fn store(raw: &mut Raw, field: Field, value: String, items: Vec<String>) {
    match field {
        Field::DarktableColors => raw.darktable_colors = Some(items),
        f => raw.values.push((f, value)),
    }
}

fn collect_attributes(start: &BytesStart<'_>, scopes: &Scopes, raw: &mut Raw) -> Result<()> {
    for attr in start.attributes() {
        let attr = attr.map_err(|e| XmpError::Malformed(e.to_string()))?;
        let key = attr.key.as_ref().to_string();
        if let Some((uri, local)) = scopes.resolve(&key)
            && let Some(f) = Field::find(&uri, local)
        {
            let value = attr
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|e| XmpError::Malformed(e.to_string()))?
                .into_owned();
            store(raw, f, value, Vec::new());
        }
    }
    Ok(())
}

/// Ce que nos champs doivent valoir après écriture (`None` : retirer).
fn targets(notes: &Notes, existing: &Raw) -> Vec<(Field, Option<String>)> {
    let rejected = notes.flag == Flag::Rejected;
    // On n'ajoute pas de champ « vide » qui n'existait pas : pas de bruit dans le fichier.
    let keep_zero = |field: Field, value: i64| -> Option<String> {
        (value != 0 || existing.has(field)).then(|| value.to_string())
    };
    let rating = if rejected { -1 } else { i64::from(notes.stars) };
    let (pick, pick_label, good) = match notes.flag {
        Flag::None => (0, 0, None),
        Flag::Accepted => (1, 3, Some("True")),
        Flag::Rejected => (-1, 1, Some("False")),
    };
    let single = (notes.colors.len() == 1).then(|| notes.colors[0]);
    vec![
        (Field::Rating, keep_zero(Field::Rating, rating)),
        (
            Field::StarsBeforeReject,
            (rejected && notes.stars > 0).then(|| notes.stars.to_string()),
        ),
        (Field::Pick, keep_zero(Field::Pick, pick)),
        (Field::Good, good.map(str::to_string)),
        (Field::PickLabel, keep_zero(Field::PickLabel, pick_label)),
        // xmp:Label passe avant darktable:colorlabels chez darktable : seulement s'il n'y a
        // qu'une couleur, sinon darktable n'en verrait qu'une.
        (Field::Label, single.map(|c| c.label().to_string())),
        (Field::DigikamColor, single.map(|c| c.digikam().to_string())),
    ]
}

/// Applique `notes` à un paquet XMP : seuls nos champs changent, tout le reste est recopié
/// tel quel. Nos champs sont écrits en attributs du premier `rdf:Description` (les couleurs
/// darktable, une séquence, en élément) et retirés des autres endroits.
pub fn apply(xml: &str, notes: &Notes) -> Result<String> {
    let existing = scan(xml)?;
    if existing.notes() == *notes && consistent(&existing, notes) {
        return Ok(xml.to_string());
    }
    let targets = targets(notes, &existing);
    let dt_colors: Vec<u8> = notes.colors.iter().map(|c| c.darktable()).collect();

    let mut reader = new_reader(xml);
    let mut writer = Writer::new(Vec::new());
    let mut scopes = Scopes::default();
    let mut depth = 0usize;
    let mut first_description: Option<usize> = None;
    let mut done_first = false;
    let mut inside_description: Option<usize> = None;
    let mut skip: Option<usize> = None;
    // Espaces en attente : supprimés avec un élément retiré, recopiés sinon.
    let mut pending: Option<Event<'static>> = None;
    let mut child_indent = String::from("\n   ");

    let write = |writer: &mut Writer<Vec<u8>>, event: Event<'_>| -> Result<()> {
        writer
            .write_event(event)
            .map_err(|e| XmpError::Malformed(e.to_string()))
    };

    loop {
        let event = reader.read_event()?;
        if skip.is_some() {
            match &event {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    if skip == Some(depth) {
                        skip = None;
                    }
                    depth -= 1;
                }
                Event::Eof => break,
                _ => {}
            }
            continue;
        }
        match event {
            Event::Text(ref t) if t.chars().all(char::is_whitespace) => {
                if let Some(p) = pending.take() {
                    write(&mut writer, p)?;
                }
                pending = Some(event.into_owned());
            }
            Event::Start(ref e) | Event::Empty(ref e) => {
                let is_empty = matches!(event, Event::Empty(_));
                scopes.push(e)?;
                depth += 1;
                let qname = name_of(e);
                let our_child = inside_description == Some(depth - 1)
                    && scopes
                        .resolve(&qname)
                        .and_then(|(u, l)| Field::find(&u, l))
                        .is_some();
                if our_child {
                    // Champ en élément : retiré (avec les espaces qui le précèdent).
                    pending = None;
                    if is_empty {
                        scopes.pop();
                        depth -= 1;
                    } else {
                        skip = Some(depth);
                        scopes.pop();
                    }
                    continue;
                }
                if let Some(Event::Text(t)) = &pending
                    && inside_description == Some(depth - 1)
                {
                    child_indent = t.to_string();
                }
                if let Some(p) = pending.take() {
                    write(&mut writer, p)?;
                }
                if is_rdf(&scopes, &qname, "Description") {
                    let first = !done_first;
                    done_first = true;
                    let rebuilt = rebuild_description(e, &scopes, first.then_some(&targets[..]))?;
                    if first {
                        first_description = Some(depth);
                    }
                    if is_empty {
                        if first && !dt_colors.is_empty() {
                            // Il faut un élément enfant : <x/> devient <x>…</x>.
                            write(&mut writer, Event::Start(rebuilt.borrow()))?;
                            write_colors(&mut writer, &scopes, &dt_colors, &child_indent)?;
                            write(&mut writer, Event::End(rebuilt.to_end()))?;
                        } else {
                            write(&mut writer, Event::Empty(rebuilt))?;
                        }
                        scopes.pop();
                        depth -= 1;
                        continue;
                    }
                    write(&mut writer, Event::Start(rebuilt))?;
                    inside_description = Some(depth);
                    continue;
                }
                write(&mut writer, event.clone())?;
                if is_empty {
                    scopes.pop();
                    depth -= 1;
                }
            }
            Event::End(ref e) => {
                if inside_description == Some(depth) {
                    if first_description == Some(depth) && !dt_colors.is_empty() {
                        write_colors(&mut writer, &scopes, &dt_colors, &child_indent)?;
                    }
                    inside_description = None;
                }
                if let Some(p) = pending.take() {
                    write(&mut writer, p)?;
                }
                write(&mut writer, Event::End(e.borrow()))?;
                scopes.pop();
                depth -= 1;
            }
            Event::Eof => break,
            other => {
                if let Some(p) = pending.take() {
                    write(&mut writer, p)?;
                }
                write(&mut writer, other)?;
            }
        }
    }
    if let Some(p) = pending.take() {
        write(&mut writer, p)?;
    }
    String::from_utf8(writer.into_inner()).map_err(|e| XmpError::Malformed(e.to_string()))
}

/// Vrai si les champs présents sont déjà exactement ceux qu'on écrirait (rien à faire).
fn consistent(existing: &Raw, notes: &Notes) -> bool {
    let targets = targets(notes, existing);
    let scalars_ok = targets
        .iter()
        .all(|(f, v)| existing.get(*f).map(str::trim) == v.as_deref());
    let dt: Vec<String> = notes
        .colors
        .iter()
        .map(|c| c.darktable().to_string())
        .collect();
    let dt_ok = match &existing.darktable_colors {
        None => dt.is_empty(),
        Some(list) => *list == dt,
    };
    scalars_ok && dt_ok
}

/// Un attribut dans le texte brut d'une balise ouvrante, avec ses positions.
struct RawAttr {
    /// Début des espaces qui le précèdent.
    space: usize,
    /// Début du nom.
    start: usize,
    /// Fin (après le guillemet fermant).
    end: usize,
    key: String,
    /// Position de la valeur (entre les guillemets).
    value: std::ops::Range<usize>,
}

/// Découpe le texte brut d'une balise ouvrante (`nom a="1"  b='2'`) en attributs, avec
/// leurs positions, pour pouvoir la modifier sans toucher à sa mise en page.
fn raw_attributes(raw: &str, name_len: usize) -> Result<Vec<RawAttr>> {
    let bytes = raw.as_bytes();
    let malformed = || XmpError::Malformed(format!("balise illisible : <{raw}>"));
    let mut attrs = Vec::new();
    let mut i = name_len;
    loop {
        let space = i;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let key = raw[start..i].to_string();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            return Err(malformed());
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let quote = *bytes.get(i).ok_or_else(malformed)?;
        if quote != b'"' && quote != b'\'' {
            return Err(malformed());
        }
        let value_start = i + 1;
        let value_end = value_start
            + raw[value_start..]
                .find(quote as char)
                .ok_or_else(malformed)?;
        i = value_end + 1;
        attrs.push(RawAttr {
            space,
            start,
            end: i,
            key,
            value: value_start..value_end,
        });
    }
    Ok(attrs)
}

/// Balise ouvrante d'un `rdf:Description` modifiée **dans son texte** : nos attributs
/// changent de valeur sur place ou disparaissent (avec l'espace qui les précède), les
/// nouveaux s'ajoutent en fin de balise avec l'espacement du dernier attribut. Tout le
/// reste (ordre, mise en page, guillemets) est conservé. Pour le premier `rdf:Description`
/// (`targets`), y écrit nos champs et déclare les espaces de noms nécessaires ; pour les
/// autres, retire seulement nos champs.
fn rebuild_description<'a>(
    start: &BytesStart<'_>,
    scopes: &Scopes,
    targets: Option<&[(Field, Option<String>)]>,
) -> Result<BytesStart<'a>> {
    let name = name_of(start);
    let raw: &str = start;
    let attrs = raw_attributes(raw, name.len())?;
    let field_of = |key: &str| scopes.resolve(key).and_then(|(u, l)| Field::find(&u, l));
    let target = |field: Field| -> Option<Option<String>> {
        targets?
            .iter()
            .find(|(f, _)| *f == field)
            .map(|(_, v)| v.clone())
    };

    let mut out = String::with_capacity(raw.len() + 64);
    let mut cursor = 0;
    let mut written: Vec<Field> = Vec::new();
    for a in &attrs {
        let Some(field) = field_of(&a.key) else {
            continue;
        };
        match target(field).flatten() {
            // Valeur modifiée sur place (première occurrence seulement).
            Some(value) if !written.contains(&field) => {
                out.push_str(&raw[cursor..a.value.start]);
                out.push_str(&value);
                cursor = a.value.end;
                written.push(field);
            }
            // Retiré, avec les espaces qui le précèdent.
            _ => {
                out.push_str(&raw[cursor..a.space]);
                cursor = a.end;
            }
        }
    }
    // Fin de balise : on insère avant les espaces finaux éventuels.
    let tail_start = attrs.last().map_or(name.len(), |a| a.end);
    out.push_str(&raw[cursor..tail_start.max(cursor)]);
    let separator = attrs
        .last()
        .map(|a| raw[a.space..a.start].to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| " ".to_string());
    let mut declared: Vec<(String, String)> = Vec::new();
    let mut additions = String::new();
    for (field, value) in targets.unwrap_or(&[]) {
        let Some(value) = value else { continue };
        if written.contains(field) {
            continue;
        }
        let (uri, local, preferred) = field.name();
        let prefix = match scopes.prefix(uri) {
            Some(p) => p.to_string(),
            None => match declared.iter().find(|(_, u)| u == uri) {
                Some((p, _)) => p.clone(),
                None => {
                    let p = free_prefix(scopes, &declared, preferred);
                    declared.push((p.clone(), uri.to_string()));
                    p
                }
            },
        };
        additions.push_str(&format!("{separator}{prefix}:{local}=\"{value}\""));
    }
    for (p, uri) in &declared {
        out.push_str(&format!("{separator}xmlns:{p}=\"{uri}\""));
    }
    out.push_str(&additions);
    out.push_str(&raw[tail_start.max(cursor)..]);
    Ok(BytesStart::from_content(out, name.len()).into_owned())
}

/// Préfixe libre pour déclarer un espace de noms (le préfixe usuel s'il n'est pas déjà pris).
fn free_prefix(scopes: &Scopes, declared: &[(String, String)], preferred: &str) -> String {
    let taken = |p: &str| scopes.uri(p).is_some() || declared.iter().any(|(d, _)| d == p);
    if !taken(preferred) {
        return preferred.to_string();
    }
    (1..)
        .map(|n| format!("{preferred}{n}"))
        .find(|p| !taken(p))
        .expect("préfixe libre")
}

/// Écrit `<darktable:colorlabels><rdf:Seq><rdf:li>n</rdf:li>…` dans le premier
/// `rdf:Description`, avec l'indentation de ses autres enfants.
fn write_colors(
    writer: &mut Writer<Vec<u8>>,
    scopes: &Scopes,
    colors: &[u8],
    indent: &str,
) -> Result<()> {
    let rdf = scopes.prefix(ns::RDF).unwrap_or("rdf").to_string();
    let (dt_prefix, declare) = match scopes.prefix(ns::DARKTABLE) {
        Some(p) => (p.to_string(), false),
        None => (free_prefix(scopes, &[], "darktable"), true),
    };
    let w = |writer: &mut Writer<Vec<u8>>, e: Event<'_>| {
        writer
            .write_event(e)
            .map_err(|e| XmpError::Malformed(e.to_string()))
    };
    let tag = format!("{dt_prefix}:colorlabels");
    let mut start = BytesStart::new(tag.clone());
    if declare {
        start.push_attribute((format!("xmlns:{dt_prefix}").as_str(), ns::DARKTABLE));
    }
    let seq = format!("{rdf}:Seq");
    let li = format!("{rdf}:li");
    w(
        writer,
        Event::Text(BytesText::from_escaped(indent.to_string())),
    )?;
    w(writer, Event::Start(start))?;
    w(
        writer,
        Event::Text(BytesText::from_escaped(format!("{indent} "))),
    )?;
    w(writer, Event::Start(BytesStart::new(seq.clone())))?;
    for c in colors {
        w(
            writer,
            Event::Text(BytesText::from_escaped(format!("{indent}  "))),
        )?;
        w(writer, Event::Start(BytesStart::new(li.clone())))?;
        w(writer, Event::Text(BytesText::new(&c.to_string())))?;
        w(writer, Event::End(BytesEnd::new(li.clone())))?;
    }
    w(
        writer,
        Event::Text(BytesText::from_escaped(format!("{indent} "))),
    )?;
    w(writer, Event::End(BytesEnd::new(seq)))?;
    w(
        writer,
        Event::Text(BytesText::from_escaped(indent.to_string())),
    )?;
    w(writer, Event::End(BytesEnd::new(tag)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sidecar créé par darktable à l'import (structure réelle, auteur anonymisé).
    const DARKTABLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 4.4.0-Exiv2">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:exif="http://ns.adobe.com/exif/1.0/"
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:xmpMM="http://ns.adobe.com/xap/1.0/mm/"
    xmlns:darktable="http://darktable.sf.net/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
   exif:DateTimeOriginal="2026:10:16 14:24:58.000"
   xmp:Rating="0"
   xmpMM:DerivedFrom="DSCF0092.RAF"
   darktable:xmp_version="5"
   darktable:history_end="0">
   <darktable:history>
    <rdf:Seq/>
   </darktable:history>
   <dc:creator>
    <rdf:Seq>
     <rdf:li>Auteur &amp; Cie</rdf:li>
    </rdf:Seq>
   </dc:creator>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
"#;

    fn notes(stars: u8, flag: Flag, colors: &[Color]) -> Notes {
        Notes {
            stars,
            flag,
            colors: colors.to_vec(),
        }
    }

    #[test]
    fn reads_the_darktable_sidecar() {
        assert_eq!(parse(DARKTABLE).unwrap(), Notes::default());
    }

    #[test]
    fn unchanged_notes_leave_the_file_byte_for_byte_identical() {
        assert_eq!(apply(DARKTABLE, &Notes::default()).unwrap(), DARKTABLE);
    }

    #[test]
    fn changing_stars_only_touches_the_rating() {
        let out = apply(DARKTABLE, &notes(3, Flag::None, &[])).unwrap();
        assert_eq!(parse(&out).unwrap(), notes(3, Flag::None, &[]));
        assert_eq!(
            out,
            DARKTABLE.replace(r#"xmp:Rating="0""#, r#"xmp:Rating="3""#)
        );
    }

    #[test]
    fn everything_else_survives_a_full_update() {
        let full = notes(4, Flag::Accepted, &[Color::Green]);
        let out = apply(DARKTABLE, &full).unwrap();
        assert_eq!(parse(&out).unwrap(), full);
        for kept in [
            "x:xmptk=\"XMP Core 4.4.0-Exiv2\"",
            "exif:DateTimeOriginal=\"2026:10:16 14:24:58.000\"",
            "xmpMM:DerivedFrom=\"DSCF0092.RAF\"",
            "darktable:history_end=\"0\"",
            "<darktable:history>\n    <rdf:Seq/>\n   </darktable:history>",
            "<rdf:li>Auteur &amp; Cie</rdf:li>",
        ] {
            assert!(out.contains(kept), "perdu : {kept}\n{out}");
        }
        // Une seule couleur : les trois champs, lisibles partout.
        assert!(out.contains(r#"xmp:Label="Green""#));
        assert!(out.contains(r#"digiKam:ColorLabel="4""#));
        assert!(out.contains("<darktable:colorlabels>"));
        assert!(out.contains(r#"xmlns:digiKam="http://www.digikam.org/ns/1.0/""#));
        assert!(out.contains(r#"xmpDM:pick="1""#) && out.contains(r#"xmpDM:good="True""#));
        // Et revenir à rien redonne exactement le fichier de départ, aux champs « 0 » près.
        let back = apply(&out, &Notes::default()).unwrap();
        assert_eq!(parse(&back).unwrap(), Notes::default());
        for gone in ["colorlabels", "xmp:Label=", "ColorLabel=", "xmpDM:good="] {
            assert!(!back.contains(gone), "{gone} aurait dû disparaître\n{back}");
        }
        // Les champs qui existaient restent, remis à zéro.
        assert!(back.contains(r#"xmp:Rating="0""#) && back.contains(r#"digiKam:PickLabel="0""#));
    }

    #[test]
    fn rejecting_keeps_the_stars_for_later() {
        let rejected = notes(3, Flag::Rejected, &[]);
        let out = apply(DARKTABLE, &rejected).unwrap();
        assert!(out.contains(r#"xmp:Rating="-1""#), "standard : -1 = rejeté");
        assert!(out.contains(r#"tri:RatingBeforeReject="3""#));
        assert!(out.contains(r#"digiKam:PickLabel="1""#) && out.contains(r#"xmpDM:pick="-1""#));
        assert_eq!(parse(&out).unwrap(), rejected);
        // Annuler le rejet : les étoiles reviennent, le champ tri disparaît.
        let back = apply(&out, &notes(3, Flag::None, &[])).unwrap();
        assert!(back.contains(r#"xmp:Rating="3""#));
        assert!(!back.contains("RatingBeforeReject"));
        assert_eq!(parse(&back).unwrap(), notes(3, Flag::None, &[]));
    }

    #[test]
    fn several_colors_go_to_darktable_only() {
        let two = notes(0, Flag::None, &[Color::Red, Color::Blue]);
        let out = apply(DARKTABLE, &two).unwrap();
        assert!(!out.contains("xmp:Label"), "darktable n'en verrait qu'une");
        assert!(out.contains("<rdf:li>0</rdf:li>") && out.contains("<rdf:li>3</rdf:li>"));
        assert_eq!(parse(&out).unwrap(), two);
    }

    #[test]
    fn element_form_and_other_prefixes_are_understood() {
        // Bridge : préfixe « xap » et valeurs en éléments.
        let bridge = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:xap="http://ns.adobe.com/xap/1.0/">
   <xap:Rating>2</xap:Rating>
   <xap:Label>Blue</xap:Label>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;
        assert_eq!(parse(bridge).unwrap(), notes(2, Flag::None, &[Color::Blue]));
        let out = apply(bridge, &notes(5, Flag::None, &[Color::Blue])).unwrap();
        assert!(
            out.contains(r#"xap:Rating="5""#),
            "même préfixe que le fichier\n{out}"
        );
        assert!(
            !out.contains("<xap:Rating>"),
            "ancienne forme élément retirée"
        );
        assert!(
            !out.contains("xmlns:xmp="),
            "pas de seconde déclaration du même espace"
        );
        assert_eq!(parse(&out).unwrap(), notes(5, Flag::None, &[Color::Blue]));
    }

    #[test]
    fn values_written_by_other_software_are_read() {
        let lightroom = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:xmpDM="http://ns.adobe.com/xmp/1.0/DynamicMedia/"
    xmp:Rating="3.0" xmpDM:pick="1" xmpDM:good="True"/>
 </rdf:RDF></x:xmpmeta>"#;
        assert_eq!(parse(lightroom).unwrap(), notes(3, Flag::Accepted, &[]));
        let digikam = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:digiKam="http://www.digikam.org/ns/1.0/" digiKam:PickLabel="1" digiKam:ColorLabel="6"/>
 </rdf:RDF></x:xmpmeta>"#;
        assert_eq!(
            parse(digikam).unwrap(),
            notes(0, Flag::Rejected, &[Color::Purple])
        );
    }

    #[test]
    fn an_empty_description_can_receive_colors() {
        let empty = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about=""/></rdf:RDF></x:xmpmeta>"#;
        let two = notes(1, Flag::None, &[Color::Yellow, Color::Purple]);
        let out = apply(empty, &two).unwrap();
        assert_eq!(parse(&out).unwrap(), two, "{out}");
    }

    #[test]
    fn malformed_or_foreign_files_are_refused() {
        assert!(matches!(
            apply("<x:xmpmeta><rdf:RDF></x:xmpmeta>", &Notes::default()),
            Err(XmpError::Malformed(_))
        ));
        assert!(matches!(
            parse("<root><child/></root>"),
            Err(XmpError::NoDescription)
        ));
    }

    #[test]
    fn new_packet_is_minimal_and_valid() {
        let out = apply(NEW_PACKET, &notes(2, Flag::None, &[])).unwrap();
        assert_eq!(parse(&out).unwrap(), notes(2, Flag::None, &[]));
        assert_eq!(apply(NEW_PACKET, &Notes::default()).unwrap(), NEW_PACKET);
    }

    #[test]
    fn sidecar_names_follow_the_convention() {
        let image = Path::new("/photos/DSCF0092.RAF");
        assert_eq!(
            Convention::Darktable.sidecar(image),
            Path::new("/photos/DSCF0092.RAF.xmp")
        );
        assert_eq!(
            Convention::Adobe.sidecar(image),
            Path::new("/photos/DSCF0092.xmp")
        );
        assert_eq!(
            Convention::of(image, Path::new("/photos/DSCF0092.RAF.XMP")),
            Convention::Darktable
        );
        assert_eq!(
            Convention::of(image, Path::new("/photos/DSCF0092.xmp")),
            Convention::Adobe
        );
    }

    #[test]
    fn stars_are_capped_and_keep_the_flag() {
        let mut n = Notes {
            flag: Flag::Rejected,
            ..Notes::default()
        };
        n.set_stars(7);
        assert_eq!((n.stars, n.flag), (5, Flag::Rejected));
    }

    #[test]
    fn single_color_replaces_or_removes() {
        let mut n = Notes::default();
        n.toggle_color(Color::Red, false);
        n.toggle_color(Color::Blue, false);
        assert_eq!(n.colors, [Color::Blue]);
        n.toggle_color(Color::Blue, false);
        assert!(n.colors.is_empty());
    }

    #[test]
    fn multiple_colors_toggle_and_stay_sorted() {
        let mut n = Notes::default();
        n.toggle_color(Color::Purple, true);
        n.toggle_color(Color::Red, true);
        assert_eq!(n.colors, [Color::Red, Color::Purple]);
        assert!(n.has_color(Color::Purple));
        n.toggle_color(Color::Purple, true);
        assert_eq!(n.colors, [Color::Red]);
        n.clear_colors();
        assert_eq!(n, Notes::EMPTY);
    }
}
