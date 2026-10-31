# Module `tri::xmp`

Notes de tri d'une photo — **étoiles** (0–5), **couleurs**, **accepté / rejeté** — lues et
écrites dans ses sidecars XMP, en restant lisibles par darktable, Lightroom, digiKam et
Capture One. Décisions de format : `CLAUDE.md`, « Notation XMP ».

## Principe : ne toucher qu'à nos champs

Un sidecar contient bien plus que des notes : historique de développement darktable,
mots-clés, auteur, droits… `tri` ne doit rien en perdre. Le XMP est donc relu **événement
par événement** (`quick-xml`) et réécrit **à l'identique**, sauf nos champs :

- la balise `rdf:Description` qui porte nos attributs est modifiée **dans son texte** :
  valeur changée sur place, attribut retiré avec l'espace qui le précède, nouvel attribut
  ajouté en fin de balise avec l'espacement du dernier (un attribut par ligne chez exiv2) ;
- un champ écrit sous forme d'élément (`<xmp:Rating>3</xmp:Rating>`) est retiré et remplacé
  par un attribut ;
- les champs sont reconnus par leur **espace de noms**, pas par leur préfixe : Bridge écrit
  `xap:Rating`, que `tri` comprend et met à jour sous le même préfixe ;
- si rien ne change, le fichier n'est pas réécrit du tout ; sinon, écriture atomique
  (fichier temporaire puis renommage) ;
- un sidecar **mal formé** (XML invalide, ou sans `rdf:Description`) n'est jamais réécrit :
  `XmpError::Malformed` / `NoDescription` ;
- seul un `rdf:Description` **de premier niveau** porte nos champs : un autre peut être
  niché dans une propriété (`Iptc4xmpCore:CreatorContactInfo` de Lightroom, coordonnées
  du photographe) et reste tel quel (issue n° 27, 23/10/2026 : pris pour le principal, il
  faisait perdre les champs placés après lui, et une version « enregistrée » n'était
  jamais écrite) ; une liste (`write_list`) qui ne se relit pas est une erreur, jamais un
  succès silencieux.

## Les champs

| Note | Champ | Valeurs | Lu par |
|---|---|---|---|
| Étoiles | `xmp:Rating` | 0–5 (`-1` = rejeté, standard ISO 16684-1) | tous |
| Accepté / rejeté | `xmpDM:pick` | 1 / 0 / -1 | Lightroom Classic ≥ 13.2 |
| | `xmpDM:good` | `True` / absent / `False` | Lightroom Classic ≥ 13.2 |
| | `digiKam:PickLabel` | 3 / 0 / 1 (2 = en attente, lu comme « rien ») | digiKam |
| Couleurs | `xmp:Label` | `Red`, `Yellow`, `Green`, `Blue`, `Purple` | Lightroom, Capture One, darktable |
| | `darktable:colorlabels` | séquence de 0 (rouge) à 4 (violet) | darktable |
| | `digiKam:ColorLabel` | 1 rouge, 3 jaune, 4 vert, 5 bleu, 6 magenta | digiKam |
| Étoiles d'une photo rejetée | `tri:RatingBeforeReject` (`urn:tri:xmp:1.0`) | 1–5 | tri |

Règles d'écriture :

- **rejeté** : `xmp:Rating = -1` (ce que voient les autres logiciels) et les étoiles
  mémorisées dans `tri:RatingBeforeReject` : annuler le rejet les restaure ;
