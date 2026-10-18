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
action-core-filter-cycle = Filtre de type : tout, RAW, plates
action-core-filter-stars-0 = Filtre : étoiles indifférentes
action-core-filter-stars-1 = Filtre : au moins 1 étoile
action-core-filter-stars-2 = Filtre : au moins 2 étoiles
action-core-filter-stars-3 = Filtre : au moins 3 étoiles
action-core-filter-stars-4 = Filtre : au moins 4 étoiles
action-core-filter-stars-5 = Filtre : 5 étoiles
action-core-filter-flag-accepted = Filtre : acceptées seulement
action-core-filter-flag-not_rejected = Filtre : masquer les rejetées
action-core-filter-flag-rejected = Filtre : rejetées seulement
action-core-filter-color-red = Filtre : rouge (ajouter / retirer)
action-core-filter-color-yellow = Filtre : jaune (ajouter / retirer)
action-core-filter-color-green = Filtre : vert (ajouter / retirer)
action-core-filter-color-blue = Filtre : bleu (ajouter / retirer)
action-core-filter-color-purple = Filtre : violet (ajouter / retirer)
action-core-filter-clear = Effacer tous les filtres
action-core-filter-blurry = Filtre : masquer les floues
action-core-filter-only_blurry = Filtre : seulement les floues
action-core-view-filter_bar = Afficher / masquer la barre de filtres
action-core-view-rating_bar = Afficher / masquer le bandeau de notation
action-core-rate-auto_advance = Avance automatique après une note : oui / non
action-core-select-toggle = Sélectionner / désélectionner l'image
action-core-select-visual = Mode visuel : sélectionner une plage (v pour valider)
action-core-view-compare = Comparer les images sélectionnées côte à côte
action-core-view-peaking = Afficher / masquer la netteté (contours nets, en 1:1)
action-core-view-peaking_color = Netteté : couleur suivante
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
action-core-file-move = Déplacer vers…
action-core-edit-undo = Annuler la dernière modification
action-core-edit-redo = Rétablir
action-core-edit-history = Historique des modifications
action-core-stack-toggle = Déplier / replier la pile
action-core-stack-all = Déplier / replier toutes les piles
action-core-stack-settings = Regroupement des images semblables
action-core-stack-mode = Piles (grille ; plein écran : flèches gauche / droite dans la pile, haut / bas de pile en pile)
action-core-stack-choose = Choisir comme couverture de la pile (sa préférée)
stack-chosen = { $name } : couverture de sa pile
stack-position = pile : { $n } / { $total }
stack-slider-tip = Ressemblance : vers la droite, des piles plus grandes ; pendant le réglage, les piles sont dépliées (Ctrl+G : réglages fins)
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
action-core-metadata-open = Métadonnées : ouvrir (histogramme en grand)
action-core-metadata-select_up = Métadonnées : étendre la sélection vers le haut
action-core-metadata-select_down = Métadonnées : étendre la sélection vers le bas
action-core-metadata-copy_all = Métadonnées : tout copier
action-core-view-folders = Afficher / masquer le panneau des dossiers
action-core-view-refresh_folders = Actualiser les dossiers (disque branché)
action-core-view-histogram = Afficher / masquer l'histogramme
action-core-view-histogram_scale = Histogramme : IL / linéaire
action-core-view-histogram_large = Histogramme en grand
action-core-view-histogram_channel = Grand histogramme : canal suivant
action-core-histogram-red = Grand histogramme : rouge seul
action-core-histogram-green = Grand histogramme : vert seul
action-core-histogram-blue = Grand histogramme : bleu seul
action-core-histogram-all = Grand histogramme : tous les canaux
action-core-view-clipping = Afficher / masquer les zones brûlées et bouchées
action-core-view-metadata = Afficher / masquer le panneau des métadonnées
action-core-view-open = Ouvrir en plein écran
action-core-view-zoom = Zoom 100 % / ajusté
action-core-view-zoom_in = Zoom : palier supérieur
action-core-view-zoom_out = Zoom : palier inférieur
action-core-view-zoom_fit = Zoom : ajusté
action-core-view-zoom_100 = Zoom : 100 %
view-info-zoom = { " · " }{ $percent } %
view-loupe-loading = 100 % en cours…
action-core-view-source = Basculer JPEG boîtier / rendu RAW
action-core-view-pan_left = Image zoomée : aller à gauche
action-core-view-pan_right = Image zoomée : aller à droite
action-core-view-pan_up = Image zoomée : aller en haut
action-core-view-pan_down = Image zoomée : aller en bas
action-core-view-back = Fermer / revenir
action-core-app-help = Afficher ou masquer cette aide
action-core-app-settings = Réglages
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

