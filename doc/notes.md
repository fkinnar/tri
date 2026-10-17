# Module `tri::notes`

Notes de tri d'un dossier **en mémoire**, leur modification, et les filtres sur ces notes.
Le format des sidecars est l'affaire de [xmp.md](xmp.md).

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
- `reload()` : relit tout (notes changées par un autre logiciel).

Pourquoi tout en mémoire : **filtrer doit rester instantané** (`CLAUDE.md`, « analyse ≠
filtrage ») ; seule une modification touche le disque.

Mesure (17/10/2026, Mac Apple Silicon, release, copies de 131 vrais sidecars darktable) :
**7 ms** de chargement (23 ms à froid), soit ~0,06 ms par image. Synchrone à l'ouverture
d'un dossier pour l'instant ; à passer en tâche de fond si une carte SD lente le demande.

Opérations sur `xmp::Notes` : `set_stars(n)` (le drapeau ne change pas : une photo rejetée
garde ses étoiles), `toggle_color(c, multiple)` (une seule couleur : une autre remplace, la
même retire ; plusieurs : ajout / retrait, comme darktable), `clear_colors()`.

## Filtres

`NotesRule` regroupe les critères, combinés par « et » ; `None` = critère ignoré :

- `stars: Option<StarsRule>` : `AtLeast(n)`, `Exactly(n)` (`Exactly(0)` : non notées),
  `AtMost(n)` ;
- `flags: Option<Vec<Flag>>` : drapeaux acceptés (`[None, Accepted]` masque les rejetées) ;
- `colors: Option<ColorRule>` : l'une des couleurs `any_of`, et/ou sans couleur (`none`).

`rule.filter(&book)` donne un `NotesFilter`, qui implémente le même trait
[`Filter`](filter.md) que le filtre par type : pas de code spécial pour les filtres
natifs, comme le prévoit `CLAUDE.md`.
