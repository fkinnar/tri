# Interface graphique : `tri-gui`

Application egui/eframe, dans son propre paquet (`gui/`) du workspace Cargo. Le moteur
(`tri`) ne dépend pas d'egui : la séparation moteur / affichage du `CLAUDE.md` est ainsi
garantie par Cargo lui-même.

```sh
cargo run --release -p tri-gui -- <dossier> [--cache <dossier du cache>] [--config <fichier>]
```

Tous les réglages (thème, raccourcis, dimensions de la grille, délais, capacités, couleurs,
et ceux du moteur) viennent du fichier de configuration : voir [settings.md](settings.md)
et [config.example.toml](../config.example.toml). Une configuration invalide est signalée en
rouge dans la barre d'état (détail au survol), et les réglages par défaut sont utilisés.

Toujours en **`--release`** pour s'en servir : en build de débogage, le code Rust n'est pas
optimisé, et le 1:1 du JPEG boîtier passe de 0,4 s à 2 s (rotation, conversion pour le GPU),
le 100 % RAW de 4,5 à 7 s. `--cache` utilise un autre dossier de cache que celui du système
(mesures « à froid », essais).

## Ce qui est en place

- **Grille de vignettes** du dossier, alimentée en tâche de fond par le
  [scheduler](scheduler.md) ; seules les lignes visibles sont dessinées
  (`ScrollArea::show_rows`), ce qui tient des milliers d'images.
- **Icônes des panneaux** (17/10/2026) dans la barre d'état : à gauche, panneau des
  dossiers, barre de filtres, bandeau de notation ; à droite, panneau des métadonnées.
  Chaque icône dessine une fenêtre et la bande du panneau, pleine quand il est affiché ;
  le raccourci est rappelé au survol. Mêmes actions que le clavier.
- **Barre d'état** : progression en **gris discret** (`[gui.colors] progress`, gris du
  thème par défaut) plutôt qu'en couleur d'accent, qui attirait le regard sur une
  information de fond (retour du 17/10/2026) ; dossier, nombre d'images (dont RAW), filtre, progression des vignettes,
  des aperçus « JPEG boîtier » (masquée s'il n'y a aucun RAW) et des aperçus standard, nom
  de l'image sélectionnée.
- **Sélection** au clavier ou à la souris ; le scheduler est prévenu (`set_focus`), donc
  l'aperçu de l'image sélectionnée et de ses voisines passe en tête de file.
- **Sélection au clic, survol encadré** (17/10/2026, deuxième version) : la sélection
  (fond turquoise plein) ne change qu'au clic ou au clavier ; la vignette survolée par la
  souris n'est qu'encadrée en gris clair (`[gui.colors] hover`, `[gui.grid] hover_frame`).
  Historique : au départ, survol (fond gris) et sélection (turquoise) se ressemblaient au
  point de faire croire à deux images sélectionnées ; on a d'abord essayé « le survol
  sélectionne » (un seul curseur), mais aller vers le bandeau de notation en traversant
  d'autres vignettes changeait alors l'image notée. Des styles bien distincts (cadre
  contre fond plein) règlent le premier problème sans créer le second. Le pointeur, et
  avec lui le cadre, est **masqué dès qu'on tape** et réapparaît au premier mouvement de
  la souris, dans la grille comme en plein écran.
- **Panneau des dossiers** (17/10/2026), à gauche de la grille : emplacements dépliables
  en arborescence, rangés par sections titrées et séparées d'un trait (20/10/2026) :
  disques amovibles, réseau, dossiers personnels (avec Bureau, Documents,
  Téléchargements, Images), favoris, puis seulement le système (racine, repliée) ; case
  **Sous-dossiers** (20/10/2026, gardée) : les images des sous-dossiers s'affichent aussi
  (`[gui.folders] subfolders_depth` niveaux), lue à la demande par le
  moteur (`places::Tree`, voir [places.md](places.md)). Le triangle déplie ou replie (le
  double-clic sur le nom aussi), un clic sur le nom ouvre le dossier sans redémarrer
  (`folder::Folder`, voir [folder.md](folder.md)) : les filtres sont gardés, la sélection
  repart de la première image. Un disque ou une carte branchés apparaissent d'eux-mêmes en quelques secondes
  (surveillance du moteur, `places::Monitor`) ; « Actualiser » ou `F5` relit tout de suite
  volumes et dossiers. Chaque volume a une icône « éjecter » : confirmation (modale), le
  dossier ouvert est fermé s'il est sur ce volume (sinon le système refuse), puis
  éjection en fond (`places::eject`) ; le résultat, ou le message du système, s'affiche
  dans la barre d'état. Un volume **fixé** (`[places] fixed`, ex. le disque de travail
  toujours branché) n'a pas d'icône d'éjection mais une icône de disque, non cliquable :
  pas d'éjection par accident (idée de l'utilisateur, 17/10/2026). Le
  dossier ouvert est déplié et montré au démarrage. `Maj+D` affiche ou masque le
  panneau ; réglages `[gui.folders]` (affiché, largeur, retrait, dossiers cachés). Sans
  dossier en argument, `tri-gui` rouvre le dernier dossier ouvert (`[gui] reopen_last`,
  mémorisé par le moteur dans le dossier de données), sinon s'ouvre sur le panneau. La barre de filtres passe à la
  ligne quand la place manque.
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
    séquence retire le filtre), `f 6`…`f 9`, `f °`, `f ,`, `f .` couleurs (ajouter / retirer),
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
- **Netteté** (17/10/2026, moteur : `tri::sharpness`, voir `sharpness.md`) : en plein
  écran (ajusté ou 1:1 depuis le 21/10/2026), `p` surligne les contours nets (focus
  peaking), `Ctrl+P` change de couleur (3 couleurs réglables, `[gui.sharpness] colors`) ;
  seuil lié au curseur de sévérité (`Ctrl+J` / `Ctrl+K`, curseur dans le bandeau).
  Calculée par le chargeur, teintée au dessin.
- **Photos floues** (17/10/2026, moteur : score de `tri::sharpness`, voir `sharpness.md`) :
  mention « floue » sur la vignette, ligne « Netteté » du panneau des métadonnées, filtres
  « sans les floues » (`f b`) et « floues seulement » (`f B`) ; jugement relatif à la médiane du dossier. Les barres de
  progression terminées sont masquées (`[gui] progress_hide_done`), la barre d'état reste
  courte.
- **Collimateur** (17/10/2026) : repère du collimateur de mise au point en plein écran et
  en comparaison (`m`), lu dans le RAW (Fujifilm pour l'instant) ; carré, croix, cercle ou
  cœur (`[gui.view] focus_marker`), couleur `[gui.colors] focus`, liseré sombre dessous
  pour rester visible sur tout fond.
- **État d'affichage gardé** (17/10/2026, `gui/src/state.rs`) : ce qu'on affiche ou
  masque d'une touche (collimateur, carte de netteté, zones, histogramme, panneaux,
  barres, avance automatique) est repris au lancement suivant, depuis `gui-state.toml`
  du dossier de données ; la configuration ne donne que la valeur du premier lancement
  (`[gui] remember_display = false` : toujours elle). Écrit seulement quand l'état change
  (fichier temporaire puis renommage), jamais pendant une capture automatique ni dans les
  tests (dossier de données temporaire).
