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

Pas de filtrage avancé, pas de notation XMP, pas de dynamique/plugins/libloading à ce stade.
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
    récemment utilisés.
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
feature `turbojpeg` (défaut), repli pur Rust sinon.
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
  `Config::thumbnails_first` du moteur, `--mixed` en CLI). À exposer dans le futur fichier
  de configuration de l'application, avec la taille « Auto » des aperçus.
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
- [ ] Fichier de configuration de l'application (taille d'aperçu Auto/fixe, politique
      `thumbnails_first`, profil de raccourcis, thème).
- [ ] Une fois ce chemin bout-en-bout validé : revenir à ce document pour décider quelle
      partie de l'architecture cible (dynamique, filtrage, JSON FFI) attaquer ensuite.