## Panneaux, éjection, notes

panel-no-image = Aucune image sélectionnée.
histogram-enlarge = Agrandir ({ $keys })
histogram-computing = histogramme en cours de calcul
panel-reading = lecture…
folders-title = Dossiers
folders-refresh = Actualiser
folders-refresh-tip = Relire les volumes et les dossiers (F5). Un disque branché apparaît aussi de lui-même en quelques secondes.
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
eject-failed = « { $name } » non éjecté : { $error }
cache-unavailable = cache inaccessible : { $error }

## Comparaison, plein écran, barre d'état

rating-not-saved = note non enregistrée : { $error }
compare-needs-two = comparer : sélectionnez au moins 2 images (Espace, v, Ctrl+A, Maj+clic)
compare-limited = comparaison limitée aux { $max } premières ([gui.compare] max)
view-info-selected = { " " }· sélectionnée
view-info-peaking = { " " }· netteté
view-info-peaking-computing = { " " }· netteté en calcul
view-info-peaking-one-to-one = { " " }· netteté : en 1:1 (z)
view-info-sensor = { " " }({ $percent } % du capteur)
view-info-preview-computing = { " " }· aperçu en cours de calcul
view-info-raw-pending = { " " }· rendu RAW en cours
view-info-full-computing = { " " }· calcul du 100 % en cours
view-info-one-to-one-loading = { " " }· chargement du 1:1
view-hint-raw = b : JPEG boîtier / RAW   z : 1:1   i : histogramme   o : zones   Échap : grille
view-hint-flat = z : 1:1   i : histogramme   o : zones   Échap : grille
status-no-folder = aucun dossier
status-images = { $n ->
    [one] { $n } image
   *[other] { $n } images
} ({ $raws } RAW) · filtre : { $filter }
status-selected = { $n ->
    [one] { $n } sélectionnée
   *[other] { $n } sélectionnées
}
status-visual-mode = { " " }· mode visuel
progress-thumbnails = vignettes
progress-embedded = JPEG boîtier
progress-previews = aperçus
progress-histograms = histogrammes
status-key-problems = { $n ->
    [one] raccourcis : { $n } problème
   *[other] raccourcis : { $n } problèmes
}
status-invalid-config = configuration invalide : réglages par défaut
status-show-offload = Afficher le déchargement
status-retry-eject = Réessayer l'éjection
status-dismiss = ignorer
status-open-session = Ouvrir la séance « { $name } »

## Boîtes, bandeau, filtres, aide

