# Licences, brevets et distribution

> **Avertissement :** ce document résume ce qu'on a pu vérifier et ce qui est communément
> admis. Ce n'est pas un avis juridique. Avant toute distribution de binaires, en particulier
> commerciale ou par une organisation, il faut faire vérifier ces points.

L'objectif est de publier `tri` en open source s'il aboutit. Ce document recense ce que ça
implique, pour que le choix de la licence et le mode de distribution soient faits en
connaissance de cause et pas découverts après coup.

**Résumé :**

1. Le code Rust et les crates utilisés n'imposent presque rien (MIT, Apache-2.0…).
2. LibRaw, libheif et libde265 sont sous **LGPL** : la liaison dynamique est compatible avec
   n'importe quelle licence de projet, à condition de respecter quelques obligations si on les
   livre avec le binaire.
3. **Le vrai sujet est HEVC** (le codec des HEIC d'iPhone), couvert par des **brevets** :
   c'est un problème *distinct* de la licence du code. Il concerne la distribution de
   *binaires* capables de décoder du HEVC, pas le code source de `tri`.
4. La libheif de Homebrew embarque **x265 (GPL)**. Elle ne doit pas être redistribuée telle
   quelle avec un binaire de `tri` sous licence non GPL.

---

## 1. Licences des dépendances

### Bibliothèques système (liées dynamiquement)

Relevées le 16/10/2026 sur les paquets Homebrew installés (`brew info`) et les liens réels
(`otool -L`).

| Bibliothèque | Version | Licence | Rôle dans `tri` |
|---|---|---|---|
| LibRaw | 0.22.2 | **LGPL-2.1-only OU CDDL-1.0** (au choix) | Décodage RAW |
| ↳ libomp (OpenMP) | — | Apache-2.0 WITH LLVM-exception | Parallélisme interne de LibRaw |
| ↳ little-cms2 | 2.19.1 | MIT | Gestion couleur de LibRaw |
| ↳ jpeg-turbo | 3.2.0 | IJG AND Zlib AND BSD-3-Clause | JPEG dans LibRaw |
| libheif | 1.23.6 | **LGPL-3.0-or-later** | Conteneur HEIF/HEIC |
| ↳ libde265 | 1.1.3 | **LGPL-3.0-or-later** | Décodeur **HEVC** (brevets, voir §2) |
| ↳ **x265** | 4.3 | **GPL-2.0-or-later** | Encodeur HEVC : **inutile pour `tri`**, mais lié par la libheif de Homebrew |
| ↳ aom | 3.15.2 | BSD-2-Clause | AV1 (AVIF), inutilisé pour l'instant |
| libjpeg-turbo (`libturbojpeg`) | 3.1.0 | IJG AND Zlib AND BSD-3-Clause | JPEG rapide (feature `turbojpeg`), aucun brevet actif. **Compilée et liée statiquement** (sources embarquées par `turbojpeg-sys`, voir [build.md](build.md)) : licences permissives, seule obligation l'attribution (mentions IJG et BSD dans la documentation d'un binaire distribué) |

**LibRaw** : on retient l'option LGPL-2.1. La CDDL est incompatible avec la GPL ; la LGPL
garde toutes les portes ouvertes.

### Crates Rust (compilés dans le binaire)

