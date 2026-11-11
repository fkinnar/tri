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

## Balance automatique

`auto_balance`, sur les **pixels presque neutres** : ne comptent que les pixels peu
saturés (écart entre canaux sous `[adjust] auto_balance_chroma`, 20 % du plus fort), ni
bouchés ni presque brûlés ; on les neutralise, puis on recommence
(`auto_balance_passes`, 4), ce qui est neutre changeant avec la correction. Moins de 1 %
de pixels neutres : le « monde gris » (moyenne de toute l'image), faute de mieux. Retour
du 10/11/2026 : sur un portrait au visage et au pull chauds, le monde gris donnait
4 343 K (bien trop froid), les pixels neutres 5 119 K, à côté de la balance du boîtier
(5 167 K). La pipette, elle, prend la moyenne de sa zone.

## Exposition automatique

`auto_exposure` : la correction qui amène la luminance médiane de l'image (sa balance
corrigée, sans les autres réglages) à `[adjust] auto_exposure_target` (0,18, le gris
moyen des posemètres), **à priorité aux hautes lumières** : en éclaircissant, on s'arrête
quand le centile `auto_exposure_highlights` (0,99) atteint
`auto_exposure_highlight_target` (0,6) ; en assombrissant, quand il y descend. Retour du
10/11/2026 : sur un portrait en low-key (pull et fond sombres, mesure spot sur le visage),
la médiane seule donnait +3,3 IL et un visage brûlé ; la priorité aux hautes lumières
donne +0,3 IL. Une scène de neige, elle, n'est pas ramenée au gris.

**Compensation** `auto_exposure_bias` (+0,5 IL ; retour du même jour : « généralement un
peu sombre ») : relève ensemble la médiane visée et le plafond des hautes lumières
(celui-ci jamais au-delà de 0,95). Pas le +0,7 IL de darktable, qui compense la marge que
le boîtier garde sur les données RAW : on mesure ici le JPEG du boîtier, où elle est déjà
absorbée ; ce sont les deux cibles, prudentes, qui assombrissaient. Sur le portrait
ci-dessus : +0,82 IL.

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

Chaque image 8 bits est ramenée en lumière linéaire avec **sa** courbe (`Transfer`).
Depuis la gestion des couleurs (11/11/2026, [colour.md](colour.md)), toute image affichée
est dans l'espace de travail de `tri` (primaires Rec.2020, courbe sRGB), rendus LibRaw
compris (`OutputSpace::WORKING`) : la courbe est sRGB pour toutes ; avant, les rendus
LibRaw étaient en BT.709, et en décoder un comme du sRGB faussait la mesure dans les
ombres (essai du 10/11/2026 sur une photo sombre : +1,18 IL au lieu de +1). Les réglages
restent définis sur des **canaux sRGB** : chaque pixel passe de l'espace de travail à la
lumière linéaire aux primaires sRGB (`colour::working_to_srgb_linear`, valeurs sous 0 ou
au-dessus de 1 hors du gamut sRGB), y est réglé, puis revient ; une balance, un mélange
ou une lecture de la pipette donnent le même résultat qu'avant la gestion des couleurs.

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

`report` : exposition totale à reporter (correction + créatif, aussi séparées), balance
en kelvins (et celle de la prise de vue), chaleur, teinte, gains, mélange appliqué en pour
cent. Première étape avant d'écrire dans les formats des logiciels de développement : on
reporte ces valeurs à la main (onglet « Valeurs »).

## Dans l'interface

Section **« Réglages »** du panneau de droite (en bas, après les EXIF ; grille et plein
écran), pour l'image courante (`gui/src/app/adjust.rs`), réorganisée après le premier
essai (retours du 10/11/2026) :

- **« Synchroniser N images »**, en tête : grisée avec une seule image, cochée par défaut
  dès qu'il y en a plusieurs. Voir plus bas.
