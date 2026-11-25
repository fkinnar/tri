# Mode léger (`tri::light`, #597)

Sur un NAS ou iCloud, calculer tout le cache d'un dossier revient à relire chaque RAW
plusieurs fois à travers le réseau : aperçus boîtier, puis exposition (le RAW entier),
netteté, piles, palettes. En **mode léger**, seules les **vignettes** sont calculées en
fond ; le reste attend qu'on en ait besoin.

## Ce qui change

- **Vignettes** : calculées en fond, comme partout.
- **Aperçu de l'image affichée et de ses voisines** (aperçu boîtier d'un RAW, aperçu
  standard d'une image plate) : demandé dès qu'elles sont affichées
  (`Scheduler::set_light`, puis `set_shown`), jamais pour tout le dossier.
- **Analyses** (exposition, netteté, piles, couleurs) : celles réglées « en fond »
  passent « à la demande » (`light::lighten`) : leur outil reste grisé tant que le
  dossier n'est pas calculé, ce qui est déjà dans le cache est lu.
- **Une image ouverte en plein écran** (double-clic, Entrée) : tout son cache, analyses
  comprises (`Scheduler::request_all`). Sur 2000 photos dont on garde 25, seules ces 25
  coûtent un calcul complet.
- **« Tout calculer »** au clic droit : sur un dossier, pour ce dossier seulement (jamais
  tout le NAS d'un coup) ; sur une image ou une sélection, pour elles.

## Où il s'applique

Section `[light]` :

| Clé | Défaut | Rôle |
|---|---|---|
| `network` | `true` | Emplacements réseau (partages montés, iCloud) en mode léger d'office |
| `folders` | `[]` | Dossiers en mode léger, sous-dossiers compris |
| `except` | `[]` | Dossiers sortis du mode léger, sous-dossiers compris |

La règle **la plus profonde** l'emporte (`LightSettings::root_of`) : un dossier ajouté
vaut pour ses sous-dossiers, une exception aussi ; à profondeur égale, un choix de
l'utilisateur l'emporte sur l'emplacement réseau. Le clic droit d'un dossier du panneau
(**Mode léger** / **Quitter le mode léger**, action `core.folders.light`, sans touche par
défaut) réécrit ces listes au plus court (`LightSettings::toggled`) : revenir sur un choix
retire l'entrée au lieu d'en ajouter une contraire.

## Repères

- Panneau des dossiers : une **plume** à côté de la racine du mode léger (le dossier
  choisi, ou le partage), une plume barrée sur une exception (`LightSettings::mark`).
- Barre d'état : la plume et « léger » à côté du nom du dossier ; au survol, la racine.