close = Fermer
histogram-linear-scale = Échelle linéaire
histogram-ev-scale = Échelle en IL
or-escape = ou Échap
rating-bar-title = Notes
rating-bar-applies = notes appliquées aux { $n } images sélectionnées
filter-summary-stars = { $n ->
    [5] 5 étoiles
    [one] { $n } étoile et plus
   *[other] { $n } étoiles et plus
}
filter-summary-or = { " ou " }
filter-all = tout
filter-flat = plates
filter-bar-all = Tout
filter-bar-flat = Plates
filter-bar-clear = Effacer
filter-bar-count = { $shown }/{ $n }
badge-over = { $percent } % de pixels saturés (brûlée)
badge-under = { $percent } % de pixels bouchés
help-title = Raccourcis
help-search-hint = Rechercher (ex. « pnd », « etoile 3 »)…
help-no-match = Aucune action ne correspond.
help-close-hint = Échap ou F1 pour fermer.
empty-no-folder = Choisissez un dossier dans le panneau de gauche (Maj+D l'affiche).
empty-folder = Aucune image dans ce dossier.
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
rename-failed = renommage impossible : { $error }
transfer-copy-title = { $n ->
    [one] Copier { $n } image vers…
   *[other] Copier { $n } images vers…
}
transfer-move-title = { $n ->
    [one] Déplacer { $n } image vers…
   *[other] Déplacer { $n } images vers…
}
transfer-files = { $n ->
    [one] { $n } fichier (paires et sidecars XMP compris)
   *[other] { $n } fichiers (paires et sidecars XMP compris)
}
transfer-shared = { $name } est partagé avec une image qui reste : copié
transfer-create = le dossier sera créé
transfer-copy = Copier
transfer-move = Déplacer
transfer-create-copy = Créer et copier
transfer-create-move = Créer et déplacer
transfer-hint = Tab : compléter · flèches haut / bas : destinations récentes
transfer-recent = Récentes
transfer-progress = { $done } / { $total } fichiers
transfer-failed = copie / déplacement impossible : { $error }
groups-title = Regroupement des images semblables
groups-enabled = Regrouper en piles
groups-threshold = Écart toléré entre deux images voisines
groups-threshold-tip = Plus haut : des piles plus grandes ; plus bas : seulement les rafales.
groups-gap = Écart de temps maximal
groups-gap-tip = Deux images prises à plus d'intervalle ne sont jamais groupées (0 : sans limite).
groups-summary = { $images } images : { $stacks } piles, { $groups } groupes en tout
groups-computing = empreintes : { $done } / { $total }
groups-default = Seuil par défaut
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
settings-save-failed = enregistrement de { $path } impossible : { $error }
settings-reset = défaut
settings-default = valeur par défaut : { $value }
settings-restart = au prochain lancement
settings-section-restart = Réglages du moteur : appliqués au prochain lancement.
settings-no-path = emplacement du fichier de configuration inconnu (--config FICHIER)
settings-section-preview = Aperçus
settings-section-render = Rendu RAW
settings-section-cache = Cache
settings-section-scheduler = Calculs en arrière-plan
settings-section-xmp = Notes (XMP)
settings-section-places = Emplacements
settings-section-watch = Surveillance du dossier
settings-section-exposure = Exposition
settings-section-offload = Déchargement de carte
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
folders-place-root = « { $name } » est un emplacement : il ne se renomme ni ne se supprime
folder-menu-new = Nouveau dossier…
folder-menu-rename = Renommer…
folder-menu-delete = Supprimer…
folder-menu-offload = Décharger la carte…
folder-menu-eject = Éjecter
folder-menu-offload-here = Décharger une carte ici…
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
folders-favorite = Favori
folders-favorite-added = « { $name } » ajouté aux favoris
folders-favorite-removed = « { $name } » retiré des favoris
folders-favorite-error = Favoris non enregistrés : { $error }

## Ce qui est affiché

shown-thumbnail = Vignette (aperçu en cours de calcul)
shown-embedded = JPEG boîtier
shown-raw = Rendu RAW
shown-raw-full = Rendu RAW · 100 %
shown-embedded-full = JPEG boîtier · 1:1
shown-image-full = Image · 100 %
shown-exposure = Analyse d'exposition
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
keys-shadowed = « { $keys } » ({ $action }) ne se déclenche jamais : c'est le début de « { $longer } »
keys-unknown-key = touche inconnue « { $text } »
keys-empty = raccourci vide
key-shift-prefix = Maj+
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
filter-bar-only-blurry = floues seulement
filter-bar-count-tip = images affichées / images du dossier
badge-blurry = floue
panel-sharpness = Netteté
panel-sharpness-value = { $score } (× { $folder } le dossier, × { $neighbours } les voisines)
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
stats-raw-luminance = Données RAW : chaque photosite ne mesure qu'une couleur, pas de luminance ; le vert en est la meilleure approximation. Valeurs en IL sous la saturation.
stats-all = Tous les canaux
histogram-channel-tip = { $channel } ({ $key } ; { $keys } : canal suivant)
analysis-over = saturé
analysis-under = bouché
