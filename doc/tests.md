# Tests

## Organisation

- **Tests unitaires** : dans chaque module, sous `#[cfg(test)] mod tests`. Ils testent la
  logique interne, y compris les fonctions privées (ex. `bitmap_from_processed`), sans
  dépendre de fichiers externes.
- **Tests d'intégration** : dans `tests/`, un fichier par module (`tests/raw.rs`…). Ils
  utilisent uniquement l'API publique, comme le ferait l'interface. Les fichiers temporaires
  passent par le crate `tempfile` (supprimés automatiquement).
- **Fixtures** : `tests/fixtures/` ne contient que de petits fichiers synthétiques, impossibles
  à générer depuis Rust sans dépendance lourde. `quadrants.heic` (856 octets, 64×48, quadrants
  rouge / vert / bleu / blanc) a été produit par :
  `heif-enc -q 95 -p chroma=444 quadrants.png -o quadrants.heic`.

## Lancer les tests

```sh
cargo test                       # tout ce qui ne nécessite pas de fichier externe
TRI_TEST_RAW=/chemin/photo.RAF TRI_TEST_HEIC=/chemin/photo.heic \
  cargo test --release -- --include-ignored                               # tout
```

Les deux configurations de features doivent passer :

```sh
cargo test                          # avec HEIF (défaut)
cargo test --no-default-features    # sans HEIF
```

Les tests HEIF sont `#[cfg(feature = "heif")]`. Sans la feature, un test dédié vérifie qu'un
HEIC donne `HeifNotCompiled`.

Les tests qui ont besoin d'un vrai fichier sont marqués `#[ignore = "nécessite TRI_TEST_…"]` :
aucun fichier RAW n'est versionné (trop lourd, et propre à chaque boîtier). `cargo test` les
signale comme ignorés au lieu de les faire passer en silence. Avec `--include-ignored`, ils
échouent clairement si la variable n'est pas définie. `--release` est conseillé : le
développement RAW est beaucoup plus lent en debug.

Les tests sur vrai RAW sont **sérialisés** par un `Mutex` statique (`serial()` dans
`tests/raw.rs`) : chaque rendu LibRaw occupe déjà plusieurs cœurs, et les lancer en parallèle
les rendait environ 50× plus lents (302 s au lieu de 5,7 s).

Tout le workspace : `cargo test --workspace` (et `--no-default-features`). Les tests de
l'interface sont décrits dans [gui.md](gui.md#tests).

## Ce qui est couvert

### `media` (unitaires, `src/media.rs`)

- extensions RAW et plates reconnues quelle que soit la casse ; autres fichiers ignorés ;
- fichiers cachés et AppleDouble (`._x.RAF`) ignorés ;
- listes d'extensions en minuscules et disjointes ;
- tri : le RAW précède son jumeau JPEG.

### `bitmap` (unitaires, `src/bitmap.rs`)

- lecture d'un pixel, `None` hors de l'image ;
- `rotate` : position de chaque pixel pour 90°, 180° et 270°, quatre quarts de tour = identité ;
- `box_downsample` : moyenne exacte des blocs, bords ignorés ;
- `fit` en deux temps : écart moyen < 2 niveaux par rapport à un Lanczos3 direct ;
- `fit` : proportions gardées (paysage et portrait), jamais d'agrandissement (`0` = pas de
  limite), couleur unie préservée, côté minuscule arrondi à au moins 1 pixel.

### `flat` (unitaires, `src/flat/mod.rs` et `src/flat/heif.rs`)

- extensions HEIF reconnues quelle que soit la casse ;
- *(feature `heif`)* `pack_rows` : retrait du remplissage, cas sans remplissage, dernière ligne sans
  remplissage, rejet des données trop courtes et des strides incohérents.

### `flat` (intégration, `tests/flat.rs`)

- PNG, TIFF, WebP (sans perte) générés à la volée : décodés au pixel près ;
- format détecté d'après le contenu (PNG nommé `.jpg`) ;
- JPEG avec balise EXIF `Orientation = 6` : dimensions et couleurs pivotées, aussi avec
  `decode_fit` (réduction puis orientation) ;
- HEIC (fixture) : dimensions et couleur de chaque quadrant ; extension en majuscules et
  chemin non ASCII ;
