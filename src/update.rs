//! Recherche d'une nouvelle version (21/10/2026), **sur demande seulement** : aucune
//! requête au démarrage, aucune notification. Un build personnel (compilé depuis les
//! sources) le dit, et compte plutôt les commits de `main` à récupérer (22/10/2026). La liste des versions publiées est lue sur
//! l'API publique des Releases GitHub (sans compte), par `curl` (présent sur macOS, Linux
//! et Windows 10+) : pas de pile TLS à embarquer ni de dépendance de plus. Voir
//! `doc/en/update.md`.

use std::cmp::Ordering;
use std::fmt;

use serde::Deserialize;

use crate::i18n::Language;

/// Section `[update]` du fichier de configuration.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UpdateSettings {
    /// Adresse de la liste des versions (API des Releases GitHub).
    pub url: String,
    /// Comparaison d'un build personnel avec la branche publiée (API GitHub « compare »),
    /// `{commit}` remplacé par le commit compilé.
    pub compare_url: String,
    /// Délai maximal de la requête (secondes).
    pub timeout_s: u64,
    /// Formulaire de signalement d'un problème (issue n° 7) : version et système y sont
    /// ajoutés en paramètres, qui préremplissent les champs du même nom.
    pub issue_url: String,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        UpdateSettings {
            url: "https://api.github.com/repos/fkinnar/tri/releases".into(),
            compare_url: "https://api.github.com/repos/fkinnar/tri/compare/{commit}...main".into(),
            timeout_s: 10,
            issue_url: "https://github.com/fkinnar/tri/issues/new?template=bug.yml".into(),
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

/// Marque, dans les notes d'une Release, le début de la partie française (22/10/2026) :
/// la CI écrit l'anglais (`CHANGELOG.md`), puis cette marque (invisible sur GitHub) et le
/// français (`CHANGELOG.fr.md`) replié dans un `<details>`.
pub const FRENCH_NOTES: &str = "<!-- tri:fr -->";

impl Release {
    /// Notes dans la langue voulue : la partie française après [`FRENCH_NOTES`], sinon
    /// l'anglaise avant. Notes sans marque (versions plus anciennes) : telles quelles.
    pub fn notes_in(&self, language: Language) -> &str {
        let Some((english, french)) = self.notes.split_once(FRENCH_NOTES) else {
            return self.notes.trim();
        };
        match language {
            Language::En => english.trim(),
            Language::Fr => {
                let french = french.trim();
                let french = french
                    .strip_prefix("<details>")
                    .map(|rest| rest.split_once("</summary>").map_or(rest, |(_, r)| r))
                    .unwrap_or(french);
                french.trim().trim_end_matches("</details>").trim()
            }
        }
    }
}

/// Nom du système, tel que le propose le formulaire de signalement (`macOS`, `Linux`,
/// `Windows`), sinon celui de Rust.
pub fn system_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macOS",
        "linux" => "Linux",
        "windows" => "Windows",
        other => other,
    }
}

/// Version et système en une ligne, à coller dans un signalement :
/// `tri 1.0.0-alpha.4 (abc1234), macOS aarch64`.
pub fn report_line(build: &Build) -> String {
    format!(
        "tri {}, {} {}",
        build.label(),
        system_name(),
        std::env::consts::ARCH
    )
}

/// Adresse du formulaire de signalement, préremplie : version (avec le commit d'un build
/// de nuit ou personnel), système et architecture.
pub fn issue_url(build: &Build, settings: &UpdateSettings) -> String {
    let separator = if settings.issue_url.contains('?') {
        '&'
    } else {
        '?'
    };
    format!(
        "{}{separator}version={}&system={}&system-details={}",
        settings.issue_url,
        encode(&build.label()),
        encode(system_name()),
        encode(std::env::consts::ARCH)
    )
}

/// Encodage d'un paramètre d'adresse (RFC 3986 : seuls lettres, chiffres et `-._~`
/// restent tels quels).
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Ce binaire, tel que compilé (`build.rs`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Build {
    /// Numéro de version (`Cargo.toml`).
    pub version: String,
    /// Commit compilé, si les sources venaient d'un dépôt git.
    pub commit: Option<String>,
    /// Provenance : version publiée, build de nuit ou build personnel.
    pub channel: Channel,
}

/// Provenance d'un binaire (`TRI_BUILD_CHANNEL` à la compilation, voir `build.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// Version publiée (paquet de la page Releases, compilé par la CI sur un tag).
    Release,
    /// Build de nuit (CI, chaque nuit où `main` a changé ; release « nightly »).
    Nightly,
    /// Compilé depuis les sources.
    Personal,
}

