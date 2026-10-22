# Paquets prêts à installer

Décidé le 19/10/2026 : des paquets que personne n'ait à compiler, pour les testeurs de
l'alpha puis la bêta publique. Un seul script, `scripts/package.py`, sur la machine du
système visé (pas de compilation croisée) ; la CI le lance sur macOS et Linux.

```sh
python3 scripts/package.py              # compile (target/package) puis empaquette
python3 scripts/package.py --no-build   # reprend la dernière compilation
python3 scripts/package.py --no-archive # dossier seulement, sans .dmg / .tar.gz
python3 scripts/package.py --sign "Developer ID Application: …"   # macOS, signé
```

Résultat dans `target/dist/` : `tri-<version>-macos-<arch>.dmg`,
`tri-<version>-linux-<arch>.tar.gz`, ou `tri-<version>-windows-x64.msi` et `.zip`.
Prérequis : ceux de la compilation
([build.md](build.md)) et [`cargo-about`](https://github.com/EmbarkStudios/cargo-about)
(`cargo install cargo-about --locked --features cli`).

## Builds de nuit

Décidé le 22/10/2026 : tant qu'on est en alpha, la CI (`.github/workflows/ci.yml`) tourne
aussi **chaque nuit** (2 h UTC) si `main` a changé depuis la dernière (le tag `nightly`
est comparé au commit courant) : tests sur les trois systèmes, paquets, puis la
**Release `nightly`** (préversion) remplacée par les paquets du jour, avec les derniers
commits en notes. Son tag n'est pas un numéro de version : la recherche de mise à jour
l'ignore. Ces binaires portent `TRI_BUILD_CHANNEL=nightly` (`release` sur un tag `v…`)
et le disent dans l'aide ([update.md](update.md)). Pour la bêta (prévu) : une branche
`candidate` pour les versions candidates, et une branche des builds de nuit.

## Ce qui est livré, et pourquoi

- **Sans HEIC** (`--no-default-features`, avec libjpeg-turbo) : brevets HEVC, voir
  [licensing.md](licensing.md).
- **Compilation à part** (`target/package`) : le binaire de développement, avec HEIC,
  n'est pas écrasé.
- **LibRaw et ses dépendances livrées avec `tri`**, en bibliothèques séparées (`.dylib`,
  `.so`) : personne n'a à installer Homebrew ni la bonne version de LibRaw, et la LGPL
  est respectée (bibliothèque remplaçable). Leurs licences sont copiées depuis le système
  de compilation (formules Homebrew, `/usr/share/doc/<paquet>/copyright` sous Debian /
  Ubuntu) dans `licenses/`, avec un `README.md` qui donne les versions et l'adresse du
  **code source de LibRaw** (obligation de la LGPL pour une bibliothèque livrée).
- **Notices des crates Rust** : `licenses/rust-crates.html`, produit par `cargo about`
  (licences acceptées dans `about.toml`, modèle `scripts/about.hbs`).
- `LICENSE` (GPL-3.0-or-later), `README.md`, `CHANGELOG.md`, `CHANGELOG.fr.md`.

## macOS : `tri.app` dans un `.dmg`

- `Contents/MacOS/tri-gui` (l'interface) et `Contents/MacOS/tri` (ligne de commande).
- `Contents/Frameworks/` : LibRaw, little-cms2, libomp, libjpeg (celle de LibRaw).
  Chaque référence est réécrite en `@rpath/…` (`install_name_tool`), le `@rpath` des
  binaires pointant vers `../Frameworks` : vérifié avec `DYLD_PRINT_LIBRARIES`, rien n'est
  chargé depuis Homebrew.
- `Info.plist` (identifiant `io.github.fkinnar.tri`, catégorie photographie), icône
  `tri.icns` tirée de `gui/assets/icon-1024.png` (`sips`, `iconutil`).
- Le `.dmg` contient l'app, un lien vers Applications et les fichiers de licence.
- Ligne de commande depuis l'app : `/Applications/tri.app/Contents/MacOS/tri`.

### Signature et notarisation

Sur Apple Silicon, tout binaire doit être signé ; modifier les bibliothèques
(`install_name_tool`) invalide leur signature. Par défaut, le script signe donc **« ad
hoc »** (sans identité). Ça suffit pour lancer l'app, mais un `.dmg` téléchargé est mis en
quarantaine : au premier lancement, macOS refuse, puis **Réglages système →
Confidentialité et sécurité → « Ouvrir quand même »** (le clic droit → Ouvrir ne suffit
plus depuis macOS 15), ou dans un terminal `xattr -dr com.apple.quarantine
/Applications/tri.app` (retire seulement la quarantaine ; `xattr -cr` efface tous les
attributs étendus, plus large que nécessaire). Acceptable pour l'alpha (testeurs
prévenus).

Pour la **bêta publique**, il faudra signer et notariser :

1. Programme Apple Developer (99 USD par an), certificat **Developer ID Application**.
2. `package.py --sign "Developer ID Application: Nom (ÉQUIPE)"` : signature avec le
   runtime renforcé (`--options runtime`) et l'horodatage, exigés par la notarisation.
   Les bibliothèques sont signées avec la même identité (pas d'exception de validation
   des bibliothèques à demander).
3. Notarisation du `.dmg` : `xcrun notarytool submit <dmg> --keychain-profile <profil>
   --wait`, puis `xcrun stapler staple <dmg>` (le ticket voyage avec le fichier).
4. En CI : certificat `.p12` et mot de passe en secrets, importés dans un trousseau
   temporaire ; identifiants de notarisation en secrets aussi.

## Linux : archive `.tar.gz` et `install.sh`

```
tri-<version>-linux-<arch>/
  bin/tri-gui, bin/tri
  lib/          LibRaw et dépendances (libraw, liblcms2, libjpeg, libgomp)
  share/applications/tri.desktop, share/icons/hicolor/256x256/apps/tri.png
  licenses/, install.sh, LICENSE, README.md, CHANGELOG.md, CHANGELOG.fr.md
```

- Les binaires cherchent leurs bibliothèques dans `../lib` (`$ORIGIN/../lib`, posé à la
  compilation), puis dans le système. En **`RPATH`** et non `RUNPATH`
  (`--disable-new-dtags`) : le `RUNPATH` ne vaut que pour les dépendances directes, et
  les dépendances de LibRaw (libjpeg, lcms2) n'étaient pas trouvées sur une distribution
  sans LibRaw. Le script vérifie le `RPATH` (`readelf -d`).
- **Vérifié le 19/10/2026** (archive compilée dans un conteneur Ubuntu 24.04, arm64) :
  installée par `install.sh` sur **Debian 13** et **Fedora 42** sans LibRaw, toutes les
  bibliothèques chargées depuis le paquet, RAF lu (`tri <dossier> --metadata`).
  L'interface reste à essayer sur un vrai bureau (Mint 22, x86_64, archive de la CI).
- **Pourquoi livrer LibRaw** : son numéro de bibliothèque change d'une distribution à
  l'autre (`libraw.so.23` sur Ubuntu 24.04, `.so.24`/`.25` ailleurs) ; un binaire lié à
  l'une ne démarre pas avec l'autre. Les dépendances moins universelles (lcms2, libjpeg,
  libgomp) la suivent ; le reste (glibc, libstdc++, X11 / Wayland, Vulkan / OpenGL) vient
  du système. Liste dans `LINUX_BUNDLED` du script ; le script vérifie avec `ldd` qu'il ne
  manque rien.
- **Distributions visées** : compilée sur Ubuntu 24.04, l'archive demande une glibc au
  moins aussi récente (2.39) : Ubuntu 24.04 et suivantes, Linux Mint 22, Debian 13, Fedora
  récentes. Pour des distributions plus anciennes : compiler sur une plus ancienne, ou un
  AppImage plus tard.
- `install.sh` copie tout dans `~/.local/lib/tri` (ou `--prefix DOSSIER`), crée les liens
  `~/.local/bin/tri-gui` et `tri`, le lanceur (`tri.desktop`, dont `Exec` pointe vers
  l'installation) et l'icône ; `install.sh --uninstall` retire le tout. Sans droits
  administrateur. Les données de `tri` (réglages, cache) ne sont jamais touchées.
- La fenêtre déclare l'identifiant d'application `tri` (Wayland), relié au lanceur
  `tri.desktop` pour l'icône et le nom.

## Windows

Ajouté le 21/10/2026 (pour qu'« une personne normale » puisse l'installer) : un
**installateur `.msi`** et un **`.zip` portable**, produits par la CI (`windows-latest`).

```
tri-<version>-windows-x64/
  tri-gui.exe, tri.exe
  raw_r.dll et ses dépendances vcpkg, runtime Visual C++ (msvcp140.dll, vcruntime140*.dll…)
  tri.ico, licenses/, LICENSE, README.md, CHANGELOG.md, CHANGELOG.fr.md
```

- **Installateur** (WiX 5, `dotnet tool install --global wix --version 5.0.2` ; la
  licence de WiX, MS-RL, ne couvre que l'outil) : installation dans `Program Files\tri`
  (pour la machine, confirmation administrateur), raccourci dans le **menu Démarrer**
  avec l'icône, désinstallation depuis les Paramètres de Windows. `UpgradeCode` fixe :
  une nouvelle version remplace la précédente. Version MSI numérique (`1.0.0` pour
  `1.0.0-alpha.2`) ; `AllowSameVersionUpgrades` remplace quand même.
- **DLL** : trouvées en lisant la table d'import des `.exe` et de chaque DLL (petit
  lecteur PE dans le script, sans `dumpbin`), cherchées dans `LIBRAW_DIR\bin` (vcpkg) et
  dans les dossiers du **runtime Visual C++ redistribuable** de Visual Studio
  (`vswhere`) ; les DLL du système (`System32`, `api-ms-*`) restent dehors. Le runtime
  est livré à côté de l'exécutable (déploiement « app-local », permis par Microsoft pour
  ces fichiers) : beaucoup de PC ne l'ont pas. Le script s'arrête si une DLL n'est
  trouvée nulle part.
- **Licences** : celles des ports vcpkg installés (`share\<port>\copyright`), adresse du
  source de LibRaw (LGPL), notices des crates Rust, comme ailleurs.
- **Pas de fenêtre de console** en release (`windows_subsystem = "windows"`,
  `gui/src/main.rs`).
- **Non signé** : SmartScreen avertit au premier lancement (« Informations
  complémentaires » → « Exécuter quand même »). Signature (certificat de signature de
  code) pour la bêta publique, comme la notarisation Apple.
- **Vérifié le 21/10/2026** : `.msi` de la CI installé sur un Windows vierge (rien
  d'installé, pas de runtime Visual C++) ; l'application se lance et affiche les images.
  Reste à passer dans toutes les fonctions.
