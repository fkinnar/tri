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
au côté de l'aperçu standard (`shrink`). **Interface de RapidRAW ouverte** : l'export
sans interface tourne quand même (il ne passe pas par l'instance unique), mais a rendu
une **image toute noire** sans erreur (essai du 26/10/2026). `shrink` refuse une image
dont aucun pixel ne dépasse 0 (le bruit du capteur suffit à dépasser 0 sur une vraie
photo) : rien n'entre dans le cache, et le message le dit en clair (« fermez-la puis
recommencez »). Si une version future renvoie le refus « already running », il est
traduit de même.

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

## Ouvrir un dossier dans le logiciel (28/10/2026)

`Developer::open_folder` donne la commande qui ouvre un dossier dans l'interface du
logiciel ; `Developers::folder_openers` liste les logiciels installés qui le savent,
`Developers::open_folder` donne la commande (lancée par `files::launch`, sans
l'attendre).

- **darktable** : `darktable <dossier>` ouvre la table lumineuse avec ce dossier comme
  collection, importé au besoin. Une instance déjà ouverte le reçoit : par D-Bus sous
  Linux ; sous macOS, par `open -a darktable.app <dossier>` (le système le lui passe,
  `_osx_openfile_callback` de darktable). Lancer le programme du `.app` démarrait une
  seconde instance, qui plantait (retour du 28/10/2026). Sous Windows, rien de tel :
  darktable déjà ouvert (`process::running`, `tasklist`), l'entrée est grisée
  (`Developer::open_blocked`).
  Cette fois avec la configuration et la bibliothèque de l'utilisateur, pas celles de
  `tri` : c'est lui qui travaille. L'interface est trouvée d'après `darktable-cli`
  (`develop::darktable_gui`) : le programme `darktable` à côté (paquet, `.app`,
  installeur Windows), l'AppImage sans la commande `darktable-cli`, ou le Flatpak sans
  `--command`.
- **RapidRAW** : pas d'ouverture de dossier ; en argument, il n'ouvre qu'une image dans
  son éditeur (`launch_request.rs` de RapidRAW : un chemin = `OpenFile`, vérifié le
  28/10/2026).

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

## Comparer les développements en plein écran (26/10/2026, #114)

`b` fait le tour de tout ce qui existe pour l'image affichée : JPEG boîtier, rendu RAW,
puis **chacun de ses développements déjà rendus** (`view::Source::next`,
`TriApp::rendered_engines`), celui de son développement le plus récent d'abord (ou du
logiciel préféré), et retour au boîtier. Un développement pas encore rendu n'entre pas
dans le tour (`Maj+B`, `<leader>d` / `<leader>p` le rendent). L'étiquette dit lequel est
affiché (« Développé dans RapidRAW »).

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

## Les développements dans la grille (26/10/2026, #114)

Après le développement, on veut voir les images développées ; au déchargement et au tri,
le JPEG boîtier suffit. `[gui.grid] render` (`gui/src/grid_render.rs`, `GridRender`),
aussi dans la barre des filtres (liste à droite, `f d` pour le suivant), retenu d'une
session à l'autre : `embedded` (JPEG boîtier, défaut), `latest` (le développement le plus
récent de chaque image, `develop::resolve` sans préférence), `darktable` / `rapidraw`
(toujours ce logiciel), `libraw` (rendu RAW). La liste ne propose que les logiciels
installés. Une vignette n'a que **deux états** : JPEG boîtier, ou développée (pastille sous
la vignette au nom du logiciel, ou « LibRaw » ; « DEV » au début, retour du 26/10/2026 : ne
disait rien). Un RAW sans le développement voulu
montre son **rendu LibRaw** : dès que la grille montre les rendus, les rendus LibRaw passent
en fond pour tout le dossier (`Scheduler::set_raw_previews`, une tâche de fond de plus). Un
développement pas encore rendu : en fond, un lot comme `Ctrl+Maj+B` (`[gui.grid] develop =
"auto"`, bilan tu s'il n'y avait rien à rendre), ou à la demande seulement (`"manual"`) ;
LibRaw, puis la vignette, en attendant. Les rendus du lot, faits hors du moteur, sont
cherchés dans le cache au plus une fois par `render_poll_ms`. Le rendu, bien plus grand
qu'une vignette, est décodé réduit (`jpeg::decode` à la taille des vignettes). Les images
plates gardent leur vignette.
Entrée ou un double-clic ouvrent le plein écran sur ce que montre la vignette : son
développement (dans le même logiciel), ou le rendu RAW ; avec le JPEG boîtier dans la
grille, la source choisie au plein écran (`b`) reste.

## Filtrer les images développées (27/10/2026)

Idée de l'utilisateur : une **bobine de film** dans la barre des filtres (`f Maj+D`),
allumée elle filtre ; à côté apparaît alors une liste des logiciels installés, à
**cocher** : les images développées dans **l'un des logiciels cochés** ; **aucun coché**,
les images **sans développement** (montrées dans leur rendu LibRaw). Repliée, la liste
nomme le logiciel s'il est seul, les sigles s'ils sont plusieurs (« DT + RR »,
`Engine::short`), « LibRaw » si aucun n'est coché. Les cases cochées sont retenues d'une
session à l'autre (`gui-state.toml`, `developed_engines`) ; au premier lancement, tous
les logiciels installés. Moteur : `develop::DevelopedRule` (logiciels cochés) et
`DevelopedFilter` (trait `Filter`), qui lisent les sidecars (`Engine::sidecar` :
historique darktable non vide, `.rrdata` de RapidRAW) ; une image plate n'a pas de
développement. Filtre sur le contenu : effacé par `f Échap`, inversible (`f i`).

