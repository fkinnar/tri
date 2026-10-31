# Module `tri::history` : annuler, rétablir

Fait le 18/10/2026, d'après les décisions prises avec l'utilisateur le même jour.

## Principes

- **Historique de session** : en mémoire, rien n'est gardé d'un lancement à l'autre
  (on n'annule pas, des jours après, une note que darktable a reprise entre-temps).
- **Une action = un pas**, même sur 30 images sélectionnées ; la paire RAW+JPEG et les
  sidecars suivent, comme à l'aller.
- **Jamais d'écrasement d'un travail fait ailleurs** : avant de défaire (ou de refaire)
  un pas, on vérifie que les fichiers sont dans l'état que ce pas a laissé. Sinon le pas
  est **refusé avec la raison** (« DSCF0305 : notes modifiées ailleurs depuis »), et
  reste où il est.
- **Hors historique** : navigation, filtres, sélection (rien n'est modifié), suppression
  définitive (déjà confirmée avec avertissement), déchargement d'une carte (trop gros, et
  il peut supprimer de la carte).

## Les pas (`Step`)

| Pas | Enregistré | Défaire | Vérifié avant |
|---|---|---|---|
| `Notes` | notes avant / après de chaque image (`snapshot`, puis `notes_step`) | notes d'avant, image par image (`NoteBook::write_only` : chaque image d'une paire retrouve **ses** notes) | notes du disque = notes d'après |
| `Rename` | le plan exécuté | renommage inverse ; copies d'un sidecar partagé supprimées | nouveaux noms présents, anciens libres |
| `Transfer` | le plan, et l'empreinte (taille, date) des fichiers écrits (`transfer_step`) | copie : copies supprimées ; déplacement : retour (même mécanique, copies vérifiées entre volumes) | fichiers écrits présents et **inchangés** (une copie retouchée depuis n'est pas supprimée) |
| `CreateDir` | le dossier créé | supprimé, **seulement s'il est encore vide** | dossier présent et vide |
| `RenameDir` | ancien et nouveau nom | renommage inverse | nouveau nom présent, ancien libre |
| `Trash` | où chaque fichier est parti (`files::delete` → `Trashed`) | `files::untrash` | emplacement d'origine libre, fichier encore dans la corbeille |
| `Developments` | développements supprimés (`files::delete_plan` → `Deleted`) : sidecars à la corbeille et XMP darktable réécrits sans historique (`Rewrite`, contenu avant / après) | XMP remis comme avant, puis `files::untrash` | XMP encore tel qu'écrit (noté dans darktable depuis : refusé), fichiers encore dans la corbeille |

Corbeille : le crate `trash` ne sait restaurer que sous Linux et Windows
(`os_limited::restore_all`, l'élément retrouvé par son emplacement d'origine). Sur macOS,
`tri` met lui-même à la corbeille par `NSFileManager.trashItemAtURL` (`objc2-foundation`,
déjà présent par `trash`), qui renvoie l'emplacement exact dans la corbeille ; annuler l'y
reprend (renommage, ou copie vérifiée si la corbeille est sur un autre volume).

## `History`

`push` (un nouveau pas efface les pas défaits), `undo`, `redo`, `go_to(position)` (liste
de l'historique : défait ou refait un à un, s'arrête au premier refus), `entries`,
`position`, plafond de pas (`[gui] history`, 200). `Applied` dit ce qui a été fait :
libellé, images à montrer, fichiers bougés (dossier à relire) ou seulement des notes.

## Interface

`u` / `Ctrl+Z` (et `Cmd+Z` sur macOS, seule exception à « Cmd réservé au système ») :
annuler ; `Ctrl+R` / `Ctrl+Maj+Z` (`Cmd+Maj+Z`) : rétablir ; `Ctrl+H` : la liste (les plus
récents en haut, pas défaits barrés, cliquer un pas y revient, « État au lancement »
défait tout). Après chaque pas : l'image concernée est sélectionnée, le dossier relu si
des fichiers ont bougé, et « Annulé : 3 étoiles : DSCF0305.RAF » dans la barre d'état ;
un refus y donne sa raison.

Non testé automatiquement : la corbeille réelle (les tests n'y touchent jamais, voir
`tests.md`) ; à vérifier à la main.
