# Module `tri::notes`

Notes de tri d'un dossier **en mémoire**, leur modification, et les filtres sur ces notes.
Les formats des sidecars sont l'affaire de [sidecars.md](sidecars.md) (XMP :
[xmp.md](xmp.md) ; RapidRAW : [rrdata.md](rrdata.md)) : depuis le 31/10/2026, une photo
a **tous ses sidecars** (lus dans le plus récent, tous écrits, les manquants créés selon
`[sidecars]`) ; `sidecars(image)` les donne, `sidecar(image)` le premier.

## `NoteBook` : les notes d'un dossier

- `NoteBook::load(images, convention)` / `load_files(&files, convention)` : lit les notes
  de toutes les images. Chaque dossier n'est listé qu'**une fois** (`xmp::SidecarIndex`),
  puis seuls les sidecars existants sont lus.
- `notes(image)` : notes en mémoire (vides pour une image sans sidecar ou inconnue) ;
  `error(image)` : sidecar illisible ou mal formé ; `sidecar(image)` : le sidecar lu, ou
  celui qui sera créé.
- `edit(image, |notes| …)` : modifie, **écrit**, puis renvoie les images dont les notes ont
  changé. En convention Adobe, `IMG.RAF` et `IMG.JPG` partagent `IMG.xmp` : noter l'un
  change l'autre, et les deux sont renvoyés pour être rafraîchis à l'écran. Après écriture,
  le dossier est relu (le sidecar a pu être créé). En cas d'erreur, la mémoire ne change
  pas : l'affichage reste fidèle au disque. Un sidecar mal formé n'est jamais modifié.
- **Paires** (choix du 17/10/2026) : `siblings(image)` donne les images de même nom à
  l'extension près, sans tenir compte de la casse (`IMG.JPG` pour `IMG.RAF`, mais pas
  `IMG_01.JPG`) ; `edit` leur donne les mêmes notes, quelle que soit la convention
  (convention darktable : `IMG.RAF.xmp` et `IMG.JPG.xmp` ; Adobe : un seul `IMG.xmp`,
  écrit une fois). Une paire déjà différente est réalignée. Un sidecar mal formé dans la
  paire est signalé (`XmpError::Image`, qui nomme l'image) sans empêcher les autres
  écritures. `shared(image)` donne tout ce qui reçoit les mêmes notes : la paire, et la
  photo avec ses développements principaux darktable et RapidRAW (XMP et `.rrdata`,
  issue n° 234, 31/10/2026 ; voir `developments.md`) ; l'historique en fait son
  instantané.
- `reload()` : relit tout (notes changées par un autre logiciel).
- **Lecture en fond** (23/10/2026, issue n° 15) : `NoteBook::pending(images, convention)`
  crée un carnet sans rien lire (notes vides) ; `load_counting(&files, convention,
  &compteur)` lit tout en comptant (dans un autre fil, `folder::Loading`) ; `adopt(lu)`
  reprend ce qui a été lu, **sauf** les images lues ou écrites entre-temps (une note posée
  pendant la lecture, un sidecar modifié ailleurs), plus fraîches ; `seed(&ancien)`
  garde les notes d'un carnet précédent du même dossier en attendant (dossier relu après
  un renommage) ; `is_pending()`. Une image notée avant la fin de la lecture est lue seule,
  avec sa paire.
- **Accès disque** (issues n° 15 et 29) : un sidecar n'est ouvert que s'il existe
  (`SidecarIndex::existing_sidecar`, sans `stat` quand il n'y en a qu'un) ; après une
  écriture, seules l'image et sa paire sont relues (avant : tout le dossier, à chaque
  note ; un sidecar n'est partagé qu'entre images de même nom).

Pourquoi tout en mémoire : **filtrer doit rester instantané** (`CLAUDE.md`, « analyse ≠
filtrage ») ; seule une modification touche le disque.

Mesure (17/10/2026, Mac Apple Silicon, release, copies de 131 vrais sidecars darktable) :
**7 ms** de chargement (23 ms à froid), soit ~0,06 ms par image. Sur un disque dur externe
(23/10/2026, 2 820 DNG et leurs sidecars Lightroom) : **15 s**, ~5 ms par image (un
déplacement de tête par fichier) ; d'où la lecture en fond ci-dessus.

Opérations sur `xmp::Notes` : `set_stars(n)` (le drapeau ne change pas : une photo rejetée
garde ses étoiles), `toggle_color(c, multiple)` (une seule couleur : une autre remplace, la
même retire ; plusieurs : ajout / retrait, comme darktable), `clear_colors()`.

`reload_sidecars(&sidecars)` relit seulement les images concernées par des sidecars
modifiés ailleurs (surveillance du dossier, `doc/watch.md`) et renvoie celles dont les
notes ont changé.

## Filtres

`NotesRule` regroupe les critères, combinés par « et » ; `None` = critère ignoré :

- `stars: Option<StarsRule>` : `AtLeast(n)`, `Exactly(n)` (`Exactly(0)` : non notées),
  `AtMost(n)` ;
- `flags: Option<Vec<Flag>>` : drapeaux acceptés (`[None, Accepted]` masque les rejetées) ;
- `colors: Option<ColorRule>` : l'une des couleurs `any_of`, et/ou sans couleur (`none`).

`rule.filter(&book)` donne un `NotesFilter`, qui implémente le même trait
[`Filter`](filter.md) que le filtre par type : pas de code spécial pour les filtres
natifs, comme le prévoit `CLAUDE.md`.
