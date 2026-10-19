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
use std::process::Stdio;
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
    /// Valeur dans le fichier de configuration (`"jpeg"`, `"tiff"`, `"png"`).
    pub fn key(self) -> &'static str {
        match self {
            Format::Jpeg => "jpeg",
            Format::Tiff => "tiff",
            Format::Png => "png",
        }
    }

    /// Extension des fichiers écrits.
    pub fn extension(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::Tiff => "tif",
            Format::Png => "png",
        }
    }
}

/// Accentuation de sortie (idée de l'utilisateur, 19/10/2026, comme Lightroom) : compense
/// la douceur due à la réduction (écran) ou à l'encre (tirage). Appliquée après la
/// réduction ; paramètres dans [`Sharpening`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sharpen {
    /// Aucune.
    #[default]
    None,
    /// Pour l'écran : légère et fine.
    Screen,
    /// Pour le tirage : rayon plus large.
    Print,
}

impl Sharpen {
    /// Valeur dans le fichier de configuration.
    pub fn key(self) -> &'static str {
        match self {
            Sharpen::None => "none",
            Sharpen::Screen => "screen",
            Sharpen::Print => "print",
        }
    }
}

/// Paramètres d'un masque flou (accentuation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unsharp {
    /// Rayon (écart type du flou gaussien), en pixels de l'image écrite.
    pub radius: f32,
    /// Force : 1,0 ajoute toute la différence avec l'image floue.
    pub amount: f32,
    /// Seuil, en niveaux sur 8 bits : en dessous, rien (pas de bruit accentué).
    pub threshold: f32,
}

/// Réglages de l'accentuation (section `[export.sharpening]`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Sharpening {
    /// Écran : rayon (pixels).
    pub screen_radius: f32,
    /// Écran : force.
    pub screen_amount: f32,
    /// Écran : seuil (niveaux sur 8 bits).
    pub screen_threshold: f32,
    /// Tirage : rayon (pixels).
    pub print_radius: f32,
    /// Tirage : force.
    pub print_amount: f32,
    /// Tirage : seuil (niveaux sur 8 bits).
    pub print_threshold: f32,
}

impl Default for Sharpening {
    fn default() -> Self {
        Self {
            screen_radius: 0.6,
            screen_amount: 0.6,
            screen_threshold: 2.0,
            print_radius: 1.2,
            print_amount: 0.9,
            print_threshold: 3.0,
        }
    }
}

impl Eq for Sharpening {}

impl Sharpening {
    /// Paramètres de `sharpen` ; `None` : pas d'accentuation.
    pub fn of(&self, sharpen: Sharpen) -> Option<Unsharp> {
        match sharpen {
            Sharpen::None => None,
            Sharpen::Screen => Some(Unsharp {
                radius: self.screen_radius,
                amount: self.screen_amount,
                threshold: self.screen_threshold,
            }),
            Sharpen::Print => Some(Unsharp {
                radius: self.print_radius,
                amount: self.print_amount,
                threshold: self.print_threshold,
            }),
        }
    }
}

/// Accentue `pixels` (masque flou) : `original + force × (original − flou)`, là où la
/// différence dépasse le seuil.
pub fn unsharp(pixels: &crate::raw::Rgb16, u: Unsharp) -> crate::raw::Rgb16 {
    if u.radius <= 0.0 || u.amount <= 0.0 {
        return pixels.clone();
    }
    let blurred = image::imageops::blur(pixels, u.radius);
    let threshold = u.threshold.max(0.0) * 257.0;
    let mut out = pixels.clone();
    for (o, b) in out.iter_mut().zip(blurred.iter()) {
        let d = f32::from(*o) - f32::from(*b);
        if d.abs() > threshold {
            *o = (f32::from(*o) + u.amount * d).round().clamp(0.0, 65535.0) as u16;
        }
    }
    out
}

/// Ce à quoi un profil s'applique (idée de l'utilisateur, 19/10/2026).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppliesTo {
    /// Partout.
    #[default]
    All,
    /// Au rendu d'un RAW (darktable, LibRaw).
    Raw,
    /// À une image déjà développée : JPEG boîtier, JPEG, TIFF… (« JPEG tel quel » : un RAW
    /// n'a pas d'espace couleur, son rendu doit en recevoir un).
    Flat,
}

