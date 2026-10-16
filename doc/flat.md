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

### `is_heif(path) -> bool`

Vrai si l'extension fait partie de `HEIF_EXTENSIONS`.

### `FlatError` et `Result<T>`

| Variante | Quand |
|---|---|
| `Io(io::Error)` | Fichier absent ou illisible |
| `Image(image::ImageError)` | Format non reconnu, fichier corrompu (hors HEIF) |
| `Heif(HeifError)` | Erreur libheif : fichier corrompu, codec absent… |
| `UnexpectedHeifLayout` | libheif n'a pas fourni de RGB 8 bits entrelacé (ne devrait pas arriver) |

`FlatError` implémente `std::error::Error`, avec `source()` qui renvoie l'erreur d'origine.

## Mécanismes internes

- **`pack_rows(data, width, height, stride)`** : libheif peut remplir la fin de chaque ligne
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

## Licences

libheif et le décodeur HEVC libde265 sont sous LGPL et liés dynamiquement. HEVC est un codec
couvert par des brevets ; c'est sans conséquence pour un usage personnel. Il faudra y penser
en cas de distribution binaire.
