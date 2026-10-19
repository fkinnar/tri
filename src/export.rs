//! Export d'images développées (19/10/2026) : JPEG, TIFF ou PNG, avec un **profil
//! d'export** par usage (« Retouche » : TIFF 16 bits en ProPhoto, vers un éditeur externe ;
//! « Web » : JPEG sRGB). Première étape de l'« après-développement » (voir `CLAUDE.md`).
//!
//! `tri` n'implémente aucun profil couleur ni format : **darktable fait tout**
//! (`darktable-cli`, déjà piloté pour les rendus développés, voir [`crate::develop`]). Un
//! profil n'est qu'une poignée d'options traduites en arguments de `darktable-cli`. Chaque
//! image est développée d'après son sidecar (`IMAGE.RAF.xmp`, historique et recadrage
//! compris) ; sans développement, avec les réglages par défaut de darktable.
//!
//! Toutes les images d'un export passent par **un seul lancement** de darktable (son
//! démarrage, ~6 s, n'est payé qu'une fois) ; darktable annonce chaque fichier écrit, ce
//! qui donne la progression. Rien n'est jamais écrasé (darktable ajoute `_01`…), aucun
//! sidecar n'est écrit. Voir `doc/export.md`.

use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;

use crate::develop::{Darktable, DevelopError};
use crate::media::{MediaKind, kind_of};

/// Espaces couleur proposés (liste déroulante des réglages), dans l'ordre de darktable.
pub const COLORS: [&str; 7] = [
    "",
    "srgb",
    "adobergb",
    "rec709",
    "rec2020",
    "prophoto",
    "display-p3",
];

/// Espaces qui n'ont de sens qu'en 16 bits ou plus (idée de l'utilisateur, 19/10/2026 :
/// « empêcher une andouille d'exporter du JPEG en ProPhoto ») : ProPhoto, très large, et
/// les espaces linéaires de darktable. En 8 bits, leurs dégradés se cassent en marches
/// (postérisation) ; sRGB, Adobe RGB et Display P3 restent sûrs en 8 bits.
pub const DEEP_COLORS: [&str; 3] = ["rec709", "rec2020", "prophoto"];

/// Espaces proposés en 8 bits (JPEG) : [`COLORS`] sans [`DEEP_COLORS`].
pub const COLORS_8BIT: [&str; 4] = ["", "srgb", "adobergb", "display-p3"];

/// `color` est-il permis à `bits` bits par canal ? Un profil `.icc` est laissé au jugement
/// de l'utilisateur.
pub fn color_fits(color: &str, bits: u8) -> bool {
    bits >= 16 || !DEEP_COLORS.contains(&color.trim().to_lowercase().as_str())
}

/// Intentions de rendu proposées.
pub const INTENTS: [&str; 5] = ["", "perceptual", "relative", "saturation", "absolute"];

/// Format du fichier écrit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// JPEG (8 bits).
    Jpeg,
    /// TIFF (8, 16 ou 32 bits).
    Tiff,
    /// PNG (8 ou 16 bits).
    Png,
}

impl Format {
    /// Extension des fichiers écrits.
    pub fn extension(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::Tiff => "tif",
            Format::Png => "png",
        }
    }
}

/// Profil d'export : ce qu'on produit pour un usage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    /// Identifiant (`[export.profiles.<id>]`).
    pub id: String,
    /// Nom affiché ; vide : nom traduit des profils fournis, sinon l'identifiant.
    pub name: String,
    /// Format.
    pub format: Format,
    /// Bits par canal : 8 (JPEG), 8 ou 16 (PNG), 8, 16 ou 32 (TIFF).
    pub bits: u8,
    /// Qualité JPEG (1 à 100).
    pub quality: u8,
    /// Espace couleur, ceux de la liste d'export de darktable (sans les variantes HDR) :
    /// `srgb`, `adobergb`, `prophoto` (linéaire, celui de darktable), `rec709` et
    /// `rec2020` (linéaires), `display-p3` ; ou le chemin d'un profil `.icc` / `.icm` ; ou
    /// `""` (« réglages de l'image » : module « profil de couleur de sortie »).
    pub color: String,
    /// Intention de rendu : `perceptual`, `relative`, `saturation`, `absolute`, ou `""`
    /// (celle du développement).
    pub intent: String,
    /// Côté long maximal en pixels ; 0 : pleine définition (jamais d'agrandissement).
    pub max_side: u32,
    /// Ajouté au nom du fichier (`DSCF0001-web.jpg`) ; vide : le nom de l'image.
    pub suffix: String,
    /// Dossier des fichiers écrits (idée de l'utilisateur, 19/10/2026 : propre au profil) :
    /// `/…` absolu, `~/…` depuis le dossier personnel, sinon **relatif au dossier de
    /// chaque image** (`"export"` : un sous-dossier à côté des images ; `""` ou `"."` :
    /// à côté d'elles). Les variables de darktable (`$(YEAR)`…) y sont permises.
    pub folder: String,
}

