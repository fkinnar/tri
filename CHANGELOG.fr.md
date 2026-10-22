# Historique

*[English version](CHANGELOG.md)*

## À venir

- **Plein écran** : numéro, source et taille dans la barre d'état au lieu de sur l'image ;
  en mode concentration, sur l'image, masqués par `Ctrl+I`.
- **Couleur de fond** : `Ctrl+L` fait tourner le fond de la grille et du plein écran entre
  celui du thème, gris foncé, noir et blanc (`[gui.colors] backgrounds`), sans doublon.
- **Panneaux** : nom du fichier en ligne des métadonnées ; un clic sur un dossier donne
  le clavier au panneau, comme pour les métadonnées ; liseré plus fin
  (`[gui.dialogs] focus_frame_width`).
- **Notes** : les bulles des étoiles, couleurs et drapeaux donnent leurs raccourcis.
- **Aide** : s'ouvre tout de suite à sa taille (elle grandissait sur plusieurs images
  la première fois), et la colonne des touches ne déborde plus de la fenêtre
  (issue n° 11). Ouverture fluide : préparée au démarrage, seules les lignes visibles
  dessinées (75 ms par image ramenées à 0,3 ms).
- **Plein écran** : badges brûlée, bouchée et floue dans le bandeau, comme dans la
  grille (clic : grand histogramme, carte de netteté ; issue n° 9).
- **Plein écran** : notes plus affichées deux fois ; sur l'image seulement quand le
  bandeau de notation est masqué (issue n° 8).
- **Filtre « bien exposées »** : la grille ne défilait plus (refiltrée sans fin, chaque
  fois ramenée à la sélection ; issue n° 10).
- **Plein écran** : les versions d'une image dans une liste sur la ligne des notes ;
  l'image ne change plus de taille d'une photo à l'autre (issue n° 5).
- **Boîte « Mise à jour »** : notes de version mises en forme (gras, listes, code) au
  lieu du Markdown brut (issue n° 3).
- **Aide** : version plus visible et copiable (avec le système), et un bouton
  « Signaler un problème » qui ouvre le formulaire de GitHub prérempli (issue n° 7), aussi
  `Ctrl+B b` (le leader, puis b).
- **Réglages** : une clé réglée dans votre fichier de configuration est en gras,
  maintenant dit dans la fenêtre et au survol (issue n° 6).
- **Notes de version dans la langue de l'interface** : la boîte « Mise à jour » montre
  les notes en français ou en anglais ; historique en anglais (`CHANGELOG.md`) et en
  français (`CHANGELOG.fr.md`), les deux dans la Release.
- **Recadrage** : une poignée approchée par l'intérieur se prend dès que le curseur
  change (elle déplaçait le cadre entier ; issue n° 1).

## 1.0.0-alpha.4 — 22/10/2026

Quatrième version de test, avant la bêta 1.

- **Rechercher une mise à jour** : bouton en haut de l'aide (F1) et `tri --check-update`,
  sur demande seulement (page Releases, lue par `curl`) ; la boîte donne les notes de la
  nouvelle version et ouvre sa page. Un build de nuit ou personnel le dit, et compte les
  commits de `main` qui lui manquent (« git pull, puis recompiler ») ; numéro de version
  avec le commit dans l'aide.
- **Recherche dans l'aide et les réglages** : thèmes et sections restent visibles
  (sections trouvées en tête, avec leur nombre de résultats), le titre d'une section la
  trouve en entière, mots-clés cachés (« vim » trouve le jeu de raccourcis), et les mots
  trouvés tels quels écartent les correspondances à lettres éparpillées.
- **Builds de nuit** : la CI construit et teste chaque nuit où `main` a changé, paquets
  sur la Release « nightly ».
- **Filtres** : icône « inverser » à la taille des pastilles de couleur.
- **Notes à la souris** : recliquer l'étoile posée remet à zéro, comme le filtre des
  étoiles ; le bouton « ⊘ » est retiré (au clavier, la touche 0 ; `1` deux fois reste
  à 1).

## 1.0.0-alpha.3 — 21/10/2026

Troisième version de test, la première avec **Windows** (`.msi` et `.zip` portable), à
côté des paquets macOS (`.dmg`) et Linux (`.tar.gz`), toujours sans HEIC.

