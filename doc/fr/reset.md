# Module `tri::reset` : réinitialiser un dossier

Issue n° 411 (décidée avec l'utilisateur le 11/11/2026) : une boîte, une case par chose à
effacer, parmi quatre.

| Partie | Quoi | Annulable |
|---|---|---|
| Cache | Les entrées des images (`Cache::clear_folder`), gardées avec les photos (`.tri/cache`) ou dans le cache central : marche sur tout dossier | non (recalculé) |
| Données de tri | Les fichiers de données du dossier ([store.md](store.md) : filtres et tri, curseurs, couvertures, développements préférés, mémoire des notes) ; l'interrupteur « garder les données de tri ici » reste | non |
| Notes | Étoiles, couleurs, drapeaux **remis à zéro dans les sidecars, jamais supprimés** (idée de l'utilisateur) : seuls les champs qu'écrit `tri` changent, le reste (historique darktable, mots-clés de Lightroom…) est réécrit à l'identique, donc aussi sur des sidecars touchés par d'autres logiciels. Toutes les sources existantes de la photo (XMP, `.rrdata`, `.pp3`, `.arp`, et l'image elle-même quand elle reçoit les notes d'office) ; aucun sidecar n'est créé (`NoteBook::clear`). Une photo en conflit est sautée et signalée | oui |
| Versions | La liste `tri:Versions` vidée | oui |

Notes et versions se défont ensemble, en un pas (`history::Step::Reset`).

## API

- `Parts { cache, data, notes, versions }`, `Parts::parse("cache,data,ratings,versions")` ;
- `folders(dossier, profondeur)` : le dossier et ses sous-dossiers (sauf cachés) sur
  `profondeur` niveaux (1 : le dossier seul) ;
- `survey(cache, data, dossiers, convention) -> Survey` : ce qui serait effacé (place du
  cache, fichiers de données, photos notées, photos en conflit, versions), pour la boîte
  (en fond : les notes et versions de chaque image sont lues) ;
- `run(cache, data, dossiers, parts, convention) -> Outcome` : place libérée, fichiers de
  données, notes remises à zéro et versions retirées (`Outcome::step` donne le pas
  d'historique), images laissées et pourquoi. Au mieux : une image qu'on ne peut pas
  écrire est laissée, le reste continue.

## Interface

Clic droit sur un dossier, groupe Préférences : « Réinitialiser ce dossier… » (« ce
disque… »), action `core.folders.reset` (sans raccourci par défaut). « Vider le cache de
ce dossier… » (`core.folders.cache_clear`) ouvre la même boîte, le cache seul coché. Par
défaut : cache et données cochés, notes et versions non ; « Sous-dossiers compris »
décoché sur un dossier, coché sur un disque. Les chiffres sont mesurés en fond ; une
ligne à zéro est grisée, le bouton aussi tant qu'il ne reste rien à effacer. **Les notes
seulement si « Actions sur les fichiers » est coché** (remarque de l'utilisateur,
11/11/2026 : remettre à zéro les notes de tout un dossier est une action sur les
fichiers) ; sinon la case est grisée et sa bulle dit où l'autoriser. Le dossier ouvert,
s'il est dedans, est relu à la fin.

## Ligne de commande

`tri <dossier> --reset cache,data,notes,versions [--depth N] [--dry-run]` (`ratings`
accepté pour `notes`) : `--dry-run` compte seulement.
