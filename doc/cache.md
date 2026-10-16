# Module `tri::cache`

Cache persistant sur disque des images calculées : vignettes, aperçus standard, rendus 1:1.
Le principe et les choix (trois niveaux, half_size, emplacement central) sont décrits dans
`CLAUDE.md`, section « Aperçus et cache ». Ce module ne fait que **stocker et retrouver** ; il
ne calcule rien. La file de travail qui le remplit en tâche de fond viendra par-dessus.

## Organisation sur disque

```
<racine>/
└── v1/                      version du format (FORMAT_VERSION)
    ├── thumbnail/
    │   ├── 3f/
    │   │   └── 3f9a…e1.jpg  une entrée = un JPEG qualité 90
    │   └── …
    ├── standard/
    └── full/
```

- **Un fichier par entrée**, sans base de données ni index : l'état du cache *est* le contenu
  du dossier. Rien à reconstruire, rien à corrompre au-delà d'un fichier.
- **Sous-dossiers sur 2 caractères** du hash : au plus quelques centaines de fichiers par
  dossier, même avec des dizaines de milliers d'entrées.
- **`v1/`** : changer le format (ou l'algorithme de clé) se fait en incrémentant
  `FORMAT_VERSION`. Les anciennes entrées deviennent invisibles et peuvent être supprimées en
  vidant le cache.

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
2. le **chemin absolu** du fichier source (`std::path::absolute`) : `photo.RAF` et
   `/dossier/photo.RAF` donnent la même clé ;
3. sa **taille** et sa **date de modification** (nanosecondes) ;
4. le **niveau** (`Thumbnail`, `Standard`, `Full`) ;
5. les **paramètres** de calcul, une chaîne libre fournie par l'appelant (ex.
   `"half=1;side=2560"`).

Conséquences :

- **fichier modifié** (retouché, réécrit) ⇒ nouvelle clé ⇒ recalcul automatique ;
- **réglages changés** (taille d'aperçu, balance des blancs…) ⇒ idem, à condition que
  l'appelant les inclue dans `params` ;
- **dossier déplacé ou renommé** ⇒ le cache n'est plus retrouvé (choix assumé : vérifier le
  contenu imposerait de relire chaque RAW de ~30 Mo) ;
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
| `contains(&key)` | L'entrée existe-t-elle ? (sans la décoder) |
| `get(&key) -> Option<Bitmap>` | Lit et décode l'entrée |
| `put(&key, &bitmap)` | Écrit l'entrée (JPEG qualité 90), en écrasant l'ancienne |
| `remove(&key)` | Supprime une entrée (sans erreur si absente) |
| `clear(Some(level))` / `clear(None)` | Vide un niveau / tout le cache |
| `usage(level) -> Usage` | Octets et nombre d'entrées d'un niveau |
| `evict(level, max_bytes) -> Usage` | Supprime les entrées les moins récemment utilisées jusqu'à `max_bytes`, renvoie ce qui a été libéré |

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
