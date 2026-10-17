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
| [settings.md](settings.md) | Fichier de configuration (TOML) : emplacement, sections, erreurs ; module `tri::settings` |
| [config.example.toml](config.example.toml) | Toutes les clés avec leur valeur par défaut, commentées |
| [media.md](media.md) | Module `tri::media` : parcours de dossier, RAW / images plates |
| [filter.md](filter.md) | Module `tri::filter` : trait `Filter`, groupes, `KindFilter` |
| [cache.md](cache.md) | Module `tri::cache` : cache persistant des vignettes et aperçus |
| [preview.md](preview.md) | Module `tri::preview` : calcul des vignettes et aperçus |
| [scheduler.md](scheduler.md) | Module `tri::scheduler` : file de travail en fond |
| [jpeg.md](jpeg.md) | Module `tri::jpeg` : JPEG rapide (libjpeg-turbo, repli pur Rust) |
| [exposure.md](exposure.md) | Module `tri::exposure` : histogrammes sur les données du capteur, saturés / bouchés, masque |
| [files.md](files.md) | Module `tri::files` : renommer, corbeille, afficher, éditeur externe (paires et sidecars suivent) |
| [sharpness.md](sharpness.md) | Module `tri::sharpness` : carte de netteté (focus peaking) en 1:1 |
| [selection.md](selection.md) | Module `tri::selection` : sélection multiple (marques, mode visuel, cibles des actions) |
| [folder.md](folder.md) | Module `tri::folder` : dossier ouvert (images, notes, liste filtrée) |
| [offload.md](offload.md) | Module `tri::offload` : déchargement d'une carte (copie vérifiée, historique, suppression) |
| [places.md](places.md) | Module `tri::places` : emplacements (volumes, dossier personnel, racine) et arborescence |
| [metadata.md](metadata.md) | Module `tri::metadata` : métadonnées de prise de vue (LibRaw, EXIF), lignes à afficher |
| [notes.md](notes.md) | Module `tri::notes` : notes d'un dossier en mémoire, modification, filtres sur les notes |
| [xmp.md](xmp.md) | Module `tri::xmp` : notes (étoiles, couleurs, accepté/rejeté) dans les sidecars XMP |
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
│   ├── files.rs          renommer, corbeille, afficher, éditeur externe
│   ├── bitmap.rs         Bitmap : image RGB8 commune
│   ├── cache.rs          cache persistant sur disque (clés BLAKE3, JPEG)
│   ├── preview.rs        calcul des vignettes, aperçus et 1:1 (réglages de calcul)
│   ├── scheduler.rs      file de travail en fond : priorités, progression, annulation
│   ├── selection.rs      sélection multiple : marques, mode visuel, cibles
│   ├── sharpness.rs      carte de netteté (focus peaking)
│   ├── settings.rs       fichier de configuration : sections du moteur, lecture
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
