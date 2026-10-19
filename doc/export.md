# Module `tri::export` : exporter les images développées

Décidé le 19/10/2026 avec l'utilisateur : première étape de l'« après-développement »
(voir `CLAUDE.md`). On exporte des JPEG, TIFF ou PNG avec un **profil d'export** par
usage. Les usages de l'utilisateur sont les profils fournis :

- **Retouche** (`edit`) : TIFF 16 bits en ProPhoto, pleine définition, à destination d'un
  éditeur externe ;
- **Web** (`web`) : JPEG sRGB qualité 90, 2048 pixels au plus.

Sans darktable, l'export passe par LibRaw (voir plus bas). Étape suivante prévue : les
versions recadrées dans `tri` (stockées comme instructions dans le XMP).

## darktable fait tout

`tri` n'implémente ni format ni profil couleur : `darktable-cli`, déjà piloté pour les
rendus développés ([develop.md](develop.md)), s'en charge. Un profil n'est qu'une poignée
d'options traduites en arguments (`Profile::darktable_args`) :

| Clé | darktable-cli |
|---|---|
| `format` (`jpeg`, `tiff`, `png`) | `--out-ext jpg / tif / png` |
| `bits` | `--conf plugins/imageio/format/tiff/bpp=…` (ou `png/bpp`) |
| `quality` | `--conf plugins/imageio/format/jpeg/quality=…` |
| `color` | `--icc-type SRGB / ADOBERGB / LIN_REC709 / LIN_REC2020 / PROPHOTO_RGB / DISPLAY_P3`, ou `FILE` + `--icc-file` pour un `.icc` |
| `intent` | `--icc-intent PERCEPTUAL / RELATIVE_COLORIMETRIC / …` |
| `max_side` | `--width N --height N` (0 : pleine définition), `--upscale false` |

Toujours `--hq true` : un export est fait pour la qualité.

Options vérifiées le 19/10/2026 sur darktable 5.6.2 (`darktable-cli --help`,
`--help icc-type`, clés de `darktablerc`). Les espaces couleur proposés sont ceux de la liste
d'export de darktable, sans les variantes HDR (PQ, HLG), inutiles pour un export photo.
Attention : la **ProPhoto intégrée à darktable est linéaire** (« Linear ProPhoto RGB »,
relu par ExifTool). Pour une ProPhoto gamma 1,8 classique, il faut donner le chemin d'un
profil `.icc`. C'est celle qu'utilise l'utilisateur (confirmé le 19/10/2026 : la seule
proposée par l'interface de darktable) : le profil « Retouche » donne donc le même fichier
que son export darktable habituel.

## Pas de ProPhoto en JPEG

Idée de l'utilisateur, 19/10/2026 : « empêcher une andouille d'exporter du JPEG en
ProPhoto ». ProPhoto (très large) et les espaces linéaires (Rec709, Rec2020) n'ont de sens
qu'en **16 bits ou plus** : en 8 bits, leurs dégradés se cassent en marches
(postérisation). sRGB, Adobe RGB et Display P3 restent sûrs en 8 bits.

- **Moteur** : `Profile::check` refuse un tel profil, avec la raison. Le refus a lieu dès
  la lecture des réglages, comme toute valeur invalide ; un fichier de configuration écrit
  à la main est donc signalé au démarrage.
- **Fenêtre de réglages** : la liste déroulante des espaces ne propose pas ceux-là pour un
  profil en JPEG ou en 8 bits, ni le JPEG pour un profil en ProPhoto (`fits`). Une
  modification incohérente est refusée avec la raison.
- Un profil `.icc` est laissé au jugement de l'utilisateur (`color_fits`).

## Un seul lancement de darktable

Toutes les images d'un export passent par **un seul** `darktable-cli` (`--import` par
image) : son démarrage (~6 s) n'est payé qu'une fois. Mesuré le 19/10/2026 sur deux RAF
X-T4, en JPEG 2048 px : 13,5 s en tout.

