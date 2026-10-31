# Module `tri::developments` : les développements d'une photo

Décidé le 30/10/2026 : tout ce qui **rend** une photo est un *développement*, au même
niveau : le développement principal de **darktable** (son XMP, quand il a été écrit par
darktable) et ses **clones** (`IMG_01.RAF.xmp` sans image `IMG_01.RAF`), le principal de
**RapidRAW** (`IMG.RAF.rrdata`) et ses **copies virtuelles** (`IMG.RAF.<id>.rrdata`). Le
rendu LibRaw neutre, c'est la photo elle-même ; le JPEG boîtier est son affichage initial,
pas un développement. Les « versions » gardent leur sens dans `tri` : les recadrages.

## Un développement est une image de la grille

Un développement n'a pas de fichier image : il a un **chemin propre**, jamais sur le
disque, dans le dossier de la photo et avec son extension : `IMG@dt.RAF`, `IMG@dt-01.RAF`
(clone 1), `IMG@rr.RAF`, `IMG@rr-c33b94.RAF`. Le reste de `tri` (cache, notes, interface,
exports) le traite comme une image :

- les **pixels** sont ceux de la photo (`media::source(path)` rend le fichier à lire) ;
- les **notes** sont les siennes, là où son logiciel les lit : le XMP du clone, le
  `.rrdata` de la copie ; la photo et le développement principal de darktable partagent
  `IMG.RAF.xmp` (`NoteBook`, `shared_with`). `tri` n'écrit que ses champs et ne crée jamais
  lui-même un `IMG_NN.RAF.xmp` ;
- le **rendu** est celui de son sidecar (`Engine::sidecar` le résout ; une copie RapidRAW
  passe par `--adjustments`).

Une photo qui a des développements d'un logiciel **n'a plus de rendu « développé » à
elle** pour ce logiciel (`Engine::sidecar(photo)` rend `None`) : son sidecar est celui du
développement, qui a sa vignette ; la photo reste le JPEG boîtier et le rendu LibRaw. Une
photo sans développement garde le comportement d'avant (`Maj+B` la développe à la
demande, `b` fait le tour des rendus).

## Fonctions

- `find(photo, names, images, is_darktable)` : les développements d'une photo d'après les
  noms des fichiers de son dossier, dans l'ordre : principal darktable (si son XMP est de
  darktable : `is_darktable` lit son début, `darktable:xmp_version` ou un historique),
  clones par numéro (de darktable aussi : un `IMG_01.RAF.xmp` orphelin écrit par un
  autre logiciel, digiKam par exemple, n'est pas un clone), principal RapidRAW, copies
  par identifiant. Un XMP de Lightroom, Capture One ou digiKam ne déclenche donc rien.
- **Registre** (global, rempli par `media::scan`) : `set_for(photo, developments)`
  remplace les développements connus d'une photo (un clone supprimé n'est plus connu),
  `register` en ajoute un ; `get(path)` rend le développement d'un chemin propre, `of(photo)`
  ceux d'une photo dans l'ordre, `has(photo, engine)` dit si elle en a un de ce logiciel,
  `order(path)` le rang d'un développement (pour trier).
- `Development::path()`, `own_file()` (le fichier qu'on peut supprimer sans toucher à la
  photo : clone, copie virtuelle, ou `.rrdata` du principal RapidRAW ; jamais le XMP du
  principal darktable, partagé avec la photo), `notes_in_rrdata()`.
- **Supprimer un développement** (décidé le 30/10/2026) : `removal()` dit ce que ça
  touche, `Removal::File` (le fichier propre, corbeille) ou `Removal::Strip` (principal
  darktable : son XMP réécrit **sans l'historique**, `xmp::strip_darktable`, tout ce que
  darktable y écrit sauf `colorlabels`, une note ; les notes restent et la photo n'a plus
  de développement). `Rewrite::strip(development)` prépare la réécriture (contenu avant et
  après, rien d'écrit), `apply()` / `revert()` l'écrivent ou la défont, en relisant le
  fichier juste avant : modifié entre-temps, rien n'est écrit (`false`). Côté darktable, sa
  bibliothèque garde l'historique tant qu'il ne relit pas le XMP (option « rechercher les
  XMP mis à jour au démarrage »). Le plan complet est dans `files.md` (`plan_delete`,
  `delete_plan`), l'annulation dans `history.md` (`Step::Developments`).
- `Development::rank(siblings)` : sa place pour l'étiquette, `Rank::Main`,
  `Rank::Clone(n)` (le numéro du fichier) ou `Rank::Copy(n)` (le rang de la copie virtuelle
  parmi celles de la photo, la première = 1 : l'identifiant ne dit rien).
- `latest(developments)` : le plus récent d'après la date de son sidecar.
- `preferred(data)`, `prefer(data, development, on)` : le développement préféré d'une
  photo (couverture de sa pile), un par photo, dans `preferred-developments.txt` du dossier
  de données.

## Ordre dans la liste

`media::compare` range un développement **avec sa photo**, juste après elle et avant
l'autre fichier de la paire : `IMG.RAF`, `IMG@dt.RAF`, `IMG@rr.RAF`, `IMG.JPG`, dans l'ordre
du registre (pas celui des noms propres : `IMG@dt-01` passerait avant `IMG@dt`, `IMG@dt`
après `IMG2`). Les tris par date (`sort`) donnent au développement la clé de sa photo.

## Dans l'interface

Voir `gui.md`, « Piles de développements » : une photo est une pile de ses développements,
repliée par défaut (couverture et « N dév. »), dépliable (`Maj+V`) ; `Maj+W` / `Maj+Q`
passent de l'un à l'autre ; en plein écran un développement montre son rendu, `b` montre
le JPEG boîtier à sa place (avant / après).

## Export

Un développement est exporté par **son logiciel** (`Ctrl+E`, 31/10/2026 : les copies
RapidRAW par RapidRAW, les clones par darktable, quoi qu'on regarde), sous le nom que ce
logiciel donne à ses exports (`IMG_01`, `IMG_VC01`) ; voir `export.md`, « On exporte ce
qu'on regarde » et « Par RapidRAW ».

## À venir

Comparaison des développements d'une photo ; le compte « N images » de la barre d'état
compte les développements ; leurs vignettes ne se calculent en fond qu'avec
`[gui.grid] develop = "auto"`.
