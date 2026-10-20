# Module `tri::metadata`

Métadonnées de prise de vue : appareil, objectif, exposition, correction, mode, focale,
flash, date, dimensions, taille du fichier. Lues à la demande (quelques ms), pas mises en
cache.

## Sources

- **RAW** : LibRaw dès l'ouverture, sans décompresser (`RawFile::shooting_info`, shim
  `tri_libraw_metadata`) : fabricant, modèle, objectif, ISO, temps de pose, ouverture,
  focale et équivalent 24 × 36, date (mise en forme en heure locale par le shim :
  `localtime` + `strftime`, pas de dépendance de dates), dimensions. LibRaw n'expose pas
  la **correction d'exposition** ni le mode : ils viennent de l'**EXIF du JPEG
  embarqué** (`thumbnail()`, ~10 ms), qui ne fait que compléter (LibRaw fait foi).
  Mesuré : 2 à 8 ms par RAF X-T4.
- **Images plates** : leur propre EXIF, lu par `kamadak-exif` (JPEG, TIFF, HEIF, PNG,
  WebP) ; dimensions lues dans l'en-tête (`image::image_dimensions`) faute d'EXIF, puis
  échangées si l'orientation EXIF est d'un quart de tour (5 à 8).
- Une image sans EXIF donne des champs vides, pas une erreur.

Choix de `kamadak-exif` (17/10/2026) : BSD-2-Clause, une seule dépendance
(`mutate_once`, BSD-2-Clause), le plus utilisé (3,7 M de téléchargements récents), dernier
commit octobre 2025, lit tous nos conteneurs d'images plates. Écartés : `rexif` (JPEG et
TIFF seulement, licence non identifiée sur GitHub), `nom-exif` (plus lourd : chrono,
regex, nom… ; licence « non standard » sur crates.io).

## Affichage

`Metadata::rows()` : lignes (libellé, valeur) en français, dans l'ordre, champs inconnus
omis — prêtes pour n'importe quelle interface : « Exposition : 1/250 s · f/2,8 ·
ISO 400 », « Correction : +0,7 IL », « Focale : 5,7 mm (éq. 24 × 36 : 26 mm) »,
« Date : 16/10/2026 à 14:32:05 », « Dimensions : 4160 × 6240 (26 Mpx) ». Aides :
`camera()` (sans répéter le fabricant), `shutter_text()`, `exposure_text()`.

`Metadata::embedded_size` (21/10/2026) : pour un RAW, les dimensions du JPEG embarqué,
lues dans son seul en-tête (orientation du RAW appliquée) ; la résolution du « JPEG
boîtier » en plein écran.

## Ligne de commande

```
tri <dossier> --metadata [--raw|--flat] [--depth N]
```

Vérifié (17/10/2026) : RAF X-T4 (objectif Sigma, mode manuel), JPEG exporté, HEIC
d'iPhone 13 Pro (focale 5,7 mm, orientation portrait).

## Collimateur de mise au point (`focus_point`, 17/10/2026)

Coordonnées relatives (0 à 1) dans l'image **affichée** (orientation appliquée), pour
dessiner le repère du collimateur. Lu dans la **note du fabricant** de l'EXIF du JPEG
embarqué (`MakerNote`, que `kamadak-exif` rend brute) :

- **Fujifilm** : en-tête `FUJIFILM`, position de l'IFD (32 bits), IFD petit-boutiste,
  balise **0x1023** `FocusPixel` (deux entiers 16 bits x, y, en pixels de l'image du JPEG
  avant orientation : on divise par `PixelXDimension` / `PixelYDimension`), puis
  l'orientation EXIF (1 à 8) est appliquée. Vérifié sur un RAF X-T4 tenu verticalement
  (ExifTool : `FocusPixel 2517 1317`, JPEG 4416 × 2944, orientation 6) : (0,553 ; 0,570),
  bien sur le poteau où le boîtier avait fait le point. (Une première version lisait la
  balise 0x1031 : point faux, attrapé par le test sur un vrai fichier.)
- Autres marques : pas encore (`None`) ; chacune a son propre format de note.

Tests : note synthétique (lue, autre marque refusée, note tronquée refusée), orientations ;
`tests/metadata.rs` sur un vrai RAF (`TRI_TEST_RAW`, `#[ignore]`).