- Chaque image est développée d'après **son** sidecar `IMAGE.RAF.xmp`, historique et
  recadrage compris. Vérifié : DSCF0331, recadré dans darktable, sort en 2763 × 4912.
  Une image sans développement (pas de sidecar, ou sidecar écrit par `tri` seul) reçoit
  les réglages par défaut de darktable. Ce ne sont pas les préréglages automatiques de
  l'utilisateur (configuration propre à `tri`, `--apply-custom-presets false`).
- **Nom des fichiers** : `$(FILE_NAME)` (nom de l'image sans extension), puis le suffixe
  du profil, puis l'extension. darktable n'écrase jamais rien : il ajoute `_01`…
- **Progression** : darktable écrit une ligne `[export_job] exported to `…'` par fichier.
  Lue au fil de l'eau, elle donne la progression et les fichiers vraiment écrits
  (`Report::written`). Les images sans fichier écrit sont comptées (`Report::failed`).
- **Annulation** : le programme est arrêté ; les fichiers déjà écrits restent.
- **Paires RAW+JPEG** (`sources`) : le JPEG d'une paire dont le RAW est aussi demandé
  n'est pas exporté. Le développement est celui du RAW, et les deux donneraient le même
  fichier.
- **Rien n'est écrit chez l'utilisateur** : configuration propre
  (`<cache>/darktable-export`, distincte de celle des rendus développés, pour que les deux
  puissent tourner en même temps), bibliothèque en mémoire, `write_sidecar_files=never`.
  Vérifié : sidecars identiques octet par octet après export.

## Profils fournis

| Profil | Format | Espace | Taille | Sans recompression |
|---|---|---|---|---|
| Retouche (`edit`) | TIFF 16 bits | ProPhoto linéaire | pleine | — |
| JPEG tel quel (`asis`) | JPEG 95 | d'origine (`""`) | pleine | oui |
| Web (`web`) | JPEG 90 | sRGB | 2048 px | oui |

« JPEG tel quel » (demandé le 19/10/2026) : un JPEG est copié sans rien toucher (taille,
espace : un JPEG Adobe RGB le reste) ; un RAW sort en JPEG pleine taille, dans l'espace du
développement (darktable), ou en sRGB (LibRaw).

## Réglages (`[export]`)

```toml
[export.profiles.web]        # autant de profils que voulu
name = ""                    # vide : nom fourni traduit, sinon l'identifiant
format = "jpeg"
bits = 8
quality = 90
color = "srgb"
intent = ""
max_side = 2048
suffix = ""
folder = "export"            # voir « Dossier du profil »
```

Chaque clé absente vient du profil fourni de même identifiant (`edit`, `web`), sinon d'un
JPEG sRGB (`ProfileToml`, `Profiles::deserialize`). La fenêtre de réglages peut ainsi
n'écrire qu'une valeur sans faire disparaître le reste du profil. Ordre : profils fournis,
puis ceux de l'utilisateur par identifiant.

### Dossier du profil

Idée de l'utilisateur (19/10/2026) : le dossier fait partie du profil (« Web » dans
`~/Web`, « Retouche » à côté des images…), plutôt que retenu d'un export à l'autre. Règle
de `folder` (`folder_for`, `output_pattern`) :

- `/…` : absolu ;
- `~/…` : depuis le dossier personnel ;
- sinon **relatif au dossier de chaque image** : `"export"` (défaut) crée un sous-dossier
  à côté des images ; `""` ou `"."` écrit **à côté de la source**. Passé à darktable comme
  `$(FILE_FOLDER)/<dossier>/$(FILE_NAME)` : une sélection venue de plusieurs dossiers
  s'exporte dans chacun.
- Les variables de darktable sont permises (`$(YEAR)`…, voir la documentation de
  darktable), passées telles quelles.

Vérifié le 19/10/2026 : sous-dossiers créés par darktable, accents et espaces compris
(`séance été/export/…`), et `"."` à côté du RAF.

