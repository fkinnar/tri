# Module `tri::sharpness` — carte de netteté (focus peaking)

Pour vérifier la mise au point à 1:1 (demandé le 17/10/2026) : les contours nets de
l'image sont surlignés d'une couleur vive, comme le *focus peaking* des boîtiers.

## Décisions (17/10/2026)

- **En 1:1 seulement**, en plein écran : en grille, ce serait gourmand et illisible à
  cette taille.
- **2-3 couleurs configurables, rotation par un raccourci** : selon l'image, une couleur
  ressort mieux qu'une autre (une couleur proche du sujet s'y perd).

## Calcul

`peaking(&Bitmap, &SharpnessSettings) -> Peaking` : un octet par pixel (255 contour, 0
rien), aux dimensions de l'image.

1. Luminance approchée `(R + 2G + B) / 4` (le vert porte l'essentiel du détail).
2. Gradient de **Sobel**, `(|gx| + |gy|) / 4` (0 à 510 ; un bord franc noir / blanc
   donne 255), comparé à un **seuil fixe** (`threshold`, 48 ; 32 jusqu'au 17/10/2026,
   jugé trop permissif : une photo molle restait marquée à 4 %, contre 1 % à 48 ; une
   nette passe de 16 % à 8 %, toujours franchement marquée) : pas relatif à l'image,
   pour que deux photos d'une rafale se comparent — la plus nette montre plus de contours.
3. **Points isolés retirés** (`min_neighbours`, 2) : un contour est une ligne, le bruit
   à ISO élevé fait des points épars (mesuré sur un RAF X-T4 à 2000 ISO : la peau se
   couvrait de points).
4. **Épaississement** (`spread`, 1 pixel de chaque côté) : un contour d'un pixel est à
   peine visible à 1:1 sur un écran haute densité. `marked` compte avant épaississement.

La carte est calculée sur l'**image affichée** (JPEG boîtier à sa taille, ou rendu RAW
complet), donc superposable pixel pour pixel, orientation comprise. Le JPEG boîtier est
accentué par le boîtier, ce qui gonfle un peu la carte ; le rendu RAW ne l'est pas. Piste
pour plus tard (score par image, filtre « nettes seulement », cache d'analyse) : calculer
sur la mosaïque RAW (canal vert), sans accentuation ni dématriçage.

Mesures (Mac Apple Silicon, `--release`, JPEG boîtier X-T4 2944 × 4416) : 35 ms pour le
gradient seul, 111 ms avec le retrait des points isolés et l'épaississement. Pas de
parallélisation : inutile à ce coût, et la carte n'est calculée qu'à la demande.

Outil de réglage : `cargo run --release --example decode -- peaking <image> <sortie.png>
[--threshold N]` (contours en rouge, temps, part de pixels marqués). Seuils comparés sur
le visage : 48 ne marque que les bords francs ; 32 marque aussi les lignes de la main et
les lèvres.

## Score de netteté : repérer les photos floues (17/10/2026)

But : écarter les photos **objectivement** floues (mise au point ratée, bougé), sans IA
pour commencer, et poser le circuit (calcul en fond, cache d'analyse, filtre) qu'une
méthode plus fine (yeux nets sur un portrait…) reprendra plus tard. À affiner avec des
photos ratées exprès (l'utilisateur en prépare).

`score(&Bitmap, &SharpnessSettings) -> Score` :

1. **Taille fixe** : l'image est réduite au décodage à `score_side` (1536 px de côté long),
   pour que les boîtiers se comparent et que le bruit pèse moins. RAW : le JPEG boîtier
   (`preview::compute_sharpness`, workers légers, ~120 ms) ; image plate : elle-même.
2. **Luminance étirée** entre ses centiles 1 % et 99 % (`score_stretch`) : sans cela, une
   photo sous-exposée mais nette passait pour floue (mesuré sur un portrait sombre : 0,21
   × la médiane avant, 0,42 après). Écart de luminance sous `score_min_range` : pas de
   détail (image toute blanche), score nul.
3. **Par cases** (`score_tiles` = 16 sur le côté long) : gradient moyen (Sobel, racine de
   l'énergie) de chaque case.
4. **Les cases les plus nettes** font le score (`score_top_share` = 5 %) : la question est
   « y a-t-il quelque chose de net quelque part ? ». Un portrait au fond flou volontaire
   garde un bon score ; un bougé ou une mise au point ratée fait tout chuter.

**Jugement relatif** (`judge`) : le score ne veut rien dire dans l'absolu (ISO, objectif,
accentuation du boîtier). Une photo est **floue** si elle est sous `blur_ratio` (0,5) × la
**médiane du dossier**, **ou** sous `neighbour_ratio` (0,8) × la médiane de ses
**voisines** (`neighbours` = 3 photos avant et 3 après, dans l'ordre du dossier : même
scène, même rafale le plus souvent). Les deux règles se complètent : la première attrape
les longues séries floues (leurs voisines sont floues aussi), la seconde une ratée isolée
dans une série nette et très texturée, qui reste au niveau du dossier.

**Calibrage du 17/10/2026** sur 94 RAF X-T4 ratés exprès et étiquetés par l'utilisateur
(vert : nette, rouge : floue, jaune : limite ; dossier `2026-10-17_MISS`) :

| | Dossier seul (< 0,5) | Dossier + voisines (< 0,8) |
|---|---|---|
| Rouges attrapées | 12 / 15 | **14 / 15** |
| Vertes accusées à tort | 0 / 56 | **0 / 56** (la plus basse à 0,95 × ses voisines) |
| Jaunes signalées | 1 / 15 | 1 / 15 |

Enseignements :
- **Mise au point sur l'arrière-plan** (DSCF0361 : sujet flou, poteau net) :
  indétectable sans savoir où est le sujet. Le collimateur (`FocusPixel`, enregistré par
  Fuji dans le repère du JPEG embarqué) ne suffit pas : il était sur le poteau, net — le
  boîtier a fait ce qu'on lui demandait. Il faudra une détection du sujet (personne,
  visage) : l'étape « IA ».
- **Netteté au collimateur** (prototype) : pas meilleure que le score global, car elle
  dépend du contenu de la zone (une joue lisse paraît floue) ; comparer des zones de
  scènes différentes n'a pas de sens.
- Les **jaunes** (limites) restent pour la plupart au niveau des vertes : écarts trop
  fins pour un score sans IA à cette taille d'analyse.
- Une photo sans étiquette signalée (DSCF0347) était bien floue à 100 %.

**Circuit** : niveau de cache d'analyse `Level::Sharpness` (entrée binaire, `Score::to_bytes`,
~1 Ko, version `SCORE_VERSION` dans la clé), calculé en fond par le scheduler pour tout le
dossier (`[sharpness] background`, workers légers, après vignettes et aperçus boîtier),
lu par `preview::sharpness`. Filtre `BlurFilter` (trait `Filter`, comme les autres) : une
photo sans score encore calculé est gardée.

Interface : mention « floue » sur la vignette (`[gui.grid] blur_badge`), ligne
« Netteté » dans le panneau des métadonnées (rapports au dossier et aux voisines), filtres
« sans les floues » (`f b`) et « floues seulement » (`f B`, pour vérifier les photos
signalées avant de les rejeter : `BlurRule`), barre de progression « netteté ».

**Collimateur** (17/10/2026) : le repère du collimateur de mise au point (lu dans le RAW,
`Metadata::focus_point`, voir `metadata.md`) est dessiné en plein écran et en comparaison
(`m`, `[gui.view] focus_point`, forme `focus_marker` : carré, croix, cercle ou cœur).
Il montre d'un coup d'œil une mise au point sur l'arrière-plan, que le score ne voit pas. Le filtre
s'applique une fois tous les scores connus (la médiane bouge tant qu'ils arrivent), et
seulement si le jugement a changé.