- **Aide-mémoire des touches** en haut à droite du plein écran (« b : JPEG boîtier / RAW,
  z : 1:1… ») : masqué par défaut depuis le 17/10/2026 (il mordait sur l'image, F1 ouvre
  l'aide complète avec recherche) ; `[gui.view] key_hints = true` pour le retrouver.
- **1:1 depuis la grille** (17/10/2026) : `z` (ou le bouton « 1:1 » du bandeau) ouvre
  l'image sélectionnée directement à 100 %, pour vérifier vite la netteté ; Échap ramène
  à la grille. Au clavier et au bouton, le 1:1 est **centré sur le collimateur** quand le
  boîtier l'a enregistré (`[gui.view] zoom_on_focus_point`), sinon au milieu ; si les
  métadonnées ne sont pas encore lues, il se recentre dès leur arrivée. Le double-clic
  zoome toujours sur le point cliqué.
- **Bandeau de notation, à droite** (17/10/2026) : 1:1 (fond turquoise quand on y est),
  image précédente / suivante (chevrons, utiles surtout en 1:1) et avance automatique (étoile suivie d'une flèche : « après une
  note, on avance » ; fond turquoise quand elle est active), en boutons à icône
  (`[gui] bar_icon_size`), libellé et raccourci au survol.
- **Histogramme en courbe lissée** (18/10/2026, retour : les barres faisaient escalier) :
  les cases du moteur sont regroupées en classes (`[gui.histogram] classes`), lissées par
  un noyau gaussien (`smoothing`, écart type en classes), puis dessinées en surface pleine
  par canal avec un liseré. Les classes larges comblent les cases vides d'un JPEG en
  échelle IL (l'ancien « peigne »). **Pastilles** saturés / bouchés des coins : allumées
  au-delà de `indicator_min` (0,1 % des pixels) et non plus dès un pixel (sur DSCF0305 :
  0,007 % de bouchés, quelques pixels morts, allumait la pastille, la courbe loin du
  bord). Le **grand histogramme** (`Maj+E`) est une boîte comme les autres : titre,
  déplaçable, boutons en bas, à sa propre largeur (`Dialog::free_width`).
  Couleurs en **mélange additif** (retour du 18/10/2026) : rouge + vert = jaune, les trois
  = blanc, comme la lumière (`fill_strength`). Le **petit** histogramme (panneau) ne montre
  que la courbe ; la ligne d'indicateurs (source, marge ; saturés et bouchés sont dans
  le tableau, par canal, retour du 18/10/2026) et un **tableau de
  statistiques** par canal ne sont que dans le grand : médiane, moyenne, plage des ombres
  aux hautes lumières (centiles `stats_tail`, 1 % et 99 %), écart-type, saturés, bouchés,
  en IL sous la saturation (`Exposure::channel_stats`, calculé dans le moteur à partir
  de l'histogramme en IL déjà en cache). Pas de luminance sur des données RAW (un
  photosite ne mesure qu'une couleur) : le vert en tient lieu, c'est indiqué.
  **Outil d'analyse** (idée de l'utilisateur, 18/10/2026) : quatre pastilles en bas à
  droite du grand histogramme (rouge, vert, bleu, blanc = tous ; `Maj+C` les fait tourner
  et ouvre le grand histogramme) n'affichent qu'un canal, sa seule ligne de statistiques
  et ses zones. À droite du tableau, une **petite image d'analyse** : la vignette en gris,
  zones saturées en jaune et bouchées en bleu (`analysis_over`, `analysis_under`), d'après
  le masque d'exposition du moteur (bits par canal, `tri::exposure::mask`) ; tous canaux :
  saturé dans un canal quelconque, bouché dans l'ensemble des canaux
  (`histogram::analysis_image`, testé). Mise en page : depuis le 19/10/2026, un carré à
  droite de l'histogramme, à sa hauteur, fond un peu plus clair que la boîte
  (`analysis_lighten`), image centrée, sans légende (voir plus bas). Le pointeur
  « saisir » de la boîte déplaçable ne remplace plus celui d'un élément (main sur les
  pastilles).
- **Actualiser** (panneau des dossiers) : icône de flèche circulaire, libellé, raccourci
  (F5) et explication au survol.
- **Boîtes** (notation au clic droit, éjection, déchargement, renommage, corbeille ;
  retour du 17/10/2026) : **déplaçables** en glissant leur fond ou leur titre, plus
  grandes et aérées (`[gui.dialogs]` : marges, espacement, largeurs ; titre en couleur
  d'accent, `title_color`, souligné d'une ligne de séparation).
  La `Modal` d'egui ne se déplace pas (sa zone couvre aussi le fond grisé) : `Dialog`
  garde la position et la décale au glissement ; le cadre est dans la zone de
  glissement, et les textes n'y sont pas sélectionnables (sinon glisser le titre
  sélectionnait du texte). Boutons et champs restent prioritaires.
- **Barre de filtres compacte** (retour du 17/10/2026 : elle débordait sur un écran de
  portable) : icônes dessinées (étoiles ; drapeaux : coche verte = acceptées, croix grise
  = masquer les rejetées, croix rouge = rejetées seulement ; goutte d'eau barrée =
  masquer les floues, goutte = floues seulement, `[gui.colors] blur_drop` ; couleurs),
  compteur « 26/26 » ; le libellé complet et le raccourci de chaque bouton au survol.
  Type de fichier (21/10/2026, retour d'un testeur : « Plates » n'était pas clair) : deux
  **interrupteurs**, des étiquettes RAW et JPG (JPEG, TIFF, PNG, HEIC…) à la taille des
  autres icônes (sans icône de page, retour du 21/10/2026), à la place de « Tout / RAW /
  Plates » ; allumé, le type est montré, l'un des deux reste allumé (`marks::file_type`). Curseur de sévérité du flou montré seulement avec un filtre sur
  la netteté, comme celui des piles avec les piles ; même largeur (90 points).
- **Comparaison côte à côte** (17/10/2026, `c`, `gui/src/compare.rs` pour la disposition) :
  les images sélectionnées (au moins 2, au plus `[gui.compare] max`, 6 par défaut : chacune
  garde son aperçu et son 1:1 en mémoire graphique), disposées pour être vues le plus
  grand possible (nombre de colonnes choisi d'après les proportions des images et de la
  fenêtre, dernière ligne centrée). L'**image active** (cadre de la couleur de sélection)
  est l'image courante : panneau des métadonnées, histogramme et bandeau la suivent ;
  flèches / hjkl la changent (haut / bas d'une ligne à l'autre), clic aussi. Les notes
  vont à l'image active seule (avance automatique : vers l'image comparée suivante) ;
  `Espace` **l'élimine** (elle quitte la comparaison et la sélection : on garde la
  meilleure d'une rafale par élimination) ; `Entrée` ou double-clic l'ouvre en plein écran ;
  Échap ou `c` revient à la grille. `z` : **1:1 lié**, au même point relatif de chaque
  image, déplacé ensemble (glisser n'importe laquelle, Maj + flèches) ; `b` change la
  source de toutes ; `w` superpose les zones. Le moteur sert toutes les images comparées
  (`Scheduler::set_shown`), pas seulement l'image courante et ses voisines.
- **Sélection multiple** (17/10/2026, moteur : `tri::selection`, voir `selection.md`) :
  `Espace` marque, `v` mode visuel (plage qui suit le curseur, `v` valide, Échap annule),
  `Ctrl+A` / `Ctrl+Maj+A`, Maj+clic (plage), Cmd/Ctrl+clic (ajout / retrait) ; Échap
  désélectionne avant de quitter le plein écran. Notes, corbeille et éditeur s'appliquent
  aux images sélectionnées visibles. Cadre orange (`[gui.colors] marked`).
- **Développé dans darktable** (19/10/2026, voir `develop.md`) : `Maj+B` ou le bouton
  diaphragme lance le rendu (quelques secondes, gardé en cache) ; ensuite seulement, `b`
  passe du JPEG boîtier au rendu RAW puis au développé ; collimateur et zones masqués sur le développé.
  **Résolution** dans la ligne d'information (21/10/2026, `TriApp::shown_resolution`) :
  celle de l'image à 100 % dans la source affichée (1:1 chargé, sinon dimensions des
  métadonnées ; JPEG boîtier : `Metadata::embedded_size`, lue dans l'en-tête du JPEG
  embarqué) ; pour une version, son cadre, réduit au côté long de son préréglage
  d'export ; **pendant le recadrage**, le cadre en cours et le préréglage d'export de la
  barre (en direct). Rien pour le développé : darktable rend à la
  taille de l'aperçu, son recadrage compris ; la vraie taille demandera de lire ce
  recadrage dans le sidecar (à faire).
  Le choix « développé » reste d'une image à l'autre, mais une image **sans
  développement** se montre dans la source d'avant (boîtier ou RAW), avec son 1:1 ;
  « pas encore développé (Maj+B) » le dit (`TriApp::source_for`, retour du 20/10/2026 :
  après un Maj+B, plus aucune autre image ne zoomait).
