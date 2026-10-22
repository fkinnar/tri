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
- **Versions** d'une image en plein écran : une liste « Versions : Original ▾ » sur la
  ligne des notes (issue n° 5, 22/10/2026 : sur une ligne à elles, elles apparaissaient
  selon l'image et changeaient sa taille ; des pastilles au bout de la ligne étaient
  recouvertes par les icônes de droite sur un écran étroit) :
  - « Original » puis une entrée par version ;
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

- **Grille** : badge dans la légende des vignettes qui ont des versions, à gauche du
  format (icône de recadrage dessinée et leur nombre ; en haut à gauche jusqu'au
  21/10/2026) ; un clic passe en recadrage. Versions lues dans les sidecars à la demande, puis
  gardées ; relues après une modification (enregistrement, annulation, surveillance du
  dossier).
- La barre du recadrage ne propose que les profils d'export qui s'appliquent à l'image
  recadrée (`applies_to`), et **jamais un profil « tel quel »**, quelle que soit l'image.
  Un recadrage change l'image, donc un profil qui ne change rien n'a pas de sens.
  Question de l'utilisateur (19/10/2026) : « tel quel » est déduit du profil
  (`Profile::is_as_is` : JPEG, sans recompression, espace d'origine, pleine taille, sans
  accentuation), pas d'un réglage. Un profil de l'utilisateur ainsi réglé est traité de
  même.
- Export d'une version sans profil : le profil choisi, sinon le premier profil JPEG (Web
  par défaut), plutôt qu'un TIFF de retouche.

## Export d'une version

- `Ctrl+E` sur une version montrée l'exporte, elle seule, sur l'image où elle a été
  recadrée (sa `source`) ; la boîte est titrée « Exporter la version « Story » » et
  pré-remplie avec son profil s'il en a un (sinon le dernier choisi, sinon le profil par
  défaut).
- Fichier : `DSCF0001-Story.jpg` (`export::Cut`, nom nettoyé des caractères interdits par
  les systèmes : `3:2` donne `3_2`).
- **LibRaw ou JPEG boîtier** : rendu, puis recadrage, puis réduction et écriture
  (`export_libraw` avec `Some(cut)`). Pas de copie sans recompression : recadrer, c'est
  recompresser.
- **darktable** : `export_version_darktable` rend un **TIFF 16 bits temporaire** dans
  l'espace du profil, en pleine définition ; il est ensuite recadré, réduit et écrit au
  format du profil, avec le profil ICC du TIFF. Il n'y a ainsi qu'une seule compression,
  et le TIFF est supprimé. Vérifié le 19/10/2026 sur le RAF de l'utilisateur (test ignoré
  `darktable_exports_a_cropped_version`).
- Le cadre est en proportions de l'image **vue** : sur un développé darktable recadré
  (2763 × 4912 pour le RAF de test), ce sont ses proportions, pas celles du capteur.

## Préréglages de recadrage

Idée de l'utilisateur (19/10/2026) : « Story Insta » = un nom, un rapport avec son
orientation (9:16) et un profil d'export (Web). On choisit le préréglage, on ajuste le
cadre, Entrée.

- **Stockage** : `[versions.presets.<id>]` (`VersionSettings`, `Presets`), avec `name`,
  `ratio` (`"free"`, `"original"`, ou `"l:h"` dans l'orientation voulue) et `profile`
  (`""` : aucun). Une clé absente vient du préréglage fourni de même identifiant, comme
  pour les profils d'export. Préréglages fournis : `story` (9:16, Web) et `post` (4:5,
  Web).
- **Barre du recadrage** : liste « Préréglage » ; `Tab` / `Maj+Tab` passe au suivant /
  précédent (`p` jusqu'au 21/10/2026 : c'est aussi la carte de netteté, qui restait
  affichée ; `p` l'éteint aussi pendant le recadrage).
  - Choisir un préréglage applique son rapport **exact**, son nom et son profil.
  - Le profil n'est appliqué que s'il existe et s'applique, et jamais un profil « tel
    quel ».
  - Le cadre se recentre au plus grand de ce rapport, puis on l'ajuste.
- **Enregistrer comme préréglage** (petite disquette à côté de la liste des
  préréglages ; Enregistrer et Annuler à droite de la barre, à part, retour du
  20/10/2026) : le rapport, le nom et le profil du cadre en cours
  sont écrits dans le fichier de configuration, sans perdre les commentaires
  (`config_edit`). L'identifiant est tiré du nom (`versions::new_id` : minuscules,
  accents retirés, numéro s'il est pris). Appliqué à chaud.
- Fenêtre de réglages : une seule entrée « Préréglages de recadrage », liste déroulante
  des préréglages, « Nouveau… » (copie de celui affiché), « Supprimer » (ceux de
  l'utilisateur) ; liste déroulante des rapports pour `ratio` (comme les profils
  d'export, voir `export.md`).

Au passage (19/10/2026) : les profils d'export, les préréglages et darktable modifiés
dans la fenêtre de réglages s'appliquent désormais **à chaud** (auparavant, il fallait
relancer).