impl AppliesTo {
    /// Valeur dans le fichier de configuration.
    pub fn key(self) -> &'static str {
        match self {
            AppliesTo::All => "all",
            AppliesTo::Raw => "raw",
            AppliesTo::Flat => "flat",
        }
    }

    /// Le profil vaut-il pour un export de RAW rendus (`flat` faux) ou d'images déjà
    /// développées (`flat` vrai) ?
    pub fn allows(self, flat: bool) -> bool {
        match self {
            AppliesTo::All => true,
            AppliesTo::Raw => !flat,
            AppliesTo::Flat => flat,
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
    /// JPEG **sans recompression** quand c'est possible (idée de l'utilisateur,
    /// 19/10/2026) : un JPEG d'origine copié tel quel, un JPEG boîtier écrit tel quel
    /// (orientation posée dans l'EXIF) ; une image à réduire est réencodée. Sans effet sur
    /// un rendu (darktable, LibRaw) ni hors JPEG.
    pub passthrough: bool,
    /// Ce à quoi il s'applique : `"all"`, `"raw"` (rendu d'un RAW), `"flat"` (image déjà
    /// développée) ; la boîte d'export ne propose que les profils qui conviennent.
    pub applies_to: AppliesTo,
    /// Accentuation de sortie : `"none"`, `"screen"`, `"print"`.
    pub sharpen: Sharpen,
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
            // Retour de l'utilisateur (19/10/2026) : par défaut, ne jamais recompresser un
            // JPEG quand on peut l'éviter.
            passthrough: true,
            applies_to: AppliesTo::All,
            sharpen: Sharpen::None,
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
            // « JPEG tel quel » (demandé le 19/10/2026) : un JPEG d'origine est copié sans
            // rien toucher (ni taille, ni espace couleur : un JPEG Adobe RGB le reste) ; un
            // RAW sort en JPEG pleine taille, dans l'espace du développement.
            Profile {
                quality: 100,
                color: String::new(),
                applies_to: AppliesTo::Flat,
                ..Profile::base("asis")
            },
            Profile {
                max_side: 2048,
                sharpen: Sharpen::Screen,
                ..Profile::base("web")
            },
        ]
    }

    /// Profil « tel quel » : un JPEG copié sans rien toucher (sans recompression, espace
    /// d'origine, pleine taille) ; sa qualité affichée n'a pas de sens.
    pub fn is_as_is(&self) -> bool {
        self.format == Format::Jpeg
            && self.passthrough
            && self.color.trim().is_empty()
            && self.max_side == 0
            && self.sharpen == Sharpen::None
    }

    /// Les clés du profil telles qu'écrites dans `[export.profiles.<id>]` (pour en créer
    /// un depuis l'interface, toutes ses valeurs explicites).
    pub fn entries(&self) -> Vec<(&'static str, crate::config_edit::Value)> {
        use crate::config_edit::Value;
        vec![
            ("name", Value::Str(self.name.clone())),
            ("format", Value::Str(self.format.key().into())),
            ("bits", Value::Int(i64::from(self.bits))),
            ("quality", Value::Int(i64::from(self.quality))),
            ("color", Value::Str(self.color.clone())),
            ("intent", Value::Str(self.intent.clone())),
            ("max_side", Value::Int(i64::from(self.max_side))),
            ("suffix", Value::Str(self.suffix.clone())),
            ("folder", Value::Str(self.folder.clone())),
            ("passthrough", Value::Bool(self.passthrough)),
            ("applies_to", Value::Str(self.applies_to.key().into())),
            ("sharpen", Value::Str(self.sharpen.key().into())),
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
    passthrough: Option<bool>,
    applies_to: Option<AppliesTo>,
    sharpen: Option<Sharpen>,
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
            if let Some(v) = t.passthrough {
                p.passthrough = v;
            }
            if let Some(v) = t.applies_to {
                p.applies_to = v;
            }
            if let Some(v) = t.sharpen {
                p.sharpen = v;
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
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExportSettings {
    /// Profil proposé pour un rendu de RAW (identifiant).
    pub default_raw: String,
    /// Profil proposé pour une image déjà développée (JPEG boîtier, JPEG…).
    pub default_flat: String,
    /// Ajouté au nom du JPEG d'une paire exporté avec son RAW (`DSCF0001-camera.jpg`).
    pub camera_suffix: String,
    /// Accentuation de sortie (`[export.sharpening]`).
    pub sharpening: Sharpening,
    /// Profils (`[export.profiles.<id>]`).
    pub profiles: Profiles,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            default_raw: "edit".into(),
            default_flat: "asis".into(),
            camera_suffix: "-camera".into(),
            sharpening: Sharpening::default(),
            profiles: Profiles::default(),
        }
    }
}

impl ExportSettings {
    /// Les profils qui s'appliquent à un export d'images déjà développées (`flat`) ou de
    /// rendus de RAW, dans l'ordre (voir [`Profile::applies_to`]).
    pub fn applicable(&self, flat: bool) -> Vec<&Profile> {
        self.profiles
            .0
            .iter()
            .filter(|p| p.applies_to.allows(flat))
            .collect()
    }

    /// Position, dans [`ExportSettings::applicable`], du profil proposé par défaut
    /// (`default_flat` ou `default_raw`) ; le premier s'il est inconnu ou ne s'applique pas.
    pub fn default_index(&self, flat: bool) -> usize {
        let id = if flat {
            &self.default_flat
        } else {
            &self.default_raw
        };
        self.applicable(flat)
            .iter()
            .position(|p| p.id == *id)
            .unwrap_or(0)
    }
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
    /// Raisons connues des échecs (export LibRaw : image par image).
    pub errors: Vec<(PathBuf, String)>,
    /// Fichiers écrits sans recompression (voir [`Profile::passthrough`]).
    pub passthrough: usize,
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
    let mut cmd = dt.cli.command();
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

// --- Sans darktable : LibRaw, ou le JPEG boîtier -----------------------------------------

/// Image exportée sans darktable : on exporte ce qu'on regarde (idée de l'utilisateur,
/// 19/10/2026).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Local {
    /// Rendu de base par LibRaw (la touche `b`, « rendu RAW »).
    LibRaw,
    /// Le JPEG embarqué du RAW (« JPEG boîtier »), à sa taille.
    Embedded,
}

/// Le profil peut-il être exporté **sans darktable** ? Sinon, la raison : pas de fichier
/// `.icc` ni de Display P3 (LibRaw ne les produit pas), pas de TIFF 32 bits.
pub fn libraw_check(profile: &Profile, _local: Local) -> Result<(), String> {
    profile.check()?;
    if libraw_space(&profile.color).is_none() {
        return Err(crate::t!(
            "export-libraw-color",
            profile = profile.id.as_str(),
            color = profile.color.as_str()
        ));
    }
    if profile.bits > 16 {
        return Err(crate::t!(
            "export-libraw-bits",
            profile = profile.id.as_str()
        ));
    }
    Ok(())
}

/// Espace de sortie LibRaw et profil ICC (généré par `moxcms`) d'un espace de profil.
/// Mêmes espaces que darktable : ProPhoto, Rec709 et Rec2020 **linéaires** ; sRGB et Adobe
/// RGB avec leur courbe. `""` (celui du développement) : sRGB, l'espace des aperçus.
fn libraw_space(color: &str) -> Option<(crate::raw::OutputSpace, moxcms::ColorProfile)> {
    use crate::raw::OutputSpace;
    use moxcms::{ColorProfile, ToneReprCurve};
    let linear = |mut p: ColorProfile, name: &str| {
        // Courbe vide : identité (norme ICC, `curv` sans entrée).
        p.red_trc = Some(ToneReprCurve::Lut(Vec::new()));
        p.green_trc = Some(ToneReprCurve::Lut(Vec::new()));
        p.blue_trc = Some(ToneReprCurve::Lut(Vec::new()));
        p.description = Some(moxcms::ProfileText::PlainString(name.into()));
        p
    };
    let flat = (1.0, 1.0);
    Some(match color.trim().to_lowercase().as_str() {
        "" | "srgb" => (
            OutputSpace {
                color: 1,
                gamma: (1.0 / 2.4, 12.92),
            },
            ColorProfile::new_srgb(),
        ),
        "adobergb" => (
            OutputSpace {
                color: 2,
                gamma: (1.0 / 2.199_218_8, 0.0),
            },
            ColorProfile::new_adobe_rgb(),
        ),
        "prophoto" => (
            OutputSpace {
                color: 4,
                gamma: flat,
            },
            linear(ColorProfile::new_pro_photo_rgb(), "Linear ProPhoto RGB"),
        ),
        "rec709" => (
            OutputSpace {
                color: 1,
                gamma: flat,
            },
            linear(ColorProfile::new_srgb(), "Linear Rec709 RGB"),
        ),
        "rec2020" => (
            OutputSpace {
                color: 8,
                gamma: flat,
            },
            linear(ColorProfile::new_bt2020(), "Linear Rec2020 RGB"),
        ),
        _ => return None,
    })
}

/// Espace couleur d'origine d'une image déjà développée (JPEG boîtier, image plate) :
/// remarque de l'utilisateur (19/10/2026), un JPEG boîtier peut être en Adobe RGB (réglage
/// du boîtier), il ne faut pas le supposer sRGB.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Origin {
    Srgb,
    AdobeRgb,
    /// Profil ICC intégré (autre que sRGB ou Adobe RGB reconnus).
    Icc(Vec<u8>),
}

