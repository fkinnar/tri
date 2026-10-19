# Module `tri::overlay` : superpositions

Idée de l'utilisateur (19/10/2026) : vérifier en plein écran qu'une photo convient à un
usage, par exemple qu'un texte ou un logo reste lisible à tel endroit, ou qu'elle tient
dans un format de publication (4:5, 9:16). Plus des grilles de composition (tiers, nombre
d'or, diagonales). Premier « plugin » de `tri`, pour éprouver l'architecture prévue dans
le `CLAUDE.md`.

## Des formes, jamais des pixels

Famille « overlay géométrique pur » : le moteur ne calcule rien sur l'image. Le trait
`Overlay` renvoie des **formes vectorielles** (`Shape` : segment, rectangle, texte, image)
en coordonnées relatives à l'image, (0, 0) en haut à gauche et (1, 1) en bas à droite.
L'interface les dessine à sa résolution, zoom compris ; un futur TUI ferait ce qu'il peut.
`shapes(aspect)` reçoit le rapport largeur / hauteur de l'image : un cadre 4:5, une
marge ou une diagonale réciproque en dépendent.

## Deux sortes de plugins, un seul trait

- **Écrits en Rust** et compilés avec `tri` : les grilles intégrées (`builtin`, `Grid` :
  tiers, nombre d'or, spirale d'or — demandée le 19/10/2026 : « certains aiment » —,
  diagonales et diagonales réciproques). La spirale : quarts de cercle dans les carrés
  successifs du rectangle d'or, approchés par des segments, étirée sur l'image comme dans
  Lightroom (pivotée pour une image en hauteur, sans quoi elle serait écrasée) ;
  retournable dans un fichier (`flip`). Couleur et opacité :
  `[gui.colors] overlay`, `[gui.view] overlay_opacity`.
- **Définis par l'utilisateur, sans code** : un fichier TOML par superposition dans le
  dossier `overlays` du dossier de données (`load_dir`, `user_dir`), lu à chaque pression
  de la touche (un fichier modifié se voit tout de suite). Un fichier mal écrit est
  signalé (barre d'état) sans empêcher les autres.

Pas de `libloading` ni de langage de script : comme décidé, des traits statiques et une
liste tant qu'aucun plugin externe n'en a besoin.

## Format d'un fichier

Exemple complet et commenté : `overlay.example.toml` (un test vérifie qu'il se lit). Un
nom (défaut : le nom du fichier) et des formes `[[shape]]`, selon leur `kind` :

| `kind` | Clés | Défauts |
|---|---|---|
| `text` | `text`, `anchor`, `size` (hauteur, fraction du petit côté), `margin`, `color`, `opacity` | bas droite, 0,04, 0,03, blanc, 1 |
| `image` | `path` (relatif au fichier, ou `~/…`), `anchor`, `width` (fraction de la largeur), `margin`, `opacity` | bas droite, 0,15, 0,03, 1 |
| `frame` | `ratio` (`"4:5"`, largeur:hauteur), `color`, `opacity`, `shade` (voile hors cadre) | blanc, 1, 0,5 |
| `grid` | `grid` (`thirds`, `golden`, `spiral`, `diagonals`), `color`, `opacity`, `flip` (`x`, `y`, `xy`) | blanc, 0,6, aucun |

Ancrages : `top-left`, `top`, `top-right`, `left`, `center`, `right`, `bottom-left`,
`bottom`, `bottom-right`. Une clé inconnue est une erreur (faute de frappe signalée).

**Les formes qui suivent un cadre se placent dans ce cadre** : le logo et le texte d'une
publication 4:5 tombent dans la publication, pas dans le hors-cadre qui sera coupé, et
une grille après un cadre est celle du recadrage.

## Dans l'interface

`o` : superposition suivante (aucune, tiers, nombre d'or, spirale, diagonales, puis celles de
l'utilisateur), comme dans Lightroom ; son nom dans la barre d'état. `Maj+O` fait tourner
les lignes (l'œil de la spirale passe d'un coin à l'autre, 4 orientations, gardées d'une
image à l'autre) ; un texte ou un logo ne se retournent pas. En plein écran et en
comparaison, sous le repère du collimateur. Traits de `[gui.view] overlay_line_width`
points, avec un liseré sombre ; logos chargés une fois (rechargés si les fichiers sont
relus).
