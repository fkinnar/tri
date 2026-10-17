# Interface graphique : `tri-gui`

Application egui/eframe, dans son propre paquet (`gui/`) du workspace Cargo. Le moteur
(`tri`) ne dépend pas d'egui : la séparation moteur / affichage du `CLAUDE.md` est ainsi
garantie par Cargo lui-même.

```sh
cargo run --release -p tri-gui -- <dossier> [--cache <dossier du cache>] [--config <fichier>]
```

Tous les réglages (thème, raccourcis, dimensions de la grille, délais, capacités, couleurs,
et ceux du moteur) viennent du fichier de configuration : voir [settings.md](settings.md)
et [config.example.toml](config.example.toml). Une configuration invalide est signalée en
rouge dans la barre d'état (détail au survol), et les réglages par défaut sont utilisés.

Toujours en **`--release`** pour s'en servir : en build de débogage, le code Rust n'est pas
optimisé, et le 1:1 du JPEG boîtier passe de 0,4 s à 2 s (rotation, conversion pour le GPU),
le 100 % RAW de 4,5 à 7 s. `--cache` utilise un autre dossier de cache que celui du système
(mesures « à froid », essais).

## Ce qui est en place

- **Grille de vignettes** du dossier, alimentée en tâche de fond par le
  [scheduler](scheduler.md) ; seules les lignes visibles sont dessinées
  (`ScrollArea::show_rows`), ce qui tient des milliers d'images.
- **Barre d'état** : dossier, nombre d'images (dont RAW), filtre, progression des vignettes,
  des aperçus « JPEG boîtier » (masquée s'il n'y a aucun RAW) et des aperçus standard, nom
  de l'image sélectionnée.
- **Sélection** au clavier ou à la souris ; le scheduler est prévenu (`set_focus`), donc
  l'aperçu de l'image sélectionnée et de ses voisines passe en tête de file.
- **Un seul curseur, clavier ou souris** (17/10/2026) : bouger la souris sur une vignette la
  sélectionne, et le clavier repart de là. Seul un vrai mouvement compte (déplacement du
  pointeur non nul) : la grille qui défile à la molette sous un pointeur immobile ne change
  pas la sélection. Le pointeur est **masqué dès qu'on tape** et réapparaît au premier
  mouvement de la souris, dans la grille comme en plein écran. Pas de surbrillance de survol
  distincte : avant, survol (gris) et sélection (turquoise) donnaient l'impression de deux
  images sélectionnées. Le survol ne fait pas défiler la grille (la sélection bougerait avec
  elle) ; le clic et le clavier, si.
- **Notation** (17/10/2026) : étoiles, couleurs, accepté / rejeté sur l'image sélectionnée
  (grille ou plein écran), écrites aussitôt dans son sidecar XMP (voir [xmp.md](xmp.md),
  [notes.md](notes.md)). Notes chargées à l'ouverture du dossier, avec celles déjà posées
  par darktable, Lightroom ou digiKam. Affichage dessiné en formes (pas de glyphes, voir
  `marks.rs`) : drapeau (coche verte / croix rouge), étoiles dorées, pastilles de couleur,
  en bas à gauche de la vignette et du plein écran ; vignette d'une photo rejetée estompée
  (`[gui.grid] rejected_opacity`). Une écriture impossible (sidecar mal formé, disque en
  lecture seule) est signalée dans la barre d'état, et la note affichée reste celle du
  disque. Une seule couleur par photo par défaut (`[xmp] multiple_colors`).
