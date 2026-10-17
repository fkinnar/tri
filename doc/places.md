# Module `tri::places`

Emplacements proposés pour choisir un dossier, et arborescence des dossiers, **sans
interface** : l'interface graphique ne fait que dessiner les lignes de [`Tree::rows`] ; un
TUI ou un autre affichage pourra faire de même.

## Emplacements (`places()`)

| Nature | macOS | Linux et autres Unix | Windows |
|---|---|---|---|
| `Volume` | `/Volumes/*` | `/media/<USER>/*`, `/run/media/<USER>/*`, `/mnt/*` | lettres de lecteur `C:\`… |
| `Home` | `$HOME` | `$HOME` | `%USERPROFILE%` |
| `Root` | `/` | `/` | — |

Pourquoi pas seulement le dossier personnel : une carte SD est montée dans `/Volumes`
(macOS) ou `/media/<utilisateur>` (Linux), pas dans `~` (retour du 17/10/2026). La racine
donne l'arborescence complète. Les liens symboliques de `/Volumes` sont écartés
(`Macintosh HD` pointe vers `/`). L'OS et l'environnement sont injectés dans les fonctions
internes (`volume_dirs`, `home_dir`) pour être testés sur toutes les plateformes, comme
`settings::default_path`.

## Emplacements masqués (`PlaceFilter`, section `[places]`)

- `hide_backups` (défaut : vrai) : les volumes de sauvegarde **Time Machine** sont
  masqués, reconnus à leur **contenu** et pas à leur nom (choisi par chacun) :
  `backup_manifest.plist` à la racine (disque APFS), `Backups.backupdb` (disque HFS,
  volume des instantanés locaux `com.apple.TimeMachine.localsnapshots`). Vérifié sur la
  machine de développement (17/10/2026) : « TimeMachine » et les instantanés disparaissent,
  « Backup » (un disque ordinaire) et « Work » restent ;
- `exclude` : motifs comparés au **nom** (`"Backup"`) ou au **chemin** (`"/mnt/*"`), sans
  tenir compte de la casse, avec `*` et `?` (petite fonction `wildcard`, sans dépendance) ;
  s'applique à tous les emplacements (on peut masquer la racine) ;
- `places_filtered(&filtre)` ; `Tree::new(filtre, cachés)` garde le filtre pour
  `refresh`.

## Sous-dossiers (`subfolders(dir, hidden)`)

Triés par nom sans tenir compte de la casse ; dossiers cachés (`.xxx`) seulement si
`hidden` ; liens vers des dossiers suivis ; erreur si `dir` est illisible.

## Arborescence (`Tree`)

- rien n'est parcouru d'avance : un dossier est lu quand on le **déplie** (`expand`,
  `toggle`), puis gardé ; instantané même sur un NAS ;
- `rows()` : lignes visibles à plat (`Row { path, name, depth, expanded, error }`), dans
  l'ordre d'affichage ; un dossier illisible déplié porte son erreur ;
- `reveal(path)` : déplie les dossiers parents de `path` sous l'emplacement le plus précis
  qui le contient (un volume avant la racine), pour montrer le dossier ouvert ;
- `refresh()` : relit les emplacements (carte insérée) et les dossiers dépliés ;
  `reload()` : seulement les dossiers ;
- un même dossier présent sous deux emplacements (`/Volumes/SD` sous « Volumes » et sous
  la racine) est déplié aux deux endroits (état rangé par chemin).
