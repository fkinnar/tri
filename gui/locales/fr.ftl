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
action-core-filter-stars-0 = Filtre : étoiles indifférentes
action-core-filter-stars-1 = Filtre : 1 étoile (au moins, exactement ou au plus : f =)
action-core-filter-stars-2 = Filtre : 2 étoiles (au moins, exactement ou au plus : f =)
action-core-filter-stars-3 = Filtre : 3 étoiles (au moins, exactement ou au plus : f =)
action-core-filter-stars-4 = Filtre : 4 étoiles (au moins, exactement ou au plus : f =)
action-core-filter-stars-5 = Filtre : 5 étoiles (au moins, exactement ou au plus : f =)
action-core-filter-stars-op = Filtre d'étoiles : au moins, exactement, au plus
action-core-filter-flag-accepted = Filtre : acceptées seulement
action-core-filter-flag-not_rejected = Filtre : masquer les rejetées
volume-back = Disque revenu : dossier relu, calculs repris.
action-core-filter-flag-rejected = Filtre : rejetées seulement
action-core-filter-color-red = Filtre : rouge (ajouter / retirer)
action-core-filter-color-yellow = Filtre : jaune (ajouter / retirer)
action-core-filter-color-green = Filtre : vert (ajouter / retirer)
action-core-filter-color-blue = Filtre : bleu (ajouter / retirer)
action-core-filter-color-purple = Filtre : violet (ajouter / retirer)
action-core-filter-clear = Effacer tous les filtres
action-core-filter-blurry = Filtre : masquer les floues
action-core-filter-well_exposed = Filtre : bien exposées (ni brûlées ni bouchées)
action-core-filter-stack = Filtre : seulement la pile courante, toute dépliée (encore : tout)
action-core-filter-invert = Filtre : inverser (ce que les filtres retirent)
action-core-filter-developed = Filtre : développées (allumer / éteindre ; logiciels à cocher dans la liste)
action-core-grid-render = Rendu des vignettes suivant : JPEG boîtier, couverture, un logiciel, LibRaw
action-core-filter-sort = Ordre suivant : nom, date de prise de vue, date de modification
action-core-filter-sort_reverse = Inverser l'ordre (croissant / décroissant)
sort-name = Nom
sort-captured = Prise de vue
sort-modified = Modification
sort-notice = Ordre : { $key }, { $direction }
filter-analysis-partial = Analyses en cours : filtre appliqué aux { $done } / { $total } photos déjà analysées ; la grille se complète au fil du calcul.
sort-ascending = croissant
sort-descending = décroissant
action-core-view-filter_bar = Afficher / masquer la barre de filtres
action-core-view-image_info = Mode concentration : afficher / masquer les infos et les notes sur l'image
action-core-view-background = Couleur de fond suivante (thème, gris foncé, noir, blanc)
action-core-view-rating_bar = Afficher / masquer le bandeau de notation
action-core-view-grid_info = Afficher / masquer les infos des vignettes (nom, format, notes, badges)
action-core-rate-auto_advance = Avance automatique après une note : oui / non
action-core-select-toggle = Sélectionner / désélectionner l'image
action-core-select-visual = Mode visuel : sélectionner une plage (v pour valider)
action-core-view-compare = Comparer les images sélectionnées côte à côte (sans sélection : la pile repliée, ou la photo et ses développements)
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
   *[other] Afficher dans le gestionnaire de fichiers
}
action-core-file-edit = Ouvrir dans l'éditeur externe
action-core-file-copy = Copier vers…
action-core-file-export-report = Rapport d'export
action-core-file-export = Exporter…
action-core-view-crop = Recadrer (nouvelle version)
action-core-view-version_next = Version suivante de l'image (original, puis chaque version)
action-core-crop-move-left = Recadrage : déplacer le cadre à gauche
action-core-crop-move-right = Recadrage : déplacer le cadre à droite
action-core-crop-move-up = Recadrage : déplacer le cadre vers le haut
action-core-crop-move-down = Recadrage : déplacer le cadre vers le bas
action-core-crop-size-left = Recadrage : rapprocher le côté droit
action-core-crop-size-right = Recadrage : éloigner le côté droit
action-core-crop-size-up = Recadrage : remonter le bas
action-core-crop-size-down = Recadrage : descendre le bas
action-core-crop-ratio-free = Recadrage : rapport libre
action-core-crop-ratio-original = Recadrage : rapport de l'image
action-core-crop-ratio-square = Recadrage : 1:1
action-core-crop-ratio-5_4 = Recadrage : 5:4 (4:5)
action-core-crop-ratio-3_2 = Recadrage : 3:2 (2:3)
action-core-crop-ratio-16_9 = Recadrage : 16:9 (9:16)
action-core-crop-ratio-4_3 = Recadrage : 4:3 (3:4)
action-core-crop-rotate = Recadrage : portrait ou paysage
action-core-crop-save = Recadrage : enregistrer la version
action-core-crop-preset = Recadrage : préréglage suivant
action-core-crop-preset_prev = Recadrage : préréglage précédent
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
action-core-stack-mode = Piles (grille ; plein écran : flèches gauche / droite dans la pile, haut / bas de pile en pile)
action-core-dev-toggle = Déplier / replier les développements de la photo
action-core-dev-next = Développement suivant de la photo
action-core-dev-fold_all = Replier toutes les piles de développements
action-core-dev-prev = Développement précédent de la photo
action-core-stack-choose = Choisir comme couverture de la pile (sa préférée)
action-core-stack-first = Première image de la pile (ou la photo, avant ses développements)
action-core-stack-last = Dernière image de la pile (ou le dernier développement)
action-core-stack-enter = En entrant dans une pile : sa première photo (allumé) ou sa couverture
## Développements d'une photo (darktable, RapidRAW ; 30/10/2026)
dev-count = { $n } dév.
dev-count-tip = { $n ->
    [one] { $n } développement
   *[other] { $n } développements
  } · { $keys } : déplier / replier