impl Profile {
    /// Profil de base d'un identifiant inconnu des profils fournis : JPEG sRGB.
    fn base(id: &str) -> Profile {
        Profile {
            id: id.into(),
            name: String::new(),
            format: Format::Jpeg,
            bits: 8,
            quality: 90,
            color: "srgb".into(),
            intent: String::new(),
            max_side: 0,
            suffix: String::new(),
            folder: "export".into(),
        }
    }

    /// Les profils fournis : les usages de l'utilisateur (19/10/2026), modifiables.
    pub fn defaults() -> Vec<Profile> {
        vec![
            Profile {
                format: Format::Tiff,
                bits: 16,
                color: "prophoto".into(),
                ..Profile::base("edit")
            },
            Profile {
                max_side: 2048,
                ..Profile::base("web")
            },
        ]
    }

    /// Le profil est-il cohérent ? Sinon, la raison.
    pub fn check(&self) -> Result<(), String> {
        let bits_ok = match self.format {
            Format::Jpeg => self.bits == 8,
            Format::Png => matches!(self.bits, 8 | 16),
            Format::Tiff => matches!(self.bits, 8 | 16 | 32),
        };
        if !bits_ok {
            return Err(crate::t!(
                "export-bad-bits",
                profile = self.id.as_str(),
                bits = self.bits,
                format = self.format.extension()
            ));
        }
        if !(1..=100).contains(&self.quality) {
            return Err(crate::t!(
                "export-bad-quality",
                profile = self.id.as_str(),
                quality = self.quality
            ));
        }
        color_args(&self.color).map_err(|value| {
            crate::t!(
                "export-bad-color",
                profile = self.id.as_str(),
                value = value
            )
        })?;
        if !color_fits(&self.color, self.bits) {
            return Err(crate::t!(
                "export-needs-16-bits",
                profile = self.id.as_str(),
                color = self.color.as_str()
            ));
        }
        intent_arg(&self.intent).map_err(|value| {
            crate::t!(
                "export-bad-intent",
                profile = self.id.as_str(),
                value = value
            )
        })?;
        Ok(())
    }

    /// Arguments de `darktable-cli` propres au profil (avant `--core`) et réglages de sa
    /// configuration (après, en `--conf`).
    fn darktable_args(&self) -> Result<(Vec<String>, Vec<String>), String> {
        self.check()?;
        let side = self.max_side.to_string();
        let mut args = vec![
            "--out-ext".into(),
            self.format.extension().into(),
            "--width".into(),
            side.clone(),
            "--height".into(),
            side,
            // Export : qualité d'abord (rendu complet puis réduction).
            "--hq".into(),
            "true".into(),
            "--upscale".into(),
            "false".into(),
        ];
        args.extend(color_args(&self.color).expect("vérifié"));
        if let Some(intent) = intent_arg(&self.intent).expect("vérifié") {
            args.extend(["--icc-intent".into(), intent.into()]);
        }
        let conf = match self.format {
            Format::Jpeg => vec![format!(
                "plugins/imageio/format/jpeg/quality={}",
                self.quality
            )],
            Format::Tiff => vec![format!("plugins/imageio/format/tiff/bpp={}", self.bits)],
            Format::Png => vec![format!("plugins/imageio/format/png/bpp={}", self.bits)],
        };
        Ok((args, conf))
    }
}

/// `--icc-type` (et `--icc-file`) pour un espace couleur ; `Err(valeur)` si inconnu.
fn color_args(color: &str) -> Result<Vec<String>, String> {
    let lower = color.trim().to_lowercase();
    let builtin = match lower.as_str() {
        "" => return Ok(Vec::new()),
        "srgb" => "SRGB",
        "adobergb" => "ADOBERGB",
        "prophoto" => "PROPHOTO_RGB",
        "rec709" => "LIN_REC709",
        "rec2020" => "LIN_REC2020",
        "display-p3" => "DISPLAY_P3",
        _ if lower.ends_with(".icc") || lower.ends_with(".icm") => {
            let file = crate::settings::expand_home(color.trim());
            return Ok(vec![
                "--icc-type".into(),
                "FILE".into(),
                "--icc-file".into(),
                file.display().to_string(),
            ]);
        }
        _ => return Err(color.into()),
    };
    Ok(vec!["--icc-type".into(), builtin.into()])
}

