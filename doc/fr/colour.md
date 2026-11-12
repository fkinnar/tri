# Gestion des couleurs (`tri::colour`)

*[English version](../en/colour.md)*

Issue n° 422, conçue avec l'utilisateur le 11/11/2026, après un rendu darktable sorti
largement trop saturé sur un écran wide gamut calibré : `tri` ne gérait aucune couleur.
Toute image était supposée en sRGB, et sur macOS la fenêtre n'était même pas déclarée en
sRGB : le système envoyait les valeurs à l'écran sans conversion.

## Principe

**Ne jamais supposer, lire le profil ; convertir une fois ; laisser l'écran faire le
reste.**

```
source (son profil) ──conversion unique, à la mise en cache──▶ espace de travail ──▶ écran
```

### L'espace de travail

Tout ce que `tri` montre (vignettes, aperçus, 1:1) est dans un seul **espace de
travail** : **primaires Rec.2020, courbe sRGB, blanc D65** (`colour::working`, son ICC
`colour::working_icc`).

Cet ICC a toujours les mêmes octets : `moxcms` y inscrit sa date de création, remplacée
par une date fixe (et l'identifiant MD5 mis à zéro). Il est aussi reconnu quelle que soit
sa date, pour les copies écrites avant. Corrigé le 12/11/2026 : d'un lancement à l'autre,
il n'était plus reconnu, et sa description (« sRGB curve ») le faisait prendre pour du
sRGB. Les copies de la grille des développements étaient converties une seconde fois, et
la grille paraissait terne à côté du plein écran.

- **Large.** Il contient Display P3, l'Adobe RGB, et les écrans wide gamut qui les
  dépassent. L'Eizo de l'utilisateur, plus large que l'Adobe RGB mais pas P3, a écarté P3
  comme espace de travail.
- **Indépendant de l'écran.** Le cache sert sur tous les écrans, et une fenêtre déplacée
  vers un autre écran ne recalcule rien.
- **Mêmes gris que le sRGB.** Il en partage la courbe et le blanc : l'interface d'egui,
  dessinée en sRGB, garde sa luminosité.
- **Sûr en 8 bits.** Le cache est en JPEG 8 bits. Un dégradé sRGB lisse passé par l'espace
  de travail puis revenu en sRGB (deux arrondis, le cas de Linux et Windows) ne bouge que
  de 2 niveaux au plus, ±1 presque partout (mesuré le 11/11/2026, test
  `eight_bit_round_trip_stays_within_two_levels`).

### Les sources

| Source | Son espace | Converti |
|---|---|---|
| RAW rendu par LibRaw | demandé directement dans l'espace de travail (`OutputSpace::WORKING` : sortie Rec.2020 de LibRaw, courbe sRGB) | rien à faire |
| JPEG boîtier (aperçu embarqué) | son ICC, sinon son EXIF (Adobe RGB selon la norme DCF : `InteroperabilityIndex` R03, ou `ColorSpace` 2), sinon sRGB | à la mise en cache |
| Image plate (JPEG, PNG, TIFF, WebP) | son ICC, sinon sRGB (EXIF pour un JPEG) | à la mise en cache, **à sa profondeur** : un TIFF ou PNG 16 bits est converti en 16 bits, puis arrondi à 8 bits (pas de cassures dans un dégradé ProPhoto) |
| HEIF / HEIC | son ICC, sinon NCLX (Display P3 d'un iPhone), sinon sRGB | à la mise en cache |
| Rendu darktable, RawTherapee, ART, RapidRAW | l'ICC que le logiciel intègre (le profil de sortie de l'historique ou du `.pp3`, **jamais imposé par `tri`**), sinon sRGB | **à la lecture** du cache (`Cache::get`) : le JPEG du logiciel reste intact en cache, donc aucun développement n'est refait |

Le profil d'une source est lu par `colour::Origin` (aussi utilisé par l'export). sRGB et
Adobe RGB sont reconnus à leur description, sauf les profils linéaires ; tout autre ICC est
lu par `moxcms`.

`tri` écrit des JPEG parmi les rendus développés : les copies à la taille des vignettes
pour la grille, ou la réduction d'un rendu RawTherapee ou ART. Chacun porte son profil :
l'ICC de travail pour une copie déjà convertie, celui du logiciel pour une réduction. Ainsi,
rien n'est converti deux fois.

### L'écran

