# Module `tri::store` : les données de tri d'un dossier

Ce que `tri` retient d'un dossier de photos, en dehors des notes (toujours dans les
sidecars XMP), et où il le range (11/11/2026, issue n° 378). Usage : décharger une carte
sur une clé USB ou un SSD externe et emporter ce disque d'un ordinateur à l'autre, ou
synchroniser un dossier (Syncthing, un cloud) : tout ce dont `tri` a besoin doit voyager
avec les photos.

## Un petit fichier par sorte

| Fichier | Contenu | Écrit par |
|---|---|---|
| `notes.json` | Mémoire des notes : ce qui a été lu de chaque source, le réglage « notes dans les images » du dossier | `reconcile::Memory` ([reconcile.md](reconcile.md)) |
| `view.json` | Filtres, tri, rendu des vignettes, « Empiler automatiquement », logiciels du filtre « développées », « Sous-dossiers », image courante et piles dépliées (écrites en quittant le dossier) | l'interface (`gui/src/state.rs`, `FolderView`) |
| `tuning.json` | Valeurs des curseurs : sévérité des piles, pauses, progression (`group-*`, écart de temps `group-gap` depuis #414), sévérité du flou (`blur-strictness`), de l'exposition (`clip-over-strictness`, `clip-under-strictness`) | `store::value` / `store::set_value` |
| `choices.json` | Couvertures de piles (`covers`) et développements préférés (`developments`), par **nom de fichier** | `store::chosen` / `store::choose` |

Un fichier par sorte : quand deux ordinateurs modifient le même dossier synchronisé, un
conflit de synchronisation ne touche que le fichier concerné. Des noms de fichiers
seulement, jamais de chemin absolu : un disque monté en `/Volumes/T7`, `/media/u/T7` ou
`E:\` garde tout. Un fichier revenu à sa valeur par défaut est supprimé.

Pas de verrou (il ne protège rien entre deux ordinateurs hors ligne) : chaque écriture
relit le fichier juste avant (`update_json`), ne change que sa propre clé, et remplace le
fichier d'un coup (fichier temporaire, puis renommage : `write_atomic`).

## Un seul interrupteur : « garder les données de tri dans ce dossier / sur ce disque »

Tout ou rien (décision de l'utilisateur) : **le cache et les données ensemble**.

- **Activé** : le fichier `.tri/keep` dans un dossier parent, ou le dossier lui-même
  (`holder(dossier)`, gardé en mémoire par dossier, oublié à chaque changement :
  `changed`). Chaque dossier en dessous garde ses fichiers dans son propre `.tri/` (un
  dossier déplacé emporte ses données), et le cache de tout ce qui est dessous va dans
  `<racine>/.tri/cache` ([cache.md](cache.md)). Le repère est un fichier, pas la présence
  de `.tri/cache` : qui exclut le cache d'une synchronisation (une ligne, `.tri/cache`
  dans `.stignore`) garde l'interrupteur sur l'autre ordinateur.
- **Désactivé** (par défaut) : rien n'est écrit dans le dossier des photos ; les
  fichiers vont dans le dossier de données de `tri`, `folders/<empreinte>/` (empreinte du
  chemin du dossier ; un fichier `folder` redonne le chemin, pour retrouver les dossiers
  d'un disque). Même repli pour un dossier **en lecture seule** (carte verrouillée, NAS),
  même interrupteur activé.
- Sans dossier de données (tests, outils) : le `.tri/` du dossier, plutôt que nulle part.

`keep(data, racine)` pose l'interrupteur et déplace les données des dossiers en dessous
gardées sur la machine ; `release(data, racine)` le retire et les y ramène (parcours du
disque). Les deux sont appelées par `Cache::use_disk` / `Cache::release_disk`, qui
déplacent le cache : depuis l'interface (clic droit sur un dossier ou un disque, boîte de
déchargement), la ligne de commande (`--cache-here` / `--cache-user`) et le réglage
`[cache] location`.

## Mise en ordre à l'ouverture : `settle(data, dossier)`

Appelée à l'ouverture d'un dossier (`Folder::open`, `Folder::scan_counting` pour le
dossier lui-même, `Loading` pour les sous-dossiers de ses images) :

- un cache gardé avec les photos à l'ancienne (`.tri-cache`, dans le dossier ou un parent)
  était un choix explicite : renommé en `.tri/cache`, repère `.tri/keep` posé ;
- les fichiers rangés du mauvais côté (interrupteur changé ici ou par une
  synchronisation, `.tri/notes.json` écrit d'office avant le 11/11/2026) passent de
  l'autre ; présents des deux côtés, le plus récent gagne ;
- ce que le dossier de données gardait pour ce dossier sous son chemin absolu avant le
  11/11/2026 (`folder-views`, `*-strictness.tsv`, `group-*.tsv`, `stack-covers.txt`,
  `preferred-developments.txt`, `notes/<empreinte>.json`) rejoint ses fichiers et quitte
  les anciens (supprimés une fois vides).

Au mieux : ce qui ne peut pas bouger (lecture seule) reste où il est.
`relocate(data, de, vers)` : après le renommage ou le déplacement d'un dossier depuis
`tri`, ses données gardées sur la machine suivent (celles de son `.tri/` sont parties avec
lui).

## Ce qui reste sur la machine

Favoris, réglages, raccourcis, partages réseau, dernier dossier, case « Sous-dossiers »,
destinations récentes, journal d'export, historique de la palette : ils appartiennent à
l'ordinateur, pas à un dossier.

Montrer où sont les données d'un dossier (« sur le disque » / « sur cet ordinateur ») est
prévu dans le résumé du dossier (#388).
