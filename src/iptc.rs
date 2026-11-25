//! Métadonnées de création (#277) : auteur, copyright, conditions d'utilisation, légende,
//! titre, lieu… posés dans les images, en **XMP** seulement (décidé le 25/11/2026 : lu par
//! Lightroom, darktable, Capture One, Photos, Bridge, l'Explorateur Windows ; pas d'IPTC-IIM
//! binaire).
//!
//! - **Jeux** dans la configuration (`[iptc.sets.<id>]`), un par défaut (`[iptc] default`).
//!   Les valeurs acceptent `{year}` (l'année de la date du fichier : la prise de vue pour
//!   une photo du boîtier) et `{name}` (le nom de l'image, sans extension).
//! - **Ne jamais écraser par surprise** : un champ n'est posé que s'il est vide, sauf ceux
//!   que le jeu cite dans `replace` (le copyright, typiquement).
//! - **Où** : une image qui porte un XMP embarqué (JPEG, TIFF, PNG, WebP, HEIF) le reçoit
//!   dans le fichier, sans recompression ([`crate::embedded::rewrite`]) ; un RAW dans son
//!   sidecar ([`crate::xmp::sidecar_path`]), que les logiciels de développement reprennent
//!   à l'export.
//! - **Quand** : à l'export (clé `iptc` d'un préréglage d'export), et sur demande (une
//!   sélection dans l'interface, `tri --iptc <id> <images>`).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::xmp::{self, Property, Shape};

/// Espaces de noms des champs.
mod ns {
    pub const DC: &str = "http://purl.org/dc/elements/1.1/";
    pub const RIGHTS: &str = "http://ns.adobe.com/xap/1.0/rights/";
    pub const PHOTOSHOP: &str = "http://ns.adobe.com/photoshop/1.0/";
}

/// Un champ d'un jeu : sa clé dans la configuration et sa propriété XMP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    /// Clé (`creator`, `copyright`…), aussi celle de `replace`.
    pub key: &'static str,
    /// Propriété XMP.
    pub property: Property,
}

const fn field(
    key: &'static str,
    uri: &'static str,
    local: &'static str,
    prefix: &'static str,
    shape: Shape,
) -> Field {
    Field {
        key,
        property: Property {
            uri,
            local,
            prefix,
            shape,
        },
    }
}

/// Les champs connus, dans l'ordre des jeux.
pub const FIELDS: [Field; 10] = [
    field("creator", ns::DC, "creator", "dc", Shape::Seq),
    field("copyright", ns::DC, "rights", "dc", Shape::Alt),
    field(
        "usage_terms",
        ns::RIGHTS,
        "UsageTerms",
        "xmpRights",
        Shape::Alt,
    ),
    field(
        "web_statement",
        ns::RIGHTS,
        "WebStatement",
        "xmpRights",
        Shape::Text,
    ),
    field("credit", ns::PHOTOSHOP, "Credit", "photoshop", Shape::Text),
    field("title", ns::DC, "title", "dc", Shape::Alt),
    field("caption", ns::DC, "description", "dc", Shape::Alt),
    field("city", ns::PHOTOSHOP, "City", "photoshop", Shape::Text),
    field("state", ns::PHOTOSHOP, "State", "photoshop", Shape::Text),
    field(
        "country",
        ns::PHOTOSHOP,
        "Country",
        "photoshop",
        Shape::Text,
    ),
];

/// « Protégé par copyright » (`xmpRights:Marked`), posé avec un copyright s'il est absent :
/// c'est ce que Lightroom et Bridge affichent comme statut.
const MARKED: Property = Property {
    uri: ns::RIGHTS,
    local: "Marked",
    prefix: "xmpRights",
    shape: Shape::Text,
};

/// Un jeu de métadonnées (`[iptc.sets.<id>]`) ; un champ vide n'est pas posé.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IptcSet {
    /// Nom affiché ; vide : l'identifiant.
    pub name: String,
    /// Auteur (`dc:creator`) ; plusieurs auteurs séparés par « ; ».
    pub creator: String,
    /// Mention de copyright (`dc:rights`), ex. `© {year} Prénom Nom`.
    pub copyright: String,
    /// Conditions d'utilisation (`xmpRights:UsageTerms`).
    pub usage_terms: String,
    /// Adresse des conditions ou du site (`xmpRights:WebStatement`).
    pub web_statement: String,
    /// Crédit (`photoshop:Credit`).
    pub credit: String,
    /// Titre (`dc:title`).
    pub title: String,
    /// Légende (`dc:description`).
    pub caption: String,
    /// Ville (`photoshop:City`).
    pub city: String,
    /// Région, province (`photoshop:State`).
    pub state: String,
    /// Pays (`photoshop:Country`).
    pub country: String,
    /// Champs toujours remplacés (les autres ne sont posés que s'ils sont vides).
    pub replace: Vec<String>,
}

