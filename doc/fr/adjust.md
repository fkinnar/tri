# Réglages de visualisation (`tri::adjust`)

Issue n° 137, décisions du 10/11/2026. Quelques réglages de base **pour comparer et trier
une série**, pas pour développer : exposition, balance des blancs, noir et blanc par
mélange des canaux. La clé : **appliquer les réglages d'une image aux autres**, en
harmonisant ce qui doit l'être (une photo à -1 IL et une bien exposée finissent pareilles).

Faits : le moteur, la ligne de commande, l'interface (section « Réglages »), le rapport
des valeurs. Ensuite : l'écriture pour d'autres logiciels (RawTherapee / ART d'abord),
les vraies vignettes LibRaw en option, l'export des images réglées. Les réglages s'appliquent **avant** tout développement par un autre logiciel :
une image développée dans darktable n'est pas concernée.

## Couches

Les réglages d'une image (`Layers`) sont des couches, qu'on peut refaire séparément :

| Couche | Rôle | Part vers les autres images (mode par défaut) |
|---|---|---|
| `correction` (exposition, balance) | ramener l'image à une base commune | **harmonisée** : calculée pour chaque image |
| `creative` (exposition, balance), facultatif | l'intention, la même pour la série | **copiée** (le même décalage) |
| `mono` (mélangeur) | noir et blanc | **copié** |

Trois modes (`Mode`) peuvent être forcés couche par couche (`Modes`) :

- **absolu** : la valeur telle quelle ;
- **décalage** : le même écart ajouté à ce que l'image a déjà ;
- **harmonisation** : l'écart calculé pour que l'image ressemble à la référence ;
- et **garder** : l'image garde la sienne.

## Valeurs

- **Exposition** en IL.
- **Balance des blancs** enregistrée en gains rouge et bleu (`Gains`), relatifs à la
  balance de la prise de vue, le vert servant de référence. Montrée et tapée en
  **kelvins et teinte** pour la correction (voir plus bas) ; le décalage créatif garde
  **chaleur** (log₂ rouge / bleu, positif = plus chaud) et **teinte** (positif =
  magenta), relatives.
- **Mélangeur** : part du rouge, du vert, du bleu (1,0 = 100 %). **Normalisé** par
  défaut : les parts sont ramenées à une somme de 100 % à l'application, déplacer un
  curseur ne change pas la luminosité (`normalise`, écrit dans le sidecar seulement quand
  il est coupé). Préréglages façon filtres colorés : `neutral` (luminance Rec. 709),
  `red`, `orange`, `yellow`, `green`, `blue`.

## Kelvins (`adjust::kelvin`)

Retour du 10/11/2026 : la chaleur relative ne parlait pas ; on tape une température comme
dans un logiciel de développement. Les kelvins ne sont qu'une **présentation** des gains
enregistrés :

- un éclairage (K, teinte) est un point du plan CIE 1960 (u, v) : le lieu de Planck à K
  (approximation de Krystek), décalé perpendiculairement de `teinte / 3000` (Duv ;
  positif : vers le vert, donc l'image vire au magenta, comme dans Lightroom) ;
- sa couleur en sRGB linéaire est celle d'un blanc éclairé par lui, dans un rendu
  équilibré pour D65 ;
- l'image montrée est équilibrée pour l'éclairage de la prise de vue, de couleur `shot` :
  pour un RAW, la neutre du boîtier (`1 / cam_mul`) ramenée à la balance « lumière du
  jour » de LibRaw (`pre_mul`, D65) puis passée par `rgb_cam`
  (`RawFile::as_shot_white`, lue à l'ouverture, sans décompresser) ; D65 supposé pour une
  image plate. Équilibrer pour un autre éclairage `W`, c'est multiplier chaque canal par
  `shot / W` (von Kries dans l'espace de sortie), ramené au vert.

Approximation assumée : chaque logiciel calcule ses kelvins avec son propre modèle du
boîtier ; ceux de tri en sont proches, pas identiques (sur un RAF X-T4 en balance auto,
environ 5 970 K avec une teinte verte, cohérent avec la table de températures du
boîtier).

Préréglages (`[adjust]`, teinte nulle) : lumière du jour 5 500 K, nuageux 6 500, ombre
7 500, tungstène 2 850, fluorescent 3 800, flash 5 500 ; plus « telle que prise » (aucun
gain) et « auto » (monde gris).

## Exposition automatique

`auto_exposure` : la correction qui amène la luminance médiane de l'image (sa balance
corrigée, sans les autres réglages) à `[adjust] auto_exposure_target` (0,18, le gris
moyen des posemètres).

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

## Dans l'interface

Section **« Réglages »** du panneau de droite (en bas, après les EXIF ; grille et plein
écran), pour l'image courante (`gui/src/app/adjust.rs`) :

- **Correction** : exposition et **« Auto »** ; température (K) et teinte ; la liste des
  balances (telle que prise, auto sur toute l'image, préréglages avec leurs kelvins) ;
  **« Pipette »** : en plein écran, un clic sur une zone qui devrait être grise (`[gui]
  adjust_eyedropper_size`, 4 % de l'image) ou un **cadre tracé** pour une balance auto
  sur cette partie de l'image ; depuis la grille, le bouton ouvre le plein écran. Toutes
  les valeurs se **tapent** : clic sur le champ à côté du curseur (exposition au centième
  d'IL, kelvins à l'unité).
- **Décalage créatif** : case, puis exposition, chaleur, teinte. Décochée puis recochée
  dans la session : ses valeurs reviennent.
- **Noir et blanc** : case, filtres en boutons (neutre, rouge, orange, jaune, vert, bleu),
  rouge / vert / bleu en %, **« Normalisé »** et le total du mélange.
- Le **rapport** sous les curseurs (exposition totale, correction, créatif ; balance ;
  mélange), et **« Copier les valeurs »**.
- **« Appliquer à la sélection »** : les réglages de l'image courante vers les autres
  images sélectionnées, en modes par défaut (corrections harmonisées, mesurées sur les
  vignettes) ; un seul pas de l'historique.
- **« Avant »** : toutes les images sans leurs réglages (bascule), sans toucher aux
  sidecars.
- **« Réinitialiser »**. Double-clic sur un curseur : valeur par défaut.

Les curseurs montrent le changement tout de suite (vignette et grande image) ; le sidecar
est écrit quand on lâche, un pas de l'historique par geste (`u` / Ctrl+Z l'annule). Les
réglages sont lus en fond dans les sidecars (jamais d'attente sur un disque lent) et
appliqués aux pixels juste avant la texture (`gui/src/adjust_view.rs`, partagé avec le fil
de chargement). **À l'écran, tout est fait sur l'image** (approximation sur les JPEG,
mélangeur exact) ; l'exposition sur les données du capteur reste pour la ligne de
commande et l'export à venir. Un développement (darktable…) n'est jamais réglé.

Actions sans raccourci par défaut, dans la palette : `core.adjust.before_after`,
`core.adjust.apply`.

## Ligne de commande

```
tri --adjust <référence> <image>... --to <dossier> [--ev IL|auto] [--neutral auto|x,y,l,h]
    [--kelvin K[,teinte]|<préréglage>] [--creative-ev IL] [--warmth IL] [--tint IL]
    [--mono <préréglage>|r,v,b] [--embedded]
```

La première image est la référence, réglée par les options (`--ev`, `--neutral` et
`--kelvin` : sa correction, `--ev auto` l'exposition automatique ; `--creative-ev`, `--warmth`, `--tint` : le décalage créatif ; `--mono` : un
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

`src/adjust/kelvin.rs` : D65 ≈ 6 500 K, kelvins et couleurs inverses, plus de kelvins
réchauffent l'image et une teinte positive ajoute du magenta, gains ↔ kelvins.
`src/adjust/mod.rs` : un IL double la lumière (sRGB et BT.709), courbes réversibles,
mélangeur, chaleur / teinte, pipette sur une zone grise, harmonisation d'une image à
-1 IL, balance alignée sur la référence corrigée, série en noir et blanc mesurée après le
mélange, modes forcés, rapport, mélange normalisé, exposition automatique, décalage
créatif gardé coché sans valeurs. `tests/cli.rs` : une copie à -1 IL reçoit environ +1 IL.
