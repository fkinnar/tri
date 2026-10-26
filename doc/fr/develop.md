# Module `tri::develop` : rendu par un logiciel de développement

Idée de l'utilisateur (19/10/2026) : voir dans `tri` ses photos **telles qu'on les a
développées** ailleurs, darktable d'abord. Le rendu est **demandé** : `Maj+B` ou le
bouton (diaphragme) du bandeau sous l'image. Ensuite, `b` passe du JPEG boîtier au rendu
RAW, puis au **développé**, et revient au boîtier ; tant que l'image n'a pas été
développée, `b` ne propose que boîtier et RAW et ne lance jamais darktable (retour de
l'utilisateur, 19/10/2026 : en tri, le rendu de base suffit presque toujours, inutile
d'attendre 10 s pour rien).

## Un plugin compilé qui pilote un programme externe

Comme les autres plugins (superpositions, envoi), le code est en Rust et compilé avec
`tri` ; il appelle `darktable-cli` en **processus séparé** : darktable ne peut pas faire
tomber `tri`, et sa licence reste la sienne. RawTherapee (`rawtherapee-cli`, profils
`.pp3`) et ART suivraient le même modèle.

**Plusieurs logiciels** (26/10/2026, pour RapidRAW et les suivants) : chaque logiciel
est un `Engine` (identifiant, nom, sidecar qui porte son développement) et un plugin du
trait `Developer` (`render`) ; `Developers` est la liste de ceux trouvés sur la machine
(`Developers::find`, fichiers de travail sous `<cache>/develop`). Une image peut avoir
été développée dans plusieurs : `resolve` choisit celui du réglage `[develop] default`
(liste déroulante dans la fenêtre de réglages : « auto » et les logiciels installés)
s'il a développé l'image, sinon le développement le plus récent (`"auto"`, défaut). Choix
de l'utilisateur : quelqu'un qui travaille dans darktable et essaie un autre logiciel
règle `default = "darktable"`, sans que le plus récent passe devant.

- `find_darktable(réglage)` : `[develop] darktable` = `"auto"` (le `PATH`, puis les
  emplacements habituels : `/Applications/darktable.app/…` sur macOS, `/usr/bin` sous
  Linux, `Program Files` sous Windows), un chemin, ou `""` (jamais). Sous Linux, s'il
  n'est pas trouvé : une **AppImage** de darktable (`*darktable*.appimage` dans
  `~/AppImages`, `~/Applications`, `~/.local/bin`, `~/bin`, les téléchargements, `/opt` ;
  la plus récente), lancée avec `darktable-cli` en premier argument : l'AppImage exécute
  alors ce binaire au lieu de l'interface (documenté par darktable, « invocation per
  OS »). C'est l'installation de l'utilisateur sous Mint (`~/AppImages/darktable.appimage`,
  19/10/2026) : **vérifié par lui** le jour même, rendu et export fonctionnent. Un chemin d'AppImage est aussi accepté dans le réglage. Sinon, le
  **Flatpak** `org.darktable.Darktable` (installation système ou
  utilisateur, `flatpak info`), lancé par `flatpak run --command=darktable-cli
  org.darktable.Darktable` (`Cli` : programme et arguments placés devant ceux de `tri`) ;
  `"flatpak"` le choisit d'office. Constaté le 19/10/2026 sur Linux Mint : le Gestionnaire
  de logiciels installe darktable en Flatpak, sans `darktable-cli` dans le `PATH` (et son
  lanceur exporté ouvre l'interface, pas `darktable-cli`). Le Flatpak doit pouvoir lire
  les images et le cache de `tri` (dossier personnel) : à vérifier pour des images hors
  du dossier personnel.
- `darktable_sidecar(image)` : `IMAGE.RAF.xmp`, **seulement s'il porte un vrai
  développement** (`darktable:history_end` > 0) : un sidecar écrit par `tri` seul (notes)
  n'en a pas.
- `Darktable::render` : `darktable-cli image sidecar sortie --width --height --hq
  --core --configdir <cache>/darktable --library :memory: --conf
  write_sidecar_files=never`, annulable (le programme est arrêté).

## RapidRAW (26/10/2026, #114)

RapidRAW (AGPL-3.0, appelé en processus séparé) a une commande sans interface depuis
la version 1.6 : `rapidraw export <image> --output <fichier> --format jpeg --quality 95`,
qui applique d'elle-même les réglages du sidecar **`IMAGE.RAF.rrdata`** posé à côté de
l'image (`rapidraw_sidecar` ; ses copies virtuelles `IMAGE.RAF.<id>.rrdata` ne sont
pas lues). Elle exporte à pleine taille, sans option de taille : `tri` réduit ensuite
au côté de l'aperçu standard (`shrink`). Lu dans son code source : l'export **refuse de
tourner pendant que l'interface de RapidRAW est ouverte** (instance unique) ; le
message le dit en clair (« fermez-le puis recommencez »).