- **Exporter** (19/10/2026, voir `export.md`) : `Ctrl+E`, boîte (`gui/src/export.rs`) avec
  les préréglages d'export (flèches haut / bas sur la liste seulement, résumé de chacun ;
  Tab passe au dossier) et le dossier (complétion par Tab) ; export en fond, barre de progression et bouton × dans la barre d'état, bilan à
  la fin. Profil choisi retenu pendant la session. Image exportée : ce qu'on regarde
  (grille : darktable, sinon LibRaw ; plein écran : la source affichée), modifiable dans
  la boîte (`export::Render`). Boîte épurée (retour du 19/10/2026) : profils, dossier et
  destination réelle seulement ; les explications (flèches, règle du dossier, rendu) sont
  dans la doc.
- **Rapport d'export** (20/10/2026, voir `export.md`) : `Ctrl+Maj+E` ou bouton
  « Rapport d'export » avant le bilan dans la barre d'état ; `export::report_dialog`,
  journal `TriApp::export_log` lu au lancement. Le clic du bouton qui l'ouvre tombe aussi
  sur le fond de la boîte dans la même image : ignoré (`export_report_since`).
- **Icône, barre du haut, mode concentration** (19/10/2026) : icône choisie parmi trois
  pistes (un « t » et l'étoile de la note, `scripts/make-icon.py`, images dans
  `gui/assets`) pour la fenêtre, le Dock et la barre des tâches ; **barre du haut** avec
  le **mot-symbole « t★ri »** à gauche, comme darktable (idée de l'utilisateur : plus net
  que l'icône suivie du nom ; lettres teintées de la couleur du texte du thème, étoile
  jaune, version au survol ; `[gui] title_bar`, `title_bar_size`). **Mode concentration** (`F11`, `Ctrl+Maj+F` : sur macOS, F11 montre
  le bureau) : plein écran du système, toutes les barres et tous les panneaux masqués
  (pas fermés : ils reviennent tels quels), indication pour en sortir quelques secondes
  en haut (`focus_mode_hint_ms`). Échap n'en sort pas : il sert à revenir à la grille.
- **Thème gris neutre** (`[gui] theme = "neutral"`, 19/10/2026, façon Lightroom) :
  panneaux gris foncé, fond central gris neutre en grille comme en plein écran (un fond
  noir fait paraître les images plus claires et plus contrastées), cellules un peu plus
  claires, fonds des piles accordés, noms des images en sombre sur les cellules, champs
  de saisie à part ; tons relevés sur une capture de darktable (panneaux `#2a2a2a`, fond
  `#5e5e5e`, cellules `#767676`, champs `#3e3e3e`, sélection gris clair `#b3b3b3` et
  texte sombre, sauf `selection` donnée) ; couleurs `[gui.colors] neutral_*`. Retouche
  seulement les fonds du thème sombre d'egui (`Theme::apply`), remis d'origine en
  changeant de thème ; appliqué à chaud depuis les réglages.
