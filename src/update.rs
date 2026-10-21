//! Recherche d'une nouvelle version (21/10/2026), **sur demande seulement** : aucune
//! requête au démarrage, aucune notification. La liste des versions publiées est lue sur
//! l'API publique des Releases GitHub (sans compte), par `curl` (présent sur macOS, Linux
//! et Windows 10+) : pas de pile TLS à embarquer ni de dépendance de plus. Voir
//! `doc/en/update.md`.

use std::cmp::Ordering;
use std::fmt;
use std::process::Command;

use serde::Deserialize;

/// Section `[update]` du fichier de configuration.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UpdateSettings {
    /// Adresse de la liste des versions (API des Releases GitHub).
    pub url: String,
    /// Délai maximal de la requête (secondes).
    pub timeout_s: u64,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        UpdateSettings {
            url: "https://api.github.com/repos/fkinnar/tri/releases".into(),
            timeout_s: 10,
        }
    }
}

/// Une version publiée.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// Numéro (« 1.0.0-beta.1 », sans le « v » du tag).
    pub version: String,
    /// Page de la version (téléchargements).
    pub url: String,
    /// Notes de la version (Markdown).
    pub notes: String,
}

/// Erreur de la recherche.
#[derive(Debug)]
pub enum UpdateError {
    /// `curl` introuvable.
    NoCurl(std::io::Error),
    /// Requête échouée (réseau, délai, réponse HTTP) : message de `curl`.
    Request(String),
    /// Réponse illisible.
    Format(String),
}

impl fmt::Display for UpdateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCurl(e) => write!(f, "{}", crate::t!("update-no-curl", error = e.to_string())),
            Self::Request(m) => write!(f, "{}", crate::t!("update-request-failed", error = m)),
            Self::Format(m) => write!(f, "{}", crate::t!("update-bad-answer", error = m)),
        }
    }
}

impl std::error::Error for UpdateError {}

/// Numéro de version au sens de SemVer : `majeur.mineur.correctif[-préversion]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    core: [u64; 3],
    pre: Vec<String>,
}

impl Version {
    /// Lit « 1.0.0-alpha.3 » (un « v » en tête est accepté) ; `None` si illisible.
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.trim().trim_start_matches('v');
        let text = text.split('+').next()?; // métadonnées de build : ignorées
        let (core, pre) = match text.split_once('-') {
            Some((c, p)) => (c, p.split('.').map(str::to_string).collect()),
            None => (text, Vec::new()),
        };
        let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
        let core = [parts.next()??, parts.next()??, parts.next()??];
        parts.next().is_none().then_some(Version { core, pre })
    }
}

impl Ord for Version {
    /// Ordre SemVer : une préversion passe avant la version finale ; ses identifiants se
    /// comparent un à un, numériques en nombres, avant les textuels.
    fn cmp(&self, other: &Self) -> Ordering {
        self.core
            .cmp(&other.core)
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => {
                    for (a, b) in self.pre.iter().zip(&other.pre) {
                        let order = match (a.parse::<u64>(), b.parse::<u64>()) {
                            (Ok(x), Ok(y)) => x.cmp(&y),
                            (Ok(_), Err(_)) => Ordering::Less,
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => a.cmp(b),
                        };
                        if order != Ordering::Equal {
                            return order;
                        }
                    }
                    self.pre.len().cmp(&other.pre.len())
                }
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Versions publiées, depuis la réponse JSON de l'API des Releases GitHub (brouillons
/// écartés, préversions gardées : `tri` est en alpha / bêta).
pub fn parse_releases(json: &str) -> Result<Vec<Release>, UpdateError> {
    #[derive(Deserialize)]
    struct Item {
        tag_name: String,
        html_url: String,
        #[serde(default)]
        body: Option<String>,
        #[serde(default)]
        draft: bool,
    }
    let items: Vec<Item> =
        serde_json::from_str(json).map_err(|e| UpdateError::Format(e.to_string()))?;
    Ok(items
        .into_iter()
        .filter(|i| !i.draft)
        .map(|i| Release {
            version: i.tag_name.trim_start_matches('v').to_string(),
            url: i.html_url,
            notes: i.body.unwrap_or_default(),
        })
        .collect())
}

/// La plus récente des versions plus récentes que `current`, s'il y en a une.
pub fn newer<'a>(current: &str, releases: &'a [Release]) -> Option<&'a Release> {
    let current = Version::parse(current)?;
    releases
        .iter()
        .filter_map(|r| Version::parse(&r.version).map(|v| (v, r)))
        .filter(|(v, _)| *v > current)
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, r)| r)
}

/// Lit la liste des versions publiées (`curl`, délai `settings.timeout_s`).
pub fn fetch(settings: &UpdateSettings) -> Result<Vec<Release>, UpdateError> {
    let mut command = Command::new("curl");
    command.args([
        "--silent",
        "--show-error",
        "--fail",
        "--location",
        "--max-time",
        &settings.timeout_s.to_string(),
        "--header",
        "Accept: application/vnd.github+json",
        "--user-agent",
        concat!("tri/", env!("CARGO_PKG_VERSION")),
        &settings.url,
    ]);
    // Windows : pas de fenêtre de console le temps de la requête (interface graphique).
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let output = command.output().map_err(UpdateError::NoCurl)?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(UpdateError::Request(message));
    }
    parse_releases(&String::from_utf8_lossy(&output.stdout))
}

/// Recherche une version plus récente que `current` : `None` si `current` est la
/// dernière.
pub fn check(current: &str, settings: &UpdateSettings) -> Result<Option<Release>, UpdateError> {
    let releases = fetch(settings)?;
    Ok(newer(current, &releases).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn versions_follow_semver_precedence() {
        let ordered = [
            "1.0.0-alpha.2",
            "1.0.0-alpha.3",
            "1.0.0-alpha.10",
            "1.0.0-beta",
            "1.0.0-beta.1",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.1.0",
            "2.0.0",
        ];
        for pair in ordered.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
        }
        assert_eq!(v("v1.0.0-alpha.3"), v("1.0.0-alpha.3"));
        assert_eq!(v("1.0.0+build.5"), v("1.0.0"));
        for bad in ["", "1.0", "1.0.x", "1.0.0.0", "latest"] {
            assert!(Version::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn the_newest_release_above_the_current_version_is_offered() {
        let json = r#"[
            {"tag_name": "v1.0.0-beta.1", "html_url": "https://x/b1", "body": "Bêta", "draft": false},
            {"tag_name": "v1.0.0-beta.2", "html_url": "https://x/b2", "body": null, "draft": true},
            {"tag_name": "v1.0.0-alpha.3", "html_url": "https://x/a3", "draft": false},
            {"tag_name": "nightly", "html_url": "https://x/n", "draft": false}
        ]"#;
        let releases = parse_releases(json).unwrap();
        assert_eq!(releases.len(), 3, "brouillon écarté");
        let found = newer("1.0.0-alpha.3", &releases).unwrap();
        assert_eq!(found.version, "1.0.0-beta.1");
        assert_eq!(found.url, "https://x/b1");
        assert_eq!(found.notes, "Bêta");
        assert!(
            newer("1.0.0-beta.1", &releases).is_none(),
            "déjà la dernière"
        );
        assert!(
            newer("1.0.0", &releases).is_none(),
            "finale après les préversions"
        );
        assert!(parse_releases("{oops").is_err());
    }
}
