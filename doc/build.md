# Compilation et plateformes

## Prérequis

`tri` lie la bibliothèque système [LibRaw](https://www.libraw.org/) (API C). Un compilateur C
est aussi nécessaire, pour le petit shim `csrc/libraw_shim.c`.

| Plateforme | Installation |
|---|---|
| Debian / Ubuntu / Mint | `sudo apt install libraw-dev pkg-config build-essential` |
| macOS (Homebrew) | `brew install libraw pkg-config` (+ outils Xcode en ligne de commande) |
| Windows (non testé) | `vcpkg install libraw:x64-windows` puis `LIBRAW_DIR` (voir plus bas) |

Le script [`prerequisites.sh`](../prerequisites.sh) rappelle ces commandes.

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

## Variables d'environnement

| Variable | Rôle |
|---|---|
| `LIBRAW_DIR` | Racine d'une installation LibRaw (`include/`, `lib/`) ; court-circuite pkg-config |
| `LIBRAW_LIB` | Nom de la bibliothèque à lier avec `LIBRAW_DIR` (défaut : `raw`) |
| `TRI_TEST_RAW` | Fichier RAW utilisé par les tests d'intégration (voir [tests.md](tests.md)) |