`find_rapidraw(réglage)` : `[develop] rapidraw` = `"auto"` (`rapidraw` / `RapidRAW`
dans le `PATH`, `/usr/bin` du paquet `.deb`, `RapidRAW.app` sur macOS, `Program Files`
sur Windows ; sous Linux, une AppImage `*rapidraw*`, puis le Flatpak
`io.github.CyberTimon.RapidRAW`), un chemin (l'exécutable ou l'AppImage, qui reçoit
`export` directement), `"flatpak"`, ou `""`. Messages du programme dans un fichier (pas
un tube : un programme bavard ne bloque pas). La vraie résolution du développé n'est
lue que pour darktable : pour RapidRAW, inconnue plutôt que fausse.

## Rien n'est écrit chez l'utilisateur

Configuration propre à `tri` (dans son cache), bibliothèque en mémoire : ni la
configuration ni la bibliothèque de darktable ne sont touchées, et darktable peut rester
ouvert (pas de verrou). `write_sidecar_files=never` : le sidecar n'est jamais réécrit,
quelle que soit la configuration ; un test le vérifie sur un vrai rendu (sur une copie).

## Cache et calcul

Niveau `Level::Developed(engine)`, un par logiciel (dossier `developed` pour darktable,
inchangé : son cache reste valable) : le JPEG du logiciel, à la taille de l'aperçu standard,
rangé **tel quel** dans le cache. Sa clé comprend l'empreinte du sidecar (taille, date) :
modifier le développement dans darktable refait le rendu. Dans l'interface, un développé
déjà affiché dont le sidecar a changé (surveillance du dossier, ou au moment de `b` /
`Maj+B`) est oublié : `b` revient au rendu de base et `Maj+B` le refait (retour du
19/10/2026 : un recadrage modifié n'était pas vu). Calcul **sur demande
explicite seulement** (`Maj+B`, image affichée ; une image déjà en cache reste dans la
rotation de `b`, même d'une séance à l'autre), par le worker lourd (darktable occupe toute la
machine), jamais tout le dossier en fond. Mesuré le 19/10/2026 (RAF X-T4, Mac Apple
Silicon) : 8 à 10 s par image, dont ~6 s de démarrage de darktable.

## Taille du développé (21/10/2026)

`developed_size(xmp, taille)` : la taille de l'image que darktable produirait à pleine
résolution, pour la résolution affichée en plein écran. Lue dans l'historique du
sidecar (`darktable:history`, entrées avant `darktable:history_end`, dernier état de
chaque module, instance principale) :

- **`crop`** : ses paramètres sont quatre flottants 32 bits petit-boutistes en
  hexadécimal, bords gauche, haut, droit et bas en fractions de l'image (vérifié sur de
  vrais sidecars de l'utilisateur, darktable 5, crop v3) ;
- **`flip`** : `-1` (`ffffffff`) suit l'orientation EXIF ; une autre valeur est une
  orientation de darktable, largeur et hauteur échangées si le bit 4 est mis ;
- **`ashift`**, **`clipping`**, **`rotatepixels`**, **`scalepixels`** actifs : taille
  inconnue (`None`), plutôt qu'une taille fausse ; de même pour des paramètres
  compressés (`gz…`).

## Dans l'interface

En attendant le rendu, le rendu RAW (ou le boîtier) avec « développement darktable en
cours… » ; sans développement pour l'image, la raison à la place. Pas de 1:1 développé :
on zoome dans le rendu. Le collimateur et les zones saturées / bouchées, placés sur le
capteur entier, sont masqués sur un développé (recadré). Sans darktable, la source n'est
pas proposée et `Maj+B` le dit.

**Tout un dossier en fond** (25/10/2026, issue n° 69) : `Ctrl+Maj+B` (le dossier ouvert)
ou le clic droit sur un dossier du panneau, « Développer tout le dossier (darktable) ».
Chaque image qui a un sidecar darktable à historique est rendue l'une après l'autre
(`gui/src/develop_batch.rs`), par le même producteur que `Maj+B` (`CachedPreviews`,
niveau `Developed`) : un autre logiciel de rendu branché là servira de même. Déjà en
cache : passée. Un lot à la fois ; sa progression est une tâche de fond (« × » :
interrompre), le bilan (rendus, déjà faits, échecs) va dans la barre d'état ; ensuite,
`b` propose le développé de chaque image rendue.

**Choisir le logiciel** (26/10/2026, choix de l'utilisateur ; raccourcis réglables) :
`Maj+B` et `<leader>r` développent l'image dans son logiciel (celui de son
développement, voir `[develop] default`), `<leader>d` dans darktable, `<leader>p` dans
RapidRAW (`Action::DevelopWith`, identifiants `plugin.darktable.develop`,
`plugin.rapidraw.develop` ; `core.develop.render` et `core.develop.folder` pour les
deux autres). Au clic droit sur une image, « Développer avec » : les logiciels
installés, ou le rendu RAW (LibRaw). Le logiciel choisi passe devant celui du
développement pour cette image, le temps de la session (`develop_choice`). Au clic
droit sur un dossier, « Développer tout le dossier… » ouvre une boîte : chaque image
dans le logiciel de son développement (sinon elle garde le rendu RAW), ou tout dans un
logiciel (les images qu'il a développées). `Ctrl+Maj+B` : chaque image dans le sien,
sans boîte.

À venir : `darktable-cli` sait traiter un dossier en un seul lancement (démarrage
amorti) — pour l'envoi de JPEG développés de l'« après-développement ».
