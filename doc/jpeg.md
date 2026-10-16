# Module `tri::jpeg`

Décodage et encodage JPEG rapides, utilisés partout où le moteur manipule du JPEG :

- aperçus embarqués des RAW (vignettes) ;
- images plates JPEG (`flat::decode_fit`) ;
- lecture et écriture des entrées du [cache](cache.md).

## Deux implémentations

| | Feature `turbojpeg` (défaut) | Sans la feature |
|---|---|---|
| Bibliothèque | [libjpeg-turbo](https://libjpeg-turbo.org/) via le crate `turbojpeg` | crate `image` (décodeur zune-jpeg, pur Rust) |
| Décodage réduit à l'IDCT (1/2, 1/4, 1/8) | oui | non (pleine taille) |
| Choix du sous-échantillonnage à l'encodage | oui | non |

Si libjpeg-turbo refuse un fichier (JPEG CMJN, variante exotique), `decode` retente avec le
crate `image` : un JPEG lisible reste lisible quelle que soit la configuration.

Aucune orientation n'est appliquée ici : `flat` lit la balise EXIF, et pour un RAW c'est la
métadonnée `flip` du RAW qui fait foi.

## API

- `decode(bytes, min_side) -> Result<Bitmap, JpegError>` : décode en RGB8. Avec
  `min_side > 0`, choisit la plus forte réduction à l'IDCT (1/8, 1/4, 1/2) qui garde le plus
  grand côté **≥ `min_side`** ; l'appelant fait ensuite la réduction exacte avec
  `Bitmap::fit`. `0` = pleine taille.
- `encode(bitmap, quality, chroma) -> Result<Vec<u8>, JpegError>` : `Chroma::Full` (4:4:4,
  couleur à pleine résolution, pour le 1:1) ou `Chroma::Half` (4:2:0, vignettes et aperçus).
- `TURBO: bool` : vrai si la feature est compilée.

## Mesures

Mac Apple Silicon, build release :

| Opération | `image` | turbojpeg |
|---|---|---|
| Décodage aperçu embarqué RAF 4416×2944 | 117 ms | 63 ms (1/1), 46 ms (1/8) |
| Décodage JPEG boîtier 6240×4160 | 224 ms | 172 ms (1/1), 132 ms (1/8) |
| Encodage aperçu 1710×2560, qualité 90 | 41 ms | 10 ms |
| Décodage aperçu 1710×2560 (lecture du cache) | 18 ms | 11 ms |

Le décodage réduit gagne moins qu'on pourrait l'espérer : dans un JPEG de haute qualité,
l'essentiel du temps part dans le décodage entropique (Huffman), incompressible ; l'échelle
réduite n'économise que l'IDCT et la conversion de couleur.

Effet sur un dossier réel (472 images, 236 RAF + 236 JPEG, cache vide) : grille complète en
**14–19 s au lieu de 29 s**, cache 20 % plus léger (encodage 4:2:0 de libjpeg-turbo). Détail
dans [scheduler.md](scheduler.md#mesures).

## Licence

libjpeg-turbo : IJG AND BSD-3-Clause AND Zlib, permissives, compilée depuis ses sources et
liée statiquement (voir [build.md](build.md#libjpeg-turbo-compilée-avec-le-projet)) ; crates `turbojpeg` et
`turbojpeg-sys` : Unlicense OR MIT. Aucun brevet actif sur le JPEG de base. Voir
[licensing.md](licensing.md).