- **Piles** : curseur de sévérité comme celui du flou ; au centre, le seuil calculé
  d'après le dossier (coupure entre séries et changements de scène), à droite plus
  strict (petites piles) ; retenu par dossier. `tri --groups` regroupe au centre calculé.
- **Curseurs** : double-clic = valeur par défaut, partout. Flou : à fond à gauche, toutes
  les images ; en plein écran, la grille n'est plus refiltrée avant le retour (l'image
  regardée pouvait changer).
- **Couleurs** : étoiles, couleur choisie et interrupteurs [RAW] / [JPG] dans la couleur
  d'accent du thème (fini le jaune et le grand carré), badge RAW des vignettes aussi ;
  filtre « sans les floues » : un triangle au lieu de la goutte barrée.
- **Barre d'état** : plus de barres de progression éclair pendant un réglage
  (`[gui] progress_delay_ms`).
- « JPEG boîtier » devient « Rendu boîtier ».
- **Plein écran** : résolution du développé darktable, son recadrage lu dans le sidecar.
- **Filtres** : « bien exposées » (soleil, `f w` : sans les brûlées ni les bouchées) et
  « inverser » (◐, `f i` : ce que les filtres retirent ; « floues seulement » devient
  « nettes » inversé, `f B` retiré).
- **Paires RAW+JPEG** : case « avec l'autre fichier de la paire » dans les boîtes Supprimer
  et Copier / déplacer (supprimer tous les JPEG emportait les RAW).
- **Recadrage** : préréglages sur `Tab` / `Maj+Tab` (`p` reste la carte de netteté).
- **Windows** : installateur `.msi` (menu Démarrer, désinstallation depuis les
  Paramètres) et zip portable, avec LibRaw et le runtime Visual C++ ; plus de fenêtre de
  console derrière l'interface.
- **Réglages** : descriptions en anglais dans la fenêtre de réglages
  (`doc/config.example.en.toml`), selon la langue de l'interface.
- **Grille** : taille des vignettes réglable (curseur en bas, `+` / `-`, `*` ou bouton
  « tout à l'écran »), infos des vignettes masquables (`Maj+T`) ; nom sans extension et
  format (RAW, JPEG, TIFF…) écrit à droite ; photo floue : contour de goutte ; notes sans
  rectangle sombre (étoiles en bas à gauche, drapeau en bas à droite, couleurs en haut à
  droite) ; écrêtage en triangle mi-blanc mi-noir en haut à gauche (clic : grand histogramme),
  versions dans la légende (clic : recadrer) ; effacer les filtres :
  entonnoir barré tout à droite de la barre des filtres.

## 1.0.0-alpha.2 — 21/10/2026

Deuxième version de test, la première publiée avec ses paquets sur la page Releases
(dépôt public). Paquets macOS (`.dmg`) et Linux (`.tar.gz`), sans HEIC ; Windows à venir.

- **Dossiers** : copier ou déplacer un dossier entier (`Ctrl+C` ou `y` / `Ctrl+X` dans
  le panneau des dossiers, clic droit, annulable) ; `y` copiait les images
  sélectionnées au lieu du dossier. **Glisser-déposer** d'un dossier ou d'images de la
  grille sur un dossier, toujours confirmé (Option sur macOS, Ctrl ailleurs : copier).
  Panneau rangé en sections titrées : disques amovibles, réseau (partages montés, GVFS
  sous Linux), dossiers personnels (Bureau, Documents, Téléchargements, Images, comme le
  Finder), favoris, puis le système. Le clic droit « Ajouter aux favoris » ouvrait la boîte de
  suppression : corrigé.
- **Flou** : curseur de sévérité dans la barre des filtres (au centre les réglages, à
  droite plus strict, à gauche plus souple), en direct, retenu par dossier ; le panneau
  des métadonnées donne aussi le rapport à la pile.
- **Plein écran** : résolution de ce qu'on regarde (une version : son cadre, réduit par
  son préréglage d'export).
- **Réglages** : le préréglage d'export d'un préréglage de recadrage se choisit dans une
  liste (plus de faute de frappe possible).
- **Netteté** : la carte (`p`) s'affiche aussi en vue ajustée et suit le curseur de
  sévérité (plus souple : plus de contours nets) ; en plein écran, le curseur dans le
  bandeau et `Ctrl+J` / `Ctrl+K`.
- **Plein écran** : résolution juste pendant le recadrage et pour le JPEG boîtier.
- **Documentation** bilingue (`doc/en/`, `doc/fr/`), `README.md` en anglais.
- **Barre des filtres** : deux interrupteurs, étiquettes RAW et JPG, à la place de « Tout /
  RAW / Plates » ; curseur du flou montré seulement avec un filtre sur la netteté, de la
  largeur de celui des piles.
- **Sous-dossiers** : case du panneau des dossiers, leurs images dans la grille aussi
  (3 niveaux par défaut), choix gardé.
- **Copier / déplacer vers…** : titre sans verbe et bouton « Valider » ; destinations
  récentes sans doublons.
- **Plein écran** : après un développement darktable (Maj+B), les autres images ne
  zoomaient plus (« pas encore développé ») ; une image sans développement se montre
  maintenant dans la source d'avant, avec son 1:1.
- **Piles** : le curseur de similarité ne déplie plus les piles pendant le réglage (on
  voit les piles obtenues) ; Maj tenue pour les voir dépliées.
- **Rapport d'export** (`Ctrl+Maj+E`, ou le bouton à côté du bilan d'un export) : les
  exports terminés, gardés d'une session à l'autre, avec un lien vers chaque dossier de
  destination ; effaçable.
