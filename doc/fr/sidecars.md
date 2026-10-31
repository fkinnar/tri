# Module `tri::sidecars` — les formats de sidecars de notes

Demandé par l'utilisateur le 31/10/2026 : quand on note une image, la note doit aller
dans **tous les sidecars que l'on connaît** pour elle, chaque logiciel lisant le sien.
Noter dans `tri` suffit alors : darktable (XMP) et RapidRAW (`.rrdata`) suivent, même
sur une image jamais ouverte dans l'un d'eux. Un format = une implémentation du trait
`Format`, et la liste `formats()` (un « vecteur de trait ») : le XMP ([xmp.md](xmp.md),
lu par darktable, Lightroom, Capture One, digiKam…) et le `.rrdata` de RapidRAW
([rrdata.md](rrdata.md)). Un prochain logiciel à sidecar propre (RawTherapee, `.pp3`)
s'ajoute ici, avec sa section de réglages.

## Le trait `Format`

- `id()` : `"xmp"`, `"rrdata"` (ligne de commande) ; `engine()` : le logiciel qui le
  lit, s'il n'y en a qu'un (le XMP est partagé) ; `owns(sidecar)` : d'après le nom.
- `existing(image, index)` : le sidecar de ce format qu'a la photo, d'après
  `xmp::SidecarIndex` (une liste du dossier, aucun accès disque par image : issue n° 15) ;
  `new_path(image, convention)` : où le créer quand il manque.
- `read`, `write` (le reste du fichier gardé, voir chaque format), `create` (notes
  seules : pour le `.rrdata`, des réglages nuls, comme RapidRAW pour une image notée
  sans être développée).
- `of(sidecar)` : le format d'un sidecar d'après son nom ; `by_id(id)`.

## Quand créer un sidecar manquant : `[sidecars.<logiciel>]`

Un sidecar **existant** reçoit toujours les notes. Un sidecar **manquant** est créé selon
`create` de la section du logiciel (`[sidecars.rapidraw]`, `SidecarSettings`) :

| `create` | Créé quand on note |
|---|---|
| `"auto"` (défaut) | si le logiciel est installé sur la machine, ou si le dossier contient déjà de ses sidecars |
| `"always"` | toujours |
| `"never"` | jamais : à la demande seulement (ci-dessous) |

Le XMP est toujours créé (convention `[xmp] sidecar`). `SidecarSettings::creates(installed,
in_folder)` donne les identifiants à créer ; le carnet les reçoit par
`NoteBook::set_creates` (interface : logiciels trouvés par `develop::Developers`,
`NoteBook::has_format` pour le dossier). Un utilisateur sans RapidRAW ne voit donc jamais
apparaître de `.rrdata`.

## À la demande : `NoteBook::generate`, `tri --sidecars`

Pour un logiciel qu'on n'utilise qu'à l'occasion (`create = "never"`), on génère ce
qu'il lui faut quand on en a besoin : `NoteBook::generate(images, formats, dry_run)` crée,
pour les photos qui ont des notes, les sidecars des formats demandés qu'elles n'ont pas
encore, avec leurs notes ; les développements, les images sans notes et celles déjà
pourvues sont passés (`Generated` : créés, inchangées, passées, erreurs). En ligne de
commande : `tri <dossier> --sidecars rapidraw|xmp|all [--dry-run]`. Dans l'interface :
à venir (par image ou par dossier, un logiciel ou tous).

## Un `.rrdata` sans réglages n'est pas un développement

RapidRAW écrit un `IMG.RAF.rrdata` dès qu'on note une image, avec des réglages nuls ou
réduits à son marqueur `baselineExposure` ; `tri` en crée de même. Ce fichier n'est qu'un
sidecar de notes : `rrdata::is_development` (des réglages autres que vides) décide, et
`developments::find` ne fait un développement `IMG@rr.RAF` que d'un `.rrdata` qui
développe (une copie virtuelle en est toujours un). Pas de pile « 1 dév. » pour une photo
seulement notée.

## Dans le carnet (`notes.md`)

Une photo a **plusieurs sidecars** (`Entry::sidecars`, le plus récent en tête : lu ; tous
écrits) ; un développement n'a que le sien. `edit` écrit les existants puis crée les
manquants des formats actifs ; l'annulation (`write_only`) n'écrit que les existants.

## Tests

`sidecars.rs` : format d'après le nom, politique de création selon le réglage, les
logiciels installés et le dossier. `notes.rs` : noter une photo écrit son XMP **et** son
`.rrdata` ; un `.rrdata` manquant est créé quand la politique le dit, pas sinon ;
`generate` crée les manquants des photos notées seulement, simulation comprise.
`developments.rs` / `media.rs` : un `.rrdata` sans réglages ne fait pas de développement.
