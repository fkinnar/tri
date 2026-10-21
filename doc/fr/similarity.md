# Module `tri::similarity` : regrouper les images semblables

Décidé le 18/10/2026 avec l'utilisateur, sur le modèle de Capture One : des **piles**
automatiques (rafales, séries d'une même scène), un **curseur de seuil** qui regroupe en
direct. Ce module est le moteur ; l'affichage en piles est dans l'interface.

## Empreinte (`Signature`)

Calculée une fois par image, sur une petite version (`[similarity] side`, 64 px : le JPEG
boîtier d'un RAW, l'image plate réduite), et gardée dans le cache d'analyse
(`Level::Similarity`, ~70 octets) :

- **composition** : 9 × 8 cases de luminance, chacune comparée à sa voisine de droite,
  64 bits (« dHash ») ; insensible aux petits décalages, à l'exposition et aux détails ;
- **couleurs** : couleur moyenne de 4 × 4 cases ; sépare deux scènes de même
  composition (ciel et mer, intérieur et extérieur) ;
- **heure de prise de vue** (EXIF / LibRaw), en secondes : seuls les écarts servent.

Distance de 0 (identiques) à 1 : `shape_weight` × (bits différents / 64) + le reste ×
écart moyen des couleurs (×4, plafonné : un écart de 10 % est déjà très visible).

## Regroupement

Les images sont prises **dans l'ordre de la grille** (celui de la séance) : deux voisines
sont du même groupe si leur distance est sous le seuil (`threshold`, 0,25 par défaut) et
leur écart de temps sous `max_gap_s` (60 s ; 0 : sans limite). `distances` calcule les
liens entre voisines une fois ; `group` n'est ensuite qu'un parcours de la liste :
instantané, d'où le curseur en direct. Un lien inconnu (empreinte pas encore calculée)
coupe le groupe.

Calibrage (18/10/2026, 94 RAF d'un portrait en extérieur puis en intérieur) : distances
de 0,01 à 0,05 dans une rafale, 0,1 à 0,3 entre poses d'une même série, 0,4 à 0,55 entre
scènes ; au seuil 0,25 : 20 groupes, la séance d'intérieur (2 h 35 plus tard) séparée.
`tri <dossier> --groups [--threshold X]` montre les liens et les groupes, pour régler.
Calcul : ~50 ms par RAF (ouverture LibRaw et JPEG embarqué réduit), workers légers.

Restent (décidé : plus tard) : corrections à la main (fusionner, couper), groupes non
contigus (même sujet à deux moments).

## Centre calculé d'après le dossier, curseur de sévérité (21/10/2026)

Idée de l'utilisateur : même logique que le curseur du flou. Le curseur des piles va de
-1 à 1 ; **au centre**, le seuil calculé d'après le dossier ; **à droite plus strict**
(seuil plus bas, petites piles : seulement les rafales), **à gauche plus souple** (piles
plus grandes) ; double-clic : retour au centre.

- `suggested_threshold` : la coupure d'**Otsu** sur les distances entre voisines (celle
  qui sépare le mieux les petites distances, même série, des grandes, changement de
  scène). Les liens déjà coupés par l'écart de temps ne comptent pas. Repli sur
  `threshold` si `auto = false`, s'il y a moins de `auto_min_links` liens, ou si la
  séparation (part de la variance expliquée par la coupure) reste sous
  `auto_separation` ; borné entre `threshold / auto_range` et `threshold × auto_range`.
  Mesuré sur trois vraies séances (17/10/2026) : 0,21, 0,16 et 0,15 (séparation 0,66 à
  0,77), contre 0,25 fixe.
- `with_strictness(centre, s, strictness_span)` : `centre × span^(-s)` (×3 à ÷3 par
  défaut).
- `folder_strictness` / `remember_strictness` : la sévérité choisie, retenue pour chaque
  dossier (`group-strictness.tsv` du dossier de données) ; au centre, elle est oubliée.
  Les seuils par dossier d'avant (`group-thresholds.tsv`) ne sont plus lus.
- `tri <dossier> --groups` regroupe au centre calculé (`--threshold X` pour imposer un
  seuil).

## Couvertures choisies

`chosen_covers` / `choose_cover` : la préférée de chaque pile, choisie par l'utilisateur
(une par pile), séparée des notes ; retenue dans `stack-covers.txt` du dossier de données.