- **Recherche** (19/10/2026) : `/` en grille et dans le panneau des dossiers (en plein
  écran, `/` reste « ajusté »), Ctrl+F partout. **Grille** : champ après le compteur de
  la barre des filtres (`[gui.grid] search_width`), filtre en direct par nom de fichier
  (`tri::fuzzy::NameFilter`, combiné aux autres filtres, « Effacer » le vide aussi) ;
  Entrée garde le filtre et rend le clavier à la grille, Échap l'efface. **Dossiers** :
  champ sous le titre ; pendant la recherche, les résultats remplacent l'arborescence
  (voir `places.md` : `~/…`, `/…`, sinon depuis le dossier pointé) ; ↑ ↓ choisissent,
  Entrée ou un clic ouvre (et montre le dossier dans l'arborescence), Échap referme.
  egui retire le clavier au champ dès qu'Échap arrive : `search_focused` retient le champ
  actif de l'image précédente pour qu'Échap lui revienne.
- **Grand histogramme revu** (19/10/2026) : photo d'analyse dans un **carré à droite de
  l'histogramme, à sa hauteur**, sans légende (« les couleurs parlent d'elles-mêmes ») ;
  masquée par défaut (`[gui.histogram] analysis`), `p` ou l'icône de photo (à gauche de
  celle de l'échelle, en couleur d'accent quand elle est montrée) la basculent, et
  l'histogramme prend alors toute la largeur ; statistiques dessous. Les déplacements
  (flèches, hjkl, Début / Fin, pages) passent sous la boîte : on parcourt les images,
  l'histogramme suit. L'échelle se bascule par une **icône** à gauche des pastilles de canal
  (droite : linéaire, courbe logarithmique : IL ; elle montre l'échelle en cours), plus
  par un bouton texte à côté de « Fermer ».
- **Superpositions** (`o`, `Maj+O` orientation, comme Lightroom, 19/10/2026, voir
  `overlay.md` ; les zones saturées / bouchées passent de `o` à `w`) : grilles intégrées
  et superpositions de l'utilisateur (texte, logo, format 4:5…), en plein écran et en
  comparaison ; nom dans la barre d'état.
- **AirDrop** (19/10/2026, voir `send.md`) : action `plugin.airdrop.send`, bouton de
  partage dans la boîte du clic droit, sans touche par défaut ; macOS seulement (masquée
  ailleurs : `Action::available`).
- **Au clavier dans l'aide et les réglages** (retours du 18/10/2026) : l'aide défile aux
  flèches haut / bas (`[gui.dialogs] help_scroll_line`), Page préc. / suiv. et Ctrl+U /
  Ctrl+D, la recherche gardant les lettres. Réglages : la liste des sections défile
  jusqu'à celle choisie aux flèches (et les réglages repartent du haut) ; depuis la
  recherche, → (recherche vide), Entrée ou Tab donnent le clavier au premier réglage
  montré, puis Tab / Maj+Tab passent de l'un à l'autre. Un test parcourt toutes les
  sections : il a trouvé un « ← » des commentaires de l'exemple, absent de la police
  d'egui (remplacé par des mots).
- **Curseur masqué** (18/10/2026, pour voir les piles d'ensemble) : en grille, Échap
  quand il n'y a plus rien à fermer (sélection multiple déjà vidée), ou un clic dans le
  vide (qui vide aussi la sélection), masque l'image en surbrillance. Une action qui vise
  l'image courante (se déplacer, noter, supprimer, ouvrir, sélectionner…,
  `uses_cursor`) le fait seulement réapparaître, à sa place, sans agir sur une image
  qu'on ne voyait pas ; un clic sur une image aussi. Filtres, panneaux, bandeaux restent
  utilisables curseur masqué. Recliquer l'image ou Espace écartés : double-clic (ouvrir)
  et sélection multiple.
- **Raccourcis dans la fenêtre de réglages** (section « Raccourcis [keys] », 18/10/2026) :
  une ligne par action, par thème comme l'aide (la recherche trouve aussi les actions :
  libellé, identifiant, touches). Ses raccourcis en pastilles (ceux du profil, ou ceux de
  `[keys]` : libellé en gras), un clic retire ; « enregistrer… » prend les touches
  tapées ensuite (Échap compris, la fenêtre ne se ferme pas ; une séquence touche après
  touche, finie après le délai des séquences ; un caractère sans touche nommée, « ° »,
  pris comme caractère) ; le petit champ accepte un raccourci écrit (« g g ») ;
  « défaut » revient au profil. Les conflits s'affichent en tête et sous l'action.
  `Keymap::specs_for` / `keys_spec` écrivent les raccourcis comme `[keys]` les lit (un
  test vérifie l'aller-retour de tous les raccourcis des profils).
- **Fenêtre de réglages** (`Ctrl+,`, 18/10/2026 ; `gui/src/settings_window.rs`, moteur :
  `tri::config_edit`, voir `config_edit.md`) : sections à gauche (flèches haut / bas
  depuis la recherche), réglages et leurs commentaires à droite, recherche floue comme
  l'aide (nom, section, description). Champ selon le type : case à cocher, nombre à
  glisser ou taper, couleur (sélecteur), texte ou liste TOML (écrits à la validation).
  Clé présente dans le fichier en gras, bouton « défaut » pour la retirer. Chaque
  changement est validé (fichier entier relu) puis écrit (quand la souris est lâchée) ;
  une valeur refusée revient en arrière avec le message d'erreur. Appliqués tout de
  suite : interface (`[gui.*]`), raccourcis, langue, `[files]`, `[xmp]`, `[offload]` ;
  le moteur (`[preview]`, `[render]`, `[cache]`, `[scheduler]`, `[exposure]`,
  `[sharpness]`, `[places]`, `[data]`) et la taille de la fenêtre au prochain lancement,
  c'est indiqué. Fichier : celui lu au lancement, sinon `--config` ou l'emplacement par
  défaut. Sections nommées par un libellé traduit suivi du nom du fichier en gris
  (« Couleurs [gui.colors] », `settings-section-*` dans les catalogues ; un test vérifie
  qu'aucune section n'en manque). Les clés de dossier (`dir`, `*_dir`, `destination`) ont
  la complétion par Tab.
- **Zoom continu et loupe** (18/10/2026, `[gui.zoom]`) : l'échelle (1,0 = 100 %, un
  pixel de l'image par pixel de l'écran) va de l'ajusté à `max` (400 %). Clavier, par
  **paliers** comme Lightroom : `+` / `-` (rangée du haut ou pavé numérique : liés comme
  caractères, la touche dépendant de la disposition) palier supérieur / inférieur
  (`steps`), `*` 100 %, `/` ajusté (inversés le 18/10/2026, comme Lightroom), `z` et le
  double-clic basculent ajusté / 100 % ; on peut **dézoomer sous l'ajusté** jusqu'à
  `min` (10 %), l'image plus petite que l'écran, centrée ; le bouton 1:1 n'est allumé
  qu'à 100 %.
  Souris et pavé tactile, en **continu** : pincer, ou Ctrl / Cmd + molette, zoome autour
  du pointeur ; défiler déplace l'image zoomée. Au-delà de l'ajusté, c'est le 1:1 qui est
  affiché, réduit ou agrandi ; l'échelle s'affiche (« 200 % ») ; elle est gardée d'une
  image à l'autre, comme la position, et vaut aussi en comparaison. **Loupe** : en vue
  ajustée, tant que `loupe_key` est maintenue (`t` par défaut depuis le 19/10/2026 :
  avec Maj, chaque raccourci Maj+… la faisait apparaître ; une touche libre ou un
  modificateur, une touche déjà liée à une action est signalée ; `<` impossible, egui
  n'en reçoit que le caractère), après `loupe_delay_ms` (pas de clignotement sur une
  pression brève), un carré de `loupe_size` points sous le pointeur montre
  l'image à `loupe_scale` (100 %) ; son 1:1 est chargé pour l'occasion, l'aperçu agrandi
  en attendant (« 100 % en cours »).