impl Origin {
    /// D'un JPEG : son profil ICC intégré, sinon l'EXIF (Adobe RGB selon la norme DCF :
    /// `InteroperabilityIndex` « R03 », ou `ColorSpace` 2 de certains boîtiers), sinon sRGB
    /// (l'espace par défaut de l'EXIF).
    fn of_jpeg(bytes: &[u8]) -> Origin {
        if let Some(icc) = crate::jpeg::icc_profile(bytes) {
            return Origin::of_icc(icc);
        }
        let exif = exif::Reader::new().read_from_container(&mut std::io::Cursor::new(bytes));
        let adobe = exif.is_ok_and(|e| {
            let interop = e
                .get_field(exif::Tag::InteroperabilityIndex, exif::In::PRIMARY)
                .is_some_and(|f| f.display_value().to_string().contains("R03"));
            let space = e
                .get_field(exif::Tag::ColorSpace, exif::In::PRIMARY)
                .and_then(|f| f.value.get_uint(0));
            interop || space == Some(2)
        });
        if adobe {
            Origin::AdobeRgb
        } else {
            Origin::Srgb
        }
    }

    /// D'un profil ICC : sRGB et Adobe RGB reconnus à leur description (ASCII ou UTF-16).
    fn of_icc(icc: Vec<u8>) -> Origin {
        let has = |text: &str| {
            let ascii = text.as_bytes();
            let wide: Vec<u8> = text.bytes().flat_map(|b| [0, b]).collect();
            icc.windows(ascii.len()).any(|w| w == ascii)
                || icc.windows(wide.len()).any(|w| w == wide.as_slice())
        };
        if has("sRGB") {
            Origin::Srgb
        } else if has("Adobe RGB") {
            Origin::AdobeRgb
        } else {
            Origin::Icc(icc)
        }
    }

    /// D'un fichier image (plate) : JPEG comme ci-dessus ; autre format : son profil ICC
    /// intégré, sinon sRGB.
    fn of_file(bytes: &[u8]) -> Origin {
        if crate::jpeg::dimensions(bytes).is_some() {
            return Origin::of_jpeg(bytes);
        }
        use image::ImageDecoder;
        image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_decoder().ok())
            .and_then(|mut d| d.icc_profile().ok().flatten())
            .map_or(Origin::Srgb, Origin::of_icc)
    }

    /// Le profil d'export `color` est-il déjà cet espace (rien à convertir) ? `""` : oui
    /// (l'espace d'origine est gardé).
    fn is(&self, color: &str) -> bool {
        match color.trim().to_lowercase().as_str() {
            "" => true,
            "srgb" => *self == Origin::Srgb,
            "adobergb" => *self == Origin::AdobeRgb,
            _ => false,
        }
    }

    fn profile(&self) -> Option<moxcms::ColorProfile> {
        match self {
            Origin::Srgb => Some(moxcms::ColorProfile::new_srgb()),
            Origin::AdobeRgb => Some(moxcms::ColorProfile::new_adobe_rgb()),
            Origin::Icc(icc) => moxcms::ColorProfile::new_from_slice(icc).ok(),
        }
    }

    /// Profil ICC à intégrer quand on garde cet espace.
    fn icc(&self) -> Option<Vec<u8>> {
        match self {
            Origin::Icc(icc) => Some(icc.clone()),
            other => other.profile()?.encode().ok(),
        }
    }
}

/// Image déjà développée à exporter (JPEG boîtier d'un RAW, ou image plate) : ses pixels
/// (orientation appliquée), son espace d'origine, et les octets JPEG à copier tels quels
/// si c'en est un (orientation du RAW posée dans l'EXIF pour un JPEG boîtier).
struct Developed {
    pixels: crate::raw::Rgb16,
    origin: Origin,
    jpeg: Option<Vec<u8>>,
}

fn developed(image: &Path) -> Result<Developed, DevelopError> {
    let failed = |e: &dyn std::fmt::Display| DevelopError::Other(e.to_string());
    let to16 = |bmp: crate::bitmap::Bitmap| {
        let data = bmp.data.iter().map(|&v| u16::from(v) * 257).collect();
        crate::raw::Rgb16::from_raw(bmp.width, bmp.height, data)
            .ok_or_else(|| DevelopError::Other("image".into()))
    };
    if kind_of(image) == Some(MediaKind::Raw) {
        let mut raw = crate::raw::RawFile::open(image).map_err(|e| failed(&e))?;
        let rotation = raw.orientation();
        let crate::raw::Thumbnail::Jpeg(bytes) = raw.thumbnail().map_err(|e| failed(&e))? else {
            // Aperçu non compressé (rare) : pas d'espace connu, sRGB.
            let bmp = crate::preview::embedded_full(image).map_err(|e| failed(&e))?;
            return Ok(Developed {
                pixels: to16(bmp)?,
                origin: Origin::Srgb,
                jpeg: None,
            });
        };
        let exif_orientation = match rotation {
            crate::bitmap::Rotation::None => 1,
            crate::bitmap::Rotation::Cw90 => 6,
            crate::bitmap::Rotation::Cw180 => 3,
            crate::bitmap::Rotation::Cw270 => 8,
        };
        let bmp = crate::jpeg::decode(&bytes, 0).map_err(|e| failed(&e))?;
        return Ok(Developed {
            pixels: to16(bmp.rotate(rotation))?,
            origin: Origin::of_jpeg(&bytes),
            jpeg: crate::jpeg::set_orientation(&bytes, exif_orientation),
        });
    }
    let bytes = std::fs::read(image).map_err(|e| failed(&e))?;
    let origin = Origin::of_file(&bytes);
    let pixels = match crate::flat::decode_deep(image).map_err(|e| failed(&e))? {
        crate::flat::Deep::Bits16(img) => img,
        crate::flat::Deep::Bits8(bmp) => to16(bmp)?,
    };
    let jpeg = crate::jpeg::dimensions(&bytes).is_some().then_some(bytes);
    Ok(Developed {
        pixels,
        origin,
        jpeg,
    })
}

/// Version (recadrage, [`crate::versions`]) à exporter : son cadre et son nom (ajouté au
/// nom du fichier : `DSCF0001-Story.jpg`).
#[derive(Clone, Debug, PartialEq)]
pub struct Cut {
    /// Cadre, en proportions de l'image.
    pub crop: crate::versions::Rect,
    /// Nom de la version.
    pub name: String,
}

impl Cut {
    /// Pour un nom de fichier : `-` puis le nom, sans les caractères interdits par les
    /// systèmes (`/ \ : * ? " < > |`).
    fn suffix(&self) -> String {
        let clean: String = self
            .name
            .trim()
            .chars()
            .map(|c| {
                if "/\\:*?\"<>|".contains(c) || c.is_control() {
                    '_'
                } else {
                    c
                }
            })
            .collect();
        format!("-{clean}")
    }
}

/// Ce qu'on exporte, hors images : profil, dossier, version (recadrage) et accentuation.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    /// Profil d'export.
    pub profile: &'a Profile,
    /// Dossier (voir [`Profile::folder`]).
    pub folder: &'a str,
    /// Version à exporter (recadrage), sinon l'image entière.
    pub cut: Option<&'a Cut>,
    /// Paramètres de l'accentuation de sortie.
    pub sharpening: Sharpening,
    /// JPEG de paires dont le RAW est aussi exporté, et le suffixe ajouté à leur nom
    /// (`DSCF0001-camera.jpg` à côté de `DSCF0001.jpg`, rendu du RAW).
    pub paired: Option<(&'a HashSet<PathBuf>, &'a str)>,
}

