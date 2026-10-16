# Documentation de `tri`

`tri` est une visionneuse RAW rapide pour le tri après une séance photo (équivalent libre de
FastRawViewer), pour Linux, macOS et, si possible, Windows. Le contexte et les décisions
d'architecture sont dans [`CLAUDE.md`](../CLAUDE.md) ; ce dossier documente le code tel qu'il
existe.

## Sommaire

| Document | Contenu |
|---|---|
| [build.md](build.md) | Prérequis, compilation par plateforme, variables d'environnement |
| [media.md](media.md) | Module `tri::media` : parcours de dossier, RAW / images plates |
| [filter.md](filter.md) | Module `tri::filter` : trait `Filter`, groupes, `KindFilter` |
| [raw.md](raw.md) | Module `tri::raw` : lecture et développement des RAW via LibRaw |
| [tests.md](tests.md) | Organisation et lancement des tests |

## Vue d'ensemble du code

```
tri/
├── build.rs              localise LibRaw, compile le shim C, émet les directives de lien
├── csrc/libraw_shim.c    accès aux paramètres LibRaw sans setter dans l'API C
├── src/
│   ├── lib.rs            crate bibliothèque (le « moteur ») : déclare les modules
│   ├── main.rs           démo en ligne de commande : liste un dossier filtré
│   ├── media.rs          parcours de dossier, classification RAW / plate
│   ├── filter.rs         trait Filter, groupes, KindFilter
│   └── raw/
│       ├── ffi.rs        déclarations brutes de l'API C LibRaw (privé)
│       └── mod.rs        API sûre : RawFile, Bitmap, Thumbnail, RenderOptions, RawError
├── examples/raw.rs       outil en ligne de commande pour tester le module raw
└── tests/                tests d'intégration (un fichier par module)
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
