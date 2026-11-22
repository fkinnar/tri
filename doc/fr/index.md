# Index de recherche des palettes (`tri::index`)

Étape 1 de l'issue #561 : chercher des palettes dans **tous les dossiers**, pas seulement
celui qui est ouvert. Les étapes suivantes (grille des résultats, « Retrouver… » pour un
dossier déplacé, racines surveillées) s'appuieront sur ce module.

## Principe : un cache de recherche, pas une bibliothèque

L'utilisateur ne veut pas de « bibliothèque » visible de l'extérieur : un catalogue à la
Lightroom casse dès qu'on déplace un dossier hors de l'application (dans darktable, d'un
disque interne vers un disque de stockage…). D'où :

- **Seulement des données recalculables** : les palettes. Les notes, versions, etc. restent
  dans les sidecars XMP. L'index peut être effacé à tout moment : les recherches sont
  partielles jusqu'aux prochains calculs. Rien n'est écrit à côté des photos.
- **Identité par le contenu, pas par le chemin.** Chaque photo a une **empreinte**
  (`fingerprint`) : sa taille et un hachage BLAKE3 (tronqué à 16 octets) de ses
  `fingerprint_kb` premiers et derniers Ko (32 par défaut). Une palette ne dépend que du
  contenu et de ses paramètres (`preview::stamp` : réglages des palettes et, pour le rendu
  d'un logiciel, l'état du développement dans le sidecar). Une photo déplacée garde donc sa
  palette ; son nouveau chemin est simplement noté quand `tri` la revoit.
- **Rien n'est suivi** (déplacements, copies, suppressions), dans `tri` ou ailleurs :
  - déplacée : l'ancien chemin ne mène plus à rien, ce qui se verra à l'affichage d'un
    résultat ;
  - copiée : deux chemins, une empreinte, donc deux résultats ;
  - modifiée : une autre empreinte ; l'ancienne entrée devient orpheline.

## La base

SQLite (`rusqlite`, SQLite compilée avec le projet), fichier `index.sqlite` du **dossier
des données** de `tri` (`[data] dir`), pas du cache, qu'on vide. Journal WAL : on lit
pendant qu'on écrit. Deux tables :

| Table | Clé | Contenu |
|---|---|---|
| `paths` | chemin (octets du système ; UTF-16 sous Windows) | taille, date de modification (ns, celle des clés du cache), empreinte (vide tant qu'aucune palette n'a été calculée) |
| `palettes` | (empreinte, source) | tampon des paramètres, palette (format du cache, `Palette::to_bytes`) |

La taille et la date gardées avec chaque chemin permettront de retrouver la vignette d'un
résultat dans le cache **sans relire le fichier** (rapide sur un NAS ou un disque
débranché). Une base d'un autre schéma (`user_version`) ou illisible est refaite.

## Toutes les photos vues (#561, schéma 2)

Pour « Toutes les photos » (une racine virtuelle au-dessus de tous les emplacements, où les
filtres font office de recherche), l'index note **chaque photo vue**, pas seulement celles
qui ont une palette : `index::seen(dossier, niveaux, images)` à l'ouverture d'un dossier
(interface) et au calcul d'un dossier (« Calculer »). Chemin, taille et date seulement :
l'empreinte n'est lue qu'au calcul d'une palette. Les photos de ces niveaux qui n'y sont
plus sont oubliées (supprimées, ou déplacées : revues ailleurs, elles y seront notées) ;
une photo modifiée (autre taille ou date) perd son empreinte. Les développements
(`IMG@dt-01.RAF`) ne sont pas des fichiers : pas notés, ils se retrouvent en relisant le
dossier. `Index::photos(sous)` les liste, sans parcourir de dossier. Le passage au
schéma 2 refait la base une fois : les palettes déjà en cache s'y renotent d'elles-mêmes.

## Empreintes de similitude (schéma 3)

Les empreintes des piles (`similarity::Signature`, ~70 octets) sont notées avec chaque
chemin (`paths.signature`, et `signature_stamp` : leurs paramètres), dès qu'elles sont
prêtes (`index::note_signature`, planificateur et « Calculer »). Elles suivent le
fichier : un fichier modifié les perd, un dossier réparé les garde. « Toutes les
photos » les charge d'un coup à l'ouverture (`Index::signatures`) : « semblables »
partout sans relire le cache photo par photo (100 000 photos : ~7 Mo). Le passage au
schéma 3 refait la base une fois.

## Remplissage

Toute palette **prête**, calculée ou trouvée dans le cache, est signalée par
`index::note` :
- par le planificateur (`scheduler::finish`), donc à l'ouverture d'un dossier, que ses
  palettes soient calculées ou déjà en cache ;
- par les calculs choisis (`compute::run`, clic droit › Calculer, `tri --compute`).

Un **fil à part** (`index::start`, `tri-index`) écrit par lots (au plus `batch`, une
transaction) : le calcul ne l'attend jamais. Un chemin déjà connu à la même taille et la
même date, avec la palette aux mêmes paramètres, ne coûte qu'une lecture de la base ;
sinon l'empreinte est calculée (au plus 2 × 32 Ko lus) et la palette relue dans le cache.
`index::flush` attend que tout soit écrit (ligne de commande, avant de quitter).
L'interface démarre l'index avec le moteur (jamais dans les tests) ; `--compute` aussi,
quand il calcule des palettes.

## Réglages `[index]`

| Clé | Défaut | Rôle |
|---|---|---|
| `enabled` | `true` | noter les palettes dans l'index |
| `fingerprint_kb` | 32 | Ko lus au début et à la fin d'un fichier pour son empreinte |
| `batch` | 256 | palettes notées par écriture, au plus |

## Mesures (22/11/2026)

100 000 palettes synthétiques (6 couleurs, couverture de 96 × 96 points bruités), Mac
Apple Silicon, release (`cargo test --release --test index -- --ignored --nocapture`) :

| | |
|---|---|
| palette | 2,4 Ko en moyenne (les vraies : ~1,2 Ko, mesuré sur 131 photos, #562) |
| base | 429 Mo |
| écriture, lots de 256 | 48 s (~0,5 ms par palette, en fond) |
| lecture de tout | 0,34 s |
| « palettes proches » sur tout | 96 ms |
| « contient cette couleur » sur tout | 68 ms |

La recherche en mémoire suffit (moins de 0,1 s, plus le chargement, une fois). La taille
de la base est le point à surveiller : couverture compressée ou moins fine dans l'index, à
décider sur de vraies photos.

## Tests

- `src/index.rs` : photo déplacée (un chemin de plus, même palette), palette recalculée à
  d'autres réglages (remplacée), empreinte (seulement les bouts du fichier), base d'un
  autre schéma refaite, chemins non ASCII.
- `tests/index.rs` : de bout en bout, calcul réel de palettes, index rempli par le fil
  d'écriture, rien de neuf au second calcul, photo déplacée retrouvée ; la mesure
  ci-dessus (`#[ignore]`).
