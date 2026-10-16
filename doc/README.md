# Documentation de `tri`

`tri` est une visionneuse RAW rapide pour le tri après une séance photo (équivalent libre de
FastRawViewer), pour Linux, macOS et, si possible, Windows. Le contexte et les décisions
d'architecture sont dans [`CLAUDE.md`](../CLAUDE.md) ; ce dossier documente le code tel qu'il
existe.

## Sommaire

| Document | Contenu |
|---|---|
| [gui.md](gui.md) | Interface egui : grille, plein écran, actions et raccourcis |
| [build.md](build.md) | Prérequis, compilation par plateforme, variables d'environnement |
| [media.md](media.md) | Module `tri::media` : parcours de dossier, RAW / images plates |
| [filter.md](filter.md) | Module `tri::filter` : trait `Filter`, groupes, `KindFilter` |
| [cache.md](cache.md) | Module `tri::cache` : cache persistant des vignettes et aperçus |
| [preview.md](preview.md) | Module `tri::preview` : calcul des vignettes et aperçus |
| [scheduler.md](scheduler.md) | Module `tri::scheduler` : file de travail en fond |
| [jpeg.md](jpeg.md) | Module `tri::jpeg` : JPEG rapide (libjpeg-turbo, repli pur Rust) |
| [bitmap.md](bitmap.md) | Module `tri::bitmap` : type d'image RGB8 commun |
| [flat.md](flat.md) | Module `tri::flat` : décodage JPEG, PNG, TIFF, WebP, HEIC |
| [raw.md](raw.md) | Module `tri::raw` : lecture et développement des RAW via LibRaw |
| [tests.md](tests.md) | Organisation et lancement des tests |
| [licensing.md](licensing.md) | Licences des dépendances, brevets HEVC, options de distribution |

## Vue d'ensemble du code

```
tri/
├── build.rs              localise LibRaw, compile le shim C, émet les directives de lien
├── csrc/libraw_shim.c    accès aux paramètres LibRaw sans setter dans l'API C
├── src/
│   ├── lib.rs            crate bibliothèque (le « moteur ») : déclare les modules
│   ├── main.rs           démo en ligne de commande : liste, aperçus en fond, cache
│   ├── media.rs          parcours de dossier, classification RAW / plate
│   ├── filter.rs         trait Filter, groupes, KindFilter
│   ├── bitmap.rs         Bitmap : image RGB8 commune
│   ├── cache.rs          cache persistant sur disque (clés BLAKE3, JPEG)
│   ├── jpeg.rs           JPEG rapide : libjpeg-turbo (feature) ou crate image
│   ├── flat/
│   │   ├── mod.rs        décodage des images plates (crate image)
│   │   └── heif.rs       HEIC/HEIF via libheif (feature `heif`)
│   └── raw/
│       ├── ffi.rs        déclarations brutes de l'API C LibRaw (privé)
│       └── mod.rs        API sûre : RawFile, Thumbnail, RenderOptions, RawError
├── gui/                  paquet tri-gui : l'interface egui (voir gui.md)
├── examples/decode.rs    outil en ligne de commande : aperçu / rendu RAW, décodage plat
└── tests/                tests d'intégration (un fichier par module)
    └── fixtures/         petits fichiers de test synthétiques (HEIC)
```

Le moteur (`src/lib.rs` et ses modules) ne dépend d'aucune bibliothèque d'affichage : il
produit des données (`Bitmap`, listes de fichiers) que l'interface consomme.

## Documentation générée

Chaque élément public porte aussi un commentaire `///` (imposé par `#![warn(missing_docs)]`). La documentation API navigable
s'obtient avec :

```sh
cargo doc --no-deps --open
```

Ces fichiers Markdown expliquent le *pourquoi* et l'enchaînement ; `cargo doc` reste la
référence pour les signatures exactes.