impl<'a> Request<'a> {
    /// Image entière, accentuation par défaut.
    pub fn new(profile: &'a Profile, folder: &'a str) -> Request<'a> {
        Request {
            profile,
            folder,
            cut: None,
            sharpening: Sharpening::default(),
            paired: None,
        }
    }

    /// Exporter la version `cut`.
    pub fn cut(self, cut: Option<&'a Cut>) -> Request<'a> {
        Request { cut, ..self }
    }

    /// Paramètres de l'accentuation (`[export.sharpening]`).
    pub fn sharpening(self, sharpening: Sharpening) -> Request<'a> {
        Request { sharpening, ..self }
    }

    /// Ajouté au nom du fichier de `image` : version (`-Story`), puis JPEG de paire
    /// (`-camera`).
    fn extra(&self, image: &Path) -> String {
        let mut extra = self.cut.map(Cut::suffix).unwrap_or_default();
        if let Some((paired, suffix)) = self.paired
            && paired.contains(image)
        {
            extra.push_str(suffix);
        }
        extra
    }
}

/// Ce que contient un export : des RAW (rendus), des images déjà développées (JPEG,
/// TIFF… : copiées, converties ou réduites), ou les deux.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Seulement des RAW.
    Raw,
    /// Seulement des images déjà développées (JPEG boîtier compris).
    Flat,
    /// Les deux.
    Mixed,
}

impl Kind {
    /// Le profil s'applique-t-il à cet export ? Pour un mélange : seulement les profils
    /// valables pour les deux (le plus petit dénominateur, retour du 19/10/2026).
    pub fn allows(self, profile: &Profile) -> bool {
        match self {
            Kind::Raw => profile.applies_to.allows(false),
            Kind::Flat => profile.applies_to.allows(true),
            Kind::Mixed => profile.applies_to == AppliesTo::All,
        }
    }
}

/// Images d'un export, rangées : RAW, images déjà développées, et parmi elles les JPEG de
/// paires dont le RAW est aussi exporté.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Batch {
    /// RAW (rendus par darktable, LibRaw, ou leur JPEG boîtier).
    pub raws: Vec<PathBuf>,
    /// Images déjà développées.
    pub flats: Vec<PathBuf>,
    /// JPEG de paires (leur RAW est dans `raws`) : exportés aussi, nom suffixé.
    pub paired: HashSet<PathBuf>,
}

impl Batch {
    /// Range `images` (sans doublon). Retour du 19/10/2026 : le JPEG d'une paire est
    /// exporté **en plus** de son RAW (l'un développé par l'appareil, l'autre par darktable
    /// ou LibRaw), sous un autre nom ; sauf `embedded` (on exporte le JPEG boîtier des
    /// RAW : ce serait deux fois la même image de l'appareil), où il est laissé de côté.
    pub fn new(images: &[PathBuf], embedded: bool) -> Batch {
        let images = if embedded {
            sources(images)
        } else {
            let mut seen = HashSet::new();
            images.iter().filter(|p| seen.insert(*p)).cloned().collect()
        };
        let (raws, flats): (Vec<PathBuf>, Vec<PathBuf>) = images
            .into_iter()
            .partition(|p| kind_of(p) == Some(MediaKind::Raw));
        let raw_stems: HashSet<PathBuf> = raws.iter().map(|p| p.with_extension("")).collect();
        let paired = flats
            .iter()
            .filter(|p| raw_stems.contains(&p.with_extension("")))
            .cloned()
            .collect();
        Batch {
            raws,
            flats,
            paired,
        }
    }

    /// Ce que contient l'export ; `embedded` : les RAW sont exportés par leur JPEG
    /// boîtier, une image déjà développée.
    pub fn kind(&self, embedded: bool) -> Kind {
        let raws = !self.raws.is_empty() && !embedded;
        let flats = !self.flats.is_empty() || (embedded && !self.raws.is_empty());
        match (raws, flats) {
            (true, true) => Kind::Mixed,
            (true, false) => Kind::Raw,
            _ => Kind::Flat,
        }
    }

    /// Nombre d'images exportées.
    pub fn len(&self) -> usize {
        self.raws.len() + self.flats.len()
    }

    /// Vrai si rien n'est à exporter.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Comment les RAW d'un export sont rendus.
#[derive(Clone, Copy, Debug)]
pub enum Renderer<'a> {
    /// Par darktable.
    Darktable(&'a Darktable),
    /// Sans darktable : LibRaw ou JPEG boîtier.
    Local(Local),
}

/// Exporte `batch` : les RAW par `renderer` (darktable passe par un TIFF temporaire s'il
/// faut recadrer ou accentuer), puis les images déjà développées (copiées, converties ou
/// réduites ; JPEG de paire suffixé de `pair_suffix`). Une progression, un bilan.
pub fn run(
    batch: &Batch,
    renderer: Renderer<'_>,
    req: &Request<'_>,
    pair_suffix: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, usize, &Path),
) -> Result<Report, DevelopError> {
    let total = batch.len();
    let mut report = Report::default();
    if !batch.raws.is_empty() {
        let mut step = |done: usize, _: usize, path: &Path| progress(done, total, path);
        let part = match renderer {
            Renderer::Darktable(dt)
                if req.cut.is_some() || req.profile.sharpen != Sharpen::None =>
            {
                export_via_tiff(dt, &batch.raws, req, cancel, &mut step)?
            }
            Renderer::Darktable(dt) => {
                export(dt, &batch.raws, req.profile, req.folder, cancel, &mut step)?
            }
            Renderer::Local(local) => export_libraw(&batch.raws, req, local, cancel, &mut step)?,
        };
        report.merge(part);
    }
    if !batch.flats.is_empty() {
        let offset = report.written.len();
        let flats = Request {
            paired: Some((&batch.paired, pair_suffix)),
            ..*req
        };
        let step = |done: usize, _: usize, path: &Path| progress(offset + done, total, path);
        let part = export_libraw(&batch.flats, &flats, Local::LibRaw, cancel, step)?;
        report.merge(part);
    }
    Ok(report)
}

impl Report {
    fn merge(&mut self, other: Report) {
        self.written.extend(other.written);
        self.failed += other.failed;
        self.errors.extend(other.errors);
        self.passthrough += other.passthrough;
    }
}

