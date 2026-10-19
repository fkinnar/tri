//! Modification du fichier de configuration depuis une interface (fenêtre de réglages).
//!
//! Deux pièces, indépendantes de l'affichage :
//!
//! - [`Schema`] : la liste des réglages, **lue dans le fichier d'exemple**
//!   (`doc/config.example.toml`) : sections, clés, valeurs par défaut et commentaires. Un
//!   test garantit déjà que l'exemple reste égal aux valeurs du code ; tout nouveau
//!   réglage apparaît donc de lui-même dans la fenêtre, sans écran à écrire.
//! - [`ConfigFile`] : le fichier de l'utilisateur, modifié **sans rien perdre** (ses
//!   commentaires, sa mise en page, les clés que l'interface ne connaît pas) grâce à
//!   `toml_edit`, qui garde le texte d'origine autour de ce qu'on change.
//!
//! La validation (le fichier modifié est-il toujours lisible ?) revient à l'appelant, qui
//! connaît le type complet des réglages ([`crate::settings::parse`]). Voir
//! `doc/config_edit.md`.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item, Table};

/// Valeur d'un réglage, telle que l'interface la modifie.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Booléen : case à cocher.
    Bool(bool),
    /// Entier.
    Int(i64),
    /// Nombre à virgule.
    Float(f64),
    /// Texte (couleur `"#rrggbb"` comprise).
    Str(String),
    /// Autre (liste, table) : écrit tel quel en TOML (`[1280.0, 800.0]`).
    Toml(String),
    /// Liste de textes (raccourcis d'une action : `["Maj+P", "a"]`), écrite par
    /// l'interface ; relue comme [`Value::Toml`].
    Strings(Vec<String>),
}

impl Value {
    fn from_toml(v: &toml_edit::Value) -> Value {
        match v {
            toml_edit::Value::Boolean(b) => Value::Bool(*b.value()),
            toml_edit::Value::Integer(i) => Value::Int(*i.value()),
            toml_edit::Value::Float(f) => Value::Float(*f.value()),
            toml_edit::Value::String(s) => Value::Str(s.value().clone()),
            other => Value::Toml(other.clone().decorated("", "").to_string()),
        }
    }

    /// Valeur TOML ; `None` si un texte `Toml` n'est pas du TOML valide.
    fn to_toml(&self) -> Option<toml_edit::Value> {
        Some(match self {
            Value::Bool(b) => (*b).into(),
            Value::Int(i) => (*i).into(),
            Value::Float(f) => (*f).into(),
            Value::Str(s) => s.as_str().into(),
            Value::Toml(text) => text.trim().parse().ok()?,
            Value::Strings(list) => {
                toml_edit::Value::Array(list.iter().map(String::as_str).collect())
            }
        })
    }

    /// Vrai pour une couleur `"#rrggbb"`.
    pub fn is_color(&self) -> bool {
        matches!(self, Value::Str(s) if parse_color(s).is_some())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Str(s) => write!(f, "\"{s}\""),
            Value::Toml(t) => write!(f, "{t}"),
            Value::Strings(_) => match self.to_toml() {
                Some(v) => write!(f, "{}", v.decorated("", "")),
                None => Ok(()),
            },
        }
    }
}

/// Couleur `"#rrggbb"` en composantes.
pub fn parse_color(s: &str) -> Option<[u8; 3]> {
    let hex = s.strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([c(0)?, c(2)?, c(4)?])
}

/// Un réglage du schéma.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Nom de la clé dans sa section.
    pub key: String,
    /// Valeur par défaut (celle de l'exemple).
    pub default: Value,
}

/// Réglages décrits par un même commentaire (« Badges de la grille : … » au-dessus de
/// `badge_over` et `badge_under`).
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// Commentaire, en un paragraphe (vide s'il n'y en a pas).
    pub doc: String,
    /// Les réglages, dans l'ordre du fichier.
    pub entries: Vec<Entry>,
}

/// Une section (`[gui.colors]`).
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    /// Nom complet, avec les points (`gui.colors`).
    pub name: String,
    /// Commentaire au-dessus de l'en-tête de section.
    pub doc: String,
    /// Les groupes de réglages.
    pub groups: Vec<Group>,
}

