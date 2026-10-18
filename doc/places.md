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

## Emplacements masqués (`PlaceSettings`, section `[places]`)

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

## Surveillance des volumes (`Monitor`)

Un disque ou une carte branchés apparaissent d'eux-mêmes (demandé le 17/10/2026) :
`Monitor::start(réglages, on_change)` relit la liste des emplacements toutes les
`[places] watch_ms` (2000 ms ; 0 : pas de surveillance) dans un fil d'exécution à part, et
appelle `on_change` seulement quand elle change. L'interface y répond par
`Tree::set_places` (ce qui était déplié le reste).

- **Pourquoi relire plutôt qu'écouter le système** : les notifications de montage
  diffèrent sur macOS, Linux et Windows et demandent une dépendance ; relire `/Volumes`
  coûte quelques appels système, pour au plus deux secondes de retard.
- **Pourquoi un fil à part** : un volume réseau qui ne répond plus peut bloquer une
  lecture plusieurs secondes ; c'est alors le fil qui attend, pas l'interface.
- La liste de départ est lue **avant** de lancer le fil : un volume branché juste après
  l'appel est bien signalé (course trouvée par le test).
- Arrêt à la destruction (`Drop`) : le fil est réveillé (`unpark`) et attendu, sans
  attendre la fin de l'intervalle.
- `Monitor::with_source(intervalle, source, on_change)` surveille une liste quelconque
  (tests : un dossier temporaire où l'on « branche » un volume).
- Vérifié sur macOS (17/10/2026) avec une image disque montée par `hdiutil attach` :
  apparition en moins de 2 s, disparition au `detach`.

## Éjection (`eject(volume)`)

Avec l'outil du système (demandé le 17/10/2026) : `diskutil eject` (macOS) ; sous Linux,
`gio mount -e` (l'outil de l'environnement de bureau, comme le gestionnaire de fichiers),
sinon `udisksctl unmount` puis `power-off` sur le périphérique lu dans `/proc/mounts`
(`device_of`, espaces écrits `\040`) ; Windows : verbe « Éjecter » de l'explorateur via
PowerShell (**non testé**). Bloquant (de 0,4 s à quelques secondes) : l'interface l'appelle
dans un fil à part. En cas de refus (disque interne, fichier ouvert), le message du
système est renvoyé tel quel. Un disque USB « fixe » (disque dur externe) s'éjecte comme
une carte, comme dans le Finder. Vérifié sur macOS avec une image disque : éjectée en
0,36 s ; volume absent : « diskutil : Failed to find disk ».

Les lignes d'emplacement de `Tree::rows` portent leur nature (`Row::kind`) et
`Row::ejectable` : seuls les volumes non **fixés** proposent l'éjection. `[places] fixed`
(mêmes motifs qu'`exclude`) désigne les disques externes toujours branchés (travail,
sauvegarde) ; `PlaceSettings::ejectable`, et `Tree::can_eject` à vérifier avant `eject`
(l'interface le fait au moment de confirmer).

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

## Curseur clavier (18/10/2026)

`Tree` garde une ligne pointée au clavier (panneau des dossiers façon tmux, voir
`gui.md`) : `cursor` (rang dans `rows`), `cursor_path`, `set_cursor`, `point_at`,
`move_cursor`, `cursor_to_end`, `cursor_right` (déplie ; déjà déplié : premier
sous-dossier), `cursor_left` (replie ; déjà replié : dossier parent). Le curseur retient
son rang et son dossier : un dossier peut paraître sous deux emplacements (le rang
départage), et des lignes peuvent paraître au-dessus (le dossier retrouve la ligne). Dans
le moteur, pour un futur TUI.

