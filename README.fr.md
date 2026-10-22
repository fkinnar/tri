# tri

**Visionneuse RAW rapide pour trier une séance photo**, sous macOS, Linux et
Windows : vignettes de tout un dossier en quelques secondes, plein écran en rendu boîtier
ou en vrai rendu RAW, notes (étoiles, couleurs, accepté / rejeté) écrites en XMP et lues
par darktable, digiKam, Lightroom…, filtres, piles d'images semblables, netteté,
histogramme RAW, zoom et loupe, déchargement de cartes. Entièrement pilotable au
clavier (jeu « classique » ou « vim »), à la souris aussi.

> **Version alpha** (`1.0.0-alpha.3`) : à tester sur une **copie** de vos photos. Les
> notes vont dans des fichiers `.xmp` à côté des images ; renommer, déplacer ou supprimer
> agit sur les vrais fichiers (annulable avec `Ctrl+Z`, ou `u` en vim, pendant la
> session).

*[English version](README.md)*

## Installer

**Paquets prêts à installer**, sur la [page Releases](https://github.com/fkinnar/tri/releases)
(sans HEIC, voir [doc/fr/packaging.md](doc/fr/packaging.md)). Entre deux versions, la
préversion **`nightly`** porte les paquets du `main` de la nuit :

- **macOS** (`.dmg`) : glisser `tri.app` dans Applications. L'app n'est pas encore
  signée par Apple : au premier lancement, macOS refuse ; ouvrir alors **Réglages
  système → Confidentialité et sécurité → « Ouvrir quand même »**, ou retirer la
  quarantaine dans un terminal : `xattr -dr com.apple.quarantine /Applications/tri.app`.
- **Linux** (`.tar.gz`, Ubuntu 24.04 / Mint 22 ou plus récent) : décompresser, puis
  `./install.sh` (dans `~/.local`, sans droits administrateur ; `--uninstall` pour
  retirer). `tri` apparaît dans le menu des applications.
- **Windows** (`.msi`, 64 bits) : lancer l'installateur ; `tri` apparaît dans le menu
  Démarrer (désinstallation depuis les Paramètres de Windows). Pas encore signé :
  SmartScreen avertit, cliquer **« Informations complémentaires » → « Exécuter quand
  même »**. Ou le `.zip` portable : décompresser, puis lancer `tri-gui.exe`.

**Compiler depuis les sources** :

1. Rust (stable récent, édition 2024) : <https://rustup.rs>
2. Les bibliothèques système, voir [`prerequisites.sh`](prerequisites.sh) :
   - macOS : `brew install libraw libheif cmake pkg-config`
   - Linux Mint 22 / Ubuntu 24.04 : `sudo apt install libraw-dev libheif-dev
     libheif-plugin-libde265 zlib1g-dev cmake nasm pkg-config build-essential`
3. `cargo build --release -p tri-gui`, puis `target/release/tri-gui [dossier]`.

Détails (Windows, options, HEIC) : [doc/fr/build.md](doc/fr/build.md).

## Premiers pas

- [Premiers pas](doc/fr/manual.md) (court exprès : le reste est à découvrir),
  [first steps](doc/en/manual.md) en anglais.
- `F1` ou `?` : toutes les touches, avec une recherche. `Ctrl+,` : les réglages (dont le
  jeu de raccourcis, le thème « gris neutre », la langue).
- Notes : `0`–`5` étoiles, `6`–`9` couleurs, `Maj+P` / `Maj+X` accepté / rejeté.
- `Entrée` plein écran, `z` 100 %, `b` rendu boîtier ou rendu RAW, `/` rechercher.

## Documentation

Tout est dans [`doc/fr/`](doc/fr/README.md) (en anglais : [`doc/en/`](doc/en/README.md)) :
un fichier par module, le pourquoi des choix, les réglages
([`doc/config.example.toml`](doc/config.example.toml), commenté), les licences et brevets
([`doc/fr/licensing.md`](doc/fr/licensing.md)). Historique des versions :
[CHANGELOG.md](CHANGELOG.md). La suite, et comment aider :
[ROADMAP.fr.md](ROADMAP.fr.md).

## Licence

GPL-3.0-or-later : voir [LICENSE](LICENSE). Les bibliothèques LGPL (LibRaw, libheif)
sont liées dynamiquement ; les binaires distribués ne contiennent pas de décodeur HEVC
(voir [doc/fr/licensing.md](doc/fr/licensing.md)).