/// `--icc-intent` pour une intention ; `Ok(None)` : celle du développement.
fn intent_arg(intent: &str) -> Result<Option<&'static str>, String> {
    Ok(Some(match intent.trim().to_lowercase().as_str() {
        "" => return Ok(None),
        "perceptual" => "PERCEPTUAL",
        "relative" => "RELATIVE_COLORIMETRIC",
        "saturation" => "SATURATION",
        "absolute" => "ABSOLUTE_COLORIMETRIC",
        _ => return Err(intent.into()),
    }))
}

/// Un profil tel qu'écrit dans le fichier : chaque clé absente vient du profil fourni de
/// même identifiant (ou du profil de base) ; la fenêtre de réglages peut ainsi n'écrire
/// qu'une valeur.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ProfileToml {
    name: Option<String>,
    format: Option<Format>,
    bits: Option<u8>,
    quality: Option<u8>,
    color: Option<String>,
    intent: Option<String>,
    max_side: Option<u32>,
    suffix: Option<String>,
    folder: Option<String>,
}

/// Les profils d'export, dans l'ordre : profils fournis, puis ceux de l'utilisateur (par
/// identifiant).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profiles(pub Vec<Profile>);

impl Default for Profiles {
    fn default() -> Self {
        Profiles(Profile::defaults())
    }
}

impl<'de> Deserialize<'de> for Profiles {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let written = BTreeMap::<String, ProfileToml>::deserialize(d)?;
        let mut list = Profile::defaults();
        for (id, t) in written {
            let index = match list.iter().position(|p| p.id == id) {
                Some(i) => i,
                None => {
                    list.push(Profile::base(&id));
                    list.len() - 1
                }
            };
            let p = &mut list[index];
            if let Some(v) = t.name {
                p.name = v;
            }
            if let Some(v) = t.format {
                p.format = v;
                // Format changé sans profondeur : celle que le format accepte toujours.
                if t.bits.is_none() && v == Format::Jpeg {
                    p.bits = 8;
                }
            }
            if let Some(v) = t.bits {
                p.bits = v;
            }
            if let Some(v) = t.quality {
                p.quality = v;
            }
            if let Some(v) = t.color {
                p.color = v;
            }
            if let Some(v) = t.intent {
                p.intent = v;
            }
            if let Some(v) = t.max_side {
                p.max_side = v;
            }
            if let Some(v) = t.suffix {
                p.suffix = v;
            }
            if let Some(v) = t.folder {
                p.folder = v;
            }
        }
        // Profil incohérent (ProPhoto en JPEG…) : refusé dès la lecture, comme toute
        // valeur invalide (la fenêtre de réglages refuse la modification et dit pourquoi).
        for p in &list {
            p.check().map_err(serde::de::Error::custom)?;
        }
        Ok(Profiles(list))
    }
}

impl Profiles {
    /// Le profil `id`.
    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.0.iter().find(|p| p.id == id)
    }
}

/// Réglages (section `[export]`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExportSettings {
    /// Profils (`[export.profiles.<id>]`).
    pub profiles: Profiles,
}

/// `"."` (ou `"./"`) : à côté de l'image, comme `""`.
fn next_to(folder: &str) -> &str {
    match folder.trim() {
        "." | "./" | ".\\" => "",
        other => other,
    }
}

/// Dossier où `folder` (voir [`Profile::folder`]) envoie l'export de `image`.
pub fn folder_for(folder: &str, image: &Path) -> PathBuf {
    let path = crate::settings::expand_home(next_to(folder));
    if path.is_absolute() {
        path
    } else {
        image.parent().unwrap_or(Path::new("")).join(path)
    }
}

/// Motif de sortie pour darktable : dossier absolu, ou relatif au dossier de chaque image
/// (`$(FILE_FOLDER)`, vérifié le 19/10/2026 : darktable crée les sous-dossiers, accents
/// et espaces compris), puis `$(FILE_NAME)` (nom de l'image sans extension) et le
/// suffixe ; darktable ajoute l'extension.
fn output_pattern(folder: &str, suffix: &str) -> PathBuf {
    let name = format!("$(FILE_NAME){suffix}");
    let path = crate::settings::expand_home(next_to(folder));
    if path.is_absolute() {
        path.join(name)
    } else {
        PathBuf::from("$(FILE_FOLDER)").join(path).join(name)
    }
}

