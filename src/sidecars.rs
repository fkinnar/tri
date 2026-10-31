//! Formats de sidecars de notes (31/10/2026, demandé par l'utilisateur) : les notes
//! d'une photo vont dans **tous les sidecars que l'on connaît** pour elle, chaque logiciel
//! lisant le sien. Un format = un `trait` ([`Format`]) et une liste ([`formats`]) : le XMP
//! ([`crate::xmp`], lu par darktable, Lightroom, Capture One, digiKam…) et le `.rrdata` de
//! RapidRAW ([`crate::rrdata`]) ; un prochain logiciel à sidecar propre (RawTherapee,
//! `.pp3`) s'ajoute ici, avec sa section de réglages. Voir `doc/en/sidecars.md`.
//!
//! Un sidecar **existant** est toujours écrit. Un sidecar **manquant** est créé selon la
//! section `[sidecars.<logiciel>]` ([`Create`]) : jamais, toujours, ou (`auto`) quand le
//! logiciel est installé ou que le dossier en contient déjà. Le XMP est toujours créé
//! (convention `[xmp] sidecar`). À la demande, [`crate::notes::NoteBook::generate`] crée
//! ceux d'un logiciel pour des images choisies (`tri --sidecars`).

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::develop::Engine;
use crate::xmp::{Convention, Notes, SidecarIndex};

/// Un format de sidecar de notes.
pub trait Format: Send + Sync {
    /// Identifiant (`"xmp"`, `"rrdata"`), nom de la ligne de commande.
    fn id(&self) -> &'static str;
    /// Logiciel qui le lit, s'il n'y en a qu'un (le XMP est partagé par plusieurs).
    fn engine(&self) -> Option<Engine>;
    /// Vrai si `sidecar` est de ce format (d'après son nom).
    fn owns(&self, sidecar: &Path) -> bool;
    /// Sidecar existant de ce format pour la photo `image`, d'après l'index du dossier.
    fn existing(&self, image: &Path, index: &SidecarIndex) -> Option<PathBuf>;
    /// Où créer le sidecar de `image` quand il manque.
    fn new_path(&self, image: &Path, convention: Convention) -> PathBuf;
    /// Les notes du sidecar `sidecar`.
    fn read(&self, sidecar: &Path) -> Result<Notes, String>;
    /// Écrit `notes` dans le sidecar existant `sidecar`, le reste gardé ; vrai si le
    /// fichier a changé.
    fn write(&self, sidecar: &Path, notes: &Notes) -> Result<bool, String>;
    /// Crée le sidecar `sidecar` avec `notes` (rien d'autre dedans).
    fn create(&self, sidecar: &Path, notes: &Notes) -> Result<(), String>;
}

/// Les sidecars XMP (`IMG.RAF.xmp` ou `IMG.xmp`).
pub struct Xmp;

impl Format for Xmp {
    fn id(&self) -> &'static str {
        "xmp"
    }
    fn engine(&self) -> Option<Engine> {
        None
    }
    fn owns(&self, sidecar: &Path) -> bool {
        sidecar
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("xmp"))
    }
    fn existing(&self, image: &Path, index: &SidecarIndex) -> Option<PathBuf> {
        index.existing_sidecar(image)
    }
    fn new_path(&self, image: &Path, convention: Convention) -> PathBuf {
        convention.sidecar(image)
    }
    fn read(&self, sidecar: &Path) -> Result<Notes, String> {
        crate::xmp::read_sidecar(sidecar).map_err(|e| e.to_string())
    }
    fn write(&self, sidecar: &Path, notes: &Notes) -> Result<bool, String> {
        crate::xmp::write_sidecar(sidecar, notes).map_err(|e| e.to_string())
    }
    fn create(&self, sidecar: &Path, notes: &Notes) -> Result<(), String> {
        crate::xmp::write_sidecar(sidecar, notes)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// Les sidecars de RapidRAW (`IMG.RAF.rrdata`).
pub struct Rrdata;

impl Format for Rrdata {
    fn id(&self) -> &'static str {
        "rrdata"
    }
    fn engine(&self) -> Option<Engine> {
        Some(Engine::RapidRaw)
    }
    fn owns(&self, sidecar: &Path) -> bool {
        sidecar
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("rrdata"))
    }
    fn existing(&self, image: &Path, index: &SidecarIndex) -> Option<PathBuf> {
        index.setting_file(image, ".rrdata")
    }
    fn new_path(&self, image: &Path, _convention: Convention) -> PathBuf {
        crate::rrdata::path(image)
    }
    fn read(&self, sidecar: &Path) -> Result<Notes, String> {
        crate::rrdata::read(sidecar).map_err(|e| e.to_string())
    }
    fn write(&self, sidecar: &Path, notes: &Notes) -> Result<bool, String> {
        crate::rrdata::write(sidecar, notes).map_err(|e| e.to_string())
    }
    fn create(&self, sidecar: &Path, notes: &Notes) -> Result<(), String> {
        crate::rrdata::create(sidecar, notes).map_err(|e| e.to_string())
    }
}

