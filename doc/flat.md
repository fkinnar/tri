# Module `tri::flat`

Décodage des images « plates » — déjà développées par l'appareil ou un logiciel — en
[`Bitmap`](bitmap.md) RGB8. Aucun développement : on lit simplement les pixels.

| Format | Extensions | Décodeur |
|---|---|---|
| JPEG | `jpg`, `jpeg`, `jpe` | crate `image` |
| PNG | `png` | crate `image` |
| TIFF | `tif`, `tiff` | crate `image` |
| WebP | `webp` | crate `image` |
| HEIF / HEIC | `heic`, `heif`, `hif` | libheif, via le crate `libheif-rs` |

`heic` : iPhone. `hif` : extension HEIF utilisée par les boîtiers Fujifilm récents.

## Feature `heif`

Le support HEIF est une **feature Cargo**, activée par défaut :

```sh
cargo build --release                         # avec HEIF (libheif requise)
cargo build --release --no-default-features   # sans HEIF : aucun lien vers libheif / HEVC
```

Pourquoi : HEVC est breveté et la libheif de Homebrew embarque x265, sous GPL (voir
[licensing.md](licensing.md)). Un binaire distribué doit pouvoir s'en passer.

Tout le code libheif est dans `src/flat/heif.rs`, compilé seulement avec la feature. Sans
elle :

- `decode` sur un fichier HEIF renvoie `FlatError::HeifNotCompiled` (« HEIF non pris en
  charge »), sans même lire le fichier ;
- les variantes `FlatError::Heif` et `UnexpectedHeifLayout` n'existent pas ;
- `HEIF_SUPPORTED` vaut `false`, ce qui permet à l'interface d'adapter son message ;
- `media::scan` **liste quand même** les fichiers HEIC. Ils existent, et l'interface doit
  pouvoir dire « présent, mais non décodable par ce binaire » plutôt que de les cacher.

## API

### `decode(path) -> Result<Bitmap>`

Décode le fichier et **applique l'orientation** : le `Bitmap` renvoyé est dans le bon sens,
prêt à afficher.

- **HEIF** (choisi par extension, insensible à la casse) : le fichier est lu en mémoire puis
  passé à libheif (`HeifContext::read_from_bytes`). On évite ainsi `read_from_file`, qui
  n'accepte que des chemins UTF-8. libheif applique d'elle-même les transformations stockées
  dans le fichier (rotation `irot`, miroir `imir`, recadrage `clap`). On demande un plan RGB
  8 bits entrelacé ; les sources 10 bits sont converties par libheif.
- **Autres formats** : le format est **détecté d'après le contenu**, pas d'après l'extension
  (un PNG nommé `.jpg` est décodé correctement). L'orientation EXIF est lue puis appliquée.
  Une balise EXIF illisible est ignorée (image affichée sans rotation) plutôt que de faire
  échouer le décodage.

### `decode_fit(path, max_side) -> Result<Bitmap>`

Comme `decode`, mais réduit l'image à `max_side` (voir `Bitmap::fit` ; `0` = pas de
réduction). Pour les formats décodés par `image`, l'orientation EXIF est appliquée *après*
la réduction : tourner une vignette coûte beaucoup moins cher que tourner une image de
26 Mpx. `decode(path)` équivaut à `decode_fit(path, 0)`. Utilisé pour les vignettes et les
aperçus standard des images plates.

### `is_heif(path) -> bool`

Vrai si l'extension fait partie de `HEIF_EXTENSIONS`, que la feature soit compilée ou non.

### `HEIF_SUPPORTED: bool`

`true` si ce binaire a été compilé avec la feature `heif`.

### `FlatError` et `Result<T>`

| Variante | Quand |
|---|---|
| `Io(io::Error)` | Fichier absent ou illisible |
| `Image(image::ImageError)` | Format non reconnu, fichier corrompu (hors HEIF) |
| `HeifNotCompiled` | Fichier HEIF, binaire compilé sans la feature `heif` |
| `Heif(HeifError)` | *(feature `heif`)* Erreur libheif : fichier corrompu, codec absent… |
| `UnexpectedHeifLayout` | *(feature `heif`)* libheif n'a pas fourni de RGB 8 bits entrelacé (ne devrait pas arriver) |

`FlatError` implémente `std::error::Error`, avec `source()` qui renvoie l'erreur d'origine.

## Mécanismes internes

- **`heif::decode`** (`src/flat/heif.rs`) : lecture en mémoire, contexte libheif, image
  primaire, décodage en RGB entrelacé.
- **`heif::pack_rows(data, width, height, stride)`** : libheif peut remplir la fin de chaque ligne
  pour l'alignement mémoire (`stride` > `width * 3`). Cette fonction pure recopie les lignes
  sans le remplissage, et rejette un stride incohérent ou des données trop courtes.

## Mesures

Mac Apple Silicon, build release :

| Image | Temps |
|---|---|
| HEIC iPhone 3024×4032 | ≈ 0,2 s |
| JPEG Fujifilm 4160×6240 (orientation appliquée) | ≈ 0,35 s |

Pour la grille, décoder l'image entière puis la réduire coûte cher. Les pistes, à mesurer
avant de choisir : la vignette EXIF embarquée dans les JPEG, la vignette HEIF
(`thumbnail_ids`), ou un décodage JPEG à résolution réduite.

## Licences et brevets

libheif et le décodeur HEVC libde265 sont sous LGPL, liés dynamiquement. **HEVC est couvert
par des brevets** : sans conséquence pour un usage personnel, mais à traiter avant de
distribuer des binaires. La libheif de Homebrew est aussi liée à l'encodeur x265, sous GPL.
D'où la feature `heif`, désactivable. Le détail et les autres options (décodeur du système sous
macOS ou Windows…) sont dans [licensing.md](licensing.md).
