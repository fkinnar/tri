# Module `tri::cache`

Cache persistant sur disque des images calculées : vignettes, aperçus standard, rendus 1:1.
Le principe et les choix (trois niveaux, half_size, emplacement central) sont décrits dans
`CLAUDE.md`, section « Aperçus et cache ». Ce module ne fait que **stocker et retrouver** ; il
ne calcule rien. La file de travail qui le remplit en tâche de fond viendra par-dessus.

## Organisation sur disque

```
<racine>/
└── v3/                      version du format (FORMAT_VERSION)
    ├── thumbnail/
    │   ├── 8c41…07/         un dossier de photos (début du hash de son chemin)
    │   │   ├── .source      ce dossier de photos, en clair
    │   │   └── 3f9a…e1.jpg  une entrée = un JPEG qualité 90
    │   └── …
    ├── embedded/            aperçus tirés du JPEG du boîtier (RAW seulement)
    ├── standard/
    ├── embedded-full/       JPEG embarqué à sa taille d'origine (1:1 du JPEG boîtier)
    ├── full/
    └── exposure/            analyses d'exposition (.bin, ~14 Ko : histogrammes, masque)
```

- **Un fichier par entrée**, sans base de données ni index : l'état du cache *est* le contenu
  du dossier. Rien à reconstruire, rien à corrompre au-delà d'un fichier.
- **Un sous-dossier par dossier de photos** (23/10/2026, format `v2`) : son nom est le
  début (16 caractères) du hash du chemin du dossier des photos. Vider le cache d'un
  dossier (`clear_folder`, `tri <dossier> --clear-cache`) supprime ce sous-dossier dans
  chaque niveau, sans devoir recalculer les clés : elles dépendent des réglages, et la
  taille de l'aperçu standard de l'écran, que la ligne de commande ne connaît pas.
  Avant, les sous-dossiers étaient les 2 premiers caractères du hash de l'entrée.
- **`v3/`** : changer le format (ou l'algorithme de clé) se fait en incrémentant
  `FORMAT_VERSION`. Les anciennes entrées deviennent invisibles ; les formats précédents
  listés dans `OLD_FORMATS` (`v1/`, `v2/`) sont supprimés en fond à l'ouverture du cache.
  `v3` (24/10/2026) : clé sans le dossier, fichier `.source` (voir plus bas).

## Cache sur le disque des photos (24/10/2026, issue n° 55)

Trois places possibles pour le cache d'une photo :

- **chez l'utilisateur** (ci-dessus) : photos qui ne bougent pas ;
- **à la racine d'un disque amovible** : un disque de transfert, branché sur plusieurs
  ordinateurs ;
- **dans un dossier** : un dossier qu'on déplace.

Les deux derniers sont un même mécanisme : un dossier caché `.tri-cache` (`DISK_DIR`)
garde le cache de toutes les photos en dessous, sous-dossiers compris, avec la même
organisation (`.tri-cache/v3/<niveau>/<dossier>/`), mais le dossier est repéré par son
chemin **relatif** à la racine (écrit avec des `/` sur tous les systèmes). Monté ailleurs
(`/Volumes/T7` sur un Mac, `/media/u/T7` sous Linux), le disque retrouve son cache.
`Cache::disk_of(dossier)` cherche le `.tri-cache` le plus proche en remontant (gardé en
mémoire par dossier, oublié à chaque changement).

- `use_disk(racine)` : crée `.tri-cache` et y **déplace** les entrées déjà calculées
  (cache central, ou `.tri-cache` plus haut) : rien n'est recalculé ;
- `release_disk(racine)` : l'inverse, les entrées vont où leurs photos les attendent
  désormais (`.tri-cache` plus haut, sinon chez l'utilisateur) ;
- `relocate(de, vers)` : après le déplacement ou le renommage d'un dossier, ses entrées
  le suivent (un `.tri-cache` dans le dossier même est parti avec lui) ;
- `apply(location, dossier, amovibles)` : le réglage `[cache] location` (`user`, `disk`,
  `folder`) appliqué à un dossier ouvert dont le cache est encore chez l'utilisateur ;
  un choix déjà fait est respecté (`wanted_root`, `places::removable_roots`).

Le fichier `.source` de chaque dossier d'entrées (chemin absolu au centre, relatif sur un
disque) permet ces déplacements sans connaître les réglages. Vers un autre disque, copie
puis suppression ; une entrée déjà présente à l'arrivée est gardée. Les niveaux 1:1 d'un
`.tri-cache` ne sont pas limités par `zoom_max_mb`, et `--cache-usage` ne compte que le
cache central. Disques FAT32 : la date de modification y est en heure locale, un
ordinateur réglé sur un autre fuseau recalcule tout (exFAT, APFS, ext4 : non).

### Emplacement par défaut : `default_dir()`

| Système | Dossier |
|---|---|
| Linux / Unix | `$XDG_CACHE_HOME/tri` si la variable est un chemin absolu, sinon `~/.cache/tri` |
| macOS | `~/Library/Caches/tri` |
| Windows | `%LOCALAPPDATA%\tri` |

Le cache n'est **jamais écrit à côté des photos** : cartes SD, disques en lecture seule, NAS.
Ces dossiers sont ceux que les systèmes prévoient pour des données recalculables : exclus des
sauvegardes Time Machine sous macOS, nettoyables par l'utilisateur.

## Les clés : `Key::new(source, level, params)`

Une clé est un hash **BLAKE3** (256 bits, en hexadécimal) de :

1. la version du format ;
2. le **nom** du fichier source (depuis `v3` ; le dossier, lui, choisit le rangement : le
   cache peut ainsi changer de place sans rien recalculer) ;
3. sa **taille** et sa **date de modification** (nanosecondes) ;
4. le **niveau** (`Thumbnail`, `Embedded`, `Standard`, `EmbeddedFull`, `Full`) ;
5. les **paramètres** de calcul, une chaîne libre fournie par l'appelant (ex.
   `"half=1;side=2560"`).

