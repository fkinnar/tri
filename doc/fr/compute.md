# Calculs choisis sur un dossier (`tri::compute`)

Issue #555 (idée de l'utilisateur, 21/11/2026) : préparer un dossier **sans l'ouvrir ni
attendre** (avant une recherche de palettes, avant de trier sur un portable, avant de
débrancher un disque lent), en choisissant **quoi** calculer.

## Les calculs (`Kind`)

| Identifiant | Ce qui est calculé | Niveaux de cache |
|---|---|---|
| `previews` | vignettes et aperçus boîtier (image plate : son aperçu) | `Thumbnail`, `Embedded` / `Standard` |
| `raw` | rendus RAW (RAW seulement) | `Standard` |
| `exposure` | histogrammes | `Exposure` |
| `sharpness` | netteté | `Sharpness` |
| `signatures` | empreintes des piles | `Similarity` |
| `palette-camera`, `palette-raw`, `palette-darktable`… | palettes d'une source | `Palette(source)` |

## `run`

Chaque image passe par le producteur du moteur (`Producer`, le même que l'interface) :
mêmes clés de cache, ce qui y est déjà est sauté. Bilan (`Summary`) : calculées, déjà en
cache, échecs (et la raison du premier), interrompu. `clear` efface d'abord ce que le
calcul remplit (« recréer »).

Plusieurs calculs tournent en même temps (une tâche chacun), mais le travail **lourd**
(LibRaw, logiciels de développement : `scheduler::is_heavy`) passe **l'un après l'autre**
(`heavy_turn`, un verrou commun) : ils occupent déjà tous les cœurs, et plusieurs rendus à
la fois sont bien plus lents qu'en série (mesuré le 16/10/2026).

## Priorité

Un calcul demandé passe **avant** le travail de fond du dossier ouvert (décision de
l'utilisateur) : l'interface met ce fond en attente (`Scheduler::set_background_paused`)
tant qu'un calcul tourne ; l'image affichée, ses voisines et les vignettes restent servies.

## Interface et ligne de commande

Clic droit sur un dossier, « Calculer » : une entrée par calcul, « Tout calculer » et
« Tout recalculer » ; chacune est une action (`core.folders.compute.*`,
`plugin.<logiciel>.compute_palettes`), donc aussi dans `:`, `?` et `[keys]`. Chaque calcul
a sa ligne dans la barre d'état, avec son bouton d'arrêt (voir [gui.md](gui.md)).

`tri <dossier> --compute previews,exposure,palette-camera [--recreate]`.

## Tests

Faux producteur : cache sauté, rendus RAW pour les RAW seulement, échecs comptés,
interruption ; identifiants aller-retour. Interface : une tâche réelle (calculées, puis
déjà en cache, puis recréées), les actions, le menu.