- **Filtres** (17/10/2026) : type (tout / RAW / plates) et notes (au moins n étoiles ;
  acceptées, sans les rejetées ou rejetées ; une ou plusieurs couleurs), combinés.
  - Clavier : préfixe `f` (choix de l'utilisateur), suivi de la touche de notation
    correspondante : `f f` type suivant, `f 1`…`f 5` au moins n étoiles, `f 0` étoiles
    indifférentes, `f p` acceptées, `f u` sans les rejetées, `f x` rejetées (la même
    séquence retire le filtre), `f 6`…`f 9`, `f °`, `f ,` couleurs (ajouter / retirer),
    `f Échap` tout effacer. Chiffres et lettres avec ou sans Maj.
  - Souris : **barre de filtres** en haut de la grille (`Maj+F` l'affiche ou la masque,
    `[gui] filter_bar`) : chaque clic déclenche la même action que le clavier (recliquer
    l'étoile active efface le filtre d'étoiles). Des filtres plus complexes (fenêtre ou
    panneau latéral) restent à discuter.
  - Barre d'état : filtres actifs en clair (« RAW · 3 étoiles et plus · sans les
    rejetées »).
  - La sélection est conservée si l'image reste visible ; sinon elle garde sa place dans
    la liste. Noter une image sous un filtre de notes refiltre aussitôt : rejeter une
    photo avec « sans les rejetées » la fait disparaître et sélectionne la suivante (tri
    rapide).
