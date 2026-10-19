# Module `tri::versions` : recadrages d'une photo

Décidé le 19/10/2026 avec l'utilisateur, première pièce de l'« après-développement »
(voir `CLAUDE.md`). Une photo peut avoir plusieurs **versions** (story 9:16, publication
4:5…). Ce sont des **instructions** de recadrage, jamais des copies : corriger le
développement suffit, toutes les versions suivent à l'export.

## Décisions de l'utilisateur

- **On recadre ce qu'on voit** : le développé de darktable (par-dessus son éventuel
  recadrage), le rendu RAW de LibRaw, ou le JPEG boîtier (`Source`). L'usage de
  l'utilisateur : ne pas recadrer dans darktable, et recadrer dans `tri`.
- **Pas de redressement** (rotation fine) : c'est le travail de darktable. Il pourra venir
  plus tard, dans un autre outil, pour garder des interfaces simples, dédiées à une tâche.
- Une version a un **nom**, un **cadre**, un **rapport**, et en option un **profil
  d'export**. Celui-ci pré-remplit seulement la boîte d'export, qui reste modifiable.
- Grille : un **badge** (« 3 versions ») sur la vignette, pas une vignette par version,
  pour ne pas surcharger la grille.
- Recadrage : la photo en grand, le cadre réglable (libre ou à rapport fixe) et un **voile
  gris translucide** autour. Rapport et orientation se règlent **à la souris aussi**.
  Raccourcis selon le profil : flèches (classique), `hjkl` (vim).

## Données

- `Rect` : le cadre, en **proportions de l'image vue** (redressée), `[x, y, largeur,
  hauteur]` entre 0 et 1, arrondi à 1e-5 dans le fichier.
- `Ratio` : `Free`, `Original` (celui de l'image, éventuellement tourné d'un quart), ou
  `Fixed(w, h)`, où l'orientation est l'ordre (4:5 portrait, 5:4 paysage). Rapports proposés
  (`PRESETS`) : libre, original, 1:1, 5:4, 3:2, 16:9, 4:3. `rotated` change l'orientation.
- `Version` : `name`, `crop`, `ratio`, `source`, `profile` (facultatif).

## Géométrie, testée sans interface

L'interface ne fait que dessiner ; tout le calcul est ici.

- `Rect::fit(ratio, aspect)` : le plus grand cadre d'un rapport, centré.
- `Rect::with_ratio` : changer de rapport ou d'orientation autour du même centre, sans
  grandir au-delà de l'aire actuelle ni sortir de l'image.
- `Rect::drag(handle, dx, dy, ratio, aspect)` :
  - un angle garde l'angle opposé fixe ;
  - un côté garde le côté opposé fixe, et l'autre axe centré si le rapport est fixe ;
  - `Move` déplace le cadre ;
  - le cadre reste toujours dans l'image, avec des côtés d'au moins `MIN_SIDE`.
- `Rect::pixels` : le cadre en pixels d'une image donnée.

## Stockage

- Dans le sidecar XMP de la photo, liste `tri:Versions` (espace de noms `urn:tri:xmp:1.0`) :
  un `rdf:Seq` dont chaque élément est un **objet JSON** (`serde_json`). Exemple :

  ```xml
  <tri:Versions xmlns:tri="urn:tri:xmp:1.0">
   <rdf:Seq>
    <rdf:li>{"name":"Story","crop":[0.07813,0.0,0.84375,1.0],"ratio":"9:16","source":"developed","profile":"web"}</rdf:li>
   </rdf:Seq>
  </tri:Versions>
  ```

- Liste générique du module XMP (`xmp::ListField`, `read_list`, `apply_list`,
  `write_list`), **à part des notes** : le réécrivain des notes ne la connaît pas, donc il
  la recopie telle quelle. Écrire la liste ne change rien d'autre, octet pour octet.
  Seuls `&`, `<` et `>` sont échappés, pour que le JSON reste lisible.
- Vérifié le 19/10/2026 sur une copie du sidecar darktable de l'utilisateur : seul ajout,
  l'élément `tri:Versions`, et `darktable-cli` rend l'image normalement.
- Une version illisible (écrite par une version plus récente de `tri` ?) est gardée telle
  quelle (`Versions::unknown`).
- **Annulable** : `history::Step::Versions` garde la liste avant et après. Si le sidecar a
  été modifié ailleurs depuis, l'annulation est refusée et rien n'est écrasé.

## Interface (`gui/src/crop.rs`)

- **`r`** (`core.view.crop`) : recadrer l'image affichée (depuis la grille, ouvre le plein
  écran). On recadre ce qu'on voit, à l'échelle ajustée.
- **Voile** gris translucide autour du cadre, cadre blanc avec liseré sombre, huit
  **poignées** : angles et côtés à la souris, glisser dedans pour déplacer. La
  superposition choisie (`o`) se cale sur le cadre.
- **Barre** sous l'image : rapports (libre, original, 1:1, 5:4, 3:2, 16:9, 4:3), le
  bouton allumé dans l'orientation choisie (4:5) ; Portrait / Paysage ; nom (il suit le
  rapport tant qu'on ne l'a pas tapé) ; profil d'export facultatif ; Enregistrer / Annuler.
- **Clavier**, contexte « recadrage » (`Scope::Crop`, prioritaire tant que le cadre est
  affiché) :
  - flèches (classique) ou `hjkl` (vim) pour déplacer ;
  - avec Maj : côté droit (gauche / droite) et bas (haut / bas) ;
  - chiffres 1 à 7 (avec ou sans Maj, AZERTY) pour le rapport ;
  - `x` pour l'orientation ;
  - Entrée pour enregistrer, Échap pour abandonner.

  Le pas du clavier : `[gui.crop] step`. Tout est réglable dans `[keys]`.
- **Versions** d'une image en plein écran : une ligne à elles sous les notes (au bout de
  la ligne des notes, les icônes de droite les recouvraient sur un écran étroit) :
  - « Original » puis une pastille par version ;
  - pour la version montrée : Modifier (rouvre son cadre, sur l'image où elle a été faite)
    et Supprimer (annulable).
  - `n` fait défiler l'original puis chaque version.
  - Une version montrée n'affiche que son cadre, ajusté à la vue ; son nom suit la source
    dans l'étiquette. Les repères placés sur le capteur (collimateur, zones) y sont
    masqués.
- Le cadre d'une nouvelle version suit les proportions réelles de l'image affichée : tant
  qu'elles sont inconnues, rien n'est calculé, et l'enregistrement attend le premier
  dessin. Il n'y a plus de proportions supposées (3:2) : une version enregistrée trop tôt
  aurait eu un cadre faux.
- Réglages `[gui.crop]` : `step`, `shade`, `shade_opacity`, `line_width`, `handle_size`,
  `handle_grab`.

## À venir

Le badge dans la grille, et l'export d'une version (rendu, puis cadre, puis profil ;
`Ctrl+E` sur une version pré-remplit la boîte avec son profil).
