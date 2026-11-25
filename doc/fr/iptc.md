# Métadonnées de création (`tri::iptc`, #277)

Auteur, copyright, conditions d'utilisation, légende, titre, lieu : les métadonnées
qu'un photographe pose dans ses images. `tri` les écrit en **XMP** seulement (décidé le
25/11/2026) : c'est ce que lisent Lightroom, darktable, Capture One, Photos, Bridge et
l'Explorateur Windows récent. Pas d'IPTC-IIM (l'ancien format binaire, segment APP13 des
JPEG), utile seulement à de très vieux logiciels.

## Jeux

Un ou plusieurs **jeux** dans la configuration, `[iptc.sets.<id>]`, un proposé par défaut
(`[iptc] default`). Le jeu `mine` (« Mes métadonnées ») est fourni vide, à remplir ; il sert
de modèle à ceux créés depuis la fenêtre de réglages (« Nouveau… »).

| Clé | Propriété XMP | Forme |
|---|---|---|
| `creator` | `dc:creator` | liste (`rdf:Seq`) ; plusieurs auteurs séparés par « ; » |
| `copyright` | `dc:rights` | texte par langue (`rdf:Alt`, `x-default`) |
| `usage_terms` | `xmpRights:UsageTerms` | texte par langue |
| `web_statement` | `xmpRights:WebStatement` | texte (une adresse) |
| `credit` | `photoshop:Credit` | texte |
| `title` | `dc:title` | texte par langue |
| `caption` | `dc:description` | texte par langue |
| `city`, `state`, `country` | `photoshop:City`, `State`, `Country` | texte |

Avec un copyright, `xmpRights:Marked = True` (« protégé par copyright » dans Lightroom et
Bridge) est posé s'il est absent. Variables dans les valeurs : `{year}` (l'année de la date
du fichier : la prise de vue pour une photo du boîtier, l'export pour un fichier exporté),
`{name}` (le nom de l'image, sans extension).

## Ne jamais écraser par surprise

Un champ n'est posé que s'il est **vide** dans l'image (absent, ou sans texte), sauf ceux
que le jeu cite dans `replace` (`replace = ["copyright"]` : le copyright est toujours le
sien). Une légende écrite ailleurs n'est jamais remplacée sans qu'on l'ait demandé. Tout le
reste du paquet est recopié tel quel (vérifié sur un vrai sidecar darktable : historique
de développement identique pour ExifTool).

## Où, quand

- **Où** : une image qui porte un XMP embarqué (JPEG, TIFF, PNG, WebP, HEIF) le reçoit
  dans le fichier, **sans recompression** : seul le paquet XMP change
  (`embedded::rewrite`, la mécanique des notes inscrites dans les images, généralisée par
  `embedded::Change`). Un RAW le reçoit dans son **sidecar** (le plus récent, sinon celui
  de `[xmp] sidecar`), que les logiciels de développement reprennent à l'export.
- **À l'export** : la clé `iptc` d'un préréglage d'export (`iptc = "mine"`) pose le jeu sur
  chaque fichier écrit, une copie sans recompression comprise (`Request::iptc`, à la fin de
  `export::run`).
- **Sur demande** : l'action `core.file.iptc` (palette, sans raccourci par défaut) ouvre la
  boîte « Métadonnées de création » sur la sélection : le jeu (liste s'il y en a
  plusieurs), ce qu'il pose (« remplace l'existant » pour ceux de `replace`), « Poser »
  (en fond), « Modifier le jeu… » (la fenêtre de réglages sur sa page). Un jeu vide le dit
  et renvoie aux réglages.
- **Ligne de commande** : `tri --iptc <jeu> <image>…|<dossier> [--dry-run]`.
- **Panneau des métadonnées** : les champs présents (XMP embarqué, sinon sidecar le plus
  récent) sous les autres lignes (`iptc::read`, `Metadata::creation`).

## API

- `xmp::read_property` / `xmp::apply_property` : une propriété XMP quelconque (`Property`,
  forme `Shape::Text`, `Seq` ou `Alt`), lue en attribut ou en élément d'un
  `rdf:Description` de premier niveau, écrite en élément dans le premier (la forme
  attribut est retirée) ; `apply_list` (versions, réglages de `tri`) passe par là.
- `iptc::apply(xml, set, vars)` : le paquet avec le jeu ; `iptc::stamp(image, set,
  convention, dry_run)` : dans l'image ou son sidecar ; `iptc::read(image)` : les champs
  présents.

## Tests

`a_set_fills_empty_fields_and_replaces_only_those_listed`,
`a_raw_gets_its_sidecar_and_a_jpeg_its_own_packet`, `the_default_set_is_chosen_or_the_first`
(`src/iptc.rs`), `the_preset_s_creation_metadata_go_into_the_written_files` (export),
`creation_metadata_go_into_the_selected_images` (interface). Vérifié à la main avec
ExifTool (JPEG validé, sidecar darktable).