- **Bouton « tout déplier »** à côté du curseur de similarité (`Maj+S`).
- **Piles** (18/10/2026, `gui/src/stacks.rs`, moteur `tri::similarity`, voir
  `similarity.md`) : les images semblables voisines (rafales, séries) forment des piles,
  repliées par défaut : une vignette, la **couverture** (l'acceptée, sinon la mieux notée,
  sinon la plus nette, sinon la première), avec le nombre d'images dans une pastille
  d'accent et des feuilles derrière ; dépliée, un trait d'accent continu au-dessus de ses
  images. `s` déplie / replie la pile de l'image, `Maj+S` toutes. La grille n'est qu'une
  **vue** : la liste complète reste celle du moteur (calculs de chaque image, netteté) ;
  la navigation et l'avance automatique sautent les images cachées. **Fenêtre de
  regroupement** (`Ctrl+G`, comme Capture One) : regrouper ou non, curseur du seuil (la
  grille suit en direct), écart de temps maximal, nombre de piles ; le seuil est retenu
  pour le dossier à la fermeture (`group-thresholds.tsv` du dossier de données ; le
  défaut : `[similarity] threshold`).
  Second retour du 18/10/2026 : **un seul interrupteur** (`Ctrl+S`, bouton pile de la
  barre des filtres et du bandeau, fond d'accent quand actif, toujours visible ; clic
  droit : `Ctrl+G`) : piles actives, regroupement en grille **et** en plein écran ← / →
  dans la pile, ↑ / ↓ de pile en pile ; inactives, ← / → de photo en photo. Piles en
  **tons de gris**, plus de turquoise. Troisième retour : toutes piles dépliées, cadres
  et traits ne se lisaient plus (un trait sous chaque image) ; une pile dépliée a
  maintenant un **fond**, deux gris en alternance d'une pile à la suivante
  (`[gui.colors] stack_band`, `stack_band_alt`), comme pendant le réglage du curseur.
  Grille **non groupée au démarrage** (retour du 19/10/2026) : `[gui.grid] stacks`
  (faux par défaut) ; les empreintes se calculent en fond quand même
  (`[similarity] background`), `Ctrl+S` regroupe aussitôt.
  Quatrième retour : une photo isolée est un groupe d'une image, avec son fond aussi
  (deux photos isolées voisines ne se confondent plus) : piles actives, chaque groupe
  alterne, deux cas au lieu de trois.
  Retours du 18/10/2026 (remplacés depuis) : cadre autour de chaque image et trait en
  bas ; **curseur de similarité** dans la barre des filtres (icône de
  pile : `Ctrl+G`) : pendant le réglage, les piles restent repliées, on voit les piles
  obtenues (retour du 20/10/2026) ; **Maj** tenue pendant le réglage les déplie, une sur
  deux sur un fond plus clair (`stack_band`) ; seuil retenu pour le dossier au lâcher. **Mode pile** en
  plein écran (`Ctrl+S`, bouton du bandeau, inspiré du mode tri de Capture One) : ← / →
  parcourent la pile, images cachées comprises (au bout : arrêt, ou retour à l'autre bout
  avec `[gui.view] stack_wrap`), ↑ / ↓ passent à la couverture de la pile d'avant /
  d'après (boutons ↑ ↓ à côté de ‹ ›), « pile : 3 / 12 » dans la barre d'état. **La
  préférée** : Entrée en mode pile, ou `Maj+Entrée`, fait de l'image la couverture de sa
  pile, séparée des notes (une par pile, retenue : `stack-covers.txt` du dossier de
  données ; la rechoisir l'oublie).
- **Panneaux au clavier, façon tmux** (18/10/2026, `actions::Scope`) : le leader
  (`[gui] leader`, `Ctrl+B` par défaut) puis `h` / `l` (ou ← / →) donne le clavier au
  panneau de gauche / de droite, `o` au suivant (dossiers, images, métadonnées ; un
  panneau masqué s'affiche, les dossiers n'existent qu'en grille). Le panneau actif a un
  liseré d'accent, la barre d'état le dit. **Ses touches passent d'abord, les autres
  gardent leur effet** (choix de l'utilisateur : `3` note toujours l'image courante) ;
  Échap rend le clavier aux images. Dossiers : ↑ ↓ / `j` `k`, → / `l` déplie (déjà
  déplié : entre), ← / `h` replie (déjà replié : remonte), Début / `gg`, Fin / `G`,
  Entrée ouvre et rend le clavier aux images (curseur dans le moteur :
  `places::Tree::cursor`…) ; sur un support amovible, `o` le décharge et `e` l'éjecte
  (mêmes boîtes que les icônes ; `o` plutôt que `d`, qui masquerait le `dd` de vim) ; sur
  un dossier ordinaire, `o` **décharge ici** : boîte de déchargement avec ce dossier pour
  racine (pas retenue pour les suivants), et une liste pour choisir la carte s'il y en a
  plusieurs. Dossiers (idée du 18/10/2026, comme nvim-tree) : `a` crée un dossier dans le
  dossier pointé, `r` / F2 le renomme, `x` / Suppr le supprime (confirmé, avec ce qu'il
  contient ; corbeille selon `[files] trash`) ; les emplacements ne se renomment ni ne se
  suppriment ; tout s'annule (`u`), le dossier ouvert suit son renommage. **Favoris**
  (18/10/2026) : `m` (« mark », comme les marque-pages de nvim-tree) ajoute le dossier
  pointé aux favoris ou l'en retire ; ils sont en tête du panneau, avant les volumes,
  marqués d'une étoile (le dossier aussi, là où il paraît dans l'arborescence), et
  suivent un renommage fait dans `tri`. À la souris :
  **clic droit** sur un dossier, menu des mêmes actions (nouveau dossier, renommer,
  supprimer, favori ; décharger une carte ici, ou sur un support amovible : décharger,
  éjecter), raccourci au survol.
  Métadonnées : l'histogramme en tête puis chaque ligne (netteté comprise) se pointent
  (↑ ↓ / `j` `k`), Entrée sur l'histogramme l'ouvre en grand ; Maj + ↑ ↓ / Maj + `j` `k`
  (ou Maj+clic) étendent la sélection, `y` la copie (une ligne : la valeur ; plusieurs :
  « libellé : valeur » par ligne ; `Ctrl+C` / `Cmd+C` aussi), Maj+`y` copie tout le
  panneau ; un clic sur une ligne la pointe et donne le clavier au panneau. Grand
  histogramme ouvert : `r`, `g`, `b` un canal, `a` tous (il prend les touches le temps
  d'être ouvert : `b` n'y change pas la source). Un seul keymap : chaque action dit son panneau (`Action::scope`) ; à la
  recherche d'une touche, celle du panneau actif l'emporte sur l'action générale, et les
  conflits ne sont signalés qu'entre actions d'un même panneau. Pas de `z` (agrandir le
  panneau, comme tmux) pour l'instant : `Maj+D` / `Maj+M` masquent déjà les panneaux.
- **Annuler / rétablir** (18/10/2026, moteur `tri::history`, voir `history.md`) : `u` ou
  `Ctrl+Z` (`Cmd+Z` sur macOS), `Ctrl+R` ou `Ctrl+Maj+Z` ; `Ctrl+H` : liste de
  l'historique de la session, cliquer un pas y revient. Notes, renommage, copie /
  déplacement, corbeille ; refus expliqué si un fichier a changé ailleurs.
- **Champs de chemin** (18/10/2026, `gui/src/path_field.rs`) : partout où l'on tape un
  dossier existant (copier / déplacer, racine du déchargement, réglages), Tab complète
  comme un terminal : un seul dossier possible, il est pris ; sinon le début commun, et
  la liste des dossiers sous le champ.
- **Pointeur dans les boîtes** : la main sur tout ce qui se clique (`interact_cursor` dans
  le cadre commun `Dialog`) ; « saisir » ailleurs, pour déplacer la boîte.
- **Actions sur les fichiers** (17/10/2026, moteur : `tri::files`, voir `files.md`) :
  renommer (`F2`), mettre à la corbeille (`Suppr`, `Backspace` — la touche « effacer »
  des Mac — ou `x`, comme dans vim ; `dd` retiré le 18/10/2026), afficher dans le Finder / le gestionnaire de
  fichiers (`Maj+R`), ouvrir dans l'éditeur externe (`e`, `[files] editor`), copier
  (`y`) ou déplacer (`Ctrl+X`, couper) vers un dossier, **toute la pile** de l'image :
  `dw` (corbeille) / `yw` (copier), comme un mot dans vim (idée de l'utilisateur,
  18/10/2026 : une ligne de la grille, `dd`, n'avait pas de sens ; piles actives
  requises). `y` étant aussi le début de `yw`, il attend la suite un court instant
  (`[gui] key_ambiguous_timeout_ms`, 300 ms) : une touche qui est un raccourci et le
  début d'un autre n'est plus signalée comme masquée, le séquenceur
  (`Sequencer::expire`, `deadline`) la fait au bout du délai, ou avant la touche
  suivante qui ne continue pas. Copier / déplacer (18/10/2026, voir `files.md` : champ avec
  complétion par Tab, destinations récentes aux flèches, progression). Aussi par six
  boutons à icône dans la boîte du clic droit (crayon, deux feuilles, flèche dans un
  dossier, poubelle, dossier, flèche sortante ; libellé et raccourci au survol ; `[gui.dialogs] icon_size`). Renommer : le nom (sans extension) est
  sélectionné, on tape le nouveau ; la boîte montre ce que deviennent l'image, sa paire et
  ses sidecars, ou pourquoi c'est impossible (nom pris) ; Entrée confirme, Échap annule.
  Corbeille : la liste des fichiers (paire et sidecars compris) avant confirmation
  (Entrée). Ensuite le dossier est relu, la vue (grille ou plein écran) gardée : l'image
  renommée reste sélectionnée, après une suppression la suivante prend sa place. Erreurs
  dans la barre d'état. Code : `gui/src/files.rs` (boîtes), `TriApp::file_action`,
  `reload_folder`.
- **Avance automatique après une note** (`[gui] auto_advance`, désactivée par défaut ;
  `Maj+A` ou le bouton « avance auto » du bandeau) : après une note au clavier ou au
  bandeau, l'image suivante est sélectionnée (pas depuis la boîte du clic droit, qui note
  l'image cliquée). Deux règles (`TriApp::rate_selected`) :
  - **sous un filtre, ne jamais sauter d'image** : si la note fait sortir l'image du
    filtre (rejet sous « sans les rejetées », 2 → 1 étoile sous « 2 étoiles et plus »),
    elle disparaît et la suivante prend déjà sa place sous la sélection ; avancer en plus
    en sauterait une (bug connu de Capture One à une époque). On n'avance donc que si
    l'image notée est toujours dans la liste ;
  - **l'autre image d'une paire RAW+JPEG**, notée en même temps, est sautée.
  Échec d'écriture du sidecar : on reste sur l'image (l'erreur est dans la barre d'état).
- **Notation à la souris** (17/10/2026, demande de l'utilisateur : tout le monde ne veut
  pas tout faire au clavier) :
  - **bandeau de notation** en bas (au-dessus de la barre d'état), dans la grille et en
    plein écran : drapeaux, étoiles et couleurs de l'image sélectionnée, cliquables,
    estompés quand ils ne sont pas posés ; `Maj+N` l'affiche ou le masque
    (`[gui] rating_bar`) ; à droite, le bouton « avance auto » (voir ci-dessous) ;
  - **clic droit** sur une vignette ou en plein écran : **boîte modale** avec les mêmes
    contrôles, pour l'image cliquée (sélectionnée). Un simple menu ne suffisait pas
    (retour du 17/10/2026) : la souris qui sortait du menu sélectionnait d'autres
    vignettes, alors que la note allait à l'image cliquée. Derrière la boîte, rien ne
    bouge : ni survol, ni navigation ; seules les touches de notation (pour cette image)
    et Échap agissent. Elle reste ouverte pour poser plusieurs notes, et se ferme par
    Échap, « Fermer » ou un clic à côté. Elle s'ouvre **au point du clic** (`modal_at`),
    pas au centre : sur un grand écran, le trajet jusqu'au centre était long (retour du
    17/10/2026, même chose pour la confirmation d'éjection, ouverte près de l'icône) ;
  - **réaction au survol** (même retour) : sur les étoiles, pastilles et drapeaux
    cliquables (bandeau, boîte, barre de filtres), pointeur en main et fond surligné
    (`marks::clickable`) ;
  - chaque clic déclenche la même action que le clavier. **Recliquer une note posée ne
    l'efface pas** (retour du 17/10/2026 : recliquer « 2 étoiles » remettait à zéro, un
    vrai danger en tri rapide ; Lightroom et Capture One ont un contrôle à part) : un
    contrôle « ⊘ » (« Retirer les étoiles », comme la touche 0) et un cercle vide
    (« Ni acceptée ni rejetée », comme Maj+U). Seules les couleurs s'ajoutent ou se
    retirent au clic, comme au clavier ;
  - contrôles nommés pour l'accessibilité (`WidgetInfo`), ce qui permet aussi aux tests
    d'interface de cliquer « 3 étoiles ».
