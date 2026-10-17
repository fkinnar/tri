# Visionneuse RAW — contexte projet

## Vision

Équivalent Linux et macOS (Windows si possible) de FastRawViewer : visionneuse RAW rapide pour le tri/culling
après une séance photo — vignettes de dossier, affichage plein écran en rendu RAW réel,
notation par étoiles/couleurs filtrable (stockée en XMP). Design minimaliste, moderne,
entièrement pilotable au clavier (inspiration nvim).

Ce projet est aussi une occasion d'approfondir Rust. La partie moteur (lecture, traitement,
filtrage) est ce qui intéresse le plus ; l'interface graphique est secondaire et volontiers
traitée a minima dans un premier temps.

## Portée actuelle (MVP) — ne pas élargir sans validation

On restreint volontairement le scope pour obtenir vite un résultat concret, avant de revenir
vers l'architecture cible décrite plus bas :

1. Parcourir un dossier et lister les fichiers RAW — tous les formats RAW gérés par LibRaw
   (RAF, CR2/CR3, NEF, ARW, DNG, ORF, RW2...), extensions comparées sans tenir compte de la
   casse. Les images « plates » (JPEG, TIFF, PNG, HEIF...) sont aussi listées : indispensable
   en pratique (cartes SD en RAW+JPEG).
2. Générer une grille de vignettes à partir du JPEG embarqué (LibRaw) pour les RAW, et de
   l'image elle-même pour les images plates (décodage simple, aucun développement).
   Attention : l'aperçu embarqué n'est pas pivoté, l'orientation EXIF doit être appliquée
   à l'affichage (le rendu complet `dcraw_process`, lui, est déjà pivoté).
3. Sélectionner une image et l'afficher en plein écran, développée via LibRaw (pas juste
   l'aperçu embarqué). Aucun toolkit d'affichage n'a encore été expérimenté : le choix reste
   à faire.
4. Navigation clavier gauche/droite entre images.
5. **Calcul en tâche de fond avec cache persistant** (ajouté au MVP le 16/10/2026, voir
   « Aperçus et cache ») : à l'ouverture d'un dossier, vignettes et aperçus standard sont
   calculés en fond, avec une progression visible dans l'interface.

Seul filtre requis dès le MVP : par type de fichier — RAW seuls, plates seules, ou les deux
(ex. une carte RAW+JPEG doit pouvoir s'afficher en entier, ou juste l'un des deux).

**Élargissement validé le 17/10/2026 (MVP terminé) : notation et filtres.** Étoiles (0–5),
couleurs, accepté/rejeté, stockés en XMP, et les filtres correspondants (même trait `Filter`
que le filtre par type). Décisions dans « Notation XMP » plus bas.

Pas de filtrage avancé, pas de dynamique/plugins/libloading à ce stade.
L'objectif est de valider la chaîne complète dossier → vignettes → rendu RAW affiché, et de
juger la vitesse/l'ergonomie avant de complexifier.

## Décisions d'architecture déjà prises (à respecter une fois le MVP validé)

Ces choix ont été discutés et arrêtés ; ne pas les remettre en question sans bonne raison,
mais ne pas non plus les implémenter avant que le MVP ci-dessus fonctionne :

- **Séparation forte moteur / affichage.** Le moteur est du Rust pur (lecture de dossier,
  traitement, filtrage), pensé pour ne jamais dépendre de l'UI. L'affichage est vu comme un
  "plugin" qui communique avec le moteur — potentiellement remplaçable à terme (ex. une UI
  web alternative).
- **Modules en librairies dynamiques**, mais seulement là où ça a du sens :
  - Le module de traitement RAW (LibRaw) doit être chargé dynamiquement via `libloading`.
  - Le module de filtrage, considéré comme le cœur métier, reste statique (traits + crates).
  - Pour l'affichage : pas encore tranché entre statique et dynamique.
- **Frontière FFI : structs typées en interne, JSON uniquement à la frontière.** Tout le code
  Rust interne utilise des structs typées normales ; la sérialisation JSON n'intervient qu'au
  passage de la frontière entre bibliothèques dynamiques. Investissement jugé rentable car il
  découple les interfaces du contrat JSON.
- **Traitement RAW appelé depuis un thread séparé à chaque appel** (vignette ou rendu HD), pour
  pouvoir paramétrer/tester librement le découpage du travail.
- **Perf vignettes (pour plus tard, pas MVP) :** découpage récursif en arbre de l'ensemble
  d'images à traiter en parallèle, + un thread dédié qui précharge les voisins de l'image
  sélectionnée (principe : on ne regarde rarement plus de 5 images/seconde, donc traiter n+1
  et n-1 pendant l'affichage de n).
  Précisé le 16/10/2026 : **en pratique, rarement plus d'un rendu LibRaw complet à la fois.**
  Pendant qu'on regarde n, on précalcule n+1 ; n-1 seulement quand on entre au milieu de la
  grille. Donc un seul worker de rendu complet, avec une priorité n > n+1 > n-1, et les
  requêtes devenues obsolètes (l'utilisateur a déjà bougé) abandonnées. Un rendu complet
  prenant ≈ 3,8 s sur un RAF X-T4, plus long que le temps passé sur une image en tri rapide,
  l'affichage doit montrer tout de suite l'aperçu embarqué (voire `half_size`), puis le
  remplacer quand le rendu complet arrive. Le parallélisme massif ne concerne que les
  vignettes (aperçus embarqués, sans calcul).
- **Filtrage (pour plus tard) :** point d'entrée unique qui retourne les images à afficher ;
  type de retour pensé dès le départ comme `Vec<Vec<Image>>` pour permettre la détection de
  groupes/séries de photos (façon Capture One), pas juste une liste plate.
- **Multiplateforme dès le MVP** : Linux et macOS au même niveau, les deux doivent compiler et
  tourner dès le départ (macOS est aussi la machine de développement quotidien). Windows est
  souhaité : ne pas faire de choix qui l'excluent (dépendances, build, chemins), même s'il
  n'est pas testé en continu.
- **Formats** : RAW et images plates (JPEG/TIFF...) dès le MVP ; vidéo/son à très long
  terme, hors scope pour longtemps.

## Décisions d'architecture complémentaires (réflexion du 16/10/2026)

Précisions actées après discussion, toujours non implémentées avant la fin du MVP ci-dessus :

- **Pas de parité stricte GUI / TUI.** Les deux backends (fenêtre classique et TUI via
  Ghostty/WezTerm, protocole graphique type Kitty) sont des "plugins d'affichage" au sens déjà
  défini. Le GUI reste la référence complète ; le TUI est un backend *best effort* (certaines
  fonctionnalités, notamment les overlays, peuvent y être absentes ou dégradées). Ne jamais
  tirer une décision de design vers le plus petit dénominateur commun des deux.
- **Contrat moteur → affichage en "rendu par zone/résolution demandée"**, pas un buffer plein
  cadre figé : le zoom/pan en TUI est contraint par la résolution pixel réelle du terminal, donc
  le moteur doit pouvoir rendre une portion d'image à une résolution donnée, pas seulement une
  image plein écran. Cohérent avec le préchargement n-1/n/n+1 déjà prévu.
- **Raccourcis clavier : indirection via des Actions nommées, pas touche → fonction.** Le JSON
  mappe touche → identifiant d'Action (`core.nav.next`, `core.rate.star.3`,
  `core.flag.reject`, `plugin.<nom>.<action>`...) ; l'espace de noms (`core.*` vs
  `plugin.<nom>.*`) est choisi dès maintenant pour que les futurs plugins puissent enregistrer
  leurs propres Actions sans collision. Deux profils de raccourcis livrés par défaut : un
  profil "vim" (hjkl + leader) et un profil "classique" (flèches), sélectionnables ; l'un ou
  l'autre peut servir de base à un profil personnalisé. La souris en GUI reste indépendante de
  ce fichier (pas de notion de clic dans le JSON), elle déclenche directement les mêmes Actions.
