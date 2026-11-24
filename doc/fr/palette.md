# Palettes de couleurs (`tri::palette`)

Issue #492 (idée de l'utilisateur, 15/11/2026) : construire des **moodboards** à partir
de ses photos et d'images de référence. Extraire la palette de chaque image, chercher
les images aux couleurs proches ou qui contiennent une couleur, les rassembler.

Le module fait le calcul sur une image ; le cache par source et le calcul en fond sont
décrits plus bas, l'interface vient ensuite.

## Une palette

`Palette` : les couleurs principales d'une image (`Swatch`), de la plus présente à la
moins présente, chacune avec sa **part** de l'image (les parts font 1). Les couleurs
sont en **OKLab** (Björn Ottosson, 2020) : un espace perceptuel, où des écarts égaux se
voient à peu près égaux, ce qui rend les distances et la fusion des couleurs proches
parlantes. `Swatch::srgb` donne la couleur à écrire (`#rrggbb`, écrêtée au sRGB),
`Swatch::working` la couleur à afficher comme les images du cache, et
`Swatch::hue_chroma` sa place sur une roue chromatique (teinte, chroma).

Les images arrivent dans l'espace de travail de `tri` (primaires Rec.2020, courbe sRGB,
voir [colour.md](colour.md)) ; OKLab est calculé depuis la lumière linéaire.

## Extraction (`extract`)

1. Une grille d'au plus `side × side` points est lue dans l'image (96 × 96 par défaut) :
   la palette ne dépend pas des détails.
2. **k-moyennes** à `colors` centres (6), `iterations` passes (12). Départ **sans
   hasard** (même image, même palette, d'un lancement à l'autre) : le point le plus
   proche de la moyenne, puis chaque fois le point situé au milieu, en cumul, des
   distances au carré aux centres déjà pris (une version déterministe de
   k-moyennes++).
3. Les couleurs plus proches que `merge` (0,03, à peine distinguables côte à côte) sont
   fusionnées ; celles sous `min_share` (2 %) sont retirées, les parts recalculées.

Quelques millisecondes par image.

## Distance (`distance`)

