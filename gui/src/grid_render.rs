//! Rendu montré dans la grille (#114, choix de l'utilisateur du 26/10/2026). Au
//! déchargement et au tri, le JPEG boîtier suffit ; après le développement, on veut voir
//! les images développées. Une vignette n'a que deux états : JPEG boîtier, ou développée (le
//! développement du logiciel choisi, sinon le rendu LibRaw).

use std::path::Path;

use tri::cache::Level;
use tri::develop::{Engine, Preference};
use tri::media::{MediaFile, MediaKind};

/// Rendu des vignettes : `[gui.grid] render`, et la liste de la barre des filtres.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridRender {
    /// Le JPEG boîtier (vignette embarquée), comme au tri.
    #[default]
    Embedded,
    /// Le développement le plus récent de l'image, quel que soit le logiciel.
    Latest,
    /// Toujours ce logiciel.
    Engine(Engine),
    /// Toujours le rendu LibRaw.
    LibRaw,
}

impl GridRender {
    /// Identifiant du fichier de configuration.
    pub fn id(self) -> &'static str {
        match self {
            GridRender::Embedded => "embedded",
            GridRender::Latest => "latest",
            GridRender::Engine(e) => e.id(),
            GridRender::LibRaw => "libraw",
        }
    }

    pub fn from_id(id: &str) -> Option<GridRender> {
        match id {
            "embedded" => Some(GridRender::Embedded),
            "latest" => Some(GridRender::Latest),
            "libraw" => Some(GridRender::LibRaw),
            id => Engine::from_id(id).map(GridRender::Engine),
        }
    }

    /// Nom lisible, pour la liste.
    pub fn name(self) -> String {
        match self {
            GridRender::Embedded => t!("grid-render-embedded"),
            GridRender::Latest => t!("grid-render-latest"),
            GridRender::Engine(e) => e.name().to_string(),
            GridRender::LibRaw => t!("grid-render-libraw"),
        }
    }

    /// Choix proposés : les logiciels installés (`engines`) seulement ; la valeur en place
    /// `current` reste visible (un logiciel désinstallé depuis).
    pub fn choices(engines: &[Engine], current: GridRender) -> Vec<GridRender> {
        let mut list = vec![GridRender::Embedded];
        if !engines.is_empty() {
            list.push(GridRender::Latest);
        }
        list.extend(engines.iter().map(|&e| GridRender::Engine(e)));
        if let GridRender::Engine(e) = current
            && !engines.contains(&e)
        {
            list.push(current);
        }
        list.push(GridRender::LibRaw);
        list
    }

    /// Le choix suivant de la liste (`f d`), en boucle.
    pub fn next(self, engines: &[Engine]) -> GridRender {
        let list = GridRender::choices(engines, self);
        let at = list.iter().position(|&r| r == self).unwrap_or(0);
        list[(at + 1) % list.len()]
    }

    /// Logiciel du développement montré pour `image` : le plus récent, ou celui choisi
    /// s'il a développé l'image ; `None` : rendu LibRaw.
    pub fn engine_for(self, image: &Path, engines: &[Engine]) -> Option<Engine> {
        match self {
            GridRender::Latest => tri::develop::resolve(image, Preference::Auto, engines),
            GridRender::Engine(e) if engines.contains(&e) && e.sidecar(image).is_some() => Some(e),
            _ => None,
        }
    }

    /// Niveau du cache montré pour `file` : la vignette (JPEG boîtier, images plates), le
    /// développement choisi, ou à défaut le rendu LibRaw (aperçu standard). Lit les
    /// sidecars : l'appelant garde le résultat.
    pub fn level(self, file: &MediaFile, engines: &[Engine]) -> Level {
        if self == GridRender::Embedded || file.kind != MediaKind::Raw {
            return Level::Thumbnail;
        }
        self.engine_for(&file.path, engines)
            .map_or(Level::Standard, Level::Developed)
    }

    /// Vrai si les vignettes des RAW demandent leur rendu LibRaw (repli compris) : à
    /// calculer en fond pour tout le dossier.
    pub fn needs_libraw(self) -> bool {
        self != GridRender::Embedded
    }

    /// Logiciel des développements à rendre en fond (`[gui.grid] develop = "auto"`) :
    /// `Some(None)` chaque image dans celui de son dernier développement, `Some(Some(e))`
    /// toujours `e` ; `None` : rien à développer.
    pub fn to_develop(self) -> Option<Option<Engine>> {
        match self {
            GridRender::Latest => Some(None),
            GridRender::Engine(e) => Some(Some(e)),
            GridRender::Embedded | GridRender::LibRaw => None,
        }
    }
}

impl<'de> serde::Deserialize<'de> for GridRender {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let id = String::deserialize(d)?;
        GridRender::from_id(&id).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "« {id} » : embedded, latest, libraw ou un logiciel (darktable, rapidraw)"
            ))
        })
    }
}

impl serde::Serialize for GridRender {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.id())
    }
}

/// Développements manquants de la grille (`[gui.grid] develop`) : rendus en fond dès que
/// la grille les montre, ou seulement à la demande (Ctrl+Maj+B).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GridDevelop {
    #[default]
    Auto,
    Manual,
}

