# Rechercher une mise à jour (`tri::update`)

Ajouté le 21/10/2026, pour les testeurs de la bêta : savoir s'il existe une version plus
récente sans aller sur GitHub.

## Principe

- **Sur demande seulement** : bouton « Rechercher une mise à jour » en haut de l'aide
  (F1), action `core.app.check_update` (sans raccourci par défaut, attribuable dans
  `[keys]`), ou `tri --check-update`. Rien ne part sur le réseau sans ce geste : ni
  vérification au démarrage, ni notification.
- **Source** : l'API publique des Releases GitHub (`[update] url`, sans compte ni jeton),
  lue par **`curl`** (présent sur macOS, Linux et Windows 10+), délai `[update]
  timeout_s` (10 s). Pas de client HTTP dans `tri` : ni pile TLS à embarquer, ni licences
  de plus. Sans `curl` ou sans réseau, l'erreur s'affiche telle quelle.
- **Comparaison** : numéros SemVer (`Version`), préversions comprises (`tri` est en alpha
  puis en bêta) : `1.0.0-alpha.3` < `1.0.0-beta.1` < `1.0.0`. Brouillons écartés, tags
  illisibles ignorés.
- **Réponse** : « Vous avez la dernière version », ou la version plus récente avec ses
  notes (celles de la page Releases, tirées de `CHANGELOG.md` et `CHANGELOG.fr.md`, montrées dans la langue de l'interface : `Release::notes_in`, la partie française après la marque `<!-- tri:fr -->` écrite par la CI ; notes plus anciennes telles quelles) et un bouton qui ouvre sa
  page dans le navigateur. **Pas d'installation automatique** (après la V1 : signature,
  droits, remplacement de l'application en cours).
- **Builds de nuit et personnels** (22/10/2026, pour les têtes en l'air) : un binaire
  qui n'est pas une version publiée le dit, et compte les commits de la branche `main`
  qui lui manquent (API « compare » de GitHub, `[update] compare_url`) : « git pull, puis
  recompiler » pour un build personnel, « dans le prochain build de nuit » pour un build
  de nuit ; « à jour », ou « impossible de comparer » si son commit n'est pas sur GitHub
  (commits locaux pas encore poussés). La provenance est gravée à la compilation par
  `build.rs` : le commit (`git rev-parse HEAD`, recompilé quand `HEAD` bouge) et
  `TRI_BUILD_CHANNEL`, posée par la CI (`release` sur un tag, `nightly` pour le build de
  nuit, voir [packaging.md](packaging.md)) ; sans elle, build personnel. Le numéro de
  version de l'aide et du logo la porte : `1.0.0-alpha.3 (e51c81b)`,
  `1.0.0-alpha.3 nightly e51c81b`.

## API

| Élément | Rôle |
|---|---|
| `UpdateSettings` | section `[update]` : `url`, `compare_url`, `timeout_s` |
| `Build::current()` | ce binaire : version, commit, `Channel` (publiée, de nuit, personnel) ; `label()` |
| `Version::parse` | « 1.0.0-alpha.3 » (« v » en tête accepté), ordre SemVer |
| `parse_releases(json)` | versions publiées depuis la réponse de l'API (brouillons écartés) |
| `newer(current, releases)` | la plus récente des versions plus récentes que `current` |
| `parse_compare(json)` | commits de `main` absents du build (`ahead_by` de `commit...main`) |
| `fetch(settings)` / `check(build, settings)` | requête par `curl`, puis `Status` : `Latest`, `Newer(release)`, `Build { behind }` |
| `build_text(build, behind)` | les deux lignes affichées pour un build de nuit ou personnel |
| `Release::notes_in(langue)` | notes de la version dans la langue de l'interface |
| `report_line(build)` / `issue_url(build, settings)` | version et système en une ligne (copiés depuis l'aide) ; formulaire de signalement prérempli (`[update] issue_url`, champs `version`, `system`, `system-details` du modèle `bug.yml`) |

Dans l'interface, la requête tourne dans un fil à part (`TriApp::check_update`) ;
la boîte (`update_dialog`) montre « recherche en cours », puis le résultat ; Échap ferme.
En haut de l'aide aussi (issue n° 7) : la version en évidence, une icône qui la copie
avec le système, et « Signaler un problème », qui ouvre le formulaire de GitHub prérempli.

## Tests

Unitaires : ordre SemVer (préversions numériques et textuelles, version finale après ses
préversions, numéros illisibles), lecture d'une réponse (brouillon écarté, tag non
numérique ignoré, version proposée), builds personnels et de nuit (commits à récupérer
depuis une réponse « compare » locale, pas de requête sans commit, libellé). Interface :
liste servie par un fichier local (`file://`, lu par `curl`, aucun réseau) : version
plus récente avec ses notes, puis « dernière version » ; build personnel avec des
commits à récupérer.
