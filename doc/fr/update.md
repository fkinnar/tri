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
  notes (celles de la page Releases, tirées du `CHANGELOG.md`) et un bouton qui ouvre sa
  page dans le navigateur. **Pas d'installation automatique** (après la V1 : signature,
  droits, remplacement de l'application en cours).

## API

| Élément | Rôle |
|---|---|
| `UpdateSettings` | section `[update]` : `url`, `timeout_s` |
| `Version::parse` | « 1.0.0-alpha.3 » (« v » en tête accepté), ordre SemVer |
| `parse_releases(json)` | versions publiées depuis la réponse de l'API (brouillons écartés) |
| `newer(current, releases)` | la plus récente des versions plus récentes que `current` |
| `fetch(settings)` / `check(current, settings)` | requête par `curl`, puis comparaison |

Dans l'interface, la requête tourne dans un fil à part (`TriApp::check_update`) ;
la boîte (`update_dialog`) montre « recherche en cours », puis le résultat ; Échap ferme.

## Tests

Unitaires : ordre SemVer (préversions numériques et textuelles, version finale après ses
préversions, numéros illisibles), lecture d'une réponse (brouillon écarté, tag non
numérique ignoré, version proposée). Interface : liste servie par un fichier local
(`file://`, lu par `curl`, aucun réseau) : version plus récente avec ses notes, puis
« dernière version ».