- **Deux familles de plugins sur une image, à ne pas confondre :**
  - *Overlay géométrique pur* (règle des tiers, nombre d'or...) : aucun calcul sur les pixels,
    pure superposition vectorielle côté affichage. Simple à supporter des deux côtés (GUI natif,
    TUI approximatif ou absent).
  - *Overlay calculé* (carte de netteté, détection de visage/yeux...) : nécessite d'analyser
    l'image décodée. Le plugin renvoie une structure (formes vectorielles + éventuellement une
    heatmap séparée), jamais un buffer de pixels déjà composé — ça laisse le GUI composer à
    pleine résolution et le TUI faire ce qu'il peut, sans format pixel unique imposé aux deux.
  - Les deux familles de plugins (galerie : `Vec<Image> -> Vec<Vec<Image>>`, et image : overlay)
    restent des `trait` Rust statiques avec une registry (`Vec<Box<dyn ...>>`) pour l'instant —
    pas de dynamique (`libloading`) pour elles tant qu'aucun besoin réel de plugin externe
    n'apparaît. Seul le module LibRaw reste dynamique, comme déjà décidé.
- **Analyse ≠ filtrage, à distinguer dans le schéma de métadonnées dès maintenant.** Calculer un
  score (netteté, détection de visage/yeux pour la "netteté des yeux" sur un portrait) est
  coûteux et doit tourner une fois puis être mis en cache ; filtrer sur ce score doit rester
  instantané. Conséquence pour le stockage :
  - Les métadonnées *utilisateur* (étoiles, couleurs, flags conservé/rejeté/neutre) restent dans
    les sidecars XMP, comme prévu.
  - Les métadonnées *dérivées* par des plugins d'analyse (score de netteté, zones de visage...)
    vivent dans un cache séparé, pas dans le XMP — ce n'est pas une donnée produite par
    l'utilisateur, et XMP ne convient pas à une heatmap.
  - Le schéma de ce cache est extensible par plugin (ex. un blob structuré par image et par
    plugin) plutôt que des colonnes fixes, pour ne pas devoir le faire évoluer à chaque nouveau
    plugin d'analyse (détection IA "yeux nets sur un portrait" compris, horizon lointain).
  - Les filtres natifs (étoiles, couleurs, flags) implémentent le même `trait` que les futurs
    filtres netteté/IA — pas de code spécial pour les built-in.

## Aperçus et cache (décidé le 16/10/2026)

Modèle inspiré de Lightroom : une taille d'aperçu configurée, générée automatiquement en tâche
de fond ; les aperçus 1:1 générés à la demande.

- **Trois niveaux** :
  - *vignette* (grille, ~400 px) : aperçu embarqué pour les RAW, image décodée pour les
    plates ; générée en fond, workers parallèles (peu de calcul) ;
  - *aperçu standard* (plein écran) : rendu LibRaw réduit au côté long configuré ; généré en
    fond pour tout le dossier dès l'ouverture ;
  - *1:1* (zoom 100 %) : rendu LibRaw complet, à la demande, mis en cache.
- **Aperçu boîtier (décidé le 16/10/2026)** : niveau de cache `Embedded`, le JPEG embarqué
  du RAW réduit à la taille standard. Calculé juste après les vignettes (tout le dossier
  consultable en ~35 s au lieu de ~3 min sur le dossier de test). **Affiché par défaut en
  plein écran** (précisé par l'utilisateur le 16/10/2026 : en tri rapide, le JPEG boîtier
  est une bonne base, le RAW se calcule tranquillement derrière), **basculable** vers le
  rendu RAW (touche `b`), et l'interface indique toujours lequel est affiché : le JPEG
  boîtier reflète les réglages du boîtier, pas les données RAW (exposition, hautes
  lumières).
- **Usage du 100 %** (16/10/2026) : présélection des candidates, puis vérification de la
  netteté à 100 % quand il en reste peu. Le zoom et la zone affichée sont conservés d'une
  image à l'autre pour comparer au même endroit.
- **Source et échelle indépendantes** (décidé le 16/10/2026) : `b` = source (JPEG boîtier
  par défaut / rendu RAW, reste active d'une image à l'autre), `z` = échelle (ajusté / 1:1).
  Zoomer ne change jamais de source. En JPEG, le 1:1 est le JPEG embarqué à sa taille
  (niveau `EmbeddedFull`, étiqueté « 71 % du capteur ») ; en RAW, le rendu complet à 100 %.
  Le rendu RAW plat est voulu : c'est l'intérêt de voir les données brutes.
- **Pas de luminosité automatique LibRaw** (corrigé le 16/10/2026, retour utilisateur) :
  activée par défaut dans LibRaw, elle rendait « bien exposée » une photo sous-exposée
  (DSCF0130 : 39 → 129 de luminance moyenne). Le rendu RAW montre l'exposition réelle de la
  prise de vue ; `RenderOptions::auto_bright` reste disponible, désactivé.
- **Rendus RAW à la demande dans l'interface** (`RawPreviews::OnDemand`) : rien en mode
  JPEG ; en mode RAW, l'image affichée et ses voisines. Calcul de tout le dossier en fond
  possible en option (`RawPreviews::Background`, à exposer dans la configuration).
  Motif : les rendus RAW ne peuvent pas être accélérés (décompression du RAF déjà
  parallélisée par LibRaw ; aperçus plus petits : −21 % seulement ; rendus en parallèle :
  plus lent). Mesures dans `doc/scheduler.md`.