/// Liste des réglages, tirée du fichier d'exemple.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schema {
    /// Les sections, dans l'ordre du fichier ; les sections vides (`[keys]`, dont les clés
    /// sont des identifiants d'action choisis par l'utilisateur) sont omises.
    pub sections: Vec<Section>,
}

/// Dernier paragraphe de commentaires d'un préfixe (lignes après la dernière ligne vide),
/// sans les `#`, en une seule ligne.
fn comment(prefix: Option<&str>) -> String {
    let text = prefix.unwrap_or_default();
    let paragraph = text
        .rsplit("\n\n")
        .next()
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix('#'))
        .map(str::trim)
        .collect::<Vec<_>>();
    paragraph.join(" ").trim().to_string()
}

impl Schema {
    /// Lit le fichier d'exemple `text`.
    pub fn parse(text: &str) -> Result<Schema, String> {
        let doc: DocumentMut = text
            .parse()
            .map_err(|e: toml_edit::TomlError| e.to_string())?;
        let mut schema = Schema::default();
        collect(doc.as_table(), "", &mut schema.sections);
        Ok(schema)
    }

    /// Valeur par défaut de `section`.`key`.
    pub fn default_of(&self, section: &str, key: &str) -> Option<&Value> {
        self.sections
            .iter()
            .find(|s| s.name == section)?
            .groups
            .iter()
            .flat_map(|g| &g.entries)
            .find(|e| e.key == key)
            .map(|e| &e.default)
    }
}

/// Sections de `table` (nommée `name`) et de ses sous-tables, dans l'ordre du fichier.
fn collect(table: &Table, name: &str, out: &mut Vec<Section>) {
    let mut section = Section {
        name: name.to_string(),
        doc: comment(table.decor().prefix().and_then(|p| p.as_str())),
        groups: Vec::new(),
    };
    let mut subtables = Vec::new();
    for (key, item) in table.iter() {
        match item {
            Item::Value(v) => {
                let doc = table
                    .key(key)
                    .and_then(|k| k.leaf_decor().prefix())
                    .and_then(|p| p.as_str())
                    .map(|p| comment(Some(p)))
                    .unwrap_or_default();
                let entry = Entry {
                    key: key.to_string(),
                    default: Value::from_toml(v),
                };
                match section.groups.last_mut() {
                    Some(group) if doc.is_empty() => group.entries.push(entry),
                    _ => section.groups.push(Group {
                        doc,
                        entries: vec![entry],
                    }),
                }
            }
            Item::Table(t) => {
                let full = if name.is_empty() {
                    key.to_string()
                } else {
                    format!("{name}.{key}")
                };
                subtables.push((t.position(), full, t));
            }
            _ => {}
        }
    }
    // Une section sans clé (`[keys]`, remplie par l'utilisateur) reste listée : l'interface
    // lui donne son propre volet.
    if !section.groups.is_empty() || (!name.is_empty() && subtables.is_empty()) {
        out.push(section);
    }
    // Sous-tables dans l'ordre de leur en-tête dans le fichier.
    subtables.sort_by_key(|(position, _, _)| *position);
    for (_, full, t) in subtables {
        collect(t, &full, out);
    }
}

/// Le fichier de configuration de l'utilisateur, en cours de modification.
#[derive(Clone, Debug)]
pub struct ConfigFile {
    /// Emplacement du fichier.
    pub path: PathBuf,
    doc: DocumentMut,
}

