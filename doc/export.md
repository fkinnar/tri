# Module `tri::export` : exporter les images développées

Décidé le 19/10/2026 avec l'utilisateur : première étape de l'« après-développement »
(voir `CLAUDE.md`). On exporte des JPEG, TIFF ou PNG avec un **profil d'export** par
usage. Les usages de l'utilisateur sont les profils fournis :

- **Retouche** (`edit`) : TIFF 16 bits en ProPhoto, pleine définition, à destination d'un
  éditeur externe ;
- **Web** (`web`) : JPEG sRGB qualité 90, 2048 pixels au plus.

Étapes suivantes, prévues : versions recadrées dans `tri` (stockées comme instructions dans
le XMP), puis un repli LibRaw sans darktable.

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
profil `.icc`.

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
- `tri --export <profil> <image>... [--to <dossier>] [--dry-run]` : `--to` remplace le
  dossier du profil (même règle).

## Tests

- Unitaires (`export::tests`) : profils fournis et arguments, clés complétées par le
  profil de même identifiant, profils incohérents refusés (ProPhoto en JPEG compris, dès
  la lecture des réglages), paires, destination, lecture des lignes de progression.
- `darktable_exports_a_tiff_and_a_jpeg` (`#[ignore]`, `TRI_TEST_DARKTABLE_RAW`) : vrai
  export TIFF puis JPEG d'une copie, progression, taille, sidecar intact.
- Interface : `ctrl_e_offers_the_export_profiles_and_remembers_the_choice` ;
  fenêtre de réglages : `export_profiles_only_offer_coherent_choices`.
