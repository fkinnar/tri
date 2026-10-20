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

Une image plate est analysée **à sa profondeur d'enregistrement**
(`flat::decode_deep`, depuis le 17/10/2026) : 16 bits pour un TIFF ou un PNG 16 bits
(`Exposure::from_rgb16`, `Source::Flat { bits: 16 }`), 8 bits pour le reste (JPEG, WebP,
HEIF : un HEIC 10 bits est encore ramené à 8 bits par libheif). Les valeurs sont
**ramenées en lumière linéaire** (décodage sRGB) avant tout calcul en IL et des seuils saturé / bouché : 128 sur
255 n'est pas la moitié de la lumière mais ~22 % (-2,2 IL). Avant le 17/10/2026, un gris
moyen apparaissait à -1 IL. L'histogramme linéaire, lui, reste sur les valeurs du fichier
(0 à 255, ou 0 à 65 535), comme les histogrammes habituels. Un test le montre : un
presque-noir à -9,96 IL en 16 bits n'est pas bouché, mais l'est une fois ramené à 3 sur
255 (-10,1 IL).

**Plage utile** (`Exposure::stops`) : log₂ de la plage d'un RAW (13,9 IL en 14 bits, 11,8
en 12 bits), ~11,7 IL pour une image 8 bits sRGB (plus petite valeur non nulle, ramenée
en lumière linéaire), ~19,7 IL pour 16 bits (affichage limité à `ev_range`). L'interface montre cette plage par défaut (retour du 17/10/2026 : un
JPEG s'affichait sur 12 IL comme un RAW).

## Ce qui est calculé (`Exposure`)

Un seul passage sur les pixels construit l'histogramme **complet** (une case par valeur
et par canal) et le masque ; tout le reste en est dérivé :

- `linear` : histogramme linéaire de 0 (noir) à `range` (blanc), `[exposure] linear_bins`
  cases ;
- `ev` : histogramme **en IL sous la saturation** (échelle par défaut de l'interface),
  de `-ev_range` à 0, `ev_bins_per_stop` cases par IL ; le noir et tout ce qui est plus
  sombre vont dans la première case ;
- `total`, `over`, `under` par canal : pixels, saturés (≥ blanc × 2^`over_ev`), bouchés
  (≤ blanc × 2^`under_ev`). `over_ev` vaut **-0,02 IL** par défaut, pas 0 : sur un RAF
  X-T4, un rouge ou un bleu saturé est enregistré de 16383 au centre à 16323 vers les
  bords, en paliers (correction de vignetage du boîtier sur les données brutes). Au seuil
  exact, seul un rectangle central comptait comme saturé dans les trois canaux : défaut
  révélé par l'affichage des zones par canal (17/10/2026). Aucune image normale de la
  séance n'est touchée par cette marge (ni même par -0,05 IL) ; `over_fraction` / `under_fraction` : canal le plus touché ;
- `high` : valeur exacte au 99,9e centile par canal ; `headroom_ev()` : marge avant
  saturation du canal le plus exposé, en IL (« exposer à droite » ; 0 = au blanc). Calculée
  sur l'histogramme complet : une première version sur l'histogramme réduit donnait
  -0,004 IL pour une image blanche ;
- `mask` : `mask_size` cases sur le côté long, bits `OVER_R/G/B` (au moins un pixel saturé
  dans la case : un reflet brûlé doit se voir) et `UNDER` (au moins `mask_under_share`, la
  moitié, des pixels de la case sous le seuil des ombres) ; **déjà pivoté** selon
  l'orientation du boîtier, pour être superposé tel quel à l'image affichée. Première
  version : `UNDER` seulement si *tous* les pixels étaient bouchés ; le bruit du capteur
  près du noir en remontant presque toujours un, le pull noir n'avait aucune zone (vu en
  capture le 17/10/2026). `VERSION` (2) entre dans la clé du cache : le calcul a changé
  sans que les réglages changent.

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

## Statistiques par canal (`Exposure::channel_stats`, 18/10/2026)

Tirées de l'histogramme en IL (1/8 d'IL par case par défaut), sans relire l'image :
médiane et centiles (interpolés dans la case où le cumul franchit le seuil), moyenne et
écart-type (centres des cases), parts saturée et bouchée du canal. En IL sous la
saturation : la moyenne est celle des tons, pas de la lumière. Montrées dans le grand
histogramme de l'interface. Pas de luminance pour un RAW (chaque photosite ne mesure
qu'une couleur) ; pour une image plate, une vraie luminance demanderait un histogramme de
plus dans l'analyse (non fait).

## Bouché par canal (18/10/2026)

Le masque marque aussi, pour chaque canal, les cases où au moins `mask_under_share` des
valeurs de ce canal sont bouchées (`mask::UNDER_R`, `UNDER_G`, `UNDER_B`, fonctions
`mask::over(c)` / `mask::under(c)`), en plus du bouché tous canaux confondus
(`mask::UNDER`, inchangé : zones du plein écran). Sert à l'image d'analyse du grand
histogramme quand un seul canal est choisi. Format du cache inchangé (un octet par case),
mais `VERSION` passe à 5 : les analyses déjà en cache sont recalculées.