## On exporte ce qu'on regarde

Règle de l'utilisateur (19/10/2026) : l'image exportée est celle qu'on regarde.

| Où | Image exportée |
|---|---|
| Grille | développée par darktable s'il est là, sinon rendu LibRaw |
| Plein écran, développé (`Maj+B`) | développée par darktable |
| Plein écran, rendu RAW (`b`) | rendu LibRaw |
| Plein écran, JPEG boîtier | le JPEG boîtier (aperçu embarqué, à sa taille) |

La boîte d'export ne propose **pas de choix** : on fait `Ctrl+E` sur l'image voulue
(retour de l'utilisateur, 19/10/2026 : pas de complexité de plus dans la boîte).
`Render::of` décide, une ligne d'explication le dit. En ligne de commande : darktable
s'il est là, `--libraw` ou `--embedded` pour les autres.

Le **JPEG boîtier** (`Local::Embedded`) est l'aperçu embarqué du RAW, à sa taille (71 % du
capteur sur un X-T4), redressé.

## Espace d'origine des images déjà développées

Remarque de l'utilisateur (19/10/2026) : un JPEG boîtier peut être en **Adobe RGB**
(réglage de presque tous les boîtiers), et un JPEG du disque aussi. On ne suppose donc
rien : `Origin` lit l'espace réel de chaque JPEG boîtier ou image plate.

1. Le **profil ICC intégré** (`jpeg::icc_profile` : segments APP2 remis bout à bout ;
   autres formats : celui que lit le crate `image`). sRGB et Adobe RGB y sont reconnus à
   leur description, en ASCII ou en UTF-16.
2. Sinon l'**EXIF**, selon la norme DCF : `InteroperabilityIndex` « R03 » (Adobe RGB) ou
   « R98 » (sRGB), et `ColorSpace` 2, qu'écrivent certains boîtiers pour Adobe RGB.
3. Sinon sRGB, l'espace par défaut de l'EXIF.

Vérifié sur le X-T4 de l'utilisateur : JPEG boîtier en « R98 », donc sRGB.

L'espace du profil d'export décide ensuite :

- `""` (**« JPEG tel quel »**) : l'espace d'origine est **gardé**, avec son profil ICC.
- Même espace que l'origine : rien à convertir.
- Autre espace : **conversion** par `moxcms` (16 bits), puis le profil ICC du nouvel
  espace. Un JPEG Adobe RGB exporté en « Web » sort donc en sRGB, comme il se doit pour
  le web.

## Sans recompression (`passthrough`)

Idée de l'utilisateur (19/10/2026) : ne pas recompresser une seconde fois un fichier déjà
compressé. Réglage de chaque profil (`passthrough`), **activé par défaut** (retour de
l'utilisateur), case « Sans recompression » de la boîte, `--passthrough` en ligne de
commande. Il ne s'applique qu'aux profils JPEG, depuis un JPEG, **dans le même espace**
(voir plus haut : une conversion recompresse forcément) :

- **JPEG d'origine** (image plate) : copié tel quel, octet par octet (EXIF, profil
  compris).
- **JPEG boîtier** d'un RAW : ses octets sont écrits tels quels ; seule l'**orientation**
  du RAW est posée dans son EXIF, sans recompresser (`jpeg::set_orientation` : balise
  modifiée sur place, ou segment EXIF minimal ajouté). Vérifié sur un RAF X-T4 : données
  compressées identiques à l'aperçu embarqué extrait par ExifTool, même taille de
  fichier, « Rotate 90 CW ».
- Une image **plus grande que le côté maximal du profil** est réencodée (réduire, c'est
  recompresser). Un **rendu** (darktable, LibRaw) n'a rien de compressé à garder : sans
  effet, et la case n'est pas proposée.
- Le bilan dit combien de fichiers ont été écrits sans recompression
  (`Report::passthrough`).

## Sans darktable : LibRaw

