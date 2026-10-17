# Module `tri::exposure`

Exposition d'une image : histogrammes sur les **vraies données**, pixels saturés et
bouchés, masque des zones concernées. Donnée *dérivée* (cache d'analyse, voir
`CLAUDE.md`), jamais écrite dans le XMP.

## Pourquoi les données du capteur

Le JPEG du boîtier (affiché par défaut) reflète les réglages du boîtier : courbe, balance
des blancs, compression des hautes lumières. Il peut paraître brûlé alors que le RAW ne
l'est pas, et inversement. L'histogramme d'un RAW est donc **toujours** calculé sur la
mosaïque du capteur (décision de l'utilisateur, 17/10/2026), comme FastRawViewer :

- après `libraw_unpack` seulement : pas de dématriçage, pas de balance des blancs, pas de
  courbe ([`raw::Sensor`](raw.md)) ;
- à la profondeur réelle (12, 14 ou 16 bits, déduite du blanc du capteur) ;
- **noir soustrait pixel par pixel** : chez Fuji, le noir est un motif 6 × 6
  (`cblack[6..]` de LibRaw, ~1022 en 14 bits), pas une valeur unique — lu `black = 0`
  au premier essai, d'où des ombres faussées ;
- canaux du filtre : rouge, vert (les deux verts Bayer ensemble), bleu, motif Bayer ou
  X-Trans lu par `libraw_COLOR`.

Une image plate est analysée sur ses valeurs 8 bits (les TIFF / PNG 16 bits sont
aujourd'hui ramenés en 8 bits par `flat` : à faire évoluer).

## Ce qui est calculé (`Exposure`)

Un seul passage sur les pixels construit l'histogramme **complet** (une case par valeur
et par canal) et le masque ; tout le reste en est dérivé :

- `linear` : histogramme linéaire de 0 (noir) à `range` (blanc), `[exposure] linear_bins`
  cases ;
- `ev` : histogramme **en IL sous la saturation** (échelle par défaut de l'interface),
  de `-ev_range` à 0, `ev_bins_per_stop` cases par IL ; le noir et tout ce qui est plus
  sombre vont dans la première case ;
- `total`, `over`, `under` par canal : pixels, saturés (≥ blanc × 2^`over_ev`), bouchés
  (≤ blanc × 2^`under_ev`) ; `over_fraction` / `under_fraction` : canal le plus touché ;
- `high` : valeur exacte au 99,9e centile par canal ; `headroom_ev()` : marge avant
  saturation du canal le plus exposé, en IL (« exposer à droite » ; 0 = au blanc). Calculée
  sur l'histogramme complet : une première version sur l'histogramme réduit donnait
  -0,004 IL pour une image blanche ;
- `mask` : `mask_size` cases sur le côté long, bits `OVER_R/G/B` (au moins un pixel saturé
  dans la case) et `UNDER` (tous les pixels de la case sous le seuil des ombres) ; **déjà
  pivoté** selon l'orientation du boîtier, pour être superposé tel quel à l'image affichée.

## Coût (RAF X-T4, 26 Mpx, Mac Apple Silicon, 17/10/2026)

- `unpack` 0,35–0,55 s, puis ~40 ms pour tout le reste : **0,38 s en moyenne** par RAW sur
  129 fichiers d'une vraie séance (`tri <dossier> --exposure --raw`) ; 0,15 s pour une
  image entièrement brûlée (compression) ;
- `unpack` est déjà parallélisé par LibRaw (OpenMP) : 2 fils = 1,7 s par fichier, 8 fils =
  9 s. Les analyses RAW passent donc par l'unique worker LibRaw, comme les rendus.

Vérifié sur la séance : DSCF0129 (photo entièrement blanche) → 99,8 % saturés ;
DSCF0214–0220 (pull noir, -3,5 IL de marge) → 4 à 6 % bouchés ; les portraits sur fond
blanc → -0,45 IL de marge, rien de saturé (exposition bien poussée à droite).

## Cache

`to_bytes` / `from_bytes` : format binaire petit-boutiste, entête versionné
(`TRIEXP01`), masque compressé par plages (presque toujours vide) : quelques Ko par image.

## Ligne de commande

```
tri <dossier> --exposure [--raw|--flat] [--depth N]
```

Une ligne par image : source (profondeur, noir, blanc), marge avant saturation, saturés,
bouchés, durée.
