# Module `tri::files` — actions sur les fichiers

Renommer, supprimer (corbeille), afficher dans le gestionnaire de fichiers, ouvrir dans un
éditeur externe (premier lot demandé le 17/10/2026). Tout le travail est dans le moteur :
l'interface (`gui/src/files.rs`) ne fait que la saisie et la confirmation, et la ligne de
commande offre les mêmes actions.

## Principe : une image ne voyage jamais seule

- Ses **sidecars XMP** la suivent toujours : sans eux, ses notes seraient perdues, ou
  attribuées plus tard à une autre image du même nom.
- Ses **clones** et les **réglages des logiciels de développement** la suivent aussi
  (29/10/2026 : un clone darktable restait derrière l'image déplacée) : clones darktable
  `IMG_01.RAF.xmp`, `IMG_02.RAF.xmp`… (un sidecar d'une vraie image `IMG_01.RAF` reste à
  elle), RawTherapee `IMG.RAF.pp3`, ART `IMG.RAF.arp`, Capture One
  `CaptureOne/Settings…/IMG.RAF.cos` (copié ou déplacé dans le même sous-dossier de la
  destination, renommé à sa place). `xmp::SidecarIndex::companions` les rassemble ; les
  notes, elles, ne lisent que les sidecars de l'image. Les copies virtuelles de Lightroom
  ne vivent que dans son catalogue : rien à faire suivre. RapidRAW `IMG.RAF.rrdata` suit
  aussi (30/10/2026).
- Un **clone darktable** affiché dans la grille (voir `media.md`) ne se sépare jamais de
  son image (choix de l'utilisateur, 30/10/2026 : « tri ne peut pas scinder les
  versions ») : le copier, le déplacer ou le renommer agit sur l'original, et tous ses
  clones suivent (`Mer_01.RAF.xmp` après avoir renommé `IMG_01.RAF` en « Mer »). Le
  **supprimer** ne supprime que son sidecar, comme dans darktable ; l'original reste.
- L'autre image de sa **paire RAW+JPEG** (même nom à l'extension près, sans tenir compte de
  la casse) la suit aussi, par défaut (`[files] pairs = true`), comme pour les notes : on
  rejette une photo, pas un fichier. Les boîtes **Supprimer** et **Copier / déplacer
  vers…** ont une case « avec l'autre fichier de la paire » quand la sélection en
  entraîne d'autres (`adds_pairs`, 21/10/2026 : supprimer tous les JPEG d'une séance
  RAW+JPEG emportait les RAW) ; la décocher met à jour la liste.
- Un sidecar **partagé** (`IMAGE.xmp`, convention Adobe, commun au RAF et au JPG) n'est
  touché que si toutes les images qui l'utilisent le sont. Sans les paires, renommer le RAF
  seul **copie** ce sidecar sous le nouveau nom (les deux images gardent leurs notes) ;
  supprimer le RAF seul le laisse en place.

## Préparer, puis agir

Chaque action se prépare d'abord, pour montrer avant d'agir la liste des fichiers touchés,
et pour que les erreurs prévisibles arrivent avant la première modification.