- **Déchargement d'une carte** (17/10/2026) : icône « décharger » sur chaque volume non
  fixé du panneau des dossiers ; boîte près du clic : inventaire en fond, racine et
  dossier de la séance (proposé `AAAA-MM-JJ_` ; racine mémorisée), cases à cocher (RAW seulement,
  ouvrir le dossier, éjecter la carte, supprimer de la carte les fichiers copiés et
  vérifiés), copie vérifiée en fond avec progression et « Interrompre », puis suites
  cochées enchaînées (suppression, ouverture, éjection) et bilan. **En arrière-plan**
  (« Continuer en arrière-plan », Échap) : la copie continue, progression cliquable dans
  la barre d'état ; à la fin, la séance est proposée (« Ouvrir la séance ») plutôt
  qu'ouverte d'office. Voir [offload.md](offload.md). Le travail est dans
  le moteur ([offload.md](offload.md)), la boîte (`offload.rs`) ne fait qu'afficher.
- **Panneau des métadonnées** (17/10/2026), à droite, en grille et en plein écran
  (`Maj+M`, `[gui.metadata]`) : lignes du moteur (`Metadata::rows`, voir
  [metadata.md](metadata.md)) pour l'image sélectionnée, valeurs à la ligne plutôt que
  d'élargir le panneau (noms d'objectif). Lecture dans un fil à part (`meta.rs`,
  `MetaLoader`) : quelques ms en local, mais un NAS ne doit pas figer l'interface ;
  résultats gardés en mémoire, oubliés au changement de dossier. En grille, le panneau
  montre aussi l'**histogramme** de l'image sélectionnée (en plein écran, il est sur
  l'image) ; ses indicateurs passent à la ligne quand le panneau est étroit.
- **Histogramme et alertes d'exposition** (17/10/2026), calculés par le moteur sur les
  **données du capteur** pour un RAW, même quand le JPEG boîtier est à l'écran (voir
  [exposure.md](exposure.md)) ; le dessin est dans `histogram.rs` :
  - **histogramme** en haut du panneau des métadonnées, en grille comme en plein écran
    (`i` l'affiche ou le masque) ; panneau masqué : pas d'histogramme (demande du
    17/10/2026 : la surimpression sur l'image gênait plus qu'elle n'aidait). **Grand
    histogramme** en boîte modale centrée (`Maj+E`, ou clic sur l'histogramme du
    panneau ; `[gui.histogram] large` : part de la largeur de la fenêtre), fermé par
    Échap, la même touche ou un clic à côté ;
    étendue = **plage utile de l'image** (~13,9 IL pour un RAW 14 bits, ~11,7 pour un
    JPEG ; `[gui.histogram] ev_shown` : plafond, 0 = automatique) ; canaux R, V, B superposés,
    une barre par case (pas par pixel, sinon un « peigne »), échelle **en IL sous la
    saturation** par défaut (`Maj+I` : linéaire), graduations, pastilles rouge / bleue si
    des pixels sont saturés / bouchés, et en clair : profondeur du RAW, marge avant
    saturation, parts saturées et bouchées ;
  - **zones** superposées à l'image (`w`, « warnings » ; `o` jusqu'au 19/10/2026) : masque du moteur en texture (échantillonnage au
    plus proche), étiré sur l'image (aussi à 100 %). Saturé **par canal** (demandé le
    17/10/2026, comme FastRawViewer) : un seul canal → sa couleur (`clip_red`,
    `clip_green`, `clip_blue`), deux canaux → couleurs additionnées (rouge + vert =
    jaune), les trois → `clip_over` (magenta) ; bouché → `clip_under` (cyan : le bleu se
    confondait avec « bleu seul saturé » et se voyait mal sur les ombres).
    `[gui.histogram] clipping_channels = false` : une seule couleur pour toute
    saturation. Couleurs et opacité réglables ;
  - **badges** dans la grille : pastille rouge (brûlée) / bleue (bouchée) en haut à droite
    de la vignette au-delà de `[gui.histogram] badge_over` / `badge_under`, détail au
    survol ; analyses lues dans le cache dans le budget de chargement de l'image ;
  - vérifié sur une vraie séance : photo entièrement brûlée (rouge partout, badge),
    portrait au pull noir (zones bleues dans les plis, badge bleu, -3,5 IL de marge),
    portraits sur fond blanc (rien, -0,45 IL de marge).
