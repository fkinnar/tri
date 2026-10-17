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
   donne 255), comparé à un **seuil fixe** (`threshold`, 32) : pas relatif à l'image,
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

## Réglages

| Section | Clé | Défaut | Rôle |
|---|---|---|---|
| `[sharpness]` | `threshold` | 32 | seuil du gradient |
| | `spread` | 1 | épaississement (pixels) |
| | `min_neighbours` | 2 | voisins marqués requis (0 : tout garder) |
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

## Tests

Unitaires (`src/sharpness.rs`) : un bord franc est marqué des deux côtés, une zone
uniforme et les bords de l'image jamais ; le seuil est absolu ; points isolés retirés,
lignes gardées ; épaississement ; petites images sans panique. Interface : `p` en ajusté
ne demande rien, en 1:1 la carte arrive à la taille de l'image (pixel pour pixel), quitter
le 1:1 retire la demande, `Ctrl+P` tourne sur les couleurs et remontre la carte.