- **macOS.** La fenêtre est déclarée dans l'espace de travail (`display::declare_working` :
  l'espace couleur de la couche Metal, tiré de l'ICC de travail). ColorSync convertit vers
  le profil de l'écran, calibration comprise, et suit la fenêtre d'un écran à l'autre.
  wgpu remet cet espace à zéro à chaque reconfiguration de la surface (un
  redimensionnement, par exemple) : il est donc redéclaré à chaque image. Ça coûte deux
  messages Objective-C quand il est déjà en place.
- **Linux, Windows.** Le système ne convertit rien pour les applications. Chaque image est
  convertie vers le profil de l'écran avant de devenir une texture (`display::to_screen`,
  dans le fil de chargement pour les grandes images). Le profil vient de trois endroits,
  dans cet ordre :
  1. `[gui] display_profile`, un fichier `.icc` ou `.icm` (`~/…` accepté). C'est un
     secours quand la détection échoue, et c'est utile pour les tests.
  2. Le système (`display::Screen::follow`) :
     - **Linux** : la propriété X11 `_ICC_PROFILE` de la fenêtre racine, publiée par
       colord (ou `dispwin`) pour l'**écran principal**. Elle est lue avec `x11rb`, en pur
       Rust. Sous Wayland, ça ne marche que si le compositeur la publie pour XWayland.
     - **Windows** : `GetICMProfileW` sur l'écran **où est la fenêtre**, le profil associé
       dans « Gestion des couleurs ». Il est suivi quand la fenêtre change d'écran
       (`MonitorFromWindow` à chaque image, sans coût notable).
  3. Sinon, sRGB.

  Quand le profil change, toutes les textures d'images sont refaites sur place, comme
  après un réglage, sans retomber sur la vignette. Un profil illisible ou inutilisable par
  `moxcms` est noté au journal, et le sRGB est utilisé. Le profil est lu au démarrage,
  quand le réglage change, et quand la fenêtre change d'écran (Windows) : un profil changé
  dans colord pendant que `tri` tourne est vu au lancement suivant.

L'interface (couleurs d'egui, en sRGB) n'est pas convertie. Sur macOS, ses gris ne
changent pas, mais ses couleurs vives (sélection, pastilles) sortent plus saturées.
L'utilisateur l'a accepté le 11/11/2026, à adapter si ça gêne.

### Ce qui reste sur les données réelles

- **Les analyses d'exposition** : histogrammes, alertes d'écrêtage, zones.
- **Les scores de netteté et les empreintes des piles** : leurs clés de cache ne changent
  pas, donc rien n'est recalculé.
- **L'histogramme du JPEG boîtier** (`preview::camera_exposure`) : il lit le JPEG embarqué
  dans son propre espace. Dans l'aperçu converti, les valeurs écrêtées par le boîtier ne
  seraient plus à 255.

### Réglages de visualisation

Ils restent définis sur des **canaux sRGB**, comme avant la gestion des couleurs :
balance, mélangeur, pipette, luminance. Chaque pixel passe de l'espace de travail à la
lumière linéaire aux primaires sRGB (`working_to_srgb_linear`, matrice 3×3 exacte ;
valeurs sous 0 ou au-dessus de 1 hors du gamut sRGB), y est réglé, puis revient. Voir
[adjust.md](adjust.md).

## Ce qui change pour l'utilisateur

- **Les couleurs** suivent le profil de la source et celui de l'écran.
- **Les rendus RAW sont un peu plus clairs, surtout dans les ombres.**
  - LibRaw sortait la courbe BT.709 (vidéo), que `tri` affichait comme du sRGB : plus
    sombre que les données, un gris à 18 % sortait à 104 au lieu de 118.
  - Sur un RAF de test, la luminance moyenne affichée est passée de 29 à 44 sur 255.
  - Les données sont les mêmes (0,5 niveau d'écart en moyenne, mesuré le 11/11/2026).
    L'utilisateur a choisi la lecture correcte.
- **Le cache est refait une fois.** La version des couleurs entre dans la clé des niveaux
  d'image (`cs=1` dans `preview::params`) : vignettes, aperçus boîtier, rendus RAW, copies
  de la grille. Les rendus développés ne sont pas refaits (convertis à la lecture), ni les
  analyses.
- **`tri --adjust`** écrit ses JPEG avec l'ICC de travail intégré (`jpeg::with_icc`, sans
  recompresser).

## Coût