- **pas de bruit** : un champ qui vaudrait 0 n'est ajouté que s'il existait déjà (on met à
  jour `xmp:Rating="0"` de darktable, on n'ajoute pas `xmpDM:pick="0"` partout) ;
- **une seule couleur** : `xmp:Label`, `digiKam:ColorLabel` et `darktable:colorlabels` ;
  **plusieurs** : seulement `darktable:colorlabels`. darktable lit `xmp:Label` *avant* ses
  propres couleurs (vérifié dans son code source) : l'écrire avec plusieurs couleurs lui en
  ferait perdre.

Règles de lecture : rejeté si `xmp:Rating = -1`, `xmpDM:pick = -1` ou
`digiKam:PickLabel = 1` ; accepté si `xmpDM:pick = 1` ou `digiKam:PickLabel = 3`. Couleurs :
`xmp:Label` d'abord (comme darktable), sinon `darktable:colorlabels`, sinon
`digiKam:ColorLabel`. Les étoiles écrites « 3.0 » (certains logiciels) sont acceptées.

## Quel sidecar

| Convention | Nom | Utilisée par |
|---|---|---|
| `Darktable` (défaut) | `DSCF0092.RAF.xmp` | darktable, digiKam : un sidecar par fichier |
| `Adobe` | `DSCF0092.xmp` | Lightroom, Capture One, Bridge : partagé par le RAF et le JPG d'une paire |

- s'il existe des sidecars, on **lit et écrit le plus récent** (date de modification) ;
- sinon, on en crée un selon la convention de `[xmp] sidecar` : un utilisateur de Capture
  One règle `"adobe"` une fois pour toutes ;
- la recherche liste le dossier et compare les noms **sans tenir compte de la casse**
  (`IMG.XMP` est trouvé), avec les vrais noms du disque : sur macOS, `IMG.XMP` et `IMG.xmp`
  sont le même fichier, sur Linux deux fichiers distincts. `SidecarIndex` liste un dossier
  une seule fois pour toutes ses images.

Un nouveau sidecar est un paquet XMP minimal (`x:xmptk="tri"`). darktable, à l'import,
lit nos notes puis réécrit le fichier à sa façon.

## Conversion d'une convention à l'autre

```
tri <dossier> --convert-xmp darktable|adobe [--dry-run] [--raw|--flat] [--depth N]
```

`xmp::convert(images, cible, simulation)` recopie les notes de chaque image, lues dans son
sidecar le plus récent, dans le sidecar de la convention cible (créé s'il manque ; s'il
existe, seuls nos champs changent) :

- **aucun sidecar n'est supprimé** : celui de darktable garde l'historique de
  développement ;
- le sidecar cible devient le plus récent, donc celui que `tri` lit et écrit ensuite (s'il
  était déjà à jour, seule sa date change) ;
- pas de bruit : une image sans notes n'obtient pas de sidecar ;
- vers Adobe, `IMG.RAF` et `IMG.JPG` partagent `IMG.xmp` : notés différemment, c'est un
  **conflit**, la première image (le RAW, dans l'ordre de `media::scan`) l'emporte et
  l'autre est signalée ; vers darktable, `IMG.xmp` est recopié dans `IMG.RAF.xmp` et
  `IMG.JPG.xmp` ;
- `--dry-run` liste ce qui serait écrit ; relancer une conversion ne change rien.

Vérifié (17/10/2026) sur des copies des 131 sidecars darktable réels : 18 sidecars Adobe
créés (les 113 autres photos n'ont pas de notes), notes relues à l'identique par ExifTool,
fichiers valides, sidecars darktable inchangés octet par octet, seconde conversion sans
effet.

## API

- `read(image) -> Notes`, `write(image, &notes, convention) -> chemin du sidecar` ;
  `read_sidecar(chemin)`, `write_sidecar(chemin, &notes)` : un sidecar précis ;
- `convert(images, convention, simulation) -> Conversion` (bilan : écrits, inchangés,
  conflits, erreurs) ;
- `parse(xml) -> Notes`, `apply(xml, &notes) -> xml` : le même travail sur un texte ;
- `strip_darktable(xml) -> xml` (30/10/2026) : le texte sans le développement darktable
  (attributs et éléments de son espace de noms, sauf `colorlabels`, une note ; le reste
  octet pour octet), pour « supprimer le développement principal » (`developments.md`) ;
- `SidecarIndex::new(dossier)`, `.sidecars(image)`, `.sidecar_path(image, convention)` ;
  `existing_sidecars`, `sidecar_path` (une image isolée) ;
- `Notes { stars, flag, colors }`, `Flag { None, Accepted, Rejected }`,
  `Color { Red, Yellow, Green, Blue, Purple }`, `Convention { Darktable, Adobe }`.

## Vérification sur de vrais sidecars (17/10/2026)

Sur des **copies** des 131 sidecars darktable d'une séance réelle (dont deux copies
virtuelles avec ~750 lignes d'historique de développement) :

- réécriture sans changement : **131 fichiers identiques octet par octet** ;
- notes variées écrites (étoiles, accepté, rejeté avec étoiles, une et deux couleurs), puis
  relues par **ExifTool**, référence indépendante : toutes les notes relues à l'identique,
  **tous les autres champs inchangés**, et ExifTool valide les fichiers (`-validate` : OK).

## À vérifier en conditions réelles

- darktable : **vérifié par l'utilisateur le 17/10/2026**, les notes posées par `tri`
  passent dans darktable (avec un délai avant qu'il les prenne en compte). Il ne relit un
  XMP modifié ailleurs que si l'option « rechercher les fichiers XMP mis à jour au
  démarrage » est active ; s'il tourne pendant le tri, il peut réécrire le sidecar depuis
  sa base et effacer nos notes ;
- Lightroom, Capture One, digiKam : à faire vérifier par des testeurs ;
- Capture One : comportement avec `xmp:Rating = -1` et `xmp:Label` non documenté ;
- Lightroom : bug connu (13.2) si `xmpDM:pick` est absent du fichier.