| Fonction | Rôle |
|---|---|
| `adds_pairs(images) -> bool` | les paires ajoutent-elles des fichiers (l'autre image pas choisie) ? |
| `group(images, pairs) -> Group` | images (paires comprises), sidecars, clones et réglages à elles seules (`sidecars`), sidecars partagés laissés (`shared`) |
| `plan_rename(image, new_stem, settings) -> Result<Rename>` | nouveaux chemins (`moves`) et copies de sidecars partagés (`copies`) ; refuse un nom vide, avec `/` ou `\`, `.` / `..` (`InvalidName`) ou déjà pris (`Exists`) ; changer seulement la casse est permis |
| `rename(&Rename)` | exécute ; jamais d'écrasement (revérifié juste avant chaque fichier) ; sur une erreur, ce qui a été renommé est remis en place et les copies effacées |
| `plan_delete(images, settings) -> Group` | fichiers à supprimer |
| `delete(&Group, trash)` | corbeille (crate [`trash`](https://crates.io/crates/trash)) ou suppression définitive |
| `reveal(path)` | gestionnaire de fichiers, fichier sélectionné |
| `editor_command(editor, files)` / `open_with(editor, files)` | éditeur externe (commande construite à part, pour être testée sans rien lancer) ; programmes lancés sans entrée ni sorties (leurs traces ne vont pas dans le terminal de `tri`) |

Le renommage garde l'extension : on saisit le nom sans elle. `IMG.RAF`, `IMG.JPG`,
`IMG.RAF.xmp`, `IMG.xmp` deviennent `NOUVEAU.RAF`, `NOUVEAU.JPG`, `NOUVEAU.RAF.xmp`,
`NOUVEAU.xmp` : chaque nom perd ses premiers caractères (l'ancien nom) au profit du
nouveau, ce qui couvre les deux conventions de sidecar.

Le cache des aperçus est indexé par chemin : une image renommée est recalculée ; les
entrées de l'ancien nom deviennent orphelines (les 1:1 sont limités en taille, le reste
s'efface avec `tri --clear-cache`).

## Par système

| | Afficher | Corbeille | Application par défaut |
|---|---|---|---|
| macOS | `open -R` | Finder (via `trash` : « Remettre » fonctionne) | `open` |
| Linux | `org.freedesktop.FileManager1.ShowItems` par `gdbus` (Nautilus, Nemo, Dolphin…), sinon `xdg-open` du dossier | spécification freedesktop | `xdg-open` |
| Windows (non testé) | `explorer /select,` | corbeille du système | `cmd /C start` |

Les programmes lancés ne sont pas attendus ; un fil récupère leur fin (pas de processus
zombie).

## Réglages (`[files]`)

| Clé | Défaut | Rôle |
|---|---|---|
| `pairs` | `true` | la paire RAW+JPEG suit |
| `trash` | `true` | corbeille ; `false` : suppression définitive |
| `editor` | `[]` | programme puis arguments, `{files}` remplacé par les fichiers (ajoutés à la fin sinon) ; vide : application par défaut |
| `allow_actions` | `false` | l'interface peut renommer, copier, déplacer et supprimer images et dossiers |

Exemples d'éditeur : `["darktable"]`, `["open", "-a", "darktable"]` (macOS),
`["gimp", "{files}"]`.

**Actions interdites par défaut** (choix de l'utilisateur, 30/10/2026). Un logiciel de
développement (darktable, Lightroom, Capture One…) range les images dans sa
bibliothèque ou son catalogue et les attend à leur place : les renommer, les déplacer ou
les supprimer depuis `tri` les lui fait perdre. Sans `allow_actions`, l'interface grise
ces actions (bulle : comment les autoriser) et un raccourci le dit dans la barre d'état ;
une case « Actions sur les fichiers » du panneau de droite (Grille) les autorise et
l'écrit dans la configuration. Autorisées, les boîtes qui renomment, déplacent ou
suppriment le rappellent (copier ne casse rien). Le choix est le même pour tout, images
et dossiers, sans regarder les sidecars (une détection d'après les XMP a été écartée :
`tri` en écrit lui-même pour les notes, la règle devenait confuse). Pas concernés : créer
un dossier, le déchargement d'une carte, la ligne de commande (une commande tapée est
un choix explicite).

## Ligne de commande

```sh
tri --rename <image> <nom> [--dry-run]
tri --delete <image>... [--permanent] [--dry-run]
tri --reveal <image>
tri --edit <image>...
```

## Tests

Unitaires (`src/files.rs`) : groupe d'une paire avec ses deux sidecars, sidecar partagé
gardé sans les paires, noms invalides, commande de l'éditeur (`{files}`, ajout en fin,
application par défaut), URI encodée. Intégration (`tests/files.rs`), sur des fichiers
temporaires : renommage d'une paire et de ses sidecars avec les notes relues, nom pris
refusé sans rien changer, sidecar partagé copié, changement de casse seul, suppression
(définitive) de la paire et de ses sidecars sans toucher au reste. Le test de la vraie
corbeille est `#[ignore]` : il met un fichier dans la corbeille de l'utilisateur.

## Copier / déplacer vers un dossier (18/10/2026)

`plan_transfer(images, dossier, mode, réglages)` prépare, `transfer(plan, progression)`
exécute. Comme pour le renommage, la **paire** (`[files] pairs`) et les **sidecars XMP**
suivent, sous le même nom ; un sidecar partagé avec une image qui reste (`IMAGE.xmp`
d'une paire dont une seule image part) est **copié**, jamais déplacé : les deux images
gardent leurs notes.

- **Jamais d'écrasement** : un nom déjà présent dans la destination fait échouer le plan,
  avant toute écriture. Destination absente, ou dossier d'origine : refusées aussi.
- **Copie vérifiée** : écrite sous un nom temporaire (`.NOM.tri-tmp`), relue et comparée à
  l'original (BLAKE3, comme le déchargement), **date de modification gardée** (elle
  départage les deux conventions de sidecars, voir `xmp.md`), puis renommée.
- **Déplacement** : simple renommage sur le même volume ; vers un autre volume (disque,
  carte, NAS : `CrossesDevices`), copies vérifiées de **tous** les fichiers, puis
  suppression des originaux, jamais avant.
- **En cas d'erreur**, ce qui a été fait est défait (copies supprimées, renommages remis
  en place).
- Destinations récentes : `recent_destinations` / `remember_destination` (fichier
  `destinations` du dossier de données, `[files] recent` au plus, disparues omises).
  Complétion de la saisie : `complete_dir` (sous-dossiers, casse ignorée, cachés
  seulement après un « . ») et `common_prefix`.
