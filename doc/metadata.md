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

## Ligne de commande

```
tri <dossier> --metadata [--raw|--flat] [--depth N]
```

Vérifié (17/10/2026) : RAF X-T4 (objectif Sigma, mode manuel), JPEG exporté, HEIC
d'iPhone 13 Pro (focale 5,7 mm, orientation portrait).