/// Ce que le lot de la grille rend en fond parmi `files` : les développements (leur
/// vignette est leur rendu, dans leur logiciel : `force` à `None`, « chacun dans le
/// sien ») toujours ; avec `develop` à `Auto` et un rendu `shown` développé, toutes les
/// images, dans le logiciel que `shown` impose (`Some(e)`) ou celui de leur dernier
/// développement (`None`).
pub fn to_render(
    files: &[MediaFile],
    develop: GridDevelop,
    shown: GridRender,
) -> (Vec<MediaFile>, Option<Engine>) {
    match shown.to_develop() {
        Some(force) if develop == GridDevelop::Auto => (files.to_vec(), force),
        _ => (
            files
                .iter()
                .filter(|f| tri::media::is_development(&f.path))
                .cloned()
                .collect(),
            None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: Engine = Engine::Darktable;
    const RR: Engine = Engine::RapidRaw;

    #[test]
    fn the_grid_always_renders_its_developments_and_the_photos_only_automatically() {
        let dir = tempfile::tempdir().unwrap();
        let file = |name: &str| MediaFile {
            path: dir.path().join(name),
            kind: MediaKind::Raw,
        };
        // Le clone, connu du registre (c'est lui qui fait d'un chemin un développement).
        tri::developments::set_for(
            &file("a.RAF").path,
            &[tri::developments::Development {
                engine: DT,
                copy: tri::developments::Copy::Clone(1),
                photo: file("a.RAF").path,
                sidecar: dir.path().join("a_01.RAF.xmp"),
            }],
        );
        let files = vec![file("a.RAF"), file("a@dt-01.RAF"), file("b.RAF")];
        // Développements seuls, chacun dans son logiciel : en manuel, ou en JPEG boîtier.
        for (develop, shown) in [
            (GridDevelop::Manual, GridRender::Latest),
            (GridDevelop::Manual, GridRender::Engine(DT)),
            (GridDevelop::Auto, GridRender::Embedded),
            (GridDevelop::Auto, GridRender::LibRaw),
        ] {
            let (to_do, force) = to_render(&files, develop, shown);
            assert_eq!(to_do, vec![file("a@dt-01.RAF")], "{develop:?} {shown:?}");
            assert_eq!(force, None);
        }
        // Automatique et rendu développé : tout le dossier, dans le logiciel du rendu.
        let (to_do, force) = to_render(&files, GridDevelop::Auto, GridRender::Engine(RR));
        assert_eq!(to_do.len(), 3);
        assert_eq!(force, Some(RR));
        assert_eq!(
            to_render(&files, GridDevelop::Auto, GridRender::Latest).1,
            None
        );
    }

    #[test]
    fn ids_round_trip_and_choices_follow_what_is_installed() {
        for r in [
            GridRender::Embedded,
            GridRender::Latest,
            GridRender::Engine(DT),
            GridRender::Engine(RR),
            GridRender::LibRaw,
        ] {
            assert_eq!(GridRender::from_id(r.id()), Some(r));
        }
        assert_eq!(GridRender::from_id("capture-one"), None);
        assert_eq!(
            GridRender::choices(&[], GridRender::Embedded),
            [GridRender::Embedded, GridRender::LibRaw]
        );
        assert_eq!(
            GridRender::choices(&[DT], GridRender::Engine(RR)),
            [
                GridRender::Embedded,
                GridRender::Latest,
                GridRender::Engine(DT),
                GridRender::Engine(RR),
                GridRender::LibRaw
            ],
            "la valeur en place reste visible"
        );
        assert_eq!(GridRender::LibRaw.next(&[DT]), GridRender::Embedded);
        assert_eq!(GridRender::Embedded.next(&[DT]), GridRender::Latest);
    }

    #[test]
    fn a_thumbnail_is_jpeg_or_developed_else_libraw() {
        let dir = tempfile::tempdir().unwrap();
        let raw = |name: &str| MediaFile {
            path: dir.path().join(name),
            kind: MediaKind::Raw,
        };
        let (a, b) = (raw("a.RAF"), raw("b.RAF"));
        std::fs::write(&a.path, b"raw").unwrap();
        std::fs::write(&b.path, b"raw").unwrap();
        // a : développé dans darktable (historique) ; b : rien.
        std::fs::write(
            dir.path().join("a.RAF.xmp"),
            r#"<rdf:Description darktable:history_end="3"/>"#,
        )
        .unwrap();
        let flat = MediaFile {
            path: dir.path().join("c.jpg"),
            kind: MediaKind::Flat,
        };
        let engines = [DT, RR];
        for r in [GridRender::Latest, GridRender::Engine(DT)] {
            assert_eq!(r.level(&a, &engines), Level::Developed(DT), "{r:?}");
            assert_eq!(r.level(&b, &engines), Level::Standard, "{r:?}");
        }
        // RapidRAW n'a pas développé a : rendu LibRaw.
        assert_eq!(GridRender::Engine(RR).level(&a, &engines), Level::Standard);
        // darktable pas installé : rendu LibRaw aussi.
        assert_eq!(GridRender::Engine(DT).level(&a, &[RR]), Level::Standard);
        assert_eq!(GridRender::LibRaw.level(&a, &engines), Level::Standard);
        assert_eq!(GridRender::Embedded.level(&a, &engines), Level::Thumbnail);
        assert_eq!(GridRender::Latest.level(&flat, &engines), Level::Thumbnail);
    }
}
