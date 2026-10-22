//! Langue de l'application : chaînes traduites, par identifiant, avec
//! [Fluent](https://projectfluent.org) (Mozilla).
//!
//! Le code ne contient pas de texte destiné à l'utilisateur, seulement des identifiants
//! (`t!("cache-cleared")`) ; les textes sont dans des catalogues `.ftl`, un par langue,
//! embarqués dans le binaire : `locales/<langue>.ftl` pour le moteur (et la ligne de
//! commande), d'autres catalogues pour l'interface ([`register`]). Le français est la
//! langue de référence : un identifiant absent d'une langue s'y affiche en français, et
//! un identifiant absent partout s'affiche tel quel (une erreur, détectée par les tests).
//!
//! Fluent choisit la forme du pluriel selon la langue : en français 0 et 1 sont au
//! singulier (« 0 image »), en anglais seul 1 l'est. Les nombres décimaux sont mis en
//! forme avant d'être passés ([`decimal`] : virgule en français). Voir `doc/en/i18n.md`.

use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use serde::Deserialize;

/// Langue des textes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Français (langue de référence).
    #[default]
    Fr,
    /// Anglais.
    En,
}

impl Language {
    /// Toutes les langues disponibles.
    pub const ALL: [Language; 2] = [Language::Fr, Language::En];

    /// Code de la langue (`"fr"`, `"en"`).
    pub fn code(self) -> &'static str {
        match self {
            Language::Fr => "fr",
            Language::En => "en",
        }
    }

    /// Langue d'un code ou d'une locale (`"fr"`, `"fr-FR"`, `"en_US.UTF-8"`), sans tenir
    /// compte de la casse ; `None` si elle n'est pas disponible.
    pub fn from_locale(locale: &str) -> Option<Language> {
        let primary = locale
            .split(['-', '_', '.', '@'])
            .next()?
            .to_ascii_lowercase();
        Language::ALL.into_iter().find(|l| l.code() == primary)
    }

    /// Langue du système (`sys-locale`) si elle est disponible, sinon l'anglais (la plus
    /// largement comprise).
    pub fn system() -> Language {
        sys_locale::get_locales()
            .find_map(|l| Language::from_locale(&l))
            .unwrap_or(Language::En)
    }
}

/// Réglage de la langue (section `[i18n]`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageSetting {
    /// Langue du système.
    #[default]
    Auto,
    /// Français.
    Fr,
    /// Anglais.
    En,
}

impl LanguageSetting {
    /// Langue à utiliser.
    pub fn resolve(self) -> Language {
        match self {
            LanguageSetting::Auto => Language::system(),
            LanguageSetting::Fr => Language::Fr,
            LanguageSetting::En => Language::En,
        }
    }
}

/// Section `[i18n]` du fichier de configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct I18nSettings {
    /// `"auto"` (langue du système, défaut), `"fr"` ou `"en"`.
    pub language: LanguageSetting,
}

/// Catalogues du moteur.
const ENGINE: &[(Language, &str)] = &[
    (Language::Fr, include_str!("../locales/fr.ftl")),
    (Language::En, include_str!("../locales/en.ftl")),
];

