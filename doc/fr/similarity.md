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
- `threshold_by_rank(distances triées, centre, s, courbe, span, min)` (25/10/2026, issue n° 61 :
  côté strict sensible au pixel, et des piles restaient tout à droite) : le curseur suit
  le **rang** des distances (`distances_by_rank`), pas leur valeur. Au centre, `centre` ;
  vers la droite, de moins en moins de liens joints, à parts égales, jusqu'à aucun ; vers
  la gauche, jusqu'à `centre × span` (jusqu'à tout regrouper depuis le 06/11/2026, voir
  plus bas). Simulé sur six vraies séances : tout à droite, 9 à
  40 piles restaient avec le seuil géométrique, aucune maintenant. Moins de `min` liens :
  `with_strictness`.
- `folder_strictness` / `remember_strictness` : la sévérité choisie, retenue pour chaque
  dossier (clé `group-strictness` du `tuning.json` du dossier, [store.md](store.md)) ; au
  centre, elle est oubliée.
  Les seuils par dossier d'avant (`group-thresholds.tsv`) ne sont plus lus.
- `tri <dossier> --groups` regroupe au centre calculé (`--threshold X` pour imposer un
  seuil).

## Pauses et progression du curseur (06/11/2026, issue n° 306)

Mesuré sur FULLTEST (570 RAF d'une séance d'autoportraits en studio, même cadrage) :
l'empreinte voit surtout le fond et le cadrage. Une main près du visage la change à
peine. « Main sur la tête » et « bras le long du corps » sont à 0,34 l'une de l'autre,
autant que deux images d'une même pose où l'on bouge dans le cadre (jusqu'à 0,41). Des
empreintes plus fines (grilles de gris 12 × 16 à 48 × 64, tolérantes aux décalages,
médianes de fenêtres de 2 à 4 images) ne séparaient pas mieux les poses. En revanche, les
changements de pose tombaient sur des **pauses** : 9 à 302 s, contre 1 à 3 s entre deux
images d'une même pose.

- `with_pauses(liens, poids)` : une pause plus longue que la **cadence** du dossier
  (écart médian, au moins 1 s) multiplie la distance par
  1 + poids × log2(pause / cadence). Multiplier plutôt qu'ajouter : deux images
  identiques restent ensemble, la pause départage les autres. Le **centre** du curseur
  reste calculé sur les distances sans pause : une pause ne fait que couper davantage.
  Mesuré à 0,1 : le « buste » de 234 photos se sépare à fond à gauche en 12 + 102 + 120
  (bras devant le visage, main sur la tête ou au menton, bras le long du corps) ; au
  centre, sur trois autres séances, de 0 à 4 piles de plus.
- Une pause est un **indice**, pas une règle (retour de l'utilisateur) : en studio, le
  contenu compte d'abord ; en photo de rue, une pause marque souvent un autre lieu.
  D'où le curseur **« Importance des pauses »** de la fenêtre `Ctrl+G`, retenu par
  dossier (`folder_pauses` / `remember_pauses`, `group-pauses.tsv`) :
  `pause_weight(poids, t, étendue)`, au centre `[similarity] pause_weight`, à fond à
  gauche 0 (pauses ignorées), à fond à droite × `pause_range`.
- **Progression du curseur** (retour de l'utilisateur : réglable, mais la gauche allait
  encore trop vite ; l'idée d'un paramètre sur toute l'échelle est la sienne) :
  `threshold_by_rank` va désormais jusqu'à **tout regrouper** à fond à gauche (plus de
  « centre × étendue », qui s'arrêtait à 0,375 sur FULLTEST, sous les 0,41 qui
  réuniraient les piles « bras croisés »). Entre le centre et chaque bout, le nombre de
  piles (liens coupés + 1) suit, avec λ = `strictness_curve` (2 par défaut) :
  - vers la **gauche**, une **échelle logarithmique** à la position `u^λ` : chaque pixel
    retire la même part des piles, donc de plus en plus de pixels pour une pile de moins
    à mesure qu'on va à gauche ; λ > 1 adoucit le départ du centre. Une première version
    (moyenne de puissance λ des deux côtés) avait une pente infinie au bout gauche : à
    λ = 2, de 39 piles à 2 dans les derniers 10 % (« au pixel près », retour de
    l'utilisateur) ;
  - vers la **droite**, une **moyenne de puissance λ** : λ = 1, à parts égales ; au-delà,
    plus rapide près du centre.

  Sur FULLTEST (123 piles au centre), à -0,2 / -0,6 / -0,9 / -0,95 / -1 : 104 / 28 / 4,4
  / 3 / 2 piles à λ = 2 (à λ = 1 : 54 / 10 / 3 / 2,5 / 2). Sur une demi-glissière de
  100 px, il faut environ 3 à 4 px par pile vers 3 à 5 piles. Curseur **« Progression du curseur »** de la fenêtre
  `Ctrl+G`, retenu par dossier (`folder_curve` / `remember_curve`, `group-curve.tsv`),
  de 1 / `strictness_curve_max` à `strictness_curve_max`. `strictness_span` ne sert
  plus qu'aux dossiers de trop peu d'images (`with_strictness`).
- `tri <dossier> --groups` montre aussi la distance avec la pause, et regroupe avec elle.

## Couvertures choisies

`chosen_covers` / `choose_cover` : la préférée de chaque pile, choisie par l'utilisateur
(une par pile), séparée des notes ; retenue par nom dans le `choices.json` du dossier de
chaque image ([store.md](store.md)).