Conséquences :

- **fichier modifié** (retouché, réécrit) ⇒ nouvelle clé ⇒ recalcul automatique ;
- **réglages changés** (taille d'aperçu, balance des blancs…) ⇒ idem, à condition que
  l'appelant les inclue dans `params` ;
- **dossier déplacé ou renommé** ⇒ le cache n'est plus retrouvé, sauf s'il le suit
  (`relocate` : renommage et déplacement depuis `tri`, case « Emporter le cache ») ou
  s'il est dans le dossier (`.tri-cache`) ; vérifier le contenu imposerait de relire
  chaque RAW de ~30 Mo ;
- l'ancienne entrée n'est **pas supprimée** : elle reste jusqu'au vidage ou à l'éviction.

Chaque champ est préfixé par sa longueur avant d'être hashé : (`ab`, `c`) et (`a`, `bc`) ne
peuvent pas produire la même clé.

Pourquoi BLAKE3 plutôt que le `DefaultHasher` de la bibliothèque standard : ce dernier ne
garantit pas un résultat identique d'une version de Rust à l'autre, ce qui invaliderait le
cache à chaque mise à jour du compilateur. Un test unitaire (`key_hash_is_stable`) fige une
valeur de référence pour détecter tout changement accidentel.

## API de `Cache`

| Méthode | Rôle |
|---|---|
| `Cache::open(root)` | Ouvre un cache dans `root`, en créant les dossiers si besoin |
| `Cache::open_default()` | Idem à `default_dir()` |
| `with_quality(q)` | Même cache, qualité JPEG `q` (1 à 100, défaut `DEFAULT_JPEG_QUALITY` = 90) pour les entrées écrites désormais ; hors de la clé, voir [settings.md](settings.md#réglages-et-cache) |
| `contains(&key)` | L'entrée existe-t-elle ? (sans la décoder) |
| `get(&key) -> Option<Bitmap>` | Lit et décode l'entrée |
| `get_bytes(&key)` / `put_bytes(&key, &bytes)` | Entrée binaire d'une analyse (`Level::Exposure`, `Level::Sharpness`, fichier `.bin`), même écriture atomique ; `Level::is_image` distingue les deux sortes |
| `put(&key, &bitmap)` | Écrit l'entrée (JPEG à la qualité du cache, via [`jpeg`](jpeg.md) : 4:4:4 pour le 1:1, 4:2:0 ailleurs), en écrasant l'ancienne |
| `remove(&key)` | Supprime une entrée (sans erreur si absente) |
| `clear(Some(level))` / `clear(None)` | Vide un niveau / tout le cache |
| `clear_folder(dossier, niveau)` | Vide les entrées des images de `dossier` (pas de ses sous-dossiers), un niveau ou tous, quels que soient les réglages avec lesquels elles ont été calculées ; renvoie ce qui a été libéré |
| `usage(level) -> Usage` | Octets et nombre d'entrées d'un niveau |
| `evict(level, max_bytes) -> Usage` (branché le 17/10/2026 : `EngineSettings::trim_cache` limite chaque niveau 1:1 à `[cache] zoom_max_mb`, au démarrage de l'interface dans un fil à part et à la fin de `tri --previews` ; jusque-là, les 1:1 s'accumulaient sans limite) | Supprime les entrées les moins récemment utilisées jusqu'à `max_bytes`, renvoie ce qui a été libéré |

### Robustesse

- **Écriture atomique** : `put` écrit dans un fichier temporaire du même dossier
  (`<hash>.jpg.<pid>-<n>.tmp`), le synchronise sur disque, puis le renomme. Un lecteur voit
  l'ancienne entrée ou la nouvelle, jamais une moitié, même si le programme est tué en
  pleine écriture. Les temporaires orphelins sont ignorés par `usage` et `evict`.
- **Auto-réparation** : une entrée illisible (JPEG tronqué, disque abîmé) est supprimée par
  `get`, qui renvoie `None` : l'appelant la recalcule comme une entrée absente.
- **Partage** : `Cache` ne garde aucun état en mémoire (juste le chemin). Il est `Clone`,
  utilisable depuis plusieurs threads, et même plusieurs processus, sur le même dossier.
- **Dernier usage** : `get` remet à jour la date de modification du fichier. `evict` s'en sert
  comme date de dernier usage, ce qui évite d'avoir à tenir un index.

## Tailles et performances

JPEG qualité 90 : quelques dizaines de Ko pour une vignette de 400 px, de l'ordre de 0,5 à
1 Mo pour un aperçu de 2560 px, 10 à 15 Mo pour un 1:1 de 26 Mpx. Ces chiffres sont
indicatifs et seront mesurés quand la file de travail remplira le cache.