impl Channel {
    /// Depuis la valeur de `TRI_BUILD_CHANNEL` (absente ou inconnue : build personnel).
    pub fn parse(text: Option<&str>) -> Channel {
        match text {
            Some("release") => Channel::Release,
            Some("nightly") => Channel::Nightly,
            _ => Channel::Personal,
        }
    }
}

impl Build {
    /// Le binaire en cours.
    pub fn current() -> Build {
        Build {
            version: env!("CARGO_PKG_VERSION").into(),
            commit: option_env!("TRI_COMMIT").map(str::to_string),
            channel: Channel::parse(option_env!("TRI_CHANNEL")),
        }
    }

    /// Commit abrégé (7 caractères, comme git).
    pub fn short_commit(&self) -> Option<&str> {
        self.commit.as_deref().map(|c| &c[..c.len().min(7)])
    }

    /// « 1.0.0-beta.1 » ; « 1.0.0-beta.1 nightly 1a2b3c4 » pour un build de nuit,
    /// « 1.0.0-beta.1 (1a2b3c4) » pour un build personnel.
    pub fn label(&self) -> String {
        let commit = self.short_commit().unwrap_or("?");
        match self.channel {
            Channel::Release => self.version.clone(),
            Channel::Nightly => format!("{} nightly {commit}", self.version),
            Channel::Personal => format!("{} ({commit})", self.version),
        }
    }
}

/// Réponse de la recherche.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Version distribuée, la dernière publiée.
    Latest,
    /// Version distribuée, une plus récente est publiée.
    Newer(Release),
    /// Build de nuit ou personnel : nombre de commits de `main` absents de ce build
    /// (0 = à jour), `None` si on ne peut pas savoir (commit inconnu, ou absent de
    /// GitHub : commits locaux pas encore poussés).
    Build {
        /// Commits de `main` absents de ce build.
        behind: Option<u64>,
    },
}

