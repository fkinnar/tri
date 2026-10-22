# Module `tri::sort` : ordre des images

Ajouté le 22/10/2026. L'ordre des images d'un dossier, au choix :

| Clé (`SortKey`) | Ordre | Usage |
|---|---|---|
| `Name` (défaut) | nom de fichier, celui de `media::scan` | l'ordre du boîtier |
| `Captured` | date de prise de vue (EXIF) | plusieurs boîtiers dans une séance, numérotation repartie à 0001, fichiers renommés |
| `Modified` | date de modification du fichier | exports, images retouchées |

`Order { key, descending }` ; `sort(fichiers, ordre, prise_de_vue)` trie sur place.

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

Dans l'interface : liste et flèche de sens à droite de la barre des filtres, `f s` (clé
suivante) et `f r` (sens), choix retenu d'un lancement à l'autre ; valeurs du premier
lancement `[gui] sort` et `sort_descending`.

Pas encore (bêta) : tri par note ou par couleur. Les piles et le jugement de netteté
s'appuient sur l'ordre de prise de vue (voisines, rafales) : à concevoir avec eux.
