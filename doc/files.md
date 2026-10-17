# Module `tri::files` — actions sur les fichiers

Renommer, supprimer (corbeille), afficher dans le gestionnaire de fichiers, ouvrir dans un
éditeur externe (premier lot demandé le 17/10/2026). Tout le travail est dans le moteur :
l'interface (`gui/src/files.rs`) ne fait que la saisie et la confirmation, et la ligne de
commande offre les mêmes actions.

## Principe : une image ne voyage jamais seule

- Ses **sidecars XMP** la suivent toujours : sans eux, ses notes seraient perdues, ou
  attribuées plus tard à une autre image du même nom.
- L'autre image de sa **paire RAW+JPEG** (même nom à l'extension près, sans tenir compte de
  la casse) la suit aussi, par défaut (`[files] pairs = true`), comme pour les notes : on
  rejette une photo, pas un fichier.
- Un sidecar **partagé** (`IMAGE.xmp`, convention Adobe, commun au RAF et au JPG) n'est
  touché que si toutes les images qui l'utilisent le sont. Sans les paires, renommer le RAF
  seul **copie** ce sidecar sous le nouveau nom (les deux images gardent leurs notes) ;
  supprimer le RAF seul le laisse en place.

## Préparer, puis agir

Chaque action se prépare d'abord, pour montrer avant d'agir la liste des fichiers touchés,
et pour que les erreurs prévisibles arrivent avant la première modification.

| Fonction | Rôle |
|---|---|
| `group(images, pairs) -> Group` | images (paires comprises), sidecars à elles seules (`sidecars`), sidecars partagés laissés (`shared`) |
| `plan_rename(image, new_stem, settings) -> Result<Rename>` | nouveaux chemins (`moves`) et copies de sidecars partagés (`copies`) ; refuse un nom vide, avec `/` ou `\`, `.` / `..` (`InvalidName`) ou déjà pris (`Exists`) ; changer seulement la casse est permis |
| `rename(&Rename)` | exécute ; jamais d'écrasement (revérifié juste avant chaque fichier) ; sur une erreur, ce qui a été renommé est remis en place et les copies effacées |
| `plan_delete(images, settings) -> Group` | fichiers à supprimer |
| `delete(&Group, trash)` | corbeille (crate [`trash`](https://crates.io/crates/trash)) ou suppression définitive |
| `reveal(path)` | gestionnaire de fichiers, fichier sélectionné |
| `editor_command(editor, files)` / `open_with(editor, files)` | éditeur externe (commande construite à part, pour être testée sans rien lancer) |

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

Exemples d'éditeur : `["darktable"]`, `["open", "-a", "darktable"]` (macOS),
`["gimp", "{files}"]`.

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