Ligne de commande : `tri <dossier> --sharpness` (score, rapports au dossier et aux
voisines, jugement, durée) : l'outil de réglage des seuils.

Mesuré le 17/10/2026 sur le dossier de test (26 RAF X-T4) : 24 photos entre 0,80 et 1,08 ×
la médiane ; signalées : une photo blanche (score nul) et un portrait sous-exposé (0,42),
effectivement mou à 100 % (cils et contours des yeux pas francs) — à confirmer sur des
photos ratées exprès.

Limites connues : le jugement relatif suppose des sujets comparables dans le dossier (un
dossier de paysages et de portraits mêlés est moins fiable) ; le JPEG boîtier est
accentué ; le bruit à ISO très élevé ressemble à du détail.

## Réglages

| Section | Clé | Défaut | Rôle |
|---|---|---|---|
| `[sharpness]` | `threshold` | 48 | seuil du gradient |
| | `spread` | 1 | épaississement (pixels) |
| | `min_neighbours` | 2 | voisins marqués requis (0 : tout garder) |
| | `score_side` | 1536 | score : côté long de l'image analysée |
| | `score_tiles` | 16 | score : cases sur le côté long |
| | `score_top_share` | 0.05 | score : part des cases les plus nettes |
| | `score_stretch` | 0.01 | score : centiles d'étirement de la luminance |
| | `score_min_range` | 8 | score : écart minimal de luminance (sinon score nul) |
| | `blur_ratio` | 0.5 | floue sous cette fraction de la médiane du dossier |
| | `neighbours` | 3 | voisines comparées de chaque côté (0 : aucune) |
| | `neighbour_ratio` | 0.8 | floue sous cette fraction de la médiane des voisines |
| | `background` | `true` | scores de tout le dossier en fond |
| `[gui.sharpness]` | `visible` | `false` | carte affichée au démarrage |
| | `colors` | rouge, vert, bleu | couleurs de la rotation |
| | `opacity` | 1.0 | opacité des contours |