Étape 2 du plan (19/10/2026, « plus simple », selon l'utilisateur) : sans darktable
installé (ou `--libraw` en ligne de commande), l'export passe par LibRaw
(`export_libraw`). On obtient le **rendu de base** de `tri`, celui de la touche `b` :
balance des blancs du boîtier, pas de luminosité automatique, aucun réglage. C'est
l'exposition réelle de la prise de vue, plus plate que les réglages par défaut de
darktable.

- **RAW** : `RawFile::render_deep`, rendu complet en **16 bits** directement dans
  l'espace demandé (`params.output_color` et `params.gamm` de LibRaw).
  | Espace | LibRaw | Profil ICC intégré |
  |---|---|---|
  | `srgb`, `""` | 1, courbe sRGB (1/2,4, 12,92) | sRGB |
  | `adobergb` | 2, gamma 2,2 | Adobe RGB |
  | `prophoto` | 4, linéaire | « Linear ProPhoto RGB », comme darktable |
  | `rec709` | 1, linéaire | « Linear Rec709 RGB » |
  | `rec2020` | 8, linéaire | « Linear Rec2020 RGB » |

  Display P3, un fichier `.icc` et le TIFF 32 bits demandent darktable : `libraw_check`
  le dit, la boîte d'export aussi, et le bouton reste grisé.
- **Images plates** (JPEG, TIFF…) et **JPEG boîtier** : leur espace d'origine est gardé
  ou converti (voir « Espace d'origine »).
- Réduction Lanczos3 en 16 bits (`image::imageops::resize`), puis écriture par le crate
  `image` : JPEG (8 bits, qualité du profil), TIFF ou PNG en 8 ou 16 bits.
- **Profils ICC** : générés par `moxcms`, la gestion couleur en pur Rust déjà compilée
  par `image` (voir [licensing.md](licensing.md)), puis intégrés aux fichiers. Aucun
  fichier `.icc` n'est livré.
- Même dossier, mêmes noms, même règle : rien n'est écrasé (`unique_path` : `_01`…).
  Une image en échec n'arrête pas les autres ; la raison est dans `Report::errors`.
- Mesuré le 19/10/2026 (deux RAF X-T4, Mac Apple Silicon) : TIFF 16 bits ProPhoto en
  pleine définition, 8,6 s pour les deux. Comparé visuellement à l'export darktable de la
  même image : cadrage, orientation et couleurs cohérents, darktable plus clair (ses
  réglages par défaut).

## Interface et ligne de commande

- `Ctrl+E` (`Cmd+E` sur macOS), action `core.file.export` : boîte sur la sélection (ou
  l'image courante).
  - **Profil** : flèches haut / bas ou clic ; le dernier choisi est retenu pendant la
    session.
  - **Dossier** : celui du profil, qui suit le choix du profil ; modifiable pour cette
    fois (complétion par Tab), avec la règle rappelée et la destination réelle de la
    première image.
  - **Export** : Entrée. Il se fait en fond, avec une barre de progression dans la barre
    d'état (bouton × pour l'arrêter), puis un bilan.
- `tri --export <profil> <image>... [--to <dossier>] [--libraw|--embedded] [--passthrough] [--dry-run]` :
  `--to` remplace le dossier du profil (même règle) ; `--libraw` force le rendu LibRaw,
  `--embedded` exporte le JPEG boîtier.

## Tests

- Unitaires (`export::tests`) : profils fournis et arguments, clés complétées par le
  profil de même identifiant, profils incohérents refusés (ProPhoto en JPEG compris, dès
  la lecture des réglages), paires, destination, lecture des lignes de progression.
- `darktable_exports_a_tiff_and_a_jpeg` (`#[ignore]`, `TRI_TEST_DARKTABLE_RAW`) : vrai
  export TIFF puis JPEG d'une copie, progression, taille, sidecar intact.
- Interface : `ctrl_e_offers_the_export_profiles_and_remembers_the_choice` ;
  fenêtre de réglages : `export_profiles_only_offer_coherent_choices`.
