# `tri`

## Histoire

Comme beaucoup d'outils, `tri` est né d'une frustration : les logiciels
disponibles, libres comme commerciaux, ne correspondent pas à mon flux de
travail. J'ai donc essayé de construire l'outil qu'il me faut.

## Flux de travail

1. Décharger une ou plusieurs cartes mémoire.
2. Noter les photos (principalement avec des étoiles) et sélectionner les meilleures.
3. Développer dans mon logiciel préféré du moment.
4. Exporter différentes versions des images (réseaux sociaux, archives…).

## Problématique

- Beaucoup d'images sont souvent similaires : les comparer est fastidieux.
- Il n'est pas toujours facile d'évaluer quelles photos sont parfaitement nettes.
- Sans développer, il est parfois difficile d'évaluer si une exposition est bonne.
- Créer et conserver les différentes versions d'une image alourdit le flux.

## Objectifs

> `tri` est encore en phase de développement et n'est donc pas parfait.
> Vous avez un rôle à jouer pour faire remonter des bogues et des idées
> d'amélioration.

### Principes

- Pas de catalogue : on travaille directement sur les dossiers, les notes vont
  dans des sidecars à côté des fichiers.
- Utilisable entièrement au clavier.
- Multiplateforme : Windows, Linux, macOS.
- Libre (licence GPL v3.0).

### Déchargement

`tri` peut gérer le déchargement des cartes mémoire sur un volume local, sans
avoir besoin de passer par un outil tiers.

### Afficher rapidement les brutes

On n'a pas forcément besoin d'un développement poussé pour trier : souvent, le
JPEG intégré suffit.

### Afficher des données pertinentes

#### *Histogramme*

Dans presque tous les logiciels, comme sur les appareils photo, l'histogramme
est calculé à partir du JPEG intégré. `tri` propose un histogramme basé sur les
**données brutes**, ce qui permet de mieux évaluer la présence de données
brûlées ou enterrées dans l'image.

#### *Surimpression*

`tri` propose d'afficher en surimpression un masque de contraste, pour évaluer
la netteté. Il peut aussi afficher les zones brûlées ou enterrées.

### Filtrage et tri

#### *Notes*

`tri` peut noter les images (étoiles, libellés de couleur, drapeaux), et ces
notes sont compatibles avec les principaux logiciels du marché.

#### *Filtres*

Dans `tri`, on peut filtrer et trier les photos par note (étoiles, couleurs,
drapeaux), mais aussi exclure automatiquement les photos floues ou mal exposées.
On peut aussi regrouper les photos similaires pour les comparer et sélectionner
plus facilement lesquelles conserver.

### Et aussi

#### *Versions*

`tri` permet de créer plusieurs versions d'une même image développée, ce qui est
utile par exemple pour dériver plusieurs ratios différents pour les réseaux
sociaux. Pas besoin de créer des copies virtuelles dans votre logiciel de
développement.

#### *Développements*

`tri` peut appeler les logiciels qui le proposent via leur interface en ligne de
commande, ce qui lui permet de générer un développement lui-même : pas besoin
d'exporter un JPEG ou un TIFF depuis votre logiciel de traitement favori.

#### *Ligne de commande*

`tri` peut être utilisé en ligne de commande, ce qui permet d'automatiser
certaines tâches (traitements, sidecars, actions sur les fichiers, déchargement,
export), sans devoir passer par l'interface graphique.

#### *Pilotage au clavier*

`tri` peut être utilisé au clavier, sans jamais toucher à la souris ; avec
l'habitude, c'est bien plus efficace et rapide. Par ailleurs, il propose par
défaut des raccourcis classiques, et des raccourcis inspirés de vim.
