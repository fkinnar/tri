# Module `tri::export` : exporter les images développées

Décidé le 19/10/2026 avec l'utilisateur : première étape de l'« après-développement »
(voir `CLAUDE.md`). On exporte des JPEG, TIFF ou PNG avec un **profil d'export** par
usage. Les usages de l'utilisateur sont les profils fournis :

- **Retouche** (`edit`) : TIFF 16 bits en ProPhoto, pleine définition, à destination d'un
  éditeur externe ;
- **Web** (`web`) : JPEG sRGB qualité 90, 2048 pixels au plus.

**Vocabulaire** (décidé le 20/10/2026) : l'interface et la ligne de commande disent
**« préréglage d'export »**, comme « préréglage de recadrage » : un même mot pour la même
idée, et « profil » reste aux **profils couleur** (ICC), sans ambiguïté dans la boîte
d'export. Le code (`Profile`) et le fichier de configuration (`[export.profiles.<id>]`)
gardent leurs noms, pour ne casser aucune configuration ; ce document dit encore
« profil » par endroits, au sens de préréglage.

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
- **Paires RAW+JPEG** : voir « Paires RAW + JPEG, mélanges » (les deux sont exportés).
- **Rien n'est écrit chez l'utilisateur** : configuration propre
  (`<cache>/darktable-export`, distincte de celle des rendus développés, pour que les deux
  puissent tourner en même temps), bibliothèque en mémoire, `write_sidecar_files=never`.
  Vérifié : sidecars identiques octet par octet après export.

## Profils fournis

| Profil | Format | Espace | Taille | Sans recompression | S'applique à |
|---|---|---|---|---|---|
| Retouche (`edit`) | TIFF 16 bits | ProPhoto linéaire | pleine | — | tout |
| JPEG tel quel (`asis`) | JPEG | d'origine (`""`) | pleine | oui | images développées |
| Web (`web`) | JPEG 90 | sRGB | 2048 px | oui | tout |

« JPEG tel quel » (demandé le 19/10/2026) : un JPEG est copié sans rien toucher (taille,
espace : un JPEG Adobe RGB le reste ; pas de qualité affichée, c'est celle du fichier
d'origine). **Pour exporter des JPEG seulement** (JPEG boîtier, JPEG d'origine) : un RAW
n'a pas d'espace couleur, son rendu doit en recevoir un. Quand on exporte un JPEG, il
est proposé d'abord.

**À quoi s'applique un profil** (`applies_to`, idée de l'utilisateur, 19/10/2026) :
`"all"` (défaut), `"raw"` (rendu d'un RAW : darktable, LibRaw) ou `"flat"` (image déjà
développée : JPEG boîtier, JPEG, TIFF…). « JPEG tel quel » vaut `"flat"`. La boîte ne
propose que les profils qui s'appliquent à ce qu'on exporte (`AppliesTo::allows`,
`exports_flat`), et la ligne de commande refuse les autres, avec la raison.

**Profil proposé** (19/10/2026) : `[export] default_raw` (rendu de RAW, défaut `edit`) et
`default_flat` (image déjà développée, défaut `asis`), plutôt qu'un paramètre par profil
(un seul choix par cas, jamais deux profils « par défaut »). Dans la séance, le dernier
profil choisi pour ce type d'export l'emporte. Un identifiant inconnu, ou un profil qui ne
s'applique pas, retombe sur le premier de la liste (`ExportSettings::default_index`).

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
| Grille | développée par le logiciel de son rendu développé (darktable, RapidRAW), sinon rendu LibRaw |
| Plein écran, développé (`Maj+B`) | développée par le logiciel affiché |
| Plein écran, rendu RAW (`b`) | rendu LibRaw |
| Plein écran, JPEG boîtier | le JPEG boîtier (aperçu embarqué, à sa taille) |

