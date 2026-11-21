# Palettes de couleurs (`tri::palette`)

Issue #492 (idée de l'utilisateur, 15/11/2026) : construire des **moodboards** à partir
de ses photos et d'images de référence. Extraire la palette de chaque image, chercher
les images aux palettes proches ou qui contiennent une couleur, les rassembler.

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
infinie.

## Palettes proches (`rank_similar`)

Les éléments sous le seuil, du plus proche au plus lointain ; ceux dont la palette n'est
pas encore calculée gardés à la fin, ceux sans palette possible retirés. Seuil de départ
et bout du curseur : `similar_threshold` (0,08), `similar_max` (0,3).

## Part d'une couleur (`Palette::share_near`)

La part de l'image dont la couleur est à moins d'un rayon OKLab d'une couleur donnée :
« contient assez de ce bleu » (la pipette existe déjà dans l'interface).

## Cache

`Palette::to_bytes` / `from_bytes` : version du calcul (`VERSION`), nombre de couleurs,
puis L, a, b et part de chacune. Une autre version est refusée (recalcul).

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

Section « Palette » du panneau de droite : choix de la source, bande, roue et couleurs
de l'image sélectionnée, avancement dans la barre d'état ; voir [gui.md](gui.md).

## Ligne de commande

`tri <dossier> --palette [--source camera|raw|developed]` : chaque image, ses couleurs
`#rrggbb` et leur part, rangées dans le cache comme dans l'interface. Les rendus finis :
seulement ceux déjà faits par l'interface (aucun logiciel lancé en ligne de commande).

## Réglages `[palette]`

`colors`, `side`, `iterations`, `merge`, `min_share` (voir `doc/config.example.toml`).

## Tests

Bandes de couleurs synthétiques : deux couleurs et leurs parts, même image même palette,
fusion des proches et retrait des miettes, distances ordonnées et symétriques, part
d'une couleur, aller-retour binaire, teinte du rouge et chroma du gris, aller-retour
sRGB ↔ OKLab.
