# Module `tri::folder`

Un **dossier ouvert** : ce qu'une interface (graphique, TUI) ou la ligne de commande
manipule quand on « ouvre un dossier ».

- `Folder::open(dir, profondeur, convention)` : chemin rendu absolu (sans résoudre les
  liens : `std::path::absolute`, pas de préfixe `\\?\` sous Windows), images
  ([media.md](media.md)), notes ([notes.md](notes.md)) ;
- `Folder::filtered(type, règle)` : filtre par type puis filtre sur les notes, combinés ici
  une fois pour toutes (avant : dans l'interface) ;
- `Folder::default()` / `is_none()` : aucun dossier ouvert (interface lancée sans dossier).

- `remember(données, dossier)` / `last(données)` : dernier dossier ouvert, mémorisé dans
  le fichier `last-folder` du dossier de données (`EngineSettings::data_dir` :
  `[data] dir`, sinon celui du système), relu seulement s'il existe toujours ; chemin en
  octets bruts sous Unix (noms non UTF-8), UTF-8 ailleurs. L'interface rouvre ce dossier
  quand elle est lancée sans argument (`[gui] reopen_last`).

- `subfolders(données)` / `remember_subfolders(données, oui)` : la case « Sous-dossiers »
  du panneau des dossiers (20/10/2026), gardée d'une session à l'autre (fichier
  `subfolders`) ; l'interface ouvre alors le dossier sur `1 + [gui.folders]
  subfolders_depth` niveaux (`Folder::open`, `media::scan` : dossiers cachés écartés).
- `folder_value` / `remember_folder_value` : une valeur retenue par dossier (seuil des
  piles, sévérité du flou), une ligne « valeur, tabulation, chemin » dans un fichier du
  dossier de données.

Champs publics (`dir`, `files`, `notes`) : un dossier ouvert n'a pas d'invariant à
protéger entre eux, et l'interface modifie les notes (`folder.notes.edit`).

Pourquoi un module à part : le moteur doit faire le vrai travail et rester indépendant de
l'interface (TUI, commandes comme « décharger une carte » sans interface, à venir).
