# Réglages de visualisation (`tri::adjust`)

Issue n° 137, décisions du 10/11/2026. Quelques réglages de base **pour comparer et trier
une série**, pas pour développer : exposition, balance des blancs, noir et blanc par
mélange des canaux. La clé : **appliquer les réglages d'une image aux autres**, en
harmonisant ce qui doit l'être (une photo à -1 IL et une bien exposée finissent pareilles).

Première étape (ce module) : le moteur et la ligne de commande. Ensuite : l'interface
(curseurs, pipette, avant / après, « Appliquer à la sélection », vignettes), le rapport
des réglages par image, puis l'écriture pour d'autres logiciels (RawTherapee / ART
d'abord). Les réglages s'appliquent **avant** tout développement par un autre logiciel :
une image développée dans darktable n'est pas concernée.

## Couches

Les réglages d'une image (`Layers`) sont des couches, qu'on peut refaire séparément :

| Couche | Rôle | Part vers les autres images (mode par défaut) |
|---|---|---|
| `correction` (exposition, balance) | ramener l'image à une base commune | **harmonisée** : calculée pour chaque image |
| `creative` (exposition, balance) | l'intention, la même pour la série | **copiée** (le même décalage) |
| `mono` (mélangeur) | noir et blanc | **copié** |

Trois modes (`Mode`) peuvent être forcés couche par couche (`Modes`) :

- **absolu** : la valeur telle quelle ;
- **décalage** : le même écart ajouté à ce que l'image a déjà ;
- **harmonisation** : l'écart calculé pour que l'image ressemble à la référence ;
- et **garder** : l'image garde la sienne.

## Valeurs

- **Exposition** en IL.
- **Balance des blancs** en gains rouge et bleu (`Gains`), relatifs à la balance de base de
  l'image, le vert servant de référence. Pour les curseurs et le rapport : **chaleur**
  (log₂ rouge / bleu, positif = plus chaud) et **teinte** (positif = magenta). Pas de
  kelvins : chaque logiciel les calcule avec son propre modèle du boîtier ; un écart par
  rapport à la prise de vue se reporte mieux.
- **Mélangeur** : part du rouge, du vert, du bleu (1,0 = 100 %), somme libre. Préréglages
  façon filtres colorés : `neutral` (luminance Rec. 709), `red`, `orange`, `yellow`,
  `green`, `blue`.

## Où ils s'appliquent

| | RAW rendu par LibRaw | JPEG boîtier, images plates |
|---|---|---|
| Exposition | sur les données du capteur, avant dématriçage (`RenderOptions::exposure`, `exp_shift` de LibRaw ; bornes -2 à +3 IL, le reste sur l'image) : hautes lumières récupérables | sur l'image, en linéaire : le brûlé ne revient pas |
| Balance | sur l'image rendue, en linéaire | idem |
| Mélangeur | exact | exact |

La balance d'un RAW est faite **sur l'image rendue, pas sur les multiplicateurs du
capteur** (écart assumé par rapport à la décision de départ) : l'harmonisation mesure la
dominante sur l'image, dans l'espace de sortie ; la passer au capteur demanderait une
conversion par la matrice couleur du boîtier, exacte pour une seule couleur. À revoir si
l'écart se voit à l'usage.

Chaque image 8 bits est ramenée en lumière linéaire avec **sa** courbe (`Transfer`) :
BT.709 pour un rendu LibRaw (la courbe des aperçus, `OutputSpace::PREVIEW`), sRGB pour un
JPEG. Décoder un rendu LibRaw en sRGB faussait la mesure dans les ombres (essai du
10/11/2026 sur une photo sombre : +1,18 IL au lieu de +1).

## Mesures et harmonisation

`measure` donne, sur l'image telle qu'elle est montrée (réglages compris) :

- la **luminance médiane** (Rec. 709, linéaire) : peu sensible à un ciel brûlé ;
- la **dominante** : moyennes linéaires rouge / vert et bleu / vert, sans les pixels
  bouchés ni presque brûlés (un canal écrêté fausse le rapport) ; sur une zone (`Zone`)
  pour la **pipette**, sur toute l'image pour le « monde gris ».

`apply_to(référence, base de la référence, réglages de la cible, base de la cible, modes)` :

1. décalage créatif et noir et blanc d'abord (ils comptent dans la mesure) ;
2. balance harmonisée : la cible prend la dominante de la référence **corrigée** ;
3. exposition harmonisée : la luminance médiane de la cible, avec tous ses nouveaux
   réglages, égale celle de la référence avec les siens. L'exposition est un simple
   facteur en linéaire : un seul calcul suffit, aux pixels écrêtés près.

Une petite image suffit pour les mesures (800 px en ligne de commande). Sur une image en
noir et blanc, l'harmonisation travaille sur le noir et blanc (décidé : l'écart se mesure
après les réglages déjà posés).

La décision de départ prévoyait de mesurer sur les histogrammes RAW du cache d'analyse ;
mesurer sur l'image montrée marche pareil pour toutes les sources (RAW, JPEG boîtier,
images plates) et tient compte du noir et blanc.

## Rapport

`report` : exposition totale à reporter (correction + créatif, aussi séparées), chaleur,
teinte, gains, mélange en pour cent. Première étape avant d'écrire dans les formats des
logiciels de développement : on reporte ces valeurs à la main.

## Ligne de commande

```
tri --adjust <référence> <image>... --to <dossier> [--ev IL] [--neutral auto|x,y,l,h]
    [--creative-ev IL] [--warmth IL] [--tint IL] [--mono <préréglage>|r,v,b] [--embedded]
```

La première image est la référence, réglée par les options (`--ev` et `--neutral` : sa
correction ; `--creative-ev`, `--warmth`, `--tint` : le décalage créatif ; `--mono` : un
préréglage ou trois pourcentages). Les suivantes reçoivent ses réglages en mode par
défaut. Chaque image est écrite en `<nom>-adjusted.jpg` dans `--to` (jamais à côté des
photos), à la taille des aperçus (`--side`), et son rapport est affiché. Un RAW est rendu
par LibRaw ; `--embedded` prend son JPEG boîtier (approximation).

Exemple : harmoniser une série sur sa première image, neutraliser la balance par une zone
grise et passer en noir et blanc filtre orange :

```
tri --adjust DSCF0101.RAF DSCF0102.RAF DSCF0103.RAF --to /tmp/essai \
    --neutral 0.40,0.55,0.10,0.10 --mono orange
```

## Tests

`src/adjust.rs` : un IL double la lumière (sRGB et BT.709), courbes réversibles,
mélangeur, chaleur / teinte, pipette sur une zone grise, harmonisation d'une image à
-1 IL, balance alignée sur la référence corrigée, série en noir et blanc mesurée après le
mélange, modes forcés, rapport. `tests/cli.rs` : une copie à -1 IL reçoit environ +1 IL.