impl IptcSet {
    /// Valeur du champ `key`.
    pub fn value(&self, key: &str) -> &str {
        match key {
            "creator" => &self.creator,
            "copyright" => &self.copyright,
            "usage_terms" => &self.usage_terms,
            "web_statement" => &self.web_statement,
            "credit" => &self.credit,
            "title" => &self.title,
            "caption" => &self.caption,
            "city" => &self.city,
            "state" => &self.state,
            "country" => &self.country,
            _ => "",
        }
    }

    /// Vrai si le jeu ne pose rien.
    pub fn is_empty(&self) -> bool {
        FIELDS.iter().all(|f| self.value(f.key).trim().is_empty())
    }

    /// Toutes les valeurs, pour écrire un jeu en entier dans la configuration (un nouveau
    /// créé depuis l'interface).
    pub fn entries(&self) -> Vec<(&'static str, crate::config_edit::Value)> {
        use crate::config_edit::Value;
        let mut entries = vec![("name", Value::Str(self.name.clone()))];
        entries.extend(
            FIELDS
                .iter()
                .map(|f| (f.key, Value::Str(self.value(f.key).to_string()))),
        );
        entries.push((
            "replace",
            Value::Toml(format!(
                "[{}]",
                self.replace
                    .iter()
                    .map(|r| format!("{r:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        ));
        entries
    }
}

/// Jeu fourni, vide, à remplir (`[iptc.sets.mine]`, « Mes métadonnées ») : il sert de
/// modèle aux jeux créés depuis l'interface. Le copyright y est remplacé d'office.
pub const MINE: &str = "mine";

/// Les jeux : celui fourni ([`MINE`]), complété ou remplacé par ceux de la configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IptcSets(pub BTreeMap<String, IptcSet>);

impl Default for IptcSets {
    fn default() -> Self {
        let mine = IptcSet {
            replace: vec!["copyright".into()],
            ..IptcSet::default()
        };
        IptcSets(BTreeMap::from([(MINE.to_string(), mine)]))
    }
}

impl<'de> Deserialize<'de> for IptcSets {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let written = BTreeMap::<String, IptcSet>::deserialize(d)?;
        let mut sets = IptcSets::default();
        sets.0.extend(written);
        Ok(sets)
    }
}

/// Section `[iptc]` du fichier de configuration.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IptcSettings {
    /// Jeu proposé sur demande (identifiant) ; vide : le premier.
    pub default: String,
    /// Les jeux (`[iptc.sets.<id>]`).
    pub sets: IptcSets,
}

impl Default for IptcSettings {
    fn default() -> Self {
        IptcSettings {
            default: MINE.into(),
            sets: IptcSets::default(),
        }
    }
}

impl IptcSettings {
    /// Le jeu `id` ; vide : celui par défaut, sinon le premier.
    pub fn set(&self, id: &str) -> Option<(&str, &IptcSet)> {
        let id = if id.is_empty() {
            self.default.as_str()
        } else {
            id
        };
        if id.is_empty() {
            return self.sets.0.iter().next().map(|(k, v)| (k.as_str(), v));
        }
        self.sets.0.get_key_value(id).map(|(k, v)| (k.as_str(), v))
    }
}

/// Ce que valent `{year}` et `{name}` pour une image.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vars {
    /// Année (« 2026 »).
    pub year: String,
    /// Nom de l'image sans extension.
    pub name: String,
}

impl Vars {
    /// D'après le fichier `image` : l'année de sa date (la prise de vue pour une photo du
    /// boîtier), son nom ; l'année en cours si la date manque.
    pub fn of(image: &Path) -> Vars {
        let modified = std::fs::metadata(image)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| std::time::SystemTime::now());
        Vars {
            year: chrono::DateTime::<chrono::Local>::from(modified)
                .format("%Y")
                .to_string(),
            name: image
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }

    fn fill(&self, text: &str) -> String {
        text.replace("{year}", &self.year)
            .replace("{name}", &self.name)
    }
}

/// Le paquet XMP `xml` avec le jeu `set` : chaque champ non vide posé s'il est absent (ou
/// vide) dans le paquet, ou s'il est dans `replace` ; tout le reste gardé tel quel.
pub fn apply(xml: &str, set: &IptcSet, vars: &Vars) -> Result<String, String> {
    let mut xml = xml.to_string();
    let err = |e: xmp::XmpError| e.to_string();
    for f in FIELDS {
        let wanted = set.value(f.key).trim();
        if wanted.is_empty() {
            continue;
        }
        let present = xmp::read_property(&xml, f.property)
            .map_err(err)?
            .is_some_and(|v| v.iter().any(|s| !s.trim().is_empty()));
        if present && !set.replace.iter().any(|r| r == f.key) {
            continue;
        }
        let values: Vec<String> = match f.property.shape {
            Shape::Seq => vars
                .fill(wanted)
                .split(';')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            _ => vec![vars.fill(wanted)],
        };
        xml = xmp::apply_property(&xml, f.property, &values).map_err(err)?;
        if f.key == "copyright" && xmp::read_property(&xml, MARKED).map_err(err)?.is_none() {
            xml = xmp::apply_property(&xml, MARKED, &["True".to_string()]).map_err(err)?;
        }
    }
    Ok(xml)
}

/// Les champs de création déjà dans l'image (son XMP embarqué, sinon son sidecar le plus
/// récent) : (clé de [`FIELDS`], valeur), dans l'ordre des champs, les vides omis. Pour le
/// panneau des métadonnées.
pub fn read(image: &Path) -> Vec<(&'static str, String)> {
    let packet = if crate::embedded::supports(image) {
        crate::embedded::read_packet(image).ok().flatten()
    } else {
        xmp::existing_sidecars(image)
            .into_iter()
            .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
            .and_then(|p| std::fs::read_to_string(p).ok())
    };
    let Some(xml) = packet else {
        return Vec::new();
    };
    FIELDS
        .iter()
        .filter_map(|f| {
            let values = xmp::read_property(&xml, f.property).ok().flatten()?;
            let text = values
                .iter()
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
                .collect::<Vec<_>>();
            // Une liste par langue : la première (`x-default`).
            let text = match f.property.shape {
                Shape::Alt => text.first().map(|t| t.to_string()),
                _ => (!text.is_empty()).then(|| text.join("; ")),
            }?;
            Some((f.key, text))
        })
        .collect()
}

/// Pose le jeu `set` sur l'image `image` : dans le fichier s'il porte un XMP embarqué,
/// sinon (un RAW) dans son sidecar (`convention` s'il n'en a pas). Renvoie vrai si un
/// fichier a changé. `dry_run` : ne rien écrire, dire seulement si ça changerait.
pub fn stamp(
    image: &Path,
    set: &IptcSet,
    convention: xmp::Convention,
    dry_run: bool,
) -> Result<bool, String> {
    let vars = Vars::of(image);
    let edit = |xml: &str| apply(xml, set, &vars);
    if crate::embedded::supports(image) {
        if dry_run {
            let current = crate::embedded::read_packet(image)
                .map_err(|e| e.to_string())?
                .unwrap_or_else(|| xmp::new_packet_body().to_string());
            return Ok(edit(&current)? != current);
        }
        return crate::embedded::rewrite(image, &edit);
    }
    let sidecar = xmp::sidecar_path(image, convention);
    let current = match std::fs::read_to_string(&sidecar) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => xmp::NEW_PACKET.to_string(),
        Err(e) => return Err(format!("{} : {e}", sidecar.display())),
    };
    let updated = edit(&current)?;
    if updated == current {
        return Ok(false);
    }
    if !dry_run {
        std::fs::write(&sidecar, updated).map_err(|e| format!("{} : {e}", sidecar.display()))?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> IptcSet {
        IptcSet {
            creator: "Ana Lima; Bo Chen".into(),
            copyright: "© {year} Ana Lima".into(),
            caption: "Une légende pour {name}".into(),
            city: "Liège".into(),
            replace: vec!["copyright".into()],
            ..IptcSet::default()
        }
    }

    #[test]
    fn a_set_fills_empty_fields_and_replaces_only_those_listed() {
        let vars = Vars {
            year: "2026".into(),
            name: "DSCF0001".into(),
        };
        let packet = format!(
            "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"{}\">\
             <rdf:Description rdf:about=\"\" xmlns:photoshop=\"{}\" xmlns:xmp=\"{}\" \
             photoshop:City=\"Namur\" xmp:Rating=\"3\">\
             <dc:rights xmlns:dc=\"{}\"><rdf:Alt><rdf:li xml:lang=\"x-default\">old</rdf:li>\
             </rdf:Alt></dc:rights></rdf:Description></rdf:RDF></x:xmpmeta>",
            xmp::ns::RDF,
            ns::PHOTOSHOP,
            xmp::ns::XMP,
            ns::DC
        );
        let out = apply(&packet, &set(), &vars).unwrap();
        let read = |p: Property| xmp::read_property(&out, p).unwrap();
        let prop = |key: &str| FIELDS.iter().find(|f| f.key == key).unwrap().property;
        assert_eq!(
            read(prop("creator")),
            Some(vec!["Ana Lima".to_string(), "Bo Chen".to_string()])
        );
        assert_eq!(
            read(prop("copyright")),
            Some(vec!["© 2026 Ana Lima".to_string()]),
            "remplacé : dans replace"
        );
        assert_eq!(
            read(prop("city")),
            Some(vec!["Namur".to_string()]),
            "déjà là (en attribut) : gardé"
        );
        assert_eq!(
            read(prop("caption")),
            Some(vec!["Une légende pour DSCF0001".to_string()])
        );
        assert_eq!(read(MARKED), Some(vec!["True".to_string()]));
        // Le reste est gardé, et une deuxième fois ne change plus rien.
        assert_eq!(xmp::parse(&out).unwrap().stars, 3);
        assert_eq!(apply(&out, &set(), &vars).unwrap(), out);
    }

    #[test]
    fn a_raw_gets_its_sidecar_and_a_jpeg_its_own_packet() {
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join("DSCF0001.RAF");
        std::fs::write(&raw, b"raw").unwrap();
        assert!(stamp(&raw, &set(), xmp::Convention::Darktable, true).unwrap());
        assert!(
            !xmp::sidecar_path(&raw, xmp::Convention::Darktable).exists(),
            "à l'essai"
        );
        assert!(stamp(&raw, &set(), xmp::Convention::Darktable, false).unwrap());
        let sidecar =
            std::fs::read_to_string(xmp::sidecar_path(&raw, xmp::Convention::Darktable)).unwrap();
        assert!(sidecar.contains("Liège"), "{sidecar}");
        assert!(!stamp(&raw, &set(), xmp::Convention::Darktable, false).unwrap());
        let jpeg = dir.path().join("IMG.jpg");
        image::RgbImage::new(8, 8).save(&jpeg).unwrap();
        let before = std::fs::read(&jpeg).unwrap();
        assert!(stamp(&jpeg, &set(), xmp::Convention::Darktable, false).unwrap());
        let packet = crate::embedded::read_packet(&jpeg).unwrap().unwrap();
        assert!(packet.contains("Ana Lima"), "{packet}");
        // Relus pour le panneau des métadonnées.
        let shown = read(&jpeg);
        assert_eq!(shown[0], ("creator", "Ana Lima; Bo Chen".to_string()));
        assert!(read(&raw).iter().any(|(k, v)| *k == "city" && v == "Liège"));
        let after = std::fs::read(&jpeg).unwrap();
        assert!(
            after.ends_with(&before[before.len() - 100..]),
            "image gardée telle quelle"
        );
    }

    #[test]
    fn the_default_set_is_chosen_or_the_first() {
        let mut s = IptcSettings::default();
        assert_eq!(
            s.set("").map(|(k, v)| (k, v.is_empty())),
            Some((MINE, true))
        );
        s.sets.0.insert("b".into(), set());
        s.sets.0.insert("a".into(), IptcSet::default());
        s.default = String::new();
        assert_eq!(s.set("").map(|(k, _)| k), Some("a"));
        s.default = "b".into();
        assert_eq!(s.set("").map(|(k, _)| k), Some("b"));
        assert!(s.set("z").is_none());
        // Lu dans la configuration : le jeu fourni reste, complété.
        let read: IptcSettings =
            toml::from_str("[sets.club]\ncreator = \"Club\"\n[sets.mine]\ncreator = \"Moi\"\n")
                .unwrap();
        assert_eq!(read.sets.0.len(), 2);
        assert_eq!(read.sets.0["mine"].creator, "Moi");
    }
}