- **Notation à la souris** (17/10/2026, demande de l'utilisateur : tout le monde ne veut
  pas tout faire au clavier) :
  - **bandeau de notation** en bas (au-dessus de la barre d'état), dans la grille et en
    plein écran : drapeaux, étoiles et couleurs de l'image sélectionnée, cliquables,
    estompés quand ils ne sont pas posés ; `Maj+N` l'affiche ou le masque
    (`[gui] rating_bar`) ;
  - **clic droit** sur une vignette ou en plein écran : menu avec les mêmes contrôles. Il
    sélectionne la vignette et note *cette* image, même si le pointeur survole d'autres
    vignettes en allant vers le menu ; le menu reste ouvert pour poser plusieurs notes ;
  - chaque clic déclenche la même action que le clavier : recliquer la note active la
    retire (étoiles, drapeau), une couleur s'ajoute ou se retire ;
  - contrôles nommés pour l'accessibilité (`WidgetInfo`), ce qui permet aussi aux tests
    d'interface de cliquer « 3 étoiles ».
- **Paires RAW+JPEG** : noter `IMG.RAF` note aussi `IMG.JPG` (choix de l'utilisateur,
  17/10/2026), voir [notes.md](notes.md).
- **Aide** (`F1` ou `?`, fermeture par Échap) : chaque action avec un libellé compréhensible
  et ses touches ; l'identifiant technique n'apparaît qu'au survol. L'aide est **modale** :
  tant qu'elle est ouverte, seules Échap, l'aide et Quitter agissent.
- Taille des aperçus standard **« Auto »** : plus grand côté de l'écran, en pixels physiques.
- Thème sombre par défaut.

- **Plein écran** (Entrée ou double-clic) : voir plus bas.

## Plein écran

Pensé pour le tri : en passe rapide, le **JPEG du boîtier** sert de base (disponible en
quelques secondes pour tout le dossier) pendant que les rendus RAW se calculent derrière ;
puis on vérifie la netteté des candidates **à 100 %**.

- **Deux réglages indépendants** (choix du 16/10/2026) : `b` choisit la **source** (JPEG
  boîtier par défaut, ou rendu RAW), `z` choisit l'**échelle** (ajusté ou 1:1). Zoomer ne
  change jamais de source, donc jamais le « look » de l'image ; `b` en restant zoomé passe
  d'un 1:1 à l'autre au même endroit. La source choisie reste active d'une image à l'autre.
  Si la source voulue n'est pas encore calculée, la meilleure disponible est affichée
  (l'autre source, puis la vignette agrandie). Les images plates n'ont qu'une source.
- **Rendus RAW à la demande** : en mode JPEG, aucun rendu RAW n'est calculé, la machine
  reste libre. En mode RAW, l'interface demande au scheduler le rendu de l'image affichée
  et de ses voisines (`Scheduler::request`, politique `RawPreviews::OnDemand`) ; un premier
  `b` coûte ~0,5 s. Le calcul de tout le dossier en fond reste possible
  (`RawPreviews::Background`, défaut du moteur et de la démo en ligne de commande).
- **Ce qui est affiché est toujours écrit** en haut à gauche : position, nom, « JPEG
  boîtier », « Rendu RAW », « Rendu RAW · 100 % », « Vignette (aperçu en cours de calcul) »,
  et « calcul du 100 % en cours » le cas échéant. Le JPEG boîtier reflète les réglages du
  boîtier (contraste, simulation de film) ; le rendu RAW de LibRaw est plus neutre, donc
  plus terne : c'est attendu.
- **1:1** : `z` (centré), ou double-clic (le point cliqué reste sous le curseur). Glisser
  avec la souris déplace l'image, sans jamais laisser de bord vide.
  - en JPEG : le JPEG embarqué à sa taille d'origine, ~70 ms. Étiquette honnête : « JPEG
    boîtier · 1:1 (71 % du capteur) » (4416 px sur 6240 pour un X-T4). Netteté et débruitage
    du boîtier : répond à « est-ce assez net pour ce que j'en ferai ? » ;
  - en RAW : le rendu complet, ~4–5 s, « Rendu RAW · 100 % ». Répond à « la mise au point
    est-elle vraiment sur l'œil ? ».
  En attendant, l'image reste ajustée avec la mention « chargement du 1:1 » ou « calcul du
  100 % en cours ». Le cadrage du JPEG embarqué peut différer de quelques pixels de celui du
  RAW ; la position relative est gardée, donc la même zone à quelques pixels près.
- **Navigation** : ←/→ (h/l), Début/Fin (g/Maj+G). **En 1:1, le zoom et la zone affichée
  sont conservés d'une image à l'autre** : on compare la netteté de plusieurs vues au même
  endroit. Échap revient à la grille (sur l'image affichée).
  - **Préchargement** : zoomé, le 1:1 des deux voisines est demandé et chargé à l'avance ;
    en avançant d'une image, son 1:1 est en général déjà là (mesuré : affichage immédiat).
  - **Jamais de passage par la vue ajustée ni d'écran noir** : si le 1:1 de la nouvelle
    image n'est pas encore prêt, le meilleur aperçu disponible est affiché **déjà à
    l'échelle 1:1** (taille estimée d'après le dernier 1:1 affiché, même boîtier), flou un
    instant puis net, comme dans Lightroom. En dernier recours, la vignette (chargée en
    1–2 ms) tient lieu d'attente.
  - Mode RAW : la source reste RAW d'une image à l'autre. Si aucun rendu RAW n'est encore
    prêt pour la nouvelle image, le JPEG boîtier est affiché en attendant, avec la mention
    « rendu RAW en cours » (sans elle, on croyait le mode RAW désactivé : retour du
    16/10/2026). Les demandes partent **dans l'ordre des priorités** (image affichée
    ajustée, son 1:1, puis les voisines) : envoyées dans un autre ordre, un 100 % (~4,5 s,
    non interruptible) passait devant le rendu ajusté (~0,5 s) et le JPEG restait affiché
    plusieurs secondes.
  - Mode RAW : un 100 % RAW (~4,5 s, que LibRaw ne sait pas interrompre) n'est demandé
    qu'après **0,4 s passées sur l'image** (`gui.view.full_raw_dwell_ms`). Sans ce délai, en enchaînant
    vite les flèches, chaque image traversée pouvait lancer son 100 %, et le rendu RAW de
    l'image où l'on s'arrêtait attendait jusqu'à ~5 s derrière un calcul devenu inutile (le
    JPEG restait affiché : retour du 16/10/2026). Mesuré après correction : aucune demande
    de 100 % pour les images traversées, rendu RAW de l'image d'arrivée en 0,8 s. Le rendu
    ajusté (~0,5 s) et le 1:1 du JPEG (rapide) restent demandés immédiatement.
  - Mode RAW : le préchargement passe par l'unique worker LibRaw (~4,5 s par 1:1). En
    sautant une image, le 1:1 voulu attend la fin de celui de la voisine déjà commencé (le
    dématriçage X-Trans ne s'interrompt pas) : jusqu'à ~6 s mesurées.
  - Dans un dossier RAW+JPEG, les voisines d'un .RAF sont ses .JPG : le préchargement RAW
    ne sert vraiment qu'avec le filtre « RAW seuls » (`f f`).
  - Dans un dossier RAW+JPEG, le 1:1 d'un .JPG (6240 px) et celui du JPEG embarqué d'un
    .RAF (4416 px) n'ont pas le même grossissement : c'est le principe du 1:1 (un pixel de
    l'image pour un pixel de l'écran).
- **Fluidité** : les grandes images sont décodées par un thread de chargement
  (`loader.rs`) : l'image affichée d'abord, puis ses voisines n+1 et n-1 (préchargement). La
  liste remplace la précédente : une image dépassée n'est jamais chargée pour rien. Les
  grandes textures vivent dans un LRU de 8 entrées (un 1:1 de 26 Mpx occupe ~100 Mo de
  mémoire graphique). Une image plus grande que la limite de texture du GPU est réduite
  pour l'affichage (sa taille d'origine sert toujours à la géométrie).

## Modules

| Fichier | Rôle |
|---|---|
| `main.rs` | Arguments, fenêtre, thème |
| `app.rs` | `TriApp` : état, événements du moteur, dessin |
| `actions.rs` | Actions nommées et raccourcis |
| `grid.rs` | Calculs purs de la grille : colonnes, navigation, défilement |
| `textures.rs` | Cache LRU des textures |
| `view.rs` | Logique pure du plein écran : choix de la source, ajustement, 100 %, déplacement |
| `loader.rs` | Thread de chargement des grandes images |
| `marks.rs` | Dessin des notes : étoiles, pastilles de couleur, drapeau |

### Actions et raccourcis (`actions.rs`)

Une touche n'appelle jamais une fonction directement : elle est traduite en `Action`, avec
un identifiant stable (`core.nav.next`, `core.filter.cycle`…), comme décidé dans le
`CLAUDE.md`. La section `[keys]` du fichier de configuration associe des touches à ces
identifiants (voir plus bas) ; la souris déclenche les mêmes actions.

Deux profils sont livrés et actifs ensemble (ils ne se recouvrent pas) :

| Action | Classique | Vim |
|---|---|---|
| `core.nav.prev` / `core.nav.next` | ← / → | h / l |
| `core.nav.up` / `core.nav.down` | ↑ / ↓ | k / j |
| `core.nav.first` / `core.nav.last` | Début / Fin | gg / Maj+G |
| `core.nav.page_up` / `core.nav.page_down` | Page préc. / suiv. | Ctrl+U / Ctrl+D |
| `core.view.open` | Entrée | Entrée |
| `core.view.zoom` | z | z |
| `core.view.source` | b | b |
| `core.view.pan_left` / `pan_right` | Maj+Flèche gauche / droite | Maj+h / Maj+l |
| `core.view.pan_up` / `pan_down` | Maj+Flèche haut / bas | Maj+k / Maj+j |
| `core.view.back` | Échap | Échap |
| `core.app.help` | F1 | ? |
| `core.app.quit` | q | q |

Notation, commune aux deux profils (17/10/2026, pensée aussi pour l'AZERTY) :

| Action | Touches |
|---|---|
| `core.rate.star.0` … `core.rate.star.5` | 0 … 5, avec ou sans Maj |
| `core.rate.color.red` / `yellow` / `green` / `blue` | 6 / 7 / 8 / 9, avec ou sans Maj |
| `core.rate.color.purple` | ° / , |
| `core.flag.accept` / `reject` / `none` | Maj+P / Maj+X / Maj+U |

Pourquoi : sur l'AZERTY, les chiffres de la rangée du haut demandent Maj, ceux du pavé
numérique non ; egui-winit traduit la touche *logique* et, s'il ne la connaît pas (« & »,
« é »), la touche *physique* : les deux donnent `Num1`… Exception : le 6 non majuscule de
l'AZERTY est « - », reconnu comme `Minus`, d'où Maj sur la rangée du haut. Le violet est sur
« ° » (Maj + la touche à droite du 0 en AZERTY), qu'egui ne nomme pas : c'est un
**caractère**, reçu par l'événement `Text` ; seuls les caractères liés à un raccourci sont
pris en compte (les autres interrompraient une séquence). La virgule (`Comma`) couvre le
pavé numérique français (egui ne distingue pas le pavé) et celle de la rangée du bas. `u`
seul reste libre pour l'annulation (à venir), comme dans vim.

Une couleur déjà posée est retirée par la même touche ; en mode une couleur (défaut), une
autre couleur remplace la précédente.

Cmd et Alt ne déclenchent aucune action : ces combinaisons restent au système (Cmd+Q…).
Ctrl est permis (Ctrl+U / Ctrl+D du profil vim) ; Sous Linux et Windows, egui signale aussi
Ctrl comme `command` : seul `mac_cmd`, ou `command` sans Ctrl, désigne Cmd. Pas de raccourci
Ctrl + flèches (sur macOS, cela change de bureau), d'où Maj pour le déplacement.

Filtres, communs aux deux profils : voir « Filtres » plus haut (`core.filter.*` : `f f`,
`f 0`…`f 5`, `f p` / `f u` / `f x`, `f 6`…`f 9`, `f °`, `f ,`, `f Échap`) ;
`core.view.filter_bar` : Maj+F ; `core.view.rating_bar` : Maj+N.

Caractères : egui-winit livre la touche (Maj + `Minus`, physique) **et** le caractère
(« ° ») dans la même image ; `Keymap::chords` ignore une touche suivie d'un caractère lié à
un raccourci, sinon elle interromprait `f °`.

**Section `[keys]`** du fichier de configuration, rangée par action :

```toml
[keys]
"core.flag.accept" = ["Maj+P", "a"]
"core.nav.first" = ["Début", "g g"]
```

- les raccourcis cités **remplacent** ceux des profils pour cette action (liste vide : plus
  aucun) ; les profils (`[gui] keys`) restent la base ;
- une touche s'écrit comme l'aide l'affiche (« Flèche gauche », « Page préc. », « Échap »,
  « Maj+P », « Ctrl+u ») ou avec le nom d'egui (« ArrowLeft », « PageUp ») ; une majuscule
  seule vaut Maj + la lettre (« G », comme dans vim) ; une séquence se sépare par des
  espaces (« g g ») ; un caractère sans nom de touche (« ° ») est reconnu comme caractère ;
