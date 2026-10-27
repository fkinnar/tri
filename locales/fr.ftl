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
metadata-focal-equivalent = { $focal } mm (éq. 24 × 36 : { $equivalent } mm)
metadata-focal-value = { $focal } mm
metadata-flash = Flash
metadata-flash-fired = déclenché
metadata-flash-not-fired = non déclenché
metadata-date = Date
metadata-date-value = { $day }/{ $month }/{ $year } à { $time }
metadata-dimensions = Dimensions
metadata-dimensions-value = { $width } × { $height } ({ $mpx } Mpx)
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
files-not-in-trash = { $path } n'est plus dans la corbeille
history-notes-changed = { $name } : notes modifiées ailleurs depuis (rien n'a été changé)
history-missing = { $name } est introuvable (déplacé ou supprimé ailleurs)
history-modified = { $name } a été modifié ailleurs depuis (rien n'a été changé)
history-nothing = rien à annuler
history-trash-one = { $name } mis à la corbeille
history-create-dir = dossier { $name } créé
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
settings-defaults-used = { $warning } ; réglages par défaut utilisés
settings-file-error = configuration { $path } : { $error }

## Ligne de commande

cli-usage = 
    usage :
      tri <dossier> [--raw|--flat] [--depth N]
      tri <dossier> --previews [--raw|--flat] [--depth N] [--side N] [--thumb N] [--workers N] [--mixed] [--cache DOSSIER]
      tri [<dossier>] --clear-cache [<niveau>] [--depth N] [--cache DOSSIER]
      tri --cache-usage [--cache DOSSIER]
      tri <dossier> --cache-here|--cache-user [--cache DOSSIER]
      tri --check-update
      tri <dossier> --convert-xmp darktable|adobe [--dry-run] [--raw|--flat] [--depth N]
      tri <dossier> --exposure [--raw|--flat] [--depth N]
      tri <dossier> --metadata [--raw|--flat] [--depth N]
      tri <dossier> --groups [--threshold X]
      tri <dossier> --sharpness [--raw|--flat] [--depth N]
      tri --offload <carte> [--to <racine>] [--folder <nom>] [--raw-only] [--dry-run] [--previews] [--delete-copied [--delete-left]] [--eject]
      tri --rename <image> <nom> [--dry-run]
      tri --delete <image>... [--permanent] [--dry-run]
      tri --copy|--move <image>...|<dossier> --to <dossier> [--dry-run]
      tri --reveal <image>
      tri --edit <image>...
      tri --export <préréglage> <image>... [--to <dossier>] [--libraw|--embedded] [--passthrough] [--dry-run]
    option commune : --config FICHIER (réglages ; défaut : emplacement du système)
cli-unknown-convention = convention inconnue : { $value } (darktable ou adobe)
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
cli-rename-shared-copy = (copie : partagé)
cli-delete-trash = corbeille :
cli-delete-permanent = supprimé :
cli-delete-kept = gardé (partagé) :
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
cli-groups-header = image	suivante	distance	écart (s)
cli-groups-summary = { $n } images, { $groups } groupes au seuil { $threshold } ({ $ms } ms par empreinte)
cli-cache-cleared = cache vidé ({ $level })
cli-cache-here = cache gardé dans { $place }
cli-cache-released = cache retiré de { $place }
cli-cache-moved = { $n } entrées déplacées
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
develop-unknown-engine = logiciel de développement inconnu : « { $value } » (auto, darktable, rapidraw)
develop-unavailable = { $engine } introuvable (réglage [develop] { $id })
develop-rapidraw-open = interface ouverte : l'export en ligne de commande ne tourne pas en même temps, fermez-la puis recommencez
develop-rapidraw-black = image toute noire : l'interface de RapidRAW est sans doute ouverte, fermez-la puis recommencez
develop-failed = { $engine } : { $error }
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
