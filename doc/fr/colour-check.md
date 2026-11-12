# Vérifier les couleurs

*[English version](../en/colour-check.md)*

`tri` gère les couleurs : il lit le profil de chaque image (sRGB, Adobe RGB, Display P3,
ProPhoto…) et convertit vers le profil de votre écran. Cette fiche sert à vérifier que
c'est juste **chez vous**, en dix minutes. Elle intéresse surtout ceux qui ont un **écran
calibré** ou un écran à large gamut, en particulier sous **Windows** et **Linux**, où
c'est `tri` qui fait la conversion vers l'écran (sous macOS, c'est le système).

## Ce qu'il faut

- Une version de `tri` du **12/11/2026 ou plus récente** (nightly, ou compilée depuis
  `main`). Une version plus ancienne affiche toutes les images comme du sRGB.
- Les **mires** : le dossier [`doc/charts`](../charts) du dépôt. Téléchargez-le en entier
  (ou le zip du dépôt) et copiez-le où vous voulez. Si vous compilez `tri`, vous pouvez
  aussi les fabriquer, TIFF 16 bits compris :
  `cargo run --release --example colour_charts -- <dossier>`.
- Un **logiciel de référence qui gère les couleurs** : Photoshop, Lightroom, darktable,
  GIMP, ou un navigateur (Chrome, Firefox : glissez-y une mire). Sous Windows, la
  visionneuse Photos n'est pas fiable pour ça.

## Les mires

C'est **la même image**, enregistrée dans plusieurs espaces de couleur, chacune avec son
profil intégré (plus un JPEG sRGB sans profil). Dans un logiciel qui gère les couleurs,
elles doivent toutes **paraître identiques**.

De haut en bas :
1. rouge, vert, bleu, cyan, magenta, jaune ;
2. peau, ciel, feuillage, orange, violet, gris moyen ;
3. un dégradé gris ;
4. des dégradés du noir au rouge, au vert et au bleu ;
5. un tour des teintes ;
6. des couleurs **plus vives que le sRGB** (celles d'un écran Display P3), à côté du
   rouge sRGB.

La dernière rangée est l'exception : dans les fichiers `srgb…`, ces couleurs sont
coupées (c'est normal, le sRGB ne les contient pas), et un peu dans les `adobergb…`. Sur
un écran large gamut, elles sont plus vives dans les autres fichiers ; sur un écran
sRGB, presque pareilles partout.

## Les vérifications

1. **Les mires entre elles, dans `tri`.** Ouvrez le dossier des mires. Passez de l'une à
   l'autre, dans la grille puis en grand (flèches gauche / droite). Rangées 1 à 5 : rien
   ne doit bouger. Un fichier plus terne ou plus saturé que les autres, notez lequel.
2. **`tri` contre votre logiciel de référence.** Ouvrez `srgb.jpg` puis `prophoto.jpg`
   dans les deux, côte à côte, sur le même écran. Les couleurs doivent être les mêmes.
3. **Le profil d'écran trouvé par `tri`** (Windows, Linux). Dans les réglages
   (`Ctrl+,`), cherchez « trace » et cochez `[log] trace`. Relancez `tri`, ouvrez le
   journal (palette de commandes, `Ctrl+Maj+P` ou `:` en profil vim : « Journal ») et
   cherchez la ligne **« profil de l'écran : … »**. Elle donne le nom du profil utilisé ;
   « sRGB » veut dire qu'aucun profil n'a été trouvé. Décochez la trace ensuite (le journal grossit vite).
   - Sous Windows, le profil est celui associé à l'écran dans « Gestion des couleurs ».
     Déplacez la fenêtre sur un autre écran : la ligne doit changer.
   - Sous Linux, c'est le profil que colord (ou `dispwin`) publie pour l'**écran
     principal**, sous X11 (sous Wayland, seulement si le compositeur le publie).
   - Si le profil n'est pas trouvé, vous pouvez l'indiquer à la main : réglage
     `[gui] display_profile`, le chemin d'un fichier `.icc` ou `.icm`.

## Ce qu'il faut nous renvoyer

- Système, écran, et s'il est calibré (avec quel profil).
- Les fichiers qui ne ressemblent pas aux autres dans `tri` (vérification 1).
- `tri` et le logiciel de référence : pareils ou pas, et lequel était le logiciel de
  référence (vérification 2). Une photo de l'écran aide plus qu'une capture d'écran : une
  capture enregistre les valeurs envoyées à l'écran, pas ce qu'il montre.
- La ligne « profil de l'écran : … » (vérification 3).

Merci !