- Ligne de commande : `tri --copy|--move <image>...|<dossier> --to <dossier> [--dry-run]`.
- Interface : `y` (copier, *yank* de vim) et `Ctrl+X` (déplacer, « couper » ; `x` supprime,
  comme dans vim), sur la sélection,
  et boutons du clic droit ; boîte avec champ (Tab complète, flèches haut / bas : récentes,
  dossier absent créé s'il n'en manque qu'un niveau), bascule copier / déplacer,
  progression pendant la copie (dans un fil : la fenêtre reste vivante). « Continuer en
  arrière-plan » (25/10/2026, issue n° 72) referme la boîte : la copie continue, sa
  progression est dans la barre d'état (un clic rouvre la boîte), la fin y est annoncée
  et le dossier relu ; les autres boîtes (renommer, supprimer…) restent utilisables, et
  plusieurs copies peuvent tourner ainsi (`TriApp::background_transfers`).

## Copier / déplacer un dossier (20/10/2026)

`plan_transfer_dir(dossier, dest, mode)` prépare la copie ou le déplacement d'un dossier
entier, sous-dossiers compris (`DirTransfer` : fichiers, sous-dossiers à créer, cible
`dest/nom`) ; `transfer_dir(plan, progress)` l'exécute. Le cache de `tri` gardé dans
le dossier (`.tri-cache`, #55) ne suit pas : rangé par chemin, il ne servirait à rien
ailleurs, et ses fichiers temporaires allaient et venaient pendant la copie (#157 :
« No such file ») ; après un déplacement vers un autre volume, celui resté dans la
source est supprimé.

- **Refusés** : un dossier dans lui-même ou un de ses sous-dossiers (`IntoItself`), là
  où il est déjà (`SameFolder`), par-dessus un dossier du même nom (`Exists`).
- **Déplacement sur le même volume** : un seul renommage, instantané.
- **Autre volume, ou copie** : sous-dossiers créés, copies vérifiées de chaque fichier
  (comme pour les images) ; pour un déplacement, les originaux ne partent qu'ensuite, et
  les sous-dossiers vidés avec eux. Un fichier apparu entre-temps dans la source y reste,
  avec ses dossiers. Sur une erreur, ce qui a été écrit est retiré.
- **Annulable** (`history::Step::TransferDir`) : un déplacement revient à sa place ; une
  copie n'est retirée que telle qu'on l'a laissée (rien d'ajouté ni de modifié depuis).
- Interface : panneau des dossiers, `Ctrl+C` (classique) / `y` (vim) et `Ctrl+X`, ou
  clic droit « Déplacer vers… » / « Copier vers… » ; la même boîte que pour les images.
  **Glisser-déposer** : un dossier, ou des images de la grille (la sélection si l'image
  glissée en fait partie), lâchés sur un dossier du panneau ouvrent cette boîte, remplie,
  qui confirme toujours ; copier en tenant **Option (⌥)** sur macOS (comme le Finder ;
  Ctrl + clic y est le clic droit), **Ctrl** ailleurs (Nautilus, Explorateur). Ce qu'on
  glisse, et copier ou déplacer, s'écrit sous le pointeur. Les emplacements (volume,
  dossier personnel, racine) ne bougent pas.
- Boîte (retour du 20/10/2026) : titre sans verbe (« Dossier « Séance » vers… », « 3
  images vers… »), le choix Déplacer / Copier, et **Valider** : le verbe n'apparaît qu'une
  fois. Destinations récentes comparées en chemins (`~/Images/` complété par Tab et
  `~/Images` ne font plus deux lignes). **À faire** : proposer de **fusionner** quand la
  destination contient déjà un dossier du même nom (refusé pour l'instant).

## Corbeille et annulation (18/10/2026)

`delete` renvoie les fichiers mis à la corbeille (`Trashed` : emplacement d'origine, et
de quoi les retrouver), `untrash` les remet en place (voir `history.md`). Sur macOS, la
mise à la corbeille passe par `NSFileManager` (emplacement exact renvoyé) plutôt que par
le crate `trash`, qui ne le donne pas.

`delete_with_progress(&Group, corbeille, progression)` : la même, avec (faits, total)
après chaque fichier. L'interface la lance dans un fil à part, avec une barre dans la
boîte (24/10/2026, issue n° 43 : une centaine d'images à la corbeille figeait la
fenêtre). Sous Linux et Windows, les fichiers partent un par un, puis la corbeille est
lue une seule fois (indexée par emplacement d'origine) pour retrouver de quoi les en
ressortir.

## Dossiers (18/10/2026)

`create_dir(parent, nom)`, `rename_dir(dossier, nom)` (même parent ; la casse seule peut
changer), `dir_contents` (fichiers et sous-dossiers, pour la confirmation),
`delete_dir(dossier, corbeille)` (tout le contenu ; à la corbeille, de quoi l'en
ressortir). Noms vérifiés comme pour les images (`check_name`). Annulables (voir
`history.md`). Dans l'interface (`TriApp::after_folder_change`), le travail de fond suit :
dossier ouvert supprimé, il se ferme et ses calculs s'arrêtent ; sous-dossier supprimé avec
« Sous-dossiers » coché, ses images quittent la liste ; rendus darktable d'un dossier
supprimé, interrompus (25/10/2026).

