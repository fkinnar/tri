# Module `tri::folder`

Un **dossier ouvert** : ce qu'une interface (graphique, TUI) ou la ligne de commande
manipule quand on « ouvre un dossier ».

- `Folder::open(dir, profondeur, convention)` : chemin rendu absolu (sans résoudre les
  liens : `std::path::absolute`, pas de préfixe `\\?\` sous Windows), images
  ([media.md](media.md)), notes ([notes.md](notes.md)) ;
- `Folder::scan(dir, profondeur, convention)` (23/10/2026, issue n° 15) : comme `open`,
  sans lire les notes, qui restent vides en attendant (`NoteBook::pending`). Lister un
  dossier est rapide ; lire ses sidecars, non : 2 820 images sur un disque dur externe,
  15 s de fenêtre vide. L'interface ouvre ainsi tout dossier, puis lance
  `Loading::start(&dossier, profondeur, surveillance)` : un fil `tri-folder` lit les notes
  (`NoteBook::load_counting`, progression `progress()` → (lues, total)) et, si demandé,
  l'état du dossier pour sa surveillance (`watch::Snapshot`, un accès disque par fichier,
  autant le faire là). `try_take()` rend `Loaded { notes, snapshot }` une fois fini ;
  l'interface reprend les notes par `NoteBook::adopt` ;
- `Folder::scan_counting(dir, profondeur, convention, stop, trouvées)` (29/10/2026) :
  `scan` interruptible, qui compte les images trouvées (`media::scan_counting`). Sur un
  NAS, même la liste peut prendre du temps : l'interface la lit dans un fil, l'attend
  `[gui.folders] open_wait_ms` (150 ms : un disque local ne voit rien changer), puis la
  laisse finir en fond avec une boîte au centre (« Ouverture de « séance »… 340 images
  trouvées », Annuler ou Échap) ; le dossier d'avant reste affiché en attendant. Le
  dossier du lancement (en argument, ou le dernier) suit le même chemin (30/10/2026 : sur
  un NAS, la fenêtre attendait la liste plusieurs secondes) : la fenêtre s'ouvre tout de
  suite, l'arborescence se déplie jusqu'au dossier une fois lu, et un dernier dossier
  introuvable est dit au centre (rouvert à son retour) ;
- `Folder::filtered(type, règle)` : filtre par type puis filtre sur les notes, combinés ici
  une fois pour toutes (avant : dans l'interface) ;
- `Folder::default()` / `is_none()` : aucun dossier ouvert (interface lancée sans dossier).

- `remember(données, dossier)` / `last(données)` : dernier dossier ouvert, mémorisé dans
  le fichier `last-folder` du dossier de données (`EngineSettings::data_dir` :
  `[data] dir`, sinon celui du système), relu seulement s'il existe toujours ; chemin en
  octets bruts sous Unix (noms non UTF-8), UTF-8 ailleurs. L'interface rouvre ce dossier
  quand elle est lancée sans argument (`[gui] reopen_last`). `remembered(données)` le
  rend même s'il a disparu (disque débranché : l'interface le dit, issue n° 66).

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