Sous le filtre, les vignettes montrent le développement des logiciels cochés, quel que
soit le rendu choisi pour la grille (retours du 27/10/2026) : le plus récent s'ils sont
plusieurs (`DevelopedRule::engine_for`), le rendu LibRaw si aucun n'est coché ; le plein
écran s'ouvre dessus. Inversé, le filtre laisse à la grille son rendu
(`TriApp::shown_render`).

## Autres logiciels de développement (recherche du 28/10/2026)

Ce que chaque logiciel permet depuis la ligne de commande : un rendu sans interface (ce
dont a besoin un plugin de rendu), l'ouverture d'un dossier (« Ouvrir dans… »), et
l'endroit où il range les retouches (pour savoir qu'une image a été développée).
D'après les docs et pages de manuel ; ce qui ne vient que de forums est signalé.

| Logiciel | Rendu sans interface | Ouvrir un dossier | Retouches | Linux |
|---|---|---|---|---|
| darktable | `darktable-cli` (intégré) | `darktable <dossier>` (intégré) | `photo.raw.xmp` | oui |
| RapidRAW | `rapidraw export` (intégré) | non : une image seulement | `photo.raw.rrdata` | oui |
| RawTherapee | `rawtherapee-cli` | `rawtherapee <dossier>` | `photo.raw.pp3` | oui |
| ART | `ART-cli` | `ART <dossier>` | `photo.raw.arp` | oui |
| vkdt | `vkdt-cli` (GPU) | non vérifié | `photo.raw.cfg` (forum) | oui |
| digiKam / showFoto | non | `showfoto <dossier>` | XMP | oui |
| Lightroom Classic | non | fichiers en argument → boîte d'import | catalogue, XMP `crs:` en option | non |
| Lightroom (cloud) | non | non | dans le nuage | non |
| Capture One | non (AppleScript sous macOS) | non (forum) | `CaptureOne/Settings…/*.cos` (sessions) | non |
| DxO PhotoLab / PureRAW | non | non | `photo.raw.dop` (forum) | non |
| ON1 Photo RAW | non | non | base, `.on1` en option | non |
| Photomator | non | non | sidecars dans Images / iCloud | non |
| Luminar Neo | non | non | catalogue | non |
| Exposure X7 | non | non | `.exposurex7` dans un sous-dossier | non |
| NX Studio | non | non | `NKSC_PARAM/*.nksc` | non |

- **RawTherapee et ART** sont les seuls candidats à un plugin comme darktable : rendu
  `rawtherapee-cli -o sortie.jpg -s -j92 -Y -c photo.raf` (`-s` : le sidecar ; `-c` en
  dernier), même chose pour `ART-cli` avec le `.arp`. Pas d'option de taille : il
  faudrait la donner par un profil (`-p`). Ouverture d'un dossier documentée ; le
  comportement avec une instance déjà ouverte n'est pas documenté (à essayer, vu
  darktable sous macOS). Décision de l'utilisateur : plus tard, pas urgent.
- **vkdt** : `vkdt-cli -g photo.raf.cfg --filename sortie --format o-jpg --width 2560
  --height 2560`, sur la carte graphique (Vulkan) ; Windows non mentionné.
- **Logiciels commerciaux** : aucun ne se pilote en ligne de commande pour un rendu.
  Au mieux, reconnaître leurs sidecars pour marquer « développé dans… », sans afficher
  leur rendu. Lightroom Classic accepte des fichiers en argument (boîte d'import, cassé
  quand il est déjà ouvert selon FastRawViewer) ; Capture One a un dictionnaire
  AppleScript, réputé incomplet.
- « Ouvrir dans… » reste donc à darktable (et un jour RawTherapee / ART).

Sources : [RawTherapee](https://rawpedia.rawtherapee.com/Command-Line_Options),
[ART](https://manpages.opensuse.org/Tumbleweed/ART/ART.1.en.html),
[vkdt](https://github.com/hanatos/vkdt),
[showFoto](https://dyn.manpages.debian.org/trixie/showfoto/showfoto.1.en.html),
[darktable](https://www.mankier.com/1/darktable),
[Lightroom Classic, XMP et ACR](https://helpx.adobe.com/au/lightroom-classic/desktop/organize-photos-in-lightroom-classic/create-xmp-acr-files.html),
[Lightroom Classic lancé par FastRawViewer](https://fastrawviewer.com/node/855),
[Capture One, AppleScript](https://support.captureone.com/hc/en-us/articles/360002681418),
[DxO (forum)](https://forum.dxo.com/t/automation-using-cli/36444),
[Exposure](https://support.exposure.software/hc/en-us/articles/360005225913-How-large-are-Exposure-s-sidecar-files),
[NX Studio](https://nikonimglib.com/nxstdo/onlinehelp/en/save_80.html),
[Photomator](https://support.pixelmator.com/faq-photomator/advanced-workflows/save-photomator-linked-files-to-an-alternate-location-on-mac).
