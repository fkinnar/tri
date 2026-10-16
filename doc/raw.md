# Module `tri::raw`

Lecture des fichiers RAW via LibRaw : extraction de l'aperçu JPEG embarqué (rapide, pour les
vignettes) et développement complet des données du capteur (pour l'affichage plein écran).

## Pourquoi l'API C de LibRaw

LibRaw est écrite en C++, mais l'ABI C++ n'est pas stable et Rust ne peut pas l'appeler
directement. LibRaw fournit une API C (`libraw_*`) qui l'enveloppe ; c'est elle qu'on utilise
via FFI.

## Organisation

- **`ffi.rs`** (privé) : déclarations `unsafe extern "C"` des fonctions LibRaw utilisées, et
  les deux types qui traversent la frontière :
  - `LibRawData` : handle opaque (`libraw_data_t*`), jamais déréférencé côté Rust ;
  - `ProcessedImage` : miroir `#[repr(C)]` de `libraw_processed_image_t`, une struct terminée
    par un tableau de taille variable (`data[1]` en C) qui contient les pixels ou le JPEG.
- **`mod.rs`** : l'API sûre, décrite ci-dessous. Tout l'`unsafe` est confiné ici.
- **`csrc/libraw_shim.c`** : deux setters C (`tri_libraw_set_use_camera_wb`,
  `tri_libraw_set_half_size`) pour des champs de `params` que l'API C n'expose pas.

## API publique

### `RawFile`

Un fichier RAW ouvert, qui possède une instance LibRaw.

```rust
let mut raw = RawFile::open("DSCF7171.RAF")?;
let thumb = raw.thumbnail()?;                      // aperçu embarqué
let bitmap = raw.render(&RenderOptions::default())?; // développement complet
```

- `open(path)` : `libraw_init` puis ouverture du fichier. Le chemin est passé en octets bruts
  sous Unix (les noms non UTF-8 fonctionnent), en UTF-16 sous Windows. Seul le format est
  analysé à ce stade (~1 à 5 ms) : les données du capteur ne sont pas encore lues.
- `thumbnail()` : `libraw_unpack_thumb` → `libraw_dcraw_make_mem_thumb`. Renvoie l'aperçu
  produit par le boîtier, sans aucun calcul. ~10 ms sur un RAF X-T4.
- `render(&options)` : règle les paramètres, puis `libraw_unpack` → `libraw_dcraw_process` →
  `libraw_dcraw_make_mem_image`. Renvoie un `Bitmap` RGB 8 bits, **déjà pivoté** selon
  l'orientation du boîtier.
- On peut appeler `thumbnail()` puis `render()` sur le même `RawFile`.
- **Libération** : `Drop` appelle `libraw_close`, y compris si une étape a échoué.
- **Threads** : `RawFile` est `Send` (une instance LibRaw n'est liée à aucun thread, on peut la
  déplacer vers un thread de travail) mais pas `Sync` (deux threads ne doivent pas l'utiliser
  en même temps). Des instances distinctes peuvent travailler en parallèle.

### `Thumbnail`

```rust
pub enum Thumbnail { Jpeg(Vec<u8>), Bitmap(Bitmap) }
```

Presque tous les boîtiers embarquent un JPEG (`Jpeg`, octets du fichier JPEG tels quels).
Certains anciens formats fournissent un bitmap brut, converti en `Bitmap`.

**Attention :** l'aperçu embarqué n'est *pas* pivoté. Un portrait apparaît en paysage, avec
une balise EXIF `Orientation` (souvent 6 ou 8) qu'il faut appliquer à l'affichage.

### `Bitmap`

Type commun défini dans [`tri::bitmap`](bitmap.md) : RGB 8 bits entrelacé, sans remplissage.

### `RenderOptions`

| Champ | Défaut | Effet |
|---|---|---|
| `camera_wb` | `true` | Balance des blancs enregistrée par le boîtier. Sinon, multiplicateurs « lumière du jour » de LibRaw (rendu souvent trop chaud ou trop froid). |
| `half_size` | `false` | Demi-résolution : chaque bloc de photosites devient un pixel, sans dématriçage. Environ 10× plus rapide. |
| `bright` | `1.0` | Luminosité appliquée par LibRaw. |

La sortie est toujours en 8 bits par canal (`libraw_set_output_bps(8)`).

### `RawError` et `Result<T>`

| Variante | Quand |
|---|---|
| `Init` | `libraw_init` a renvoyé un pointeur nul (mémoire) |
| `InvalidPath(path)` | Chemin contenant un octet nul, impossible à passer en C |
| `LibRaw { code, message }` | Code d'erreur LibRaw, avec son message (`libraw_strerror`) : fichier absent, format non supporté, fichier corrompu… |
| `EmptyImage` | LibRaw a renvoyé une image nulle sans code d'erreur |
| `UnsupportedImage { kind, colors, bits }` | Image produite dans un format que `Bitmap` ne gère pas |

`RawError` implémente `Display` et `std::error::Error` : il fonctionne avec `?` vers
`Box<dyn Error>`.

### `version()`

Version de la LibRaw effectivement chargée (ex. `0.22.2-Release`).

## Mécanismes internes

- **`check(code)`** : transforme un code de retour LibRaw (0 = succès) en `Result`.
- **`MemImage`** : enveloppe une `ProcessedImage` allouée par LibRaw ; son `Drop` appelle
  `libraw_dcraw_clear_mem`. Les données sont copiées dans un `Vec` Rust avant libération.
- **`bitmap_from_processed(header, bytes)`** : conversion pure (sans FFI, donc testable)
  d'une image LibRaw en `Bitmap`. Accepte le RGB 8 bits et les niveaux de gris 8 bits
  (étendus en RGB) ; rejette le reste et les données tronquées.

## Mesures

X-T4 (RAF 26 Mpx, capteur X-Trans), Mac Apple Silicon, build release :

| Opération | Temps |
|---|---|
| `open` | 1 à 5 ms |
| `thumbnail` (JPEG 4416×2944) | ≈ 10 ms |
| `render`, pleine résolution (4170×6246) | ≈ 3,8 s |
| `render`, `half_size` (2085×3123) | ≈ 0,35 s |

Le dématriçage X-Trans est particulièrement coûteux ; les capteurs Bayer sont plus rapides.

**Un rendu est déjà parallèle en interne** (≈ 440 % CPU sur un seul `render`) et une instance
en pleine résolution occupe ~200 Mo. Lancer plusieurs rendus complets en même temps ne fait
pas gagner de temps, au contraire : neuf rendus prennent 5,7 s en série, 18 s sur 4 threads et
302 s sur un thread par cœur (contention CPU et mémoire). L'extraction des aperçus embarqués
n'a pas ce problème : elle ne fait aucun calcul.

## Outil de test

```sh
cargo run --release --example decode -- thumb  photo.RAF aperçu.jpg
cargo run --release --example decode -- render photo.RAF rendu.png [--half] [--daylight] [--bright X]
```

Affiche la version de LibRaw, les dimensions et les temps de chaque étape.