La boîte d'export ne propose **pas de choix** : on fait `Ctrl+E` sur l'image voulue
(retour de l'utilisateur, 19/10/2026 : pas de complexité de plus dans la boîte).
`Render::of` décide, une ligne d'explication le dit. En ligne de commande : darktable
s'il est là, `--libraw` ou `--embedded` pour les autres.

**Un développement est toujours rendu par son logiciel** (31/10/2026, modèle des
développements, voir `developments.md`) : une copie RapidRAW de la sélection part par
RapidRAW, un clone darktable par darktable, quoi qu'on regarde par ailleurs ; le tableau
ci-dessus ne vaut que pour les photos. `Renderers` porte les logiciels d'un export
(`photos` pour les photos, `darktable` et `rapidraw` pour leurs développements),
`Renderers::split` range les RAW, `run` lance chaque part à la suite, avec une seule
progression. Un développement dont le logiciel manque est exporté comme sa photo. Le
fichier porte le nom que **son logiciel** donne à ses exports (`export_name`) : `IMG_01`
pour le clone darktable n° 1, `IMG_VC01` pour la première copie virtuelle RapidRAW de la
photo ; un développement principal, le nom de la photo.

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

## Paires RAW + JPEG, mélanges

Retour de l'utilisateur (19/10/2026) : le JPEG d'une paire a été développé par l'appareil,
le RAW l'est par darktable ou LibRaw ; ce sont deux images différentes, toutes deux à
exporter.

- `Batch::new` range l'export : les RAW (rendus), les images déjà développées (JPEG,
  TIFF… : copiées, converties ou réduites), et parmi elles les **JPEG de paires** dont le
  RAW est aussi exporté. Ceux-ci prennent le suffixe `[export] camera_suffix` (`-camera`) :
  `DSCF0001.jpg` (rendu du RAW) et `DSCF0001-camera.jpg` (JPEG de l'appareil). Vérifié le
  19/10/2026 sur une vraie paire, avec le profil Web, en 12,8 s.
- Exception : en plein écran sur le **JPEG boîtier**, la paire ne part qu'une fois. Le
  JPEG boîtier et le fichier JPG sont alors la même image de l'appareil.
- `run` exporte les RAW par le moteur choisi (darktable, en passant par un TIFF
  temporaire s'il faut recadrer ou accentuer ; LibRaw ; JPEG boîtier), puis les images
  déjà développées, avec une seule progression et un seul bilan. La ligne de commande et
  l'interface passent toutes deux par là.
- **Profils proposés** (`Kind`) : RAW seuls, profils qui s'appliquent aux RAW ; images
  développées seules, ceux qui s'appliquent à elles ; **mélange**, seulement ceux qui
  s'appliquent à tout (`applies_to = "all"`), le plus petit dénominateur.
- La boîte le dit sous son titre, par exemple « 1 RAW, développé par darktable · 1 JPEG
  de l'appareil (paire, nom suffixé) ».
- Le profil proposé est retrouvé par son **identifiant** (le dernier choisi pour ce type
  d'export, sinon `default_raw` / `default_flat`), puisque la liste change selon ce qu'on
  exporte.

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

## Accentuation de sortie (`sharpen`)

Idée de l'utilisateur (19/10/2026), comme Lightroom : accentuer pour la destination, pour
compenser la douceur due à la réduction (écran) ou à l'encre (tirage). Clé `sharpen` de
chaque profil : `"none"` (Retouche, JPEG tel quel : l'éditeur externe ou rien),
`"screen"` (Web), `"print"`.

- **Masque flou** (`unsharp`), en 16 bits, **après la réduction**, comme Lightroom :
  `original + force × (original − flou gaussien)`, là où la différence dépasse un seuil.
  Le seuil évite d'accentuer le bruit. La fonction du crate `image` n'a pas de réglage de
  force, d'où une version maison.
- Paramètres dans `[export.sharpening]`, pour l'écran et le tirage :
  `screen_radius` / `screen_amount` / `screen_threshold`, et de même `print_*`. Par
  défaut : écran 0,6 px, force 0,6, seuil 2 niveaux ; tirage 1,2 px, force 0,9, seuil 3.
  Valeurs de départ, à ajuster à l'usage.
- **darktable** n'accentue pas pour nous : un profil qui accentue (ou une version) passe
  par `export_via_tiff`. darktable rend un TIFF 16 bits temporaire, en un seul
  lancement, qui est ensuite recadré, réduit, accentué et écrit. Une seule compression.
- Une **copie sans recompression** n'est pas accentuée : accentuer, c'est réencoder. Un
  profil qui accentue n'est donc jamais « tel quel ».
- Résumé dans la boîte : « … · accentuation écran ».

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

## Par RapidRAW

Les développements RapidRAW (le principal `IMG.RAF.rrdata`, les copies virtuelles
`IMG.RAF.<id>.rrdata`) sont exportés par RapidRAW (`export_rapidraw`, 31/10/2026), comme
ses rendus (`develop.md`) :

- **Un lancement par image** : `rapidraw export <photo> --output <tiff> --format tiff
  --tiff-bit-depth 16`, plus `--adjustments <json>` pour une copie virtuelle (ses réglages
  tirés de son `.rrdata` par `rrdata::adjustments`) ; le développement principal est
  appliqué par RapidRAW lui-même. La commande ne prend qu'une image et un jeu de
  réglages ; elle n'a ni taille, ni espace couleur, ni recadrage.
- **TIFF 16 bits temporaire, toujours en sRGB** : le seul espace que RapidRAW produit.
  L'image est ensuite **convertie** dans l'espace du profil (`moxcms`, comme une image
  déjà développée : « Espace d'origine »), recadrée (version), réduite, accentuée et
  écrite comme les autres (`finish`). Mêmes espaces qu'avec LibRaw (`libraw_check`) ; un
  espace plus large que sRGB n'y gagne rien, la boîte le dit (« RapidRAW rend en sRGB »).
- **Interface de RapidRAW ouverte** : image toute noire sans erreur (voir `develop.md`) ;
  refusée, avec le même message que pour les rendus. Une image en échec n'arrête pas les
  autres (`Report::errors`) ; les fichiers temporaires (`<cache>/rapidraw-export`) sont
  effacés.
- Une photo **sans** développement RapidRAW passe aussi par là quand on regarde un rendu
  RapidRAW en plein écran (`Render::Developed(RapidRaw)`) : réglages par défaut de
  RapidRAW.

## Créer un profil depuis l'interface

Demandé le 19/10/2026 : plus besoin d'éditer le fichier. Dans la fenêtre de réglages
(`Ctrl+,`), une seule entrée **« Profils d'export »** à gauche (retour du 20/10/2026 : une
entrée par profil encombrait la liste) ; à droite, l'explication commune, puis :

- une **liste déroulante** des profils (fournis, puis ceux de l'utilisateur) : le profil
  choisi se règle en dessous ;
- **« Nouveau… »** : demande un nom, puis copie le profil affiché sous ce nom. **Toutes**
  ses valeurs sont écrites, pour que le nouveau profil ne dépende pas des valeurs par
  défaut d'un autre. Identifiant tiré du nom (`versions::new_id`). Ensuite, il se règle
  comme les autres (listes déroulantes, refus des incohérences) ;
- **« Renommer… »** (issue n° 18, 22/10/2026) : le nom n'est plus un réglage modifiable
  parmi les autres (il se validait d'un Entrée, sans contrôle) ; il est dans la liste
  déroulante, et se change par ce bouton (`Naming::Rename` ; l'identifiant ne change
  pas). **Pas deux noms pareils** dans une famille (issue n° 17), casse et espaces mis
  à part (`versions::same_name`) : le nom passe en rouge, « Ce nom est déjà pris », et
  Créer / Renommer restent grisés. « Enregistrer comme préréglage » depuis le recadrage
  numérote un nom pris (« Story (2) », `versions::free_name`) ;
- **« Supprimer »**, pour les profils de l'utilisateur seulement (grisé sur un profil
  fourni : ils se modifient mais ne se suppriment pas).
- En interne, chaque profil reste une section (`SettingsWindow::schema` : sections de
  l'exemple, plus celles trouvées dans le fichier ; `ConfigFile::subtables`,
  `remove_table`) ; la liste de gauche n'en montre qu'une par famille (`listed`), et la
  recherche les trouve toutes (« Profil d'export « tirage » »).
- Appliqué à chaud : le profil est aussitôt proposé dans la boîte d'export.

Les préréglages de recadrage se créent de la même façon (voir `versions.md`).

## Interface et ligne de commande

- `Ctrl+E` (`Cmd+E` sur macOS), action `core.file.export` : boîte sur la sélection (ou
  l'image courante).
  - **Préréglage** : flèches haut / bas quand la liste a le clavier (au départ), ou clic ;
    Tab passe au dossier, où les flèches ne changent plus le préréglage (retour du
    20/10/2026). Le dernier choisi est retenu pendant la session.
  - **Dossier** : celui du profil, qui suit le choix du profil ; modifiable pour cette
    fois (complétion par Tab), avec la règle rappelée et la destination réelle de la
    première image.
  - **Export** : Entrée. Il se fait en fond, avec une barre de progression dans la barre
    d'état (bouton × pour l'arrêter), puis un bilan.
- **Rapport d'export** (`Ctrl+Maj+E`, action `core.file.export.report` ; idée de
  l'utilisateur du 20/10/2026) : la liste des exports terminés, les plus récents en
  haut, dans une zone à hauteur limitée qui défile. Pour chacun : préréglage, version,
  fichiers écrits sur images demandées, depuis quand (« il y a 3 min » : pas de
  bibliothèque de dates, et l'âge suffit), arrêt ou échec. Ses **dossiers** de
  destination (un par dossier d'images si le dossier est relatif) sont des liens qui les
  montrent dans le gestionnaire de fichiers (`files::reveal` sur leur premier fichier) ;
  la liste des fichiers et les échecs se déplient. Bouton « Rapport d'export » dans la
  barre d'état, **avant** le bilan d'un export (un long chemin ne le pousse pas hors de la
  fenêtre). « Effacer la liste » vide le journal : petit balai en bas à droite (bulle
  « Effacer la liste »), loin de Fermer (retour du 20/10/2026).
  - Moteur : `tri::export_log` (`Entry`, `read`, `append`, `clear`), fichier
    `exports.jsonl` du dossier de données (un objet JSON par ligne : un export ajouté ne
    réécrit pas le reste ; ligne illisible ignorée). Gardé d'une session à l'autre,
    limité aux `[export] report_max` plus récents (500 ; 0 : sans limite) pour ne pas
    grossir pendant des années.
- `tri --export <profil> <image>... [--to <dossier>] [--libraw|--embedded] [--passthrough] [--dry-run]` :
  `--to` remplace le dossier du profil (même règle) ; `--libraw` force le rendu LibRaw,
  `--embedded` exporte le JPEG boîtier.

## Tests

- Unitaires (`export::tests`) : profils fournis et arguments, clés complétées par le
  profil de même identifiant, profils incohérents refusés (ProPhoto en JPEG compris, dès
  la lecture des réglages), paires, destination, lecture des lignes de progression.
- `darktable_exports_a_tiff_and_a_jpeg` (`#[ignore]`, `TRI_TEST_DARKTABLE_RAW`) : vrai
  export TIFF puis JPEG d'une copie, progression, taille, sidecar intact.
- `developments_are_named_like_their_software_and_rendered_by_it` : noms d'export des
  développements (`IMG_02`, `IMG_VC01`), répartition par logiciel (`Renderers::split`) ;
  `rapidraw_exports_a_raw_reduced_to_the_profile` (`#[ignore]`, `TRI_TEST_RAW`, RapidRAW
  installé) : vrai export réduit au profil, temporaires effacés.
- Interface : `ctrl_e_offers_the_export_profiles_and_remembers_the_choice` ;
  fenêtre de réglages : `export_profiles_only_offer_coherent_choices`.