/// Ce que dit la recherche pour un build de nuit ou personnel : sa provenance, puis les
/// commits de `main` à récupérer (prochain build de nuit, ou `git pull`), à jour, ou
/// inconnu.
pub fn build_text(build: &Build, behind: Option<u64>) -> [String; 2] {
    let commit = build.short_commit().unwrap_or("?");
    let nightly = build.channel == Channel::Nightly;
    let first = if nightly {
        crate::t!("update-nightly", commit = commit)
    } else {
        crate::t!("update-build", commit = commit)
    };
    let second = match behind {
        Some(0) => crate::t!("update-build-current"),
        Some(n) if nightly => crate::t!("update-nightly-behind", count = n),
        Some(n) => crate::t!("update-build-behind", count = n),
        None => crate::t!("update-build-unknown"),
    };
    [first, second]
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

/// Commits de la branche publiée absents du build, depuis la réponse de l'API
/// « compare » (`base...main`, base = le commit du build : `ahead_by`).
pub fn parse_compare(json: &str) -> Result<u64, UpdateError> {
    #[derive(Deserialize)]
    struct Compare {
        ahead_by: u64,
    }
    serde_json::from_str::<Compare>(json)
        .map(|c| c.ahead_by)
        .map_err(|e| UpdateError::Format(e.to_string()))
}

/// Requête GET par `curl` : code HTTP (0 pour un `file://`) et corps.
fn get(url: &str, settings: &UpdateSettings) -> Result<(u32, String), UpdateError> {
    let mut command = crate::process::command("curl");
    command.args([
        "--silent",
        "--show-error",
        "--location",
        "--max-time",
        &settings.timeout_s.to_string(),
        "--header",
        "Accept: application/vnd.github+json",
        "--user-agent",
        concat!("tri/", env!("CARGO_PKG_VERSION")),
        "--write-out",
        "\n%{http_code}",
        url,
    ]);
    let output = command.output().map_err(UpdateError::NoCurl)?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(UpdateError::Request(message));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", &text));
    Ok((code.trim().parse().unwrap_or(0), body.to_string()))
}

/// Erreur pour un code HTTP d'échec.
fn http_error(code: u32) -> UpdateError {
    UpdateError::Request(format!("HTTP {code}"))
}

/// Lit la liste des versions publiées (`curl`, délai `settings.timeout_s`).
pub fn fetch(settings: &UpdateSettings) -> Result<Vec<Release>, UpdateError> {
    match get(&settings.url, settings)? {
        (code, _) if code >= 400 => Err(http_error(code)),
        (_, body) => parse_releases(&body),
    }
}

/// Recherche une mise à jour de `build` : version publiée plus récente, ou, pour un
/// build personnel, commits de `main` à récupérer (`git pull`).
pub fn check(build: &Build, settings: &UpdateSettings) -> Result<Status, UpdateError> {
    if build.channel != Channel::Release {
        let Some(commit) = &build.commit else {
            return Ok(Status::Build { behind: None });
        };
        let url = settings.compare_url.replace("{commit}", commit);
        return match get(&url, settings)? {
            // Commit absent de GitHub (pas encore poussé, ou d'un autre dépôt).
            (404 | 422, _) => Ok(Status::Build { behind: None }),
            (code, _) if code >= 400 => Err(http_error(code)),
            (_, body) => parse_compare(&body).map(|n| Status::Build { behind: Some(n) }),
        };
    }
    let releases = fetch(settings)?;
    Ok(match newer(&build.version, &releases) {
        Some(release) => Status::Newer(release.clone()),
        None => Status::Latest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_issue_form_is_prefilled_with_version_and_system() {
        let build = Build {
            version: "1.0.0-alpha.4".into(),
            commit: Some("abc1234def".into()),
            channel: Channel::Personal,
        };
        let url = issue_url(&build, &UpdateSettings::default());
        assert!(url.starts_with("https://github.com/fkinnar/tri/issues/new?template=bug.yml&"));
        assert!(
            url.contains("&version=1.0.0-alpha.4%20%28abc1234%29&"),
            "{url}"
        );
        assert!(url.contains(&format!("&system={}&", system_name())));
        assert!(report_line(&build).starts_with("tri 1.0.0-alpha.4 (abc1234), "));
        let settings = UpdateSettings {
            issue_url: "https://example.org/new".into(),
            ..UpdateSettings::default()
        };
        assert!(issue_url(&build, &settings).starts_with("https://example.org/new?version="));
    }

    #[test]
    fn release_notes_follow_the_language() {
        let release = |notes: &str| Release {
            version: "1.0.0".into(),
            url: String::new(),
            notes: notes.into(),
        };
        let both = release(
            "- **Crop**: fixed\n\n<!-- tri:fr -->\n<details><summary>Français</summary>\n\n- **Recadrage** : corrigé\n\n</details>\n",
        );
        assert_eq!(both.notes_in(Language::En), "- **Crop**: fixed");
        assert_eq!(both.notes_in(Language::Fr), "- **Recadrage** : corrigé");
        let old = release("\n- **Notes** : seulement en français\n");
        assert_eq!(
            old.notes_in(Language::En),
            "- **Notes** : seulement en français"
        );
    }

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

    #[test]
    fn a_personal_build_counts_the_commits_to_pull() {
        let dir = tempfile::tempdir().unwrap();
        let compare = dir.path().join("compare.json");
        std::fs::write(
            &compare,
            r#"{"status": "behind", "ahead_by": 3, "behind_by": 0}"#,
        )
        .unwrap();
        let url = format!(
            "file://{}",
            compare.display().to_string().replace('\\', "/")
        );
        let url = if url.starts_with("file:///") {
            url
        } else {
            url.replacen("file://", "file:///", 1)
        };
        let settings = UpdateSettings {
            url: "file:///nowhere".into(),
            compare_url: url.replace("compare.json", "{commit}"),
            timeout_s: 5,
            ..UpdateSettings::default()
        };
        // Le fichier porte le nom du « commit » : l'adresse est bien construite.
        std::fs::rename(&compare, dir.path().join("abc1234def")).unwrap();
        let mut build = Build {
            version: "1.0.0-alpha.3".into(),
            commit: Some("abc1234def".into()),
            channel: Channel::Personal,
        };
        assert_eq!(build.label(), "1.0.0-alpha.3 (abc1234)");
        assert_eq!(
            check(&build, &settings).unwrap(),
            Status::Build { behind: Some(3) }
        );
        build.channel = Channel::Nightly;
        assert_eq!(build.label(), "1.0.0-alpha.3 nightly abc1234");
        assert_eq!(
            check(&build, &settings).unwrap(),
            Status::Build { behind: Some(3) },
            "un build de nuit se compare aussi à main"
        );
        build.commit = None;
        assert_eq!(
            check(&build, &settings).unwrap(),
            Status::Build { behind: None },
            "sans commit, pas de requête"
        );
        build.channel = Channel::Release;
        assert_eq!(build.label(), "1.0.0-alpha.3");
        assert!(
            check(&build, &settings).is_err(),
            "liste des versions introuvable"
        );
        assert!(parse_compare("{}").is_err());
        assert_eq!(Channel::parse(Some("nightly")), Channel::Nightly);
        assert_eq!(Channel::parse(Some("release")), Channel::Release);
        assert_eq!(Channel::parse(None), Channel::Personal);
    }
}
