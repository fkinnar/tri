# Catalogue de l'interface — français (langue de référence).
# Chaque identifiant doit exister aussi dans en.ftl (vérifié par les tests).

## Actions (aide, bulles, boutons) : action-<identifiant de l'action>

action-core-nav-prev = Image précédente
action-core-nav-next = Image suivante
action-core-nav-up = Ligne du dessus
action-core-nav-down = Ligne du dessous
action-core-nav-first = Première image
action-core-nav-last = Dernière image
action-core-nav-page_up = Page précédente
action-core-nav-page_down = Page suivante
action-core-filter-cycle = Filtre de type : tout, RAW, JPEG…
action-core-filter-kind-all = Filtre de type : RAW et JPEG
action-core-filter-kind-raw = Filtre de type : RAW seulement
action-core-filter-kind-flat = Filtre de type : JPEG seulement
action-core-filter-stars-0 = Filtre : étoiles indifférentes
action-core-filter-stars-1 = Filtre : 1 étoile
action-core-filter-stars-2 = Filtre : 2 étoiles
action-core-filter-stars-3 = Filtre : 3 étoiles
action-core-filter-stars-4 = Filtre : 4 étoiles
action-core-filter-stars-5 = Filtre : 5 étoiles
action-core-filter-stars-op = Filtre d'étoiles : au moins, exactement, au plus
action-core-filter-colors-all = Filtre de couleurs : toutes les couleurs choisies (et) ou l'une d'elles (ou)
filter-flag-toggle-accepted = Filtre : garder les acceptées (cumulable avec les autres drapeaux)
filter-flag-toggle-neutral = Filtre : garder les neutres, sans drapeau (cumulable)
filter-flag-toggle-rejected = Filtre : garder les rejetées (cumulable)
action-core-filter-flag-accepted = Filtre : acceptées seulement
action-core-filter-flag-not_rejected = Filtre : masquer les rejetées
volume-back = Disque revenu : dossier relu, calculs repris.
action-core-filter-flag-rejected = Filtre : rejetées seulement
action-core-filter-color-red = Filtre : rouge
action-core-filter-color-yellow = Filtre : jaune
action-core-filter-color-green = Filtre : vert
action-core-filter-color-blue = Filtre : bleu
action-core-filter-color-purple = Filtre : violet
action-core-filter-clear = Effacer tous les filtres
action-core-filter-blurry = Filtre : masquer les floues
action-core-filter-well_exposed = Filtre : bien exposées (ni brûlées ni bouchées)
action-core-filter-stack = Filtre : seulement la pile courante, toute dépliée (encore : tout)
action-core-filter-invert = Filtre : inverser (ce que les filtres retirent)
action-core-filter-conflicts = Filtre : en conflit (notes en désaccord d'un logiciel à l'autre)
kw-core-filter-conflicts = conflit désaccord notes logiciels trancher
action-core-filter-developed = Filtre : développées (allumer / éteindre ; logiciels à cocher dans la liste)
action-core-grid-render = Rendu affiché précédent (vignettes et plein écran)
action-core-view-render-embedded = Rendu affiché : JPEG boîtier
action-core-view-render-cover = Rendu affiché : couverture (développement préféré, sinon le plus récent)
action-core-view-render-darktable = Rendu affiché : Darktable
action-core-view-render-rapidraw = Rendu affiché : RapidRAW
action-core-view-render-rawtherapee = Rendu affiché : RawTherapee
action-core-view-render-art = Rendu affiché : ART
action-core-view-render-libraw = Rendu affiché : rendu RAW (LibRaw)
action-core-filter-sort = Ordre suivant : nom, date de prise de vue, date de modification, étoiles, couleur, drapeau
action-core-filter-sort_reverse = Inverser l'ordre (croissant / décroissant)
action-core-filter-sort_by-name = Trier par nom
action-core-filter-sort_by-captured = Trier par date de prise de vue
action-core-filter-sort_by-modified = Trier par date de modification
action-core-filter-sort_by-stars = Trier par étoiles
action-core-filter-sort_by-color = Trier par couleur
action-core-filter-sort_by-flag = Trier par drapeau
sort-name = Nom
sort-captured = Prise de vue
sort-modified = Modification
sort-stars = Étoiles
sort-color = Couleur
sort-flag = Drapeau
sort-notice = Ordre : { $key }, { $direction }
filter-analysis-partial = Analyses en cours : filtre appliqué aux { $done } / { $total } photos déjà analysées ; la grille se complète au fil du calcul.
sort-ascending = croissant
sort-descending = décroissant
action-core-view-filter_bar = Afficher / masquer la barre de filtres
action-core-view-image_info = Afficher / masquer les infos et les notes sur l'image
action-core-view-background = Couleur de fond suivante (thème, gris foncé, noir, blanc)
action-core-view-rating_bar = Afficher / masquer le bandeau de notation
action-core-view-grid_info = Afficher / masquer les infos des vignettes (nom, format, notes, badges)
action-core-rate-auto_advance = Avance automatique après une note : oui / non
action-core-select-toggle = Sélectionner / désélectionner l'image
action-core-select-visual = Mode visuel : sélectionner une plage (la même touche valide)
action-core-view-compare = Comparer les images sélectionnées côte à côte (sans sélection : la pile repliée, la photo et ses développements, sinon toutes les images)
action-core-view-peaking = Afficher / masquer la netteté (contours nets)
action-core-view-sharpness_lenient = Netteté plus souple (moins de photos floues, plus de contours nets)
action-core-view-sharpness_strict = Netteté plus stricte (plus de photos floues, moins de contours nets)
action-core-view-peaking_color = Netteté : couleur suivante
action-core-view-overlay = Superposition suivante (grilles, puis les vôtres : texte, logo, format)
overlay-shown = Superposition : { $name }
overlay-none = Superposition : aucune
overlay-error = Superposition { $file } : { $error }
action-core-view-overlay_orient = Superposition : tourner les lignes (œil de la spirale d'un coin à l'autre)
overlay-orientation = Superposition : orientation { $n } / 4
action-core-search = Rechercher : en grille, par nom de fichier ; dans le panneau des dossiers, un dossier (aussi /)
search-grid-hint = Rechercher ({ $keys })
search-folder-close = Fermer la recherche (Échap)
search-folder-hint = Rechercher… ~/ ou /
search-folder-tip = Rechercher un dossier ({ $keys }) : parmi les sous-dossiers du dossier pointé ; « ~/… » depuis le dossier personnel, « /… » depuis la racine ou un chemin tapé ; « \\nas\photo », « smb://nas/photo », « ftp://nas », « sftp://… », « davs://… » pour un serveur réseau. ↑ ↓ choisissent, Entrée ouvre, Échap referme.
search-folder-in = dans { $base }
search-folder-running = recherche…
search-folder-none = aucun dossier trouvé
search-folder-connect = Se connecter à « { $name } »
search-folder-address = adresse d'un serveur : \\serveur\partage, smb://serveur/partage ; aussi afp://, ftp://, sftp://, dav://, davs://, nfs://
search-folder-shares-running = recherche des partages de { $host }…
search-folder-shares-failed = partages de { $host } introuvables : { $error }
search-folder-shares-none = aucun partage sur { $host }
search-folder-shares-enter = Entrée : partages de { $host }
filter-summary-name = « { $query } »
action-core-view-focus_mode = Mode concentration : plein écran, sans panneaux ni barres
focus-mode-on = Mode concentration ({ $keys } pour en sortir)
action-core-view-focus_point = Afficher / masquer le collimateur de mise au point
action-core-select-all = Tout sélectionner
action-core-select-clear = Tout désélectionner
action-core-file-rename = Renommer…
action-core-file-delete = Mettre à la corbeille…
action-core-file-reveal = { $os ->
    [macos] Afficher dans le Finder
    [windows] Afficher dans l'Explorateur
    [nemo] Afficher dans Nemo
    [nautilus] Afficher dans Fichiers
    [dolphin] Afficher dans Dolphin
    [thunar] Afficher dans Thunar
    [caja] Afficher dans Caja
   *[other] Afficher dans le gestionnaire de fichiers
}
action-core-file-edit = Ouvrir dans l'éditeur externe
action-core-file-copy = Copier vers…
action-core-file-export-report = Rapport d'export
action-core-file-export = Exporter…
action-core-version-new = Nouvelle version (recadrer l'image, préréglage d'export facultatif)
action-core-version-next = Version suivante de l'image (original, puis chaque version)
action-core-version-prev = Version précédente de l'image
action-core-version-move-left = Version : déplacer le cadre à gauche
action-core-version-move-right = Version : déplacer le cadre à droite
action-core-version-move-up = Version : déplacer le cadre vers le haut
action-core-version-move-down = Version : déplacer le cadre vers le bas
action-core-version-size-left = Version : rapprocher le côté droit
action-core-version-size-right = Version : éloigner le côté droit
action-core-version-size-up = Version : remonter le bas
action-core-version-size-down = Version : descendre le bas
action-core-version-ratio-free = Version : rapport libre
action-core-version-ratio-original = Version : rapport de l'image
action-core-version-ratio-square = Version : 1:1
action-core-version-ratio-5_4 = Version : 5:4 (4:5)
action-core-version-ratio-3_2 = Version : 3:2 (2:3)
action-core-version-ratio-16_9 = Version : 16:9 (9:16)
action-core-version-ratio-4_3 = Version : 4:3 (3:4)
action-core-version-rotate = Version : portrait ou paysage
action-core-version-save = Version : enregistrer
action-core-version-preset = Version : préréglage suivant
action-core-version-preset_prev = Version : préréglage précédent
action-core-version-ratio_next = Version : rapport suivant
action-core-version-ratio_prev = Version : rapport précédent
action-core-version-tiles_next = Version : une colonne de morceaux de plus
action-core-version-tiles_prev = Version : une colonne de morceaux de moins
action-core-version-rows_next = Version : une ligne de morceaux de plus
action-core-version-rows_prev = Version : une ligne de morceaux de moins
action-core-version-ratio_custom = Version : taper un rapport à soi (17:23…)
action-core-version-export_next = Version : préréglage d'export suivant
action-core-version-export_prev = Version : préréglage d'export précédent
action-core-version-name = Version : modifier le nom
action-core-version-save_preset = Version : enregistrer comme préréglage
action-core-version-reset = Version : revenir au cadre d'origine (l'image entière)
action-core-file-delete_stack = Supprimer toute la pile
action-core-file-delete_photo = Supprimer la photo avec ses développements…
action-core-file-copy_stack = Copier toute la pile vers…
stack-actions-need-stacks = Pas de pile ici (piles : { $keys }) : cette action vaut pour toute une pile, ou une photo et ses développements
stack-nothing-to-open = Rien à ouvrir ici
stack-nothing-to-close = Rien à fermer ici
action-plugin-airdrop-send = Envoyer par AirDrop
send-started = { $name } : { $n ->
    [one] { $n } image à envoyer
   *[other] { $n } images à envoyer
}
action-core-file-move = Déplacer vers…
action-core-edit-undo = Annuler la dernière modification
action-core-edit-redo = Rétablir
action-core-edit-history = Historique des modifications
action-core-stack-toggle = Déplier / replier la pile (rafale, sinon les développements de la photo)
action-core-stack-open = Ouvrir le niveau suivant : la pile de rafale, puis les développements
action-core-stack-close = Fermer le niveau ouvert : les développements, puis la pile de rafale
action-core-stack-all = Déplier / replier toutes les piles (rafales et développements)
action-core-stack-settings = Regroupement des images semblables
action-core-stack-mode = Piles : les photos semblables regroupées (en plein écran : image suivante / précédente dans la pile, pile suivante / précédente)
action-core-dev-toggle = Déplier / replier les développements de la photo
action-core-dev-next = Développement suivant de la photo
action-core-dev-fold_all = Replier toutes les piles de développements
action-core-dev-pairs = Regrouper ou séparer RAW et JPEG (paires du dossier)
action-core-dev-prev = Développement précédent de la photo
action-core-stack-choose = Choisir comme couverture de la pile (sa préférée)
action-core-stack-first = Première image de la pile (ou la photo, avant ses développements)
action-core-stack-last = Dernière image de la pile (ou le dernier développement)
action-core-stack-enter = En entrant dans une pile : sa première photo (allumé) ou sa couverture
## Développements d'une photo (darktable, RapidRAW ; 30/10/2026)
dev-count-tip = { $n ->
    [one] { $n } développement
   *[other] { $n } développements
  } · { $has ->
    [yes] clic ou { $keys }
   *[no] clic
  } : déplier / replier
dev-clone = clone { $n }
dev-copy = copie { $n }
dev-none = Cette photo n'a pas de développement (darktable, RapidRAW)
dev-before = Avant : le JPEG boîtier à la place du développement (<core.view.source> : retour)
dev-after = Après : le développement
dev-preferred = { $name } : couverture de la photo
dev-preferred-none = Plus de développement préféré : la pile se replie sur le plus récent
dev-fold-tip = Replier les développements{ $has ->
    [yes] {" "}({ $keys })
   *[no] {""}
  } · Alt+clic : replier et garder celui-ci en couverture