- **Paires RAW+JPEG** : noter `IMG.RAF` note aussi `IMG.JPG` (choix de l'utilisateur,
  17/10/2026), voir [notes.md](notes.md).
- **Aide** (`F1` ou `?`, fermeture par Échap ou `F1`) : chaque action avec un libellé
  compréhensible et ses touches, **rangée par thème** (Navigation, Affichage, Notation,
  Filtres, Panneaux, Application : `Action::group`) ; l'identifiant technique n'apparaît
  qu'au survol. L'aide est **modale** : tant qu'elle est ouverte, seules Échap, l'aide et
  Quitter agissent.
- **Recherche dans l'aide, à la fzf** : le champ en tête a le focus dès l'ouverture, on
  tape directement. Chaque mot de la requête doit se retrouver lettre par lettre, dans
  l'ordre, éventuellement avec des trous, dans le libellé, les touches ou l'identifiant
  (« pnd » → *panneau des dossiers*, « etoile 3 », « maj+e », « core.nav »), sans tenir
  compte des accents ni de la casse ; les mots peuvent être dans le désordre. Résultats
  triés par pertinence (`actions::fuzzy_score` : lettres consécutives et débuts de mots
  comptent plus, les trous pénalisent ; à égalité, le libellé le plus court). Pendant la
  saisie, `?` s'écrit dans le champ : on ferme par Échap ou `F1`.
- **Saisie dans un champ** (recherche de l'aide, nom de dossier du déchargement) : les
  touches sont du texte ; seules celles qui ne tapent rien (Échap, F1 à F12 :
  `Trigger::types_no_text`) restent des raccourcis.
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

Deux jeux de raccourcis sont livrés, **au choix** (`[gui] profile` : `"classic"` par
défaut, `"vim"`, ou `"both"` pour les deux ensemble, qui ne se recouvrent pas ; liste
déroulante dans la fenêtre de réglages). Décidé le 18/10/2026 : un utilisateur d'outils
classiques n'a que faire des touches de vim, et inversement ; ce qui est pareil pour les
deux n'est écrit qu'une fois, dans les listes communes (`BASICS`, notation, filtres,
sélection, fichiers, panneaux, histogramme ; `actions.rs`), le propre de chaque jeu dans
`CLASSIC` / `VIM`. Un test vérifie que chaque jeu couvre toutes les actions, sans conflit.
L'aide et les textes qui citent une touche la lisent dans la table en vigueur : ils
suivent le jeu choisi.

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
| `core.app.help` | F1, ? | F1, ? |
| `core.app.quit` | q | q |
| `core.file.copy` / `core.file.move` | Ctrl+C / Ctrl+X | y / Ctrl+X |
| `core.file.copy_stack` / `delete_stack` | Ctrl+Maj+C / Maj+Suppr | yw / dw |
| `core.file.delete` | Suppr, Backspace | x, Suppr, Backspace |
| `core.edit.undo` / `redo` | Ctrl+Z / Ctrl+Maj+Z, Ctrl+Y | u, Ctrl+Z / Ctrl+R, Ctrl+Maj+Z |
| panneaux (dossiers, métadonnées) | flèches, Début / Fin, Maj + flèches | j k h l, gg / G, Maj + j k, y |

Notation, commune aux deux profils (17/10/2026, pensée aussi pour l'AZERTY) :

| Action | Touches |
|---|---|
| `core.rate.star.0` … `core.rate.star.5` | 0 … 5, avec ou sans Maj |
| `core.rate.color.red` / `yellow` / `green` / `blue` | 6 / 7 / 8 / 9, avec ou sans Maj |
| `core.rate.color.purple` | ° / , / . |
| `core.flag.accept` / `reject` / `none` | Maj+P / Maj+X / Maj+U |

Pourquoi : sur l'AZERTY, les chiffres de la rangée du haut demandent Maj, ceux du pavé
numérique non ; egui-winit traduit la touche *logique* et, s'il ne la connaît pas (« & »,
« é »), la touche *physique* : les deux donnent `Num1`… Exception : le 6 non majuscule de
l'AZERTY est « - », reconnu comme `Minus`, d'où Maj sur la rangée du haut. Le violet est sur
« ° » (Maj + la touche à droite du 0 en AZERTY), qu'egui ne nomme pas : c'est un
**caractère**, reçu par l'événement `Text` ; seuls les caractères liés à un raccourci sont
pris en compte (les autres interrompraient une séquence). La virgule (`Comma`) et le
point (`Period`) couvrent le séparateur décimal du pavé numérique, « , » sur macOS mais
« . » sous Linux Mint (constaté le 17/10/2026 ; egui ne distingue pas le pavé), ainsi que
les touches de la rangée du bas. `u`
seul reste libre pour l'annulation (à venir), comme dans vim.

Une couleur déjà posée est retirée par la même touche ; en mode une couleur (défaut), une
autre couleur remplace la précédente.

Cmd et Alt ne déclenchent aucune action : ces combinaisons restent au système (Cmd+Q…).
Ctrl est permis (Ctrl+U / Ctrl+D du profil vim) ; Sous Linux et Windows, egui signale aussi
Ctrl comme `command` : seul `mac_cmd`, ou `command` sans Ctrl, désigne Cmd. Pas de raccourci
Ctrl + flèches (sur macOS, cela change de bureau), d'où Maj pour le déplacement.

Filtres, communs aux deux profils : voir « Filtres » plus haut (`core.filter.*` : `f f`,
`f 0`…`f 5`, `f p` / `f u` / `f x`, `f 6`…`f 9`, `f °`, `f ,`, `f .`, `f Échap`) ;
`core.view.filter_bar` : Maj+F ; `core.view.rating_bar` : Maj+N ; `core.view.folders` :
Maj+D ; `core.view.refresh_folders` : F5 ; `core.view.histogram` : i ;
`core.view.histogram_scale` : Maj+I ; `core.view.histogram_channel` : Maj+C ;
`core.view.clipping` : o ; `core.view.metadata` :
Maj+M.

Caractères : « ° » arrive comme une touche (Maj + `Minus`, physique, egui ne connaissant
pas « ° ») **puis** comme caractère, parfois dans une image suivante (constaté le
17/10/2026 sous macOS et Linux : `f °` notait en violet au lieu de filtrer). Deux
protections : `Keymap::chords` ignore une touche suivie d'un caractère lié dans la même
image, et le `Sequencer` ignore, au milieu d'une séquence, une touche liée à aucun
raccourci (même seule) : `f` reste en attente jusqu'au caractère. Une touche qui a un sens
seule interrompt toujours la séquence (`g` puis `j` : ligne du dessous). `f &` marche sur
l'AZERTY parce qu'egui, ne connaissant pas « & », se rabat sur la touche physique `1`.

**Section `[keys]`** du fichier de configuration, rangée par action :

```toml
[keys]
"core.flag.accept" = ["Maj+P", "a"]
"core.nav.first" = ["Début", "g g"]
```

- les raccourcis cités **remplacent** ceux des profils pour cette action (liste vide : plus
  aucun) ; le jeu choisi (`[gui] profile`) reste la base ;
- une touche s'écrit comme l'aide l'affiche (« Flèche gauche », « Page préc. », « Échap »,
  « Maj+P », « Ctrl+u ») ou avec le nom d'egui (« ArrowLeft », « PageUp ») ; une majuscule
  seule vaut Maj + la lettre (« G », comme dans vim) ; une séquence se sépare par des
  espaces (« g g ») ; un caractère sans nom de touche (« ° ») est reconnu comme caractère ;
