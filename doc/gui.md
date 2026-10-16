# Interface graphique : `tri-gui`

Application egui/eframe, dans son propre paquet (`gui/`) du workspace Cargo. Le moteur
(`tri`) ne dépend pas d'egui : la séparation moteur / affichage du `CLAUDE.md` est ainsi
garantie par Cargo lui-même.

```sh
cargo run --release -p tri-gui -- <dossier>
```

## Ce qui est en place

- **Grille de vignettes** du dossier, alimentée en tâche de fond par le
  [scheduler](scheduler.md) ; seules les lignes visibles sont dessinées
  (`ScrollArea::show_rows`), ce qui tient des milliers d'images.
- **Barre d'état** : dossier, nombre d'images (dont RAW), filtre, progression des vignettes et
  des aperçus standard, nom de l'image sélectionnée.
- **Sélection** au clavier ou à la souris ; le scheduler est prévenu (`set_focus`), donc
  l'aperçu de l'image sélectionnée et de ses voisines passe en tête de file.
- **Filtre par type** (touche `f`) : tout → RAW → plates. La sélection est conservée si
  l'image reste visible.
- **Aide** (`F1` ou `?`) : chaque action avec ses touches et son identifiant.
- Taille des aperçus standard **« Auto »** : plus grand côté de l'écran, en pixels physiques.
- Thème sombre par défaut.

Le plein écran et la navigation entre images affichées viennent à l'étape suivante.

## Modules

| Fichier | Rôle |
|---|---|
| `main.rs` | Arguments, fenêtre, thème |
| `app.rs` | `TriApp` : état, événements du moteur, dessin |
| `actions.rs` | Actions nommées et raccourcis |
| `grid.rs` | Calculs purs de la grille : colonnes, navigation, défilement |
| `textures.rs` | Cache LRU des textures |

### Actions et raccourcis (`actions.rs`)

Une touche n'appelle jamais une fonction directement : elle est traduite en `Action`, avec
un identifiant stable (`core.nav.next`, `core.filter.cycle`…), comme décidé dans le
`CLAUDE.md`. Le futur fichier de raccourcis JSON associera des touches à ces identifiants ;
la souris déclenche les mêmes actions.

Deux profils sont livrés et actifs ensemble (ils ne se recouvrent pas) :

| Action | Classique | Vim |
|---|---|---|
| `core.nav.prev` / `core.nav.next` | ← / → | h / l |
| `core.nav.up` / `core.nav.down` | ↑ / ↓ | k / j |
| `core.nav.first` / `core.nav.last` | Début / Fin | g / Maj+G |
| `core.nav.page_up` / `core.nav.page_down` | Page préc. / suiv. | u / d |
| `core.filter.cycle` | f | f |
| `core.app.help` | F1 | ? |
| `core.app.quit` | q | q |

Avec Ctrl, Cmd ou Alt, aucune action n'est déclenchée : ces combinaisons restent au système
(Cmd+Q…).

### Grille (`grid.rs`)

Fonctions pures, testées sans fenêtre :

- `Layout::fit(largeur, cellule, espace)` : nombre de colonnes ; `rows`, `row_pitch`,
  `rows_in`, `width` ;
- `navigate(sélection, n, layout, lignes_par_page, action)` : haut/bas gardent la colonne ;
  vers le bas depuis l'avant-dernière ligne, on va à la dernière image si la colonne n'existe
  pas sur la dernière ligne ;
- `scroll_to_show(layout, ligne, décalage, hauteur)` : défilement minimal pour rendre une
  ligne visible.

Le défilement est mémorisé comme une *intention* (`Reveal::Minimal` après un déplacement,
`Reveal::Top` après un changement de liste) et résolu au moment de dessiner la grille, quand
la largeur, donc le nombre de colonnes, est connue. Le calculer plus tôt donnait une mauvaise
ligne au démarrage (bug vu sur une capture).

### Textures (`textures.rs`)

Les vignettes visibles sont chargées depuis le cache disque (~1–2 ms chacune) dans un
budget de **8 ms par image affichée** : au-delà, le chargement reprend à l'image suivante,
sans figer l'interface. Les textures vivent dans un **cache LRU de 600 entrées**, indexé par
chemin (il survit donc à un changement de filtre). Sans borne, 5 000 vignettes de 400 px
occuperaient ~2 Go de mémoire graphique.

## Développement : capture automatique

```sh
TRI_GUI_SCREENSHOT=/tmp/grille.png TRI_GUI_SELECT=40 cargo run --release -p tri-gui -- <dossier>
```

L'application se capture elle-même (contenu de sa fenêtre seulement, pas l'écran) une fois
toutes les vignettes calculées et celles qui sont visibles affichées, au plus tard après
60 s, puis se ferme. `TRI_GUI_SELECT` choisit l'image sélectionnée au démarrage. Sert à
vérifier le rendu sans intervention.

## Tests

Tests unitaires du paquet (`cargo test -p tri-gui`) : identifiants d'actions uniques et
préfixés, profils complets et sans doublon, union des profils, modificateurs système
ignorés, noms de touches de l'aide ; colonnes, navigation (bords, colonnes, dernière ligne
incomplète, pages, grille vide), défilement ; LRU (éviction, remplacement, capacité
minimale) ; troncature des noms, mise à jour des statuts. Le rendu est vérifié par capture
automatique.
