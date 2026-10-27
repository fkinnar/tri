# Historique

*[English version](CHANGELOG.md)*

## À venir

- **Panneau de droite en sections** : la barre des filtres garde les filtres d'un clic
  et la recherche par nom ; leurs réglages et ceux de la grille passent dans le panneau de
  droite, en sections titrées comme celles du panneau des dossiers et repliables d'un
  clic : Histogramme, Filtres (réglages d'un filtre seulement quand il est allumé ; deux
  curseurs pour l'exposition ; logiciels de développement en cases à cocher), Grille
  (fichiers montrés, RAW et JPEG ; rendu, ordre, piles),
  EXIF en bas. Les sections du panneau des dossiers se replient aussi ; les sections
  repliées sont retenues. Titres des sections en majuscules, plus grands, à l'accent,
  espacés (`[gui.folders] section_scale`, `section_gap`). Après un changement de filtre,
  le nombre d'images s'affiche un instant au milieu de l'écran (`[gui.grid]
  filter_toast_ms`). **Au clavier** (leader puis `l`) : ↑ ↓ parcourent
  aussi les réglages, Espace les actionne, ← → les font varier, Tab / Maj+Tab (`)` / `(`
  en vim) passent d'une section à l'autre, dans le panneau des dossiers aussi. La
  section Grille commence par l'ordre ; le choix de l'entrée dans une pile ne reste que
  dans la barre du bas. Libellés « NOTE » et « FILTRE » sur les barres du bas et du haut,
  en petites majuscules à l'accent (`[gui.grid] bar_label_scale`). `[gui.grid] stack_slider` et `clip_slider` disparaissent : retirez-les de votre fichier de configuration si vous les
  aviez réglés.
- **Développements dans la grille** : après le développement, la grille peut montrer le
  développement de chaque RAW au lieu du JPEG boîtier : `[gui.grid] render` = `embedded`
  (défaut), `latest`, `darktable`, `rapidraw` ou `libraw`, aussi une liste de la barre
  des filtres et `f d`, retenu d'une session à l'autre. Un RAW sans le développement
  voulu montre son rendu LibRaw, calculé en fond ; les développements manquants sont
  rendus en fond (`[gui.grid] develop = "auto"`) ou à la demande (`"manual"`). Une
  pastille sous la vignette nomme le logiciel (issue n° 114).
- **Filtre « développées »** : une bobine de film dans la barre des filtres (`f Maj+D`),
  puis les logiciels installés à cocher : les images développées dans l'un d'eux,
  montrées dans le plus récent, quel que soit le rendu de la grille ; aucun coché, les
  images sans développement, en rendu LibRaw. Inversible ; les logiciels cochés sont
  retenus. Le plein écran s'ouvre sur ce que montre la vignette.
- **RapidRAW**, deuxième logiciel de développement à côté de darktable : `rapidraw
  export` applique son sidecar `.rrdata` ; trouvé dans le PATH, aux endroits habituels,
  en AppImage ou en Flatpak (`[develop] rapidraw`). `[develop] default` = `"auto"`
  (dernier développement) ou un logiciel. Interface de RapidRAW ouverte, son export
  donnait une image noire : elle est maintenant refusée avec un message clair (issue
  n° 114).
- **Choix du logiciel** : `<leader> r` (le logiciel de l'image), `<leader> d` darktable,
  `<leader> p` RapidRAW ; « Développer avec » dans la boîte du clic droit ; une boîte
  pour développer tout un dossier (chaque image dans son logiciel, ou toutes dans un
  seul). En plein écran, `b` passe par le JPEG boîtier, le rendu RAW, puis chaque
  développement déjà rendu, pour les comparer. Les erreurs de développement nomment leur
  logiciel. **Identifiants d'actions changés** : `core.develop.render`,
  `core.develop.folder` (et nouveaux `plugin.rapidraw.develop`, `core.grid.render`,
  `core.filter.developed`) : une section `[keys]` qui cite les anciens est à mettre à
  jour (issue n° 114).
- **« Darktable »** avec une majuscule partout où le nom s'affiche ; identifiants,
  valeurs de configuration et `darktable-cli` restent en minuscules.
- **Filtre des étoiles** : au moins, exactement ou au plus (`f =` ou le bouton ≥ / = / ≤
  devant les étoiles) (issue n° 165).
- **Supports amovibles** (Linux) : branchés mais pas montés, ils apparaissent grisés dans
  une nouvelle section « Périphériques », montés et ouverts d'un clic ; ceux qui arrivent
  sont montés d'eux-mêmes (`[places] auto_mount`, activé par défaut) ; les disques fixes
  ont leur section « Disques » (issue n° 113).
- **Panneau des dossiers** : titres de section plus grands que les dossiers, en gras
  (`[gui.folders] section_scale` ; issue n° 120).
- **Panneaux** : un trait en haut du panneau qui a le clavier (`[gui.dialogs]
  focus_line`, `focus_line_width` ; issue n° 77).
- **Aide et réglages** : les séquences qui commencent par le préfixe des panneaux
  s'écrivent `<leader> r`, le leader étant indiqué à part (issue n° 175).
- **Piles** : le bouton d'entrée dans les piles est aussi dans la barre de la grille, et
  son clic fonctionne ; entrer dans une pile repliée depuis la grille le suit aussi
  (issue n° 62). Sous `f c`, une pile ne se coupe plus en deux (issue n° 163). Piles
  actives, `c` sur une photo seule la compare (issue n° 167).
- **Touches** : une touche maintenue ne répète que les déplacements (navigation,
  défilement, zoom…), pas les actions ponctuelles : maintenir `e` ouvrait l'éditeur en
  boucle (issue n° 160).
- **Windows** : les programmes externes (éjection, éditeur) n'ouvrent plus de fenêtre de
  console (issue n° 159).
- **Dossiers** : copier ou déplacer un dossier n'emporte plus son propre `.tri-cache` ;
  un clic efface un message de la barre d'état (issue n° 157).
- **Tâches de fond** : replier rend au panneau sa hauteur (issue n° 158).
- **Fenêtre sous Wayland** : de nouveau agrandie au lancement quand elle l'était (issue
  n° 58).
- **Fenêtre étroite** : dans le menu « … » de la barre des filtres, les listes s'ouvrent
  en sous-menus (elles se refermaient avec le menu ; issue n° 181).
- **Linux** : `install.sh --uninstall` met à jour le cache d'icônes ; l'icône de
  l'application disparaissait après le passage de l'archive au `.deb`.
- **Plusieurs cartes à la suite** : la carte suivante est prise aussi après avoir
  refermé le bilan ou déchargé en arrière-plan, et quand elle est montée au même chemin
  que la précédente (deux cartes du même boîtier) (issue n° 37).
- **Tâches de fond** : repliées, une fine barre grise discrète (`[gui]
  tasks_folded_height`, texte au survol) ; elle reste à sa place une fois dépliée, un
  second clic replie le détail (issue n° 36).
- **Panneaux** : la ligne pointée au clavier est une bande grise discrète sur toute la
  largeur (`[gui.dialogs] cursor_tint`) au lieu d'un cadre d'accent, coupé à gauche dans
  le panneau des dossiers (issue n° 77).
- **Piles** : `f c` encore replie la pile qu'il avait dépliée (issue n° 52) ; le bouton
  « |‹ » (entrer dans une pile par sa couverture ou sa première photo) est toujours dans
  le bandeau du plein écran, et Ctrl+Maj+S dit lequel est actif (issue n° 62).
- **Grille** : un refiltrage (curseur d'un filtre, notes, analyses qui arrivent) garde
  la région affichée quand la sélection est hors de la vue, au lieu de revenir au début
  (issue n° 100).
- **Déchargement** : la barre de progression suit les fichiers quand certains sont
  passés (déjà déchargés ou déjà présents) (issue n° 101).
- **Disque retiré** sans éjection pendant les calculs : rebranché, le dossier ouvert
  est relu et les calculs reprennent (issue n° 98).
- **Fenêtre sous Wayland** : sa taille est retenue (la position reste au compositeur)
  (issue n° 58).
- **Dossier supprimé** : un sous-dossier supprimé (« Sous-dossiers » coché) quitte la
  grille et ses calculs s'arrêtent ; un lot darktable sur un dossier supprimé est
  interrompu.
- **Comparer une pile** : `c` sans sélection compare côte à côte les photos de la pile
  courante (repliée aussi) ; `f c` ne montre dans la grille que la pile courante,
  dépliée (issue n° 52).
- **Plusieurs cartes à la suite** : la boîte de déchargement restée ouverte sur son
  bilan, brancher la carte suivante la fait repartir sur elle, vers le même dossier de
  séance (issue n° 37).
- **Tâches de fond** : une seule barre globale (nombre de tâches, avancement moyen) en
  bas du panneau des métadonnées, ou dans la barre d'état s'il est masqué, au lieu
  d'une rangée de barres ; un clic déplie une barre par tâche au-dessus (issue n° 36).
- **Tout un dossier dans darktable** : Ctrl+Maj+B, ou le clic droit sur un dossier du
  panneau, rend en fond chaque image qui a un sidecar darktable, l'une après l'autre,
  dans le cache (progression dans les tâches de fond, « × » pour interrompre, bilan à
  la fin) (issue n° 69).
- **Grille** : plus de badge d'écrêtage sur une pile repliée, à la place de son nombre
  d'images (issue n° 75).
- **Panneaux** : le panneau qui a le clavier a un fond un peu plus clair (plus foncé
  en thème clair) au lieu d'un liseré d'accent ; `[gui.dialogs] focus_panel_tint`
  remplace `focus_frame_width`. Le curseur des métadonnées n'encadre plus
  l'histogramme (issues n° 77, 78).
- **Filtre « bien exposées »** : un seul curseur, avec un interrupteur à l'icône des
  badges d'écrêtage pour choisir entre hautes lumières brûlées et noirs bouchés, au
  lieu de deux curseurs aux libellés gris peu lisibles.
- **Filtres netteté et exposition pendant les analyses** : la grille se complète au
  fil du calcul (au plus toutes les `[gui] analysis_refilter_ms`, 5 s) au lieu
  d'attendre la fin, et la barre d'état dit combien de photos sont analysées quand on
  allume le filtre ; un refiltrage n'abandonne plus l'analyse en cours (issue n° 73).
- **Copier / déplacer vers…** : « Continuer en arrière-plan » referme la boîte pendant
  la copie ; sa progression est dans la barre d'état (un clic la rouvre) et la fin y
  est annoncée (issue n° 72).
- **Fenêtre étroite** : la fin de la barre des filtres ne recouvre plus le champ de
  recherche (tri et nombre d'images dans un menu « … », inversion et effacement
  toujours visibles), et les boutons de droite du bandeau de notation passent à la
  ligne au lieu de recouvrir les couleurs (issue n° 71).
- **Dernier dossier disparu** : lancée alors que le disque du dernier dossier est
  débranché, la fenêtre dit lequel et pourquoi, en grand au centre, et le rouvre
  d'elle-même quand le disque revient, sauf si un autre dossier a été ouvert
  entre-temps (issue n° 66).
- **Fenêtre** : sa taille, sa position et son agrandissement sont repris au lancement
  suivant (issue n° 58 ; position indisponible sous Wayland).
- **Piles** : début et fin de la pile avec `(` / `)` (vim) ou Maj+Début / Maj+Fin
  (classique) ; dans le profil vim, toute la pile est désormais `d)` / `y)` (au lieu
  de `dw` / `yw` : une pile est une phrase, comme dans vim ; en AZERTY, Maj+5 donne
  toujours 5 étoiles). En mode pile, un bouton « |‹ » près des flèches (Ctrl+Maj+S,
  `[gui.view] stack_enter`) choisit si ↑ / ↓ arrivent sur la couverture de la pile ou
  sur sa première photo (issue n° 62). L'emplacement du cache a une liste déroulante
  dans la fenêtre de réglages.
- **Builds de nuit** : leurs paquets portent la date et le commit dans leur nom
  (`tri-1.0.0-alpha.7-nightly-20261025-d16c32f-…`), plus celui de la version publiée.
- **Curseur des piles** : vers la droite (plus strict), il suit le rang des distances
  du dossier, donc chaque cran joint à peu près autant de photos et tout à droite il
  ne reste aucune pile ; il était sensible au pixel de ce côté et s'arrêtait avec des
  piles restantes (issue n° 61).
- **Paquets Linux** : un `.deb` (Debian, Ubuntu, Mint) et une AppImage (pour essayer
  sans rien installer), à côté de l'archive `.tar.gz`.
- **Déchargement** : les aperçus et analyses de chaque photo se calculent dès qu'elle est
  copiée, pendant la suite de la copie ; la séance est prête plus tôt. Case dans la boîte
  de déchargement, `[offload] previews` (oui par défaut) ; `--previews` en ligne de
  commande (issue n° 40).
- **Cache sur une clé USB** : le passage à l'heure d'hiver ne fait plus recalculer tout le
  cache d'un disque FAT32 (dates en heure locale, décalées d'une heure) ; lire le cache
  n'écrit plus sur le disque ; activer le filtre « bien exposées » pendant les analyses ne
  fige plus la fenêtre (issue n° 74). Le cache est recalculé une fois.
- **Avance automatique, mode pile** : option `[gui.view] stack_advance_next`
  (désactivée par défaut) pour passer, au bout d'une pile, à la première photo de la pile
  suivante.
- **Corbeille** : supprimer beaucoup d'images ne fige plus la fenêtre ; une barre de
  progression s'affiche dans la boîte (sous Linux, toute la corbeille était aussi relue
  pour chaque fichier ; issue n° 43).
- **Boîtes** : la même croix de fermeture en haut à droite de toutes les boîtes, au lieu
  d'un bouton « Fermer » en bas (issue n° 39).
- **Déchargement des RAW seulement** : la progression ne compte que les RAW, un par un
  (elle comptait aussi les JPEG laissés sur la carte et avançait de deux en deux ;
  issue n° 38).
- **Grille** : Ctrl / Cmd + molette, ou une pince au pavé tactile, change la taille des
  vignettes (issue n° 44).
- **Filtre « bien exposées »** : deux curseurs, « brûlé » et « bouché », à côté du
  soleil, comme celui du flou ; tout à gauche, ce côté n'exclut plus rien (photos sur
  fond noir, blancs brûlés exprès). Retenus par dossier ; les badges suivent
  (`[gui.histogram] strictness_span` ; issue n° 48).
- **Panneau des dossiers** : au survol, l'icône « décharger » d'un volume passe à la
  couleur d'accent et « éjecter » en rouge, pour ne plus les confondre (issue n° 41).
- **Avance auto** : l'étoile du bouton reste visible quand il est allumé (elle avait la
  couleur d'accent du fond ; issue n° 59).
- **Avance auto dans les piles** : en plein écran en mode pile, une note passe à la photo
  suivante de la pile, comme →, et non à la pile suivante (issue n° 60).
- **Programmes lancés** (éditeur, application par défaut, gestionnaire de fichiers) :
  leurs messages ne remplissent plus le terminal d'où `tri` a été lancé.
- **Cache avec les photos** : le cache peut être gardé à la racine d'un disque amovible
  (disque de transfert, retrouvé sur un autre ordinateur) ou dans un dossier (il le suit
  quand on le déplace) : clic droit dans le panneau des dossiers,
  `tri <dossier> --cache-here` / `--cache-user`, ou par défaut avec
  `[cache] location = "disk"` / `"folder"`. Déplacer ou renommer un dossier depuis `tri`
  emporte son cache. Le cache existant est recalculé une fois (nouveau format ;
  issue n° 55).

## 1.0.0-alpha.7 — 23/10/2026

Septième version de test : gros dossiers ouverts aussitôt, cache d'un dossier en ligne de commande, corrections XMP et réglages.

- **Cache d'un dossier** : `tri <dossier> --clear-cache [niveau] [--depth N]` vide
  seulement le cache des images de ce dossier (et de ses sous-dossiers jusqu'à
  `--depth`), pour les faire recalculer ; tous les niveaux acceptés (`embedded-full`,
  `developed` aussi). Le cache est désormais rangé par dossier de photos : le cache
  existant est recalculé une fois, l'ancien supprimé en fond.
- **Réglages** : pendant une recherche, un clic sur une section de la liste l'ouvre
  comme sans recherche (les préréglages d'export s'affichaient tous à la suite ;
  issue n° 24).
- **Versions** : « Original » est réservé à l'image non recadrée de la liste des
  versions ; une version au rapport de l'image s'appelle d'abord « Recadrage ».
- **Gros dossiers sur un disque lent ou un NAS** : la grille s'affiche aussitôt, les notes
  se lisent en fond avec leur progression dans la barre d'état (2 820 images sur un disque
  dur externe : 15 s de fenêtre vide auparavant) ; poser une note ne relit plus tous les
  sidecars du dossier ; les résultats du moteur ne figent plus les premières images
  (issues n° 15 et 29).
- **Ordre** : l'ordre retenu est aussi appliqué au lancement (issue n° 30).
- **XMP** : sidecars avec un `rdf:Description` niché (Lightroom, coordonnées du
  photographe) : les versions de recadrage s'écrivent de nouveau (annoncées enregistrées,
  elles ne l'étaient pas), et une note placée après le bloc niché est lue et remplacée
  (issue n° 27).
- **`tri <dossier> --previews`** : attend la fin des analyses de fond (netteté,
  empreintes) avant de se terminer ; certaines n'étaient pas mises en cache, et le
  tableau final du cache les oubliait.

## 1.0.0-alpha.6 — 22/10/2026

Sixième version de test : noms des versions et des préréglages, recherche des réglages plus rapide.

- **Versions** : plus deux versions d'une image du même nom (casse comprise) ; le nom
  proposé est numéroté, un nom déjà pris passe en rouge et ne s'enregistre pas
  (issue n° 14).
- **Réglages** : les préréglages d'export et de recadrage se renomment par un bouton
  « Renommer… » au lieu d'un champ du nom modifiable, et deux préréglages ne peuvent plus
  porter le même nom (issues n° 17 et 18).
- **Réglages** : le défilement d'une recherche (« export ») redevient fluide : les
  catalogues de traduction étaient relus pour chaque réglage à chaque image (issue n° 16).

## 1.0.0-alpha.5 — 22/10/2026

Cinquième version de test : les issues des testeurs, ordre des images, favoris nommés.

- **Ordre** : par nom, date de prise de vue ou de modification, croissant ou décroissant
  (liste et flèche dans la barre des filtres, `f s` / `f r`, retenu) ; les paires RAW+JPEG
  restent ensemble.
- **Favoris** : un favori peut être nommé (`n` ou clic droit ; chemin au survol), et se
  déplie sans déplier le même dossier dans l'arborescence, et inversement.
- **Plein écran** : numéro, source et taille dans la barre d'état au lieu de sur l'image ;
  en mode concentration, sur l'image, masqués par `Ctrl+I`.
- **Couleur de fond** : `Ctrl+L` fait tourner le fond de la grille et du plein écran entre
  celui du thème, gris foncé, noir et blanc (`[gui.colors] backgrounds`), sans doublon.
- **Panneaux** : nom du fichier en ligne des métadonnées ; un clic dans un panneau lui
  donne le clavier, un clic sur les images le leur rend ; liseré plus fin
  (`[gui.dialogs] focus_frame_width`).
- **Notes** : les bulles des étoiles, couleurs et drapeaux donnent leurs raccourcis.
- **Aide** : s'ouvre tout de suite à sa taille (elle grandissait sur plusieurs images
  la première fois), et la colonne des touches ne déborde plus de la fenêtre
  (issue n° 11). Ouverture fluide : préparée au démarrage, seules les lignes visibles
  dessinées (75 ms par image ramenées à 0,3 ms).
- **Plein écran** : badges brûlée, bouchée et floue dans le bandeau, comme dans la
  grille (clic : grand histogramme, carte de netteté ; issue n° 9).
- **Plein écran** : notes plus affichées deux fois ; sur l'image seulement quand le
  bandeau de notation est masqué (issue n° 8).
- **Filtre « bien exposées »** : la grille ne défilait plus (refiltrée sans fin, chaque
  fois ramenée à la sélection ; issue n° 10).
- **Plein écran** : les versions d'une image dans une liste sur la ligne des notes ;
  l'image ne change plus de taille d'une photo à l'autre (issue n° 5).
- **Boîte « Mise à jour »** : notes de version mises en forme (gras, listes, code) au
  lieu du Markdown brut (issue n° 3).
- **Aide** : version plus visible et copiable (avec le système), et un bouton
  « Signaler un problème » qui ouvre le formulaire de GitHub prérempli (issue n° 7), aussi
  `Ctrl+B b` (le leader, puis b).
- **Réglages** : une clé réglée dans votre fichier de configuration est en gras,
  maintenant dit dans la fenêtre et au survol (issue n° 6).
- **Notes de version dans la langue de l'interface** : la boîte « Mise à jour » montre
  les notes en français ou en anglais ; historique en anglais (`CHANGELOG.md`) et en
  français (`CHANGELOG.fr.md`), les deux dans la Release.
- **Recadrage** : une poignée approchée par l'intérieur se prend dès que le curseur
  change (elle déplaçait le cadre entier ; issue n° 1).

## 1.0.0-alpha.4 — 22/10/2026

Quatrième version de test, avant la bêta 1.

- **Rechercher une mise à jour** : bouton en haut de l'aide (F1) et `tri --check-update`,
  sur demande seulement (page Releases, lue par `curl`) ; la boîte donne les notes de la
  nouvelle version et ouvre sa page. Un build de nuit ou personnel le dit, et compte les
  commits de `main` qui lui manquent (« git pull, puis recompiler ») ; numéro de version
  avec le commit dans l'aide.
- **Recherche dans l'aide et les réglages** : thèmes et sections restent visibles
  (sections trouvées en tête, avec leur nombre de résultats), le titre d'une section la
  trouve en entière, mots-clés cachés (« vim » trouve le jeu de raccourcis), et les mots
  trouvés tels quels écartent les correspondances à lettres éparpillées.
- **Builds de nuit** : la CI construit et teste chaque nuit où `main` a changé, paquets
  sur la Release « nightly ».
- **Filtres** : icône « inverser » à la taille des pastilles de couleur.
- **Notes à la souris** : recliquer l'étoile posée remet à zéro, comme le filtre des
  étoiles ; le bouton « ⊘ » est retiré (au clavier, la touche 0 ; `1` deux fois reste
  à 1).

## 1.0.0-alpha.3 — 21/10/2026

Troisième version de test, la première avec **Windows** (`.msi` et `.zip` portable), à
côté des paquets macOS (`.dmg`) et Linux (`.tar.gz`), toujours sans HEIC.

- **Piles** : curseur de sévérité comme celui du flou ; au centre, le seuil calculé
  d'après le dossier (coupure entre séries et changements de scène), à droite plus
  strict (petites piles) ; retenu par dossier. `tri --groups` regroupe au centre calculé.
- **Curseurs** : double-clic = valeur par défaut, partout. Flou : à fond à gauche, toutes
  les images ; en plein écran, la grille n'est plus refiltrée avant le retour (l'image
  regardée pouvait changer).
- **Couleurs** : étoiles, couleur choisie et interrupteurs [RAW] / [JPG] dans la couleur
  d'accent du thème (fini le jaune et le grand carré), badge RAW des vignettes aussi ;
  filtre « sans les floues » : un triangle au lieu de la goutte barrée.
- **Barre d'état** : plus de barres de progression éclair pendant un réglage
  (`[gui] progress_delay_ms`).
- « JPEG boîtier » devient « Rendu boîtier ».
- **Plein écran** : résolution du développé darktable, son recadrage lu dans le sidecar.
- **Filtres** : « bien exposées » (soleil, `f w` : sans les brûlées ni les bouchées) et
  « inverser » (◐, `f i` : ce que les filtres retirent ; « floues seulement » devient
  « nettes » inversé, `f B` retiré).
- **Paires RAW+JPEG** : case « avec l'autre fichier de la paire » dans les boîtes Supprimer
  et Copier / déplacer (supprimer tous les JPEG emportait les RAW).
- **Recadrage** : préréglages sur `Tab` / `Maj+Tab` (`p` reste la carte de netteté).
- **Windows** : installateur `.msi` (menu Démarrer, désinstallation depuis les
  Paramètres) et zip portable, avec LibRaw et le runtime Visual C++ ; plus de fenêtre de
  console derrière l'interface.
- **Réglages** : descriptions en anglais dans la fenêtre de réglages
  (`doc/config.example.en.toml`), selon la langue de l'interface.
- **Grille** : taille des vignettes réglable (curseur en bas, `+` / `-`, `*` ou bouton
  « tout à l'écran »), infos des vignettes masquables (`Maj+T`) ; nom sans extension et
  format (RAW, JPEG, TIFF…) écrit à droite ; photo floue : contour de goutte ; notes sans
  rectangle sombre (étoiles en bas à gauche, drapeau en bas à droite, couleurs en haut à
  droite) ; écrêtage en triangle mi-blanc mi-noir en haut à gauche (clic : grand histogramme),
  versions dans la légende (clic : recadrer) ; effacer les filtres :
  entonnoir barré tout à droite de la barre des filtres.

## 1.0.0-alpha.2 — 21/10/2026

Deuxième version de test, la première publiée avec ses paquets sur la page Releases
(dépôt public). Paquets macOS (`.dmg`) et Linux (`.tar.gz`), sans HEIC ; Windows à venir.

- **Dossiers** : copier ou déplacer un dossier entier (`Ctrl+C` ou `y` / `Ctrl+X` dans
  le panneau des dossiers, clic droit, annulable) ; `y` copiait les images
  sélectionnées au lieu du dossier. **Glisser-déposer** d'un dossier ou d'images de la
  grille sur un dossier, toujours confirmé (Option sur macOS, Ctrl ailleurs : copier).
  Panneau rangé en sections titrées : disques amovibles, réseau (partages montés, GVFS
  sous Linux), dossiers personnels (Bureau, Documents, Téléchargements, Images, comme le
  Finder), favoris, puis le système. Le clic droit « Ajouter aux favoris » ouvrait la boîte de
  suppression : corrigé.
- **Flou** : curseur de sévérité dans la barre des filtres (au centre les réglages, à
  droite plus strict, à gauche plus souple), en direct, retenu par dossier ; le panneau
  des métadonnées donne aussi le rapport à la pile.
- **Plein écran** : résolution de ce qu'on regarde (une version : son cadre, réduit par
  son préréglage d'export).
- **Réglages** : le préréglage d'export d'un préréglage de recadrage se choisit dans une
  liste (plus de faute de frappe possible).
- **Netteté** : la carte (`p`) s'affiche aussi en vue ajustée et suit le curseur de
  sévérité (plus souple : plus de contours nets) ; en plein écran, le curseur dans le
  bandeau et `Ctrl+J` / `Ctrl+K`.
- **Plein écran** : résolution juste pendant le recadrage et pour le JPEG boîtier.
- **Documentation** bilingue (`doc/en/`, `doc/fr/`), `README.md` en anglais.
- **Barre des filtres** : deux interrupteurs, étiquettes RAW et JPG, à la place de « Tout /
  RAW / Plates » ; curseur du flou montré seulement avec un filtre sur la netteté, de la
  largeur de celui des piles.
- **Sous-dossiers** : case du panneau des dossiers, leurs images dans la grille aussi
  (3 niveaux par défaut), choix gardé.
- **Copier / déplacer vers…** : titre sans verbe et bouton « Valider » ; destinations
  récentes sans doublons.
- **Plein écran** : après un développement darktable (Maj+B), les autres images ne
  zoomaient plus (« pas encore développé ») ; une image sans développement se montre
  maintenant dans la source d'avant, avec son 1:1.
- **Piles** : le curseur de similarité ne déplie plus les piles pendant le réglage (on
  voit les piles obtenues) ; Maj tenue pour les voir dépliées.
- **Rapport d'export** (`Ctrl+Maj+E`, ou le bouton à côté du bilan d'un export) : les
  exports terminés, gardés d'une session à l'autre, avec un lien vers chaque dossier de
  destination ; effaçable.
- **Vocabulaire** : « préréglage d'export », comme « préréglage de recadrage »
  (« profil » reste aux profils couleur).
- **Réglages** : une seule entrée pour les préréglages d'export et une pour ceux de
  recadrage, avec une liste, « Nouveau… » et « Supprimer ».
- **Recadrage** : « Enregistrer comme préréglage » devient une petite disquette à côté
  de la liste des préréglages ; Enregistrer et Annuler à droite.
- **Boîte d'export** : les flèches ne changent le préréglage que sur sa liste ; Tab passe
  au dossier.

## 1.0.0-alpha.1 — 19/10/2026

Première version de test (dépôt privé ; la bêta sera publique). Paquets prêts à
installer pour macOS (`.dmg`) et Linux (`.tar.gz`), sans HEIC ; Windows à venir.

- **Dossiers** : panneau des dossiers (volumes, favoris, recherche `/`), surveillance du
  dossier ouvert, déchargement de cartes (copie vérifiée, doublons, éjection).
- **Grille** : vignettes en tâche de fond avec cache persistant, filtres (type, étoiles,
  couleurs, drapeau, netteté, nom), sélection multiple, piles d'images semblables
  (seuil réglable en direct), comparaison côte à côte.
- **Plein écran** : JPEG boîtier ou vrai rendu RAW (LibRaw), zoom continu jusqu'à 400 %,
  loupe, repère du collimateur (Fujifilm), carte de netteté, zones saturées / bouchées,
  superpositions (grilles, spirale d'or, texte, logo, format 4:5…).
- **Notes** : étoiles, couleurs, accepté / rejeté, en XMP (conventions darktable et
  Adobe, rien d'autre n'est touché), annulables.
- **Analyses** : histogramme RAW en IL, score de netteté jugé dans le dossier et dans la
  pile, regroupement par similarité.
- **Fichiers** : renommer, corbeille, copier / déplacer vers…, éditeur externe, AirDrop
  (macOS).
- **Développé dans darktable** : `Maj+B` (ou le bouton sous l'image) fait
  le rendu darktable-cli, en cache, rien d'écrit chez vous ; `b` le montre ensuite.
- **Export** : `Ctrl+E`, JPEG / TIFF / PNG par profils d'export (« Retouche » TIFF 16 bits
  ProPhoto, « Web » JPEG sRGB), développés par darktable (installé, en AppImage ou en
  Flatpak) ; sans darktable, rendu de base par LibRaw ; pas de ProPhoto en 8 bits.
- **Interface** : tout au clavier (jeux « classique » et « vim », raccourcis réglables),
  fenêtre de réglages, aide avec recherche, français et anglais, thèmes sombre, clair et
  gris neutre ; icône, barre du haut, mode concentration (F11).
