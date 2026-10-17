# Module `tri::folder`

Un **dossier ouvert** : ce qu'une interface (graphique, TUI) ou la ligne de commande
manipule quand on « ouvre un dossier ».

- `Folder::open(dir, profondeur, convention)` : chemin rendu absolu (sans résoudre les
  liens : `std::path::absolute`, pas de préfixe `\\?\` sous Windows), images
  ([media.md](media.md)), notes ([notes.md](notes.md)) ;
- `Folder::filtered(type, règle)` : filtre par type puis filtre sur les notes, combinés ici
  une fois pour toutes (avant : dans l'interface) ;
- `Folder::default()` / `is_none()` : aucun dossier ouvert (interface lancée sans dossier).

Champs publics (`dir`, `files`, `notes`) : un dossier ouvert n'a pas d'invariant à
protéger entre eux, et l'interface modifie les notes (`folder.notes.edit`).

Pourquoi un module à part : le moteur doit faire le vrai travail et rester indépendant de
l'interface (TUI, commandes comme « décharger une carte » sans interface, à venir).