dev-folded-already = Toutes les piles de développements sont repliées
dev-badge-tip = Développement : { $name }
dev-badge-tip-pending = Développement : { $name } (vignette de la photo en attendant son rendu)
view-info-development = avant ({ $name } en cours ou absent)

stack-chosen = { $name } : couverture de sa pile
stack-position = pile : { $n } / { $total }
stack-slider-tip = Sévérité des piles : au centre, la valeur calculée d'après le dossier ; vers la droite plus strict (petites piles), vers la gauche plus souple ; double-clic : retour au centre ; Maj pendant le réglage : piles dépliées ({ $keys } : réglages fins)
blur-slider-tip = Sévérité du flou : vers la droite plus strict (plus de photos floues), vers la gauche plus souple ; au centre, les réglages. Double-clic : retour au centre
clip-slider-over = brûlé
clip-slider-over-tip = Photos brûlées exclues : vers la droite plus strict, vers la gauche plus souple ; tout à gauche, les hautes lumières brûlées n'excluent plus rien. Au centre, les réglages. Double-clic : retour au centre
clip-slider-under = bouché
clip-slider-switch = Curseur : { $side } (clic : passer à l'autre côté)
clip-slider-under-tip = Photos bouchées exclues : vers la droite plus strict, vers la gauche plus souple ; tout à gauche, les noirs bouchés n'excluent plus rien (fonds noirs). Au centre, les réglages. Double-clic : retour au centre
sharpness-strictness = Sévérité de la netteté : { $value } % (0 : les réglages)
stack-icon-tip = Clic droit : réglages fins ({ $keys })
action-core-panel-left = Clavier au panneau de gauche
action-core-panel-right = Clavier au panneau de droite
action-core-panel-next = Clavier au panneau suivant
action-core-folders-up = Dossiers : dossier du dessus
action-core-folders-down = Dossiers : dossier du dessous
action-core-folders-expand = Dossiers : déplier (ou entrer)
summary-title = Ce dossier
summary-photos = Photos
summary-photos-tip = Toutes les photos du dossier : un clic retire tous les filtres.
summary-filter-on = Cliquer : ne montrer que celles-ci (les autres filtres sont retirés)
summary-filter-off = Filtre montré ; cliquer : tout montrer
summary-pairs = Paires RAW+JPEG
summary-pairs-tip = Un RAW et une image plate du même nom, dans le même dossier (le RAW+JPEG du boîtier). Regroupées (menu du dossier, clic droit), la ligne devient un filtre.
summary-developments = Développements
summary-engine-off = { $engine } est absent ou désactivé : ses développements ne sont pas montrés dans la grille ni comptés dans le total du dossier.
summary-untouched = Sans note
summary-starred = Avec des étoiles
summary-accepted = Acceptées
summary-rejected = Rejetées
summary-colored = Avec un libellé couleur
summary-conflicts = Notes en conflit
summary-conflicts-tip = Les notes de ces images diffèrent selon leur source (sidecar, image, autre logiciel) : à trancher avant de les noter.
summary-versions = Avec des versions
summary-cache = Cache : { $size }
summary-cache-clear = Vider
summary-cache-clear-tip = Vider le cache de ce dossier (vignettes, aperçus, analyses ; ils se recalculent), après confirmation
summary-data-folder = Données de tri : dans ce dossier
summary-data-disk = Données de tri : sur le disque ({ $path })
summary-data-machine = Données de tri : sur cette machine
action-core-folders-collapse = Dossiers : replier (ou remonter)
action-core-folders-open = Dossiers : ouvrir le dossier
action-core-folders-first = Dossiers : premier dossier
action-core-folders-last = Dossiers : dernier dossier
action-core-metadata-up = Métadonnées : ligne du dessus
action-core-metadata-down = Métadonnées : ligne du dessous
action-core-metadata-copy = Métadonnées : copier la valeur
action-core-folders-offload = Dossiers : décharger le support amovible
action-core-folders-eject = Dossiers : éjecter le support amovible
action-core-folders-create = Dossiers : nouveau dossier
action-core-folders-rename = Dossiers : renommer le dossier
action-core-folders-delete = Dossiers : supprimer le dossier
action-core-folders-favorite = Dossiers : ajouter aux favoris ou en retirer
action-core-folders-favorite_name = Dossiers : nommer le favori
action-core-folders-connect = Dossiers : se connecter à un partage réseau
action-core-folders-forget = Dossiers : oublier le partage réseau mémorisé
action-core-folders-cache_clear = Dossiers : vider le cache gardé dans le dossier ou le disque
action-core-folders-reset = Dossiers : réinitialiser le dossier ou le disque (cache, données de tri, notes, versions, au choix)
action-core-folders-copy = Dossiers : copier le dossier vers…
action-core-folders-move = Dossiers : déplacer le dossier vers…
action-core-metadata-open = Métadonnées : ouvrir (histogramme en grand)
action-core-metadata-select_up = Métadonnées : étendre la sélection vers le haut
action-core-metadata-select_down = Métadonnées : étendre la sélection vers le bas
action-core-metadata-copy_all = Métadonnées : tout copier
action-core-metadata-section_next = Panneau de droite : section suivante
action-core-metadata-section_prev = Panneau de droite : section précédente
action-core-metadata-activate = Panneau de droite : actionner le réglage pointé
action-core-metadata-left = Panneau de droite : réglage vers la gauche (replier la section)
action-core-metadata-right = Panneau de droite : réglage vers la droite (déplier la section)
action-core-folders-section_next = Dossiers : section suivante
action-core-folders-section_prev = Dossiers : section précédente
action-core-view-folders = Afficher / masquer le panneau des dossiers
action-core-view-refresh_folders = Actualiser les dossiers (disque branché)
action-core-view-histogram = Afficher / masquer l'histogramme
action-core-view-histogram_scale = Histogramme : IL / linéaire
action-core-view-histogram_large = Histogramme en grand
action-core-view-histogram_channel = Grand histogramme : canal suivant
action-core-histogram-red = Grand histogramme : rouge seul
action-core-histogram-green = Grand histogramme : vert seul
action-core-histogram-blue = Grand histogramme : bleu seul
action-core-histogram-photo = Grand histogramme : afficher / masquer la photo d'analyse
action-core-histogram-camera = Grand histogramme : afficher / masquer la courbe du JPEG boîtier
action-core-histogram-after = Histogramme : données réelles / image après les réglages
histogram-photo-tip = Photo d'analyse (zones saturées / bouchées sur l'image en gris) : afficher / masquer ({ $keys })
action-core-histogram-all = Grand histogramme : tous les canaux
action-core-histogram-luminance = Grand histogramme : la luminance seule
action-core-view-clipping = Afficher / masquer les zones brûlées et bouchées
action-core-view-metadata = Afficher / masquer le panneau des métadonnées
action-core-view-open = Ouvrir en plein écran
action-core-view-zoom = Zoom 100 % / ajusté
action-core-view-loupe = Loupe à 100 % sous le pointeur (touche maintenue)
action-core-view-loupe_zoom_in = Zoom, ou loupe (touche de loupe tenue) : palier suivant
action-core-view-loupe_zoom_out = Zoom, ou loupe (touche de loupe tenue) : palier précédent
action-core-view-loupe_scale = Loupe : grossissement tapé en chiffres (2, 20 ou 200 pour 200 %)
action-core-view-zoom_scale = Zoom : pourcentage tapé en chiffres (2, 20 ou 200 pour 200 %)
action-core-view-zoom_in = Zoom : palier supérieur (en grille : vignettes plus grandes)
action-core-view-zoom_out = Zoom : palier inférieur (en grille : vignettes plus petites)
action-core-view-zoom_fit = Zoom : ajusté (en grille et dans les dossiers, / recherche)
action-core-view-zoom_100 = Zoom : 100 % (en grille : toutes les vignettes à l'écran, ou la taille d'avant)
view-info-zoom = { " · " }{ $percent } %
view-loupe-loading = 100 % en cours…
action-core-view-source = Rendu affiché suivant (vignettes et plein écran) : JPEG boîtier, couverture, un logiciel, rendu RAW ; sur un développement passé en revue : avant / après
action-core-view-pan_left = Image zoomée : aller à gauche
action-core-view-pan_right = Image zoomée : aller à droite
action-core-view-pan_up = Image zoomée : aller en haut
action-core-view-pan_down = Image zoomée : aller en bas
action-core-view-back = Fermer / revenir
action-core-app-help = Afficher ou masquer cette aide
action-core-app-help_here = Aide ici : les raccourcis de l'endroit où l'on est
action-core-view-review_developments = Passer en revue les développements : oui / non
action-core-file-allow_actions = Actions sur les fichiers (renommer, copier, déplacer, supprimer) : autorisées / interdites
action-core-app-palette = Palette de commandes : chercher une action ou un réglage et le lancer
action-core-app-settings = Réglages
action-core-app-check_update = Rechercher une mise à jour
action-core-adjust-before_after = Réglages : avant / après (images sans leurs réglages, ou avec)
action-core-adjust-apply = Réglages : harmoniser la sélection (exposition et balance alignées sur l'image courante)
action-core-adjust-copy = Réglages : copier ceux de l'image
action-core-adjust-paste = Réglages : coller (exposition et balance harmonisées, style et noir et blanc copiés)
action-core-app-log = Journal de l'application (erreurs et leur détail)
action-core-app-report_issue = Signaler un problème (formulaire de GitHub prérempli)
action-core-app-quit = Quitter
action-core-rate-star-0 = Retirer les étoiles
action-core-rate-star-1 = 1 étoile
action-core-rate-star-2 = 2 étoiles
action-core-rate-star-3 = 3 étoiles
action-core-rate-star-4 = 4 étoiles
action-core-rate-star-5 = 5 étoiles
action-core-rate-color-red = Couleur rouge
action-core-rate-color-yellow = Couleur jaune
action-core-rate-color-green = Couleur verte
action-core-rate-color-blue = Couleur bleue
action-core-rate-color-purple = Couleur violette
action-core-flag-accept = Accepter
action-core-flag-reject = Rejeter
action-core-flag-none = Ni acceptée ni rejetée

## Thèmes de l'aide

group-navigation = Navigation
group-selection = Sélection
group-view = Affichage
group-rating = Notation
group-filters = Filtres
group-panels = Panneaux
group-files = Fichiers
group-edit = Annuler
group-app = Application
group-crop = Versions
place-grid = Grille
place-view = Plein écran
place-compare = Comparaison
place-crop = Version
place-histogram = Grand histogramme
place-folders = Panneau des dossiers
place-metadata = Panneau des métadonnées
help-here-search-hint = Tapez pour filtrer…
help-here-title = Raccourcis ici : { $place }
help-here-footer = { $help } : toutes les actions, avec recherche · { $leader } = { $keys } (une pause après une touche montre la suite) · Échap ou clic : fermer
palette-title = Palette de commandes
palette-hint = Une action ou un réglage…  ({ $actions } actions seulement, { $settings } réglages seulement)
palette-recent = Lancées récemment
palette-empty = Rien trouvé
palette-action = action
palette-control = réglage
palette-setting = configuration
palette-keys = Flèches haut / bas : choisir · gauche / droite : changer un contrôle · Entrée : lancer · Échap : fermer
kw-group-navigation = naviguer aller déplacer suivante précédente parcourir
kw-group-selection = sélectionner choisir plusieurs multiple
kw-group-view = afficher voir vue affichage écran
kw-group-rating = noter note notes notation étoiles couleurs drapeau étiquette
kw-group-filters = filtrer trier masquer cacher montrer
kw-group-panels = panneau barre afficher masquer
kw-group-files = fichier fichiers gérer
kw-group-edit = annuler rétablir historique erreur
kw-group-crop = version versions recadrer recadrage cadrer cadre
kw-group-app = application programme
version-crop-alias = Recadrer
crop-bar-title = Version
crop-ratio-free = Libre
crop-ratio-original = Original
crop-turn-portrait = Passer en portrait
crop-turn-landscape = Passer en paysage
crop-turn-none = Format libre ou carré : pas d’orientation
crop-tiles-title = Morceaux
crop-tiles-tip = Colonnes de morceaux : le cadre coupé en morceaux égaux, chacun au rapport choisi, un fichier par morceau à l'export (3 × 1 : un carrousel Instagram) ; <core.version.tiles_next> / <core.version.tiles_prev> : une de plus / de moins
crop-rows-tip = Lignes de morceaux (1 × 3 : une bande verticale ; 3 × 3 : une grille) ; <core.version.rows_next> / <core.version.rows_prev> : une de plus / de moins
crop-section-frame = Recadrage
crop-ratio-original-tip = Le rapport de l'image ; Ctrl+clic (Cmd+clic sur Mac) : tout le cadre d'origine
crop-ratio-custom = Personnalisé
crop-ratio-custom-bad = Un rapport comme 17:23, 4/5 ou 1.91:1 ; Entrée l'applique
crop-ratio-add-tip = Ajouter ce rapport à la liste (gardé dans la configuration : [versions] ratios)
crop-ratio-added = rapport { $ratio } ajouté à la liste
crop-name = Nom
crop-profile = Export
crop-profile-none = Aucun
crop-save = Enregistrer
crop-saved = Version « { $name } » enregistrée
crop-name-taken = Ce nom est déjà pris (une autre version, ou « Original » : l’image non recadrée)
crop-name-original-ratio = Version
crop-name-taken-notice = Version non enregistrée : le nom « { $name } » est déjà pris
crop-failed = Version non enregistrée : { $error }
crop-preset-none = Préréglage
crop-preset-tip = Préréglage de version : rapport, nom et préréglage d'export (<core.version.preset> : suivant)
crop-preset-save = Enregistrer comme préréglage de version
crop-preset-save-tip = Enregistrer comme préréglage de version : rapport, nom et export du cadre en cours, gardés dans les réglages
crop-preset-saved = Préréglage « { $name } » enregistré
preset-story = Story
preset-post = Publication
preset-carousel = Carrousel
versions-current = Versions : { $name }
versions-original = Original
versions-edit = Modifier
versions-delete = Supprimer
versions-deleted = Version « { $name } » supprimée

## Panneaux, éjection, notes

panel-no-image = Aucune image sélectionnée.
histogram-enlarge = Agrandir ({ $keys })
histogram-camera-tip-short = Trait gris : JPEG boîtier
histogram-camera-row = JPEG boîtier
histogram-camera-row-tip = Trait gris : le JPEG boîtier, mesuré sous son propre blanc (255) et à sa propre hauteur, pour voir ce que le boîtier a fait des hautes lumières et des ombres. Chiffres : { $channel }.
histogram-camera-button-tip = Courbe du JPEG boîtier : afficher / masquer ({ $keys })
histogram-after-button-tip = Données réelles / image après les réglages ({ $keys }) : histogramme, pourcentages et zones
histogram-after-label = après réglages
histogram-after-sensor = capteur : { $percent } saturés
histogram-computing = histogramme en cours de calcul
panel-name = Nom
panel-reading = lecture…
folders-title = Dossiers
folders-section-devices = Périphériques
folders-section-drives = Disques
folders-section-network = Réseau
folders-section-personal = Dossiers personnels
folders-section-favorites = Favoris
folders-section-system = Système
folders-refresh = Actualiser
folders-refresh-tip = Relire les volumes et les dossiers. Un disque branché apparaît aussi de lui-même en quelques secondes.
folders-subfolders = Sous-dossiers
folders-subfolders-tip = Afficher aussi les images des sous-dossiers ({ $n } niveaux au plus, [gui.folders] subfolders_depth)
folders-collapse = Replier
folders-expand = Déplier
folders-unreadable = illisible
folders-fixed-drive = Disque fixé ([places] fixed) : pas d'éjection depuis tri
folders-offload = Décharger la carte…
eject = Éjecter
cancel = Annuler
eject-question = Éjecter « { $name } » ?
eject-open-folder = Le dossier ouvert est sur ce volume : il sera fermé.
ejecting = Éjection de « { $name } »…
ejected = « { $name } » éjecté : vous pouvez le retirer.
status-clear-tip = Cliquer pour effacer le message
status-error-tip = Cliquer pour voir le détail dans le journal (et effacer le message)
panel-section-adjust = Réglages
adjust-exposure = Exposition
adjust-warmth = Chaleur
adjust-tint = Teinte
adjust-histogram-after = Histogramme après réglages
adjust-histogram-after-tip = Une image réglée montre son histogramme, ses pourcentages saturés / bouchés et ses zones après ses réglages, dès le premier réglage (sinon : données réelles). Le même réglage que le bouton du grand histogramme. Retenu dans la configuration ([gui.histogram] after_auto).
adjust-sync = Synchroniser { $n } images
adjust-sync-none = Synchroniser la sélection
adjust-sync-tip = Plusieurs images sélectionnées : un curseur décale chacune d'autant (exposition, teinte : même écart ; température : même rapport), un bouton (auto, préréglage) recalcule chacune, le noir et blanc est copié
adjust-tab-base = Base
adjust-tab-style = Style
adjust-tab-mono = N&B
adjust-tab-values = Valeurs
adjust-filter = Filtre
adjust-mono-custom = Personnalisé
adjust-mono-none = Aucun (couleur)
adjust-copy = Copier les réglages
adjust-copy-tip = Pour les coller sur d'autres images, même dans un autre dossier
adjust-paste = Coller les réglages
adjust-paste-tip = Sur cette image (et la sélection synchronisée) : exposition et balance harmonisées sur l'image copiée, style et noir et blanc copiés
adjust-harmonise = Harmoniser la sélection
adjust-harmonise-tip = L'exposition et la balance des images sélectionnées alignées sur cette image, sans toucher au style ni au noir et blanc
adjust-picked = Pipette : balance { $kelvin } K, teinte { $tint }
adjust-folder-running = réglages automatiques du dossier en cours…
adjust-copied = réglages de { $name } copiés
adjust-paste-empty = rien à coller : copiez d'abord les réglages d'une image
adjust-missing = { $missing ->
    [one] une image sans vignette prête, laissée de côté
   *[other] { $missing } images sans vignette prête, laissées de côté
}
adjust-copy-all = Tout copier
adjust-value-exposure = Exposition totale
adjust-value-correction = Base
adjust-value-creative = Style
adjust-value-balance = Balance
adjust-value-shot = Prise de vue
adjust-value-creative-balance = Style : balance
adjust-value-warmth-tint = chaleur { $warmth }, teinte { $tint }
adjust-value-mono = Noir et blanc (R / V / B)
adjust-temperature = Température
adjust-creative-tint = Teinte
adjust-contrast = Contraste
adjust-saturation = Saturation
adjust-value-contrast-saturation = Contraste / saturation
adjust-auto = Exposition auto
adjust-auto-exposure-tip = Amène la luminance médiane de l'image au gris moyen ([adjust] auto_exposure_target) ; sur une sélection synchronisée, chaque image d'après la sienne
adjust-wb-tip = Balance des blancs : celle de la prise de vue, auto (moyenne de toute l'image) ou un préréglage ; les kelvins se tapent aussi dans la valeur
adjust-wb-custom = Personnalisée
adjust-wb-as-shot = Telle que prise (boîtier)
adjust-wb-auto = Auto (toute l'image)
adjust-wb-daylight = Lumière du jour ({ $kelvin } K)
adjust-wb-cloudy = Nuageux ({ $kelvin } K)
adjust-wb-shade = Ombre ({ $kelvin } K)
adjust-wb-tungsten = Tungstène ({ $kelvin } K)
adjust-wb-fluorescent = Fluorescent ({ $kelvin } K)
adjust-wb-flash = Flash ({ $kelvin } K)
adjust-mono-normalise = Normalisé
adjust-mono-normalise-tip = Mélange ramené à 100 % : la luminosité ne change pas quand on déplace un curseur
adjust-mono-total = total { $total } %
adjust-type-value = Cliquer pour taper une valeur, glisser pour la changer
adjust-ev-suffix = { " IL" }
adjust-eyedropper = Pipette
adjust-eyedropper-tip = Puis, sur l'image en plein écran : cliquer sur une zone qui devrait être grise, ou tracer un cadre pour une balance auto sur cette partie de l'image
adjust-mono-red = Rouge
adjust-mono-orange = Orange
adjust-mono-yellow = Jaune
adjust-mono-green = Vert
adjust-mono-blue = Bleu
adjust-red = Rouge
adjust-green = Vert
adjust-blue = Bleu
adjust-before = Avant
adjust-before-tip = Montrer les images sans leurs réglages (bascule)
adjust-reset = Réinitialiser
adjust-reset-tip = Retirer tous les réglages (de la sélection synchronisée aussi)
adjust-developed = Image développée par un autre logiciel : les réglages valent avant le développement.
adjust-write-failed = réglages de { $name } non enregistrés : { $error }
adjust-apply-none = sélectionnez d'autres images pour leur appliquer ces réglages
adjust-apply-wait = vignette pas encore prête, réessayez dans un instant
adjust-applied = { $n ->
    [one] réglages appliqués à 1 image
   *[other] réglages appliqués à { $n } images
}
adjust-applied-missing = réglages appliqués à { $n } images ; { $missing } sans vignette prête, laissées de côté
log-title = Journal de l'application
log-search-hint = Chercher dans le journal…
log-shown-errors = Erreurs
log-shown-warnings = Avertissements et erreurs
log-shown-all = Tout
log-refresh = Actualiser
log-inactive = Journal inactif : pas de dossier de données.
log-empty = Le journal est vide.
log-no-match = Aucune entrée ne correspond.
log-truncated = Les { $n } entrées les plus récentes sont montrées : cherchez pour remonter plus loin.
log-entry-tip = Cliquer pour voir ou masquer le détail
log-entry-copy-tip = Cliquer pour le bouton Copier
log-copy = Copier
log-copy-tip = Copier cette entrée et son détail
log-copy-all = Tout copier
log-copy-all-tip = Copier les entrées montrées, avec leur détail
report-title = Signaler un problème
report-log-intro = Les dernières entrées du journal aideront à comprendre le problème. Elles seront jointes au formulaire de GitHub.
report-log-privacy = Relisez-les avant d'envoyer : les chemins contiennent les noms de vos dossiers. Modifiez ou effacez ce que vous voulez.
report-log-empty = (rien au journal)
report-with-log = Ouvrir le formulaire avec ces lignes
report-without-log = Sans le journal
mounting = Montage de « { $name } »…
mounted = « { $name } » monté.
mount-failed = « { $name } » n'a pas pu être monté : { $error }
folders-device-tip = Pas monté : cliquer pour le monter.
folders-server-tip = Partage réseau mémorisé, pas connecté ({ $address }) : cliquer pour s'y connecter.
folders-neighbour-tip = Serveur du réseau ({ $address }) : un clic montre ses partages, ou se connecte.
folders-connect-tip = Taper l'adresse d'un serveur (\\nas\photo, smb://nas/photo, ftp://nas…) ; le mot de passe est demandé s'il le faut, et le serveur reste ensuite dans « Réseau ».
connecting = Connexion à « { $name } »…
connected = Connecté à « { $name } ».
connect-failed = Connexion à « { $name } » impossible : { $error }
folder-opening = Ouverture de « { $name } »…
folder-opening-found = { $count ->
    [0] Aucune image trouvée pour l'instant
    [one] 1 image trouvée
   *[other] { $count } images trouvées
}
folder-opening-why = Dossier lent (réseau, disque) : sa liste se lit en fond.
stacks-waiting = Regroupement des piles…
stacks-waiting-count = { $done } / { $total }
folder-open-cancelled = Ouverture de « { $name } » annulée.
login-title = Se connecter à « { $name } »
login-why = Le serveur { $address } demande un compte.
login-refused = Utilisateur ou mot de passe refusé : vérifiez-les et réessayez.
login-user = Utilisateur
login-domain = Domaine
login-domain-hint = WORKGROUP (le plus souvent)
login-password = Mot de passe
login-remember = Retenir le mot de passe
login-connect = Se connecter
login-cancelled = Connexion à « { $name } » abandonnée.
connected-not-saved = Connecté à « { $name } », mais le mot de passe n'a pas pu être retenu : { $error }
server-forgotten = Partage « { $name } » oublié.
eject-failed = « { $name } » non éjecté : { $error }
cache-unavailable = cache inaccessible : { $error }

## Comparaison, plein écran, barre d'état

rating-not-saved = note non enregistrée : { $error }
compare-needs-two = comparer : sélectionnez au moins 2 images ({ $keys }, Maj+clic), ou placez-vous sur une pile ou une photo développée
compare-around = comparaison des { $max } images autour de l'image courante ([gui.compare] max) ; sélectionnez-en pour choisir
gpu-error-budget = mémoire graphique insuffisante ({ $error }) : limite ramenée à { $mb } Mo pour cette session (réglage : [gui.view] textures_mb)
resampled = Développement agrandi
resampled-tip = Ce développement a été rendu en { $render } pixels ; il est agrandi à l'échelle de la photo ({ $full }) pour la comparer au même endroit. Sa netteté à cette échelle n'est donc pas celle du fichier : pour en juger, regardez le rendu RAW ou le JPEG boîtier.
compare-no-full = Image en basse définition (mémoire graphique pleine)
compare-no-full-tip = Cette image n'a pas pu être chargée en pleine définition : la mémoire graphique est pleine ({ $mb } Mo, [gui.view] textures_mb). Elle est affichée agrandie depuis un aperçu plus petit, donc moins nette que les autres.
textures-keep = Garder { $mb } Mo
textures-keep-tip = Écrire cette limite dans la configuration ([gui.view] textures_mb) : elle vaudra aussi aux prochains lancements
textures-kept = limite de mémoire graphique enregistrée : { $mb } Mo ([gui.view] textures_mb)
textures-over-budget = mémoire graphique pleine ({ $mb } Mo, [gui.view] textures_mb) : certaines images gardent leur aperçu au lieu du 1:1
compare-limit-mark = Comparaison limitée à { $max } images
compare-limited = comparaison limitée aux { $max } premières ([gui.compare] max)
view-info-selected = { " " }· sélectionnée
view-info-peaking = { " " }· netteté
view-info-peaking-computing = { " " }· netteté en calcul
view-info-sensor = { " " }({ $percent } % du capteur)
view-info-resolution = { " · " }{ $width } × { $height }
view-info-preview-computing = { " " }· aperçu en cours de calcul
view-info-raw-pending = { " " }· rendu RAW en cours
view-info-full-computing = { " " }· calcul du 100 % en cours
view-info-one-to-one-loading = { " " }· chargement du 1:1
view-hint-raw = <core.view.source> : rendu   <core.view.zoom> : 1:1   <core.view.histogram> : histogramme   <core.view.clipping> : zones   <core.view.back> : grille
view-hint-flat = <core.view.zoom> : 1:1   <core.view.histogram> : histogramme   <core.view.clipping> : zones   <core.view.back> : grille
status-no-folder = aucun dossier
status-images = { $n ->
    [one] { $n } image
   *[other] { $n } images
} ({ $raws } RAW{ $devs ->
    [0] { "" }
   *[other] , { $devs } dév.
}) · filtre : { $filter }
status-selected = { $n ->
    [one] { $n } sélectionnée
   *[other] { $n } sélectionnées
}
status-visual-mode = { " " }· mode visuel
progress-notes = notes
progress-thumbnails = vignettes
progress-embedded = rendu boîtier
progress-previews = aperçus
progress-histograms = histogrammes
status-key-problems = { $n ->
    [one] raccourcis : { $n } problème
   *[other] raccourcis : { $n } problèmes
}
status-invalid-config = configuration invalide : réglages par défaut
status-show-offload = Afficher le déchargement
tasks-global = { $n ->
        [one] 1 tâche en cours
       *[other] { $n } tâches en cours
    } · { $percent } %
tasks-unfold = Cliquer : le détail
tasks-fold = Cliquer : replier le détail
status-retry-eject = Réessayer l'éjection
status-dismiss = ignorer
status-open-session = Ouvrir la séance « { $name } »

## Boîtes, bandeau, filtres, aide

close = Fermer
close-tip = Fermer (Échap)
histogram-scale-ev = en IL
histogram-scale-linear = linéaire
histogram-scale-tip = Échelle { $current } ; clic ou { $keys } : { $other }
or-escape = ou Échap
grid-fit = Toutes les vignettes à l'écran
grid-fit-back = Un second clic : la taille d'avant
grid-size = Taille des vignettes
files-with-pairs = Avec l'autre fichier de la paire RAW+JPEG
files-with-pairs-tip = Décocher pour n'agir que sur les images choisies (ex. supprimer les JPEG en gardant les RAW)
help-copy-version = Copier la version et le système (pour un signalement)
help-version-copied = Version et système copiés
help-report = Signaler un problème
help-log = Journal
help-report-tip = Ouvre le formulaire de GitHub, prérempli avec la version, le système et les dernières lignes du journal (à relire avant)
update-tip = Interroge la page Releases du projet (rien n'est envoyé sans ce clic)
update-title = Mise à jour
update-checking = Recherche en cours…
update-latest = Vous avez la dernière version ({ $version }).
update-available = Version { $version } disponible (vous avez la { $current }).
update-open-page = Ouvrir la page de téléchargement
download-nightly = Télécharger le build de nuit
download-nightly-tip = Le build de nuit : l'état du jour, avec les dernières corrections et nouveautés, testé automatiquement mais pas encore publié. Pour essayer ce qui vient d'arriver.
download-release = { $stage ->
    [alpha] Télécharger la dernière alpha
    [beta] Télécharger la dernière bêta
   *[other] Télécharger la dernière version
}
download-release-tip = { $stage ->
    [alpha] La dernière alpha publiée : une version numérotée, avec ses notes, plus posée que le build de nuit mais moins récente.
    [beta] La dernière bêta publiée : une version numérotée, avec ses notes, plus posée que le build de nuit mais moins récente.
   *[other] La dernière version publiée, avec ses notes : plus posée que le build de nuit, mais moins récente.
}
download-searching = Recherche du paquet à télécharger…
download-started = Téléchargement dans le navigateur : { $name }
download-page = Pas de paquet pour ce système : page de la version { $version } ouverte
download-title = Télécharger tri { $version }
download-choose = Plusieurs paquets pour ce système :
download = Télécharger
download-format = { $format ->
    [appimage] Un seul fichier à lancer, sans installation, pour toutes les distributions
    [deb] Paquet Debian, Ubuntu, Linux Mint : installé par le système
    [targz] Archive avec son script d'installation (install.sh)
    [msi] Installeur Windows
    [zip] Archive à décompresser, sans installation
    [dmg] Image disque : glisser tri dans Applications
   *[other] Paquet de ce système
}
rating-bar-title = Note
filter-bar-title = Filtre
rating-bar-applies = agit sur les { $n } images sélectionnées
filter-stack-none = { $keys } : placez-vous sur une pile (piles actives)
stack-enter-cover = En entrant dans une pile : sa couverture
stack-enter-first = En entrant dans une pile : sa première photo
stack-enter-needs-stacks = { $now } (en plein écran, piles actives : { $keys })
filter-summary-stack = pile de { $n }
filter-summary-stars = { $n ->
    [5] 5 étoiles
    [one] { $n } étoile et plus
   *[other] { $n } étoiles et plus
}
filter-summary-stars-exactly = { $n ->
    [one] exactement { $n } étoile
   *[other] exactement { $n } étoiles
}
filter-summary-stars-at-most = { $n ->
    [one] { $n } étoile ou moins
   *[other] { $n } étoiles ou moins
}
stars-op-at-least = Filtre d'étoiles : au moins
stars-op-exactly = Filtre d'étoiles : exactement
stars-op-at-most = Filtre d'étoiles : au plus
filter-summary-or = { " ou " }
filter-summary-and = { " et " }
filter-colors-any-tip = L'une des couleurs choisies (ou)
filter-colors-all-tip = Toutes les couleurs choisies (et)
filter-all = tout
filter-flat = JPEG…
filter-bar-flat = JPEG…
filter-bar-raw-tip = RAW : montrés (allumé) ou masqués
filter-bar-flat-tip = JPEG et autres images (TIFF, PNG, HEIC…) : montrés (allumé) ou masqués
filter-bar-clear = Effacer
filter-bar-count = { $shown }/{ $n }
badge-over = { $percent } % de pixels saturés (brûlée) ; clic : grand histogramme
badge-under = { $percent } % de pixels bouchés ; clic : grand histogramme
badge-versions = { $n ->
    [one] 1 version ; clic : la modifier
   *[other] { $n } versions ; clic : les voir, en choisir une à modifier
}
help-title = Raccourcis
help-search-hint = Rechercher (ex. « pnd », « etoile 3 »)…
help-commands = Ligne de commande
help-commands-group = Ligne de commande : { $theme }
help-command-copy = Copier la commande
help-command-example = Voir un exemple à copier
help-command-copied = Commande copiée dans le presse-papier
help-no-match = Aucune action ne correspond.
help-close-hint = Échap ou <core.app.help> pour fermer.
empty-folder-gone = Le dossier « { $name } » n'est plus là (disque débranché ?)
empty-folder-gone-back = Il se rouvrira de lui-même quand le disque reviendra.
empty-no-folder = Choisissez un dossier dans le panneau de gauche ({ $keys } l'affiche).
empty-folder = Aucune image dans ce dossier.
filter-toast = { $n ->
    [one] 1 image
   *[other] { $n } images
}
filter-toast-none = Aucune image
develop-toast = { $n ->
    [one] 1 développement modifié à refaire ({ $done }/1)
   *[other] { $n } développements modifiés à refaire ({ $done }/{ $n })
}
develop-toast-done = { $n ->
    [one] 1 développement refait
   *[other] { $n } développements refaits
}
empty-filtered = Aucune image pour ces filtres (<core.filter.clear> pour les effacer).
color-red = rouge
color-yellow = jaune
color-green = vert
color-blue = bleu
color-purple = violet

## Filtres sur le drapeau

flag-filter-accepted = acceptées
flag-filter-not-rejected = sans les rejetées
flag-filter-rejected = rejetées

## Renommer, corbeille

rename-title = Renommer « { $name } »
rename-becomes = { $from } devient { $to }
rename-copied = { $from } copié en { $to } (partagé avec la paire)
rename-button = Renommer
enter-or-escape = Entrée / Échap
export-title = { $n ->
    [one] Exporter 1 image
   *[other] Exporter { $n } images
}
export-title-version = Exporter la version « { $name } »
export-contents-developed = { $n ->
    [one] 1 RAW, développé par { $engine }
   *[other] { $n } RAW, développés par { $engine }
}
export-contents-libraw = { $n ->
    [one] 1 RAW, rendu de base (LibRaw)
   *[other] { $n } RAW, rendu de base (LibRaw)
}
export-contents-embedded = { $n ->
    [one] le rendu boîtier de 1 RAW
   *[other] le rendu boîtier de { $n } RAW
}
export-contents-flat = { $n ->
    [one] 1 image déjà développée
   *[other] { $n } images déjà développées
}
export-contents-paired = { $n ->
    [one] 1 JPEG de l'appareil (paire, nom suffixé)
   *[other] { $n } JPEG de l'appareil (paires, noms suffixés)
}
export-contents-tiles = un fichier par morceau ({ $n } par image, numérotés _1, _2…)
export-profile = Préréglage d'export
export-profile-tip = Flèches haut / bas : préréglage · Tab : dossier · Entrée : exporter
export-destination = Dossier
export-create = sera créé
export-goes-to = Destination : { $dest }
export-no-developer = Aucun logiciel de développement trouvé : rendu de base par LibRaw.
export-rapidraw-srgb = RapidRAW rend en sRGB : l'espace « { $color } » est converti depuis sRGB.
export-passthrough = Sans recompression
export-passthrough-tip = JPEG d'origine copié tel quel, rendu boîtier écrit tel quel (orientation dans l'EXIF). Une image à réduire (taille du préréglage) est réencodée.
export-button = Exporter
export-profile-edit = Retouche
export-profile-asis = JPEG tel quel
export-profile-web = Web
export-format-jpeg = JPEG { $quality } %
export-format-tiff = TIFF { $bits } bits
export-format-png = PNG { $bits } bits
export-color-image = espace d'origine
export-color-linear = { $name } (linéaire)
export-sharpen-screen = accentuation écran
export-sharpen-print = accentuation tirage
export-size-full = pleine définition
export-size-max = { $side } px au plus
export-busy = Un export est déjà en cours.
export-done = { $n ->
    [one] 1 image exportée dans { $dest }
   *[other] { $n } images exportées dans { $dest }
}
export-done-passthrough = { " " }(dont { $n } sans recompression)
export-partial = { $n } exportées, { $failed } en échec (illisibles par Darktable ?), dans { $dest }
export-cancelled = Export arrêté (les fichiers déjà écrits restent).
export-toast-done = { $n ->
    [one] 1 image exportée
   *[other] { $n } images exportées
}
export-toast-partial = { $n } exportées, { $failed } en échec
export-toast-open-folder = Ouvrir le dossier
export-toast-report = Voir le rapport
export-toast-close = Fermer
export-progress = Export « { $profile } » { $done }/{ $total }
export-cancel = Arrêter l'export
export-report-title = Rapport d'export
export-report-empty = Aucun export pour l'instant.
export-report-version = version « { $name } »
export-report-summary = { $total ->
    [one] { $written } fichier sur 1 image
   *[other] { $written } fichiers sur { $total } images
}
export-report-passthrough = (dont { $n } sans recompression)
export-report-now = à l'instant
export-report-minutes = il y a { $n } min
export-report-hours = il y a { $n } h
export-report-days = { $n ->
    [one] il y a 1 jour
   *[other] il y a { $n } jours
}
export-report-months = il y a { $n } mois
export-report-years = { $n ->
    [one] il y a 1 an
   *[other] il y a { $n } ans
}
export-report-files = { $n ->
    [one] 1 fichier
   *[other] { $n } fichiers
}
export-report-errors = { $n ->
    [one] 1 échec
   *[other] { $n } échecs
}
export-report-clear = Effacer la liste
export-report-open = Rapport d'export
rename-failed = renommage impossible : { $error }
transfer-title = { $n ->
    [one] { $n } image vers…
   *[other] { $n } images vers…
}
transfer-files = { $n ->
    [one] { $n } fichier (sidecars XMP compris)
   *[other] { $n } fichiers (sidecars XMP compris)
}
transfer-shared = { $name } est partagé avec une image qui reste : copié
transfer-create = le dossier sera créé
transfer-copy = Copier
transfer-move = Déplacer
transfer-confirm = Valider
transfer-hint = Tab : compléter · flèches haut / bas : destinations récentes
transfer-recent = Récentes
transfer-bring-cache = Emporter le cache
transfer-bring-cache-tip = Vignettes, aperçus et analyses suivent le dossier : rien à recalculer
transfer-progress = { $done } / { $total } fichiers
transfer-background = Continuer en arrière-plan
transfer-status = copie vers « { $name } » { $done }/{ $total }
transfer-done = Copie / déplacement terminé
status-show-transfer = Afficher la copie
transfer-failed = copie / déplacement impossible : { $error }
transfer-folder-title = Dossier « { $name } » vers…
drag-images = { $n } images
drag-move = déplacer (⌥ : copier)
drag-move-ctrl = déplacer (Ctrl : copier)
drag-copy = copier
groups-title = Regroupement des images semblables
groups-enabled = Regrouper en piles
groups-threshold = Sévérité (au centre : calculée d'après le dossier)
groups-threshold-tip = Vers la droite : seulement les rafales ; vers la gauche : des piles plus grandes.
groups-threshold-value = Seuil : { $threshold } (centre du dossier : { $centre })
groups-curve = Progression du curseur
groups-curve-tip = Comment la sévérité avance. Vers la gauche, chaque pixel retire la même part des piles (il en faut de plus en plus pour une pile de moins) ; au-delà de 1, le départ du centre est plus doux, et la droite plus rapide. À fond à gauche, tout est regroupé.
groups-pauses = Importance des pauses
groups-pauses-less = contenu seul
groups-pauses-more = pauses
groups-pauses-tip = Une pause plus longue que la cadence éloigne deux images. À gauche, en studio : le contenu compte d'abord ; à droite, en photo de rue : une pause marque un autre lieu.
groups-gap = Écart de temps maximal
groups-gap-tip = Deux images prises à plus d'intervalle ne sont jamais groupées (0 : sans limite).
groups-summary = { $images } images : { $stacks } piles, { $groups } groupes en tout
groups-computing = empreintes : { $done } / { $total }
groups-default = Retour au centre
history-title = Historique des modifications
history-start = État au lancement
history-empty = Aucune modification pendant cette session.
history-undo = Annuler
history-redo = Rétablir
history-undone = Annulé : { $label }
history-redone = Rétabli : { $label }
history-refused = refusé : { $error }
focus-folders = clavier : dossiers (Échap : images)
focus-metadata = clavier : métadonnées (Échap : images)
metadata-copied = { $label } copié dans le presse-papiers
metadata-copied-lines = { $n } lignes copiées dans le presse-papiers
metadata-menu-copy = Copier la valeur
metadata-menu-copy-selection = Copier la sélection ({ $n } lignes)
metadata-menu-copy-all = Tout copier
metadata-select-all = Tout sélectionner
metadata-copy-line = { $label } : { $value }
folders-not-removable = « { $name } » n'est pas un support amovible
offload-source = Carte :
folders-no-removable = aucun support amovible à décharger
settings-title = Réglages
settings-search-hint = Rechercher un réglage (nom, section, description)…
settings-file = fichier : { $path }
settings-bold = (en gras : réglé dans ce fichier)
settings-in-file = Réglé dans votre fichier de configuration : « défaut » le retire.
settings-save-failed = enregistrement de { $path } impossible : { $error }
settings-reset = défaut
settings-color-order-sorted = Tri
settings-color-order-available = Disponibles
settings-color-sources-none = Aucun champ de couleur géré ([xmp.fields])
settings-fields-head-managed = Géré
settings-fields-head-programs = Logiciels
settings-fields-head-fields = Champs XMP
settings-fields-head-for = Pour
settings-fields-label = Lightroom, XnView, Windows…
settings-fields-darktable = darktable
settings-fields-digikam = digiKam
settings-fields-lightroom = Lightroom Classic
settings-fields-all = Tous les logiciels
settings-fields-for-color = couleur
settings-fields-for-colors = couleurs
settings-fields-for-color-flag = couleur, accepté / rejeté
settings-fields-for-flag = accepté / rejeté
settings-fields-for-stars = étoiles, rejeté
settings-color-source-darktable = darktable
settings-color-source-label = Lightroom, Capture One, XnView…
settings-color-source-digikam = digiKam
settings-color-source-digikam-hint = digiKam:ColorLabel
settings-color-source-label-hint = xmp:Label
settings-color-source-darktable-hint-several = darktable:colorlabels, toujours lu en premier
settings-color-source-darktable-hint = darktable:colorlabels, s'il n'a qu'une couleur
settings-keys-doc = Raccourcis de chaque action : ceux du jeu choisi ([gui] profile), ou ceux écrits ici, qui les remplacent pour cette action (en gras). Un clic sur un raccourci le retire ; « enregistrer… » ajoute la prochaine touche (ou séquence) tapée ; le champ accepte un raccourci écrit (« Maj+P », « g g », « ° »). Les conflits sont signalés.
loupe-key-unknown = touche de loupe inconnue : « { $key } »
loupe-key-taken = Touche de loupe « { $key } » refusée : elle sert déjà à une action (changez l'une ou l'autre : [gui.zoom] loupe_key, [keys])
settings-keys-none = aucun raccourci
settings-keys-remove = Retirer « { $keys } »
settings-keys-record = enregistrer…
settings-keys-record-tip = Tape le raccourci (une séquence : touche après touche, Échap compris) ; il est ajouté après le délai des séquences sans nouvelle touche ([gui] key_sequence_timeout_ms).
settings-keys-recording = tape le raccourci… { $keys }
settings-keys-type-hint = ou taper : g g
settings-keys-default = Revenir aux raccourcis du jeu choisi
settings-default = valeur par défaut : { $value }
settings-restart = au prochain lancement
settings-section-restart = Réglages du moteur : appliqués au prochain lancement.
settings-no-path = emplacement du fichier de configuration inconnu (--config FICHIER)
settings-section-preview = Aperçus
settings-section-render = Rendu RAW
settings-section-cache = Cache
cache-row-thumbnails = Vignettes
cache-row-thumbnails-tip = Vignettes de la grille : vite recalculées (aperçus embarqués)
cache-row-embedded = Aperçus boîtier
cache-row-embedded-tip = JPEG des boîtiers réduits à la taille de l'écran, montrés en plein écran
cache-row-standard = Aperçus RAW
cache-row-standard-tip = Rendus RAW et images plates à la taille de l'écran
cache-row-full = 1:1
cache-row-full-tip = Zooms à 100 % : JPEG boîtier à sa taille et rendus RAW complets ; les plus gros, recalculés à la demande
cache-row-developed = Développés
cache-row-developed-tip = Rendus de darktable, RawTherapee, ART et RapidRAW ; à vider après une mise à jour de l'un d'eux
cache-row-analysis = Analyses
cache-row-analysis-tip = Histogrammes, netteté et empreintes des piles : petits, mais longs à recalculer
cache-total = Total
cache-clear = Vider
cache-clear-all = Tout vider
cache-clear-confirm = Vider
cache-clear-cancel = Annuler
cache-clearing = Vidage en cours…
cache-freed = { $size } libérés
cache-clear-failed = Vidage impossible : { $error }
cache-general-only = Le cache général : celui gardé avec les photos (.tri/cache) suit son dossier.
cache-size-kb = { $n } Ko
cache-size-mb = { $n } Mo
cache-size-gb = { $n } Go
settings-section-scheduler = Calculs en arrière-plan
settings-section-xmp = Notes (XMP)
settings-choice-xmp-sidecar-darktable = Darktable
settings-choice-xmp-sidecar-darktable-hint = IMAGE.RAF.xmp
settings-choice-xmp-sidecar-adobe = Adobe
settings-choice-xmp-sidecar-adobe-hint = IMAGE.xmp
settings-choice-xmp-sidecar-both = Les deux
settings-choice-files-group-pairs-ask = Demander
settings-choice-files-group-pairs-separate = Séparées
settings-choice-files-group-pairs-grouped = Regroupées
settings-section-xmp-fields = Notes : champs gérés par logiciel
settings-section-sidecars-rapidraw = Sidecars RapidRAW (notes)
settings-section-sidecars-rawtherapee = Sidecars RawTherapee (notes)
settings-section-sidecars-art = Sidecars ART (notes)
settings-section-places = Emplacements
settings-section-keys = Raccourcis
settings-section-develop = Logiciels de développement
settings-found-none = Aucun logiciel de développement pris en charge n'a été trouvé ({ $list }).
settings-developer-auto = Auto
settings-developer-manual = Manuel
settings-developer-off = Désactivé
settings-developer-auto-tip = Trouvé à cet endroit.
settings-developer-not-found = Non trouvé.
settings-developer-disabled = { $name } désactivé
settings-developer-browse = Parcourir…
settings-developer-missing = Fichier introuvable.
settings-section-export-profile-user = Préréglage d'export « { $id } »
settings-section-versions-preset-user = Préréglage de version « { $id } »
settings-section-export-profiles = Préréglages d'export
settings-section-versions = Versions
settings-section-versions-presets = Préréglages de version
settings-member-new = Nouveau…
settings-member-name = Nom :
settings-member-create = Créer
settings-member-create-tip = Copie de celui montré, sous ce nom
settings-member-rename = Renommer…
settings-member-rename-ok = Renommer
settings-member-rename-tip = Nouveau nom de celui montré
settings-member-taken = Ce nom est déjà pris
settings-member-delete = Supprimer
settings-member-builtin = Fourni avec tri : modifiable, pas supprimable
settings-profile-none = Aucun
settings-section-versions-presets-story = Préréglage de version « Story »
settings-section-versions-presets-post = Préréglage de version « Publication »
settings-section-versions-presets-carousel = Préréglage de version « Carrousel »
settings-section-export = Export
settings-section-export-sharpening = Accentuation de sortie
settings-section-export-profiles-edit = Préréglage d'export « Retouche »
settings-section-export-profiles-asis = Préréglage d'export « JPEG tel quel »
settings-section-export-profiles-web = Préréglage d'export « Web »
settings-section-watch = Surveillance du dossier
settings-section-sort = Tri
settings-section-update = Mises à jour
settings-section-exposure = Exposition
settings-section-offload = Déchargement de carte
settings-section-remote = Copie en local d'un serveur
settings-section-neighbourhood = Voisinage réseau
settings-section-files = Fichiers
settings-section-sharpness = Netteté
settings-section-similarity = Regroupement
settings-section-i18n = Langue
settings-section-data = Données
settings-section-log = Journal de l'application
settings-section-adjust = Réglages de visualisation
settings-section-gui = Interface
settings-section-gui-grid = Grille
settings-section-gui-view = Plein écran
settings-section-gui-zoom = Zoom et loupe
settings-section-gui-dialogs = Boîtes de dialogue
settings-section-gui-compare = Comparaison
settings-section-gui-position = Repère de position
settings-section-gui-notes = Notes sur l'image
settings-section-gui-end = Bout de la pile ou de la liste
end-stack-first = Début de la pile
end-stack-last = Fin de la pile
end-list-first = Début de la sélection
end-list-last = Fin de la sélection
position-photo = Photo { $n } / { $total }
position-stack = Pile { $n } / { $total }
position-in-stack = { $n } / { $total }
position-development = Dév. { $n } / { $total }
settings-section-gui-crop = Versions (édition)
settings-section-gui-folders = Panneau des dossiers
settings-section-gui-histogram = Histogramme
settings-section-gui-sharpness = Carte de netteté
settings-section-gui-metadata = Panneau des métadonnées
settings-section-gui-colors = Couleurs
delete-to-trash = Mettre à la corbeille
delete-permanently = Supprimer définitivement
delete-trash-question = { $n ->
    [one] Mettre à la corbeille { $n } fichier ?
   *[other] Mettre à la corbeille { $n } fichiers ?
}
delete-permanently-question = { $n ->
    [one] Supprimer définitivement { $n } fichier ?
   *[other] Supprimer définitivement { $n } fichiers ?
}
delete-kept-shared = { $name } gardé (partagé avec la paire)
delete-irreversible = Irréversible.
delete-failed = suppression impossible : { $error }
delete-developments = Retirer
delete-developments-question = { $n ->
    [one] Retirer le développement de son XMP ?
   *[other] Retirer { $n } développements de leurs XMP ?
}
delete-stripped = { $name } : historique darktable retiré, notes gardées
folder-new-title = Nouveau dossier dans « { $name } »
folder-new-button = Créer
folder-rename-title = Renommer le dossier « { $name } »
folder-delete-trash = Mettre le dossier « { $name } » à la corbeille ?
folder-delete-permanently = Supprimer définitivement le dossier « { $name } » ?
folder-contents = { $files ->
    [0] Dossier vide
    [one] 1 fichier
   *[other] { $files } fichiers
}{ $dirs ->
    [0] {""}
    [one] , 1 sous-dossier
   *[other] , { $dirs } sous-dossiers
}, tout compris.
folder-failed = { $error }
folders-place-root = « { $name } » est un emplacement : il ne se renomme, ne se déplace ni ne se supprime
folder-menu-new = Nouveau…
folder-menu-rename = Renommer…
folder-menu-move = Déplacer vers…
folder-menu-copy = Copier vers…
folder-menu-delete = Supprimer…
folder-menu-offload = Décharger la carte…
folder-menu-eject = Éjecter
folder-menu-mount = Monter
folder-menu-connect = Se connecter
folder-menu-forget = Oublier ce partage
folder-menu-offload-here = Décharger une carte ici…
folder-menu-cache-folder = Garder les données de tri dans ce dossier
folder-menu-cache-disk = Garder les données de tri sur ce disque
folder-menu-cache-release-folder = Ne plus garder les données de tri dans ce dossier
folder-menu-cache-release-disk = Ne plus garder les données de tri sur ce disque
folder-menu-cache-clear-folder = Vider le cache de ce dossier…
folder-menu-cache-clear-disk = Vider le cache de ce disque…
folder-menu-cache-clear-tip = Supprimer toutes les vignettes, aperçus et analyses gardés avec ces photos (.tri/cache) ; ils se recalculent à la demande. Les autres données de tri (filtres, curseurs, couvertures…) restent
folder-menu-reset-folder = Réinitialiser ce dossier…
folder-menu-reset-disk = Réinitialiser ce disque…
reset-title = Réinitialiser « { $name } »
reset-cache = Cache
reset-cache-detail = vignettes, aperçus, analyses ({ $size }) ; ils se recalculent
reset-data = Données de tri
reset-data-detail = filtres et tri, curseurs, couvertures de piles, développements préférés ({ $n } fichiers)
reset-notes = Notes
reset-notes-detail = étoiles, couleurs, drapeaux de { $n } photos, remis à zéro dans les sidecars sans toucher au reste
reset-notes-conflicts = { $n } photos en conflit de notes seront laissées
reset-notes-note = Lightroom garde ses notes dans son catalogue (« Lire les métadonnées du fichier ») ; darktable ne relit les XMP modifiés au démarrage qu'avec son option.
reset-versions = Versions
reset-versions-detail = { $n } versions sur { $photos } photos
reset-subfolders = Sous-dossiers compris
reset-measuring = Mesure…
reset-no-undo = Cache et données de tri ne s'annulent pas ; notes et versions, si.
reset-confirm = Réinitialiser
reset-running = Réinitialisation de « { $name } »…
reset-done = « { $name } » réinitialisé
reset-skipped = { $n } images laissées (conflit ou sidecar illisible)
folder-menu-cache-tip = Le cache (vignettes, aperçus, analyses) et les données de tri (filtres et tri, curseurs, couvertures de piles…) des photos de ce dossier, sous-dossiers compris, rangés dans .tri : ils voyagent avec lui, retrouvés sur un autre ordinateur. Les notes, elles, sont toujours dans les sidecars XMP
cache-kept-here = Données de tri gardées dans { $place }
cache-released = Données de tri retirées de { $place }
cache-kept-failed = Données de tri gardées sur cet ordinateur : { $error }
cache-move-failed = Déplacement des données de tri impossible : { $error }
watch-reloaded = { $count ->
    [one] Une image ajoutée, retirée ou modifiée ailleurs : dossier relu
   *[other] { $count } images ajoutées, retirées ou modifiées ailleurs : dossier relu
}
watch-notes = { $count ->
    [one] Notes d'une image modifiées ailleurs : relues
   *[other] Notes de { $count } images modifiées ailleurs : relues
}
folder-menu-favorite-add = Ajouter aux favoris
folder-menu-favorite-remove = Retirer des favoris
folder-menu-favorite-name = Nommer le favori…
favorite-name-title = Nom du favori « { $name } »
favorite-name-button = Nommer
favorite-named = Favori nommé « { $name } »
favorite-not-favorite = « { $name } » n'est pas un favori (m pour l'ajouter)
folders-favorite = Favori
folders-favorite-added = « { $name } » ajouté aux favoris
folders-favorite-removed = « { $name } » retiré des favoris
folders-favorite-error = Favoris non enregistrés : { $error }

## Ce qui est affiché

shown-thumbnail = Vignette (aperçu en cours de calcul)
shown-embedded = Rendu boîtier
shown-raw = Rendu RAW
shown-raw-full = Rendu RAW · 100 %
shown-embedded-full = Rendu boîtier · 1:1
shown-image-full = Image · 100 %
shown-exposure = Analyse d'exposition
shown-developed = Développé dans { $engine }
view-info-developed-pending = { " · " }développement en cours…
action-core-develop-render = Développer l'image affichée (dans le logiciel de son développement)
action-plugin-darktable-develop = Développer dans Darktable (l'image affichée)
action-plugin-rapidraw-develop = Développer dans RapidRAW (l'image affichée)
action-plugin-darktable-open_folder = Dossiers : ouvrir le dossier pointé dans Darktable
action-plugin-rapidraw-open_folder = Dossiers : ouvrir le dossier pointé dans RapidRAW
action-plugin-rawtherapee-develop = Développer dans RawTherapee (l'image affichée)
action-plugin-art-develop = Développer dans ART (l'image affichée)
action-plugin-rawtherapee-open_folder = Dossiers : ouvrir le dossier pointé dans RawTherapee
action-plugin-art-open_folder = Dossiers : ouvrir le dossier pointé dans ART
action-core-develop-folder = Développer tout le dossier en fond (chaque image dans le logiciel de son développement)
folder-menu-develop = Tout développer…
action-core-file-embed_notes = Inscrire les notes dans les images (JPEG, PNG, WebP, TIFF, HEIC)
kw-core-file-embed-notes = inscrire écrire notes étoiles images fichiers jpeg en-tête embarqué explorateur windows spotlight
folder-menu-embed-notes = Inscrire les notes dans les images
embed-running = Inscription des notes dans { $n } images…
embed-busy = Inscription des notes déjà en cours
embed-none = Rien à inscrire (pas de notes, ou déjà à jour ; JPEG, PNG, WebP, TIFF, HEIC seulement)
embed-done = Notes inscrites dans { $n } images
embed-failed = { $n } images n'ont pas reçu leurs notes : { $first }
action-core-file-import_notes = Relire les notes depuis les images (changées dans un autre logiciel)
kw-core-file-import-notes = relire importer lire notes étoiles images fichiers jpeg en-tête embarqué xnview explorateur
folder-menu-import-notes = Relire les notes depuis les images
folder-menu-adjust = Réglages auto
folder-group-open = Accès
folder-group-files = Dossier
folder-group-notes = Notes
folder-group-develop = Développement
folder-group-preferences = Préférences
menu-group-inline = Mettre ce groupe au premier niveau du menu
menu-group-fold = Replier ce groupe en sous-menu
folder-menu-adjust-auto = Auto
folder-menu-adjust-exposure = Exposition
folder-menu-adjust-balance = Balance des blancs
folder-menu-adjust-tip = Automatiques, de toutes ses images, chacune d'après la sienne (Auto : balance puis exposition) ; un seul pas à annuler
import-done = Notes relues depuis { $n } images
import-none = Rien à relire (pas de notes dans les images, ou les mêmes)
import-conflicts = Notes relues depuis { $done } images ; { $n } en conflit, à trancher
import-failed = Notes de { $n } images non relues : { $first }
notes-merged = Notes fusionnées depuis d'autres logiciels pour { $n } images
filter-conflicts = En conflit
filter-conflicts-tip = Les images dont les notes se contredisent d'un logiciel à l'autre, à trancher
conflict-badge = Notes en désaccord entre leurs sources : cliquer pour trancher
conflict-title = Notes en désaccord — { $name }
conflict-intro = Plusieurs logiciels ont changé les notes de cette photo différemment ({ $fields }). Cliquez la valeur à garder pour chaque champ en gras (ou la flèche d'une ligne) ; les autres sont déjà fusionnés.
conflict-before = Avant
conflict-col-stars = Étoiles
conflict-col-flag = Drapeau
conflict-col-colors = Couleurs
conflict-result = Résultat
conflict-modified = Modifié
conflict-modified-format = %d/%m/%Y %H:%M
conflict-apply = Valider
conflict-keep-tip = Choisir toutes les valeurs de cette ligne
conflict-keep = Garder
conflict-all = Même choix pour les { $n } autres images en conflit du dossier
conflict-resolved = Notes tranchées pour { $n } images
conflict-source-image = Dans l'image
conflict-source-xmp = XMP (darktable, Lightroom…)
conflict-field-stars = étoiles
conflict-field-flag = drapeau
conflict-field-colors = couleurs
conflict-stars = { $n ->
    [0] sans étoile
    [one] 1 étoile
   *[other] { $n } étoiles
}
conflict-flag-accepted = acceptée
conflict-flag-rejected = rejetée
folder-menu-embed = Notes dans les images
folder-menu-embed-tip = Inscrire d'office les notes dans les images de ce dossier (pas ses sous-dossiers), quel que soit le réglage général
folder-embed-default = Selon les réglages
folder-embed-always = Toujours
folder-embed-never = Jamais
folder-embed-set-default = { $name } : notes dans les images selon les réglages
folder-embed-set-always = { $name } : notes toujours inscrites dans les images
folder-embed-set-never = { $name } : notes jamais inscrites dans les images
folder-embed-mark-always = Notes toujours inscrites dans les images de ce dossier
folder-embed-mark-never = Notes jamais inscrites dans les images de ce dossier
embed-level-never = jamais
embed-level-internal = disques internes
embed-level-everywhere = partout
folder-menu-sidecars = Générer les sidecars…
action-core-file-sidecars = Générer les sidecars de notes manquants (un logiciel, ou tous)
kw-core-file-sidecars = sidecars rrdata xmp rapidraw notes métadonnées générer créer
sidecars-title-folder = Sidecars de « { $folder } »
sidecars-title-images = Sidecars de { $n ->
    [one] l'image
   *[other] { $n } images
}
sidecars-all = Tous les logiciels
sidecars-format = { $engine } (.{ $id })
sidecars-preview = { $created } à créer · { $rewritten } à réécrire · { $unchanged } laissées (déjà là) · { $skipped } passées (sans notes, ou développements)
sidecars-rewrite = Réécrire aussi les sidecars existants
sidecars-rewrite-tip = Leurs notes sont remplacées par celles de tri ; le reste (développement, mots-clés…) est conservé. Un fichier illisible est recréé.
sidecars-rewrite-warning = Attention : { $n ->
    [one] un sidecar existant sera réécrit
   *[other] { $n } sidecars existants seront réécrits
} (ses notes seulement, le reste est conservé).
sidecars-go = Générer
sidecars-done = Sidecars : { $created } créés · { $rewritten } réécrits
sidecars-done-errors = { " " }· { $n } erreurs : { $error }
folder-menu-open-in = Ouvrir dans { $name }
folder-open-in-missing = { $name } ne sait pas ouvrir un dossier, ou n'est pas installé
folder-open-in-running = { $name } est déjà ouvert : fermez-le d'abord (sous Windows, on ne peut pas lui passer un dossier alors)
develop-batch-unavailable = Darktable introuvable : réglage [develop] darktable
develop-batch-busy = un dossier est déjà en cours de développement
develop-batch-progress = développement de « { $name } » { $done }/{ $total }
develop-batch-done = « { $name } » développé : { $rendered } rendus, { $cached } déjà faits, { $failed } échecs
develop-batch-error = { " " }({ $error })
develop-batch-cancelled = { " (interrompu)" }
develop-batch-cancel = Interrompre le développement
view-info-developed-none = { " · " }pas encore développé ({ $keys })
develop-not-found = aucun logiciel de développement trouvé (Darktable, RapidRAW : réglages [develop])
develop-not-installed = { $engine } n'est pas installé (ou pas trouvé : réglages [develop])
view-back-grid = Retour à la grille
view-back-view = Retour au plein écran
grid-render-embedded = JPEG boîtier
grid-render-cover = Couverture
grid-render-libraw = RAW (LibRaw)
grid-render-raw = RAW
grid-render-jpeg = JPEG
grid-render-camera = boîtier
grid-render-libraw-short = LibRaw
grid-render-badge = Développé dans { $engine }
render-unusable = { $render } : pas disponible (logiciel absent ou désactivé)
grid-render-tip = Rendu affiché
filter-settings-in-panel = Réglages : section « Filtres » du panneau de droite ({ $keys })
panel-section-histogram = Histogramme
panel-section-filters = Filtre
panel-section-grid = Grille
panel-section-view = Vue
panel-section-exif = EXIF
panel-filter-sharp = Nettes
panel-filter-exposed = Bien exposées
panel-filter-developed = Développées
panel-clip-over = Brûlé
panel-clip-under = Bouché
panel-view-source = Avant / après
panel-view-developments = Développements
panel-view-review = Passer en revue les développements
panel-view-review-tip = Coché : { $nav } passent par chaque développement d'une photo (darktable, ses clones, RapidRAW…). Décoché : de photo en photo, chacune dans le rendu affiché ; pour voir les autres développements d'une photo, dépliez-la ({ $open }). [gui.view] review_developments
panel-grid-render = Rendu affiché
panel-render-tip = Le même en grille et en plein écran : chaque photo dans ce rendu, sinon son rendu RAW (une image plate reste elle-même).
panel-grid-render-filtered = Pour l'instant, le filtre « Développées » choisit le rendu.
panel-grid-sort = Tri
panel-grid-stacks = Piles
panel-grid-stacks-on = Empiler automatiquement
panel-grid-stacks-tip = Les photos semblables qui se suivent (rafale, même cadrage) forment une pile, repliée sous sa meilleure photo.
panel-grid-similarity = Similarité
panel-grid-unfold-all = Tout déplier
panel-grid-fold-all = Tout replier
panel-grid-grouping = Réglages fins…
panel-grid-files = Fichiers
panel-grid-files-allow = Actions sur les fichiers
panel-grid-files-tip = Renommer, copier, déplacer et supprimer images et dossiers depuis tri. Interdit par défaut : darktable, Lightroom, Capture One… attendent leurs images à leur place. [files] allow_actions
files-actions-off = Actions sur les fichiers désactivées : cocher « Actions sur les fichiers » (panneau de droite, Grille) ou [files] allow_actions.
rating-dialog-selection = { $n } images sélectionnées
rating-dialog-all = Toute la sélection ({ $n } images)
rating-dialog-all-tip = Notes et actions sur toutes les images sélectionnées, comme au clavier ; décochée : seulement l'image du clic droit.
rating-dialog-rename-one = Renommer : une image à la fois (décochez « Toute la sélection »)
files-library-warning = tri agit sur les fichiers eux-mêmes : un logiciel qui les range dans sa bibliothèque ou son catalogue (darktable, Lightroom, Capture One…) ne les y retrouvera plus.
slider-lenient = plus souple
slider-strict = plus strict
filter-developed-none = sans développement
filter-developed-engine = développées dans { $engine }
filter-developed-tip = Logiciels de développement : les images développées dans l'un des logiciels cochés, montrées dans le plus récent ; aucun coché : les images sans développement, en rendu LibRaw
filter-developed-unusable = Aucune photo développée dans ce dossier : le filtre « Développées » n'a rien à montrer.
filter-developed-off = Filtre « Développées » éteint : plus aucun de ses logiciels de développement n'est actif.
develop-with = Développer avec
develop-with-choose = choisir…
develop-with-libraw = Rendu RAW (LibRaw)
develop-with-not-developed = Pas développée dans { $engine } : rien à rendre (un sidecar qui ne porte qu'une note n'est pas un développement)
develop-folder-title = Développer « { $folder } »
develop-folder-each = Développement existant de chaque image (sinon rendu RAW)
develop-folder-only = Tout dans { $engine } (les images qu'il a développées)
develop-folder-go = Développer
develop-stale = Développement modifié dans { $engine } : { $keys } pour le refaire.
shown-image = Image

## Histogramme

histogram-indicators = { $source } · marge { $headroom } IL
histogram-zero-ev = 0 IL

## Déchargement

offload-history-unreadable = historique illisible : { $error }
offload-status = déchargement de « { $name } » { $done }/{ $total }
offload-summary = « { $name } » : { $copied } copiés, { $skipped } ignorés, { $errors } erreurs
offload-summary-cancelled = , interrompu
offload-title = Décharger « { $name } »
offload-planning = inventaire de la carte…
offload-images = { $n ->
    [one] { $n } image
   *[other] { $n } images
}
offload-images-known = { $n ->
    [one] { $n } image
   *[other] { $n } images
} : { $known ->
    [one] { $known } déjà déchargée
   *[other] { $known } déjà déchargées
}, { $new ->
    [one] { $new } nouvelle
   *[other] { $new } nouvelles
}
offload-already-there = , { $n } déjà dans la destination
offload-to-copy = { " " }; { $n } à copier ({ $mb } Mo).
offload-ignored = { $n ->
    [one] { $n } autre fichier (vidéo, fichier du boîtier) n'est pas copié.
   *[other] { $n } autres fichiers (vidéos, fichiers du boîtier) ne sont pas copiés.
}
offload-root = Dossier racine
offload-session = Dossier de la séance
offload-session-tip = Proposé : AAAA-MM-JJ_ (date de la plus récente image à copier), à compléter (« _Self ») ou à remplacer entièrement ; l'année, si on la veut, va dans le dossier racine. Vide : directement dans le dossier racine.
offload-destination = Destination : { $path }
offload-raw-only = RAW seulement
offload-keep-disk = Garder les données de tri sur ce disque
offload-keep-folder = Garder les données de tri dans le dossier de la séance
offload-raw-only-tip = Les JPEG du boîtier restent sur la carte.
offload-again = { $n ->
    [one] Décharger à nouveau l'image déjà déchargée
   *[other] Décharger à nouveau les { $n } images déjà déchargées
}
offload-again-tip = Un déchargement précédent les a déjà copiées, où que ce soit (historique des déchargements : même nom, même taille, même date) : elles ne sont pas recopiées. Cochée : elles le sont, sauf si une copie identique est déjà dans la destination ; pour décharger une carte une seconde fois ailleurs. Par défaut : [offload] skip_known.
offload-open-after = Ouvrir le dossier à la fin
offload-previews = Calculer les aperçus pendant la copie
offload-previews-tip = Vignettes, rendus boîtier, histogrammes, netteté et regroupement de chaque photo dès qu'elle est copiée : la séance est prête plus tôt.
offload-eject-after = Éjecter la carte à la fin
offload-delete-after = Supprimer de la carte les fichiers copiés et vérifiés
offload-delete-after-tip = Ceux copiés et vérifiés pendant ce déchargement, et ceux déjà présents à l'identique dans la destination. Restent : erreurs, images laissées, vidéos et fichiers du boîtier.
offload-delete-left = Supprimer aussi les JPEG laissés sur la carte
offload-delete-left-tip = Ils ne sont pas copiés : ils seront perdus.
offload-start = Décharger
offload-progress = { $done } / { $total } fichiers · { $mb } / { $total_mb } Mo
offload-background = Continuer en arrière-plan
offload-interrupt = Interrompre
offload-done = { $verified ->
    [yes] { $copied } fichiers copiés et vérifiés
   *[no] { $copied } fichiers copiés
}, { $skipped } ignorés (déjà déchargés), { $errors } erreurs.
offload-done-cancelled = { " " }— interrompu
offload-not-enough-room = Pas assez de place : { $free } Mo libres dans la destination, { $mb } Mo à copier. « Décharger quand même » lance la copie ; quand la place manquera, elle s'arrêtera et demandera quoi faire.
offload-start-anyway = Décharger quand même
offload-full-title = Plus de place dans la destination
offload-full-done = { $n } fichiers déjà copiés et vérifiés, gardés dans { $path }.
offload-full-left = { $n } fichiers restent à copier ({ $mb } Mo) ; { $free } Mo libres.
offload-full-what = Faites de la place (ou branchez un autre disque), puis Réessayer : la copie reprend sur le fichier qui n'a pas tenu. Abandonner arrête là : ce qui reste demeure sur la carte, rien n'est supprimé de la carte ni éjecté.
offload-full-retry = Réessayer
offload-full-give-up = Abandonner
offload-done-full = Arrêté : plus de place dans la destination. Le reste est toujours sur la carte (ni supprimé, ni éjecté).
offload-summary-full = , arrêté : destination pleine
offload-left = { $n } JPEG laissés sur la carte.
offload-deleted = Fichiers copiés supprimés de la carte.
offload-not-deleted = { $n } fichiers n'ont pas pu être supprimés.
offload-ejecting = Éjection de la carte : voir la barre d'état.
offload-open = Ouvrir le dossier
offload-eject = Éjecter la carte

## Touches et raccourcis (affichés, et lus dans [keys] dans toutes les langues)

keys-error = [keys] { $id } : { $error }
keys-leader-taken = « { $key } » sert déjà à « { $action } » : déplacez d'abord cette action dans [keys]
keys-profile-both = [gui] profile : « both » n'existe plus, le profil classique est utilisé (« classic » ou « vim »)
keys-file-error = keys.toml [{ $section }] { $id } : { $error }
keys-file-unknown-action = action inconnue
keys-file-section = keys.toml : section inconnue « { $section } » (common, classic, vim ou after_leader)
keys-file-unreadable = Raccourcis par défaut illisibles ({ $path } : { $error }) : ceux intégrés au programme sont utilisés.
keys-conflict = [keys] « { $spec } » attribué à { $first } et à { $second } : gardé pour { $first }
keys-replaces = [keys] « { $spec } » ({ $action }) remplace le raccourci de { $other }
keys-unknown-key = touche inconnue « { $text } »
keys-empty = raccourci vide
keys-none-short = sans raccourci
key-shift-prefix = Maj+
key-leader = <leader>
key-sequence-then = { " puis " }
key-menu-more = +{ $group }…
key-menu-loupe = Loupe
key-menu-zoom = Zoom
entry-menu-digits-keys = 0 – 9
entry-menu-digits = Pourcentage : 3 chiffres aussitôt, 1 ou 2 après une pause (2 : 200 %)
key-menu-cancel = Échap : abandonner
key-menu-empty = Aucune suite ici
key-submenu-filters = Filtres
key-submenu-histogram = Histogramme
key-submenu-peaking = Carte de netteté
key-submenu-render = Rendu et développement
key-submenu-stack-open = Ouvrir un niveau
key-submenu-stack-close = Fermer un niveau
key-submenu-quit = Quitter
key-submenu-go = Aller à
key-submenu-copy = Copier
key-submenu-delete = Supprimer
key-arrow-left = Flèche gauche
key-arrow-right = Flèche droite
key-arrow-up = Flèche haut
key-arrow-down = Flèche bas
key-home = Début
key-end = Fin
key-page-up = Page préc.
key-page-down = Page suiv.
key-escape = Échap
key-enter = Entrée
key-space = Espace
key-delete = Suppr
key-backspace = Retour arrière

## Réglages

settings-bad-color = couleur « { $value } » : attendu "#rrggbb"

## Score de netteté

progress-sharpness = netteté
progress-groups = regroupement
filter-bar-no-blurry = sans les floues
filter-bar-well-exposed = bien exposées
filter-bar-inverted = inversé
filter-bar-count-tip = images affichées / images du dossier
filter-bar-more = Tri et nombre d'images (la barre manque de place)
badge-blurry = Jugée floue (score de netteté)
badge-blurry-view = Jugée floue (score de netteté) ; clic : carte de netteté
panel-sharpness = Netteté
panel-sharpness-note = × { $folder } le dossier, × { $neighbours } les voisines
panel-sharpness-note-stack = × { $folder } le dossier, × { $stack } la pile
panel-sharpness-blurry = { " " }· floue

## Statistiques de l'histogramme

stats-ev = { $value } IL
stats-percent = { $value } %
stats-red = Rouge
stats-green = Vert
stats-blue = Bleu
stats-median = Médiane
stats-mean = Moyenne
stats-range = Plage ({ $low } % à { $high } %)
stats-range-value = { $low } à { $high }
stats-spread = Écart-type
stats-over = Saturés
stats-under = Bouchés
stats-raw-luminance-short = RAW : luminance par bloc de la mosaïque · IL sous la saturation
stats-raw-luminance = Données RAW : chaque photosite ne mesure qu'une couleur ; la luminance est calculée sur chaque bloc de la mosaïque qui porte les trois (2 × 2 en Bayer, 3 × 3 en X-Trans), avec la balance des blancs de la prise de vue et les couleurs du capteur. Valeurs en IL sous la saturation.
stats-luminance = Luminance
stats-all = Tous les canaux
histogram-channel-tip = { $channel } ({ $key } ; { $keys } : canal suivant)

# Mots-clés de recherche, invisibles (aide, réglages : actions::keywords), par
# réglage, section ou action ; ajoutés à ce qu'on cherche.
kw-gui = interface fenêtre apparence
kw-gui-grid = vignettes miniatures grille planche contact
kw-gui-view = plein écran visionneuse image affichée
kw-gui-zoom = agrandir loupe 100% grossissement
kw-gui-histogram = histogramme exposition écrêtage brûlé bouché cramé
kw-gui-sharpness = netteté focus peaking mise au point flou
kw-gui-colors = couleurs thème teintes
kw-gui-folders = dossiers arborescence panneau gauche
kw-gui-metadata = exif métadonnées informations panneau droit
kw-gui-dialogs = boîtes fenêtres modales
kw-gui-crop = version recadrage recadrer cadre poignées
kw-gui-compare = comparaison côte à côte rafale
kw-keys = raccourcis clavier touches
kw-preview = aperçus cache taille rendu
kw-cache = disque espace effacer
kw-xmp = sidecar notes étoiles darktable lightroom capture one
kw-offload = carte sd importer copier déchargement
kw-export = exporter jpeg tiff sortie
kw-files = fichiers corbeille supprimer renommer éditeur
kw-sharpness = netteté flou score
kw-similarity = piles groupes rafales similaires
kw-i18n = langue français anglais traduction
kw-update = mise à jour version nouvelle
kw-develop = darktable développement rendu
kw-places = volumes disques cartes emplacements éjecter
kw-watch = surveillance nouveaux fichiers
kw-exposure = exposition histogramme brûlé bouché
kw-gui-profile = vim nvim hjkl jeu de raccourcis clavier classique flèches
kw-gui-leader = tmux préfixe panneaux ctrl+b
kw-gui-theme = sombre clair gris neutre lightroom mode nuit
kw-gui-auto-advance = suivante automatique après note
kw-gui-reopen-last = dernier dossier démarrage lancement
kw-gui-key-sequence-timeout-ms = délai séquence gg
kw-gui-raw-previews = rendu raw fond calcul
kw-gui-filter-bar = barre des filtres
kw-gui-rating-bar = bandeau notation étoiles souris
kw-gui-history = annuler historique
kw-gui-grid-cell = taille vignettes
kw-gui-grid-info = nom format badges texte vignette masquer
kw-gui-grid-rejected-opacity = rejetées transparence estompées
kw-gui-view-stack-wrap = boucle pile
kw-gui-view-focus-point = collimateur autofocus af
kw-gui-zoom-max = 400% maximum
kw-gui-zoom-loupe-key = touche loupe
kw-gui-histogram-badge-over = brûlé cramé surexposé seuil badge
kw-gui-histogram-badge-under = bouché sous-exposé seuil badge
kw-gui-sharpness-colors = couleurs peaking
kw-i18n-language = langue français anglais english
kw-xmp-sidecar = darktable lightroom capture one bridge convention
kw-xmp-multiple-colors = plusieurs couleurs étiquettes
kw-files-trash = corbeille définitif supprimer
kw-files-editor = photoshop gimp affinity ouvrir avec
kw-files-pairs = raw+jpeg paires ensemble
kw-offload-destination = dossier destination racine importer
kw-offload-eject-after = éjecter carte après
kw-offload-delete-after = effacer carte
kw-develop-darktable = darktable-cli chemin appimage flatpak
kw-develop-rapidraw = rapidraw chemin appimage flatpak export
kw-develop-default = logiciel préféré défaut darktable rapidraw auto
kw-cache-zoom-max-mb = taille disque 100% 1:1
kw-preview-standard = taille aperçu plein écran résolution
kw-render-auto-bright = luminosité automatique
kw-render-camera-wb = balance des blancs boîtier
kw-sharpness-blur-ratio = floue seuil sévérité
kw-similarity-threshold = piles seuil regroupement
kw-similarity-max-gap-s = écart temps rafale secondes
kw-update-url = github releases
kw-places-fixed = disques fixes éjecter
kw-places-exclude = masquer volumes exclure
kw-places-auto-mount = monter automatiquement carte clé périphérique
kw-export-camera-suffix = suffixe jpeg boîtier paire
kw-core-app-settings = préférences configuration options
kw-core-app-help = raccourcis aide touches
kw-core-app-help-here = aide raccourcis touches ici contexte aide-mémoire
kw-core-app-palette = commande commandes palette lancer exécuter chercher réglage
kw-core-app-check-update = mise à jour version
kw-core-app-report-issue = bug bogue signalement issue github erreur
kw-core-adjust-before-after = avant après comparer réglages original
kw-core-adjust-apply = appliquer synchroniser harmoniser uniformiser réglages exposition balance série
kw-core-adjust-copy = copier réglages exposition balance noir et blanc style
kw-core-adjust-paste = coller appliquer réglages exposition balance noir et blanc style
kw-core-app-log = journal log erreur erreurs détail plantage trace diagnostic
kw-core-view-zoom = 100% 1:1 loupe agrandir
kw-core-view-source = jpeg boîtier rendu raw basculer
kw-core-view-peaking = netteté focus peaking mise au point
kw-core-view-clipping = brûlé bouché zones exposition alerte
kw-core-view-histogram = histogramme exposition
kw-core-view-overlay = grille tiers nombre d'or superposition
kw-core-version-new = recadrer recadrage cadrer nouvelle version story instagram
kw-core-view-compare = côte à côte rafale développements
kw-core-view-focus-mode = plein écran concentration
kw-core-view-focus-point = collimateur autofocus af
kw-core-view-grid-info = nom format badges masquer texte vignettes
kw-core-file-delete = supprimer corbeille effacer
kw-core-file-delete-photo = supprimer photo développements corbeille
kw-core-file-move = déplacer ranger
kw-core-file-copy = copier dupliquer
kw-core-file-export = exporter jpeg tiff sortie
kw-core-file-reveal = finder explorateur nautilus gestionnaire de fichiers
kw-core-file-edit = photoshop gimp éditeur externe ouvrir avec
kw-core-edit-undo = annuler
kw-core-edit-redo = rétablir refaire
kw-core-filter-blurry = floues netteté
kw-core-filter-well-exposed = exposition brûlé bouché cramé soleil
kw-core-filter-invert = inverser contraire
kw-core-filter-developed = développées développement darktable rapidraw logiciel
kw-core-flag-accept = pick choisie retenue
kw-core-flag-reject = rejeter éliminer
kw-core-select-visual = vim mode visuel
kw-core-stack-toggle = piles rafales regrouper
kw-core-stack-open = déplier ouvrir pile développements parenthèses
kw-core-stack-close = replier fermer pile développements parenthèses
kw-core-dev-fold-all = développements replier tout piles
kw-core-dev-toggle = développements darktable rapidraw clones copies virtuelles pile
kw-core-dev-next = développements darktable rapidraw clones copies virtuelles
kw-core-dev-prev = développements darktable rapidraw clones copies virtuelles
kw-core-folders-offload = carte sd importer décharger
kw-core-folders-eject = éjecter carte disque
kw-core-search = chercher trouver nom fichier
kw-core-panel-next = tmux panneaux leader
kw-plugin-airdrop-send = envoyer iphone partager
kw-core-develop-render = développé rendu darktable rapidraw
kw-plugin-darktable-develop = darktable développé rendu
kw-plugin-rapidraw-develop = rapidraw développé rendu
kw-plugin-rawtherapee-develop = rawtherapee rt développé rendu pp3
kw-plugin-art-develop = art développé rendu arp
kw-plugin-darktable-open-folder = darktable ouvrir dossier table lumineuse pellicule
kw-core-view-background = fond couleur noir blanc gris lumière éclairage
kw-core-view-image-info = infos concentration plein écran étiquette
kw-core-folders-favorite-name = renommer nom favori étiquette
kw-core-folders-connect = serveur réseau partage nas smb unc connecter monter
kw-core-folders-forget = serveur réseau partage nas oublier retirer
kw-core-folders-cache-clear = vider cache effacer place disque tri-cache .tri données supprimer vignettes
kw-core-folders-reset = réinitialiser remettre à zéro effacer nettoyer cache données notes étoiles versions
kw-core-grid-render = rendu vignettes développé développement couverture darktable rapidraw libraw jpeg boîtier grille
kw-control-grid-size = taille vignettes grille agrandir rapetisser plus grandes plus petites
kw-core-view-render-embedded = rendu affiché jpeg boîtier appareil
kw-core-view-render-cover = rendu affiché couverture développement préféré récent
kw-core-view-render-darktable = rendu affiché développement darktable
kw-core-view-render-rapidraw = rendu affiché développement rapidraw
kw-core-view-render-rawtherapee = rendu affiché développement rawtherapee
kw-core-view-render-art = rendu affiché développement art
kw-core-view-render-libraw = rendu affiché raw libraw brut
kw-core-filter-sort = tri trier ordre date nom chronologique
kw-core-filter-sort-reverse = tri trier ordre sens inverse décroissant croissant
kw-core-filter-sort-by-name = tri trier ordre nom fichier alphabétique
kw-core-filter-sort-by-captured = tri trier ordre date heure chronologique exif
kw-core-filter-sort-by-modified = tri trier ordre date fichier récent
kw-core-filter-sort-by-stars = tri trier ordre note notation
kw-core-filter-sort-by-color = tri trier ordre étiquette
kw-core-filter-sort-by-flag = tri trier ordre acceptées rejetées

kw-core-filter-stars-op = étoiles comparaison exactement au plus moins égal
kw-core-filter-colors-all = couleurs libellés toutes et ou combinaison plusieurs
remote-title = Copier « { $name } » en local
remote-why = Le système ne sait pas ouvrir directement un serveur { $protocol } ({ $address }) : ses images peuvent être copiées sur cet ordinateur, puis triées ici.
remote-scanning = lecture du dossier du serveur…
remote-summary = { $images } images, { $size } à copier
remote-destination = Dans le dossier :
remote-notes-local = Seules les images et leurs sidecars XMP sont copiés ; les notes posées ensuite restent sur la copie, pas sur le serveur.
remote-start = Copier
remote-background = En arrière-plan
remote-progress = Copie de « { $name } » : { $done } / { $total } fichiers · { $size } / { $total_size }
remote-progress-short = { $done } / { $total } fichiers · { $size } / { $total_size }
remote-done = « { $name } » copié en local
remote-scan-failed = Lecture de « { $name } » impossible : { $error }
remote-copy-failed = Copie de « { $name } » interrompue : { $error }
size-mb = { $value } Mo
size-gb = { $value } Go
size-tb = { $value } To
decimal-separator = ,
path-browse = Parcourir…
settings-accent = Couleur d'accent
settings-accent-tip = Suit la couleur d'accent du thème ([gui.colors] accent) ; décochée, une couleur fixe à choisir.
settings-editor-system = Application du système
settings-editor-other = Autre application
settings-editor-system-tip = Chaque image s'ouvre dans l'application que le système choisit pour son type.
settings-editor-system-apps = Chaque image s'ouvre dans l'application que le système choisit pour son type ({ $apps }).
settings-editor-program = Programme
settings-editor-args = Arguments
settings-editor-args-tip = { "{" }files{ "}" } : les images ouvertes (ajoutées à la fin s'il manque).
settings-editor-missing = Programme introuvable.
kw-core-filter-kind-all = tout montrer fichiers
kw-core-filter-kind-raw = raw seuls masquer jpeg
kw-core-filter-kind-flat = jpeg jpg seuls plates masquer raw
kw-core-view-loupe = loupe zoom 100 pointeur agrandir grossir magnifier
loupe-hold = Loupe : maintenez { $keys } en plein écran ou en comparaison.
review-unusable = Pas de revue des développements en JPEG boîtier (ni sans logiciel de développement) : changez d'abord le rendu affiché (<core.view.source>).
review-on = Revue des développements : chaque développement d'une photo, l'un après l'autre
review-off = Revue des développements arrêtée : de photo en photo
file-actions-on = Actions sur les fichiers autorisées
file-actions-off = Actions sur les fichiers interdites
kw-core-view-review-developments = revue développements clones copies virtuelles parcourir
kw-core-file-allow-actions = autoriser interdire renommer copier déplacer supprimer fichiers sécurité
loupe-entry = Loupe : { $digits } %
loupe-scale = Loupe : { $percent } %
loupe-entry-zero = Loupe : 0 % n'a pas de sens, ignoré
kw-core-view-loupe-scale = loupe grossissement zoom pourcentage agrandir
loupe-reset = Loupe à sa valeur par défaut
zoom-reset = Zoom ajusté à la vue
loupe-reset-tip = Loupe ; un clic : { $percent } %
zoom-reset-tip = Zoom ; un clic : ajusté à la vue ({ $percent } %)
scale-edit-tip = Un clic : taper la valeur (Entrée applique, Échap annule)
zoom-entry = Zoom : { $digits } %
zoom-scale = Zoom : { $percent } %
kw-core-view-zoom-scale = zoom pourcentage agrandir
zoom-percent-range = { $value } % : hors des bornes ({ $low } à { $high } %)
zoom-steps-empty = au moins un palier de zoom
zoom-fit-value = Ajusté
zoom-fit-tip = Ajusté à la vue : { $percent } %

# Paires RAW+JPEG regroupées (#392)
pairs-ask-title = Regrouper RAW et JPEG ?
pairs-ask-text = { $name } a { $n ->
    [one] une paire RAW+JPEG
   *[other] { $n } paires RAW+JPEG
} (même nom). Les regrouper en une seule photo ?
pairs-ask-detail = Regroupée, une paire n'est qu'une vignette ; le JPEG du boîtier est rangé avec les développements du RAW (dépliez-les pour le voir). Rien ne change sur le disque. Ce choix est retenu pour ce dossier, et se change dans son menu (clic droit).
pairs-ask-always = Ne plus demander (ce choix pour tous les dossiers)
pairs-group = Regrouper
pairs-separate = Garder séparées
pairs-camera-chip = Boîtier
pairs-camera-tip = Le JPEG du boîtier, regroupé avec son RAW
pairs-grouped-notice = RAW et JPEG regroupés dans ce dossier
pairs-separated-notice = RAW et JPEG séparés dans ce dossier
folder-menu-pairs-group = Regrouper RAW et JPEG
folder-menu-pairs-separate = Séparer RAW et JPEG
pairs-filter-raw = RAW
pairs-filter-pairs = RAW+JPEG
pairs-filter-jpeg = JPEG
pairs-filter-raw-tip = Les RAW sans JPEG
pairs-filter-pairs-tip = Les paires RAW+JPEG regroupées
pairs-filter-jpeg-tip = Les JPEG (et autres images) sans RAW
offload-group-pairs = Regrouper RAW et JPEG ({ $n ->
    [one] une paire
   *[other] { $n } paires
})
offload-group-pairs-tip = Dans la séance, chaque paire RAW+JPEG n'est qu'une photo : le JPEG du boîtier est rangé avec les développements du RAW. Retenu pour le dossier de la séance ; se change dans son menu (clic droit)
folders-pairs-grouped = RAW et JPEG regroupés dans ce dossier (une photo par paire)
dev-modified-format = %d/%m %H:%M
view-render-cover = Couverture
view-render-cover-tip = En faire la couverture de la photo (la vignette de sa pile) ; recliquer : plus de couverture choisie, le plus récent
view-render-is-cover = Ce rendu est la couverture de la photo