Mesuré avec `moxcms` 0.8 sur un cœur, image de 2560 px (4,4 Mpx) :

| Conversion | Temps |
|---|---|
| 8 bits | 6 à 7,5 ms |
| 16 bits | 18 ms |
| vignette | une fraction de ms |
| décodage du même aperçu, pour comparer | environ 30 ms |

Chaque conversion est construite une fois par couple d'espaces et gardée
(`colour::to_working`, `to_screen`).

## Tests

- `src/colour.rs` :
  - gris inchangés ;
  - même couleur depuis sRGB, Adobe RGB et P3 ;
  - ICC de travail reconnu, le même d'un lancement à l'autre, et reconnu avec une autre date ;
  - profils linéaires non pris pour du sRGB ;
  - matrices inverses et luminance Rec.2020 ;
  - aller-retour en 8 bits ;
  - précision du 16 bits.
- `gui/src/display.rs` : un fichier de profil lu et décrit, un mauvais refusé ; un écran
  large reçoit des valeurs moins saturées qu'un écran sRGB.
- `tests/colour.rs` (mires synthétiques écrites avec leur ICC) :
  - même couleur depuis des JPEG en sRGB (avec et sans profil), Adobe RGB, P3 et
    ProPhoto ;
  - un PNG 16 bits ProPhoto ;
  - un rendu développé ProPhoto converti à la lecture, et sa copie pour la grille non
    convertie deux fois ;
  - `with_icc`.

## Vérifier à la main : les mires

Pour les testeurs : [colour-check.md](colour-check.md) (fiche pas à pas, sans compiler) ;
les mires JPEG et PNG sont dans [`doc/charts`](../charts).

```
cargo run --release --example colour_charts -- <dossier de sortie> [--side N] [--check-only]
```

L'outil écrit la **même** mire en sRGB, Adobe RGB, Display P3, ProPhoto, Rec.2020, et dans
les ProPhoto / Rec.2020 **linéaires** de darktable, chacune avec son ICC intégré : JPEG
8 bits, TIFF 16 bits, PNG 8 et 16 bits, plus un JPEG sRGB sans profil.

La mire, de haut en bas :
1. primaires et secondaires sRGB ;
2. couleurs courantes (peau, ciel, feuillage, orange, violet) et un gris à 18 % ;
3. un dégradé gris ;
4. des dégradés du noir au rouge, au vert et au bleu ;
5. un tour des teintes à pleine saturation ;
6. des couleurs **hors du sRGB** : primaires et secondaires Display P3, à côté du rouge
   sRGB.

Tous les fichiers doivent paraître identiques dans `tri`, et identiques dans Aperçu ou
darktable. La dernière rangée fait exception : elle est écrêtée dans les fichiers sRGB
(c'est normal) et en partie en Adobe RGB (le rouge P3). Elle est vive sur un écran large,
presque plate sur un écran sRGB.

L'outil relit ensuite chaque fichier comme `tri` (`flat::decode_working`) et affiche le
plus grand écart par rangée, en niveaux sur 255.

Vérifié le 11/11/2026 contre ColorSync, le moteur couleur de macOS (`sips -m` vers le
sRGB) : tous les fichiers concordent, aux bords des cases des JPEG près. Deux choses sont
apparues en route :
- **Le profil Display P3 de `moxcms` 0.8 porte une balise CICP fausse** : « SMPTE RP 431 »,
  le P3 du cinéma avec le blanc DCI, au lieu de « EG 432 ». ColorSync suit cette balise.
  Les mires, et le profil P3 que `tri` fabrique pour un HEIC d'iPhone, s'en passent ;
  `tri` lui-même lit les primaires de la matrice et n'était pas touché.
- **ImageIO rejette ces profils Display P3 et Rec.2020 dans un PNG** (`sips -g profile` dit
  « nil ») et lit le fichier comme du sRGB, alors qu'il accepte le même profil dans un TIFF
  ou un JPEG. Aucun PNG n'est donc écrit pour ces deux espaces, pour que la comparaison
  avec Aperçu ne trompe pas.

## Reste à faire (n° 422)

- Vérifier avec des profils de sortie exotiques, et sous Linux et Windows (Windows :
  compilation vérifiée depuis macOS seulement, non essayé par le développeur).
- Linux : un profil par écran (`_ICC_PROFILE_n`), et le protocole de gestion des couleurs
  de Wayland quand egui et wgpu l'exposeront.