- **Taille de l'aperçu standard : « Auto » par défaut** = plus grand côté de l'écran principal
  (2560 px sur la machine de dev, 2560×1440), valeur fixe configurable.
- **Source de l'aperçu standard : `half_size` si sa taille suffit**, sinon rendu complet.
  Règle : `half_size` tant que (côté long RAW / 2) ≥ côté long demandé. Mesuré sur RAF X-T4 à
  2560 px : PSNR 40–43 dB entre half_size réduit et rendu complet réduit (visuellement
  identiques sur recadrages 1:1 œil/barbe), 0,31 s + 0,15 s de réduction contre ≈ 3,8 s +
  0,31 s. En 4K (3840 > 3123), on bascule automatiquement sur le rendu complet.
- **Un seul worker LibRaw**, file à priorités : image affichée > voisines n+1/n-1 > reste du
  dossier en fond. Annulation d'un rendu devenu inutile via le callback de progression LibRaw
  (`libraw_set_progress_handler`, valeur de retour non nulle = annulation), qui donne aussi
  un pourcentage par image.
- **Progression** : événements envoyés par le moteur sur un canal (fait / total par niveau),
  l'UI ne fait qu'afficher. Le moteur reste indépendant de l'UI.
- **Cache central par utilisateur** (pas à côté des photos : cartes SD, lecture seule, NAS) :
  `$XDG_CACHE_HOME/tri` ou `~/.cache/tri` (Linux), `~/Library/Caches/tri` (macOS),
  `%LOCALAPPDATA%\tri` (Windows).
  - Clé : hash de (chemin absolu, taille, date de modification, niveau, version des paramètres
    de rendu). Fichier modifié ou réglages changés ⇒ recalcul. Déplacer un dossier invalide
    son cache (assumé).
  - Format : JPEG qualité ~90 (compact, décodage rapide).
  - Effaçable : par niveau ou en entier ; taille maximale pour les 1:1 avec éviction des moins
    récemment utilisés (`[cache] zoom_max_mb`, 2000 Mo par niveau ; branché le
    17/10/2026, au démarrage de l'interface).
  - Reprise naturelle : à la réouverture, seul ce qui manque est recalculé.
  - Pas de base de données pour l'instant (fichiers seuls). Distinct du cache d'*analyse*
    (scores de netteté, etc.) décrit plus haut.

## LibRaw depuis Rust — déjà résolu

Intégré dans `src/raw/` (`ffi.rs` = déclarations brutes, `mod.rs` = wrapper sûr `RawFile`
avec `thumbnail()` et `render(&RenderOptions)` → `Bitmap` RGB8). Outil de test paramétrable :
`cargo run --release --example decode -- <thumb|render|flat> <fichier> <sortie> [--half] [--daylight] [--bright X]`.
Images plates (JPEG/PNG/TIFF/WebP via `image`, HEIC/HEIF via `libheif-rs` ≥ libheif 1.17) :
`src/flat/`, même type de sortie `Bitmap` (`src/bitmap.rs`), orientation déjà appliquée.
JPEG (aperçus embarqués, JPEG plats, cache) via `src/jpeg.rs` : libjpeg-turbo derrière la
feature `turbojpeg` (défaut), repli pur Rust sinon. libjpeg-turbo 3.x est compilée depuis les
sources embarquées par `turbojpeg-sys` (CMake + NASM), liée statiquement : les LTS Linux
n'ont que la 2.1 (décidé le 16/10/2026, voir `doc/build.md`). API libheif fixée à `v1_17`
(Mint 22 / Ubuntu 24.04 : 1.17.6).
HEIF derrière la feature Cargo `heif` (défaut), code confiné dans `src/flat/heif.rs` ;
`--no-default-features` donne un binaire sans aucun lien HEVC (brevets, `doc/licensing.md`).
Prérequis système dans `prerequisites.sh`. Points acquis :

- LibRaw expose une **API C** (`libraw_*`) utilisable via FFI ; l'API C++ n'a pas d'ABI stable
  et n'est pas directement appelable depuis Rust.
- `build.rs` utilise `pkg-config` pour localiser la lib, avec une correction manuelle macOS :
  `pkg-config` renvoie `-lstdc++` (GNU) alors que macOS utilise `libc++` → remplacer `stdc++`
  par `c++` quand `CARGO_CFG_TARGET_OS == "macos"`. Sur Linux, `stdc++` est correct tel quel.
  Windows (non testé) : pas de pkg-config, on définit `LIBRAW_DIR` (dossier avec `include/` et
  `lib/`, ex. installation vcpkg) et éventuellement `LIBRAW_LIB` (nom de la lib, défaut `raw`).
  Ouverture des fichiers via `libraw_open_wfile` (UTF-16) sous Windows, octets bruts du chemin
  ailleurs (noms non UTF-8 acceptés).
- Édition Rust 2024 : la syntaxe est `unsafe extern "C" { ... }` (le `unsafe` se place devant
  `extern`, pas autour du bloc).
- Wrapper sûr autour du handle opaque `libraw_data_t*`, avec `Drop` qui appelle
  `libraw_close`, pour garantir la libération même en cas d'erreur.
- Chaîne testée avec succès : `libraw_init` → `libraw_open_file` → `libraw_unpack_thumb` →
  `libraw_dcraw_make_mem_thumb` (extraction du JPEG embarqué).
- Pour le rendu HD complet (nécessaire au MVP, pas juste l'aperçu) : même logique avec
  `libraw_unpack` → `libraw_dcraw_process` → `libraw_dcraw_make_mem_image` (bitmap RGB, au
  lieu du JPEG embarqué) — fonctionnel. Certains paramètres ont un setter C
  (`libraw_set_output_bps`, `libraw_set_bright`...), mais **pas** `use_camera_wb` ni
  `half_size` : ils sont réglés par un mini shim C (`csrc/libraw_shim.c`, compilé par `cc`
  avec le vrai `libraw.h`, donc layout de struct toujours correct, sans bindgen).
- Mesures (X-T4, RAF 26 Mpx, Mac Apple Silicon, release) : aperçu embarqué ≈ 10 ms ; rendu
  complet ≈ 3,8 s ; rendu `half_size` ≈ 0,35 s (X-Trans : le dématriçage complet est lent).
- **Mesuré le 16/10/2026 : un rendu LibRaw est déjà multithreadé en interne** via OpenMP
  (`libomp`, lié par la LibRaw de Homebrew ; ≈ 440 % CPU,
  ~200 Mo par instance en pleine résolution). Plusieurs rendus complets simultanés se gênent
  fortement (9 tests : 5,7 s en série, 18 s à 4 threads, 302 s à un thread par cœur). À
  prendre en compte pour le découpage parallèle des vignettes et le préchargement n-1/n+1 :
  peu de rendus complets à la fois, ce qui correspond à l'usage réel (voir « Perf
  vignettes »). (Les aperçus embarqués ne développent rien et ne sont pas concernés.)
- Pistes non vérifiées : crates `libraw-rs` / `rsraw` existent sur crates.io mais leur
  maintenance n'a pas été contrôlée — à vérifier avant adoption (préférence marquée pour des
  outils open-source activement maintenus plutôt qu'abandonnés).

## Notation XMP (décidé le 17/10/2026 ; moteur : `src/xmp.rs`, `doc/xmp.md`)

- **Sidecars, les deux conventions** : `IMAGE.RAF.xmp` (darktable, digiKam) et `IMAGE.xmp`
  (Lightroom, Capture One, Bridge ; partagé par le RAF et le JPG d'une paire). Lire **et**
  écrire dans le plus récent s'ils coexistent, sinon dans celui qui existe ; sans sidecar,
  convention de la configuration (défaut : darktable). Un utilisateur de Capture One règle
  une fois et `tri` écrit directement au bon format. Conversion de l'un vers l'autre en ligne
  de commande (pas forcément dans l'interface).
- **Ne jamais rien perdre** : modifier seulement nos champs, conserver tout le reste
  (historique darktable, mots-clés, droits…), accepter les deux formes XML (attribut ou
  élément), relire le fichier juste avant d'écrire. darktable ne relit les XMP modifiés
  ailleurs que si son option « rechercher les XMP mis à jour au démarrage » est active.
- **Étoiles** : `xmp:Rating` 0–5 (standard ISO 16684-1).
- **Rejeté** : `xmp:Rating = -1` (standard : -1 = rejeté, 0 = non noté), plus les champs
  « accepté » ci-dessous. Les étoiles d'une photo rejetée sont mémorisées dans un champ
  propre à `tri` (`tri:RatingBeforeReject`, espace de noms `urn:tri:xmp:1.0`), pour les
  restaurer si on annule le rejet (à valider à l'usage).
- **Accepté** (pas de champ standard) : `xmpDM:pick` (Lightroom Classic ≥ 13.2 : 1 / 0 / -1, avec
  `xmpDM:good` True / False) et `digiKam:PickLabel` (0 rien, 1 rejeté, 2 en attente, 3 accepté). Capture One : pas de
  documentation trouvée.
- **Couleurs** : une par défaut, plusieurs en option (configuration). darktable lit
  `xmp:Label` (« Red », « Yellow », « Green », « Blue », « Purple ») **en priorité** sur
  `darktable:colorlabels` (séquence de 0 = rouge à 4 = violet ; vérifié dans son code
  source) : en mode une couleur, écrire les deux ; en mode plusieurs couleurs, seulement
  `darktable:colorlabels` (et `xmp:Label` s'il n'y en a qu'une), sinon darktable n'en
  verrait qu'une. Avec une seule couleur, aussi `digiKam:ColorLabel` (1 rouge, 3 jaune,
  4 vert, 5 bleu, 6 magenta). Un champ à 0 n'est ajouté que s'il existait déjà (pas de
  bruit). Vérifié sur 131 vrais sidecars darktable : réécriture identique octet par octet,
  notes relues à l'identique par ExifTool, tout le reste inchangé.
- **Raccourcis de notation par défaut** (17/10/2026, pensés pour l'AZERTY aussi) : étoiles
  `0`–`5` et couleurs `6`–`9` (rouge, jaune, vert, bleu) **avec ou sans Maj** (rangée du
  haut AZERTY = Maj, pavé numérique = sans) ; violet `°`, `,` et `.` (séparateur
  décimal du pavé numérique : « , » sur macOS, « . » sous Linux Mint) ; accepté / rejeté / sans drapeau `Maj+P` / `Maj+X` / `Maj+U`. Profil vim
  aligné sur nvim : `Ctrl+U` / `Ctrl+D` pour page précédente / suivante, `u` réservé à
  l'annulation (à venir). Ctrl + lettre autorisé (seuls Cmd, Alt et Ctrl + flèches restent
  au système).
- **Raccourcis entièrement configurables en TOML** (validé le 17/10/2026, remplace l'idée
  d'un fichier JSON rangé par touche) : section `[keys]` du fichier de configuration,
  rangée par action (`"core.flag.accept" = ["Maj+P"]`), une touche (avec modificateurs),
  une séquence (`"g g"`) ou un caractère (`"°"`, pour les touches qu'egui ne nomme pas).
  Les profils restent les valeurs par défaut ; `[keys]` remplace les actions citées ; un
  raccourci attribué à deux actions est signalé au démarrage.
- Sources : spécification XMP (rating -1), exiv2 (schéma digiKam), forums Adobe et
  ExifTool (xmpDM:pick, LR 13.2), code source darktable (`src/common/exif.cc`).

## Idées pour plus tard (non urgentes, notées pour mémoire)

- **Quelques réglages rapides** sur le rendu RAW (exposition, contraste…), à terme. Pas le
  but initial : `tri` ne doit pas devenir un logiciel de développement ; ne pas remplacer
  les outils spécialisés.
- **Plugin darktable** : appliquer un préréglage darktable à une image. Piste la plus simple
  à vérifier le moment venu : `darktable-cli <RAW> <sortie> --style <nom>` (application
  d'un style en ligne de commande) ; darktable expose aussi une API Lua. Serait un plugin
  au sens du `CLAUDE.md` (statique), pas une fonction du cœur.

## Préférences de travail à respecter

- Outils open-source, self-hosted, sans abonnement, sans verrouillage propriétaire.
- **Objectif : publication open source si le projet aboutit.** La licence n'est pas encore
  choisie (MIT/Apache-2.0 ou GPL-3.0, comparaison dans `doc/licensing.md`). Toute nouvelle
  dépendance doit être vérifiée aussi sous l'angle licence et brevets, et ajoutée au tableau
  de `doc/licensing.md`. Points déjà identifiés : HEVC (HEIC iPhone) est breveté, donc un
  binaire distribué qui embarque un décodeur HEVC est exposé ; la libheif de Homebrew est liée
  à x265 (GPL), à ne jamais livrer avec un binaire sous licence permissive ; liaison
  dynamique uniquement pour les bibliothèques LGPL (LibRaw, libheif, libde265).
- **Pas de valeurs codées en dur** (demandé le 16/10/2026) : tout « potard » (délais,
  tailles, capacités, qualités, seuils, nombre de workers…) doit être un paramètre, avec sa
  valeur par défaut dans le code, regroupé dans une structure de réglages plutôt qu'en
  `const` épars. Même sans interface pour les modifier, on doit pouvoir les changer pour
  tester (fichier de configuration). Toute nouvelle valeur de ce type naît paramétrable :
  clé dans la section TOML adéquate (`src/settings.rs` pour le moteur,
  `gui/src/settings.rs` pour l'interface) **et** dans `doc/config.example.toml` (un test
  vérifie que l'exemple reste égal aux valeurs par défaut).
- Scripts/outils paramétrables plutôt que multipliés (un script avec options plutôt que N
  scripts dédiés) — vaut aussi pour l'organisation du code quand c'est pertinent.
- Décisions pragmatiques : tester avant de s'engager, ne pas optimiser avant d'avoir confirmé
  le besoin (ex. parallélisation des vignettes — à mesurer, pas à deviner).
- Avant d'adopter une dépendance externe, vérifier qu'elle est activement maintenue.
- Transparence : relire les valeurs réelles plutôt que supposer qu'une écriture a réussi.
- **Tests et documentation à chaque étape, pas après coup** :
  - tests unitaires dans chaque module (`#[cfg(test)]`), tests d'intégration dans `tests/`
    (un fichier par module, API publique uniquement) ; les tests qui ont besoin d'un vrai RAW
    sont `#[ignore]` et lisent `TRI_TEST_RAW` / `TRI_TEST_HEIC` (aucune vraie photo versionnée,
    seulement de petites fixtures synthétiques dans `tests/fixtures/`) ;
  - documentation Markdown dans `doc/` (un fichier par module + `README.md` sommaire,
    `build.md`, `tests.md`) expliquant mods, traits, fonctions et le *pourquoi* ; commentaires
    `///` sur tout l'API public pour `cargo doc` ;
  - avant chaque commit : `cargo fmt`, `cargo clippy --workspace --all-targets` sans warning,
    `cargo test --workspace` vert, **dans les deux configurations** (défaut et
    `--no-default-features`) ; pour l'interface, vérifier aussi le rendu par capture
    automatique (`TRI_GUI_SCREENSHOT`, voir `doc/gui.md`).

## Prochaine étape immédiate

- [x] Reprendre le dossier `libraw_test` (prototype JPEG embarqué) et l'étendre au rendu RAW
      complet (`libraw_dcraw_process` + `libraw_dcraw_make_mem_image`) pour obtenir un bitmap
      affichable.
- [x] `Bitmap::fit` (réduction Lanczos3) et comparaison half_size / rendu complet.
- [x] Cache persistant des aperçus (module moteur, sans UI) : `src/cache.rs`.
- [x] File de travail à priorités + progression + annulation : `src/preview.rs`,
      `src/scheduler.rs`, tests d'intégration (faux producteur + vrai RAW).
- [x] Démo CLI avec progression (`tri <dossier> --previews`, `--clear-cache`,
      `--cache-usage`) et mesures sur un vrai dossier (voir `doc/scheduler.md`).
- [x] JPEG via libjpeg-turbo (`src/jpeg.rs`, feature `turbojpeg`, défaut) : grille
      complète du dossier de test en 14–19 s au lieu de 29 s.
- Décidé le 16/10/2026 : politique « vignettes d'abord » par défaut, **réglable** (option
  `Config::thumbnails_first` du moteur, `--mixed` en CLI, `scheduler.thumbnails_first`
  dans le fichier de configuration).
- [x] Interface egui dans un paquet séparé (`gui/`, workspace Cargo) : grille de vignettes
      alimentée par le scheduler, barre d'état avec progression, sélection clavier/souris,
      filtre par type, aide, actions nommées (profils classique + vim).
- [x] Ajouter le parcours de dossier (RAW + images plates) et le filtre par type de fichier
      (`src/media.rs`, `src/filter.rs`).
- [x] Grille de vignettes : aperçus embarqués pour les RAW, image décodée pour les plates.
- [x] Plein écran : JPEG boîtier par défaut, bascule vers le rendu RAW (`b`), étiquette de ce
      qui est affiché, navigation gauche/droite avec préchargement n±1, 100 % (`z` ou
      double-clic) conservé d'une image à l'autre, retour à la grille (Échap).
- [x] Source et échelle indépendantes, 1:1 du JPEG boîtier, rendus RAW à la demande.
- [x] Retours du premier essai (16/10/2026) : 1:1 abandonné jamais redemandé (événement
      `Dropped`), préchargement des 1:1 voisins, attente affichée à l'échelle 1:1 (plus de
      passage par la vue ajustée ni d'écran noir), trace `TRI_GUI_TRACE`, `--cache`.
      Lenteur du zoom JPEG non reproduite en `--release` (0,4 s) ; confirmé disparu par
      l'utilisateur. Mode RAW zoomé : le JPEG s'affichait plusieurs secondes en changeant
      d'image (100 % passé devant le rendu ajusté) ; corrigé par l'ordre des priorités et
      d'envoi des demandes, et la mention « rendu RAW en cours ». Navigation rapide en mode
      RAW zoomé : 100 % RAW demandé seulement après 0,4 s sur l'image (les calculs lancés
      pour des images seulement traversées bloquaient l'image d'arrivée). Si ça ne suffit
      pas : piste d'un worker LibRaw dédié aux 100 %, séparé des rendus ajustés (remettrait
      en cause « un seul worker LibRaw », à discuter).
- [x] Tous les réglages paramétrables + fichier de configuration (16/10/2026, TOML choisi
      par l'utilisateur) : `src/settings.rs` (sections `[preview]`, `[render]`, `[cache]`,
      `[scheduler]`, emplacement par OS, `--config`), `gui/src/settings.rs` (`[gui]`,
      `[gui.grid]`, `[gui.view]`, `[gui.colors]`), exemple commenté
      `doc/config.example.toml`, `doc/settings.md`. Taille d'aperçu et workers « auto » ou
      fixes. Configuration invalide : réglages par défaut + avertissement (barre d'état,
      stderr). Chaque réglage de calcul n'entre que dans la clé de cache des niveaux qu'il
      influence. Restent non réglables (assumé, listé dans `doc/settings.md`) : priorités du
      scheduler, polices et marges du texte en surimpression.
- [x] Grille : survol et sélection se ressemblaient trop (retour du 16/10/2026). Premier
      choix (17/10/2026) : le survol sélectionne ; abandonné le jour même, car aller vers le
      bandeau de notation changeait l'image notée. Choix final de l'utilisateur : le clic
      sélectionne (fond turquoise), le survol n'encadre qu'en gris clair (`hover`,
      `hover_frame`) ; pointeur et cadre masqués dès qu'on tape.
- [x] Clavier (17/10/2026) : déplacement dans une image zoomée (Maj + flèches / Maj + hjkl),
      `gg` pour la première image (séquences de touches, délai réglable).
- [~] Notation XMP (17/10/2026). Fait, moteur : `src/xmp.rs` (lecture / écriture des
      sidecars, vérifiée sur de vrais sidecars darktable et par ExifTool), `src/notes.rs`
      (notes d'un dossier en mémoire, `edit`, filtres `NotesRule` sur le trait `Filter`),
      section `[xmp]`. Interface : actions et raccourcis de notation (profils), section
      `[keys]` (raccourcis par action, conflits signalés), notes affichées (grille, plein
      écran), écriture immédiate, erreurs dans la barre d'état. Filtres (choix du
      17/10/2026) : préfixe `f` au clavier + barre de filtres masquable à la souris
      (Maj+F) ; noter le RAF note aussi le JPG de la paire. Notation à la souris : bandeau
      masquable (Maj+N) et boîte modale au clic droit (ouverte au point du clic), survol
      visible (17/10/2026). Recliquer une note posée ne l'efface pas : contrôles « 0
      étoile » et « sans drapeau » à part (retour : danger en tri rapide). Notes vérifiées
      dans darktable par l'utilisateur ; autres logiciels : testeurs à trouver. À discuter plus tard : filtres
      complexes (fenêtre modale ou panneau latéral). Conversion de
      convention faite : `tri <dossier> --convert-xmp darktable|adobe [--dry-run]`.
- Feuille de route décidée avec l'utilisateur le 17/10/2026, dans cet ordre :
  1. [x] **Panneau des dossiers** à gauche (grille, masquable, Maj+D), fait le
     17/10/2026 : emplacements (volumes montés — une carte SD est dans `/Volumes`, pas
     dans `~` —, dossier personnel, racine) et arborescence lue à la demande, dans le
     moteur (`src/places.rs`, `Tree`) ; dossier ouvert = `src/folder.rs` (images, notes,
     filtres combinés). Volumes Time Machine masqués et exclusions (`[places]`),
     disque branché détecté de lui-même (`places::Monitor`), F5. Éjection d'un volume
     (`places::eject` : `diskutil eject`, `gio mount -e` / `udisksctl`, PowerShell non
     testé ; icône dans le panneau, confirmation près de l'icône) ; volumes fixés
     (`[places] fixed`) sans éjection. Dernier dossier rouvert au lancement sans
     argument (`[gui] reopen_last`, dossier de données `[data] dir`). Restent :
     navigation clavier dans l'arborescence, favoris.
  2. **Histogramme et alertes d'exposition** (sur- et sous-exposition) : sur les vraies
     données, à leur profondeur (RAW 12/14/16 bits selon le capteur, après `unpack`, sans
     dématriçage ; JPEG 8 bits ; autres formats selon leur profondeur d'enregistrement —
     `flat` ramène aujourd'hui tout en 8 bits). Calcul coûteux : à mesurer, puis cache
     d'*analyse* (pas XMP).
     Mesuré le 17/10/2026 (RAF X-T4, 26 Mpx) : `unpack` 0,35–0,55 s, histogramme complet
     14 bits ~40 ms ; noir Fuji = motif 6 × 6 (`cblack[6..]`, ~1022), blanc 16383. Comme
     les rendus, `unpack` est parallélisé par LibRaw (OpenMP) : 2 fils = 1,7 s par
     fichier, 8 fils = 9 s → **un seul worker LibRaw** pour les histogrammes aussi.
     Décisions de l'utilisateur (17/10/2026) : un RAW affiche **toujours l'histogramme
     RAW** (même quand le JPEG boîtier est à l'écran) ; échelle **en IL depuis la
     saturation** par défaut, bascule vers le linéaire ; alertes = **indicateurs**
     (pourcentages saturés / bouchés par canal) **et zones superposées sur l'image** en
     plein écran (touche) ; **badge** sur les vignettes brûlées / bouchées (seuils
     réglables). Étapes : A moteur (`src/exposure.rs`), B cache d'analyse + scheduler, C
     interface. **A fait** (17/10/2026) : `raw::Sensor` (mosaïque, noir par pixel),
     `exposure::Exposure` (histogrammes linéaire et en IL, saturés / bouchés, marge
     avant saturation, masque pivoté, format binaire), `tri <dossier> --exposure` ;
     0,38 s par RAF en moyenne, vérifié sur une vraie séance. **B fait** (17/10/2026) :
     niveau de cache `Exposure` (entrées binaires ~14 Ko), `preview::exposure`,
     scheduler (worker lourd pour un RAW, après les aperçus boîtier et avant les rendus
     RAW de fond, `[scheduler] exposure`), progression « histogrammes ». **C fait**
     (17/10/2026) : histogramme en plein écran (`i`, `Maj+I` IL / linéaire), zones
     superposées (`o`, masque en texture ; bouché = majorité des pixels d'une case),
     badges dans la grille (`[gui.histogram]`), vérifié sur une vraie séance. Zones
     colorées par canal (demandé le 17/10/2026) ; elles ont révélé que les RAF X-T4
     enregistrent un rouge / bleu saturé sous le blanc vers les bords (vignetage corrigé
     par le boîtier) : seuil de saturation par défaut à -0,02 IL. Reste à
     voir : histogramme dans la grille (panneau de droite, avec les métadonnées ?),
     profondeur réelle des TIFF / PNG 16 bits, analyse produite au passage par le rendu
     RAW (même décompression).
  3. [x] **Panneau des métadonnées** à droite (grille et plein écran, masquable,
     Maj+M), fait le 17/10/2026 : `src/metadata.rs` (LibRaw dès l'ouverture, complété
     par l'EXIF du JPEG embarqué pour la correction et le mode ; `kamadak-exif`,
     BSD-2-Clause, pour les images plates ; lignes en français), lecture en fond dans
     l'interface, `tri <dossier> --metadata`. En grille, le panneau montre aussi
     l'histogramme de l'image sélectionnée.
  4. [x] **Déchargement d'une carte** (fait le 17/10/2026, `src/offload.rs`,
     `doc/offload.md`) : choix de l'utilisateur — défauts = son usage, tout réglable ;
     dossier de séance proposé `AAAA-MM-JJ_` + champ libre, racine mémorisée d'un
     déchargement à l'autre (l'année va dans la racine ; gabarits plus tard), noms
     d'origine (renommage par gabarits plus tard), doublons jugés sur le dossier de
     destination (une carte peut être déchargée à deux endroits ; historique global en
     option), copie vérifiée (BLAKE3), puis ouvrir /
     éjecter / supprimer de la carte les fichiers vérifiés — choisis par cases à cocher
     avant la copie et enchaînés à la fin (retour du 17/10/2026), plus « RAW
     seulement ». Copie en arrière-plan possible (barre d'état), séance alors proposée
     plutôt qu'ouverte d'office. En ligne de commande :
     `tri --offload <carte> [--to] [--folder] [--dry-run] [--previews]`. À venir :
     gabarits de dossier et de noms, séances de plusieurs jours (un dossier par jour ?).
  5. Ensuite, à prioriser : actions sur les fichiers en fin de tri (déplacer les rejetées,
     copier les acceptées), éditeur externe configurable, avance automatique après une
     note (option), surveillance du dossier (fichiers ajoutés, XMP modifiés ailleurs).
  - **Demandé le 17/10/2026 (fin de journée), à faire ensuite :**
    - **Actions sur fichiers et dossiers** (supprimer, renommer, envoyer…). Pas de menu
      contextuel natif de l'OS accessible depuis egui/winit (ni Finder, ni Nautilus, ni
      Explorateur) : notre propre menu, avec « Révéler dans le gestionnaire de fichiers »
      (`open -R` macOS, `xdg-open` du dossier Linux, `explorer /select,` Windows),
      « Mettre à la corbeille » (crate `trash`, à vérifier : maintenance, licence),
      renommer, copier / déplacer vers…, ouvrir avec l'éditeur externe. Le moteur doit
      faire suivre les **sidecars XMP** et les **paires RAW+JPEG**, et le cache (clé =
      chemin) sera recalculé. Fonctions du moteur, utilisables en ligne de commande.
    - **Multilingue** : toutes les chaînes sont en français et codées en dur (interface
      *et* moteur : `Metadata::rows`, messages d'erreur). Vrai système : identifiants de
      chaînes dans le code, catalogues au moins français et anglais, langue du système
      par défaut, réglable. Piste : Fluent (`fluent-bundle`, Mozilla : pluriels et
      genres gérés, « 1 image » / « 2 images ») — à vérifier (maintenance, licence).
    - **Netteté** : overlay (carte de netteté, façon focus peaking) et score par image
      (cache d'*analyse*, comme l'exposition ; filtre « nettes seulement »). Piste :
      énergie du gradient / variance du laplacien par zone, sur la mosaïque RAW (canal
      vert) plutôt que sur le JPEG boîtier (accentuation du boîtier). Plus tard :
      visages / yeux (portrait).
    - **Passage automatique à l'image suivante après une note** (option). Piège connu
      (bug de Capture One à une époque) : sous un filtre (ex. 2 étoiles), une photo qui
      sort du filtre (2 → 1 étoile) disparaît déjà de la liste et la sélection passe
      d'elle-même à la suivante (`refilter` garde l'index) ; avancer en plus en sauterait
      une. Règle : n'avancer que si l'image notée est toujours dans la liste filtrée.
  - **Zoom continu** (demandé le 17/10/2026, pas urgent, « un confort de temps en
    temps ») : pas seulement ajusté / 1:1 ; touches `+`, `-`, `*`, `/` comme Lightroom,
    geste de pince au pavé tactile, et au-delà de 100 % au besoin. À rapprocher du
    contrat « rendu par zone / résolution demandée » (architecture cible).
  - **Annuler** (`u`, réservé) : pas une petite tâche (historique à concevoir : paires,
    filtres actifs, XMP modifiés ailleurs entre-temps ; session seule ou persistant) ; à
    reprendre plus tard.
- **Principe rappelé le 17/10/2026** : l'interface ne fait que de l'interface ; le vrai
  travail (dossiers, notes, filtres, futur déchargement) est dans le moteur, indépendant
  de l'affichage, pour un futur TUI et des commandes sans interface. Le projet est aussi
  un bac à sable pour apprendre Rust.
- [ ] Une fois ce chemin bout-en-bout validé : revenir à ce document pour décider quelle
      partie de l'architecture cible (dynamique, filtrage, JSON FFI) attaquer ensuite.

## À faire (liste tenue à jour, 17/10/2026)

Vue d'ensemble de ce qui reste, par thème ; le détail et le *pourquoi* sont dans
« Prochaine étape immédiate » ci-dessus. Cocher au fil de l'eau. ★ = demandé
explicitement par l'utilisateur comme important.

### Tri et notation
- [x] ★ **Passage automatique à l'image suivante après une note** (option) — sans sauter
      d'image sous un filtre : n'avancer que si l'image notée est toujours dans la liste.
      Fait le 17/10/2026 : `[gui] auto_advance` (désactivé par défaut), `Maj+A`, bouton
      « avance auto » du bandeau ; l'autre image d'une paire RAW+JPEG est sautée.
- [x] **Sélection multiple** (clavier et souris) : noter, déplacer, supprimer plusieurs
      images d'un coup. Décidé le 17/10/2026 : **les deux, et tout faisable au clavier**
      (« c'est ma grande frustration avec les outils du marché »). **Fait** le
      17/10/2026 : `src/selection.rs` (`doc/selection.md`), Espace, `v` (mode visuel),
      Ctrl+A / Ctrl+Maj+A, Maj+clic, Cmd/Ctrl+clic ; notes, corbeille, éditeur sur la
      sélection. Reste : déplacer / copier plusieurs images (avec « copier / déplacer
      vers… »).
- [x] **Comparaison côte à côte** (rafales), zoom et position liés ; décidé le
      17/10/2026 : **plus de 2 images** possibles. Fait le 17/10/2026 : `c` sur la
      sélection (6 au plus, `[gui.compare] max`), image active notée, `Espace` élimine,
      1:1 lié (`z`), moteur `Scheduler::set_shown`.
- [ ] **Regroupement en séries / rafales** (le `Vec<Vec<…>>` des filtres est prévu pour).
- [ ] **Annuler** (`u`, réservé) : historique à concevoir (paires, filtres actifs, XMP
      modifiés ailleurs ; session seule ou persistant).
- [ ] Filtres complexes (fenêtre modale ou panneau latéral, à discuter).

### Analyse d'image
- [ ] ★ **Netteté** : carte superposée (focus peaking) et score par image, filtrable ;
      cache d'analyse ; sur la mosaïque RAW. Plus tard : visages / yeux. Décidé le
      17/10/2026 : overlay **en 1:1 seulement** (en grille, trop gourmand et peu lisible) ;
      **2-3 couleurs configurables**, rotation de l'une à l'autre par un raccourci.
      **Overlay fait** le 17/10/2026 : `src/sharpness.rs` (`doc/sharpness.md`), Sobel à
      seuil fixe, points isolés retirés, `p` / `Ctrl+P`. Restent : score par image et
      filtre (cache d'analyse, sur la mosaïque RAW), netteté en comparaison.
      **Score sans IA fait** le 17/10/2026 (demande de l'utilisateur : « déjà virer des
      photos objectivement floues », mettre en place le mécanisme) : par cases, luminance
      étirée, jugement relatif à la médiane du dossier, cache `Level::Sharpness`, filtre
      `f b`, badge, `tri --sharpness`. **À affiner** avec les photos ratées exprès de
      l'utilisateur (bougé, mise au point ratée, fond flou volontaire, mur uni, ISO
      élevé). Pistes : mosaïque RAW (sans accentuation), comparaison dans la rafale.
      **Calibré le 17/10/2026** sur 94 photos ratées exprès (`2026-10-17_MISS`, vert /
      rouge / jaune) : règle dossier (< 0,5) **ou** voisines (< 0,8 × la médiane des 3
      avant / 3 après) → 14 rouges sur 15, 0 verte. Non détectable sans IA : mise au point
      sur l'arrière-plan (le collimateur était sur l'arrière-plan). Netteté au collimateur
      testée et écartée (dépend du contenu de la zone). Détails : `doc/sharpness.md`.
      **Faux positifs constatés** le 17/10/2026 (photos libres du manuel) : sur un
      dossier aux sujets variés, des gros plans nets sur fond très flou (papillon,
      coccinelle, oie) étaient marqués « floue » parmi des paysages très détaillés. Le
      jugement relatif suppose des scènes comparables : regroupement en rafales, ou
      normalisation par le contenu, à étudier.
      Ajouté le 17/10/2026 (retours) : filtre « floues seulement » (`f B`), carte de
      netteté resserrée (seuil 48 au lieu de 32), repère du collimateur (`m`, Fujifilm,
      carré / croix / cercle / cœur). À affiner à l'usage (l'utilisateur a étiqueté vite).
- [x] Histogramme des TIFF / PNG 16 bits à leur profondeur (`flat::decode_deep`, fait
      le 17/10/2026 ; HEIC 10 bits encore en 8 bits).
- [ ] Analyse d'exposition produite au passage par le rendu RAW (même décompression).

### Affichage
- [ ] **Zoom continu** : `+` `-` `*` `/`, pince au pavé tactile, au-delà de 100 %.
- [ ] **Loupe à 100 %** sous le pointeur, sans quitter la vue ajustée.
- [~] **Manuel des testeurs** (demandé le 17/10/2026) : `doc/manuel.md` et
      `doc/manual.md`, courts exprès (« laisser de la liberté de penser » : savoir si
      l'interface est intuitive) ; captures par `scripts/manual-screenshots.sh`. Faites
      avec des photos libres (Wikimedia Commons, CC0) ; **à refaire avec de vraies
      photos de l'utilisateur** (RAW, collimateur, histogramme RAW) : relancer le script
      sur son dossier, en `--lang fr` puis `--lang en`.
- [ ] **Fenêtre de réglages** (`Ctrl+,`, demandée le 17/10/2026) : les réglages sont
      devenus nombreux (`doc/config.example.toml`) ; les modifier depuis l'interface,
      écrits dans le fichier de configuration (sans perdre les commentaires ni les clés
      inconnues de l'utilisateur), rangés par section, avec recherche comme l'aide.
- [ ] **Jeu d'icônes** (proposé par l'utilisateur le 17/10/2026, pas urgent) : les
      icônes sont aujourd'hui dessinées en formes simples (`gui/src/marks.rs` : étoiles,
      pastilles coche / croix, goutte de netteté, panneaux, éjection…). L'utilisateur
      pourrait fournir des icônes (SVG) ; à charger alors via `egui_extras` (licence,
      maintenance à vérifier) en gardant les formes dessinées en repli.
- [x] **Aide** regroupée par thème, avec une **recherche floue « à la fzf »** (libellé,
      touches, identifiant), fait le 17/10/2026.

### Fichiers et dossiers
- [ ] ★ **Actions sur fichiers et dossiers** : notre propre menu (pas de menu natif de
      l'OS possible) — révéler dans le gestionnaire de fichiers, corbeille, renommer,
      copier / déplacer vers…, ouvrir avec l'éditeur externe ; sidecars XMP et paires
      RAW+JPEG suivent ; aussi en ligne de commande. Premier lot décidé le 17/10/2026 :
      **renommer, supprimer, ouvrir dans un éditeur externe (configurable dans le TOML),
      afficher dans le Finder** (« ce serait déjà super »). **Premier lot fait** le
      17/10/2026 : `src/files.rs` (`doc/files.md`), `[files]` (paires, corbeille, éditeur),
      `F2` / `Suppr` / `Maj+R` / `e`, boutons du clic droit, `tri --rename|--delete|
      --reveal|--edit`. Restent : copier / déplacer vers…
- [x] **Éditeur externe** configurable (`[files] editor`, fait le 17/10/2026).
- [ ] Panneau des dossiers : navigation au clavier, favoris.
- [ ] Déchargement : gabarits de noms (dossier, fichiers), cartes sur plusieurs jours
      (un dossier par jour ?), vidéos (aujourd'hui ignorées).
- [ ] Surveillance du dossier ouvert (fichiers ajoutés, XMP modifiés ailleurs).

### Internationalisation
- [x] ★ **Multilingue** : identifiants de chaînes, catalogues français et anglais
      (interface *et* moteur), langue du système par défaut, réglable ; piste Fluent.
      Choix technique laissé à Claude (17/10/2026). **Fait** le 17/10/2026 : Fluent
      (`src/i18n.rs`, `doc/i18n.md`), catalogues `locales/` et `gui/locales/`, `t!`,
      `[i18n] language` ; interface, ligne de commande, erreurs et métadonnées traduites ;
      noms de touches lus dans toutes les langues.

### Plateformes et outillage
- [ ] **Windows** : jamais compilé ni testé (build LibRaw, lettres de lecteur, éjection
      PowerShell, chemins).
- [ ] Notes XMP à faire vérifier dans digiKam, RawTherapee, ART, RapidRaw (l'utilisateur
      s'en charge) ; Lightroom et Capture One : testeurs à trouver.
- [ ] Architecture cible (LibRaw en bibliothèque dynamique, JSON à la frontière, TUI) :
      à reprendre une fois l'outil stabilisé.
