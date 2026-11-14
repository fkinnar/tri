# Historique

*[English version](CHANGELOG.md)*

## Non publié

- **Une seule liste « Rendu affiché » en plein écran** (#465) : la liste de la section
  Vue ne devient plus « Avant / après » sur un développement quand on passe d'une image
  à l'autre ; elle garde son titre et ses choix, dit le rendu à l'écran (JPEG boîtier,
  un logiciel de développement, LibRaw), et en choisir un montre ce rendu de la photo
  (son développement, le JPEG d'une paire regroupée, l'« avant » d'un développement) ou,
  si la photo n'en a pas, devient le rendu affiché de toutes les photos.
- **Paires regroupées : filtre à trois cases** (#464, #392) : dans un dossier regroupé,
  le filtre de type a trois cases, RAW (sans JPEG), RAW+JPEG (les paires) et JPEG (sans
  RAW), combinables, parcourues par la touche du type. Aussi : un développement darktable
  du JPEG rejoint lui aussi la pile du RAW, et la barre d'état compte les photos (les
  JPEG regroupés avec les développements).
- **Paires RAW+JPEG regroupées en une photo** (#463, première partie de #392) : dans un
  dossier regroupé, une paire n'est qu'une vignette, le JPEG du boîtier rangé avec les
  développements du RAW (puce « Boîtier », Maj+V pour déplier). Désactivé par défaut :
  l'ouverture d'un dossier qui a des paires pose la question une fois (« Ne plus
  demander » en fait le réglage global, `[files] group_pairs` = `ask` / `separate` /
  `grouped`) ; la décision est retenue par dossier et se change dans le menu du dossier
  ou la palette (`core.dev.pairs`).
- **CI : un test en échec est relancé une fois** (#461) : les tests passent par
  cargo-nextest sur GitHub ; un test qui ne passe qu'à la relance est marqué FLAKY (clics
  simulés perdus sur les machines de GitHub) au lieu de bloquer la PR ; un vrai bug
  échoue toujours deux fois.
- **Pas d'onglet « Base » sur un JPEG** (#458) : exposition et balance des réglages
  valent pour les RAW ; sur un JPEG (ou une autre image déjà développée), l'onglet est
  caché et « Style » s'ouvre à sa place, sauf si l'image a déjà une base, gardée pour la
  retirer.
- **Les coupes d'une version en morceaux se voient** (#460) : en plein écran, une
  version en morceaux a ses coupes tracées sur l'image, en tirets alternés clair et
  foncé, lisibles sur toute image (`[gui.crop] tile_dash`, `tile_line_width`,
  `tile_opacity`).
- **Versions en grille** (#459) : la partie « Morceaux » de la barre de la version est
  maintenant colonnes × lignes, pour une bande verticale (1 × 3) ou une grille (2 × 2,
  3 × 3) ; `g` / `Maj+G` une ligne de plus / de moins (`c` / `Maj+C` restent pour les
  colonnes) ; clé `rows` dans les préréglages de version.
- **La grille exporte ce qu'elle montre** (#457) : Ctrl+E en grille suit le rendu
  affiché des vignettes (JPEG boîtier, rendu LibRaw ou un logiciel de développement),
  réglages compris ; il passait par darktable dès qu'il était installé, sans les
  réglages.
- **Les réglages suivent à l'export** (#456) : exposition, balance des blancs,
  contraste, saturation et noir et blanc sont appliqués à l'export par LibRaw (rendu RAW,
  JPEG boîtier, images plates), l'exposition sur les données du capteur pour un RAW ;
  calculés en 16 bits sans borner, quel que soit l'espace couleur de l'export. Le « JPEG
  tel quel » reste une copie pure. Corrigé aussi : les espaces d'export « Linear Rec709 »
  et « Linear Rec2020 » gardaient la courbe sRGB en convertissant une image déjà
  développée.
- **Une version en morceaux, pour un carrousel** (#455) : une version peut être coupée
  en morceaux égaux côte à côte (liste « Morceaux » de la barre de la version, `c` /
  `Maj+C` un de plus / de moins), chacun au rapport choisi, les coupes tracées sur le
  cadre ; l'export écrit un fichier par morceau, numéroté (`DSCF0001-Pano_1.jpg`,
  `_2`…), chacun à la taille du préréglage d'export. Un préréglage « Carrousel » fourni :
  deux 4:5 pour un panorama Instagram. Clé `tiles` dans les préréglages de version.
- **Le badge des versions modifie** (#454) : un clic sur le badge d'une vignette rouvre
  le cadre de sa version, ou, s'il y en a plusieurs, ouvre l'image en plein écran avec la
  première montrée (la liste « Version » pour en choisir une, la modifier ou la
  supprimer) ; il commençait une nouvelle version. « Modifier » et « Supprimer » ont
  dans cette liste l'allure de ses autres éléments.
- **Résumé du dossier : versions et cache** (#453, partie de #388) : une ligne « Avec
  des versions » (comptée en fond, un filtre comme les autres lignes), et la place du
  dossier dans le cache avec un bouton « Vider » (avec confirmation). `f Échap` retire
  aussi les filtres du résumé.
- **La ligne de commande dans l'aide** (#452, corrige #441) : F1 liste les commandes à
  la suite des raccourcis, par thème, cherchées comme eux, avec leur forme ; un clic
  déplie un exemple rempli avec le dossier ouvert ou les images sélectionnées, et un
  bouton le copie. Dans la fenêtre de l'aide, une colonne des libellés plus
  étroite où les libellés longs passent à la ligne, un peu d'air entre les lignes et
  l'exemple dans un cadre (`[gui.dialogs] help_width`, `help_label_width`,
  `help_row_padding`, `help_example_margin`). Un court paragraphe dans le
  manuel.
- **`tri --help`** (#451, partie de #441) : chaque commande de la ligne de commande
  expliquée par thème, avec ses options et un exemple (`-h` aussi) ; le mode d'emploi
  court affiché sur une erreur vient de la même description.
- **Noter en ligne de commande** (#450, corrige #275) : `tri <image>... --stars N
  --color red,…|+blue,-red|none --flag accept|reject|none [--dry-run]`, ou la liste des
  images sur l'entrée standard ; couleurs remplacées, ou ajoutées et retirées en gardant
  les autres ; la paire RAW+JPEG et les développements principaux suivent, sidecars
  comme dans l'interface.
- **Barre des filtres : la comparaison des étoiles comme les autres boutons** (#449) :
  « ≥ / = / ≤ » est dessiné comme le ∨ / ∧ du filtre de couleurs, à sa taille, cadre gris
  au survol, et l'écart habituel après « FILTRE ».
- **Filtre de couleurs : toutes les couleurs ou l'une d'elles** (#444) : avec plusieurs
  couleurs par photo (`[xmp] multiple_colors`), un bouton ∨ / ∧ devant les
  pastilles de la barre des filtres demande les photos qui portent toutes les couleurs
  choisies plutôt que l'une d'elles (action `core.filter.colors.all`, sans raccourci par
  défaut).
- **Le résumé du dossier filtre** (#443, partie de #388) : un clic sur une ligne de « Ce
  dossier » montre ce filtre seul (une extension, les développements d'un logiciel, sans
  note, avec des étoiles, acceptées, rejetées, avec un libellé couleur ou une couleur,
  en conflit), et la ligne s'allume, du fond du dossier ouvert ; un
  second clic, ou « Photos », retire tous les filtres.
- **Résumé du dossier ouvert** (#442, partie de #388) : une section « Ce dossier » en bas
  du panneau des dossiers, repliée par défaut, compte les photos par type, les paires
  RAW+JPEG, les développements par logiciel (grisés quand le logiciel est absent ou
  désactivé), l'avancement du tri (sans note, avec des étoiles, acceptées, rejetées), les
  notes en conflit, et dit où vivent les données de tri. Cliquer une ligne pour filtrer
  viendra ensuite.
- **Le clic droit sur une sélection agit sur toute la sélection** (#440) : notes et
  actions sur les fichiers de la boîte du clic droit portent sur toutes les images
  sélectionnées, comme au clavier (une case revient à l'image du clic droit seule ;
  renommer reste une image à la fois, #439). Le bandeau de notation dit désormais qu'il
  « agit sur les N images sélectionnées », pas seulement sur leurs notes.
- **Un déchargement vers une destination pleine s'arrête et demande** (#438) : il
  échouait sans rien dire, chaque fichier suivant en erreur. La copie s'arrête désormais
  sur le fichier qui n'a pas tenu, et la boîte revient (même depuis l'arrière-plan) avec
  ce qui est fait, ce qui reste et la place libre, jusqu'à « Réessayer » (après avoir fait
  de la place) ou « Abandonner » (le reste demeure sur la carte, rien n'est supprimé ni
  éjecté). Avant la copie, la boîte prévient si la place ne suffit pas ; en ligne de
  commande, un message et un code de sortie d'erreur.
- **Déchargement : la séance datée par sa plus récente image, et ce qui a déjà été
  déchargé ignoré** (#437) : le dossier proposé prend la date de la plus récente image à
  copier (il prenait la plus ancienne) ; ce qu'un déchargement précédent a copié
  n'importe où est désormais laissé de côté par défaut (`[offload] skip_known = true`,
  d'après l'historique des déchargements), la boîte dit combien d'images sont déjà
  déchargées et combien sont nouvelles, et une case « Décharger à nouveau les n images
  déjà déchargées » les copie quand même (pour décharger une carte une seconde fois
  ailleurs).
- **Un dossier connu reste rapide quand son cache est en partie refait** (#436) : ce qui
  est encore en cache (empreintes de regroupement, netteté, exposition, un 1:1) est livré
  tout de suite au lieu d'attendre derrière les vignettes recalculées ; les piles se
  forment d'emblée après un changement des clés du cache (comme après la gestion des
  couleurs).
- **Un logiciel de développement tout juste écrit se lance quand même** (#435) : sous
  Linux, un outil refusé un instant avec « Text file busy » est réessayé quelques fois
  (155 ms au plus) ; ça faisait échouer la CI de temps en temps.
- **Un seul réglage pour l'histogramme après réglages** (#434) : la case « Histogramme
  après réglages », le bouton du grand histogramme et `e` basculent désormais le même
  réglage et se reflètent ; la case cochée, le bouton ne ramenait plus les données
  réelles.
- **Images développées plus ternes dans la grille** (#433) : les copies de la grille
  des développements portent le profil de travail, mais il changeait d'un lancement à
  l'autre (date de création) et était alors pris pour du sRGB : les copies étaient
  converties une seconde fois ; le plein écran était juste. Le profil est désormais
  toujours le même, et les copies plus anciennes sont reconnues.
- **Plus d'éclair d'un autre rendu entre deux images développées** (#432) : en passant
  d'une image développée à la suivante en plein écran, le JPEG boîtier (ou le rendu RAW),
  chargé plus tôt, s'affichait un instant entre la vignette et le développement ; le
  plein écran attend désormais sur la vignette, qui montre déjà le développement.
- **Vérifier les couleurs** (#430) : une fiche pour les testeurs
  ([doc/fr/colour-check.md](doc/fr/colour-check.md)) et les mires de couleur dans
  `doc/charts`, pour vérifier en dix minutes que `tri` affiche juste sur un écran calibré
  ou large gamut, sans rien compiler.
- **Réglages appliqués tout de suite** (#429) : `[adjust]` (exposition et balance
  automatiques), `[sidecars]` et `[watch]` changés dans la fenêtre de réglages
  s'appliquent immédiatement ; ils attendaient le prochain lancement.
- **Couvertures des piles justes dès l'ouverture** (#426) : tant que les notes sont en
  lecture, une pile montre sa couverture habituelle même si la liste a un peu changé
  depuis la dernière fois (tri ou filtre par notes, curseur bougé) ; elle montrait sa
  première image.
- **Histogramme après réglages** (#427) : un bouton du grand histogramme (ou `e`) montre
  l'histogramme, les pourcentages d'écrêtage et les zones de l'image telle qu'on la voit,
  réglages de visualisation compris ; la part saturée du capteur reste affichée. Pendant
  le glissement d'un curseur, l'histogramme suit tout de suite. Une case en tête de la
  section Réglages y passe d'office pour les images réglées ; un réglage fait suivre les
  badges de la grille. L'histogramme est désormais fixé en haut du panneau de droite et
  ne se replie plus ; `[gui] one_section_open` ne laisse qu'une section dépliée par
  panneau.
- **Mires de couleur** (#425) : `cargo run --release --example colour_charts -- <dossier>`
  écrit la même mire dans tous les espaces courants (sRGB, Adobe RGB, Display P3,
  ProPhoto, Rec.2020, linéaires), en JPEG, TIFF et PNG avec leur profil, et vérifie que
  tri les lit toutes pareil. Le profil Display P3 donné aux HEIC d'iPhone ne porte plus
  une étiquette fausse (le P3 du cinéma).
- **Gestion des couleurs** (#423) : chaque image est affichée avec son vrai espace
  couleur au lieu d'être supposée en sRGB : JPEG boîtier (Adobe RGB aussi), images plates
  avec leur profil ICC (TIFF et PNG 16 bits convertis à leur profondeur), HEIC, et
  développements avec le profil de sortie de leur historique (ProPhoto… ; tri n'en impose
  jamais). Elles passent par un espace de travail large (primaires Rec.2020, courbe sRGB),
  converties une seule fois, à la mise en cache. Sous Linux et Windows, les images sont
  converties vers le profil de l'écran (`[gui] display_profile`, sinon celui de colord ou
  de Windows) ; macOS s'en charge lui-même. Les rendus RAW sont maintenant lus avec leur
  vraie courbe : un peu plus clairs qu'avant, surtout dans les ombres. Vignettes et aperçus
  sont recalculés une fois.
- **Les couleurs suivent le profil de l'écran sur macOS** (#421) : tri déclare
  maintenant sa fenêtre en sRGB au système, qui convertit vers le profil de l'écran
  (calibration comprise). Avant, les couleurs partaient sans conversion : largement trop
  saturées sur un écran wide gamut.
- **Grille plus rapide au lancement** (#419) : les vignettes en rendu RAW ou développé
  gardent une petite copie dans le cache la première fois qu'elles sont réduites ; les
  lancements suivants la lisent au lieu de tout l'aperçu, et la grille ne se remplit plus
  une à une.
- **Les vignettes des développements ne changent plus d'image à l'ouverture** (#418) :
  une vignette dont on ne connaît pas encore le rendu développé (son sidecar en lecture)
  reste vide un instant (`[gui.grid] render_wait_ms`) au lieu de montrer le JPEG boîtier
  puis de changer ; les couvertures des piles aussi. De même pour une vignette dont les
  réglages de visualisation sont encore en lecture : montrée sans eux, toutes les vignettes
  réglées étaient ensuite redécodées d'un coup (une demi-seconde d'interface figée en
  ouvrant un dossier réglé), et une vignette n'est plus refaite quand ses réglages sont
  relus identiques. Les piles ouvertes repliées (#417) gardent aussi leurs couvertures :
  montrées d'abord avec leur première image, elles passaient à la vraie couverture une
  fois notes et scores de netteté lus ; les couvertures sont désormais retenues aussi.
  Et la grille attend les notes du dossier : triée par couleur, sa première image montrait
  l'ordre des noms sans marques, puis se réordonnait. Les notes sont lues avec la liste
  du dossier, dans le délai d'attente de l'ouverture (`[gui.folders] open_wait_ms`) :
  l'ancien dossier reste affiché jusqu'à ce que le nouveau arrive complet.
- **Les dossiers en piles s'ouvrent déjà repliés** (#417) : les piles d'un dossier sont
  retenues dans son cache après un regroupement complet ; rouvert sans rien de changé, il
  s'affiche replié tout de suite au lieu de se replier sous les yeux. Sinon la grille
  attend ses piles un moment (`[gui.grid] stacks_wait_ms`), avec une boîte
  « Regroupement des piles… » sur les grands dossiers (`[gui.grid] stacks_overlay_from`).
- **Plus de choses retenues par dossier** (#415) : l'image où l'on en était (on y revient
  en rouvrant le dossier), les piles dépliées, la case « Sous-dossiers » (`[gui.folders]
  subfolders` pour un dossier jamais réglé ; plus commune à tous les dossiers), l'écart de
  temps des piles, et les logiciels cochés du filtre « développées ». Un dossier venu d'un
  autre ordinateur garde le rendu et les logiciels choisis là-bas, même absents ici.
- **« Empiler automatiquement » retenu par dossier** (#413) : comme les filtres, le tri
  et le rendu des vignettes ; un dossier jamais réglé prend la valeur de la configuration.
- **Réinitialiser un dossier** (#412) : clic droit sur un dossier, Préférences,
  « Réinitialiser ce dossier… » (« ce disque… ») : une boîte avec une case par chose à
  effacer, chacune avec ce qu'elle concerne, mesuré en fond : cache, données de tri
  (filtres, curseurs, couvertures…), notes et versions. Les notes sont remises à zéro
  dans les sidecars, jamais supprimés : seuls étoiles, couleurs et drapeaux changent,
  les sidecars de darktable ou de Lightroom gardent le reste. Les notes demandent
  « Actions sur les fichiers ». Notes et versions s'annulent, en un pas. « Vider le
  cache… » ouvre la même boîte, le cache seul coché, désormais sur tout dossier. En ligne
  de commande : `tri <dossier> --reset cache,data,notes,versions [--dry-run]`.
- **Tout sur le disque** (#410) : « Garder les données de tri dans ce dossier / sur ce
  disque » (clic droit sur un dossier ou un disque) garde désormais, en plus du cache,
  tout ce que `tri` retient des dossiers en dessous : filtres et tri, valeurs des
  curseurs, couvertures de piles, développements préférés, mémoire des notes, dans un
  dossier caché `.tri`, par nom de fichier seulement. Un disque emporté sur un autre
  ordinateur (ou un dossier synchronisé) garde tout. Désactivé par défaut : rien n'est
  alors écrit à côté des photos (avant, `.tri/notes.json` l'était). La boîte de
  déchargement le propose, coché quand la destination est un disque amovible. Déplacé à
  la première ouverture : l'ancien `.tri-cache` devient `.tri/cache`, et ce que le
  dossier de données gardait par dossier rejoint les fichiers du dossier. Pour
  synchroniser un dossier sans son cache, exclure `.tri/cache`.
- **Boîtes au centre** (#409) : les confirmations et boîtes s'ouvrent au centre de la
  fenêtre d'après leur taille réelle (un décalage fixe les mettait de travers), la boîte
  de notation du clic droit comprise.
- **Vider le cache gardé avec les photos** (#409) : sur un dossier ou un disque qui garde
  son cache (`.tri-cache`), clic droit, Préférences, « Vider le cache de ce dossier… »
  (ou « de ce disque »), confirmé par une boîte avec sa taille ; le cache reste gardé là.
  Une action, à laquelle on peut donner un raccourci (`core.folders.cache_clear`).
- **Vider le cache depuis l'interface** (#408) : en tête de la section « Cache » de la
  fenêtre de réglages, la place des vignettes, aperçus boîtier, aperçus RAW, 1:1,
  développés et analyses, et le total ; « Vider » par ligne et « Tout vider », confirmés,
  en fond, l'espace libéré annoncé. Par exemple, vider les 1:1 sans perdre les vignettes,
  ou les développés après une mise à jour de darktable.
- **Menu du clic droit sur un dossier rangé** (#407) : en groupes, Accès (afficher dans
  le gestionnaire de fichiers, nommé : Finder, Explorateur, Nemo, Dolphin… ; ouvrir dans
  darktable, favori), Dossier, Notes, Développement et
  Préférences ; décharger reste en bas. Chaque groupe est un sous-menu
  que le petit bouton en haut à droite installe au premier niveau, entre deux traits (et
  l'icône sur son titre le replie) : chacun compose son menu, retenu pour tous les
  dossiers (`[gui.folders] menu_inline`).
- **Réglages de visualisation** (#404) : exposition, balance des blancs et noir et blanc
  (mélangeur de canaux), pour comparer et trier une série. Une nouvelle section
  « Réglages » du panneau de droite, en onglets : Base (exposition avec « Auto », à priorité aux hautes lumières ; balance
  en kelvins et teinte, préréglages telle que prise, auto sur les pixels presque neutres, lumière du jour, nuageux,
  ombre, tungstène, fluorescent et flash ; pipette : clic sur une zone grise ou cadre
  tracé pour une balance auto sur cette partie), Style (décalage créatif : exposition, chaleur,
  teinte, contraste, saturation), N&B (filtres en
  préréglages, mélange gardé à 100 % quand il est normalisé) et Valeurs (le tableau à
  reporter dans un logiciel de développement) ; un double-clic sur un onglet le remet à
  zéro ; le clic droit sur le dossier ouvert règle l'exposition, la balance ou les deux
  automatiquement sur toutes ses images, en tâche de fond. Plusieurs images sélectionnées :
  « Synchroniser N images » applique chaque changement à toutes (un curseur décale chaque
  image d'autant, la température du même rapport ; un bouton, auto ou préréglage,
  recalcule chacune ; un seul pas à annuler). Aussi copier / coller les réglages
  (exposition et balance harmonisées sur l'image copiée), « Harmoniser la sélection »,
  « Avant » pour comparer. Enregistrés dans le sidecar sous `tri:` ; montrés sur les
  vignettes et les grandes images. Tous les curseurs de l'interface avancent désormais
  par grands crans, et finement avec Maj (`[gui] slider_fine_speed`) ; les deux panneaux
  latéraux ont une largeur minimale (280 points) que le bord tiré respecte.
  `tri --adjust <référence> <images>… --to <dossier>` règle la référence et applique ses
  réglages aux autres : exposition et balance harmonisées sur la référence (une photo à
  -1 IL reçoit environ +1 IL), décalage créatif et noir et blanc copiés. Écrit les JPEG
  réglés et affiche les valeurs à reporter (`--ev auto`, `--kelvin`). Sur un RAW,
  l'exposition est appliquée aux données du capteur. Nouvelle section `[adjust]` : cible
  de l'exposition automatique, kelvins des préréglages (#137).
- **Journal de l'application** (#403) : une erreur garde un message court dans la barre
  d'état, et tout son détail va dans `tri.log`, au dossier de données : pour un logiciel
  de développement en échec, l'image, la commande, le code de sortie et tous ses
  messages ; aussi les exports, vignettes et aperçus ratés, une configuration invalide,
  les plantages (`crash.log` y est repris). Un clic sur une erreur ouvre le journal sur
  son détail. La fenêtre du journal (palette, ou bouton « Journal » de l'aide) montre les
  entrées les plus récentes d'abord, filtre par gravité, cherche, copie et affiche le
  fichier. « Signaler un problème » propose les dernières lignes, à relire et modifier
  avant de les joindre au formulaire. Nouvelle section `[log]` (taille, fichiers gardés,
  trace détaillée) et `[gui] report_log_entries` (#398).
- **Fin d'un export au milieu de l'écran** (#399) : le bilan (« 3 images exportées », le
  dossier) s'affiche aussi dans une bulle sur les images, avec « Ouvrir le dossier »
  (« Voir le rapport » quand les fichiers sont partis dans plusieurs dossiers). Il
  s'efface après quelques secondes (`[gui.grid] export_toast_ms`), attend tant que le
  pointeur est dessus ; une erreur reste jusqu'à ce qu'on la ferme, même quand d'autres
  exports finissent après (#381).
- **Ctrl+Maj+clic dans les lignes EXIF** (#401) : Ctrl+Maj+clic (Cmd+Maj+clic) ajoute la
  plage depuis la dernière ligne cliquée à la sélection, en gardant ce qui était
  sélectionné ; Maj+clic seul la remplace toujours. Un Ctrl+clic compte maintenant aussi
  comme dernière ligne cliquée, comme dans le Finder et l'Explorateur (#390).
- **Recherche dans l'aide `?`** (#402) : l'aide `?` ouverte, taper filtre la liste au fil
  de la frappe, avec la même recherche floue que F1 et la palette (mots des deux langues,
  fautes de frappe pardonnées) ; les lettres trouvées sont surlignées. Échap efface la
  recherche, un second Échap ferme ; `?` ferme toujours (#382).
- **Filtre « Développées » après avoir désactivé les logiciels** (#397) : filtre allumé,
  désactiver tous les logiciels de développement dans les réglages le laissait visible et
  actif jusqu'à ce qu'on l'éteigne à la main. Un logiciel désactivé sort maintenant de la
  liste du filtre (réactivé, il revient coché) ; quand plus aucun des logiciels du filtre
  n'est actif, le filtre s'éteint de lui-même et le dit dans la barre d'état (#395). Avec
  un seul logiciel restant, plus de ligne vide sous la case du filtre.
- **Cercle du drapeau vide sur les vignettes** (#396) : sans note, le cercle gris du
  drapeau (en bas à droite) avait un trait plus épais que celui de la couleur (en haut à
  droite) ; il reprenait l'icône du neutre des contrôles de notation, dont le trait grandit
  avec la taille. Les deux cercles sont maintenant dessinés pareil (#394).

## 1.0.0-beta.1 — 09/11/2026

Première bêta : notes accordées avec darktable et inscrites dans les images (notes dans
les images, fusion et conflits, listes de couleurs), palette de commandes et aide, zoom
et loupe au clavier, comparaison jusqu'à 21 images, versions, révision des raccourcis,
et les corrections des tests de bout en bout sur macOS, Linux et Windows.

- **Compteur d'images de la barre des filtres** (#386) : « 570/571 » pour 570 photos ; un
  sidecar RapidRAW (`.rrdata`) comptait dans le total du dossier alors que son
  développement était masqué, RapidRAW étant absent ou désactivé. Le total laisse
  maintenant de côté les développements de ces logiciels, comme la grille.
- **Plusieurs lignes EXIF** (#386, corrige #384) : Maj+clic, Ctrl+clic ou Cmd+clic pour
  ajouter ou retirer une ligne, Ctrl+A et une icône à côté du titre « EXIF » pour tout sélectionner ; la
  sélection est une bande grise sur la ligne au lieu de cadres bleus. Le clic droit sur
  une ligne copie sa valeur, la sélection ou tout.
- **Survol dans les panneaux latéraux** (#386, corrige #383 et #385) : la ligne sous la
  souris (dossiers, EXIF) prend la même bande gris clair que le curseur du clavier, sur
  toute la largeur, et devient la ligne pointée ; le dossier ouvert garde son fond
  d'accent, à part. Plus de cadre blanc d'egui au survol des contrôles, partout, comme
  les icônes des barres. Le raccourci de copie des métadonnées (`y`, `Ctrl+C` /
  `Cmd+C`) copie la ligne EXIF sous la souris, sans clic. Le pointeur est une main sur
  tout ce qui se clique dans les barres et les panneaux (boutons, cases et leur libellé,
  listes, lignes des dossiers), comme dans les boîtes, au lieu d'un mélange de main, de flèche et de
  curseur de texte ; plus de curseur de texte sur les textes des barres non plus.
- **Sortie d'une comparaison ouverte du plein écran** (#379, corrige #376) : elle
  ramenait à la grille ; elle ramène maintenant au plein écran, sur l'image active
  (bouton « Retour au plein écran »). Ouverte de la grille, elle ramène toujours à la
  grille.
- **Icônes des notes plus parlantes** (#374) : « Inscrire les notes dans les images » et
  « Relire les notes depuis les images » ont le même dessin (une photo et son étoile)
  avec une flèche qui entre ou qui sort, pour qu'on voie que l'une est l'inverse de
  l'autre ; « Générer les sidecars » montre une feuille avec une étoile et un « + ».
- **Afficher un dossier dans le gestionnaire de fichiers** (#375) : clic droit sur un
  dossier, « Afficher dans le Finder » (l'Explorateur, le gestionnaire de fichiers), comme
  pour une image.
- **darktable voit les couleurs posées dans tri** (#373) : la liste
  `darktable:colorlabels` était écrite après l'historique de darktable, qui l'ignorait
  alors en resynchronisant les XMP modifiés au démarrage. Elle va maintenant avant
  l'historique, là où darktable la met. Les sidecars déjà écrits la déplacent au prochain
  changement de couleur.
- **Raccourcis cités à jour** (#372) : les descriptions des réglages et quelques messages
  (aide-mémoire du plein écran, « Avant » d'un développement, préréglages de version…)
  citaient des raccourcis écrits en dur, parfois périmés (« Maj+B » pour voir ses
  développements, « b » pour le rendu). Ils citent maintenant l'action, et l'interface
  montre les raccourcis en vigueur, `[keys]` compris ; un test empêche d'en réécrire en
  dur. Une action sans raccourci n'a plus de « () » vide dans sa bulle.
- **Sidecars des deux conventions** (#371) : `[xmp] sidecar = "both"` crée et tient à
  jour `IMG.RAF.xmp` (darktable) et `IMG.xmp` (Lightroom, Capture One). Deux sidecars XMP
  d'une même photo sont désormais lus tous les deux et fusionnés (avant : le plus récent
  seul). « Générer les sidecars » crée ceux que la convention attend (réglé sur Adobe, un
  dossier de sidecars darktable reçoit ses `IMG.xmp`). Liste déroulante en clair :
  « Darktable », « Adobe », « Les deux », avec le nom du fichier en gris. La boîte des
  notes en désaccord montre la date de modification de chaque source (un indice : un
  sidecar de 2012 face à un de 2026).
- **Couleurs : une ou plusieurs, et champs par logiciel** (#371) : en mode une couleur,
  `xmp:Label`, la liste de darktable et digiKam sont écrits ; quand `Label` et la couleur
  de darktable ou de digiKam diffèrent (darktable ne touche jamais `Label`, qui pouvait
  masquer une couleur changée chez lui), l'ordre de lecture `[xmp] color_sources` décide
  (darktable, Label, digiKam par défaut ; à glisser dans la fenêtre de réglages). En mode plusieurs couleurs, comme darktable, seule sa liste est lue et écrite.
  `[xmp.fields]` (un tableau dans la fenêtre de réglages) : cases par logiciel (Label, darktable, digiKam, Lightroom), toutes
  cochées ; un champ décoché n'est ni lu ni écrit, il reste tel quel et sort de l'ordre de
  lecture. Liste déroulante pour `[xmp] embed` dans la
  fenêtre de réglages.
- **Notes tenues à jour d'elles-mêmes** (#368) : avec `[xmp] embed` = `internal`
  (disques internes et fixés) ou `everywhere`, chaque note va aussi dans l'image (JPEG,
  PNG, WebP, TIFF, HEIC), et une note fusionnée depuis un autre logiciel est récrite
  aussitôt dans les autres sources (une couleur de XnView arrive dans le XMP de
  darktable). Un dossier peut avoir son propre réglage, prioritaire (clic droit sur le
  dossier, « Notes dans les images » : selon les réglages, toujours, jamais), gardé dans
  son dossier `.tri` et signalé dans le panneau des dossiers ; il ne vaut pas pour les
  sous-dossiers.
- **Notes changées dans plusieurs logiciels fusionnées** (#368) : quand darktable change
  les étoiles et RapidRAW la couleur (ou XnView l'image elle-même), les deux sont
  gardés, champ par champ, au lieu que le sidecar le plus récent l'emporte, et la barre
  d'état le dit. `tri` retient ce que disait chaque source la dernière fois
  (`.tri/notes.json` à côté des photos, dans le dossier de données de `tri` pour un
  dossier en lecture seule). Deux sources qui changent le même champ différemment, ou
  des sources jamais vues ensemble en désaccord : un signe orange sur la vignette, et
  une boîte pour choisir les notes à garder (aussi pour tous les conflits du dossier
  d'un coup) ; `tri <dossier> --conflicts` les liste.
- **Notes dans les images elles-mêmes** (#276) : JPEG, PNG, WebP, TIFF et HEIC peuvent
  porter leurs notes (XMP embarqué, sans recompression, rien d'autre perdu), pour les
  retrouver dans l'Explorateur Windows, Spotlight, Lightroom, XnView… Lues quand une
  image n'a pas de sidecar (étoiles posées ailleurs). Deux actions, sur la sélection ou
  depuis le menu du dossier : « Inscrire les notes dans les images » et « Relire les
  notes depuis les images » (changées dans un autre logiciel ; annulable) ; `tri
  <dossier> --embed-notes` / `--import-notes`. Jamais dans un RAW.
- **Plus de plantage quand la mémoire graphique est pleine** (21 images comparées à
  1:1) : les grandes textures ont un budget (`[gui.view] textures_mb`, 1500 Mo) ; au-delà,
  une image garde son aperçu. Une erreur de la carte graphique est maintenant consignée et
  dite au lieu d'arrêter `tri` ; la texture en cause est libérée et le budget de la
  session ramené à ce qui tenait. En comparaison zoomée, une telle image est agrandie à la
  même échelle que les autres, avec un panneau d'avertissement. Une ligne dans le
  terminal, le détail dans `crash.log`. Un bouton « Garder N Mo » écrit la limite
  abaissée dans la configuration.
- **Un développement zoomé à l'échelle de sa photo** : à 100 %, un développement
  darktable ou RapidRAW (rendu à la taille de l'aperçu) apparaissait ~2,4 fois plus petit
  que la photo à côté ; il correspond maintenant au 100 % de la photo. Une icône d'info
  dit qu'il est agrandi, donc que sa netteté à cette échelle n'est pas celle du fichier.
- **Sortir d'une comparaison de tout** rend la grille telle qu'elle était : rien en
  surbrillance, la comparaison suivante reprend donc tout.
- **« Comparaison limitée à N images »** : une bulle, placée et minutée comme le repère
  de position (`[gui.compare] limit_seconds`, `limit_place`), au lieu d'une ligne de la
  barre d'état.
- **Recadrage et version ne font qu'un : « Version »** : une version est un cadre avec
  un préréglage d'export facultatif. L'interface dit « Version » partout ; `v` en crée
  une (au lieu de `c`), `Maj+V` / `Ctrl+Maj+V` passent à la suivante / précédente ;
  « recadrer » la trouve toujours dans l'aide et la palette. Les identifiants des
  actions deviennent `core.version.*` ; les anciens (`core.crop.*`, `core.view.crop`…)
  marchent encore dans `[keys]`. La barre des versions est rangée en parties titrées
  (Version, Recadrage avec les rapports dans une liste, Export, Nom) ; un rapport à soi
  peut s'y taper (`17:23`, `1.91:1`) et s'ajouter à la liste avec « + » (gardé dans
  `[versions] ratios`). Chaque partie a ses touches (`v` préréglage de version, `r`
  rapport, `x` orientation, `o` superpositions sur le cadre, `/` personnalisé, `e`
  export, `n` nom, `Ctrl+S` préréglage, `0` ou Ctrl+clic sur « Original » : retour au
  cadre d'origine) ; Échap ramène à la grille si la version y a été commencée ;
  l'orientation est une icône (le rectangle du cadre, debout ou couché). La barre des
  filtres, et les filtres, la vue et l'histogramme du panneau de droite, disparaissent
  pendant l'édition.
- **Gros bug : les développements RawTherapee et ART étaient exportés sans leur
  développement** (comme leur photo, un simple rendu LibRaw). Ils sont maintenant rendus
  par RawTherapee / ART, en pleine résolution (TIFF 16 bits dans l'espace couleur de leur
  profil), puis convertis, recadrés, réduits et accentués comme les autres exports.
  Interface et ligne de commande.
- **Retour à 100 % et à l'ajusté au clavier** : `<leader> m m` remet la loupe à 100 %,
  `<leader> z z` le zoom à 100 %, `<leader> z /` l'ajuste. Dans le menu des suites, `m`
  et `z` sont des sous-menus qui listent ce qui peut suivre.
- **Comparaison depuis la barre du bas** : un bouton (allumé pendant la comparaison, où
  il la quitte), et le retour à la grille aussi en comparaison. Sans sélection ni pile,
  tout ce qui est montré est comparé (au plus `[gui.compare] max`, autour de l'image
  courante) ; sans rien en surbrillance dans la grille, `=` le fait du premier coup.
- **Cmd / Ctrl+clic prend aussi l'image courante** : sans sélection, l'image en
  surbrillance compte comme choisie ; le premier Cmd / Ctrl+clic sur une autre
  sélectionne les deux. Avec une sélection et le curseur de la grille masqué, `=` (et
  les notes, les actions sur fichiers) agit aussitôt au lieu de seulement remontrer le
  curseur.
- **Un seul réglage de zoom dans la barre du bas** : les réglages du zoom et de la loupe
  sont fusionnés. Il règle le zoom (loupe ronde), ou la loupe tant que sa touche est
  tenue (loupe carrée). Un clic sur l'icône réinitialise, un clic sur la valeur permet de
  la taper. Le pointeur reste visible pendant qu'on tient la touche de loupe, la loupe ne
  s'affiche plus sur la barre, et l'interrupteur des infos sur l'image passe tout à
  droite. `TRI_GUI_TRACE` montre aussi
  les touches reçues.
- **Zoom et loupe tapés sous Linux** : après `<leader> m` ou `<leader> z`, les chiffres
  de la rangée du haut d'un clavier AZERTY (avec Maj) changeaient la note au lieu de la
  valeur : Linux envoie aussi Maj comme une touche, ce qui fermait la saisie. Un
  modificateur seul est maintenant ignoré.
- **Réglages du zoom en pourcentages** : `[gui.zoom] steps`, `max`, `min` et
  `loupe_scale` s'écrivent en pourcentages (`max = 400`), et les valeurs absurdes sont
  refusées (hors de 1 à 999 %, `max` sous 100 %, `min` au-dessus). Un fichier avec les
  anciennes valeurs (`max = 4.0`) est signalé comme invalide. La valeur de la barre du
  bas garde une largeur fixe. Dans la fenêtre de réglages, un nombre tapé est vérifié à
  Entrée ou en quittant le champ (pas à chaque chiffre), et l'erreur montre le message
  seul.
- **Comparaison** : l'image active ne revient plus à la couverture de sa pile quand
  les piles sont recalculées pendant la comparaison (arrivée d'une vignette, machines
  plus lentes).
- **Contrôles dans la palette de commandes** : sous une action qui a un contrôle dans le
  panneau de droite (une case, une liste, un curseur), la palette montre ce même
  contrôle, avec son état, pour le changer sans la fermer. Panneau et palette le
  dessinent avec le même code. Au clavier : ↓ va au contrôle, ← / → le changent,
  Entrée l'active. La recherche trouve les mots du panneau (« nettes ») et met les
  actions avant les réglages. La taille des vignettes a sa propre ligne.
- **Un rendu affiché précis depuis la palette** : une action par rendu (JPEG boîtier,
  couverture, chaque logiciel de développement, rendu RAW), sans passer par les autres.
  Pas de touche par défaut ; `[keys]` peut leur en donner une.
- **Un seul style pour les listes de raccourcis** (F1, `?`, le menu des suites, la
  palette) : touches à la couleur d'accent, une ligne sur deux sur fond plus clair, et
  le « / » entre plusieurs touches en gris, pour faire ressortir les touches.
  Dans le menu des suites, le début commun à la plupart des lignes passe dans le titre
  (`f` : « Filtre ») ; plus de précisions entre parenthèses (F1 et la palette les gardent) ;
  libellés plus courts pour les filtres d'étoiles et les couleurs
  (plus de « (ajouter / retirer) »).
- **Plus de bouton 1:1 en grille** : la loupe réglable le remplace ; `z` ouvre toujours
  l'image sélectionnée à 100 %.
- **« Toutes les vignettes à l'écran » devient un interrupteur** (bouton et `*` en
  grille) : allumé, les vignettes rétrécissent pour toutes les voir ; éteint, elles
  reprennent leur taille. Changer la taille à la main l'éteint.
- **Historique de la palette** : champ vide, la palette de commandes montre ce qu'on y a
  lancé, le plus récent d'abord ; ↑ / ↓ et Entrée le relancent. Gardé d'une session à
  l'autre (`[gui.dialogs] palette_history`, 20).
- **Piles : pauses et progression du curseur** (#306) : une pause plus longue que la
  cadence du dossier éloigne deux images (en studio, un changement de pose tombe souvent
  sur une pause). Deux nouveaux curseurs dans la fenêtre de regroupement (`Ctrl+G`),
  retenus par dossier : « Importance des pauses » (à gauche, le contenu seul ; à droite,
  les pauses coupent davantage, en photo de rue par exemple) et
  « Progression du curseur » (vers la gauche, chaque pixel retire la même part des
  piles ; au-delà de 1, départ plus doux du centre et droite plus rapide). À fond à gauche, le curseur de sévérité
  regroupe désormais tout. Réglages `[similarity] pause_weight`, `pause_range`,
  `strictness_curve`, `strictness_curve_max`. Fermée par Échap ou `Ctrl+G`, la fenêtre
  retient aussi la sévérité.
- **Comparaison** : jusqu'à 21 images par défaut (`[gui.compare] max`, 6 avant : à peu
  près la taille des vignettes sur un écran), et la loupe (`t` maintenue) sur l'image
  sous le pointeur, comme en plein écran.
- **Bouton de la pile** dans la barre des filtres, en mode pile : seulement la pile
  courante (`f c`), allumé quand on la regarde, recliquer pour tout revoir.
- **Raccourcis par défaut dans un fichier** (#280) : plus aucun raccourci écrit dans le
  code. Les jeux classique, vim et communs sont dans `keys.toml`, livré à côté du
  programme et lu au lancement. Il liste toutes les actions, avec `[]` pour celles sans
  raccourci. Une copie intégrée prend le relais si le fichier manque ou est illisible ; un
  fichier illisible est signalé dans la barre d'état. Vos propres raccourcis restent dans
  `[keys]`. Le préfixe des panneaux (`leader`, Ctrl+b) y est aussi ; `[gui] leader` est
  désormais vide par défaut et ne le remplace que si vous le réglez.
- **Revue des raccourcis** (#280) : une lettre par domaine, tirée de son nom anglais
  (c recadrage, i histogramme, m loupe, n ordre, r rendu, s piles, t infos, v
  versions…). Maj + la lettre en est la variante, Ctrl + la lettre les réglages du
  domaine, et `<leader>` + la lettre + une touche un réglage secondaire (`<leader> i s`
  : échelle de l'histogramme). Les panneaux s'affichent et se masquent avec `<leader>`
  Maj + une direction. Le profil « both » disparaît, et la loupe passe sur `m` et dans
  `keys.toml`. Un leader ou une touche de loupe déjà liés à une action sont refusés. Le
  tableau complet avant / après est dans `doc/fr/gui.md`.
- **Le zoom comme la loupe** : `<leader> z` puis un nombre sans Entrée (`150`, `20` =
  200 %, `2` = 200 %) ; `Ctrl+K` / `Ctrl+J` changent le zoom d'un palier (la loupe si sa
  touche est tenue) ; dans la barre du bas, à côté du 1:1, −, la valeur (« Ajusté » ou le
  pourcentage ; un clic : ajusté ou 100 %) et +.
- **Version précédente** d'une image en plein écran : `Maj+V`, dans l'autre sens que
  `v`.
- **Petit histogramme** : il suit lui aussi le canal choisi (`<leader> i r`, `g`, `b`,
  `l`, `a`), plus seulement le grand. Choisir un canal n'ouvre plus le grand histogramme. Dans
  le grand histogramme, `s` change l'échelle et `c` passe au canal suivant.
- **Menu des suites** (comme which-key dans nvim) : après le début d'une séquence
  (`<leader>`, `f`, `<leader> r`…), un court instant sans suite affiche en bas de l'écran
  les touches possibles et ce qu'elles font. Le menu attend alors la touche suivante sans
  délai ; Échap l'abandonne. Réglages `[gui] key_menu` et `key_menu_delay_ms` (400 ms) ;
  sous-menus nommés dans `keys.toml` (`[menus]`).
- **Aide « ici »** : `?` montre par-dessus l'écran les raccourcis de l'endroit où l'on est
  (grille, plein écran, comparaison, recadrage, grand histogramme, panneau des dossiers
  ou des métadonnées), rangés par thème, en grand. `F1` reste la liste complète avec
  recherche. Réglages `[gui.dialogs] help_here_text` et `help_here_column`.
- **Recherche de l'aide** : elle trouve aussi les mots anglais (« crop », « peaking »,
  « stack ») et pardonne une faute de frappe dans un mot assez long (« histrogramme ») ;
  des mots-clés par thème (« noter » trouve les étoiles) ; une action qui a le mot dans
  son libellé passe devant. Fautes de frappe et classement valent aussi pour la recherche
  des réglages.
- **Grossissement de la loupe tapé** (`<leader> m`) : une seconde entre deux chiffres
  (600 ms avant), et un chiffre tapé juste après complète encore le nombre au lieu de
  noter la photo (avec Maj sur un clavier AZERTY, on tape moins vite).
- **Palette de commandes** : `Ctrl+Maj+P` (classique) ou `:` (vim), on tape quelques
  lettres, Entrée lance l'action trouvée ou ouvre le réglage trouvé dans la fenêtre de
  réglages. Chaque action montre ses touches ; les actions sans raccourci deviennent
  accessibles au clavier.
- **Aide plus lisible** : pour une action qui a des touches propres au jeu choisi (vim ou
  classique), l'aide, l'aide « ici » et les textes ne montrent qu'elles (en vim, « x »
  pour supprimer, sans Suppr) ; les touches communes marchent toujours.
- **Chercher une touche** : dans l'aide et la palette, « ctrl f » (ou « Ctrl+F », « maj
  p ») ne montre que les actions liées à cette touche. Une lettre seule ne trouve plus
  que des lettres seules (une touche, « x »), plus le « f » au milieu des mots.
- **Trier par…** : une action par ordre (nom, prise de vue, modification, étoiles,
  couleur, drapeau), en plus de « Ordre suivant » : dans la palette (« tri nom »), sans
  raccourci par défaut (à poser dans `[keys]`). La palette gagne un titre et un champ plus
  aéré.
- **Préfixes de la palette** : `>` ne cherche que les actions (comme dans VS Code), `?`
  que les réglages ; seul, le préfixe les liste tous. Réglables :
  `[gui.dialogs] palette_actions_prefix`, `palette_settings_prefix`. La palette prend
  l'aspect des autres boîtes (couleurs, marges, titre, croix).
- **Recherche en grille seulement** : le champ de recherche par nom n'est plus montré en
  plein écran, où `/` remet en « ajusté ». L'aide écrit `/` pour la recherche (seul en
  vim, avec `Ctrl+F` en classique).
- **Palette, ce qu'elle ne trouvait pas** : les libellés des cases du panneau
  (« Empiler automatiquement »), le pluriel (« rendus vignette »), et trois actions pour
  les cases RAW / JPEG (RAW seulement, JPEG seulement, les deux). Une ligne trouvée par
  les mots de son titre passe devant (palette et aide).
- **Loupe dans la barre du bas** : − et + changent le grossissement (ils ne faisaient
  rien), et l'icône de loupe n'est plus un bouton.
- **Filtre grisé** : « Développées », grisé quand aucune photo du dossier n'est
  développée, ne s'allume plus par la palette (absent) ni par `f r` (refusé, avec la
  raison dans la barre d'état) ; l'éteindre reste possible.
- **La loupe dans l'aide** : « loupe » la trouve dans l'aide, `?` et la palette, avec sa
  touche (`m`, maintenue) ; lancée depuis la palette, elle rappelle qu'il faut la tenir.
- **Un seul rendu affiché** : « Source affichée » et « Développement de chaque photo »
  deviennent une seule liste, « Rendu affiché », la même en grille et en plein écran
  (JPEG boîtier, couverture, chaque logiciel, rendu RAW). `r` / `Maj+R` : rendu suivant /
  précédent pour tout le dossier, retenu d'une photo à l'autre ; sur un développement
  passé en revue, `r` reste l'avant / après.
- **Bulle de la revue des développements** : elle cite les touches du jeu choisi (`h` /
  `l` en vim, plus « ← / → » écrit en dur) et `( (` pour déplier une photo, au lieu de
  touches qui n'existaient plus.
- **Loupe au clavier** : la touche de loupe maintenue (`m`), les flèches (`hjkl` en vim)
  la déplacent sans changer d'image, avec Maj d'un grand pas ; bouger la souris la rend
  au pointeur. Pas réglables : `[gui.zoom] loupe_step`, `loupe_step_large`. La loupe
  marche aussi **dans la grille**, sur la vignette sous le pointeur (au clavier : la
  sélectionnée).
- **Les cases du panneau dans la palette** : « Passer en revue les développements » et
  « Actions sur les fichiers » ont leur action, qui bascule l'état actuel (la palette ne
  proposait que leur valeur par défaut). Les lignes de réglages de la palette s'appellent
  désormais « configuration » : elles ouvrent la valeur par défaut.
- **Pas de la loupe au clavier** : mesurés dans la loupe (l'image à 100 %), pareils en
  grille et en plein écran : un quart de loupe, une loupe entière avec Maj (sur une
  vignette, ils sautaient des centaines de pixels).
- **Panneau de droite** : chaque section commence à la même distance de son titre
  (« Vue » et « Grille », qui s'ouvrent sur un sous-titre, commençaient plus bas).
- **Palette sans filtre** : elle propose toutes les actions et tous les réglages, où que
  soit le clavier (depuis un panneau latéral, « passer » ne trouvait pas la revue des
  développements), sans limite de lignes par défaut (`[gui.dialogs] palette_max = 0`) ;
  ce qui vaut ici passe devant, et une action grisée ici dit pourquoi quand on la lance.
- **Flèches en symboles** : dans l'aide, les menus et la palette, les flèches et Entrée
  s'écrivent en symboles au lieu de « Flèche gauche »…, pris dans une police du
  système (Apple Symbols, DejaVu Sans, Segoe UI Symbol). Sans elle, en toutes lettres. Réglages
  `[gui] arrow_font`, `arrow_font_scale` ; `[keys]` accepte toujours les noms écrits.
- **Libellés sans touches** : celui des piles parlait de « flèches gauche / droite » (en
  vim, ce sont `h` / `l`) ; les libellés disent ce que fait l'action, ses touches sont à
  côté, celles du jeu choisi.
- **Loupe au bord de l'écran** : elle reste dans la zone visible (en haut ou en bas, la
  moitié sortait, surtout en grille et en mode concentration), le point visé au centre de
  ce qu'elle montre.
- **Flèches ou hjkl, selon le jeu** : après le leader, le menu des suites et l'aide
  montraient « h / Flèche gauche » ; ils ne montrent plus que `h` en vim, la flèche en
  classique (les deux touches marchent toujours).
- **Menu des suites plus lisible** : chaque touche sur la ligne de son libellé, et
  l'ordre de `keys.toml` (le violet avec les autres couleurs, plus en tête à cause de
  « ° ») ; des colonnes séparées par un trait et de l'air, chacune à sa largeur. Un trait
  discret entre les lignes, là et dans l'aide « ici » (`?`).
- **Développements des seuls logiciels actifs** : sans logiciel de développement, ou un
  logiciel désactivé, ses développements disparaissent de la grille (plus de badge ni de
  pile à déplier) ; ses sidecars ne bougent pas, le réactiver les retrouve.
- **Grossissement de la loupe au clavier** : `Ctrl+K` / `Ctrl+J` (ou `+` / `-` la loupe
  tenue) d'un palier ; `<leader> m` puis un nombre, sans Entrée : trois chiffres aussitôt
  (`150`), deux en dizaines (`20` = 200 %), un en centaines (`2` = 200 %) après une courte
  pause (`[gui.zoom] loupe_digits_ms`). La roulette, la loupe tenue, change aussi le
  grossissement (la grille ne défile pas). La valeur se voit dans la barre du bas (loupe,
  −, valeur, + ; un clic sur la valeur : 100 %). Pour la session.
- **Icônes des barres sans fond au repos** : les boutons à droite de la barre du bas
  (grille, comparaison, piles, flèches…) avaient un fond légèrement différent de la
  barre ; comme les autres icônes, ils n'en ont plus que sous le pointeur.

## 1.0.0-alpha.8 — 05/11/2026

Huitième et dernière version de test avant la bêta : logiciels de développement
(darktable, RapidRAW, RawTherapee, ART), partages réseau, piles, couleur d'accent et
réglages, corrections des tests de bout en bout.

- **Bulle du nombre d'images d'un filtre** : elle compte les photos comme la barre
  d'état ; un développement montré à part (clone, copie) ne compte plus pour une image de
  plus. Sous « Développées », un logiciel sans aucun développement dans le dossier est
  grisé.
- **Icône des infos sur l'image** dans le bandeau du bas (plein écran, à gauche du 1:1) :
  montre et change l'affichage des infos sur l'image (`Ctrl+I`) ; masquées, le repère de
  position part aussi, comme les notes.
- **Corrections du test de bout en bout** : les rendus développés lancés d'office pour la
  grille s'arrêtent quand un autre dossier s'ouvre (#303), sans le trait au-dessus des
  tâches ; bouger un curseur de sévérité d'un filtre actif montre le nombre d'images
  (#304) ; le filtre « Développées » est grisé dans un dossier sans aucun développement
  (#305).
- **Bout de la pile ou de la liste** (#307) : en plein écran, ← / → qui ne bougent plus
  le disent un instant (« Fin de la pile », « Fin de la sélection »…), place et durée dans
  `[gui.end]` ; il s'efface dès qu'une autre image s'affiche.
- **Filtre « Développées » avec un seul logiciel** (#301) : une case au lieu de deux, et
  plus de ligne « Aucun coché ». Les tâches de fond en bas du panneau de droite prennent
  sa couleur de fond (#300).
- **« Source affichée » dans l'ordre de la grille** : JPEG boîtier d'abord, puis les
  logiciels, RAW en dernier.
- **Développement de chaque photo** (plein écran) : la liste passe sous « Passer en revue
  les développements », sans titre, et se masque pendant la revue au lieu d'être grisée.
  Hors du mode pile, la revue passe par chaque développement (→ sautait ceux d'une photo
  repliée, ou restait bloqué). Sur un développement, « Source affichée » est une liste
  (le logiciel, JPEG boîtier) comme sur une photo, plutôt qu'une case « Avant ». Avec le
  JPEG boîtier pour source, les options des développements sont grisées (pas de revue),
  et leur liste ne propose plus le JPEG boîtier : il se choisit à un seul endroit, la
  source.
- **Logiciels de développement appliqués tout de suite** : changer Auto / Manuel /
  Désactivé dans les réglages relance la recherche, sans redémarrer ; un logiciel désactivé
  n'est plus proposé pour le rendu des vignettes. « Rendu des vignettes » et « Source
  affichée » disent pareil : JPEG et RAW, « boîtier » et « LibRaw » en gris, le logiciel.
  En plein écran, « Rendu (le même que la grille) » devient « Développement de chaque
  photo », ce qu'elle choisit.
- **Recherche des dossiers, sortie à la souris** : une croix au bout du champ, et un second
  clic sur l'icône « serveur », la referment (un clic par erreur laissait « smb:// »).
- **Couleur d'accent** réglable (`[gui.colors] accent`, `accent_light` pour le thème
  clair), avec un sélecteur de couleur dans les réglages : sélections, curseurs, et tout
  ce qui prend l'accent (étoiles, liseré des boîtes) la suivent. Les couleurs qui
  peuvent suivre l'accent ont une case « Couleur d'accent » plutôt que le mot à taper. Les
  boutons de la barre du bas s'allument comme ceux de la barre des filtres : icône à
  l'accent, sans fond. La couleur des titres passe dans les couleurs : `[gui.colors]
  title` (l'ancienne `[gui.dialogs] title_color` reste lue).
- **Éditeur externe plus simple à régler** : « Application du système » (celle que le
  système choisit pour chaque type d'image, nommée dans les réglages) ou « Autre
  application », choisie par « Parcourir… », avec `{files}` déjà en argument.
- **Notes sur l'image** : même cadre que le repère de position et même principe, durée
  et place réglables (`[gui.notes]`), bandeau du bas affiché ou non ; à la même place, le
  repère se met à côté des notes. En plein écran, l'icône du panneau des dossiers est grisée (il
  n'existe qu'en grille).
- **Logiciels de développement dans les réglages** : pour chacun, Auto (avec l'endroit
  où il a été trouvé au lancement : une AppImage oubliée se voit), Manuel (un chemin,
  « Parcourir… ») ou Désactivé ; sans aucun, la liste de ceux pris en charge.
- **« Parcourir… »** à côté de chaque champ de chemin (copier / déplacer vers, export,
  déchargement, copie depuis un serveur, réglages) : la fenêtre de choix de dossier du
  système.
- **Sans logiciel de développement installé** (darktable, RapidRAW, RawTherapee, ART),
  les réglages qui ne servent qu'avec eux sont masqués (fenêtre de réglages, section Vue
  du panneau) ; leurs chemins restent, pour en indiquer un installé ailleurs.
- **Repère de position** : « Dév. n / m » seulement en passant en revue les
  développements ; en plein écran, un clic sur l'image ou Entrée le remontre.
- **Plein écran, de photo en photo** : ← / → vont d'une photo à la suivante, chacune dans
  le rendu de la grille, sans passer par tous ses développements (Maj+W / Maj+Q pour
  eux). Une case « Passer en revue les développements » dans la section Vue (avec la
  liste du rendu, la même que la grille) rétablit le parcours de chaque développement
  (`[gui.view] review_developments`).
- **Plein écran, mode pile** : dans une rafale, une photo avec des développements ne
  bloque plus la navigation ; après son dernier développement, → passe à la photo
  suivante de la rafale (← à la précédente). Le repère de position a une ombre plus
  discrète.
- **Décharger sans sous-dossier** (#281) : un dossier de séance laissé vide décharge
  directement dans le dossier racine (« décharger ici » dans un dossier qu'on vient de
  créer).
- **Boîtes centrées** (#282) : l'éjection d'un support et « Développer avec… » (clic
  droit sur un dossier) s'ouvrent au centre, comme les autres boîtes.
- **Repère de position** : à chaque changement d'image, en grille et en plein écran, un
  petit cadre dit où on en est (« Photo 12 / 94 », avec les piles « Pile 3 / 20 · 2 / 5 »,
  et « Dév. 2 / 3 » pour les développements) ; durée et place réglables
  (`[gui.position]`).
- **Les boîtes se déplacent par leur titre seulement** (retour du 01/11/2026 : dans la
  fenêtre de réglages, glisser une pastille un peu à côté déplaçait toute la fenêtre) ;
  une boîte sans titre se saisit encore partout.
- Vignettes : plus de pastille « LibRaw » sur une photo montrée dans le rendu interne
  (retour du 31/10/2026 : elle ne servait à rien) ; seul le rendu d'un logiciel porte
  une pastille.
- **Fenêtre de réglages : un éditeur à pastilles pour l'ordre des couleurs du tri**
  (`[sort] color_order`, idée de l'utilisateur) : cinq emplacements vides sous « Tri »,
  les couleurs restantes sous « Disponibles » ; glisser ou cliquer pour composer l'ordre,
  écrit à chaque changement et appliqué aussitôt (la grille triée par couleur suit).
  **Corrigé : au lancement, un tri par note était appliqué avant la lecture des notes**
  (ordre des noms jusqu'au changement suivant) : la grille est retriée à la fin de la
  lecture en fond.
- **« Développer avec » : tout logiciel est grisé tant que la photo n'y est pas
  développée**, darktable compris (il rendrait par défaut, mais il n'y a rien à voir) ;
  seul le rendu LibRaw est toujours proposé. Les raccourcis le disent aussi (règle de
  l'utilisateur, 31/10/2026 : « c'est moi qui développe »).
- **Corrigé : un profil RawTherapee / ART (ou un `.rrdata` RapidRAW) rempli après la note
  qui l'avait créé n'entrait pas dans la pile des développements** avant la réouverture
  du dossier (retour du 31/10/2026) : la surveillance traite un sidecar principal qui
  devient un développement comme le XMP réécrit par darktable.
- Pastilles des clones et copies sur les vignettes : le numéro seul (« DT · 1 »,
  « RR · 2 ») ; une photo montrée dans le rendu d'un logiciel porte aussi le nom court
  (« RT », pas « RawTherapee »).
- **Tri par note** (`f s`, liste des tris de la section « Grille », `[gui] sort`) : par
  **étoiles** (une rejetée sous 0 étoile), par **couleur** (ordre du nouveau
  `[sort] color_order`, qui peut ne citer que quelques couleurs : les autres et les sans
  couleur vont à la fin) ou par **drapeau** (acceptée, sans drapeau, rejetée). Une note
  posée, annulée ou modifiée ailleurs retrie la grille aussitôt, la sélection suit
  l'image. Les couleurs s'écrivent `"red"`, `"yellow"`… en TOML et dans les vues par
  dossier enregistrées (l'ancien `"Red"` reste lu).
- **Boîte « Sidecars » : les logiciels installés seulement** (XMP toujours), comme la
  liste des rendus ; ART était proposé sans être là (retour du 31/10/2026).
- **RawTherapee / ART en AppImage sous Linux** : les AppImage récentes ne lancent que
  l'interface ; `tri` extrait l'AppImage une fois dans son cache et lance le
  `rawtherapee-cli` / `ART-cli` extrait avec ses bibliothèques. Une AppImage d'ART est
  reconnue à son nom (`ART_*`), plus confondue avec celle de darktable (« art » est dans
  « darktable »).
- **Corrigé : le rendu RawTherapee / ART d'un développement échouait** (premier essai réel,
  31/10/2026) : le lot en fond passait à la ligne de commande le chemin virtuel du
  développement (`IMG@rt.RAF`) au lieu du fichier de la photo.
- **Interrupteur des piles de retour dans le bandeau de la grille** (parti de la barre
  des filtres avec sa réorganisation, #199) et **dans la section « Vue »** du panneau de
  droite en plein écran, à côté de la source affichée (retour du 31/10/2026).
- **Corrigé : le filtre « développées » montrait des photos seulement notées, comme des
  développements darktable orphelins** (retour du jour). Deux causes : les piles et le
  rendu / filtre n'avaient pas la même règle pour « un développement darktable », et le
  filtre jugeait la photo et son développement séparément (le développement porte le
  sidecar de la photo, seul lui passait). Une seule règle désormais, partout : un XMP
  darktable n'est un développement qu'avec un **historique non vide**
  (`darktable:history_end` > 0, en attribut ou en élément) ; un XMP seulement importé
  dans darktable, ou écrit par `tri` pour les notes, n'en est pas un ; et le filtre garde
  une photo avec ses développements des logiciels cochés.
- **Corrigé : noter des photos relançait le lot de développements en fond** (retour du
  jour : vingt notes, vingt relances). RapidRAW installé, une note crée un `.rrdata`, que
  la surveillance du dossier prenait pour un nouveau développement : relecture du dossier
  et relance du lot. Un `.rrdata` qui n'a que des notes ne compte plus.
- **Corrigé : noter une photo développée dans RapidRAW refaisait son rendu en fond**
  (retour du jour : vingt notes, vingt développements). La clé de cache du rendu d'un
  `.rrdata` ne couvre plus que ses réglages, comme le XMP darktable ignore les étiquettes
  de couleur et les profils RawTherapee / ART ignorent `[General]`.
- **Infos des vignettes masquées d'elles-mêmes sur les petites cellules** (`[gui.grid]
  info_min_cell`, 110 points) : nom, format et marques étaient illisibles ; le réglage
  Maj+T ne bouge pas, les infos reviennent en agrandissant les vignettes.
- **Vignettes : marques vides** (`[gui.grid] empty_marks`, remplace `neutral_flag`) : là
  où rien n'est posé, un contour sombre (`[gui.colors] empty_mark` ; gris clair d'abord,
  invisible sur la case sélectionnée) d'étoile, de pastille de couleur et du cercle du
  drapeau neutre montre chaque information et où cliquer.
- Vignettes : liseré sombre de 2 points uniforme autour des étoiles (un contour tracé,
  une étoile agrandie amincissait les pointes) et des pastilles ; le sélecteur de marque
  est à la largeur de son contenu.
- **Développements sur la vignette : une bobine** (l'icône du filtre « développées », dans
  la couleur du texte de la cellule, sans pastille colorée) remplace « n dév. » ; le
  nombre est dans sa bulle ; l'icône « replier » d'une pile dépliée est dessinée de même,
  sans fond. Les deux sont centrées sur la pastille de couleur au-dessus, de sa taille
  apparente.
- Bandeau de notation et sélecteurs : plus de cercle d'accent autour de la couleur posée,
  la pastille pleine face aux autres fondues suffit, comme les étoiles et les drapeaux.
  L'anneau du drapeau neutre est dessiné un peu plus petit, pour paraître de la taille des
  pastilles acceptée / rejetée, et sur la vignette c'est un seul cercle sombre. Les
  drapeaux et couleurs de la barre des filtres suivent le même style : en couleur quand le
  filtre est posé, fondus sinon, sans fond ni cercle. Les **drapeaux du filtre sont trois
  boutons combinables** (acceptées, neutres, rejetées : n'importe quel mélange ; les
  préréglages du clavier restent), et les interrupteurs d'analyse se fondent au lieu de
  grisailler (puis retour au gris moyen : l'accent fondu ne se lisait pas sur la barre).
  Les pastilles « n dév. » et « replier » passent en gris foncé (`[gui.colors]
  development_mark`, un peu plus sombre en thème clair) au lieu du bleu clair.
- **Noter à la souris sur la vignette elle-même** (#257) : un clic sur les étoiles, le
  drapeau ou les pastilles de couleur d'une vignette ouvre un petit sélecteur du groupe
  concerné, à cet endroit (les contrôles du bandeau, scindés en trois) ; une étoile ou un
  drapeau le ferme, les couleurs restent ouvertes.
- **RawTherapee et ART comme logiciels de développement**, sur les mêmes traits que
  darktable et RapidRAW : leur développement principal (`IMG.RAF.pp3`, `IMG.RAF.arp`, sans
  clones) apparaît dans la pile de la photo, est rendu à la demande par `rawtherapee-cli`
  / `ART-cli` (cherchés dans le PATH, aux emplacements habituels, en AppImage ou Flatpak ;
  `[develop] rawtherapee`, `art`), et le dossier s'ouvre dans l'un ou l'autre. **Les notes
  suivent dans leurs profils** (`[General] Rank`, `ColorLabel`, `InTrash` = rejetée ;
  nouveau module `tri::pp3`, un format pour les deux), créés comme le `.rrdata` selon
  `[sidecars.rawtherapee]` / `[sidecars.art]` ; un profil réduit aux notes n'est pas un
  développement. Le drapeau neutre est le même cercle vide partout (vignette, bandeau,
  filtre), de la taille des autres pastilles ; dans le bandeau de notation, les pastilles
  acceptée / rejetée non choisies gardent leur couleur, fondue comme les pastilles de
  couleur ; la pastille « n dév. » garde une seule hauteur.
- **Mêmes touches pour les piles de rafale et de développements** : `s`, `(` `)`, `Maj+S`,
  `d)` `y)` et les flèches du mode pile agissent sur la rafale si les piles sont actives
  et qu'elle compte plusieurs photos, sinon sur les développements de la photo. Nouveaux
  **`((`** : ouvre le niveau suivant (la rafale repliée, puis les développements) et
  **`))`** : ferme le plus intérieur. `Maj+V` et `Ctrl+Maj+V` perdent leur touche par
  défaut (`core.dev.toggle`, `core.dev.fold_all` restent réglables dans `[keys]`). Les
  développements d'une photo restent désormais dans sa pile de rafale (la similarité
  coupait entre eux, et la pile de développements disparaissait dès que les piles étaient
  actives). Les pastilles « N dév. » et « replier » passent sous les pastilles de couleur
  au lieu de les recouvrir. Le **drapeau neutre est dessiné aussi** (un cercle vide à la
  place du drapeau, `[gui.grid] neutral_flag`) : chaque vignette montre toutes ses
  informations.
- **Corrigé : « Développer avec RapidRAW » sur une photo dont le `.rrdata` ne porte qu'une
  note** rendait un développement orphelin, hors de la pile : un tel sidecar n'est plus un
  développement RapidRAW pour le rendu non plus, et l'entrée est grisée dans la boîte du
  clic droit.
- **Générer les sidecars de notes manquants depuis l'interface** (suite de #253) : une
  icône « sidecar » dans la boîte du clic droit d'une vignette (la sélection) et « Générer
  les sidecars… » dans le menu du clic droit du dossier ouvert ; la boîte propose tous les
  logiciels ou un format, peut **aussi réécrire les sidecars existants** (leurs notes
  seulement, le reste conservé, un fichier illisible recréé ; avec avertissement ;
  `--rewrite` en ligne de commande), annonce le nombre de sidecars à créer ou réécrire
  avant de le faire, et rend compte dans la barre d'état. Action `core.file.sidecars`, sans touche par défaut.
- **Panneau de droite : de l'air sous chaque groupe de réglages** (`[gui.metadata]
  group_gap`, en hauteurs de ligne), et les boutons de développement du plein écran dans
  le sens de lecture, précédent à gauche (retour du 31/10/2026).
- **Barre des filtres : la recherche par nom est à droite**, juste avant le compteur
  d'images (#241).
- **Aide : « Signaler un problème » en bordeaux** (#242), et **la fenêtre d'aide se
  déplace** (#251 ; main « saisir » sur sa barre de titre). La **boîte de déchargement s'ouvre au centre** comme les autres
  (#252).
- **`q q` quitte** au lieu de `q` seul, trop immédiat à côté de `Maj+Q` (#244).
- **`tri-gui <image>` ouvre l'image en plein écran**, dans son dossier (#250), en mode
  concentration par défaut (`[gui] image_focus`) ; **`tri-gui --focus`** force le mode
  concentration dans tous les cas, pour faire de `tri` la visionneuse du système.
- **Panneau EXIF : précisions en gris** (#249) : la focale équivalente 24 × 36 et les
  coefficients de netteté à la ligne, les mégapixels derrière un tiret, sans parenthèses.
  `Metadata::rows` renvoie des `Row` (libellé, valeur, précision) ; `tri --metadata` écrit
  « valeur – précision ».
- **Plein écran : une section « Vue » dans le panneau de droite** à la place de
  « Grille » (#239) : la source affichée en liste déroulante (rendu boîtier, rendu RAW,
  chaque développement déjà rendu ; sur un développement, une case « avant ») et
  l'interrupteur des actions sur les fichiers. Le rendu des vignettes de la section Grille
  est aussi une liste déroulante (#246). Les boutons de développement de la barre du bas
  passent bien d'un développement à l'autre (ils partaient dans la notation et avançaient
  d'une photo) et sont symétriques (#240), et la pastille « N dév. » disparaît avec les infos des
  vignettes, où elle était tronquée (#243).
- **Notes écrites dans tous les sidecars de la photo** (`tri::sidecars`, un format par
  logiciel) : le XMP et le `IMG.RAF.rrdata` de RapidRAW, lus dans le plus récent, tous
  écrits. Un `.rrdata` manquant est créé quand on note selon `[sidecars.rapidraw] create`
  (`auto` : RapidRAW installé ou dossier qui en contient déjà ; `always` ; `never`) :
  une note posée dans `tri` se voit dans RapidRAW même sur une image jamais ouverte
  dedans ; `tri <dossier> --sidecars rapidraw|xmp|all [--dry-run]` les crée à la demande
  pour les photos notées. Un `.rrdata` sans réglages (image seulement notée) n'est plus
  un développement : pas de pile « 1 dév. » pour elle.
- **Rendu des vignettes retenu par dossier**, comme les filtres et le tri (`f d`, panneau
  de droite) : un dossier jamais réglé prend `[gui.grid] render`. Le rendu décide désormais
  de la **couverture** d'une photo développée : « JPEG boîtier » et « LibRaw » montrent la
  photo elle-même (la grille ne change plus après quelques secondes), **« Couverture »**
  (remplace « Dernier développement ») montre le développement préféré, sinon le plus
  récent, un logiciel son développement principal. `render = "latest"` reste accepté dans
  la configuration.
- **Replier les piles de développements à la souris** : chaque vignette d'une pile
  dépliée porte une pastille « replier » (clic : replier ; Alt+clic : replier en gardant ce
  développement en couverture). `Ctrl+Maj+V` replie toutes les piles de développements.
  `Maj+Entrée` met désormais toujours le développement en couverture (sur la photo : plus
  de préféré) au lieu de basculer.
- **Corrigé : les notes n'atteignaient RapidRAW que posées sur son développement** (#234).
  Une photo et ses développements principaux (darktable, RapidRAW) ont désormais les
  mêmes notes : noter l'un d'eux écrit le XMP de la photo (darktable, la paire RAW+JPEG)
  et le `IMG.RAF.rrdata` de RapidRAW. Les clones et copies virtuelles gardent les leurs.
  L'annulation remet les deux sidecars.
- **Développements comptés à part** dans la barre d'état (« 12 images (10 RAW, 3 dév.) » :
  une photo et ses deux copies ne font plus « 3 images »), et **leurs vignettes toujours
  rendues en fond**, chacune par son logiciel, même avec `[gui.grid] develop = "manual"`
  (qui ne concerne plus que les photos à développer pour le rendu choisi).
- **Comparaison des développements d'une photo** (`c` sur une photo développée, sans
  sélection) : le JPEG boîtier, le développement darktable, ses clones et les copies
  RapidRAW côte à côte, comme une rafale ; sur un développement, `b` montre le JPEG boîtier
  avant / après. Une rafale repliée compare toujours ses photos, chacune par la couverture
  de sa pile de développements. Échap revient à la grille sur le développement actif,
  montré à la place de la couverture.
- **Export des développements RapidRAW** (`Ctrl+E`, `tri --export`) : une copie ou le
  développement principal RapidRAW est rendu par RapidRAW (TIFF 16 bits, converti dans
  l'espace du préréglage, recadré, réduit et accentué comme les autres), un clone
  darktable par darktable, quoi qu'on regarde ; un développement sans son logiciel est
  exporté comme sa photo. Fichiers nommés comme le logiciel nomme ses exports (`IMG_01`,
  `IMG_VC01`). Corrigé : un clone darktable exporté avec un préréglage recadré ou accentué
  était passé sous silence.
- **Supprimer un développement** (Suppr / `x` sur un développement) ne retire que lui : un
  clone darktable, une copie RapidRAW ou le développement principal RapidRAW perdent leur
  sidecar, le développement principal darktable voit son XMP réécrit sans l'historique
  (notes gardées) ; la photo reste. **Ctrl+Suppr** / `dd` supprime la photo avec tous ses
  développements, même depuis un développement ; Maj+Suppr / `d)` supprime toujours
  toute la pile. Plusieurs développements sélectionnés : une case cochée par
  développement. Annulable (`u`) ; un sidecar modifié par darktable entre-temps n'est
  jamais écrasé. En ligne de commande, `tri --delete` fait de même.
- **Développements d'une photo** (darktable et ses clones, RapidRAW et ses copies
  virtuelles) : chacun est une image de la grille, avec sa vignette (son rendu), ses notes
  et son export, empilée sous sa photo (repliée par défaut, pastille « N dév. » ; `Maj+V`
  déplie, `Maj+W` / `Maj+Q` les font défiler, `Maj+Entrée` choisit la couverture, `b` en
  plein écran montre le JPEG boîtier avant / après). Une photo qui a des développements
  reste le JPEG boîtier et le rendu LibRaw ; un développement porte le nom de la photo,
  une pastille dit lequel (« DT · clone 1 », « RR · copie 2 »). Les clones étaient des
  vignettes en vrac et les copies RapidRAW manquaient.
- **Démarrage sur un dossier lent** (NAS) : la fenêtre s'ouvre tout de suite, et le
  dernier dossier se lit en fond avec sa progression ; elle attendait la liste du
  dossier plusieurs secondes, avec une icône générique dans le Dock.
- **Filtres en plein écran** : la barre des filtres (Maj+F) et tout le panneau de droite
  (filtres, grille) sont aussi là en plein écran, pour noter par passes sans repasser par
  la grille.
- **Filtres et tri retenus par dossier**, d'un lancement à l'autre
  (`[gui] remember_folder_view`, activé par défaut).
- **Copies virtuelles de RapidRAW** (`IMG.RAF.<id>.rrdata`) : elles suivent maintenant
  l'image copiée, déplacée ou renommée ; elles restaient derrière.
- **Rendu darktable par défaut** : une image importée dans darktable mais pas retouchée
  (un clone vierge, par exemple) a maintenant son rendu darktable, qui n'est pas celui
  de LibRaw ; il suffisait jusqu'ici d'un historique non vide.
- **Une vignette par clone darktable** : un clone (`IMG_01.RAF.xmp`) apparaît à côté de
  son image, avec ses propres notes et son propre développement. Le supprimer ne
  supprime que son sidecar ; le copier, le déplacer ou le renommer emporte l'image et
  tous ses clones. Le sidecar de RapidRAW (`.rrdata`) suit aussi l'image.
- **Actions sur les fichiers interdites par défaut** : renommer, copier, déplacer et
  supprimer images et dossiers depuis tri se fait seulement si on l'autorise (case
  « Actions sur les fichiers » du panneau de droite, `[files] allow_actions`) ; sinon,
  ces actions sont grisées et disent pourquoi. Autorisées, les boîtes rappellent que
  darktable, Lightroom ou Capture One ne retrouveront plus les fichiers déplacés ou
  supprimés.
- **Voisinage réseau** : les serveurs annoncés sur le réseau local (Bonjour / mDNS : SMB,
  AFP, SFTP, WebDAV, FTP, NFS) apparaissent sous « Réseau » dans le panneau des
  dossiers ; un clic liste leurs partages, comme en tapant leur adresse
  (`[neighbourhood] enabled`).
- **Développements à refaire annoncés** : quand des photos développées ailleurs
  (darktable) ont changé, un bandeau en haut dit combien de rendus sont refaits, avec une
  barre de progression, puis qu'ils sont faits ; il s'efface de lui-même (`[gui.grid]
  develop_toast_ms`).
- **Partages réseau sous Linux (GVFS)** : lister un dossier ne demande plus le type de
  chaque fichier par le réseau (1 à 2 images par seconde sur un partage ouvert par
  Nemo) ; la surveillance du dossier espace ses relevés quand ils sont lents
  (`[watch] max_share`, 10 % du temps au plus) au lieu d'occuper le réseau.
- **Les clones et les réglages des autres logiciels suivent l'image** : déplacer, copier,
  renommer ou supprimer une image emporte aussi ses clones darktable (`IMG_01.RAF.xmp`…)
  et les réglages de RawTherapee (`.pp3`), d'ART (`.arp`) et de Capture One
  (`CaptureOne/Settings…/IMG.RAF.cos`, gardé dans son sous-dossier). Ils restaient derrière.
- **Dossiers lents (NAS, disque lent)** : un dossier dont la liste prend du temps s'ouvre
  en fond, avec une boîte qui montre combien d'images sont déjà trouvées et un bouton
  Annuler (ou Échap) ; le dossier d'avant reste affiché en attendant. Quitter n'attend
  plus les vignettes en cours. `TRI_GUI_TRACE=1` trace aussi les images lentes de
  l'interface, étape par étape (`[gui] trace_slow_ms`). La grille ne relit plus la
  taille et la date de chaque fichier source à chaque image (jusqu'à 37 s figée sur un
  NAS) : elles sont relevées une fois, et oubliées quand la surveillance du dossier voit
  un fichier changer ; pendant le dessin de la grille, un relevé manquant se fait en fond
  au lieu de retenir la page, comme les sidecars dont elle a besoin (développement
  darktable, badges des versions), la demande la plus récente d'abord (en défilant vite,
  les vignettes visibles passent avant celles déjà dépassées). Un sidecar est cherché sous
  son nom exact avant de relire tout le dossier (plein écran, d'une image à l'autre). Le
  moteur vérifie d'abord les vignettes à l'écran, puis les autres par distance (à
  l'ascenseur, plus d'attente de toutes les rangées du dessus). En plein écran,
  l'histogramme du JPEG boîtier se calcule en fond.
- **Boîte de connexion réseau** : un serveur qui demande un compte que le système ne
  demande pas lui-même (Linux, Windows ; macOS garde sa boîte) ouvre une boîte :
  utilisateur, domaine (SMB), mot de passe, « Retenir le mot de passe » (gestionnaire
  d'identifiants de Windows, trousseau du bureau sous Linux). Un compte refusé fait
  revenir la boîte avec l'explication ; les autres échecs disent pourquoi (serveur
  injoignable, partage inconnu, déjà connecté sous un autre compte). Les copies FTP s'en
  servent aussi ; SFTP reste aux clés SSH.
- **Serveurs réseau, autres protocoles** : en plus de SMB, les adresses `afp://`,
  `ftp://`, `sftp://`, `dav://` / `davs://` (WebDAV) et `nfs://`, montées par le système
  quand il le sait (tout sous Linux, tout sauf SFTP sous macOS, SMB et WebDAV sous
  Windows). Ailleurs, FTP et SFTP sont copiés en local (`curl`, `sftp` d'OpenSSH : pas de
  nouvelle dépendance), après une boîte qui annonce le nombre d'images et la taille, la
  destination au choix (`[remote]`).
- **Partages réseau** : se connecter à un partage par son adresse, `\\nas\photo` ou
  `smb://nas/photo` (icône « serveur » du panneau des dossiers, `c`, ou l'adresse tapée
  dans la recherche des dossiers). Le système monte et demande le mot de passe ; le
  partage est ensuite gardé dans la section « Réseau » (clic droit ou Maj+C pour
  l'oublier). Sous Windows, les lecteurs réseau (`Z:`) sont rangés dans « Réseau ». Un
  serveur seul (`smb://nas.local/`) liste ses partages, au choix ; un partage déjà monté
  en AFP (Nemo) est reconnu.
- **Panneau des dossiers** : « Sous-dossiers » devient une icône (arborescence), grise,
  à l'accent quand l'option est active (comme les filtres), juste après le titre ;
  Actualiser passe au bord droit du panneau, et le titre « Dossiers » prend la taille
  des sections dessous.
- **Corrections de la revue du 28/10/2026** :
  - noter une image développée dans Darktable ne refait plus son rendu : seul le
    développement du sidecar compte, plus sa date (`tri` y écrit aussi les notes) ; les
    rendus déjà en cache sont refaits une fois ;
  - l'export par LibRaw suit les réglages `[render]`, comme l'affichage ;
  - la couleur `[gui.colors] error` vaut pour tous les messages d'erreur ;
  - nouveaux réglages `[gui.dialogs] poll_ms`, `max_errors`, `completions` (valeurs
    jusque-là codées en dur) ;
  - les tests tapent les raccourcis comme la vraie fenêtre les livre (couper / copier /
    coller d'egui-winit, #194), pour chaque raccourci de chaque profil.
- **Télécharger la dernière version** : en haut de l'aide (F1), deux boutons, le build
  de nuit et la dernière alpha, avec une bulle qui explique la différence. Le paquet de
  ce système se télécharge dans le navigateur ; s'il y en a plusieurs (Linux : AppImage,
  .deb, archive ; Windows : .msi, .zip), une boîte permet de choisir, et le choix est
  retenu (`[update] formats`).
- **Ouvrir un dossier dans Darktable** : clic droit sur un dossier, « Ouvrir dans
  Darktable », ou `d` dans le panneau des dossiers. Darktable s'ouvre sur ce dossier
  (importé au besoin), avec sa propre bibliothèque ; une instance déjà ouverte le reçoit
  sous Linux et macOS ; sous Windows, l'entrée est grisée tant que Darktable est
  ouvert. RapidRAW ne sait ouvrir qu'une image, pas un dossier.
- **Luminance dans le grand histogramme** : une pastille de plus (un disque à moitié
  clair) et `l` montrent la luminance seule, avec sa ligne dans le tableau.
  Sur un RAW, elle est calculée sur les données du capteur, par bloc de la mosaïque
  (2 × 2 en Bayer, 3 × 3 en X-Trans), avec la balance des blancs de la prise de vue ;
  le vert n'en tient plus lieu. Couleur `[gui.colors] hist_luminance`. Les analyses
  d'exposition en cache sont recalculées une fois.
- **JPEG boîtier dans l'histogramme** : pour un RAW, la courbe du JPEG boîtier se
  dessine en trait fin gris par-dessus celle des données du capteur, pour voir ce que le
  boîtier a fait des hautes lumières et des ombres. Dans le grand histogramme, une ligne
  « JPEG boîtier » sous Rouge, Vert, Bleu lui sert de légende, avec ses chiffres
  (médiane, moyenne, plage, étalement, saturés, bouchés) ; un bouton et `j` la montrent
  ou la masquent, et c'est retenu (`[gui.histogram] camera_jpeg`, `camera_width`,
  `[gui.colors] hist_camera`).
- **Panneau de droite en sections** : la barre des filtres garde les filtres d'un clic
  et la recherche par nom ; leurs réglages et ceux de la grille passent dans le panneau
  de droite, en sections titrées comme celles du panneau des dossiers et repliables d'un
  clic : Histogramme, Filtres (réglages d'un filtre seulement quand il est allumé ; deux
  curseurs pour l'exposition ; logiciels de développement en cases à cocher), Grille
  (tri, rendu, piles), EXIF en bas ; les cases RAW et JPEG ouvrent la section Filtres. Les sections du
  panneau des dossiers se replient aussi ; les sections repliées sont retenues. Titres
  des sections en majuscules, plus grands, à l'accent, espacés (`[gui.folders]
  section_scale`, `section_gap`). Après un changement de filtre, le nombre d'images
  s'affiche un instant au milieu de l'écran (`[gui.grid] filter_toast_ms`). **Au
  clavier** (leader puis `l`) : ↑ ↓ parcourent aussi les réglages, Espace les actionne,
  ← → les font varier, Tab / Maj+Tab (`)` / `(` en vim) passent d'une section à l'autre,
  dans le panneau des dossiers aussi. La section Grille commence par le tri ; le choix
  de l'entrée dans une pile ne reste que dans la barre du bas. Libellés « NOTE », «
  FILTRE » et « DOSSIERS » sur les barres du bas et du haut et le panneau des dossiers,
  en petites majuscules à l'accent (`[gui.grid] bar_label_scale`). `[gui.grid]
  stack_slider` et `clip_slider` disparaissent : retirez-les de votre fichier de
  configuration si vous les aviez réglés. Dans une fenêtre étroite, la fin des barres
  des filtres et des notes ne glisse plus sous le panneau de droite.
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