/// Tous les formats, le XMP d'abord (celui que `tri` lit et écrit toujours).
pub fn formats() -> &'static [&'static dyn Format] {
    static ALL: [&dyn Format; 2] = [&Xmp, &Rrdata];
    &ALL
}

/// Le format d'identifiant `id` (`"rrdata"`), ou celui du logiciel d'identifiant `id`
/// (`"rapidraw"`, le nom de sa section des réglages et de la ligne de commande).
pub fn by_id(id: &str) -> Option<&'static dyn Format> {
    formats()
        .iter()
        .copied()
        .find(|f| f.id() == id || f.engine().is_some_and(|e| e.id() == id))
}

/// Le format du sidecar `sidecar` (d'après son nom) ; le XMP à défaut.
pub fn of(sidecar: &Path) -> &'static dyn Format {
    formats()
        .iter()
        .copied()
        .find(|f| f.owns(sidecar))
        .unwrap_or(&Xmp)
}

/// Quand créer le sidecar manquant d'un logiciel (`[sidecars.<logiciel>] create`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Create {
    /// Quand le logiciel est installé, ou que le dossier contient déjà de ses sidecars.
    #[default]
    Auto,
    /// Toujours.
    Always,
    /// Jamais (à la demande seulement, `tri --sidecars`).
    Never,
}

/// Réglages d'un format (`[sidecars.<logiciel>]`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FormatSettings {
    /// Création du sidecar manquant quand on note.
    pub create: Create,
}

/// Section `[sidecars]` : une sous-section par logiciel à sidecar propre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SidecarSettings {
    /// RapidRAW (`.rrdata`).
    pub rapidraw: FormatSettings,
}

impl SidecarSettings {
    /// Réglages du format `format` (le XMP n'en a pas : toujours créé).
    fn of(&self, format: &dyn Format) -> Option<FormatSettings> {
        match format.engine()? {
            Engine::RapidRaw => Some(self.rapidraw),
            Engine::Darktable => None,
        }
    }

    /// Identifiants des formats à créer quand on note : le XMP, et chaque format selon
    /// son réglage, `installed` (logiciels présents sur la machine) et `in_folder`
    /// (le dossier en contient déjà).
    pub fn creates(
        &self,
        installed: &[Engine],
        in_folder: impl Fn(&dyn Format) -> bool,
    ) -> Vec<&'static str> {
        formats()
            .iter()
            .copied()
            .filter(|f| match self.of(*f) {
                None => true,
                Some(s) => match s.create {
                    Create::Always => true,
                    Create::Never => false,
                    Create::Auto => {
                        f.engine().is_some_and(|e| installed.contains(&e)) || in_folder(*f)
                    }
                },
            })
            .map(|f| f.id())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_are_told_from_the_sidecar_name_and_created_by_policy() {
        assert_eq!(of(Path::new("/p/IMG.RAF.xmp")).id(), "xmp");
        assert_eq!(of(Path::new("/p/IMG.RAF.rrdata")).id(), "rrdata");
        assert_eq!(of(Path::new("/p/IMG.RAF.RRDATA")).id(), "rrdata");
        assert_eq!(by_id("rapidraw").map(|f| f.id()), Some("rrdata"));
        assert_eq!(by_id("rrdata").map(|f| f.id()), Some("rrdata"));
        assert_eq!(by_id("xmp").map(|f| f.id()), Some("xmp"));
        assert!(by_id("lightroom").is_none());
        assert!(by_id("pp3").is_none());
        let auto = SidecarSettings::default();
        assert_eq!(
            auto.creates(&[], |_| false),
            ["xmp"],
            "rien d'installé, rien dans le dossier"
        );
        assert_eq!(
            auto.creates(&[Engine::RapidRaw], |_| false),
            ["xmp", "rrdata"]
        );
        assert_eq!(auto.creates(&[], |f| f.id() == "rrdata"), ["xmp", "rrdata"]);
        let never = SidecarSettings {
            rapidraw: FormatSettings {
                create: Create::Never,
            },
        };
        assert_eq!(never.creates(&[Engine::RapidRaw], |_| true), ["xmp"]);
        let always = SidecarSettings {
            rapidraw: FormatSettings {
                create: Create::Always,
            },
        };
        assert_eq!(always.creates(&[], |_| false), ["xmp", "rrdata"]);
    }
}
