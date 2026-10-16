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
- **Barre d'état** : dossier, nombre d'images (dont RAW), filtre, progression des vignettes,
  des aperçus « JPEG boîtier » (masquée s'il n'y a aucun RAW) et des aperçus standard, nom
  de l'image sélectionnée.
- **Sélection** au clavier ou à la souris ; le scheduler est prévenu (`set_focus`), donc
  l'aperçu de l'image sélectionnée et de ses voisines passe en tête de file.
- **Filtre par type** (touche `f`) : tout → RAW → plates. La sélection est conservée si
  l'image reste visible.
- **Aide** (`F1` ou `?`, fermeture par Échap) : chaque action avec un libellé compréhensible
  et ses touches ; l'identifiant technique n'apparaît qu'au survol. L'aide est **modale** :
  tant qu'elle est ouverte, seules Échap, l'aide et Quitter agissent.
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
| `core.view.back` | Échap | Échap |
| `core.app.help` | F1 | ? |
| `core.app.quit` | q | q |

Avec Ctrl, Cmd ou Alt, aucune action n'est déclenchée : ces combinaisons restent au système
(Cmd+Q…).

Chaque action a deux noms : un **identifiant** stable pour les fichiers et le code
(`core.view.back`), et un **libellé** pour l'utilisateur (« Fermer / revenir »).
`core.view.back` ferme ce qui est au premier plan : l'aide aujourd'hui, le plein écran
ensuite.

Les touches sont affichées par des **noms français en toutes lettres** (`key_label` :
« Flèche gauche », « Début », « Page suiv. », « Échap »), en police à chasse fixe pour que
« l » et « I » restent distincts. Pas de symboles : « ⏴ » (nom fourni par egui) et même
« ← » manquent dans la police par défaut d'egui. Les tests d'interface échouent sur tout
caractère sans glyphe, ce qui garantit que l'aide s'affiche partout.

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
TRI_GUI_SCREENSHOT=/tmp/aide.png TRI_GUI_SELECT=40 TRI_GUI_ACTIONS=core.app.help \
  cargo run --release -p tri-gui -- <dossier>
```

L'application se capture elle-même (contenu de sa fenêtre seulement, pas l'écran), puis se
ferme. La capture a lieu une fois toutes les vignettes calculées et celles qui sont
visibles affichées, **et au moins 1 s après le démarrage**, au plus tard après 60 s. Sans ce
délai, une capture faite pendant le fondu d'ouverture d'une fenêtre la montrait à
moitié transparente.

- `TRI_GUI_SELECT=<n>` : image sélectionnée au démarrage ;
- `TRI_GUI_ACTIONS=<id>,<id>...` : actions exécutées à la première image, par leur
  identifiant (`core.nav.next`, `core.app.help`…). Un identifiant inconnu est signalé et
  ignoré. Permet de capturer n'importe quel état de l'interface.

## Tests

**Tests d'interface** avec [egui_kittest](https://crates.io/crates/egui_kittest) (banc de
test officiel d'egui) : l'application tourne sans fenêtre, sur un dossier et un cache
temporaires, et reçoit des touches simulées. Vérifié : l'aide s'ouvre et se ferme avec F1 et
`?` (avec ou sans Maj), Échap la ferme, elle est modale, et elle affiche les libellés et
non les identifiants. Ces tests ont trouvé un glyphe manquant (« ⏴ ») dans l'aide, et
l'absence de fermeture par Échap.

Les tests de l'interface doivent être dans le paquet (`src/`, `#[cfg(test)]`) et non dans
`tests/` : `tri-gui` est un binaire, ses modules ne sont pas importables depuis `tests/`.

Tests unitaires du paquet (`cargo test -p tri-gui`) : identifiants d'actions uniques et
préfixés, profils complets et sans doublon, union des profils, modificateurs système
ignorés, noms de touches et libellés de l'aide, identifiants réversibles ; colonnes, navigation (bords, colonnes, dernière ligne
incomplète, pages, grille vide), défilement ; LRU (éviction, remplacement, capacité
minimale) ; troncature des noms, mise à jour des statuts. Le rendu est vérifié par capture
automatique.