/// Exporte `images` **sans darktable** : RAW développés par LibRaw (le rendu « plat » de
/// `tri` : balance des blancs du boîtier, sans luminosité automatique) ou leur JPEG
/// boîtier (`local`), images plates réencodées. Une image déjà développée (JPEG boîtier,
/// image plate) garde son espace d'origine (`""`), ou y est **convertie** (`moxcms`) ;
/// un JPEG est copié tel quel si possible ([`Profile::passthrough`]). Même dossier, mêmes
/// noms et même progression que [`export`] ; une image en échec n'arrête pas les autres
/// (raison dans [`Report::errors`]).
pub fn export_libraw(
    images: &[PathBuf],
    req: &Request<'_>,
    local: Local,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, usize, &Path),
) -> Result<Report, DevelopError> {
    let profile = req.profile;
    libraw_check(profile, local).map_err(DevelopError::Other)?;
    let images = sources(images);
    let total = images.len();
    let mut report = Report::default();
    for image in &images {
        if cancel.load(Ordering::Relaxed) {
            return Err(DevelopError::Cancelled);
        }
        match export_one(image, req, local, cancel) {
            Ok((out, copied)) => {
                report.passthrough += usize::from(copied);
                report.written.push(out);
                progress(
                    report.written.len(),
                    total,
                    report.written.last().expect("ajouté"),
                );
            }
            Err(DevelopError::Cancelled) => return Err(DevelopError::Cancelled),
            Err(e) => report.errors.push((image.clone(), e.to_string())),
        }
    }
    report.failed = report.errors.len();
    Ok(report)
}

/// Exporte une image ; `true` : copiée sans recompression.
fn export_one(
    image: &Path,
    req: &Request<'_>,
    local: Local,
    cancel: &AtomicBool,
) -> Result<(PathBuf, bool), DevelopError> {
    let (profile, cut) = (req.profile, req.cut);
    let failed = |e: &dyn std::fmt::Display| DevelopError::Other(e.to_string());
    let encode = |p: moxcms::ColorProfile| p.encode().map_err(|e| failed(&format!("{e:?}")));
    let (pixels, icc) = if kind_of(image) == Some(MediaKind::Raw) && local == Local::LibRaw {
        let (space, target) = libraw_space(&profile.color).expect("vérifié");
        let mut raw = crate::raw::RawFile::open(image).map_err(|e| failed(&e))?;
        let pixels = raw
            .render_deep(&crate::raw::RenderOptions::default(), space, cancel)
            .map_err(|e| match e {
                crate::raw::RawError::Cancelled => DevelopError::Cancelled,
                e => failed(&e),
            })?;
        (pixels, Some(encode(target)?))
    } else {
        let Developed {
            mut pixels,
            origin,
            jpeg,
        } = developed(image)?;
        let (w, h) = pixels.dimensions();
        let fits = profile.max_side == 0 || w.max(h) <= profile.max_side;
        // Sans recompression : JPEG vers JPEG, sans recadrage ni réduction, sans changer
        // d'espace.
        if profile.passthrough
            && cut.is_none()
            && profile.sharpen == Sharpen::None
            && profile.format == Format::Jpeg
            && fits
            && origin.is(&profile.color)
            && let Some(bytes) = jpeg
        {
            return write_bytes(image, req, &bytes).map(|out| (out, true));
        }
        let icc = if origin.is(&profile.color) {
            origin.icc()
        } else {
            // Conversion de l'espace d'origine vers celui du profil.
            let (_, target) = libraw_space(&profile.color).expect("vérifié");
            let source = origin
                .profile()
                .ok_or_else(|| DevelopError::Other(crate::t!("export-bad-icc")))?;
            let transform = source
                .create_transform_16bit(
                    moxcms::Layout::Rgb,
                    &target,
                    moxcms::Layout::Rgb,
                    moxcms::TransformOptions::default(),
                )
                .map_err(|e| failed(&format!("{e:?}")))?;
            let mut out = vec![0u16; pixels.as_raw().len()];
            transform
                .transform(pixels.as_raw(), &mut out)
                .map_err(|e| failed(&format!("{e:?}")))?;
            pixels = crate::raw::Rgb16::from_raw(w, h, out).expect("même taille");
            Some(encode(target)?)
        };
        (pixels, icc)
    };
    finish(image, pixels, icc.as_deref(), req).map(|out| (out, false))
}

/// Fin d'un export : recadrage (version), réduction au côté maximal du profil,
/// accentuation de sortie, écriture au format du profil avec le profil ICC `icc`.
fn finish(
    image: &Path,
    mut pixels: crate::raw::Rgb16,
    icc: Option<&[u8]>,
    req: &Request<'_>,
) -> Result<PathBuf, DevelopError> {
    let (profile, folder, cut, sharpening) = (req.profile, req.folder, req.cut, &req.sharpening);
    if let Some(cut) = cut {
        let (w, h) = pixels.dimensions();
        let (x, y, cw, ch) = cut.crop.pixels(w, h);
        pixels = image::imageops::crop_imm(&pixels, x, y, cw, ch).to_image();
    }
    let (w, h) = pixels.dimensions();
    let side = profile.max_side;
    if side > 0 && w.max(h) > side {
        let scale = f64::from(side) / f64::from(w.max(h));
        let (nw, nh) = (
            ((f64::from(w) * scale).round() as u32).max(1),
            ((f64::from(h) * scale).round() as u32).max(1),
        );
        pixels = image::imageops::resize(&pixels, nw, nh, image::imageops::FilterType::Lanczos3);
    }
    // Après la réduction, comme Lightroom : on accentue ce qui sera vu.
    if let Some(u) = sharpening.of(profile.sharpen) {
        pixels = unsharp(&pixels, u);
    }
    let extra = req.extra(image);
    let out = output_path(image, profile, &extra, folder, profile.format.extension())?;
    write_image(&out, &pixels, profile, icc).map_err(|e| {
        let _ = std::fs::remove_file(&out);
        DevelopError::Other(e.to_string())
    })?;
    Ok(out)
}

