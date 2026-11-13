# Catalogue du moteur et de la ligne de commande — français (langue de référence).
# Chaque identifiant doit exister aussi dans en.ftl (vérifié par les tests).

images-count = { $n ->
    [one] { $n } image
   *[other] { $n } images
}

# Messages généraux
warning-prefix = attention : { $message }

## Métadonnées

metadata-unreadable = fichier illisible : { $error }
metadata-camera = Appareil
metadata-lens = Objectif
metadata-exposure = Exposition
metadata-bias = Correction
metadata-bias-value = { $value } IL
metadata-mode = Mode
metadata-focal = Focale
metadata-focal-equivalent = éq. 24 × 36 : { $equivalent } mm
metadata-focal-value = { $focal } mm
metadata-flash = Flash
metadata-flash-fired = déclenché
metadata-flash-not-fired = non déclenché
metadata-date = Date
metadata-date-value = { $day }/{ $month }/{ $year } à { $time }
metadata-dimensions = Dimensions
metadata-dimensions-value = { $width } × { $height }
metadata-dimensions-note = { $mpx } Mpx
metadata-file = Fichier
metadata-size-kb = { $size } ko
metadata-size-mb = { $size } Mo
program-manual = Manuel
program-normal = Programme
program-aperture = Priorité ouverture
program-shutter = Priorité vitesse
program-creative = Créatif (profondeur de champ)
program-action = Action (vitesse rapide)
program-portrait = Portrait
program-landscape = Paysage

## Erreurs

