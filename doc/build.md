# Compilation et plateformes

## Prérequis

`tri` lie deux bibliothèques système :

- [LibRaw](https://www.libraw.org/) (API C), pour les RAW ;
- [libheif](https://github.com/strukturag/libheif) **≥ 1.17**, pour HEIC/HEIF, avec son
  plugin de décodage HEVC (libde265).

Un compilateur C est aussi nécessaire, pour le petit shim `csrc/libraw_shim.c`.

| Plateforme | Installation |
|---|---|
| Ubuntu ≥ 24.04 / Mint ≥ 22 | `sudo apt install libraw-dev libheif-dev libheif-plugin-libde265 libturbojpeg0-dev pkg-config build-essential` |
| macOS (Homebrew) | `brew install libraw libheif jpeg-turbo pkg-config` (+ outils Xcode en ligne de commande) |
| Windows (non testé) | `vcpkg install libraw:x64-windows libheif:x64-windows libjpeg-turbo:x64-windows`, puis `LIBRAW_DIR` et `TURBOJPEG_*` (voir plus bas) |

Le script [`prerequisites.sh`](../prerequisites.sh) rappelle ces commandes.

### libheif trop ancienne

Debian 12 et Ubuntu 22.04 fournissent une libheif < 1.17, refusée à la compilation. Dans ce
cas, `libheif-rs` peut compiler libheif lui-même (CMake requis), en activant sa feature
`embedded-libheif` dans `Cargo.toml` :

```toml
libheif-rs = { version = "3", features = ["embedded-libheif"] }
```

Pour un binaire **distribué**, voir [licensing.md](licensing.md) : la libheif de Homebrew
est liée à x265 (GPL) et HEVC est breveté.

libheif est localisée par le crate `libheif-sys` lui-même (pkg-config, ou vcpkg sous Windows
MSVC). Le `build.rs` de `tri` ne concerne que LibRaw.

## Ce que fait `build.rs`

1. **Localise LibRaw.**
   - Si `LIBRAW_DIR` est définie : on utilise `$LIBRAW_DIR/include` et `$LIBRAW_DIR/lib`, et
     la lib `raw` (nom surchargeable par `LIBRAW_LIB`, ex. `libraw`). Hors MSVC, on ajoute la
     bibliothèque standard C++.
   - Sinon : `pkg-config libraw`.
2. **Compile le shim C** (`csrc/libraw_shim.c`) avec le crate `cc`, en utilisant les mêmes
   chemins d'en-têtes : la struct `libraw_data_t` est ainsi toujours vue avec le bon layout.
3. **Émet les directives de lien**, *après* le shim : avec `ld` (Linux), une bibliothèque
   statique doit apparaître avant celles dont elle dépend.

### Particularité macOS

`pkg-config` renvoie `-lstdc++` (libstdc++ de GNU), alors que macOS utilise `libc++`.
`build.rs` remplace `stdc++` par `c++` quand la cible est macOS. Sous Linux, `stdc++` est
correct tel quel.

### Windows

Pas de pkg-config en pratique. Exemple avec vcpkg :

```powershell
$env:LIBRAW_DIR = "C:\vcpkg\installed\x64-windows"
cargo build --release
```

La DLL LibRaw doit être trouvable à l'exécution (dans le `PATH` ou à côté de l'exécutable).
Les chemins de fichiers sont transmis à LibRaw en UTF-16 (`libraw_open_wfile`), ce qui gère
les noms non ASCII.

## Features Cargo

| Feature | Défaut | Effet |
|---|---|---|
| `turbojpeg` | activée | JPEG via libjpeg-turbo (pkg-config `libturbojpeg`). Désactivée, repli sur le décodeur/encodeur pur Rust du crate `image`, plus lent : voir [jpeg.md](jpeg.md) |
| `heif` | activée | Décodage HEIC/HEIF via libheif. Désactivée (`--no-default-features`), libheif n'est plus requise ni liée : voir [flat.md](flat.md) et [licensing.md](licensing.md) |

Avec `--no-default-features`, seule LibRaw reste nécessaire comme bibliothèque système.

### libjpeg-turbo hors pkg-config (Windows)

Le crate `turbojpeg-sys` lit des variables d'environnement :

```powershell
$env:TURBOJPEG_SOURCE = "explicit"
$env:TURBOJPEG_LIB_DIR = "C:\vcpkg\installed\x64-windows\lib"
$env:TURBOJPEG_INCLUDE_DIR = "C:\vcpkg\installed\x64-windows\include"
```

Il sait aussi compiler libjpeg-turbo depuis ses sources (feature `cmake` du crate, CMake et
NASM requis), non activé ici pour rester cohérent avec LibRaw et libheif (bibliothèques du
système).

## Variables d'environnement

| Variable | Rôle |
|---|---|
| `LIBRAW_DIR` | Racine d'une installation LibRaw (`include/`, `lib/`) ; court-circuite pkg-config |
| `LIBRAW_LIB` | Nom de la bibliothèque à lier avec `LIBRAW_DIR` (défaut : `raw`) |
| `TRI_TEST_RAW` | Fichier RAW utilisé par les tests d'intégration (voir [tests.md](tests.md)) |
| `TRI_TEST_HEIC` | Vrai HEIC (iPhone) utilisé par un test d'intégration optionnel |