/// Images à exporter parmi `images` : sans doublon, et sans le JPEG d'une paire RAW+JPEG
/// dont le RAW est aussi demandé (même nom, même dossier) : le développement est celui du
/// RAW, et les deux donneraient le même fichier.
pub fn sources(images: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let unique: Vec<&PathBuf> = images.iter().filter(|p| seen.insert(*p)).collect();
    let raw_stems: HashSet<PathBuf> = unique
        .iter()
        .filter(|p| kind_of(p) == Some(MediaKind::Raw))
        .map(|p| p.with_extension(""))
        .collect();
    unique
        .into_iter()
        .filter(|p| {
            kind_of(p) != Some(MediaKind::Flat) || !raw_stems.contains(&p.with_extension(""))
        })
        .cloned()
        .collect()
}

/// Résultat d'un export.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Fichiers écrits, dans l'ordre.
    pub written: Vec<PathBuf>,
    /// Images demandées sans fichier écrit (illisibles par darktable…).
    pub failed: usize,
}

/// Exporte `images` (voir [`sources`]) avec `profile` dans `folder` (le dossier du
/// profil, ou un autre choisi pour cette fois ; voir [`Profile::folder`]), en un
/// lancement de darktable. `progress(fait, total, fichier)` à chaque fichier écrit ;
/// `cancel` arrête darktable (les fichiers déjà écrits restent).
pub fn export(
    dt: &Darktable,
    images: &[PathBuf],
    profile: &Profile,
    folder: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, usize, &Path),
) -> Result<Report, DevelopError> {
    let images = sources(images);
    if images.is_empty() {
        return Ok(Report::default());
    }
    let (args, conf) = profile.darktable_args().map_err(DevelopError::Failed)?;
    std::fs::create_dir_all(&dt.config_dir).map_err(|e| DevelopError::Failed(e.to_string()))?;
    let mut cmd = Command::new(&dt.cli);
    for image in &images {
        cmd.arg("--import").arg(image);
    }
    // darktable ajoute `_01`… plutôt que d'écraser.
    let pattern = output_pattern(folder, &profile.suffix);
    cmd.arg(pattern)
        .args(args)
        .args(["--apply-custom-presets", "false"])
        .arg("--core")
        .arg("--configdir")
        .arg(&dt.config_dir)
        .args(["--library", ":memory:"])
        // Jamais de sidecar écrit, quelle que soit la configuration de darktable.
        .args(["--conf", "write_sidecar_files=never"]);
    for c in conf {
        cmd.args(["--conf", &c]);
    }
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| DevelopError::Failed(e.to_string()))?;
    // Sorties lues dans des fils à part : darktable ne se bloque jamais sur un tuyau plein.
    let (tx, rx) = std::sync::mpsc::channel();
    let stdout = child.stdout.take().expect("stdout");
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(path) = exported_path(&line) {
                let _ = tx.send(path);
            }
        }
    });
    let mut stderr = child.stderr.take().expect("stderr");
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let total = images.len();
    let mut report = Report::default();
    let status = loop {
        for path in rx.try_iter() {
            report.written.push(path);
            progress(
                report.written.len(),
                total,
                report.written.last().expect("ajouté"),
            );
        }
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(DevelopError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(DevelopError::Failed(e.to_string())),
        }
    };
    let message = errors.join().unwrap_or_default();
    // Dernières lignes de la sortie, lues après la fin du programme.
    std::thread::sleep(Duration::from_millis(20));
    for path in rx.try_iter() {
        report.written.push(path);
        progress(
            report.written.len(),
            total,
            report.written.last().expect("ajouté"),
        );
    }
    if report.written.is_empty() && !status.success() {
        let last = message.lines().rev().find(|l| !l.trim().is_empty());
        return Err(DevelopError::Failed(
            last.unwrap_or("darktable-cli").trim().to_string(),
        ));
    }
    report.failed = total.saturating_sub(report.written.len());
    Ok(report)
}