- **Vocabulaire** : « préréglage d'export », comme « préréglage de recadrage »
  (« profil » reste aux profils couleur).
- **Réglages** : une seule entrée pour les préréglages d'export et une pour ceux de
  recadrage, avec une liste, « Nouveau… » et « Supprimer ».
- **Recadrage** : « Enregistrer comme préréglage » devient une petite disquette à côté
  de la liste des préréglages ; Enregistrer et Annuler à droite.
- **Boîte d'export** : les flèches ne changent le préréglage que sur sa liste ; Tab passe
  au dossier.

## 1.0.0-alpha.1 — 19/10/2026

Première version de test (dépôt privé ; la bêta sera publique). Paquets prêts à
installer pour macOS (`.dmg`) et Linux (`.tar.gz`), sans HEIC ; Windows à venir.

- **Dossiers** : panneau des dossiers (volumes, favoris, recherche `/`), surveillance du
  dossier ouvert, déchargement de cartes (copie vérifiée, doublons, éjection).
- **Grille** : vignettes en tâche de fond avec cache persistant, filtres (type, étoiles,
  couleurs, drapeau, netteté, nom), sélection multiple, piles d'images semblables
  (seuil réglable en direct), comparaison côte à côte.
- **Plein écran** : JPEG boîtier ou vrai rendu RAW (LibRaw), zoom continu jusqu'à 400 %,
  loupe, repère du collimateur (Fujifilm), carte de netteté, zones saturées / bouchées,
  superpositions (grilles, spirale d'or, texte, logo, format 4:5…).
- **Notes** : étoiles, couleurs, accepté / rejeté, en XMP (conventions darktable et
  Adobe, rien d'autre n'est touché), annulables.
- **Analyses** : histogramme RAW en IL, score de netteté jugé dans le dossier et dans la
  pile, regroupement par similarité.
- **Fichiers** : renommer, corbeille, copier / déplacer vers…, éditeur externe, AirDrop
  (macOS).
- **Développé dans darktable** : `Maj+B` (ou le bouton sous l'image) fait
  le rendu darktable-cli, en cache, rien d'écrit chez vous ; `b` le montre ensuite.
- **Export** : `Ctrl+E`, JPEG / TIFF / PNG par profils d'export (« Retouche » TIFF 16 bits
  ProPhoto, « Web » JPEG sRGB), développés par darktable (installé, en AppImage ou en
  Flatpak) ; sans darktable, rendu de base par LibRaw ; pas de ProPhoto en 8 bits.
- **Interface** : tout au clavier (jeux « classique » et « vim », raccourcis réglables),
  fenêtre de réglages, aide avec recherche, français et anglais, thèmes sombre, clair et
  gris neutre ; icône, barre du haut, mode concentration (F11).