- **Quatre onglets**, retenus dans la session, un point « • » sur ceux qui changent
  l'image (un réglage caché se voit) ; **double-clic sur un onglet : remis à zéro** (base :
  exposition et balance de la prise de vue ; style ; noir et blanc : la couleur), sur la
  sélection synchronisée aussi :
  - **Base** : exposition et **« Auto »** ; température (K), avec la liste des balances
    (telle que prise, auto sur toute l'image, préréglages avec leurs kelvins) et la
    **« Pipette »** (en plein écran, un clic sur une zone qui devrait être grise, `[gui]
    adjust_eyedropper_size` ; ou un **cadre tracé** pour une balance auto sur cette
    partie ; depuis la grille, le bouton ouvre le plein écran) ; teinte.
  - **Style** : exposition, chaleur, teinte, **contraste** et **saturation** (de -100 à
    +100 % ; contraste : courbe en lumière linéaire pivotée sur le gris moyen, appliquée
    à la luminance, teinte gardée, aussi au noir et blanc après le mélange ; saturation
    autour de la luminance, -100 % : gris). Pas de case (l'onglet suffit) : le
    décalage créatif existe dès qu'une valeur n'est pas nulle.
  - **N&B** : le **filtre** (liste : « Aucun (couleur) », puis rouge, orange, jaune,
    vert, bleu ; pas de neutre, la luminance seule n'apportant rien face aux filtres ; « Personnalisé » dès qu'on touche au mélange), rouge / vert /
    bleu en %, **« Normalisé »** (coché par défaut) et le total. Pas de case : choisir un
    filtre ou toucher un canal passe en noir et blanc, « Aucun » revient à la couleur.
  - **Valeurs** : le tableau à reporter dans un logiciel de développement (exposition
    totale, base, style ; balance et prise de vue en kelvins ; balance du style ;
    mélange), clic droit sur une ligne pour la copier, « Tout copier ».
- Chaque réglage : son **titre** (avec Auto, la liste ou la pipette à droite), puis le
  **curseur sur toute la largeur** et sa valeur à taper (exposition au centième d'IL,
  kelvins à l'unité). Le curseur commun de l'interface : glisser par grands crans, Maj +
  glisser pour le réglage fin ; double-clic : valeur par défaut (température et teinte :
  la balance de la prise de vue).
- **Normalisé** : le total reste à 100 %. Déplacer un canal déplace les deux autres en
  sens inverse, au prorata de leurs parts (moitié chacun s'ils sont nuls ou de signes
  opposés) ; aux bornes (-100 % à 200 %), le canal tiré s'arrête là où les autres ne
  suivent plus. Cocher la case sur un mélange qui n'est pas à 100 % le remet à l'échelle
  (`Mixer::set_locked`, `Mixer::rescaled`).
- Boutons **en icônes** (nom et explication en bulle) : exposition auto (« A » cerclé),
  pipette, et en bas, **« Copier les réglages »** / **« Coller les réglages »** (voir plus
  bas), **« Harmoniser la sélection »** (exposition et balance des images sélectionnées
  alignées sur l'image courante, sans toucher au style ni au noir et blanc ; un pas),
  **« Avant »** (toutes les images sans leurs réglages, bascule, sidecars intacts),
  **« Réinitialiser »**.

### Tout un dossier

Clic droit sur le **dossier ouvert**, **« Réglages »** : **Auto** (balance puis
exposition : l'exposition se mesure balance corrigée), **Exposition** ou **Balance des
blancs**, automatiques, de toutes ses images (même masquées par un filtre), chacune d'après la sienne, mesurée sur
sa vignette, **en tâche de fond** (l'interface ne se fige pas, le menu se ferme tout de
suite) ; un seul pas de l'historique, les images sans vignette prête laissées de côté
(bilan dans la barre d'état). Seulement le dossier ouvert, comme les entrées des notes :
ses vignettes sont là.

### Synchroniser une sélection

Plusieurs images sélectionnées et la case cochée : ce qu'on fait sur l'image courante
part vers toute la sélection, en **un seul pas** de l'historique. Règle : **un curseur
décale, un bouton calcule** (`tri::adjust::propagate`).

| Réglage | Sur les autres images |
|---|---|
| Exposition (curseur) | même écart en IL (A 0,3 → 0,5 : B 1,0 → 1,2) |
| Température (curseur) | même rapport en kelvins (règle de 3, chacune dans ses kelvins) |
| Teinte, style (curseurs) | même écart |
| Auto, préréglages, telle que prise | recalculés sur chaque image (une image à -1 IL revient à 0, celle à 0 ne bouge pas) |
| Pipette | l'image courante neutralisée sur la zone, les autres harmonisées sur elle |
| Noir et blanc | copié |

L'exposition est additive et non proportionnelle : l'IL est déjà une échelle relative
(+0,2 IL = la lumière × 1,15 sur chaque image) ; une règle de 3 sur les IL éclaircirait
plus les images déjà corrigées et ne marcherait pas depuis 0. Les kelvins, toujours
positifs, suivent bien une règle de 3 ; la teinte, autour de 0, un écart. Seule la
**couche touchée** part : changer le noir et blanc ne réharmonise pas l'exposition.
Pendant le glissement, **toute la sélection suit en direct**, et toutes les vignettes
changent ensemble : chacune est redessinée sur place d'après sa vignette sans réglage,
gardée en mémoire le temps du geste (ni disque ni décodage à chaque image) ; les images
et leurs sidecars sont écrits au relâchement. En plein écran, la grande image **reste
affichée** pendant qu'elle est recalculée en fond, puis est remplacée d'un coup (avant,
jetée, la vue retombait sur la vignette : l'image « gigotait » et la barre d'état
changeait à chaque mouvement) ; chaque changement est numéroté, une image revenue du
chargeur n'est à jour que si elle a été demandée après le dernier.

### Copier / coller

**« Copier les réglages »** retient ceux de l'image courante **et sa vignette** ;
**« Coller les réglages »** les pose sur l'image courante (et la sélection synchronisée),
même dans un autre dossier : exposition et balance **harmonisées** sur l'image copiée,
style et noir et blanc copiés (modes par défaut de `apply_to`). Pas de raccourci par
défaut : Ctrl+Maj+C et Ctrl+Maj+V sont déjà pris (copier une pile, version précédente).

Les curseurs montrent le changement tout de suite (vignette et grande image) ; le sidecar
est écrit quand on lâche, un pas de l'historique par geste (`u` / Ctrl+Z l'annule). Les
réglages sont lus en fond dans les sidecars (jamais d'attente sur un disque lent) et
appliqués aux pixels juste avant la texture (`gui/src/adjust_view.rs`, partagé avec le fil
de chargement). **À l'écran, tout est fait sur l'image** (approximation sur les JPEG,
mélangeur exact) ; l'exposition sur les données du capteur reste pour la ligne de
commande et l'export à venir. Un développement (darktable…) n'est jamais réglé.

Actions sans raccourci par défaut, dans la palette : `core.adjust.before_after`,
`core.adjust.apply` (harmoniser la sélection), `core.adjust.copy`, `core.adjust.paste`.

## Ligne de commande

```
tri --adjust <référence> <image>... --to <dossier> [--ev IL|auto] [--neutral auto|x,y,l,h]
    [--kelvin K[,teinte]|<préréglage>] [--creative-ev IL] [--warmth IL] [--tint IL]
    [--contrast %] [--saturation %]
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