- problèmes signalés au démarrage (stderr et barre d'état, détail au survol), sans
  invalider le reste du fichier : action ou touche inconnue, raccourci attribué à deux
  actions (le premier par ordre d'identifiant est gardé ; un raccourci du fichier l'emporte
  sur celui d'un profil, et le dit), raccourci qui commence une séquence plus longue (il ne
  se déclencherait jamais) ;
- un test vérifie que chaque raccourci par défaut, écrit comme l'aide l'affiche, se relit à
  l'identique.

**Séquences de touches** : un raccourci peut être une suite de touches, comme `gg` (vim). La
première est mise en attente (`Sequencer`) ; la suivante doit arriver avant
`[gui] key_sequence_timeout_ms` (1000 ms par défaut, comme le `timeoutlen` de vim). Si elle
ne complète aucune séquence, elle agit seule (`g` puis `j` : ligne du dessous). Aucun
raccourci n'est à la fois une touche seule et le début d'une séquence (vérifié par test) :
pas besoin d'attendre l'expiration pour décider.

**Déplacement dans une image zoomée** : Maj + flèches ou Maj + hjkl, d'une fraction de la
vue par appui (`[gui.view] pan_step`, 0,2), touche maintenue possible ; sans effet hors
zoom.

Chaque action a deux noms : un **identifiant** stable pour les fichiers et le code
(`core.view.back`), et un **libellé** pour l'utilisateur (« Fermer / revenir »).
`core.view.back` ferme ce qui est au premier plan : l'aide, sinon le plein écran.

Les touches sont affichées par des **noms français en toutes lettres** (`key_label` :
« Flèche gauche », « Début », « Page suiv. », « Échap »), en police à chasse fixe pour que
« l » et « I » restent distincts. Pas de symboles : « ⏴ » (nom fourni par egui) et même
« ← » manquent dans la police par défaut d'egui. Les tests d'interface échouent sur tout
caractère sans glyphe, ce qui garantit que l'aide s'affiche partout.

L'aide défile si elle dépasse 80 % de la hauteur de la fenêtre (33 actions au 17/10/2026).

### Grille (`grid.rs`)

Fonctions pures, testées sans fenêtre :

- `Layout::fit(largeur, cellule, espace)` : nombre de colonnes ; `rows`, `row_pitch`,
  `rows_in`, `width` ;
- `navigate(sélection, n, layout, lignes_par_page, action)` : haut/bas gardent la colonne ;
  vers le bas depuis l'avant-dernière ligne, on va à la dernière image si la colonne n'existe
  pas sur la dernière ligne ;
- `scroll_to_show(layout, ligne, décalage, hauteur)` : défilement minimal pour rendre une
  ligne visible.

Le défilement est mémorisé comme une *intention* (`Reveal::Minimal` après un déplacement,
`Reveal::Top` après un changement de liste) et résolu au moment de dessiner la grille, quand
la largeur, donc le nombre de colonnes, est connue. Le calculer plus tôt donnait une mauvaise
ligne au démarrage (bug vu sur une capture).

### Réglages (`settings.rs`)

Le fichier de configuration entier vu par l'interface (sections du moteur + `[gui]`) : voir
[settings.md](settings.md#interface--guisrcsettingsrs).

### Textures (`textures.rs`)

Les vignettes visibles sont chargées depuis le cache disque (~1–2 ms chacune) dans un
budget de **8 ms par image affichée** : au-delà, le chargement reprend à l'image suivante,
sans figer l'interface. Les textures vivent dans un **cache LRU de 600 entrées**, indexé par
chemin (il survit donc à un changement de filtre). Sans borne, 5 000 vignettes de 400 px
occuperaient ~2 Go de mémoire graphique.

## Développement : capture automatique

```sh
TRI_GUI_SCREENSHOT=/tmp/aide.png TRI_GUI_SELECT=40 TRI_GUI_ACTIONS=core.app.help \
  cargo run --release -p tri-gui -- <dossier>
```

L'application se capture elle-même (contenu de sa fenêtre seulement, pas l'écran), puis se
ferme. La capture a lieu une fois toutes les vignettes calculées et celles qui sont
visibles affichées (en plein écran : l'image voulue affichée, y compris le 100 % s'il est
demandé), **et au moins 1 s après le démarrage**, au plus tard après 60 s. Sans ce
délai, une capture faite pendant le fondu d'ouverture d'une fenêtre la montrait à
moitié transparente.

- `TRI_GUI_SELECT=<n>` : image sélectionnée au démarrage ;
- `TRI_GUI_ACTIONS=<étape>,<étape>...` : script exécuté au démarrage. Étapes : un
  identifiant d'action (`core.nav.next`, `core.app.help`…), `wait:<ms>` (pause, pour
  laisser le moteur travailler), `shot` (capture immédiate, sans attendre que tout soit
  prêt : pour voir un état intermédiaire). Une étape inconnue est signalée et ignorée.
  Exemple : `core.view.open,core.view.zoom,wait:1500,core.nav.next,wait:30,shot`.
