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

## Dossiers personnels et réseau (20/10/2026)

- **Dossiers habituels**, comme la barre latérale du Finder : Bureau, Documents,
  Téléchargements, Images, après le dossier personnel (`user_dirs`). Sous Linux, ceux de
  `~/.config/user-dirs.dirs` (`xdg-user-dirs` : « Bureau », « Images » sous Mint en
  français, chemin absolu permis) ; sinon les noms anglais. Absents : omis.
- **Partages réseau** (`PlaceKind::Network`) : points de montage d'un système de fichiers
  réseau (SMB, AFP, NFS, WebDAV, SSHFS, rclone…), lus dans la sortie de `mount` (macOS)
  ou `/proc/mounts` (Linux) ; un partage monté dans `/Volumes` ou `/media` change de
  section, les autres (`/etc/fstab`, `~/nas`) s'ajoutent. Sous Linux aussi les montages
  **GVFS** du gestionnaire de fichiers (`/run/user/<uid>/gvfs` : `smb-share:server=nas,
  share=photo` devient « photo (nas) ») ; un téléphone (`mtp`) ou un appareil photo
  (`gphoto2`) y est un volume. Pas d'éjection proposée pour un partage ; rien sous
  Windows (lecteurs réseau : lettres de lecteur).

## Sous-dossiers (`subfolders(dir, hidden)`)

Triés par nom sans tenir compte de la casse ; dossiers cachés (`.xxx`) seulement si
`hidden` ; liens vers des dossiers suivis ; erreur si `dir` est illisible.

## Arborescence (`Tree`)

- rien n'est parcouru d'avance : un dossier est lu quand on le **déplie** (`expand`,
  `toggle`), puis gardé ; instantané même sur un NAS ;
- `rows()` : lignes visibles à plat (`Row { path, name, depth, expanded, error, kind,
  ejectable, favorite, section }`), dans l'ordre d'affichage ; un dossier illisible
  déplié porte son erreur ;
- **sections** (demandé le 20/10/2026 : trié, et passer le moins possible par la
  racine) : `Section::Drives` (volumes montés), `Network` (partages réseau),
  `Personal` (dossier personnel et ses dossiers habituels), `Favorites`, puis seulement
  `System` (la racine, repliée tant qu'on ne l'ouvre pas) ; `PlaceKind::section`, et `Row::section` sur chaque ligne (les
  sous-dossiers prennent celle de leur emplacement) : l'interface dessine un titre et un
  trait à chaque changement de section ;
- `reveal(path)` : déplie les dossiers parents de `path` sous l'emplacement le plus précis
  qui le contient (un volume avant la racine), pour montrer le dossier ouvert ;
- `refresh()` : relit les emplacements (carte insérée) et les dossiers dépliés ;
  `reload()` : seulement les dossiers ;
- un même dossier présent sous deux emplacements (`/Volumes/SD` sous « Volumes » et sous
  la racine) est déplié aux deux endroits (état rangé par chemin).

## Recherche de dossiers (19/10/2026)

`/` ou Ctrl+F dans le panneau des dossiers. `parse_dir_query(texte, courant, home)` lit
d'où part la recherche : `~/…` du dossier personnel, `/…` de la racine ou du plus long
chemin existant tapé en tête (`/Volumes/Work/dehors`), sinon du dossier pointé ; le reste
est le motif, cherché en flou (`tri::fuzzy`) sur le chemin relatif. `walk_dirs` parcourt
en largeur (les plus proches d'abord), borné en profondeur (`[gui.folders]
search_depth`), sans dossiers cachés ni sauvegardes Time Machine, annulable.
`DirSearch` le fait en fond : résultats au fil de l'eau (`take`), `search_results` au
plus, arrêt quand on la lâche (nouvelle lettre). `home()` : le dossier personnel.

## Favoris (18/10/2026)

Dossiers choisis par l'utilisateur, montrés en tête de l'arborescence (avant les
volumes), rangés dans le dossier de données (`[data] dir`), fichier `favorites`, un
chemin par ligne, dans l'ordre d'ajout :

- `favorites(data)` : la liste, absents compris : un favori sur une carte ou un disque
  débranché revient quand le volume est remonté ;
- `set_favorite(data, dir, on)` : ajoute (à la fin) ou retire, renvoie la liste ;
- `rename_favorites(data, from, to)` : après un renommage, les favoris qui sont `from` ou
  dedans suivent ;
- `Tree::set_favorites(liste)` : lignes de profondeur 0 de nature
  `PlaceKind::Favorite` ; l'existence des dossiers est vérifiée là, et à chaque
  `set_places` / `refresh`, pas à chaque dessin (NAS lent) ; `Row::favorite` marque
  aussi le dossier là où il paraît dans l'arborescence.

Pas encore : réordonner les favoris (ordre d'ajout), suivre un renommage *défait*
(`u`) ou fait hors de `tri` (le favori disparaît alors de la liste, jusqu'à ce que le
dossier revienne).

## Curseur clavier (18/10/2026)

`Tree` garde une ligne pointée au clavier (panneau des dossiers façon tmux, voir
`gui.md`) : `cursor` (rang dans `rows`), `cursor_path`, `set_cursor`, `point_at`,
`move_cursor`, `cursor_to_end`, `cursor_right` (déplie ; déjà déplié : premier
sous-dossier), `cursor_left` (replie ; déjà replié : dossier parent). Le curseur retient
son rang et son dossier : un dossier peut paraître sous deux emplacements (le rang
départage), et des lignes peuvent paraître au-dessus (le dossier retrouve la ligne). Dans
le moteur, pour un futur TUI.