## Dans l'interface

`p` affiche ou masque la carte (`core.view.peaking`) ; `Ctrl+P` passe à la couleur
suivante (`core.view.peaking_color`) et la montre si elle était masquée. Hors 1:1, la
ligne d'information indique « netteté : en 1:1 (z) ». La carte est calculée par le fil de
chargement (`gui/src/loader.rs`, après les images voulues), envoyée comme une texture
blanche dont l'opacité marque les contours, et **teintée au dessin** : changer de couleur
ne recalcule rien. Une seule carte est gardée (celle de l'image affichée) ; quitter le 1:1
retire la demande en cours.

### Curseur de sévérité (20/10/2026)

Demandé par l'utilisateur après un portrait à très grande ouverture (yeux nets, tout le
reste flou, jugé « floue » alors qu'elle est utilisable) : un curseur dans la barre des
filtres, à côté des gouttes. **Au centre** les seuils des réglages, **à droite plus
strict** (plus de photos floues), **à gauche plus souple**. Il multiplie les trois seuils
du jugement (`blur_ratio`, `neighbour_ratio`, `group_ratio`) par `strictness_span ^
sévérité` (×0,5 à ×2 par défaut, sans dépasser 1) : `SharpnessSettings::with_strictness`.
Effet en direct (badges, filtre « floues »), instantané : les scores sont en cache, seul
le jugement est refait. Retenu par dossier au lâcher (`sharpness::folder_strictness`,
fichier `blur-strictness.tsv` du dossier de données) ; double-clic : retour au centre
(repère). Le panneau des métadonnées donne le score rapporté au dossier et aux voisines
(ou à la pile) : de quoi comprendre un jugement. Même principe à venir pour le curseur
des piles (centre calculé d'après le dossier). Le vrai remède à ce cas limite reste une
détection du sujet (yeux), plus tard.

## Tests

Score (`src/sharpness.rs`) : un flou divise le score par plus de deux, un sujet net sur un
fond uni garde le score d'une image nette partout ; une image quatre fois plus sombre a le
même score, une image blanche un score nul ; aller-retour binaire, données tronquées
refusées ; médiane, jugement relatif, filtre (les images sans score restent). Scheduler :
travail léger de fond, après l'aperçu boîtier. Interface : sur des damiers nets et un
damier flou, la floue est jugée telle, `f b` la masque puis la remontre.

Carte (`src/sharpness.rs`) : un bord franc est marqué des deux côtés, une zone
uniforme et les bords de l'image jamais ; le seuil est absolu ; points isolés retirés,
lignes gardées ; épaississement ; petites images sans panique. Interface : `p` en ajusté
ne demande rien, en 1:1 la carte arrive à la taille de l'image (pixel pour pixel), quitter
le 1:1 retire la demande, `Ctrl+P` tourne sur les couleurs et remontre la carte.

## Jugement dans les piles (18/10/2026)

Les faux positifs d'un dossier aux sujets variés (photos libres du manuel : gros plans sur
fond flou parmi des paysages) venaient surtout de la **règle des voisines** : les
« voisines » y sont d'autres scènes. Avec les piles d'images semblables (`similarity`),
`judge_in_groups` ne compare plus une photo à ses voisines, mais aux autres images de sa
pile (au moins `group_min` = 3, floue sous `group_ratio` = 0,8 × leur médiane) ; la
**règle du dossier reste** (elle attrape une rafale floue de bout en bout, que la pile ne
voit pas : toutes ses images se ressemblent). Piles du jugement plus larges que celles de
l'affichage (`group_threshold` = 0,45, contre 0,25) : la « même scène » d'une comparaison
de netteté dépasse la rafale.

Mesures, avec les étiquettes de l'utilisateur (dossier MISS, 94 RAF : 15 rouges, 56
vertes) et les 19 photos libres :

| Règles | MISS : rouges trouvées | vertes marquées | photos libres marquées |
|---|---|---|---|
| dossier + voisines (avant) | 14 / 15 | 0 | 3 |
| pile seule | 9 / 15 | 0 | — |
| dossier + pile, seuil 0,25 | 12 / 15 | 0 | 1 |
| **dossier + pile, seuil 0,45** | **14 / 15** | **0** | **1** |

Sans empreintes (`[similarity] background = false`), le jugement d'avant (dossier +
voisines). La photo restante des photos libres est sous 0,5 × la médiane du dossier :
seule une analyse du contenu (sujet, flou d'arrière-plan voulu) la distinguerait.

