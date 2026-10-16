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

## Ce qui est couvert

### `media` (unitaires, `src/media.rs`)

- extensions RAW et plates reconnues quelle que soit la casse ; autres fichiers ignorés ;
- fichiers cachés et AppleDouble (`._x.RAF`) ignorés ;
- listes d'extensions en minuscules et disjointes ;
- tri : le RAW précède son jumeau JPEG.

### `bitmap` (unitaires, `src/bitmap.rs`)

- lecture d'un pixel, `None` hors de l'image ;
- `fit` : proportions gardées (paysage et portrait), jamais d'agrandissement (`0` = pas de
  limite), couleur unie préservée, côté minuscule arrondi à au moins 1 pixel.

### `flat` (unitaires, `src/flat/mod.rs` et `src/flat/heif.rs`)

- extensions HEIF reconnues quelle que soit la casse ;
- *(feature `heif`)* `pack_rows` : retrait du remplissage, cas sans remplissage, dernière ligne sans
  remplissage, rejet des données trop courtes et des strides incohérents.

### `flat` (intégration, `tests/flat.rs`)

- PNG, TIFF, WebP (sans perte) générés à la volée : décodés au pixel près ;
- format détecté d'après le contenu (PNG nommé `.jpg`) ;
- JPEG avec balise EXIF `Orientation = 6` : dimensions et couleurs pivotées ;
- HEIC (fixture) : dimensions et couleur de chaque quadrant ; extension en majuscules et
  chemin non ASCII ;
- fichier absent → `Io` ; contenu invalide → `Image` ou `Heif` ;
- sans la feature `heif` : la fixture HEIC donne `HeifNotCompiled` ;
- avec `TRI_TEST_HEIC` : un vrai HEIC d'iPhone se décode en RGB8 cohérent.

### `filter` (unitaires, `src/filter.rs`)

- `ungrouped`, `KindFilter` (tout / RAW / plates), suppression des groupes vidés, valeur par
  défaut, chaînage `apply_all`.

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
shim C), `half_size` divise les dimensions par deux, rendu dans un autre thread.