- fichier absent → `Io` ; contenu invalide → `Image` ou `Heif` ;
- sans la feature `heif` : la fixture HEIC donne `HeifNotCompiled` ;
- avec `TRI_TEST_HEIC` : un vrai HEIC d'iPhone se décode en RGB8 cohérent.

### `cache` (unitaires, `src/cache.rs`)

- emplacement par défaut par OS (HOME, XDG_CACHE_HOME absolu ou relatif, LOCALAPPDATA),
  variables absentes ;
- clé : dépend de chaque champ, déterministe, champs non ambigus, **valeur figée**
  (`key_hash_is_stable`, à ne modifier qu'avec `FORMAT_VERSION`) ;
- fichiers temporaires uniques, dans le dossier de la cible.

### `cache` (intégration, `tests/cache.rs`)

- aller-retour JPEG (dimensions, écart moyen faible), écrasement, suppression ;
- clé changée par une nouvelle date de modification ou une nouvelle taille ;
- clé dépendant du niveau et des paramètres, identique en chemin relatif et absolu ;
- source absente → erreur ;
- niveaux isolés, vidage d'un niveau ou de tout, cache utilisable après vidage ;
- `usage` ignore les temporaires ; `evict` supprime le moins récemment utilisé (lecture =
  usage) ;
- entrée corrompue supprimée par `get` ;
- 4 threads écrivant et lisant la même clé : jamais d'entrée partielle ;
- qualité JPEG ramenée entre 1 et 100, et qualité basse ⇒ entrée plus petite.

### `preview` (unitaires, `src/preview.rs`)

- règle `half_size` (seuil exact, orientation indifférente, marge) ; paramètres de clé
  distincts par niveau et par taille ; chaque réglage ne change la clé que des niveaux qu'il
  influence ; annulation avant démarrage.

### `settings` (unitaires `src/settings.rs`, intégration `tests/settings.rs`)

- emplacement par défaut par OS (XDG absolu ou relatif, variables absentes) ;
- fichier vide = réglages par défaut, égaux à ceux des types du moteur ; fichier partiel ;
- `Auto` : `"auto"` ou entier positif, refus de `"automatique"`, `-1`, `1.5`, `true`, d'un
  entier trop grand ; 0 worker ramené à 1 ;
- clé inconnue signalée avec sa ligne, mauvais type signalé, section `[gui]` ignorée par le
  moteur ;
- fichier valide lu ; fichier invalide ou introuvable : réglages par défaut et avertissement
  nommant le fichier et la clé ; dossier et qualité du cache appliqués ;
- **`doc/config.example.toml` égal aux valeurs par défaut** (moteur ici, fichier entier dans
  `gui/src/settings.rs`).

### `scheduler` (unitaires et intégration)

Voir [scheduler.md](scheduler.md#tests). Les tests de concurrence ont été relancés en
boucle (300 à 500 fois) après chaque correction : un test qui passe une fois sur dix ne
prouve rien.

### `jpeg` (unitaires, `src/jpeg.rs`)

- choix du facteur de réduction (jamais sous `min_side`, `0` = pleine taille, image déjà
  petite, orientation indifférente) ;
- aller-retour encodage/décodage en 4:4:4 et 4:2:0 ; décodage réduit (1/4 avec turbojpeg,
  pleine taille sans) ; contenu invalide refusé.
- Ces tests passent dans les deux configurations : ils vérifient aussi le repli.

### `filter` (unitaires, `src/filter.rs`)

- `ungrouped`, `KindFilter` (tout / RAW / plates), suppression des groupes vidés, valeur par
  défaut, chaînage `apply_all`.

### `xmp` (unitaires, `src/xmp.rs`)

- `Notes` : étoiles plafonnées sans toucher au drapeau, une couleur (remplace / retire),
  plusieurs couleurs (triées).
- Réécriture sans changement identique octet par octet (sidecar darktable réaliste) ; une
  seule valeur changée ne modifie qu'elle ; mise à jour complète en gardant historique,
  mots-clés et champs inconnus ; rejet qui mémorise et restaure les étoiles ; plusieurs
  couleurs (darktable seul) ; préfixe `xap:` et forme élément ; valeurs écrites par
  d'autres logiciels (Lightroom, digiKam, « 3.0 ») ; `rdf:Description` vide ; XML mal formé
  refusé ; nouveau paquet ; noms de sidecar des deux conventions.

### `xmp` (intégration, `tests/xmp.rs`)

Dans un dossier temporaire : nouveau sidecar selon la convention, sidecar le plus récent
choisi, extension `.XMP` en majuscules, notes inchangées ⇒ fichier non réécrit (date
intacte), sidecar mal formé jamais écrasé, sidecar Adobe partagé par un RAF et son JPG,
aucun fichier temporaire laissé. Conversion : darktable → Adobe (simulation sans
écriture, source intacte, pas de sidecar vide), conflit d'une paire (le RAW l'emporte),
Adobe → darktable (un sidecar partagé recopié pour les deux images), cible à jour rendue
la plus récente, source mal formée signalée.

### `notes` (unitaires `src/notes.rs`, intégration `tests/notes.rs`)

- Règles d'étoiles, de couleurs (dont « sans couleur »), combinaison des critères, règle
  vide ; image inconnue.
- Dans un dossier temporaire : chargement (sidecar mal formé signalé sans arrêter le
  reste), modification écrite puis relue du disque, paire RAW+JPEG en convention Adobe
  (les deux rafraîchies) et darktable (même note dans les deux sidecars, casse ignorée,
  `IMG_01` à part), paire différente réalignée, sidecar mal formé dans la paire signalé
  sans bloquer le reste, sidecar mal formé jamais modifié,
  `reload` après une modification extérieure, filtres sur un dossier parcouru par
  `media::scan`.

### `places` (unitaires `src/places.rs`, intégration `tests/places.rs`)

- Dossiers des volumes par OS (macOS, Linux avec et sans `USER`, Windows), dossier
  personnel, parents à déplier, noms cachés.
- Dans un dossier temporaire : sous-dossiers triés sans casse, cachés exclus ou non,
  fichiers écartés, lien vers un dossier suivi, dossier absent en erreur ; emplacements de
  la machine (personnel, racine, chemins absolus) ; arborescence dépliée à la demande,
  repliée puis redépliée (le sous-niveau reste déplié), `reveal`, `reload` après création,
  dossier illisible signalé ; volumes Time Machine reconnus à leur contenu (APFS, HFS,
  option désactivée), exclusion par nom ou chemin, motifs `*` et `?` ; surveillance : volume
  « branché » puis retiré dans un dossier temporaire, signalé une seule fois, intervalle nul
  sans fil ; volumes fixés non éjectables (casse ignorée, seuls les volumes s'éjectent,
  `Row::ejectable`, `Tree::can_eject`) ; périphérique d'un point de montage lu dans `/proc/mounts` (espaces).

### `folder` (unitaires `src/folder.rs`, intégration `tests/folder.rs`)

Aucun dossier par défaut ; ouverture (chemin absolu, images, notes), filtres par type et
par notes combinés, note relue après réouverture, dossier absent en erreur.

### `media` + `filter` (intégration, `tests/media.rs`)

Sur une fausse carte SD temporaire (paires RAW+JPEG, casse mixte, AppleDouble, `.xmp`,
sous-dossiers, dossier nommé `x.RAF`) : contenu et ordre exacts de `scan`, effet de `depth`
(0, 1, 2, 3, illimité), dossier vide, dossier absent, fichier passé comme racine, filtre par
type sur la carte.

### `raw` (unitaires, `src/raw/mod.rs`)

- conversion RGB8, niveaux de gris → RGB, octets superflus ignorés ;
- rejet des données tronquées, du JPEG, du 16 bits, d'un nombre de canaux inattendu ;
- traduction des codes d'erreur LibRaw ;
- valeurs par défaut de `RenderOptions`, version de LibRaw.

### `raw` (intégration, `tests/raw.rs`)

Sans fichier RAW : fichier absent, fichier non RAW, chemin non ASCII transmis intact.

Avec `TRI_TEST_RAW` : aperçu JPEG valide (marqueur `FF D8`), rendu RGB8 cohérent et non noir,
aperçu puis rendu sur le même handle, effet réel de l'option balance des blancs (vérifie le
shim C), `half_size` divise les dimensions par deux, rendu dans un autre thread, `size()`
cohérent avec le rendu, aperçu embarqué pivoté par `orientation()` dans le même sens que le
rendu, annulation (drapeau déjà levé, puis levé depuis un autre thread pendant un rendu
complet).
