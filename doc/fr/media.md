# Module `tri::media`

Parcours d'un dossier et classification de chaque fichier image en **RAW** (à développer via
LibRaw) ou **image plate** (JPEG, TIFF, PNG… déjà développée, simplement décodée).

## Types

### `MediaKind`

```rust
pub enum MediaKind { Raw, Flat }
```

La nature d'un fichier, déduite de son extension. Elle détermine le traitement : aperçu
embarqué + `raw::RawFile::render` pour `Raw`, décodage direct pour `Flat`.

### `MediaFile`

```rust
pub struct MediaFile { pub path: PathBuf, pub kind: MediaKind }
```

Un fichier trouvé par `scan`. `path` inclut la racine du parcours. C'est l'unité manipulée par
les filtres (voir [filter.md](filter.md)).

## Fonctions

### `kind_of(path) -> Option<MediaKind>`

Classe un chemin d'après son extension, **sans tenir compte de la casse** (`.RAF`, `.raf`,
`.Raf`). Renvoie `None` pour :

- les extensions non gérées (`.xmp`, `.mov`, `.txt`…) et les fichiers sans extension ;
- les **fichiers cachés** (nom commençant par `.`). Sur une carte SD lue par macOS, chaque
  photo peut avoir un double `._DSCF0001.RAF` (fichier AppleDouble de métadonnées, quelques
  Ko) qui porte la même extension mais n'est pas une image.

Les listes d'extensions sont publiques : `RAW_EXTENSIONS` (formats photo gérés par LibRaw :
`raf`, `cr2`, `cr3`, `nef`, `arw`, `dng`, `orf`, `rw2`…) et `FLAT_EXTENSIONS` (`jpg`,
`jpeg`, `jpe`, `png`, `tif`, `tiff`, `webp`, `heic`, `heif`, `hif`). Elles sont en minuscules et disjointes (vérifié
par un test).

Les formats vidéo RAW (`.r3d`, `.braw`) sont volontairement exclus. AVIF n'est pas listé :
selon la distribution, libheif n'a pas toujours de décodeur AV1. Le décodage des images plates
est décrit dans [flat.md](flat.md).

### `scan(root, depth) -> io::Result<Vec<MediaFile>>`

Liste les images de `root` (via `walkdir`) :

- `depth` : `1` = le dossier seul, `2` = un niveau de sous-dossiers, etc. ; `0` = rien ;
  `usize::MAX` = sans limite (ex. une carte entière `DCIM/100_FUJI/…`).
- seuls les fichiers sont retenus (un dossier nommé `x.RAF` est ignoré) ; les liens
  symboliques ne sont pas suivis ;
- **tri** : par chemin sans extension, puis RAW avant image plate. Les paires RAW+JPEG d'un
  même cliché se suivent donc toujours dans le même ordre (`DSCF0001.RAF`, `DSCF0001.JPG`,
  `DSCF0002.RAF`…) ;
- **erreurs** : `Err` si `root` n'existe pas, est illisible ou n'est pas un dossier
  (`ErrorKind::NotADirectory`). Les entrées illisibles *à l'intérieur* (droits, lien cassé)
  sont ignorées : un fichier problématique ne doit pas empêcher d'afficher le reste.

Le parcours est fait en entier et en mémoire (pas d'itérateur paresseux) : le tri l'exige, et
quelques milliers d'entrées représentent un coût négligeable.

### Clones darktable : une vignette chacun

Un clone (« duplicate ») darktable n'a pas de fichier à lui, seulement son sidecar :
`IMG_01.RAF.xmp` pour le clone n° 1 de `IMG.RAF`. Le parcours le liste comme une image
`IMG_01.RAF` (choix de l'utilisateur, 29/10/2026 : une vignette par clone, à côté de
l'original), avec ses propres notes et son propre développement, puisque son sidecar est
exactement celui que `tri` lit pour ce nom. Le parcours le reconnaît aux noms seuls, sans
accès disque de plus : un sidecar `NOM_NN.EXT.xmp` sans image `NOM_NN.EXT`, dont l'image
`NOM.EXT` est dans le même dossier. Une vraie image de ce nom (`IMG_0001.CR2` d'un
Canon) reste une image.

Les pixels, eux, sont ceux de l'original : `source(path)` donne le fichier à lire
(l'original pour un clone, `path` sinon). LibRaw (`RawFile::open`), les images plates
(`flat::decode*`), les métadonnées, l'empreinte du cache (`cache::stamp`) et le rendu par
darktable (l'original avec le sidecar du clone) passent par elle. `is_clone(path)` dit si
`path` est un clone, `clone_name("IMG_01.RAF")` donne `("IMG.RAF", 1)`. Les clones connus
sont retenus au parcours (registre en mémoire, chemin du clone → original).

## Démonstration

Le binaire `tri` liste un dossier, avec filtre optionnel :

```sh
cargo run --release -- ~/Pictures/Import/2026-09-21            # tout
cargo run --release -- ~/Pictures/Import/2026-09-21 --raw      # RAW seuls
cargo run --release -- ~/Pictures/Import/2026-09-21 --flat --depth 3
```
