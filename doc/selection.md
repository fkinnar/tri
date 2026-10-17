# Module `tri::selection` — sélection multiple

Les images **marquées** d'une liste, en plus de l'image courante (le curseur). Demandé le
17/10/2026, au clavier **et** à la souris, avec une exigence : tout doit être faisable au
clavier. Le module ne dépend pas de l'affichage (interface graphique, futur TUI) ; l'interface
ne fait que traduire touches et clics en appels.

## Modèle

- Un ensemble de chemins marqués (`Selection`), indépendant des indices : la sélection
  survit à un changement de filtre et à une relecture du dossier (`retain_existing` oublie
  les fichiers supprimés, `renamed` suit un renommage).
- Un **mode visuel** à la vim (`start_visual` / `update_visual` / `end_visual`) : la
  plage va de l'ancre à l'image courante et suit chaque déplacement ; valider garde la
  plage, annuler rétablit les marques d'avant.
- `targets(list, cursor)` : les images sur lesquelles agir, dans l'ordre de la liste — les
  marquées **encore visibles** (on n'agit pas sur ce qu'un filtre masque), sinon l'image
  courante. Toutes les actions passent par là : pas de cas particulier « une image ».

| Fonction | Rôle |
|---|---|
| `toggle(path)` | marquer / démarquer |
| `add_range(list, from, to)` | marquer une plage (Maj+clic) |
| `select_all(list)` / `clear()` | tout / rien |
| `start_visual`, `update_visual`, `end_visual(keep)` | mode visuel |
| `targets(list, cursor)`, `visible(list)` | cibles des actions, nombre visible |

## Dans l'interface

| Touche / geste | Action |
|---|---|
| `Espace` | marquer / démarquer l'image courante (`core.select.toggle`) |
| `v` | mode visuel ; `v` valide, Échap annule (`core.select.visual`) |
| `Ctrl+A` / `Ctrl+Maj+A` | tout / rien (`core.select.all`, `core.select.clear`) |
| Échap | annule le mode visuel, sinon désélectionne, sinon revient à la grille |
| Maj+clic | plage depuis l'image courante |
| Cmd+clic (macOS), Ctrl+clic | ajouter / retirer une image |
| clic simple | sélection effacée, image courante choisie |

Les **notes** (clavier, bandeau) s'appliquent aux cibles, sans avance automatique. Une
paire RAW+JPEG n'est notée qu'une fois (noter une image note l'autre : une couleur
basculerait deux fois) ; une **couleur** a le même effet sur toutes, décidé d'après la
première (ajoutée si elle ne l'a pas, retirée sinon), au lieu d'une bascule image par image
qui mélangerait. **Corbeille** et **éditeur externe** prennent les cibles ; renommer et
afficher dans le Finder, l'image courante.

Affichage : cadre orange (`[gui.colors] marked`, `[gui.grid] marked_frame`) autour des
vignettes marquées, distinct de l'image courante (fond plein) et du survol (gris) ;
« N sélectionnées » (et « mode visuel ») dans la barre d'état ; « sélectionnée » dans la
ligne d'information du plein écran ; le bandeau de notation annonce à combien d'images ses
notes s'appliquent.

## Tests

Unitaires (`src/selection.rs`) : cible = curseur sans marque ; marques dans l'ordre de la
liste, image masquée exclue ; mode visuel dans les deux sens, annulé puis validé ; plage
bornée, tout, renommage. Interface (`gui/src/app.rs`) : Espace sur deux images, note et
couleur appliquées aux deux et pas au curseur, couleur retirée de toutes, Échap
désélectionne ; mode visuel annulé puis validé, corbeille des deux images (« 2
fichiers ») ; Ctrl+A / Ctrl+Maj+A ; Maj+clic, Cmd+clic, clic simple.