/// Exporte `images` **par darktable, en passant par un TIFF 16 bits temporaire** (espace
/// du profil, pleine définition, un seul lancement), puis recadré (version `cut`), réduit,
/// accentué et écrit au format du profil : pour ce que darktable ne fait pas lui-même. Une
/// seule compression, jamais deux.
pub fn export_via_tiff(
    dt: &Darktable,
    images: &[PathBuf],
    req: &Request<'_>,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize, usize, &Path),
) -> Result<Report, DevelopError> {
    let profile = req.profile;
    use image::ImageDecoder;
    let failed = |e: &dyn std::fmt::Display| DevelopError::Other(e.to_string());
    let images = sources(images);
    let tmp = dt.config_dir.join(format!("render-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let render = Profile {
        format: Format::Tiff,
        bits: 16,
        max_side: 0,
        suffix: String::new(),
        passthrough: false,
        applies_to: AppliesTo::All,
        sharpen: Sharpen::None,
        ..profile.clone()
    };
    let result = (|| {
        let rendered = export(
            dt,
            &images,
            &render,
            &tmp.display().to_string(),
            cancel,
            |_, _, _| {},
        )?;
        let total = images.len();
        let mut used = vec![false; total];
        let mut report = Report::default();
        for file in &rendered.written {
            // L'image d'origine du rendu : même nom (darktable ajoute « _01 » en cas de
            // doublon de nom).
            let stem = file
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let base = stem.rsplit_once('_').map_or(stem.as_str(), |(b, _)| b);
            let Some(i) = (0..total).find(|&i| {
                !used[i]
                    && images[i]
                        .file_stem()
                        .is_some_and(|s| s.to_string_lossy() == stem || s.to_string_lossy() == base)
            }) else {
                continue;
            };
            used[i] = true;
            let mut decoder = image::ImageReader::open(file)
                .and_then(|r| r.with_guessed_format())
                .map_err(|e| failed(&e))?
                .into_decoder()
                .map_err(|e| failed(&e))?;
            let icc = decoder.icc_profile().ok().flatten();
            let pixels = image::DynamicImage::from_decoder(decoder)
                .map_err(|e| failed(&e))?
                .into_rgb16();
            match finish(&images[i], pixels, icc.as_deref(), req) {
                Ok(out) => {
                    report.written.push(out);
                    progress(
                        report.written.len(),
                        total,
                        report.written.last().expect("ajouté"),
                    );
                }
                Err(e) => report.errors.push((images[i].clone(), e.to_string())),
            }
        }
        if report.written.is_empty() && report.errors.is_empty() && total > 0 {
            return Err(DevelopError::Other(crate::t!("export-version-empty")));
        }
        report.failed = total - report.written.len();
        Ok(report)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

/// Fichier de sortie de `image` (dossier créé, jamais d'écrasement).
fn output_path(
    image: &Path,
    profile: &Profile,
    extra: &str,
    folder: &str,
    ext: &str,
) -> Result<PathBuf, DevelopError> {
    let dir = folder_for(folder, image);
    std::fs::create_dir_all(&dir).map_err(|e| DevelopError::Other(e.to_string()))?;
    let stem = image
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(unique_path(
        &dir,
        &format!("{stem}{}{extra}", profile.suffix),
        ext,
    ))
}

/// Écrit `bytes` (JPEG sans recompression) à la place de l'export de `image`.
fn write_bytes(image: &Path, req: &Request<'_>, bytes: &[u8]) -> Result<PathBuf, DevelopError> {
    let out = output_path(image, req.profile, &req.extra(image), req.folder, "jpg")?;
    std::fs::write(&out, bytes).map_err(|e| DevelopError::Other(e.to_string()))?;
    Ok(out)
}

/// `dir/nom.ext`, ou `nom_01.ext`, `nom_02.ext`… : jamais d'écrasement (comme darktable).
fn unique_path(dir: &Path, name: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{name}.{ext}"));
    if !first.exists() {
        return first;
    }
    (1..)
        .map(|n| dir.join(format!("{name}_{n:02}.{ext}")))
        .find(|p| !p.exists())
        .expect("un nom libre")
}

/// Écrit `pixels` au format du profil, avec le profil ICC `icc`.
fn write_image(
    out: &Path,
    pixels: &crate::raw::Rgb16,
    profile: &Profile,
    icc: Option<&[u8]>,
) -> Result<(), image::ImageError> {
    use image::{ExtendedColorType, ImageEncoder};
    let (w, h) = pixels.dimensions();
    let file = std::io::BufWriter::new(std::fs::File::create(out)?);
    let eight = || -> Vec<u8> {
        pixels
            .as_raw()
            .iter()
            .map(|&v| ((u32::from(v) + 128) / 257) as u8)
            .collect()
    };
    let sixteen = || -> Vec<u8> {
        pixels
            .as_raw()
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect()
    };
    let deep = profile.bits >= 16;
    let (bytes, color) = if deep {
        (sixteen(), ExtendedColorType::Rgb16)
    } else {
        (eight(), ExtendedColorType::Rgb8)
    };
    let unsupported = |e| image::ImageError::Unsupported(e);
    match profile.format {
        Format::Jpeg => {
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(file, profile.quality);
            if let Some(icc) = icc {
                enc.set_icc_profile(icc.to_vec()).map_err(unsupported)?;
            }
            enc.write_image(&eight(), w, h, ExtendedColorType::Rgb8)
        }
        Format::Tiff => {
            let mut enc = image::codecs::tiff::TiffEncoder::new(file);
            if let Some(icc) = icc {
                enc.set_icc_profile(icc.to_vec()).map_err(unsupported)?;
            }
            enc.write_image(&bytes, w, h, color)
        }
        Format::Png => {
            let mut enc = image::codecs::png::PngEncoder::new(file);
            if let Some(icc) = icc {
                enc.set_icc_profile(icc.to_vec()).map_err(unsupported)?;
            }
            enc.write_image(&bytes, w, h, color)
        }
    }
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
        assert_eq!(
            ids,
            ["edit", "asis", "web", "print"],
            "fournis d'abord, gardés"
        );
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
    fn a_profile_says_what_it_applies_to() {
        let p = Profiles::default();
        assert_eq!(p.get("asis").unwrap().applies_to, AppliesTo::Flat);
        assert_eq!(p.get("web").unwrap().applies_to, AppliesTo::All);
        assert!(AppliesTo::Flat.allows(true) && !AppliesTo::Flat.allows(false));
        assert!(AppliesTo::Raw.allows(false) && !AppliesTo::Raw.allows(true));
        assert!(AppliesTo::All.allows(true) && AppliesTo::All.allows(false));
        #[derive(Deserialize)]
        struct S {
            export: ExportSettings,
        }
        let s: S = toml::from_str("[export.profiles.web]\napplies_to = \"raw\"\n").unwrap();
        assert_eq!(
            s.export.profiles.get("web").unwrap().applies_to,
            AppliesTo::Raw
        );
        assert!(toml::from_str::<S>("[export.profiles.web]\napplies_to = \"tout\"\n").is_err());
    }

    #[test]
    fn the_proposed_profile_comes_from_the_settings() {
        let mut s = ExportSettings::default();
        // Par défaut : Retouche pour un rendu, JPEG tel quel pour une image développée.
        assert_eq!(s.applicable(false)[s.default_index(false)].id, "edit");
        assert_eq!(s.applicable(true)[s.default_index(true)].id, "asis");
        // Changé dans les réglages.
        s.default_raw = "web".into();
        assert_eq!(s.applicable(false)[s.default_index(false)].id, "web");
        // Inconnu, ou qui ne s'applique pas (JPEG tel quel pour un rendu) : le premier.
        s.default_raw = "asis".into();
        assert_eq!(s.default_index(false), 0);
        s.default_flat = "nulle-part".into();
        assert_eq!(s.default_index(true), 0);
    }

    #[test]
    fn output_sharpening_raises_edges_and_leaves_flat_areas_alone() {
        // Marche grise : 20 000 / 40 000.
        let step = crate::raw::Rgb16::from_fn(40, 20, |x, _| {
            let v = if x < 20 { 20_000 } else { 40_000 };
            image::Rgb([v, v, v])
        });
        let s = Sharpening::default();
        let sharp = unsharp(&step, s.of(Sharpen::Screen).unwrap());
        // Au bord, le côté clair éclaircit, le côté sombre fonce ; loin du bord, rien.
        assert!(sharp.get_pixel(20, 10)[0] > 40_000);
        assert!(sharp.get_pixel(19, 10)[0] < 20_000);
        assert_eq!(sharp.get_pixel(2, 10)[0], 20_000);
        assert_eq!(sharp.get_pixel(37, 10)[0], 40_000);
        // Tirage : rayon plus large, le halo s'étend plus loin.
        let print = unsharp(&step, s.of(Sharpen::Print).unwrap());
        assert!(print.get_pixel(21, 10)[0] > sharp.get_pixel(21, 10)[0]);
        assert_eq!(s.of(Sharpen::None), None);
        // Profils fournis : Web accentue pour l'écran, Retouche et JPEG tel quel jamais.
        let p = Profiles::default();
        assert_eq!(p.get("web").unwrap().sharpen, Sharpen::Screen);
        assert_eq!(p.get("edit").unwrap().sharpen, Sharpen::None);
        assert_eq!(p.get("asis").unwrap().sharpen, Sharpen::None);
    }

    #[test]
    fn a_pair_exports_both_developments_under_two_names() {
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join("DSCF0001.RAF");
        let jpg = dir.path().join("DSCF0001.JPG");
        let alone = dir.path().join("autre.jpg");
        std::fs::write(&raw, b"raw").unwrap();
        image::RgbImage::from_pixel(30, 20, image::Rgb([90, 120, 150]))
            .save(&jpg)
            .unwrap();
        std::fs::copy(&jpg, &alone).unwrap();
        let images = vec![raw.clone(), jpg.clone(), alone.clone(), raw.clone()];
        let batch = Batch::new(&images, false);
        assert_eq!(batch.raws, std::slice::from_ref(&raw));
        assert_eq!(batch.flats, [jpg.clone(), alone.clone()]);
        assert!(batch.paired.contains(&jpg) && !batch.paired.contains(&alone));
        assert_eq!(batch.kind(false), Kind::Mixed);
        // JPEG boîtier : la paire une seule fois, tout est « déjà développé ».
        let embedded = Batch::new(&images, true);
        assert_eq!(embedded.flats, std::slice::from_ref(&alone));
        assert_eq!(embedded.kind(true), Kind::Flat);
        // Mélange : seulement les profils valables pour les deux.
        let p = Profiles::default();
        assert!(Kind::Mixed.allows(p.get("web").unwrap()));
        assert!(!Kind::Mixed.allows(p.get("asis").unwrap()));
        // Les images développées seules : le JPEG de paire prend le suffixe.
        let only_flats = Batch {
            raws: Vec::new(),
            ..batch.clone()
        };
        let report = run(
            &only_flats,
            Renderer::Local(Local::LibRaw),
            &Request::new(p.get("asis").unwrap(), "out"),
            "-camera",
            &AtomicBool::new(false),
            |_, _, _| {},
        )
        .unwrap();
        let names: Vec<String> = report
            .written
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["DSCF0001-camera.jpg", "autre.jpg"]);
        assert_eq!(report.passthrough, 2, "copiés tels quels");
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

    #[test]
    fn without_darktable_most_profiles_still_work() {
        let p = Profiles::default();
        assert!(
            libraw_check(p.get("edit").unwrap(), Local::LibRaw).is_ok(),
            "Retouche"
        );
        assert!(
            libraw_check(p.get("web").unwrap(), Local::LibRaw).is_ok(),
            "Web"
        );
        let p3 = Profile {
            color: "display-p3".into(),
            ..Profile::base("p3")
        };
        assert!(libraw_check(&p3, Local::LibRaw).unwrap_err().contains("p3"));
        let float = Profile {
            format: Format::Tiff,
            bits: 32,
            color: "prophoto".into(),
            ..Profile::base("f32")
        };
        assert!(libraw_check(&float, Local::LibRaw).is_err());
        // JPEG boîtier : converti au besoin (Web comme Retouche).
        assert!(libraw_check(p.get("web").unwrap(), Local::Embedded).is_ok());
        assert!(libraw_check(p.get("edit").unwrap(), Local::Embedded).is_ok());
        for color in ["", "srgb", "adobergb", "prophoto", "rec709", "rec2020"] {
            let (_, icc) = libraw_space(color).unwrap();
            assert!(icc.encode().is_ok(), "{color}");
        }
    }

    #[test]
    fn nothing_is_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let first = unique_path(dir.path(), "DSCF0001-web", "jpg");
        assert_eq!(first, dir.path().join("DSCF0001-web.jpg"));
        std::fs::write(&first, b"x").unwrap();
        let second = unique_path(dir.path(), "DSCF0001-web", "jpg");
        assert_eq!(second, dir.path().join("DSCF0001-web_01.jpg"));
    }

    #[test]
    fn a_flat_image_is_exported_with_its_icc_profile() {
        use image::ImageDecoder;
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("photo.png");
        image::RgbImage::from_pixel(300, 200, image::Rgb([200, 100, 50]))
            .save(&src)
            .unwrap();
        for (format, bits, ext) in [
            (Format::Jpeg, 8, "jpg"),
            (Format::Tiff, 16, "tif"),
            (Format::Png, 16, "png"),
        ] {
            let profile = Profile {
                format,
                bits,
                max_side: 150,
                suffix: "-test".into(),
                ..Profile::base("t")
            };
            let report = export_libraw(
                std::slice::from_ref(&src),
                &Request::new(&profile, "export"),
                Local::LibRaw,
                &AtomicBool::new(false),
                |_, _, _| {},
            )
            .unwrap();
            assert_eq!(report.failed, 0, "{:?}", report.errors);
            let out = &report.written[0];
            assert_eq!(*out, dir.path().join(format!("export/photo-test.{ext}")));
            let reader = image::ImageReader::open(out)
                .unwrap()
                .with_guessed_format()
                .unwrap();
            let mut decoder = reader.into_decoder().unwrap();
            assert_eq!(decoder.dimensions(), (150, 100), "{ext}");
            assert!(
                decoder.icc_profile().unwrap().is_some(),
                "profil ICC dans le {ext}"
            );
            let depth = decoder.color_type().bits_per_pixel() / 3;
            assert_eq!(depth, u16::from(bits), "{ext}");
        }
        // Une image plate (sans profil : sRGB) vers ProPhoto : convertie, profil intégré.
        let prophoto = Profile {
            format: Format::Tiff,
            bits: 16,
            color: "prophoto".into(),
            ..Profile::base("pp")
        };
        let report = export_libraw(
            &[src],
            &Request::new(&prophoto, "export"),
            Local::LibRaw,
            &AtomicBool::new(false),
            |_, _, _| {},
        )
        .unwrap();
        assert_eq!(report.failed, 0, "{:?}", report.errors);
        let mut decoder = image::ImageReader::open(&report.written[0])
            .unwrap()
            .with_guessed_format()
            .unwrap()
            .into_decoder()
            .unwrap();
        let icc = decoder.icc_profile().unwrap().unwrap();
        let wide: Vec<u8> = b"ProPhoto".iter().flat_map(|&b| [0, b]).collect();
        let named = icc.windows(8).any(|w| w == b"ProPhoto")
            || icc.windows(16).any(|w| w == wide.as_slice());
        assert!(named, "profil ProPhoto");
    }

    #[test]
    fn a_jpeg_is_copied_untouched_unless_it_must_shrink() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("photo.jpg");
        image::RgbImage::from_pixel(300, 200, image::Rgb([200, 100, 50]))
            .save(&src)
            .unwrap();
        let original = std::fs::read(&src).unwrap();
        let profile = |max_side| Profile {
            passthrough: true,
            max_side,
            ..Profile::base("web")
        };
        let cancel = AtomicBool::new(false);
        let run = |p: &Profile| {
            export_libraw(
                std::slice::from_ref(&src),
                &Request::new(p, "out"),
                Local::Embedded,
                &cancel,
                |_, _, _| {},
            )
            .unwrap()
        };
        // Assez petite : copiée octet par octet.
        let report = run(&profile(2048));
        assert_eq!(report.passthrough, 1);
        assert_eq!(std::fs::read(&report.written[0]).unwrap(), original);
        // « JPEG tel quel » (profil fourni) : copie pure et simple, rien de touché.
        let asis = Profiles::default().get("asis").unwrap().clone();
        let report = run(&asis);
        assert_eq!(report.passthrough, 1);
        assert_eq!(std::fs::read(&report.written[0]).unwrap(), original);
        std::fs::remove_file(&report.written[0]).unwrap();
        // À réduire : réencodée (pas de copie possible), sans écraser la première.
        let report = run(&profile(150));
        assert_eq!(report.passthrough, 0);
        assert_eq!(report.written[0], dir.path().join("out/photo_01.jpg"));
        assert_eq!(
            image::image_dimensions(&report.written[0]).unwrap(),
            (150, 100)
        );
    }

    #[test]
    fn an_adobe_rgb_jpeg_is_kept_as_is_or_converted_for_the_web() {
        use image::{ImageDecoder, ImageEncoder};
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("boitier.jpg");
        // JPEG Adobe RGB (profil intégré), vert saturé.
        let adobe = moxcms::ColorProfile::new_adobe_rgb().encode().unwrap();
        let pixels = image::RgbImage::from_pixel(64, 48, image::Rgb([20, 220, 40]));
        let mut bytes = Vec::new();
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95);
        enc.set_icc_profile(adobe).unwrap();
        enc.write_image(pixels.as_raw(), 64, 48, image::ExtendedColorType::Rgb8)
            .unwrap();
        std::fs::write(&src, &bytes).unwrap();
        assert_eq!(Origin::of_jpeg(&bytes), Origin::AdobeRgb, "reconnu");
        let profiles = Profiles::default();
        let run = |p: &Profile| {
            export_libraw(
                std::slice::from_ref(&src),
                &Request::new(p, "out"),
                Local::Embedded,
                &AtomicBool::new(false),
                |_, _, _| {},
            )
            .unwrap()
        };
        // « JPEG tel quel » : copie pure et simple, toujours Adobe RGB.
        let asis = run(profiles.get("asis").unwrap());
        assert_eq!(asis.passthrough, 1);
        assert_eq!(std::fs::read(&asis.written[0]).unwrap(), bytes);
        // « Web » (sRGB) : converti, pas copié, profil sRGB.
        let web = run(profiles.get("web").unwrap());
        assert_eq!(web.passthrough, 0, "un autre espace : pas de copie");
        let out = std::fs::read(&web.written[0]).unwrap();
        assert_eq!(Origin::of_jpeg(&out), Origin::Srgb);
        let mut decoder =
            image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(&out)).unwrap();
        assert!(decoder.icc_profile().unwrap().is_some());
        // Le vert Adobe RGB, hors du gamut sRGB : sa composante verte reste au maximum,
        // le rouge et le bleu changent (conversion faite).
        let px = image::load_from_memory(&out)
            .unwrap()
            .to_rgb8()
            .get_pixel(32, 24)
            .0;
        assert_ne!(px, [20, 220, 40], "converti : {px:?}");
    }

    /// Version exportée par darktable (rendu TIFF 16 bits temporaire, recadré, puis
    /// profil) : `TRI_TEST_DARKTABLE_RAW`, copié.
    #[test]
    #[ignore]
    fn darktable_exports_a_cropped_version() {
        let cli = crate::develop::find_darktable("auto").expect("darktable installé");
        let original = PathBuf::from(std::env::var("TRI_TEST_DARKTABLE_RAW").expect("RAW de test"));
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join(original.file_name().unwrap());
        std::fs::copy(&original, &raw).unwrap();
        let mut name = original.file_name().unwrap().to_os_string();
        name.push(".xmp");
        std::fs::copy(original.with_file_name(&name), raw.with_file_name(&name)).unwrap();
        let dt = Darktable {
            cli,
            config_dir: dir.path().join("cfg"),
            high_quality: true,
        };
        // Cadre 1:1 calculé sur l'image vue : le développé, recadré dans darktable
        // (2763 × 4912 pour le RAW de test), pas sur l'image entière du capteur.
        let ratio = crate::versions::Ratio::Fixed(1, 1);
        let cut = Cut {
            crop: crate::versions::Rect::fit(ratio, 2763.0 / 4912.0),
            name: "Story".into(),
        };
        let web = Profiles::default().get("web").unwrap().clone();
        let report = export_via_tiff(
            &dt,
            std::slice::from_ref(&raw),
            &Request::new(&web, "export").cut(Some(&cut)),
            &AtomicBool::new(false),
            |_, _, _| {},
        )
        .unwrap();
        let out = &report.written[0];
        assert_eq!(out.file_name().unwrap(), "DSCF0331-Story.jpg");
        let (w, h) = image::image_dimensions(out).unwrap();
        assert_eq!(w.max(h), 2048, "côté long du profil Web");
        assert!(w.abs_diff(h) <= 2, "carré : {w} × {h}");
        assert!(
            !dt.config_dir
                .join(format!("render-{}", std::process::id()))
                .exists()
        );
    }

    /// Export LibRaw réel : `TRI_TEST_RAW` (RAW hors du dépôt), copié.
    #[test]
    #[ignore]
    fn libraw_exports_a_raw_in_16_bit_prophoto() {
        use image::ImageDecoder;
        let original = PathBuf::from(std::env::var("TRI_TEST_RAW").expect("RAW de test"));
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join(original.file_name().unwrap());
        std::fs::copy(&original, &raw).unwrap();
        let profiles = Profiles::default();
        let report = export_libraw(
            &[raw],
            &Request::new(profiles.get("edit").unwrap(), "export"),
            Local::LibRaw,
            &AtomicBool::new(false),
            |_, _, _| {},
        )
        .unwrap();
        assert_eq!(report.failed, 0, "{:?}", report.errors);
        let reader = image::ImageReader::open(&report.written[0])
            .unwrap()
            .with_guessed_format()
            .unwrap();
        let mut decoder = reader.into_decoder().unwrap();
        assert_eq!(decoder.color_type(), image::ColorType::Rgb16);
        assert!(decoder.icc_profile().unwrap().is_some());
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
