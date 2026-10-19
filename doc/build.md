# Compilation et plateformes

## Prérequis

À installer **avant la première compilation** (une seule commande par plateforme) :

| Plateforme | Installation |
|---|---|
| Ubuntu ≥ 24.04 / Mint ≥ 22 | `sudo apt install libraw-dev libheif-dev libheif-plugin-libde265 zlib1g-dev cmake nasm pkg-config build-essential` |
| macOS (Homebrew) | `brew install libraw libheif cmake pkg-config` (+ outils Xcode en ligne de commande : `xcode-select --install`) |
| Windows (non testé) | `vcpkg install libraw:x64-windows libheif:x64-windows`, CMake et NASM, puis `LIBRAW_DIR` (voir plus bas) |

Plus Rust lui-même ([rustup](https://rustup.rs/), édition 2024). Le script
[`prerequisites.sh`](../prerequisites.sh) rappelle ces commandes. Commande Linux vérifiée sur
Mint 22.3 (16/10/2026). Commande macOS vérifiée le 17/10/2026 : `cmake` est bien
nécessaire depuis que libjpeg-turbo est compilée avec le projet (sur Apple Silicon, NASM
n'est pas requis : les optimisations SIMD ARM n'en ont pas besoin).

### Pourquoi ces paquets

`tri` lie deux bibliothèques système :

- [LibRaw](https://www.libraw.org/) (API C), pour les RAW ;
- [libheif](https://github.com/strukturag/libheif) **≥ 1.17**, pour HEIC/HEIF, avec son
  plugin de décodage HEVC (libde265).

libjpeg-turbo (feature `turbojpeg`) n'est **pas** prise au système : elle est compilée
depuis les sources embarquées par le crate `turbojpeg-sys` (voir plus bas), ce qui demande
CMake et NASM.

Un compilateur C est aussi nécessaire, pour le petit shim `csrc/libraw_shim.c`.

`zlib1g-dev` : sous Ubuntu/Mint, `libheif.pc` déclare zlib dans `Requires.private` ;
sans ce paquet, pkg-config refuse libheif (« Package 'zlib', required by 'libheif', not
found ») alors que `libheif-dev` est bien installé.

### Version de l'API libheif

`libheif-rs` 3 vise par défaut la toute dernière API (libheif ≥ 1.23), absente des
distributions LTS (Mint 22 / Ubuntu 24.04 : 1.17.6). `Cargo.toml` fixe donc la feature
`v1_17`, le minimum requis par `tri`. À relever seulement si on utilise une fonction plus
récente de libheif.

### libheif trop ancienne

Debian 12 et Ubuntu 22.04 fournissent une libheif < 1.17, refusée à la compilation. Dans ce
cas, `libheif-rs` peut compiler libheif lui-même (CMake requis), en activant sa feature
`embedded-libheif` dans `Cargo.toml` :

```toml
libheif-rs = { version = "3", default-features = false, features = ["v1_17", "embedded-libheif"], optional = true }
```

Pour un binaire **distribué**, voir [licensing.md](licensing.md) : la libheif de Homebrew
est liée à x265 (GPL) et HEVC est breveté.

libheif est localisée par le crate `libheif-sys` lui-même (pkg-config, ou vcpkg sous Windows
MSVC). Le `build.rs` de `tri` ne concerne que LibRaw.

## Workspace

Le dépôt est un workspace Cargo à deux paquets : `tri` (moteur + démo en ligne de commande, à
la racine) et `tri-gui` (interface egui, dans `gui/`). Le moteur ne dépend pas d'egui.

```sh
cargo build --release --workspace             # tout
cargo run --release -p tri-gui -- <dossier>   # l'interface
cargo test --workspace                        # tous les tests
```

`tri-gui` transmet les features `heif` et `turbojpeg` au moteur (`--no-default-features`
s'applique donc aux deux).

Sous Linux, l'interface (eframe : winit + wgpu) n'a besoin d'**aucun paquet de
développement supplémentaire** : les bibliothèques X11/Wayland/xkbcommon et le pilote
Vulkan/OpenGL sont chargés à l'exécution, pas liés (vérifié sur Mint 22.3 sans
`libxkbcommon-dev`, `libwayland-dev` ni `libxcb-*-dev`). Une session graphique normale suffit.

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
| `turbojpeg` | activée | JPEG via libjpeg-turbo, compilée depuis ses sources et liée statiquement. Désactivée, repli sur le décodeur/encodeur pur Rust du crate `image`, plus lent : voir [jpeg.md](jpeg.md) |
| `heif` | activée | Décodage HEIC/HEIF via libheif. Désactivée (`--no-default-features`), libheif n'est plus requise ni liée : voir [flat.md](flat.md) et [licensing.md](licensing.md) |

Avec `--no-default-features`, seule LibRaw reste nécessaire comme bibliothèque système.

### libjpeg-turbo compilée avec le projet

Le crate `turbojpeg` 1.x utilise l'API TurboJPEG 3 et exige **libjpeg-turbo ≥ 3.0**. Les
distributions LTS fournissent encore la 2.1 (Mint 22 / Ubuntu 24.04 : 2.1.5), refusée à la
compilation. D'où le choix (16/10/2026) d'activer la feature `cmake` de `turbojpeg-sys` sur
toutes les plateformes : il compile les sources qu'il embarque (libjpeg-turbo 3.1.0) en
bibliothèque **statique**. Même version partout, aucun paquet libjpeg-turbo à installer,
Windows compris. Coût : CMake et NASM (extensions SIMD x86 ; `require-simd` fait échouer la
compilation plutôt que de produire en silence une version lente), et ~20 s de compilation
la première fois. Licences permissives, liaison statique sans contrainte autre que
l'attribution (voir [licensing.md](licensing.md)).

Pour utiliser malgré tout une libjpeg-turbo ≥ 3.0 du système, `turbojpeg-sys` lit des
variables d'environnement :

```sh
TURBOJPEG_SOURCE=pkg-config cargo build --release        # via pkg-config (libturbojpeg)
TURBOJPEG_SOURCE=explicit TURBOJPEG_LIB_DIR=… TURBOJPEG_INCLUDE_DIR=… cargo build --release
```

## Variables d'environnement

| Variable | Rôle |
|---|---|
| `LIBRAW_DIR` | Racine d'une installation LibRaw (`include/`, `lib/`) ; court-circuite pkg-config |
| `LIBRAW_LIB` | Nom de la bibliothèque à lier avec `LIBRAW_DIR` (défaut : `raw`) |
| `TRI_TEST_RAW` | Fichier RAW utilisé par les tests d'intégration (voir [tests.md](tests.md)) |
| `TRI_TEST_HEIC` | Vrai HEIC (iPhone) utilisé par un test d'intégration optionnel |

## Vérifier le code pour Windows depuis macOS ou Linux (19/10/2026)

Sans machine Windows, on vérifie au moins que tout le code Rust (moteur, interface,
tests) se compile et passe clippy pour Windows :

```sh
rustup target add x86_64-pc-windows-msvc        # une fois
TRI_CHECK_ONLY=1 CARGO_FEATURE_PURE=1 cargo clippy --workspace --all-targets \
  --no-default-features --target x86_64-pc-windows-msvc --target-dir /tmp/tri-win
```

- `cargo check` / `clippy` ne lient rien : LibRaw n'est pas nécessaire ;
- `TRI_CHECK_ONLY` : `build.rs` saute le shim C de LibRaw (il faudrait le compilateur
  de la cible) ;
- `CARGO_FEATURE_PURE` : blake3 sans son assembleur (même raison), pour la
  vérification seulement ;
- `--no-default-features` : sans libjpeg-turbo ni libheif, compilées en C.

Premier passage le 19/10/2026 : tout se compile, clippy propre (une fonction de test
réservée à Unix marquée comme telle). Restent à vérifier sur une vraie machine : la
compilation des bibliothèques C (LibRaw via `LIBRAW_DIR`, libjpeg-turbo, libheif) et
l'exécution (chemins, lettres de lecteur, éjection, corbeille).

## Builds sur macOS, Linux et Windows : GitHub Actions (19/10/2026)

`.github/workflows/ci.yml` : sur les trois systèmes, dépendances (apt, Homebrew, vcpkg
pour LibRaw sous Windows), `cargo fmt --check`, clippy (`-D warnings`), tests, binaires
en release, déposés en artefacts avec `LICENSE`, `README.md`, `CHANGELOG.md`. Sans HEIC
(feature `heif` retirée : brevets HEVC, voir `licensing.md`), avec libjpeg-turbo.

Lancé à la main (onglet Actions, ou `gh workflow run ci.yml`) ou à chaque version
(`git tag v1.0.0-alpha.1 && git push --tags`), pas à chaque envoi : dépôt privé, minutes
gratuites limitées. Restent à faire pour des versions prêtes à installer : application
macOS (`.app` avec LibRaw et libomp dedans, noms d'installation réécrits), archive Linux
(ou AppImage), zip Windows avec les DLL de vcpkg, et les notices des dépendances.