/// Chemin d'une ligne « `[export_job] exported to `…'` » de darktable.
fn exported_path(line: &str) -> Option<PathBuf> {
    let rest = &line[line.find("exported to `")? + "exported to `".len()..];
    let end = rest.rfind('\'')?;
    Some(PathBuf::from(&rest[..end]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(profile: &Profile) -> Vec<String> {
        let (mut a, c) = profile.darktable_args().unwrap();
        a.extend(c);
        a
    }

    #[test]
    fn default_profiles_match_the_users_two_uses() {
        let p = Profiles::default();
        let edit = p.get("edit").unwrap();
        assert_eq!(
            (edit.format, edit.bits, edit.color.as_str()),
            (Format::Tiff, 16, "prophoto")
        );
        let a = args(edit);
        assert!(a.windows(2).any(|w| w == ["--icc-type", "PROPHOTO_RGB"]));
        assert!(a.contains(&"plugins/imageio/format/tiff/bpp=16".to_string()));
        assert!(a.windows(2).any(|w| w == ["--out-ext", "tif"]));
        let web = p.get("web").unwrap();
        let a = args(web);
        assert!(a.windows(2).any(|w| w == ["--icc-type", "SRGB"]));
        assert!(a.windows(2).any(|w| w == ["--width", "2048"]));
        assert!(a.contains(&"plugins/imageio/format/jpeg/quality=90".to_string()));
    }

    #[test]
    fn written_keys_complete_the_profile_of_the_same_id() {
        #[derive(Deserialize)]
        struct S {
            export: ExportSettings,
        }
        let s: S = toml::from_str(
            r#"
            [export.profiles.web]
            quality = 80
            [export.profiles.print]
            name = "Tirage"
            format = "tiff"
            bits = 8
            color = "~/profils/papier.icc"
            intent = "relative"
            "#,
        )
        .unwrap();
        let p = &s.export.profiles;
        let ids: Vec<&str> = p.0.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["edit", "web", "print"], "fournis d'abord, gardés");
        let web = p.get("web").unwrap();
        assert_eq!(
            (web.quality, web.max_side),
            (80, 2048),
            "le reste du profil fourni"
        );
        let print = p.get("print").unwrap();
        let a = args(print);
        assert!(a.windows(2).any(|w| w == ["--icc-type", "FILE"]));
        assert!(
            a.iter()
                .any(|x| x.ends_with("profils/papier.icc") && !x.starts_with('~'))
        );
        assert!(
            a.windows(2)
                .any(|w| w == ["--icc-intent", "RELATIVE_COLORIMETRIC"])
        );
    }

    #[test]
    fn incoherent_profiles_are_refused_with_a_reason() {
        let bad = [
            Profile {
                bits: 16,
                ..Profile::base("a")
            },
            Profile {
                quality: 0,
                ..Profile::base("b")
            },
            Profile {
                color: "pantone".into(),
                ..Profile::base("c")
            },
            Profile {
                intent: "joli".into(),
                ..Profile::base("d")
            },
            // ProPhoto en JPEG (8 bits) : refusé.
            Profile {
                color: "prophoto".into(),
                ..Profile::base("e")
            },
            Profile {
                format: Format::Tiff,
                color: "rec2020".into(),
                ..Profile::base("f")
            },
        ];
        for p in bad {
            let reason = p.check().unwrap_err();
            assert!(reason.contains(&p.id), "{reason}");
        }
        assert!(Profile::base("ok").check().is_ok());
    }

    #[test]
    fn prophoto_in_jpeg_is_refused_when_reading_the_settings() {
        #[derive(Debug, Deserialize)]
        struct S {
            #[allow(dead_code)]
            export: ExportSettings,
        }
        // Le profil « edit » (ProPhoto) passé en JPEG : refusé, raison donnée.
        let e = toml::from_str::<S>("[export.profiles.edit]\nformat = \"jpeg\"\n").unwrap_err();
        assert!(e.to_string().contains("16"), "{e}");
        // En TIFF 8 bits aussi ; en sRGB, le JPEG passe.
        assert!(toml::from_str::<S>("[export.profiles.edit]\nbits = 8\n").is_err());
        let ok = "[export.profiles.edit]\nformat = \"jpeg\"\ncolor = \"srgb\"\n";
        assert!(toml::from_str::<S>(ok).is_ok());
        assert!(color_fits("prophoto", 16) && !color_fits("ProPhoto", 8));
        assert!(color_fits("adobergb", 8) && color_fits("~/papier.icc", 8));
        assert!(
            COLORS_8BIT
                .iter()
                .all(|c| COLORS.contains(c) && color_fits(c, 8))
        );
    }

    #[test]
    fn a_pairs_jpeg_is_not_exported_twice() {
        let images: Vec<PathBuf> = [
            "a/DSCF1.RAF",
            "a/DSCF1.JPG",
            "a/DSCF2.JPG",
            "b/DSCF1.JPG",
            "a/DSCF1.RAF",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        assert_eq!(
            sources(&images),
            ["a/DSCF1.RAF", "a/DSCF2.JPG", "b/DSCF1.JPG"].map(PathBuf::from)
        );
    }

    #[test]
    fn the_folder_is_absolute_from_home_or_next_to_each_image() {
        let image = Path::new("/photos/séance été/DSCF0001.RAF");
        assert_eq!(
            folder_for("export", image),
            Path::new("/photos/séance été/export")
        );
        // À côté de la source : vide ou « . ».
        assert_eq!(folder_for("", image), Path::new("/photos/séance été"));
        assert_eq!(folder_for(".", image), Path::new("/photos/séance été"));
        assert_eq!(
            output_pattern(".", ""),
            Path::new("$(FILE_FOLDER)/$(FILE_NAME)")
        );
        assert_eq!(
            output_pattern("export/web", "-web"),
            Path::new("$(FILE_FOLDER)/export/web/$(FILE_NAME)-web")
        );
        #[cfg(not(windows))]
        {
            assert_eq!(folder_for("/exports", image), Path::new("/exports"));
            assert_eq!(
                output_pattern("/exports", ""),
                Path::new("/exports/$(FILE_NAME)")
            );
            let home = crate::settings::expand_home("~");
            assert_eq!(folder_for("~/Exports", image), home.join("Exports"));
        }
        // Dossier propre à chaque profil, complété comme les autres clés.
        #[derive(Deserialize)]
        struct S {
            export: ExportSettings,
        }
        let s: S = toml::from_str("[export.profiles.web]\nfolder = \"~/Web\"\n").unwrap();
        assert_eq!(s.export.profiles.get("web").unwrap().folder, "~/Web");
        assert_eq!(s.export.profiles.get("edit").unwrap().folder, "export");
    }

    #[test]
    fn darktable_progress_lines_are_read() {
        let line = "    18.2887 [export_job] exported to `/tmp/out/DSCF0331.tif'";
        assert_eq!(
            exported_path(line),
            Some(PathBuf::from("/tmp/out/DSCF0331.tif"))
        );
        assert_eq!(exported_path("[export] creating pixelpipe"), None);
    }

    /// Export réel : seulement si darktable est installé et `TRI_TEST_DARKTABLE_RAW`
    /// désigne un RAW développé (avec son sidecar), hors du dépôt.
    #[test]
    #[ignore]
    fn darktable_exports_a_tiff_and_a_jpeg() {
        let cli = crate::develop::find_darktable("auto").expect("darktable-cli installé");
        let original = PathBuf::from(std::env::var("TRI_TEST_DARKTABLE_RAW").expect("RAW de test"));
        // Sur une copie : jamais les fichiers de l'utilisateur.
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join(original.file_name().unwrap());
        std::fs::copy(&original, &raw).unwrap();
        let mut name = original.file_name().unwrap().to_os_string();
        name.push(".xmp");
        let sidecar = raw.with_file_name(&name);
        std::fs::copy(original.with_file_name(&name), &sidecar).unwrap();
        let before = std::fs::read(&sidecar).unwrap();
        let dt = Darktable {
            cli,
            config_dir: dir.path().join("cfg"),
            high_quality: true,
        };
        // Dossier du profil (relatif) : `export` à côté de l'image.
        let out = dir.path().join("export");
        let profiles = Profiles::default();
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();
        let tiff = export(
            &dt,
            std::slice::from_ref(&raw),
            profiles.get("edit").unwrap(),
            "export",
            &cancel,
            |d, t, p| seen.push((d, t, p.to_path_buf())),
        )
        .unwrap();
        assert_eq!(tiff.written.len(), 1);
        assert_eq!(seen.len(), 1, "progression : un fichier");
        assert_eq!(tiff.written[0].extension().unwrap(), "tif");
        assert_eq!(tiff.written[0].parent().unwrap(), out, "à côté de l'image");
        let web = export(
            &dt,
            &[raw],
            profiles.get("web").unwrap(),
            &out.display().to_string(),
            &cancel,
            |_, _, _| {},
        )
        .unwrap();
        let (w, h) = image::image_dimensions(&web.written[0]).unwrap();
        assert_eq!(w.max(h), 2048);
        assert_eq!(std::fs::read(&sidecar).unwrap(), before, "sidecar intact");
    }
}