Relevé via `cargo metadata` (et complété à chaque ajout : `blake3` est sous CC0-1.0 OR
Apache-2.0, `turbojpeg` et `turbojpeg-sys` sous Unlicense OR MIT, `cmake` — outil de compilation seulement — sous MIT OR Apache-2.0, `serde` et `toml` — fichier
de configuration — sous MIT OR Apache-2.0, comme leurs dépendances `toml_parser`,
`toml_datetime`, `serde_spanned`, sauf `winnow`, sous MIT ; `objc2-foundation` — macOS, emplacement d'un fichier mis à la corbeille pour
l'annulation, déjà présent par `trash` — sous MIT ; `objc2` et `objc2-app-kit` — macOS,
envoi par AirDrop (feuille de partage, `tri::send`), même projet (madsmtm/objc2, actif),
déjà présents par winit — sous Zlib OR Apache-2.0 OR MIT ; `windows-sys` — Windows,
éjection d'un volume (`places::eject`), Microsoft (microsoft/windows-rs, très actif),
déjà présent par winit et d'autres — sous MIT OR Apache-2.0 ; `toml_edit` — fenêtre de
réglages : modifier le fichier sans perdre commentaires ni mise en page, mêmes
mainteneurs que `toml` (toml-rs), très actif, déjà présent en dépendance indirecte — sous
MIT OR Apache-2.0, avec `indexmap` et `toml_writer` sous MIT OR Apache-2.0 ; `quick-xml` — sidecars XMP —
sous MIT, avec `memchr` sous Unlicense OR MIT ; `kamadak-exif` — EXIF des images
plates et du JPEG embarqué des RAW — sous BSD-2-Clause, avec `mutate_once` sous
BSD-2-Clause ; `trash` — corbeille du système, maintenu, version du 13/09/2026 — sous
MIT, avec `objc2`, `objc2-foundation`, `objc2-encode`, `urlencoding` sous MIT et
`percent-encoding`, `scopeguard`, `once_cell`, `chrono`, `windows` sous MIT OR
Apache-2.0 ; `fluent-bundle`, `fluent-syntax`, `fluent-langneg`, `intl-memoizer`,
`intl_pluralrules`, `unic-langid` — traductions — sous MIT OR Apache-2.0, avec `tinystr`
sous Unicode-3.0 et `self_cell` sous Apache-2.0 OR GPL-2.0-only, dont on retient
l'Apache-2.0 ; `sys-locale` — langue du système — sous MIT OR Apache-2.0 ; `moxcms` —
profils ICC des exports sans darktable, gestion couleur en pur Rust, déjà compilé par
`image` (même version), activement maintenu (0.9.1 du 15/09/2026) — sous BSD-3-Clause OR
Apache-2.0 ; `serde_json` — versions (recadrages) dans le XMP,
un objet JSON par élément, dtolnay, très maintenu (1.0.154 du 11/10/2026) — sous MIT OR
Apache-2.0 ; `mdns-sd` — voisinage réseau (Bonjour / mDNS), pur Rust sans runtime
asynchrone, keepsimple1/mdns-sd, actif (0.21.6 du 19/10/2026), sans ses options par
défaut — sous Apache-2.0 OR MIT, avec `flume` sous Apache-2.0 / MIT, `if-addrs` sous MIT
OR BSD-3-Clause, `socket-pktinfo` sous MIT, `mio`, `socket2`, `fastrand` sous MIT (OR
Apache-2.0)) : les 89 crates
de l'arbre de
dépendances du moteur sur macOS (`cargo tree -p tri -e normal,build`, recompté le
17/10/2026) sont sous licences
permissives : MIT, Apache-2.0, BSD, Zlib, Unlicense, presque toujours en double licence
`MIT OR Apache-2.0`. Aucun n'est sous copyleft. `libheif-rs` et `libheif-sys` sont sous MIT :
ce sont des *bindings*, la libheif elle-même reste LGPL.

**Interface (`tri-gui`)** : egui, eframe, winit, wgpu et leurs dépendances, ~370 crates de
plus, toutes permissives (MIT, Apache-2.0, BSD, Zlib, ISC, BSL-1.0, Unicode-3.0). Trois
crates en double licence, où l'on retient l'option permissive : `r-efi` (MIT OR Apache-2.0
OR LGPL-2.1-or-later) et `self_cell` (Apache-2.0 OR GPL-2.0-only). Les polices par défaut
d'egui (`epaint_default_fonts`) sont sous **OFL-1.1** et **Ubuntu Font Licence** : licences
de polices qui permettent de les embarquer dans un logiciel sous n'importe quelle licence,
avec leur notice. En développement seulement : `egui_kittest` (MIT OR Apache-2.0).