- `TRI_GUI_TRACE=1` : trace minutée de chaque étape du chargement des images (demande au
  moteur, image prête, abandonnée, lecture et décodage, image affichée) sur la sortie
  d'erreur. C'est elle qui a permis de mesurer les délais ci-dessus.

## Tests

**Tests d'interface** avec [egui_kittest](https://crates.io/crates/egui_kittest) (banc de
test officiel d'egui) : l'application tourne sans fenêtre, sur un dossier et un cache
temporaires, et reçoit des touches simulées. Vérifié : l'aide s'ouvre et se ferme avec F1 et
`?` (avec ou sans Maj), Échap la ferme, elle est modale, et elle affiche les libellés et
non les identifiants. Plein écran : ouverture par Entrée et retour par Échap, navigation
image par image (haut/bas sans effet), bascule de source, aperçu standard réellement chargé
et affiché, 1:1 réellement calculé puis affiché, conservé à l'image suivante, zoom conservé
en basculant de source. Ces tests
ont trouvé un glyphe manquant (« ⏴ ») dans l'aide, et l'absence de fermeture par Échap.

Logique du plein écran (`view.rs`, unitaires) : choix de la source et repli sur ce qui est
prêt, libellés, ajustement, placement à 100 %, limites du déplacement, point cliqué gardé
sous le curseur.