Coût de transport des parts d'une palette vers l'autre (« Earth Mover's Distance »), en
unités OKLab : 0 pour deux palettes identiques, ~0,1 déjà bien différentes. Calculé en
servant d'abord les couples de couleurs les plus proches : une approche gloutonne,
exacte dans les cas simples, jamais plus petite que la vraie, et instantanée sur des
milliers de palettes (le calcul exact demanderait un solveur de transport pour un gain
qui ne se verrait pas dans un tri). Symétrique ; une palette vide est à distance
infinie. Teinte et saturation (a, b) y comptent `chroma_weight` fois (2) face à la clarté
(retour du 21/11/2026 : à poids égal, une photo brune au fond sombre semblait aussi
proche d'une photo violette au fond sombre que les autres violettes).

## Couleurs proches (`rank_similar`)

Les éléments sous le seuil, du plus proche au plus lointain ; ceux dont la palette n'est
pas encore calculée gardés à la fin, ceux sans palette possible retirés. Seuil de départ
et bout du curseur : `similar_threshold` (0,12), `similar_max` (0,5).

## Part d'une couleur (`Palette::share_near`)

La part de l'image dont la couleur est à moins d'un rayon OKLab d'une couleur donnée :
« contient assez de ce bleu ». Comptée comme les distances entre palettes (teinte et
saturation `chroma_weight` fois). « Contient cette couleur » utilise plutôt `Palette::nearest` : la distance de la couleur
de la palette la plus proche, miettes sous `colour_min_share` (0,02) ignorées ; tolérance
`colour_radius` (0,1), `colour_radius_max` (0,4).

**Plusieurs couleurs** (`Palette::colours_cost`, `ColourQuery`, #492) : `None` si l'une
des couleurs est plus loin que la tolérance ; sinon, recherche absolue, la moyenne des
distances (une couleur seule : sa distance, l'ordre d'avant) ; hiérarchique, chaque
couleur pèse `colour_hierarchy` (0,4) fois la précédente et son coût est 1 − sa part dans
l'image (`share_near`) plus `colour_proximity` (0,25) × sa distance rapportée à la
tolérance. Mesuré le 23/11/2026 sur 300 photos d'une séance (`--find`) : avec la
distance à part égale et 0,6, l'ordre changeait à peine quand on inversait les couleurs
(une image avec 7 % de la première couleur en 5ᵉ place) ; la présence d'abord et 0,4
mettent devant les images où la première couleur occupe le plus de place.

## Couverture fine (`Coverage`)

Retour du 21/11/2026 (vidéo) : la peau dans l'ombre, orangée et saturée, était fondue
dans une couleur moyenne de la palette, bien moins saturée ; pipetée sur la photo, la
couleur ne retrouvait pas la photo. Chaque palette garde donc aussi ses points
d'échantillonnage comptés par petites cases OKLab (L par pas de 0,04 ; a et b par pas
de 0,016, de -0,32 à 0,32). « Contient cette couleur » (`Palette::nearest`) mesure la
distance en deçà de laquelle se trouve au moins `colour_coverage` de l'image (0,5 % : un
reflet pipeté suffit)
(`Coverage::reach`), et `share_near` la vraie part des pixels proches
(`Coverage::share_within`). Mesuré sur une séance (131 images) : ~1,2 Ko par palette
(0,4 à 3 Ko). Une palette d'une version précédente (sans couverture) est recalculée.

## Cache

`Palette::to_bytes` / `from_bytes` : version du calcul (`VERSION`, 2), nombre de couleurs,
puis L, a, b et part de chacune, puis la couverture (nombre de points, de cases, puis
case et points en u16). Une autre version est refusée (recalcul).

## Sources (décision de l'utilisateur, 21/11/2026)

Trois sources, pour trois usages, au choix dans l'interface des palettes :

1. **JPEG boîtier** (intégré au RAW, ou le JPEG lui-même) : quelqu'un qui photographie
   en JPEG sans y toucher ;
2. **données brutes de LibRaw** : un regard « scientifique » sur les données, hors de
   tout traitement ;
3. **rendus finis** (développements) : la recherche pour un moodboard.

Une palette par image **et par source** dans le cache (`Level::Palette(Source)`,
`palette::Source` : `Camera`, `Raw`, `Developed(logiciel)`), tirée de l'image d'où elle
vient (`preview::palette_input`) : la **vignette** (JPEG boîtier d'un RAW, image plate
réduite), le **rendu RAW** de `tri`, ou le **rendu du logiciel**. Sa clé comprend celle
de cette image : refaite (réglages de rendu changés, développement modifié dans
darktable), la palette l'est aussi. `SourceKind` est le choix de l'utilisateur
(`camera`, `raw`, `developed`) ; `SourceKind::source_for` le traduit pour chaque image :
une image plate n'a pas de données brutes (son image, la source boîtier), un rendu fini
n'existe que pour un développement de la liste.

`preview::compute_palette` calcule (l'image source d'abord, si elle manque),
`preview::palette` relit le cache. Calcul en fond : voir [scheduler.md](scheduler.md).

## Interface

Section « Couleurs » du panneau de droite (source, bande, roue et couleurs de l'image
sélectionnée), recherches dans la section Filtre (couleurs proches, couleur) ; voir
[gui.md](gui.md). Sources (`SourceKind`) : `camera`, `raw`, ou un logiciel (`darktable`…)
pour ses développements ; `developed` d'avant se relit `darktable`.

## Empiler par palette

Idée de l'utilisateur (21/11/2026), après un essai sur une vraie séance : mêler la palette
à l'empreinte des rafales n'ajoutait aucune coupure (entre voisines d'une même séance,
0,01 à 0,03). En **critère à part** (« Empiler : par palette (séries) », sous « Empiler
automatiquement », retenu par dossier : `similarity::folder_by_palette`), elle sépare les
**séries** (un éclairage, un fond) plutôt que les rafales. `stack_links` : la distance des
palettes voisines divisée par `[colours] stack_scale` (0,5, bornée à 1) prend l'échelle des
empreintes, sur laquelle le curseur des piles et son centre calculé sont calés ; **sans
l'heure** (une série est une lumière, pas un moment : à fond à gauche, tout se rejoint) ;
une palette inconnue coupe. Sans source de palette choisie,
le JPEG boîtier.

## Ligne de commande

`tri <dossier> --colours [--source camera|raw|darktable|…]` : chaque image, ses couleurs
`#rrggbb` et leur part, rangées dans le cache comme dans l'interface. Les rendus finis :
seulement ceux déjà faits par l'interface (aucun logiciel lancé en ligne de commande).
Avec `--find "#d0a080,#6a7a80"` (la plus importante d'abord) et `--hierarchical` (ou
`--mean`, la couleur moyenne, `Palette::mean`, comparée à la première) :
ensuite, les images qui contiennent toutes ces couleurs, la mieux placée d'abord, avec
leur coût et la part de chaque couleur (tolérance : `--threshold`, sinon
`colour_radius`). Avec `--posterize K --to DOSSIER` : chaque image en K couleurs
(`palette::posterize` ; 1 : sa couleur moyenne), écrite en sRGB (`NOM-pK.jpg`).

**Postérisation** (`palette::posterize`, #492, 23/11/2026) : les k couleurs principales de
l'image (k-moyennes de `palette::extract`, factorisées dans `cluster`, sans fusion des
proches ni retrait des miettes), puis chaque pixel remplacé par la plus proche (OKLab ;
un pixel déjà vu n'est pas reconverti) ; k = 1, la couleur moyenne. Renvoie l'image et ses
couleurs, parts des pixels décroissantes. Environ 10 ms pour une vignette.
En **couleurs perçues** (`posterize_with(…, true)`, expérience) : l'image en OKLab, a et b
floutés sur un carré de `perceived_surround` (0,2) du grand côté (deux passes de moyenne
glissante, `box_blur`) donnent la teinte de l'entourage ; chaque pixel en perd la part
`perceived_adaptation` (0,8 ; adaptation de l'œil à la lumière dominante), puis a et b
sont multipliés par une saturation **dégressive** : `perceived_saturation` (1,4) à
`perceived_saturation_colours` (4) couleurs, le surplus variant comme (4 / k) ^
`perceived_saturation_falloff` (0,5) — 1,8 pour 1 couleur, 1,28 pour 8, l'image entière
(cran 0) n'étant pas touchée (retours du 23/11/2026 : retirer la lumière dominante ôtait de la couleur
à tout le reste, « infiniment plus naturel » mais terne ; puis « plus il y a de couleurs,
moins il faut corriger ») ; dans les
k-moyennes (pondérées, `cluster_weighted`), un point pèse `1 + perceived_salience × écart
à son entourage / 0,1`. Sur la photo de l'utilisateur sous lumière violette, le visage
sort orange dès 2 couleurs (beige rosé en valeurs brutes).

**Palette perçue en cache** (24/11/2026) : à côté de la palette mesurée, le cache range sa
version en couleurs perçues (`Palette::perceived`, `extract_perceived` : même calcul au
cran `colors`, puis fusions et miettes comme `extract`, avec sa couverture). L'image est
d'abord réduite à `perceived_side` (512 px) : le calcul porte sur chaque pixel, et
l'entourage se mesure en part de l'image. Les deux palettes forment une seule entrée,
dans le cache comme dans l'index de recherche (`VERSION` 3 : les palettes d'avant sont
recalculées une fois) ; la clé compte aussi les réglages `perceived_*`.
`Palette::view(perceived)` donne l'une ou l'autre. Dans l'interface, l'œil de la section
Couleurs vaut pour tout : la palette du panneau, les couleurs en grand, « Couleur »,
« Couleurs proches », les jumelles et l'empilement par palette (refaits quand il bascule).
En ligne de commande, `--perceived` vaut pour les palettes affichées et `--find`.

## Réglages `[colours]`

`colors`, `side`, `iterations`, `merge`, `min_share` (voir `doc/config.example.toml`).

## Tests

Bandes de couleurs synthétiques : deux couleurs et leurs parts, même image même palette,
fusion des proches et retrait des miettes, distances ordonnées et symétriques, part
d'une couleur, aller-retour binaire, teinte du rouge et chroma du gris, aller-retour
sRGB ↔ OKLab.