dev-clone = clone { $n }
dev-copy = copie { $n }
dev-none = Cette photo n'a pas de développement (darktable, RapidRAW)
dev-before = Avant : le JPEG boîtier à la place du développement (b : retour)
dev-after = Après : le développement
dev-preferred = { $name } : couverture de la photo
dev-preferred-none = Plus de développement préféré : la pile se replie sur le plus récent
dev-fold-tip = Replier les développements ({ $keys }) · Alt+clic : replier et garder celui-ci en couverture
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
histogram-photo-tip = Photo d'analyse (zones saturées / bouchées sur l'image en gris) : afficher / masquer ({ $keys })
action-core-histogram-all = Grand histogramme : tous les canaux
action-core-histogram-luminance = Grand histogramme : la luminance seule
action-core-view-clipping = Afficher / masquer les zones brûlées et bouchées
action-core-view-metadata = Afficher / masquer le panneau des métadonnées
action-core-view-open = Ouvrir en plein écran
action-core-view-zoom = Zoom 100 % / ajusté
action-core-view-zoom_in = Zoom : palier supérieur (en grille : vignettes plus grandes)
action-core-view-zoom_out = Zoom : palier inférieur (en grille : vignettes plus petites)
action-core-view-zoom_fit = Zoom : ajusté (en grille et dans les dossiers, / recherche)
action-core-view-zoom_100 = Zoom : 100 % (en grille : toutes les vignettes à l'écran)
view-info-zoom = { " · " }{ $percent } %
view-loupe-loading = 100 % en cours…
action-core-view-source = Basculer rendu boîtier / rendu RAW
action-core-view-pan_left = Image zoomée : aller à gauche
action-core-view-pan_right = Image zoomée : aller à droite
action-core-view-pan_up = Image zoomée : aller en haut
action-core-view-pan_down = Image zoomée : aller en bas
action-core-view-back = Fermer / revenir
action-core-app-help = Afficher ou masquer cette aide
action-core-app-settings = Réglages
action-core-app-check_update = Rechercher une mise à jour
action-core-app-report_issue = Signaler un problème (formulaire de GitHub prérempli)
action-core-app-quit = Quitter
action-core-rate-star-0 = Retirer les étoiles
action-core-rate-star-1 = 1 étoile
action-core-rate-star-2 = 2 étoiles
action-core-rate-star-3 = 3 étoiles
action-core-rate-star-4 = 4 étoiles
action-core-rate-star-5 = 5 étoiles
action-core-rate-color-red = Couleur rouge (ajouter / retirer)
action-core-rate-color-yellow = Couleur jaune (ajouter / retirer)
action-core-rate-color-green = Couleur verte (ajouter / retirer)
action-core-rate-color-blue = Couleur bleue (ajouter / retirer)
action-core-rate-color-purple = Couleur violette (ajouter / retirer)
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
group-crop = Recadrage
crop-bar-title = Recadrer
crop-ratio-free = Libre
crop-ratio-original = Original
crop-portrait = Portrait
crop-landscape = Paysage
crop-name = Nom
crop-profile = Export
crop-profile-none = Aucun
crop-save = Enregistrer
crop-saved = Version « { $name } » enregistrée
crop-name-taken = Ce nom est déjà pris (une autre version, ou « Original » : l’image non recadrée)
crop-name-original-ratio = Recadrage
crop-name-taken-notice = Version non enregistrée : le nom « { $name } » est déjà pris
crop-failed = Version non enregistrée : { $error }
crop-preset-none = Préréglage
crop-preset-tip = Préréglage de recadrage : rapport, nom et préréglage d'export (p : suivant)
crop-preset-save = Enregistrer comme préréglage de recadrage
crop-preset-save-tip = Enregistrer comme préréglage de recadrage : rapport, nom et export du cadre en cours, gardés dans les réglages
crop-preset-saved = Préréglage « { $name } » enregistré
preset-story = Story
preset-post = Publication
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
folders-refresh-tip = Relire les volumes et les dossiers (F5). Un disque branché apparaît aussi de lui-même en quelques secondes.
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
compare-needs-two = comparer : sélectionnez au moins 2 images (Espace, v, Ctrl+A, Maj+clic), ou placez-vous sur une pile ou une photo développée
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
view-hint-raw = b : rendu boîtier / RAW   z : 1:1   i : histogramme   w : zones   Échap : grille
view-hint-flat = z : 1:1   i : histogramme   w : zones   Échap : grille
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
grid-size = Taille des vignettes
files-with-pairs = Avec l'autre fichier de la paire RAW+JPEG
files-with-pairs-tip = Décocher pour n'agir que sur les images choisies (ex. supprimer les JPEG en gardant les RAW)
help-copy-version = Copier la version et le système (pour un signalement)
help-version-copied = Version et système copiés
help-report = Signaler un problème
help-report-tip = Ouvre le formulaire de GitHub, prérempli avec la version et le système
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
rating-bar-applies = notes appliquées aux { $n } images sélectionnées
filter-stack-none = f c : placez-vous sur une pile (piles actives)
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
    [one] 1 version (recadrage) ; clic : recadrer
   *[other] { $n } versions (recadrages) ; clic : recadrer
}
help-title = Raccourcis
help-search-hint = Rechercher (ex. « pnd », « etoile 3 »)…
help-no-match = Aucune action ne correspond.
help-close-hint = Échap ou F1 pour fermer.
empty-folder-gone = Le dossier « { $name } » n'est plus là (disque débranché ?)
empty-folder-gone-back = Il se rouvrira de lui-même quand le disque reviendra.
empty-no-folder = Choisissez un dossier dans le panneau de gauche (Maj+D l'affiche).
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
empty-filtered = Aucune image pour ces filtres (f puis Échap pour les effacer).
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
export-report-reveal = Afficher dans le gestionnaire de fichiers
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
settings-keys-doc = Raccourcis de chaque action : ceux du jeu choisi ([gui] profile), ou ceux écrits ici, qui les remplacent pour cette action (en gras). Un clic sur un raccourci le retire ; « enregistrer… » ajoute la prochaine touche (ou séquence) tapée ; le champ accepte un raccourci écrit (« Maj+P », « g g », « ° »). Les conflits sont signalés.
loupe-key-unknown = touche de loupe inconnue : « { $key } »
loupe-key-taken = [gui.zoom] loupe_key : « { $key } » sert aussi à une action
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
settings-section-scheduler = Calculs en arrière-plan
settings-section-xmp = Notes (XMP)
settings-section-sidecars-rapidraw = Sidecars RapidRAW (notes)
settings-section-places = Emplacements
settings-section-keys = Raccourcis
settings-section-develop = Rendu Darktable
settings-section-export-profile-user = Préréglage d'export « { $id } »
settings-section-versions-preset-user = Préréglage de recadrage « { $id } »
settings-section-export-profiles = Préréglages d'export
settings-section-versions-presets = Préréglages de recadrage
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
settings-section-versions-presets-story = Préréglage de recadrage « Story »
settings-section-versions-presets-post = Préréglage de recadrage « Publication »
settings-section-export = Export
settings-section-export-sharpening = Accentuation de sortie
settings-section-export-profiles-edit = Préréglage d'export « Retouche »
settings-section-export-profiles-asis = Préréglage d'export « JPEG tel quel »
settings-section-export-profiles-web = Préréglage d'export « Web »
settings-section-watch = Surveillance du dossier
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
settings-section-gui = Interface
settings-section-gui-grid = Grille
settings-section-gui-view = Plein écran
settings-section-gui-zoom = Zoom et loupe
settings-section-gui-dialogs = Boîtes de dialogue
settings-section-gui-compare = Comparaison
settings-section-gui-crop = Recadrage
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
folder-menu-new = Nouveau dossier…
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
folder-menu-cache-folder = Garder le cache dans ce dossier
folder-menu-cache-disk = Garder le cache sur ce disque
folder-menu-cache-release = Ne plus garder le cache ici
folder-menu-cache-tip = Vignettes, aperçus et analyses des photos de ce dossier (sous-dossiers compris) rangés dans .tri-cache : ils voyagent avec lui, retrouvés sur un autre ordinateur
cache-kept-here = Cache gardé dans { $place }
cache-released = Cache retiré de { $place }
cache-kept-failed = Cache gardé chez l'utilisateur : { $error }
cache-move-failed = Déplacement du cache impossible : { $error }
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
action-core-develop-folder = Développer tout le dossier en fond (chaque image dans le logiciel de son développement)
folder-menu-develop = Développer tout le dossier…
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
sidecars-preview = { $created ->
    [0] Rien à créer
    [one] 1 sidecar à créer
   *[other] { $created } sidecars à créer
} · { $unchanged } déjà pourvues · { $skipped } passées (sans notes, ou développements)
sidecars-go = Générer
sidecars-done = { $created ->
    [0] Aucun sidecar créé
    [one] 1 sidecar créé
   *[other] { $created } sidecars créés
}
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
grid-render-embedded = JPEG boîtier
grid-render-cover = Couverture
grid-render-libraw = Rendu RAW (LibRaw)
grid-render-libraw-short = LibRaw
grid-render-badge = Développé dans { $engine }
grid-render-badge-libraw = Rendu RAW (LibRaw) : pas de développement de ce logiciel
grid-render-tip = Rendu des vignettes
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
panel-view-source = Source affichée
panel-view-before = Avant le développement (rendu boîtier)
panel-grid-render = Rendu des vignettes
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
files-library-warning = tri agit sur les fichiers eux-mêmes : un logiciel qui les range dans sa bibliothèque ou son catalogue (darktable, Lightroom, Capture One…) ne les y retrouvera plus.
slider-lenient = plus souple
slider-strict = plus strict
filter-developed-none = sans développement
filter-developed-engine = développées dans { $engine }
filter-developed-tip = Logiciels de développement : les images développées dans l'un des logiciels cochés, montrées dans le plus récent ; aucun coché : les images sans développement, en rendu LibRaw
filter-developed-none-hint = Aucun coché : sans développement (LibRaw)
develop-with = Développer avec
develop-with-choose = choisir…
develop-with-libraw = Rendu RAW (LibRaw)
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
offload-known = { $n ->
    [one] , dont { $n } déjà déchargée
   *[other] , dont { $n } déjà déchargées
}
offload-already-there = , { $n } déjà dans la destination
offload-to-copy = { " " }; { $n } à copier ({ $mb } Mo).
offload-ignored = { $n ->
    [one] { $n } autre fichier (vidéo, fichier du boîtier) n'est pas copié.
   *[other] { $n } autres fichiers (vidéos, fichiers du boîtier) ne sont pas copiés.
}
offload-root = Dossier racine
offload-session = Dossier de la séance
offload-session-tip = Proposé : AAAA-MM-JJ_ (date de la plus ancienne image), à compléter (« _Self ») ou à remplacer entièrement ; l'année, si on la veut, va dans le dossier racine.
offload-destination = Destination : { $path }
offload-raw-only = RAW seulement
offload-raw-only-tip = Les JPEG du boîtier restent sur la carte.
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
offload-left = { $n } JPEG laissés sur la carte.
offload-deleted = Fichiers copiés supprimés de la carte.
offload-not-deleted = { $n } fichiers n'ont pas pu être supprimés.
offload-ejecting = Éjection de la carte : voir la barre d'état.
offload-open = Ouvrir le dossier
offload-eject = Éjecter la carte

## Touches et raccourcis (affichés, et lus dans [keys] dans toutes les langues)

keys-error = [keys] { $id } : { $error }
keys-conflict = [keys] « { $spec } » attribué à { $first } et à { $second } : gardé pour { $first }
keys-replaces = [keys] « { $spec } » ({ $action }) remplace le raccourci de { $other }
keys-unknown-key = touche inconnue « { $text } »
keys-empty = raccourci vide
key-shift-prefix = Maj+
key-leader = <leader>
key-sequence-then = { " puis " }
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
kw-gui-crop = recadrage cadre poignées
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
kw-core-app-check-update = mise à jour version
kw-core-app-report-issue = bug bogue signalement issue github erreur
kw-core-view-zoom = 100% 1:1 loupe agrandir
kw-core-view-source = jpeg boîtier rendu raw basculer
kw-core-view-peaking = netteté focus peaking mise au point
kw-core-view-clipping = brûlé bouché zones exposition alerte
kw-core-view-histogram = histogramme exposition
kw-core-view-overlay = grille tiers nombre d'or superposition
kw-core-view-crop = recadrer version story instagram
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
kw-plugin-darktable-open-folder = darktable ouvrir dossier table lumineuse pellicule
kw-core-view-background = fond couleur noir blanc gris lumière éclairage
kw-core-view-image-info = infos concentration plein écran étiquette
kw-core-folders-favorite-name = renommer nom favori étiquette
kw-core-folders-connect = serveur réseau partage nas smb unc connecter monter
kw-core-folders-forget = serveur réseau partage nas oublier retirer
kw-core-grid-render = rendu vignettes développé développement couverture darktable rapidraw libraw jpeg boîtier grille
kw-core-filter-sort = tri trier ordre date nom chronologique
kw-core-filter-sort-reverse = tri trier ordre inverse décroissant croissant

kw-core-filter-stars-op = étoiles comparaison exactement au plus moins égal
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