Obligation pratique : reproduire les notices de copyright et de licence de ces crates dans la
distribution. Un outil comme [`cargo-about`](https://github.com/EmbarkStudios/cargo-about)
génère ce fichier automatiquement. **Adopté le 19/10/2026** (MIT OR Apache-2.0, Embark
Studios, version 0.9.2 d'août 2026 : maintenu) : outil de construction seulement, rien
n'entre dans le binaire ; `about.toml`, `scripts/package.py`, voir
[packaging.md](packaging.md).

### Ce qu'impose la LGPL (LibRaw, libheif, libde265)

Avec une **liaison dynamique** (ce que fait `tri`), la LGPL n'impose rien sur la licence de
`tri` lui-même. Si on **livre** ces bibliothèques avec le binaire (bundle `.app` macOS,
installeur Windows, AppImage), il faut :

- fournir leur code source, ou une offre écrite de le fournir, pour la version livrée ;
- inclure le texte de leur licence ;
- permettre à l'utilisateur de remplacer la bibliothèque par une autre version : c'est
  naturellement le cas avec une `.dylib`/`.so`/`.dll` séparée ;
- ne pas interdire, dans la licence de `tri`, la rétro-ingénierie nécessaire pour déboguer ces
  modifications.

Avec une **liaison statique**, il faudrait en plus fournir les objets de `tri` permettant de
relier une autre version. On l'évite : liaison dynamique uniquement. Note : le module LibRaw
doit de toute façon devenir une bibliothèque dynamique chargée par `libloading` (voir
`CLAUDE.md`).

Si on ne livre pas ces bibliothèques (paquet de distribution Linux qui dépend de `libraw` et
`libheif` du système), ces obligations reviennent à la distribution, pas à nous.

### Le piège x265 (GPL)

`otool -L` montre que la `libheif.dylib` de Homebrew est **liée directement à `libx265`**,
sous GPL-2.0-or-later. Ce n'est pas un plugin chargé à la demande : c'est une dépendance dure.

- **En développement** : aucun problème, rien n'est distribué.
- **En distribution** : livrer cette libheif avec `tri` revient à livrer du code GPL lié au
  programme. Si `tri` est sous licence permissive (MIT/Apache), c'est un problème. Et x265 ne
  sert à rien ici : il ne fait qu'*encoder* du HEVC.
- **Solution** : pour un binaire distribué, compiler libheif soi-même **sans encodeur**
  (options CMake `WITH_X265=OFF` ; libde265 seul pour le décodage), ou passer par le décodeur
  du système (voir §3).

Sous Linux, à partir de libheif 1.17, les codecs sont des plugins séparés
(`libheif-plugin-libde265`, `libheif-plugin-x265`…) : on dépend seulement du plugin de
décodage, et c'est l'utilisateur ou la distribution qui l'installe.

---

## 2. Brevets HEVC (H.265)

### De quoi il s'agit

Une photo HEIC d'iPhone est un conteneur HEIF qui contient une image compressée en **HEVC**.
HEVC est couvert par un grand nombre de brevets, détenus par de nombreuses entreprises. Leurs
licences sont gérées par plusieurs *patent pools* concurrents (notamment Access Advance et
Via Licensing Alliance), plus des titulaires indépendants. Il n'existe **pas de guichet
unique**, et les conditions varient selon le pool.

C'est **indépendant de la licence du code** : libde265 est un logiciel libre (LGPL) écrit
sans code propriétaire, mais *mettre en œuvre* HEVC peut quand même tomber sous ces brevets
dans les pays où ils sont reconnus et valides.

### Qui est concerné

- **Le code source de `tri`** : il n'implémente pas HEVC. Il appelle libheif, qui délègue à un
  plugin de décodage. Publier ce code source n'implique pas de distribuer un décodeur HEVC.
- **Un binaire distribué qui embarque libde265** (ou tout autre décodeur HEVC), par exemple un
  bundle macOS ou un installeur Windows : c'est ce cas qui peut être exposé à des demandes de
  redevances, selon les pays.
- **Usage personnel** (compiler et utiliser `tri` sur sa machine) : en pratique, pas de sujet.
- **La géographie compte** : les brevets logiciels sont largement reconnus aux États-Unis.
  En Europe, les « logiciels en tant que tels » ne sont pas brevetables, mais des brevets liés
  à HEVC y ont été délivrés. La situation n'y est donc pas nulle non plus.
- **Durée** : HEVC a été normalisé en 2013 ; les brevets durent en général 20 ans à compter
  du dépôt. Une bonne partie devrait expirer vers le début des années 2030, mais pas toute.

### Comment font les autres

C'est un indicateur utile, pas une garantie :

- **Fedora** ne livre pas de décodeur HEVC dans ses dépôts officiels. libheif y est compilée
  sans HEVC, et le support complet vient de RPM Fusion (paquet `libheif-freeworld`), un dépôt
  tiers que l'utilisateur active lui-même.
- **Debian / Ubuntu** livrent libde265 dans leurs dépôts.
- **Windows** : le décodage HEVC passe par l'extension « HEVC Video Extensions » de Microsoft
  (payante sur le Store), que l'utilisateur installe ; les applications utilisent le décodeur
  du système.
- **macOS / iOS** : le décodeur HEVC fait partie du système. Apple paie les licences, et les
  applications qui passent par les API du système (ImageIO) n'ont pas à s'en soucier.

### AVIF / AV1

AV1 est promu royalty-free par l'Alliance for Open Media (AOM), avec une licence de brevets
gratuite de ses membres. Un pool tiers (Sisvel) revendique toutefois des brevets sur AV1. Le
risque est généralement jugé bien plus faible que pour HEVC. AVIF n'est pas encore géré par
`tri`.

---

## 3. Options pour la distribution

Rien n'est à trancher avant le MVP. Ces options sont à garder en tête pour ne pas fermer de
porte :

1. **HEIF optionnel à la compilation** — ✅ **fait** : feature Cargo `heif` (activée par
   défaut). `cargo build --release --no-default-features` produit un binaire sans aucune
   dépendance HEVC (vérifié avec `otool -L` : ni libheif, ni libde265, ni x265). Dans ce
   cas, `flat::decode` renvoie `FlatError::HeifNotCompiled` pour les HEIC.
2. **Linux : dépendre de la libheif du système** (paquet `.deb`/`.rpm`, Flatpak avec
   extension). Le plugin HEVC est installé, ou non, par l'utilisateur ou la distribution. On ne
   livre aucun décodeur.
3. **macOS : utiliser le décodeur du système** (ImageIO, via les crates `objc2-*`) plutôt que
   libheif. Pas de brevet à notre charge, pas de LGPL à livrer. Contrepartie : du code
   spécifique à la plateforme derrière la même fonction `flat::decode`.
4. **Windows : passer par WIC** (Windows Imaging Component), qui utilise l'extension HEVC de
   l'utilisateur si elle est installée.
5. **Ne jamais livrer x265** : `tri` n'encode pas.

L'option 1 est en place et compatible avec toutes les autres : tout le code libheif est
confiné dans `src/flat/heif.rs`. Un décodeur du système (options 3 et 4) pourrait plus tard
s'y substituer derrière la même fonction `flat::decode`.

---

## 4. Choix de la licence de `tri` : **GPL-3.0-or-later** (décidé le 19/10/2026)

Choix de l'utilisateur : les versions modifiées et distribuées restent libres. Fichier
`LICENSE` (texte officiel de gnu.org), champ `license` des deux `Cargo.toml`. Comparaison
qui a servi à décider :

Les deux familles sont compatibles avec les dépendances actuelles, à condition de respecter
le §1 (liaison dynamique, pas de x265 livré si la licence est permissive).

| | **MIT OR Apache-2.0** | **GPL-3.0-or-later** |
|---|---|---|
| Usage dans l'écosystème Rust | Standard | Moins courant |
| Un tiers peut-il en faire un logiciel fermé ? | Oui | Non : les versions modifiées et distribuées restent libres |
| Compatibilité avec les dépendances LGPL | Oui (liaison dynamique) | Oui |
| Livrer une libheif liée à x265 (GPL) | **Non** | Oui (GPL-2.0-or-later est compatible avec GPL-3) |
| Réutilisation du moteur par d'autres projets | Très facile | Seulement par des projets GPL-compatibles |
| Clause de brevets explicite | Oui (Apache-2.0) | Oui (GPL-3) |

À noter : **aucune de ces licences ne règle la question des brevets HEVC** (§2). Les clauses
de brevets d'Apache-2.0 et de la GPL-3 ne concernent que les brevets *des contributeurs de
`tri`*, pas ceux des détenteurs de brevets HEVC.

Il faut choisir **avant la première contribution externe** : changer de licence ensuite
demande l'accord de tous les contributeurs.

---

## 5. Avant une première publication

- [x] Choisir la licence de `tri`, ajouter `LICENSE` et le champ `license` dans `Cargo.toml`
      (GPL-3.0-or-later, 19/10/2026).
- [x] Générer le fichier des notices des crates (`cargo-about`, `licenses/rust-crates.html`
      dans les paquets, 19/10/2026).
- [x] Windows (21/10/2026) : runtime Visual C++ livré à côté de l'exécutable (fichiers
      redistribuables de Visual Studio, déploiement « app-local » permis par Microsoft) ;
      installateur produit par WiX (licence MS-RL, qui ne couvre que l'outil, pas le
      `.msi` produit).
- [x] Linux (25/10/2026) : `.deb` fait par `dpkg-deb` (outil du système, rien d'ajouté) ;
      AppImage par `appimagetool` 1.9.1 (MIT, outil de compilation seulement, téléchargé
      par la CI). L'AppImage commence par le *runtime* type 2 du projet AppImage (MIT),
      un programme à part qui monte le contenu ; il embarque libfuse (LGPL-2.1) et
      squashfuse (BSD-2-Clause), sans lien avec `tri` (simple juxtaposition), sources
      sur `github.com/AppImage/type2-runtime`.
- [x] Décider du traitement de HEIF dans les binaires distribués (§3) : **sans HEIC**
      (décidé le 19/10/2026), builds sans la feature `heif` ; à rouvrir avec les décodeurs
      du système (ImageIO, WIC).
- [x] Si des bibliothèques LGPL sont livrées : inclure leurs licences et un lien vers leur
      code source exact (`licenses/README.md` des paquets, 19/10/2026). Pour la bêta
      publique, joindre aussi l'archive source de LibRaw à la publication (plus sûr
      qu'un lien qui peut disparaître).
- [x] Vérifier qu'aucun binaire distribué ne contient ou ne dépend de x265 : sans la
      feature `heif`, ni libheif ni x265 (vérifié par `otool -L` sur le paquet macOS,
      19/10/2026).
- [ ] Relire ce document : les versions, licences et pratiques des distributions ont pu
      changer.