- problèmes signalés au démarrage (stderr et barre d'état, détail au survol), sans
  invalider le reste du fichier : action ou touche inconnue, raccourci attribué à deux
  actions (le premier par ordre d'identifiant est gardé ; un raccourci du fichier l'emporte
  sur celui d'un profil, et le dit). Un raccourci qui commence une séquence plus longue
  n'est plus un problème : il attend la suite un court instant
  (`[gui] key_ambiguous_timeout_ms`) ;
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

L'aide défile si elle dépasse 75 % de la hauteur de la fenêtre (33 actions au 17/10/2026).

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
  prêt : pour voir un état intermédiaire), `rating_dialog` (boîte du clic droit pour
  l'image sélectionnée), `settings:<section>` (fenêtre de réglages ouverte sur une
  section : `settings:keys`). Une étape inconnue est signalée et ignorée.
  Exemple : `core.view.open,core.view.zoom,wait:1500,core.nav.next,wait:30,shot`.
- **Journal des plantages** (`gui/src/crash.rs`, 19/10/2026) : toute erreur Rust est
  consignée (message, fichier et ligne, fil, pile d'appels) dans `crash.log` du dossier de
  données, avant l'arrêt ; un plantage pendant le dessin de la fenêtre (appelé depuis
  AppKit sur macOS) ne laissait sinon qu'un rapport du système, sans le message.
- `TRI_GUI_TRACE=1` : trace minutée des étapes du démarrage (configuration lue, fenêtre
  ouverte, dossier lu, arborescence, première image, moteur ; ajoutées le 19/10/2026
  pour une lenteur de démarrage vue sur macOS) et de chaque étape du chargement des images (demande au
  moteur, image prête, abandonnée, lecture et décodage, image affichée) sur la sortie
  d'erreur. C'est elle qui a permis de mesurer les délais ci-dessus.

**Captures du manuel des testeurs** (`doc/manuel.md`, `doc/manual.md`) :
`scripts/manual-screenshots.sh <dossier de photos> [--lang fr|en]` copie les photos sur un
volume de démonstration (image disque « Photos » sur macOS : le panneau des dossiers ne
montre qu'elle), lance l'interface avec sa propre configuration, son cache et son dossier
de données (rien de l'utilisateur n'est touché), enchaîne les captures (grille, plein
écran, clic droit, comparaison, aide), numérote les zones et convertit en JPEG dans
`doc/manual-images/<langue>/`. Le jugement de netteté y est désactivé : sur un dossier de
sujets variés (gros plans sur fond flou parmi des paysages), il marquait à tort des
photos nettes.

## Tests

**Tests d'interface** avec [egui_kittest](https://crates.io/crates/egui_kittest) (banc de
test officiel d'egui) : l'application tourne sans fenêtre, sur un dossier et un cache
temporaires, et reçoit des touches simulées. Vérifié : l'aide s'ouvre avec F1 et
`?` (avec ou sans Maj) et se ferme avec F1, Échap la ferme, elle est modale, elle affiche
les libellés et non les identifiants, rangés par thème ; la recherche filtre (« pnd »),
« q » tapé dans la recherche ne quitte pas, et la recherche est vide à la réouverture. Plein écran : ouverture par Entrée et retour par Échap, navigation
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
bandeau ; clic droit sur la troisième vignette : elle est sélectionnée, la boîte s'ouvre ;
la souris passe sur la première vignette sans changer la sélection ; « 4 étoiles » et la
touche 8 notent la troisième image ; → ne navigue pas ; Échap ferme la boîte sans quitter
la grille.

Sélection multiple (17/10/2026) : voir [selection.md](selection.md#tests).

Netteté (17/10/2026) : voir [sharpness.md](sharpness.md#tests).

Boîtes (17/10/2026) : la boîte « Renommer », saisie par son titre et glissée, suit le
pointeur et reste ouverte.

Comparaison (17/10/2026) : `c` refusé avec une seule image (indication dans la barre
d'état) ; trois images en mode visuel puis `c` : l'image courante reste active, les
flèches restent dans la comparaison, la note ne va qu'à l'image active, `Espace` l'élimine
(comparaison et sélection), Échap revient à la grille en gardant la sélection, Entrée ouvre
le plein écran. Disposition (`compare.rs`) : deux paysages côte à côte en fenêtre large,
l'un sous l'autre en fenêtre haute, portraits, dernière ligne centrée, voisines bornées.
Moteur : `tests/scheduler.rs` vérifie que toutes les images comparées sont servies,
l'active d'abord, et que revenir au plein écran abandonne les autres.

Actions sur les fichiers (17/10/2026) : `F2`, le nom tapé remplace le nom sélectionné
(« q » ne quitte pas), la boîte annonce l'image et sa paire renommées, Entrée renomme sur le
disque et garde l'image sélectionnée ; un nom pris est refusé (rien ne change), Échap ne
ferme que la boîte ; `Suppr` puis Échap ne supprime rien, `Backspace` puis Entrée supprime
(définitivement, dans le dossier temporaire) et sélectionne la suivante ; les boutons de
la boîte du clic droit ouvrent l'action pour l'image cliquée.

Avance automatique (17/10/2026) : désactivée par défaut ; avec Maj+A, une note passe à
l'image suivante en sautant l'autre image de la paire (notée aussi, relue du disque), une
couleur aussi, la dernière image reste sélectionnée ; sous « sans les rejetées », rejeter
img0 laisse la sélection sur img1 (vérifié : sans la règle, le test échoue sur img2).

Déchargement (17/10/2026) : fausse carte (deux JPEG dans `DCIM`) déclarée comme volume,
icône « décharger », inventaire (« 2 à copier »), copie (« 2 fichiers copiés et
vérifiés »), « Ouvrir le dossier » : la séance s'ouvre à plat, historique écrit. Ce test a
trouvé un glyphe manquant (« → ») dans la boîte.

Métadonnées (17/10/2026) : la ligne « Dimensions » de l'image sélectionnée apparaît après
lecture en fond (64 × 48), `Maj+M` masque le panneau.

Exposition (17/10/2026) : une image blanche ajoutée au dossier donne une part saturée
> 99 % (badge) ; en plein écran, histogramme affiché par défaut, `i` le masque, `Maj+I`
passe en linéaire, `w` affiche les zones (masque en texture). Unitaires (`histogram.rs`) :
regroupement en colonnes sans perte de pixel, placement en IL (blanc dans la dernière
colonne, noir dans la première), couleurs du masque, nombres à la française.

Filtres (17/10/2026) : `f Maj+2`, `f 0`, `f u` (deux fois : retiré), `f 9`, `f °`,
`f Échap` ; rejeter sous « sans les rejetées » sélectionne l'image suivante ; barre
masquée / affichée par Maj+F, clics sur les interrupteurs JPEG et RAW, Effacer.

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