raw-init-failed = libraw_init a échoué
raw-invalid-path = chemin non transmissible à LibRaw : { $path }
raw-libraw-error = LibRaw erreur { $code } : { $message }
raw-cancelled = traitement LibRaw annulé
raw-empty-image = LibRaw a renvoyé une image nulle sans code d'erreur
raw-no-mosaic = RAW sans mosaïque de filtres couleur (DNG linéaire, Foveon)
raw-unsupported-image = image non gérée (type { $kind }, { $colors } canaux, { $bits } bits)
flat-read-failed = lecture impossible : { $error }
flat-decode-failed = décodage impossible : { $error }
flat-heif-not-compiled = HEIF non pris en charge (compilé sans « heif »)
flat-heif-failed = décodage HEIF impossible : { $error }
flat-heif-layout = libheif n'a pas renvoyé de RGB 8 bits
files-invalid-name = nom invalide : « { $name } »
files-exists = { $path } existe déjà
files-trash-failed = mise à la corbeille impossible : { $error }
files-launch-failed = lancement de « { $program } » impossible : { $error }
files-not-a-directory = { $path } n'est pas un dossier
files-same-folder = les images sont déjà dans { $path }
files-mismatch = copie de { $path } différente de l'original (rien n'a été gardé)
files-into-itself = impossible de mettre { $path } dans lui-même
files-changed = { $path } a changé entre-temps : rien n'a été fait
files-not-in-trash = { $path } n'est plus dans la corbeille
history-notes-changed = { $name } : notes modifiées ailleurs depuis (rien n'a été changé)
history-missing = { $name } est introuvable (déplacé ou supprimé ailleurs)
history-modified = { $name } a été modifié ailleurs depuis (rien n'a été changé)
history-nothing = rien à annuler
history-trash-one = { $name } mis à la corbeille
history-create-dir = dossier { $name } créé
history-adjust = réglages : { $name }
history-adjust-many = réglages de { $n } images
history-version-created = version « { $version } » créée : { $name }
history-version-changed = version « { $version } » modifiée : { $name }
history-version-deleted = version « { $version } » supprimée : { $name }
history-stars = { $n ->
    [0] 0 étoile
    [one] 1 étoile
   *[other] { $n } étoiles
}
history-accepted = acceptée
history-rejected = rejetée
history-no-flag = sans drapeau
history-color-added = { $color }
history-color-removed = sans { $color }
history-red = rouge
history-yellow = jaune
history-green = vert
history-blue = bleu
history-purple = violet
history-notes-one = { $what } : { $name }
history-notes-many = { $what } : { $n } images
history-rename = { $from } renommé en { $to }
history-reset = Réinitialisation de « { $name } »
history-copy-dir = dossier { $name } copié vers { $dest }
history-move-dir = dossier { $name } déplacé vers { $dest }
history-copy = { $n ->
    [one] 1 image copiée vers { $dest }
   *[other] { $n } images copiées vers { $dest }
}
history-move = { $n ->
    [one] 1 image déplacée vers { $dest }
   *[other] { $n } images déplacées vers { $dest }
}
history-trash = { $n ->
    [one] 1 fichier mis à la corbeille
   *[other] { $n } fichiers mis à la corbeille
}
history-developments = { $n ->
    [one] 1 développement supprimé
   *[other] { $n } développements supprimés
}
config-invalid-toml = valeur TOML invalide : { $value }
config-not-a-section = « { $section } » n'est pas une section
preview-io = accès fichier impossible : { $error }
preview-no-embedded = pas d'aperçu embarqué exploitable
preview-cancelled = calcul annulé
preview-not-an-image = ce niveau n'est pas une image
exposure-unreadable = données d'exposition illisibles
cache-no-user-dir = dossier de cache utilisateur introuvable
xmp-io = sidecar XMP : { $error }
xmp-malformed = sidecar XMP mal formé (non modifié) : { $error }
xmp-no-description = sidecar XMP sans rdf:Description (non modifié)
notes-conflict = notes en désaccord entre ses sources (sidecars, image) : à trancher d'abord
embed-unsupported = ce format d'image ne porte pas de notes (JPEG, PNG, WebP, TIFF, HEIC seulement)
embed-io = notes dans l'image : { $error }
embed-write-failed = écriture des notes dans l'image impossible ({ $error }) ; une copie complète est gardée à côté : { $copy }
embed-verify-failed = notes relues dans l'image différentes de celles écrites
embed-malformed = image mal formée, laissée telle quelle
embed-malformed-detail = image mal formée, laissée telle quelle : { $error }
embed-bigtiff = TIFF de plus de 4 Go (BigTIFF) : notes non inscrites
embed-too-large = notes trop volumineuses pour l'en-tête de l'image
embed-compressed = XMP compressé dans le PNG : notes non inscrites (rien ne serait gardé)
embed-heif-sequence = HEIF animé ou séquence d'images : notes non inscrites
embed-xmp-not-utf8 = XMP de l'image illisible (pas en UTF-8), laissé tel quel
settings-auto-or-integer = "auto" ou un entier positif
offload-no-data-dir = dossier de données introuvable (HOME absent)
offload-unnamed-file = fichier sans nom
offload-verify-failed = la copie relue diffère de l'original (support défectueux ?)
offload-copy-missing = copie absente ou différente : conservé
places-home = Dossier personnel
places-desktop = Bureau
places-documents = Documents
places-downloads = Téléchargements
places-pictures = Images
places-root = Racine (/)
places-device = Support de { $size } Go
places-mount-unsupported = monter un support n'est possible que sous Linux et macOS
servers-unreachable = partage injoignable : { $error }
servers-tool-missing = outil du système introuvable : { $error }
servers-refused = le système a refusé la connexion : { $error }
remote-unsupported = pas de copie en local depuis un serveur { $protocol }
remote-cancelled = copie annulée
remote-failed = échec de la copie : { $error }
servers-unsupported = { $system } ne sait pas monter un serveur { $protocol }
servers-no-listing = { $system } ne sait pas lister les partages d'un serveur { $protocol } : tapez le nom du partage
servers-not-found = connecté, mais « { $name } » n'apparaît pas parmi les partages montés
servers-login-needed = le serveur demande un compte
servers-login-refused = utilisateur ou mot de passe refusé
servers-host-unreachable = serveur injoignable : vérifiez son nom, qu'il est allumé et sur le même réseau
servers-share-missing = ce partage n'existe pas sur le serveur : tapez le nom du serveur seul pour voir ses partages
servers-other-account = Windows est déjà connecté à ce serveur sous un autre compte : déconnectez-le d'abord (net use)
servers-question = le serveur pose une question : { $question } ; connectez-vous une première fois depuis le gestionnaire de fichiers
remote-missing = dossier introuvable ou refusé sur le serveur
remote-sftp-keys = SFTP : connexion par clé SSH seulement (ssh-copy-id, ou une clé dans ~/.ssh)
remote-sftp-host-key = SFTP : serveur inconnu ; connectez-vous une première fois par ssh pour accepter sa clé
settings-defaults-used = { $warning } ; réglages par défaut utilisés
settings-file-error = configuration { $path } : { $error }

## Ligne de commande

cli-unknown-convention = convention inconnue : { $value } (darktable ou adobe)
cli-adjust-not-an-image = pas une image : { $path }
cli-adjust-report = { $image } : exposition { $exposure } IL (correction { $correction }, créatif { $creative }), balance { $kelvin } K, teinte { $tint }, N&B { $mixer } → { $out }
cli-unknown-option = option inconnue : { $option }
cli-extra-argument = argument en trop : { $argument }
cli-cache = cache : { $path }
cli-cache-level = { $entries } entrées  { $mib } Mio  (moyenne { $average } Kio)
cli-previews-start = { $n } images ({ $raws } RAW), vignettes { $thumbnail } px, aperçus { $standard } px, { $workers } workers légers + 1 lourd, { $order ->
    [thumbnails] vignettes d'abord
   *[mixed] mélangé
}
cli-previews-progress = vignettes { $thumbnails }   rendu boîtier { $embedded }   histogrammes { $histograms }   aperçus { $previews } { $percent } %   { $seconds } s
cli-thumbnails-done = vignettes terminées en { $time }
cli-embedded-done = aperçus boîtier terminés en { $time } (tout le dossier consultable)
cli-all-done = terminé en { $time } ({ $ms } ms par image en moyenne)
cli-failure = échec : { $error }
cli-trimmed = limite des 1:1 : { $n } entrées anciennes supprimées ({ $mb } Mo)
cli-offload-plan = { $card } : { $n } images ({ $known } déjà déchargées), { $ignored } autres fichiers ignorés, { $mb } Mo à copier
cli-nothing-to-copy = rien à copier
cli-destination = destination : { $path }
cli-already-there = { $n } images déjà dans la destination (ignorées si identiques)
cli-to-copy = à copier
cli-offload-progress = { $done }/{ $total } fichiers   { $mb }/{ $total_mb } Mo
cli-error = erreur
cli-offload-out-of-space = plus de place dans { $path } : { $left } fichiers restent à copier ({ $mb } Mo, { $free } Mo libres) ; arrêt, ils restent sur la carte
cli-offload-stopped-full = déchargement arrêté : destination pleine
cli-offload-done = { $verified ->
    [yes] { $copied } copiés et vérifiés
   *[no] { $copied } copiés
}, { $skipped } ignorés, { $left } laissés sur la carte, { $errors } erreurs, en { $time }
cli-deleted = { $n } fichiers supprimés de la carte
cli-not-deleted = non supprimé
cli-ejected = carte éjectée
cli-eject-failed = éjection impossible : { $error }
eject-not-a-drive = { $path } : pas une lettre de lecteur
eject-busy = volume en cours d'utilisation (fichier ouvert ?) : { $error }
cli-some-not-copied = certains fichiers n'ont pas été copiés
cli-exposure-header = fichier	source	marge (IL)	saturés	bouchés	durée
cli-exposure-raw-source = RAW { $bits } bits, noir { $black }, blanc { $white }
cli-error-detail = erreur : { $error }
cli-xmp-written = { $mode ->
    [dry-run] à écrire
   *[write] écrit
}
cli-xmp-conflict = conflit	{ $image } : notes différentes de l'image qui partage le même sidecar, ignorée
cli-xmp-summary = { $n } images : { $written } sidecars { $mode ->
    [dry-run] à écrire
   *[write] écrits
}, { $unchanged } inchangées, { $conflicts } conflits, { $errors } erreurs
cli-xmp-some-failed = certains sidecars n'ont pas pu être convertis
cli-unknown-sidecar = format de sidecar inconnu : { $value } (rapidraw, xmp ou all)
cli-sidecar-rewritten = { $mode ->
    [dry-run] à réécrire
   *[write] réécrit
}
cli-sidecars-summary = { $n } images : { $created } sidecars { $mode ->
    [dry-run] à créer
   *[write] créés
}, { $rewritten } { $mode ->
    [dry-run] à réécrire
   *[write] réécrits
}, { $unchanged } laissées (déjà pourvues), { $skipped } passées (sans notes, ou développements), { $errors } erreurs
cli-embed-written = { $mode ->
    [dry-run] à inscrire
   *[write] inscrites
}
cli-embed-summary = { $n } images : notes { $mode ->
    [dry-run] à inscrire dans { $written }
   *[write] inscrites dans { $written }
}, { $unchanged } inchangées, { $skipped } passées (RAW et autres formats), { $errors } erreurs
cli-embed-some-failed = certaines images n'ont pas pu recevoir leurs notes
cli-import-written = { $mode ->
    [dry-run] à relire
   *[write] relues
}
cli-import-summary = { $n } images : notes { $mode ->
    [dry-run] à relire depuis { $written }
   *[write] relues depuis { $written }
}, { $unchanged } inchangées (sans notes inscrites, ou les mêmes), { $skipped } passées (RAW et autres formats), { $errors } erreurs
cli-import-some-failed = les notes de certaines images n'ont pas pu être relues
cli-merged = fusionnées
cli-conflict = conflit
cli-conflict-before = avant
cli-conflicts-summary = { $n } images, { $conflicts } en conflit
cli-field-stars = étoiles
cli-field-flag = drapeau
cli-field-colors = couleurs
cli-flag-accepted = acceptée
cli-flag-rejected = rejetée
cli-rename-shared-copy = (copie : partagé)
cli-delete-trash = corbeille :
cli-delete-permanent = supprimé :
cli-delete-kept = gardé (partagé) :
cli-delete-stripped = développement darktable retiré de :
cli-dry-run = (--dry-run : rien n'a été modifié)
cli-export-unknown-profile = préréglage d'export inconnu : { $profile } (préréglages : { $known })
cli-export-plan = export « { $profile } » ({ $summary }) vers { $dest } :
cli-export-libraw = Darktable absent (ou --libraw) : rendu de base par LibRaw
cli-export-as-is-raw = le préréglage « { $profile } » ne s'applique qu'aux images déjà développées (applies_to) : un RAW n'a pas d'espace couleur, son rendu doit en recevoir un (autre préréglage, ou --embedded pour son rendu boîtier)
cli-export-raw-only = le préréglage « { $profile } » ne s'applique qu'au rendu d'un RAW (applies_to)
cli-export-passthrough = dont { $n } sans recompression
cli-export-done = { $n ->
    [one] 1 fichier écrit
   *[other] { $n } fichiers écrits
}
cli-export-failed = { $failed } images sans fichier écrit (illisibles par Darktable ?)
cli-groups-header = image	suivante	distance	écart (s)	avec la pause
cli-groups-summary = { $n } images, { $groups } groupes au seuil { $threshold } ({ $ms } ms par empreinte)
cli-cache-cleared = cache vidé ({ $level })
cli-cache-here = cache et données de tri gardés dans { $place }
cli-cache-released = cache et données de tri retirés de { $place }
cli-cache-moved = { $n } entrées déplacées
cli-reset-cache = cache : { $n } entrées
cli-reset-data = données de tri : { $n } fichiers
cli-reset-notes = notes remises à zéro : { $n } images
cli-reset-versions = versions retirées : { $n }
cli-reset-conflicts = { $n } images en conflit de notes, laissées
cli-cache-move-failed = déplacement du cache impossible : { $error }
cli-cache-all = tout
cli-cache-folder-cleared = cache de { $folder } vidé ({ $level }) : { $entries } entrées, { $mib } Mio libérés
cli-group = GROUPE { $index } ({ $n } images)

## Netteté (ligne de commande)

cli-sharpness-header = fichier	score	× dossier	× voisines	× pile	jugement	durée
cli-sharpness-blurry = FLOUE
cli-sharpness-summary = { $n } images, médiane { $median } : { $blurry } floues (sous { $ratio } × la médiane du dossier, ou { $neighbour_ratio } × celle des { $neighbours } voisines de chaque côté)

## Superpositions
overlay-thirds = Tiers
overlay-golden = Nombre d'or
overlay-spiral = Spirale d'or
overlay-diagonals = Diagonales
overlay-bad-color = couleur « { $color } » : attendu "#rrggbb"
overlay-bad-ratio = format « { $ratio } » : attendu largeur:hauteur, par exemple "4:5"
overlay-bad-image = logo { $path } illisible : { $error }

## Envoi (plugins)
send-nothing = rien à envoyer
send-main-thread = envoi impossible hors du fil principal de l'interface
send-unavailable = { $name } n'est pas disponible sur cette machine
send-refused = { $name } refuse ces fichiers

## Rendu par un logiciel de développement
develop-none = pas de développement pour cette image
develop-unknown-engine = logiciel de développement inconnu : « { $value } » (auto, darktable, rapidraw, rawtherapee, art)
develop-unavailable = { $engine } introuvable (réglage [develop] { $id })
develop-rapidraw-open = interface ouverte : l'export en ligne de commande ne tourne pas en même temps, fermez-la puis recommencez
develop-rapidraw-black = image toute noire : l'interface de RapidRAW est sans doute ouverte, fermez-la puis recommencez
develop-failed = { $engine } : { $error }
export-no-profile = pas de développement à appliquer (pas de profil, ou un profil qui ne porte que des notes)
export-bad-bits = préréglage d'export « { $profile } » : { $bits } bits impossible en { $format }
export-bad-quality = préréglage d'export « { $profile } » : qualité { $quality } hors de 1 à 100
export-bad-color = préréglage d'export « { $profile } » : espace couleur inconnu « { $value } »
export-bad-intent = préréglage d'export « { $profile } » : intention de rendu inconnue « { $value } »
export-needs-16-bits = préréglage d'export « { $profile } » : « { $color } » demande 16 bits ou plus (en 8 bits, les dégradés se cassent), donc TIFF ou PNG en 16 bits
export-libraw-color = préréglage d'export « { $profile } » : l'espace « { $color } » demande Darktable (sans Darktable : sRGB, Adobe RGB, ProPhoto, Rec709, Rec2020)
export-libraw-bits = préréglage d'export « { $profile } » : le TIFF 32 bits demande Darktable
preset-bad-ratio = préréglage « { $preset } » : rapport inconnu « { $value } » (free, original, ou l:h comme 9:16)
export-bad-icc = profil couleur de l'image illisible : conversion impossible (« JPEG tel quel » la garde telle quelle)
export-version-empty = Darktable n'a rien rendu pour cette version

# Recherche d'une mise à jour (src/update.rs)
update-no-curl = curl introuvable ({ $error }) : il fait la requête
update-request-failed = recherche impossible : { $error }
update-bad-answer = réponse illisible : { $error }
cli-update-available = Nouvelle version : { $version }
   { $url }
cli-update-latest = Vous avez la dernière version ({ $version }).
update-build = Build personnel ({ $commit }) : pas une version distribuée.
update-nightly = Build de nuit ({ $commit }) : pas une version publiée.
update-nightly-behind = { $count ->
    [one] La branche main a 1 commit de plus : il sera dans le prochain build de nuit.
   *[other] La branche main a { $count } commits de plus : ils seront dans le prochain build de nuit.
}
update-build-current = À jour avec la branche main.
update-build-behind = { $count ->
    [one] La branche main a 1 commit de plus : git pull, puis recompiler.
   *[other] La branche main a { $count } commits de plus : git pull, puis recompiler.
}
update-build-unknown = Impossible de comparer avec la branche main (commit inconnu, ou pas encore poussé).
update-nothing-published = rien de publié pour ce système

## Aide de la ligne de commande (src/commands.rs, issue n° 441)
cli-usage-title = usage :
cli-usage-more = tri --help : chaque commande expliquée, avec ses options et un exemple ; option commune : --config <fichier>
cli-help-intro = tri — visionneuse RAW pour le tri. Sans option : l'interface. En ligne de commande :
cli-help-example = exemple :
cli-help-common = option commune :
cli-theme-folder = Dossier et aperçus
cli-theme-notes = Notes
cli-theme-files = Fichiers
cli-theme-export = Export
cli-theme-offload = Déchargement
cli-theme-analysis = Analyses
cli-theme-cache = Cache et données
cli-theme-other = Divers
cli-ph-folder = dossier
cli-ph-image = image
cli-ph-images = images
cli-ph-name = nom
cli-ph-card = carte
cli-ph-root = racine
cli-ph-preset = préréglage
cli-ph-reference = référence
cli-ph-level = niveau
cli-ph-file = fichier
cli-ph-destination = destination
cli-cmd-list = Liste les images du dossier, par nature.
cli-cmd-previews = Calcule en fond vignettes, aperçus et analyses du dossier, avec la progression.
cli-cmd-convert-xmp = Recopie les notes dans les sidecars de l'autre convention (darktable : IMAGE.RAF.xmp, Adobe : IMAGE.xmp).
cli-cmd-sidecars = Crée les sidecars manquants des photos notées, pour un logiciel qu'on n'utilise qu'à l'occasion.
cli-cmd-embed-notes = Inscrit les notes dans les images elles-mêmes (JPEG, PNG, WebP, TIFF, HEIC ; les RAW sont passés).
cli-cmd-import-notes = Recopie dans les sidecars les notes inscrites dans les images (changées dans un autre logiciel).
cli-cmd-conflicts = Liste les images dont les sources de notes se contredisent, et les fusions faites.
cli-cmd-rename = Renomme l'image (extension gardée), sa paire RAW+JPEG et ses sidecars.
cli-cmd-delete = Met les images, leurs paires et leurs sidecars à la corbeille.
cli-cmd-copy = Copie ou déplace les images (ou un dossier entier), leurs paires et leurs sidecars ; copies vérifiées.
cli-cmd-reveal = Montre l'image dans le gestionnaire de fichiers.
cli-cmd-edit = Ouvre les images dans l'éditeur externe ([files] editor).
cli-cmd-export = Exporte les images avec un préréglage d'export (darktable s'il est installé, sinon LibRaw).
cli-cmd-adjust = Harmonise l'exposition et la balance des images sur la première (la référence) et écrit le résultat.
cli-cmd-offload = Décharge une carte : copie vérifiée des nouvelles images vers le dossier de la séance.
cli-cmd-exposure = Exposition de chaque image : marge avant saturation, pixels saturés et bouchés.
cli-cmd-metadata = Métadonnées de prise de vue de chaque image.
cli-cmd-sharpness = Score de netteté de chaque image, et les floues.
cli-cmd-groups = Regroupe les images semblables en piles.
cli-cmd-clear-cache = Vide le cache (un niveau ou tout) ; avec un dossier, seulement ses images.
cli-cmd-cache-usage = Taille du cache, par niveau.
cli-cmd-cache-here = Garde le cache et les données de tri dans le dossier (--cache-here), ou les rend au cache de l'utilisateur (--cache-user).
cli-cmd-reset = Efface du dossier le cache, les données de tri, les notes ou les versions.
cli-cmd-check-update = Cherche une version plus récente (sur demande seulement).
cli-opt-raw = RAW seulement
cli-opt-flat = images plates seulement (JPEG, TIFF, PNG, HEIC…)
cli-opt-depth = profondeur des sous-dossiers (1 : le dossier seul)
cli-opt-side = côté long des aperçus, en pixels
cli-opt-adjust-side = côté long de l'image écrite, en pixels
cli-opt-thumb = côté des vignettes, en pixels
cli-opt-workers = nombre de calculs légers en parallèle
cli-opt-mixed = aperçus sans attendre la fin des vignettes
cli-opt-cache = dossier du cache, à la place de celui des réglages
cli-opt-dry-run = montre ce qui serait fait, sans rien changer
cli-opt-rewrite = réécrit aussi les notes des sidecars existants
cli-opt-permanent = suppression définitive, sans corbeille
cli-opt-to = dossier de destination
cli-opt-offload-to = racine des séances (défaut : [offload] destination)
cli-opt-export-to = dossier, à la place de celui du préréglage
cli-opt-libraw = rendu LibRaw, même avec darktable installé
cli-opt-embedded = le JPEG boîtier
cli-opt-adjust-embedded = part du JPEG boîtier plutôt que du rendu RAW
cli-opt-passthrough = JPEG sans recompression quand c'est possible
cli-opt-ev = correction d'exposition en IL, ou auto
cli-opt-neutral = balance neutralisée : toute l'image (auto) ou une zone en fractions
cli-opt-kelvin = balance en kelvins (et teinte), ou un préréglage
cli-opt-creative-ev = décalage créatif d'exposition, en IL
cli-opt-warmth = décalage créatif de chaleur
cli-opt-tint = décalage créatif de teinte
cli-opt-contrast = contraste, en pour cent
cli-opt-saturation = saturation, en pour cent
cli-opt-mono = noir et blanc : un préréglage ou trois pourcentages r,v,b
cli-opt-folder = nom du dossier de la séance (défaut : AAAA-MM-JJ)
cli-opt-raw-only = laisse les JPEG sur la carte
cli-opt-previews = calcule ensuite vignettes et aperçus
cli-opt-delete-copied = supprime de la carte les fichiers copiés et vérifiés
cli-opt-delete-left = supprime aussi les JPEG laissés par --raw-only
cli-opt-eject = éjecte la carte à la fin
cli-opt-threshold = seuil de similarité (plus haut : piles plus grandes)
cli-opt-config = fichier de réglages (défaut : l'emplacement du système)