impl ConfigFile {
    /// Lit `path` ; absent : fichier vide (il sera créé à l'enregistrement).
    pub fn open(path: &Path) -> Result<ConfigFile, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("{} : {e}", path.display())),
        };
        let doc = text
            .parse()
            .map_err(|e: toml_edit::TomlError| format!("{} : {e}", path.display()))?;
        Ok(ConfigFile {
            path: path.to_path_buf(),
            doc,
        })
    }

    fn table(&self, section: &str) -> Option<&Table> {
        let mut table = self.doc.as_table();
        for part in section.split('.').filter(|p| !p.is_empty()) {
            table = table.get(part)?.as_table()?;
        }
        Some(table)
    }

    fn table_mut(&mut self, section: &str) -> Option<&mut Table> {
        let mut table = self.doc.as_table_mut();
        for part in section.split('.').filter(|p| !p.is_empty()) {
            if !table.contains_key(part) {
                let mut new = Table::new();
                // `[gui.colors]` sans `[gui]` vide au-dessus.
                new.set_implicit(true);
                table.insert(part, Item::Table(new));
            }
            table = table.get_mut(part)?.as_table_mut()?;
        }
        Some(table)
    }

    /// Valeur écrite dans le fichier pour `section`.`key` (`None` : absente, la valeur
    /// par défaut s'applique).
    pub fn get(&self, section: &str, key: &str) -> Option<Value> {
        self.table(section)?
            .get(key)?
            .as_value()
            .map(Value::from_toml)
    }

    /// Écrit `value` pour `section`.`key`, en gardant le commentaire et la mise en page
    /// d'une clé déjà présente. Erreur si un texte TOML est invalide.
    pub fn set(&mut self, section: &str, key: &str, value: &Value) -> Result<(), String> {
        let new = value
            .to_toml()
            .ok_or_else(|| crate::t!("config-invalid-toml", value = value.to_string()))?;
        let table = self
            .table_mut(section)
            .ok_or_else(|| crate::t!("config-not-a-section", section = section))?;
        if table.is_implicit() && !table.is_empty() {
            table.set_implicit(false);
        }
        match table.get_mut(key).and_then(Item::as_value_mut) {
            Some(old) => {
                let decor = old.decor().clone();
                *old = new;
                *old.decor_mut() = decor;
            }
            None => {
                table.insert(key, Item::Value(new));
                table.set_implicit(false);
            }
        }
        Ok(())
    }

    /// Retire `section`.`key` du fichier : la valeur par défaut s'applique de nouveau.
    pub fn remove(&mut self, section: &str, key: &str) {
        if let Some(table) = self.table_mut(section) {
            table.remove(key);
        }
    }

    /// Sous-sections écrites dans `section` (`tirage` pour `[export.profiles.tirage]`).
    pub fn subtables(&self, section: &str) -> Vec<String> {
        self.table(section)
            .map(|t| {
                t.iter()
                    .filter(|(_, item)| item.is_table())
                    .map(|(name, _)| name.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Retire la section `section` (avec ses clés) du fichier.
    pub fn remove_table(&mut self, section: &str) {
        let Some((parent, name)) = section.rsplit_once('.') else {
            self.doc.as_table_mut().remove(section);
            return;
        };
        if let Some(table) = self.table_mut(parent) {
            table.remove(name);
        }
    }

    /// Texte du fichier modifié.
    pub fn text(&self) -> String {
        self.doc.to_string()
    }

    /// Enregistre le fichier (écrit à côté puis renommé : jamais à moitié écrit).
    pub fn save(&self) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("toml.tri-tmp");
        std::fs::write(&tmp, self.text())?;
        std::fs::rename(&tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = "# En-tête du fichier.

[preview]
# Côté long des vignettes.
thumbnail = 400
# Badges : saturés,
# bouchés.
badge_over = 0.005
badge_under = 0.05
flag = true

[gui]
window = [1280.0, 800.0]

# Couleurs.
[gui.colors]
hover = \"#b4b4b4\"

[keys]
";

    #[test]
    fn the_schema_comes_from_the_example() {
        let s = Schema::parse(EXAMPLE).unwrap();
        let names: Vec<&str> = s.sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            ["preview", "gui", "gui.colors", "keys"],
            "[keys] vide : listée (son propre volet dans l'interface)"
        );
        let preview = &s.sections[0];
        assert_eq!(preview.doc, "", "l'en-tête du fichier n'est pas la section");
        assert_eq!(preview.groups.len(), 2);
        assert_eq!(preview.groups[1].doc, "Badges : saturés, bouchés.");
        assert_eq!(
            preview.groups[1].entries.len(),
            3,
            "sans commentaire : même groupe"
        );
        assert_eq!(s.sections[2].doc, "Couleurs.");
        assert_eq!(s.default_of("preview", "thumbnail"), Some(&Value::Int(400)));
        assert_eq!(
            s.default_of("gui", "window"),
            Some(&Value::Toml("[1280.0, 800.0]".into()))
        );
        assert!(s.default_of("gui.colors", "hover").unwrap().is_color());
        assert_eq!(parse_color("#ff8000"), Some([255, 128, 0]));
        assert_eq!(parse_color("ff8000"), None);
    }

    #[test]
    fn editing_keeps_comments_and_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let original =
            "# Mes réglages.\n[preview]\n# Plus grandes.\nthumbnail = 500 # à moi\nperso = 1\n";
        std::fs::write(&path, original).unwrap();
        let mut file = ConfigFile::open(&path).unwrap();
        assert_eq!(file.get("preview", "thumbnail"), Some(Value::Int(500)));
        assert_eq!(file.get("preview", "absent"), None);
        file.set("preview", "thumbnail", &Value::Int(600)).unwrap();
        file.set("gui.colors", "hover", &Value::Str("#101010".into()))
            .unwrap();
        file.set("gui", "window", &Value::Toml("[800.0, 600.0]".into()))
            .unwrap();
        assert!(
            file.set("gui", "window", &Value::Toml("[1,".into()))
                .is_err()
        );
        file.save().unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with(
                "# Mes réglages.\n[preview]\n# Plus grandes.\nthumbnail = 600 # à moi\nperso = 1\n"
            ),
            "{text}"
        );
        assert!(text.contains("[gui.colors]\nhover = \"#101010\""), "{text}");
        assert!(text.contains("window = [800.0, 600.0]"), "{text}");
        // Relu tel quel.
        let again = ConfigFile::open(&path).unwrap();
        assert_eq!(
            again.get("gui", "window"),
            Some(Value::Toml("[800.0, 600.0]".into()))
        );
        let mut file = again;
        file.remove("preview", "thumbnail");
        assert_eq!(file.get("preview", "thumbnail"), None);
        assert!(file.text().contains("perso = 1"));
    }

    #[test]
    fn a_section_without_keys_is_listed() {
        let schema =
            Schema::parse("[gui]\nx = 1\n\n[keys]\n# Raccourcis.\n# \"a\" = [\"b\"]\n").unwrap();
        let names: Vec<&str> = schema.sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["gui", "keys"]);
    }

    #[test]
    fn shortcuts_are_written_with_quoted_action_ids() {
        let mut file = ConfigFile::open(Path::new("/nonexistent/config.toml")).unwrap();
        let keys = Value::Strings(vec!["Maj+P".into(), "g \"g\"".into()]);
        file.set("keys", "core.flag.accept", &keys).unwrap();
        let text = file.text();
        assert!(text.contains("[keys]"), "{text}");
        assert!(
            text.contains(r#""core.flag.accept" = ["Maj+P", 'g "g"']"#),
            "{text}"
        );
        let doc: toml::Table = text.parse().unwrap();
        let list = doc["keys"]["core.flag.accept"].as_array().unwrap();
        assert_eq!(list[1].as_str(), Some("g \"g\""));
        assert!(file.get("keys", "core.flag.accept").is_some());
        assert_eq!(keys.to_string(), r#"["Maj+P", 'g "g"']"#);
    }

    #[test]
    fn a_missing_file_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sous/config.toml");
        let mut file = ConfigFile::open(&path).unwrap();
        file.set("files", "trash", &Value::Bool(false)).unwrap();
        file.save().unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "[files]\ntrash = false\n"
        );
    }

    /// Le vrai fichier d'exemple : toutes ses sections sont lues.
    #[test]
    fn the_real_example_parses() {
        let s = Schema::parse(include_str!("../doc/config.example.toml")).unwrap();
        assert!(s.sections.iter().any(|s| s.name == "gui.histogram"));
        assert!(s.default_of("files", "recent").is_some());
        let keys = s.sections.iter().find(|s| s.name == "keys").unwrap();
        assert!(keys.groups.is_empty());
    }
}
