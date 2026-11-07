# Module `embedded` : notes inscrites dans les images

`src/embedded.rs` (issue n° 276, décidé pour la bêta le 07/11/2026).

Les notes de `tri` (étoiles, couleurs, accepté / rejeté) vivent dans les sidecars (voir
[xmp.md](xmp.md), [sidecars.md](sidecars.md)). Ce module les **inscrit aussi dans les
images elles-mêmes**, pour les retrouver hors de `tri` : l'Explorateur Windows, Spotlight,
Lightroom, darktable, digiKam… Et il lit celles qu'un autre logiciel y a mises.

## Formats

| Format | Où | Notes |
|---|---|---|
| JPEG | segment APP1 `http://ns.adobe.com/xap/1.0/` | remplacé à sa place, ou ajouté après JFIF et EXIF |
| PNG | chunk `iTXt` `XML:com.adobe.xmp` | non compressé ; un XMP compressé existant : refus |
| WebP | chunk `XMP ` | un WebP simple devient étendu (`VP8X`), par `img-parts` |
| TIFF | balise 700 (`XMLPacket`) | à sa place s'il y tient (espaces de remplissage), sinon en fin de fichier avec une copie de l'IFD0 |
| HEIC / HEIF | élément `mime` `application/rdf+xml`, référence `cdsc` vers l'image principale | à sa place s'il y tient ; sinon ses données dans un `mdat` ajouté en fin de fichier, `meta` reconstruite (`iinf`, `iref`, `iloc`) et les adresses des données qui la suivent décalées d'autant ; pas les séquences (`moov`) |

Jamais un RAW : ses notes restent dans les sidecars. Vérifié avec ExifTool (`-validate`) et
macOS : grilles de tuiles comme celles d'un iPhone, pixels décodés identiques.

## Ce qui est écrit

- Le **paquet XMP** de l'image, complété par `xmp::apply` (nos champs seulement, comme
  dans un sidecar : `xmp:Rating`, `xmp:Label`, accepté / rejeté…), jamais remplacé : les
  mots-clés ou l'historique d'un autre logiciel restent.
- Une note **EXIF** `Rating` (0x4746) ou `RatingPercent` (0x4749) déjà présente est mise à
  jour sur place (deux octets), sans quoi l'Explorateur Windows pourrait montrer
  l'ancienne. Absente, elle n'est pas ajoutée : réécrire l'EXIF risque d'abîmer les
  données du boîtier (MakerNotes), et le XMP suffit. Rejetée : 0 (l'EXIF ne connaît pas
  le rejet).
- **Sans recompression** : le reste du fichier est gardé octet pour octet.
- Une image sans note et sans notes déjà inscrites n'est pas touchée.

## Écriture sûre

Le nouveau contenu est d'abord copié à côté (`.<nom>.tri.tmp`), puis écrit **dans le
même fichier** (attributs étendus, droits, étiquettes du Finder gardés), puis relu : si
les notes relues diffèrent, erreur. Si l'écriture échoue en route, la copie complète
reste à côté.

L'image change (taille, date) : ses vignettes et aperçus sont recalculés.

## Lecture

`embedded::read` lit en flux les seuls en-têtes (segments jusqu'aux données du JPEG,
chunks sautés du PNG et du WebP, IFD0 du TIFF) : rapide sur tout un dossier. Le XMP
d'abord ; à défaut, la note EXIF `Rating`.

Dans [notes.md](notes.md), une image **sans sidecar** prend les notes inscrites en elle
(étoiles posées dans l'Explorateur Windows…). Noter crée son sidecar, avec ces notes.
Ensuite le sidecar l'emporte : une note changée plus tard dans l'image se relit avec
« Relire les notes depuis les images ».

## Quand

Sur demande seulement (décidé le 07/11/2026 : l'automatique viendra après réflexion),
deux actions symétriques, sur la sélection ou, par le menu du dossier, tout le dossier :

- **« Inscrire les notes dans les images »** (`core.file.embed_notes`, en fond) : les
  notes de `tri` vont dans les images. `tri <dossier> --embed-notes [--dry-run]`,
  `NoteBook::embed`.
- **« Relire les notes depuis les images »** (`core.file.import_notes`) : les notes
  inscrites dans les images (changées dans un autre logiciel : XnView MP, l'Explorateur
  Windows…) remplacent celles des sidecars, comme une note posée dans `tri` (la paire
  RAW+JPEG suit ; un pas d'historique, annulable). Une image sans notes inscrites n'est
  pas touchée. `tri <dossier> --import-notes [--dry-run]`, `NoteBook::import`.

## Avec les autres logiciels

- **XnView MP** (vérifié sous Linux Mint le 07/11/2026) : il garde étoiles et couleurs
  dans son catalogue, lu dans les fichiers à la première ouverture d'un dossier ; ensuite,
  les notes inscrites par `tri` n'apparaissent qu'après « Mise à jour du catalogue à
  partir des fichiers ». Dans l'autre sens, une note changée dans XnView ne va dans
  l'image qu'avec « Métadonnées › Mettre à jour le fichier à partir du catalogue » ;
  `tri` la voit alors aussitôt (surveillance du dossier) si l'image n'a pas de sidecar,
  sinon avec « Relire les notes depuis les images ».

## À vérifier

- L'Explorateur Windows : il lit `xmp:Rating` et l'EXIF ; un paquet qui porte aussi
  `MicrosoftPhoto:Rating` (écrit par Windows) n'est pas encore mis à jour.
- Les HEIC d'un vrai iPhone (testé sur des fichiers produits par macOS).
