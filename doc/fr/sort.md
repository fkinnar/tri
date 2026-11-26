# Module `tri::sort` : ordre des images

Ajouté le 22/10/2026. L'ordre des images d'un dossier, au choix :

| Clé (`SortKey`) | Ordre | Usage |
|---|---|---|
| `Name` (défaut) | nom de fichier, quel que soit le dossier (22/11/2026 : « Toutes les photos » et les sous-dossiers étaient rangés dossier par dossier), puis le chemin | l'ordre du boîtier |
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
  exposées », issue n° 10). Quand les empreintes ne sont pas calculées en fond (mode
  léger, où les analyses attendent « Tout calculer » ; piles à la demande ou coupées),
  l'interface lit seulement les dates (`preview::capture_time` : ouverture LibRaw sans
  rien décompresser pour un RAW, EXIF d'une image plate), gardées dans le cache avec les
  empreintes (date absente comprise, pour ne pas relire le fichier). Avant le 26/11/2026,
  le tri restait sur les noms dans un dossier en mode léger. Ces lectures se font sur
  autant de fils que de workers légers (`[scheduler] light_workers` : sur un NAS, le
  temps est surtout l'attente du réseau), leur avancement est montré dans les tâches de
  fond (« dates de prise de vue i/N »), et le tri n'attend plus la progression des piles
  quand elles ne sont pas calculées en fond (en mode léger, elle passait derrière toutes
  les vignettes). À partir de `[gui] sort_dates_ask` dates à lire (200 ; 0 : jamais),
  une boîte prévient d'abord (« Trier par date de prise de vue ? », `dates_dialog.rs`,
  idée de l'utilisateur du 26/11/2026) : Trier, Annuler (l'ordre ne change pas), « Ne
  plus demander » (réglage mis à 0). Depuis le même jour, les dates sont lues seules
  **même quand les empreintes sont calculées en fond** (2 820 DNG sur un NAS en NFS :
  le tri attendait toutes les empreintes, bien plus longues) ; une empreinte qui arrive
  avec une date déjà lue ne relance pas de tri. Un RAW dont LibRaw n'a pas la date (des
  DNG de 2021) la prend dans l'EXIF du fichier (`metadata::exif_date`), puis dans celle
  du JPEG embarqué, enfin dans son XMP (`metadata::xmp_date`). Quand toutes les dates sont lues, la barre d'état dit combien
  manquent (« … photos sans date »), et le journal les nomme (`sort`).
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

## Proximité (`SortKey::Proximity`, #492, 23/11/2026)

Idée de l'utilisateur : « le filtre filtre, le tri trie ». Les recherches de palettes
(couleurs, couleurs proches) et « Semblables » ne font que **filtrer** ; le tri
**Proximité** range les images gardées de la plus proche à la plus lointaine (inversé :
l'inverse ; celles pas encore calculées à la fin). Le moteur n'a pas les palettes : ici,
cette clé donne l'ordre des noms, et l'interface classe (`palette::rank_similar_by`, sous
un autre tri `palette::keep_similar`, qui garde l'ordre). Dans l'interface : une
recherche qui commence passe le tri à Proximité, et l'ordre d'avant revient quand elle
s'arrête, sauf si l'on a choisi un autre tri entre deux ; sans recherche, Proximité est
grisée dans la liste, sautée par le tri suivant, et jamais retenue (dossier ou session :
l'ordre d'avant).