/// Catalogues ajoutés par l'interface ([`register`]).
static EXTRA: Mutex<Vec<(Language, &'static str)>> = Mutex::new(Vec::new());

struct Translator {
    /// La langue, puis le français (repli).
    bundles: Vec<FluentBundle<FluentResource>>,
}

/// Langue choisie ([`set_language`]).
static LANGUAGE: RwLock<Language> = RwLock::new(Language::Fr);

/// Catalogues compilés, par langue, construits à la première utilisation ; vidés quand un
/// catalogue est ajouté.
static TRANSLATORS: RwLock<Option<HashMap<Language, Translator>>> = RwLock::new(None);

fn bundle(language: Language) -> FluentBundle<FluentResource> {
    let id = language.code().parse().expect("code de langue valide");
    let mut bundle = FluentBundle::new_concurrent(vec![id]);
    // Sans les marques d'isolement Unicode autour des valeurs insérées : les polices de
    // l'interface n'ont pas ces caractères, et le texte reste en un seul sens.
    bundle.set_use_isolating(false);
    let extra = EXTRA.lock().unwrap_or_else(|e| e.into_inner()).clone();
    for (lang, source) in ENGINE.iter().copied().chain(extra) {
        if lang != language {
            continue;
        }
        let resource = FluentResource::try_new(source.to_string())
            .unwrap_or_else(|(resource, _errors)| resource);
        // Un identifiant en double garde la première définition (moteur d'abord) ; les
        // doublons sont signalés par les tests, pas à l'utilisateur.
        let _ = bundle.add_resource(resource);
    }
    bundle
}

fn translator(language: Language) -> Translator {
    let mut bundles = vec![bundle(language)];
    if language != Language::Fr {
        bundles.push(bundle(Language::Fr));
    }
    Translator { bundles }
}

/// Ajoute un catalogue (texte `.ftl`) pour `language` : l'interface y met ses propres
/// chaînes. À appeler avant [`set_language`] (ou rappeler ensuite).
pub fn register(language: Language, source: &'static str) {
    let mut extra = EXTRA.lock().unwrap_or_else(|e| e.into_inner());
    if !extra.contains(&(language, source)) {
        extra.push((language, source));
        // Catalogues compilés sans ce nouveau catalogue : à refaire.
        *TRANSLATORS.write().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Choisit la langue des textes (français tant qu'elle n'est pas choisie).
pub fn set_language(language: Language) {
    *LANGUAGE.write().unwrap_or_else(|e| e.into_inner()) = language;
}

/// Langue des textes.
pub fn language() -> Language {
    *LANGUAGE.read().unwrap_or_else(|e| e.into_inner())
}

/// Texte de l'identifiant `id`, avec ses valeurs `args`, dans la langue choisie (voir
/// [`t!`](crate::t)).
pub fn translate(id: &str, args: Option<&FluentArgs>) -> String {
    translate_in(language(), id, args)
}

/// Comme [`translate`], dans la langue `language` (lire un nom de touche écrit dans
/// n'importe quelle langue, par exemple).
pub fn translate_in(language: Language, id: &str, args: Option<&FluentArgs>) -> String {
    with_translator(language, |t| format_in(&t.bundles, id, args))
}

/// `f` sur les catalogues de `language`, lus une fois puis gardés.
fn with_translator<R>(language: Language, f: impl FnOnce(&Translator) -> R) -> R {
    {
        let all = TRANSLATORS.read().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = all.as_ref().and_then(|m| m.get(&language)) {
            return f(t);
        }
    }
    let mut all = TRANSLATORS.write().unwrap_or_else(|e| e.into_inner());
    let t = all
        .get_or_insert_with(HashMap::new)
        .entry(language)
        .or_insert_with(|| translator(language));
    f(t)
}

fn format_in(
    bundles: &[FluentBundle<FluentResource>],
    id: &str,
    args: Option<&FluentArgs>,
) -> String {
    for bundle in bundles {
        if let Some(pattern) = bundle.get_message(id).and_then(|m| m.value()) {
            let mut errors = Vec::new();
            return bundle
                .format_pattern(pattern, args, &mut errors)
                .into_owned();
        }
    }
    id.to_string()
}

/// Vrai si `id` existe dans le catalogue de `language` (sans le repli en français).
/// Catalogues gardés comme pour [`translate`] : la recherche des réglages l'appelle pour
/// chaque réglage et chaque action (issue n° 16 : le catalogue relu à chaque appel
/// coûtait 120 ms par image).
pub fn has(language: Language, id: &str) -> bool {
    with_translator(language, |t| t.bundles[0].has_message(id))
}

/// Identifiants définis dans le texte `.ftl` `source` (messages, pas les termes `-x`).
pub fn ids(source: &str) -> Vec<String> {
    let resource =
        FluentResource::try_new(source.to_string()).unwrap_or_else(|(resource, _errors)| resource);
    resource
        .entries()
        .filter_map(|entry| match entry {
            fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_string()),
            _ => None,
        })
        .collect()
}

/// Catalogues du moteur, par langue (tests de complétude).
pub fn engine_catalogs() -> &'static [(Language, &'static str)] {
    ENGINE
}