Les tests de l'interface doivent être dans le paquet (`src/`, `#[cfg(test)]`) et non dans
`tests/` : `tri-gui` est un binaire, ses modules ne sont pas importables depuis `tests/`.

Notation à la souris (17/10/2026) : clics dans le bandeau (3 étoiles, rejeter, vert, puis
retrait des étoiles et du drapeau en recliquant), relus du disque ; Maj+N masque le
bandeau ; clic droit sur la troisième vignette : elle est sélectionnée, le menu s'ouvre, et
« 4 étoiles » note cette image et pas la première.

Filtres (17/10/2026) : `f Maj+2`, `f 0`, `f u` (deux fois : retiré), `f 9`, `f °`,
`f Échap` ; rejeter sous « sans les rejetées » sélectionne l'image suivante ; barre
masquée / affichée par Maj+F, clics sur RAW, Plates, Effacer.

Notation (17/10/2026) : Maj+3, Maj+X, 8 puis « ° » (événement `Text`) écrivent le sidecar
de l'image sélectionnée (relu du disque : 3 étoiles, rejetée, violet seul), l'image
suivante est notée à part ; un sidecar mal formé est signalé et laissé intact.

Tests unitaires du paquet (`cargo test -p tri-gui`) : identifiants d'actions uniques et
préfixés, touches de notation avec ou sans Maj, Ctrl+U / Ctrl+D (aussi sous la forme
Linux, Ctrl + `command`), `u` libre, caractère « ° », lecture des raccourcis du fichier
(« Maj+P », « G », « g g », « Flèche gauche », « + », « ° »...) et relecture de toutes les
touches par défaut, remplacement par `[keys]`, conflits et séquences masquées signalés,
largeur d'une ligne de notes, profils complets et sans doublon, union des profils, modificateurs système
ignorés, noms de touches et libellés de l'aide, identifiants réversibles ; colonnes, navigation (bords, colonnes, dernière ligne
incomplète, pages, grille vide), défilement ; LRU (éviction, remplacement, capacité
minimale) ; troncature des noms, mise à jour des statuts ; réglages (`settings.rs`) :
fichier d'exemple égal aux valeurs par défaut, sections inconnues et valeurs invalides
signalées (thème, profil, couleur), section du moteur transmise, liste de profils vide. Le
rendu est vérifié par capture automatique (aussi avec un fichier de configuration :
`tri-gui <dossier> --config <fichier>`).
