# Module `tri::sort` : ordre des images

Ajouté le 22/10/2026. L'ordre des images d'un dossier, au choix :

| Clé (`SortKey`) | Ordre | Usage |
|---|---|---|
| `Name` (défaut) | nom de fichier, celui de `media::scan` | l'ordre du boîtier |
| `Captured` | date de prise de vue (EXIF) | plusieurs boîtiers dans une séance, numérotation repartie à 0001, fichiers renommés |
| `Modified` | date de modification du fichier | exports, images retouchées |
| `Stars` (31/10/2026) | étoiles, une rejetée sous 0 (comme `xmp:Rating = -1`) | les meilleures en tête (décroissant) |
| `Color` | couleurs dans l'ordre de `[sort] color_order` ; sans couleur, ou aucune de la liste, à la fin | une couleur par étape du tri |
| `Flag` | acceptée, sans drapeau, rejetée | les acceptées d'abord |

`Order { key, descending }` ; `sort(fichiers, ordre, prise_de_vue, notes, réglages)` trie
sur place. `SortKey::by_notes()` dit si la clé dépend des notes ; `SortKey::of_notes`
donne la valeur d'une image.

- **Paires RAW+JPEG gardées ensemble**, le RAW d'abord, dans les deux sens : une paire
  est classée d'après le premier de ses fichiers.
- **Clé inconnue à la fin**, dans les deux sens, dans l'ordre des noms (date de prise de
  vue pas encore lue, ou absente).
- **Dates de prise de vue** : fournies par l'appelant (`prise_de_vue(chemin)`). L'interface
  prend celles des empreintes de similarité (`similarity::Signature::time`), déjà lues en
  fond et gardées en cache d'analyse pour les piles : aucune lecture de plus sur le
  disque. En attendant leur arrivée, l'ordre reste celui des noms ; la grille est triée à
  nouveau une fois toutes là (et seulement si une date *nouvelle* est arrivée : refiltrer
  fait ré-annoncer celles du cache, piège déjà rencontré avec le filtre « bien
  exposées », issue n° 10).
- **Dates de modification** : lues sur le disque au moment du tri (`modified`).
- **Notes** : fournies par l'appelant (`notes(chemin)`, le carnet du dossier) ; la paire
  d'après son RAW, un développement d'après sa photo (même clé que les dates).
  `[sort] color_order` (`SortSettings`, éditeur à pastilles dans la fenêtre de réglages,
  voir `settings.md`) peut ne citer que quelques couleurs (demande de
  l'utilisateur, 31/10/2026 : rouge / jaune / vert ordonnées pour une chose, bleu et
  violet pour une autre) ; une image est classée d'après la mieux placée de ses couleurs
  (mode plusieurs couleurs), les autres vont à la fin avec les sans couleur, dans les
  deux sens, dans l'ordre des noms.

Dans l'interface : liste et flèche de sens à droite de la barre des filtres, `f s` (clé
suivante) et `f r` (sens), choix retenu d'un lancement à l'autre ; valeurs du premier
lancement `[gui] sort` et `sort_descending`.

Sous un tri par note, l'interface retrie dès qu'une note change (posée, annulée, modifiée
ailleurs : `TriApp::notes_changed`) : l'image prend sa place, la sélection la suit. Les
piles se forment sur l'ordre affiché (une rafale notée pareil reste contiguë, à égalité
l'ordre des noms départage) ; le jugement de netteté reste celui du moteur, sur l'ordre
de prise de vue.