/// Nombre décimal avec `digits` chiffres après la séparation, à la façon de la langue :
/// virgule en français, point en anglais.
pub fn decimal(value: f64, digits: usize) -> String {
    let text = format!("{value:.digits$}");
    match language() {
        Language::Fr => text.replace('.', ","),
        Language::En => text,
    }
}

/// Valeur pour [`t!`](crate::t) (reprise de Fluent : nombres, pour le choix du pluriel, et
/// textes).
pub type Value = FluentValue<'static>;

/// Texte traduit : `t!("id")`, ou avec des valeurs `t!("id", n = 3, name = nom)` (les
/// valeurs nommées `$n`, `$name` dans le catalogue). Les nombres entiers choisissent la
/// forme du pluriel ; les décimaux se passent déjà mis en forme ([`decimal`]).
#[macro_export]
macro_rules! t {
    ($id:expr) => {
        $crate::i18n::translate($id, None)
    };
    ($id:expr, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = $crate::i18n::Args::new();
        $(args.set(stringify!($name), $crate::i18n::value($value));)+
        $crate::i18n::translate($id, Some(&args))
    }};
}

/// Valeurs nommées d'un texte (voir [`t!`](crate::t)).
pub type Args = FluentArgs<'static>;

/// Conversion en valeur Fluent (nombres et textes), pour [`t!`](crate::t).
pub fn value(v: impl IntoValue) -> Value {
    v.into_value()
}

/// Types acceptés comme valeurs de [`t!`](crate::t).
pub trait IntoValue {
    /// La valeur Fluent.
    fn into_value(self) -> Value;
}

macro_rules! numbers {
    ($($t:ty),*) => {$(
        impl IntoValue for $t {
            fn into_value(self) -> Value {
                FluentValue::from(self as f64)
            }
        }
    )*};
}
numbers!(u8, u16, u32, u64, usize, i32, i64);

impl IntoValue for String {
    fn into_value(self) -> Value {
        FluentValue::from(self)
    }
}

impl IntoValue for &String {
    fn into_value(self) -> Value {
        FluentValue::from(self.clone())
    }
}

impl IntoValue for &str {
    fn into_value(self) -> Value {
        FluentValue::from(self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_are_recognised() {
        assert_eq!(Language::from_locale("fr-FR"), Some(Language::Fr));
        assert_eq!(Language::from_locale("en_US.UTF-8"), Some(Language::En));
        assert_eq!(Language::from_locale("FR"), Some(Language::Fr));
        assert_eq!(Language::from_locale("de-DE"), None);
        assert_eq!(Language::from_locale(""), None);
    }

    #[test]
    fn both_catalogs_define_the_same_ids() {
        let fr = ids(ENGINE[0].1);
        let mut en = ids(ENGINE[1].1);
        let mut fr_sorted = fr.clone();
        fr_sorted.sort();
        en.sort();
        assert!(!fr.is_empty());
        assert_eq!(
            fr_sorted, en,
            "mêmes identifiants en français et en anglais"
        );
        let mut unique = fr_sorted.clone();
        unique.dedup();
        assert_eq!(unique, fr_sorted, "pas d'identifiant en double");
    }

    #[test]
    fn plurals_follow_the_language() {
        // Bundles directs : la langue globale est partagée entre les tests.
        let fr = translator(Language::Fr);
        let en = translator(Language::En);
        let n = |t: &Translator, count: i64| {
            let mut args = FluentArgs::new();
            args.set("n", FluentValue::from(count as f64));
            format_in(&t.bundles, "images-count", Some(&args))
        };
        assert_eq!(n(&fr, 0), "0 image");
        assert_eq!(n(&fr, 1), "1 image");
        assert_eq!(n(&fr, 2), "2 images");
        assert_eq!(n(&en, 0), "0 images");
        assert_eq!(n(&en, 1), "1 image");
        assert_eq!(format_in(&en.bundles, "inconnu-xyz", None), "inconnu-xyz");
    }
}
