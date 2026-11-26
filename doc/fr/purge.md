# Ménage — `tri::purge`

Issue #604 (26/11/2026, idée de l'utilisateur : des photographes ont vraiment beaucoup
d'images). Trois choses grandissent sans limite ; le ménage les tient à une taille
raisonnable, sans jamais rien perdre qui puisse encore servir.

| Quoi | Réglage `[purge]` | Défaut | Ce qui part |
|---|---|---|---|
| Historique des déchargements (`offload-history.tsv`) | `offload_history` | `"1y"` | les entrées des photos plus anciennes (date de prise de vue) |
| Cache | `cache_unused` | `"never"` | les aperçus des dossiers pas ouverts depuis ce temps |
| Cache | `cache_gone` | `true` | les aperçus des dossiers qui n'existent plus |
| Données de tri (`folders/<empreinte>/`) | `folder_data_gone` | `true` | celles des dossiers qui n'existent plus |

Durées au choix : `"1w"`, `"1m"`, `"3m"`, `"1y"`, `"3y"`, `"never"` (liste déroulante
dans la fenêtre de réglages, section « Ménage »).

## Historique des déchargements

Une carte qui revient avec des photos de plus de quelques semaines est très peu probable.
`History::purge(avant, à_l_essai)` retire les entrées des photos prises avant la date
limite et les ajoute au bout de l'**archive** voisine, `offload-history-archive.tsv`
(même format, jamais relue : la trace de l'endroit où chaque fichier a été copié). Les
lignes illisibles restent. « Oublier cette carte » (#602), lui, n'archive rien : le but
est d'oublier vraiment une carte de test.

## Cache

`Cache::purge(pas_ouvert_depuis, disparu, à_l_essai)` parcourt le cache central, dossier
d'entrées par dossier d'entrées : chacun a son repère `.source` (le dossier des photos).

- **Dossier disparu** : voir [`gone`](#un-dossier-introuvable-na-pas-forcément-disparu).
- **Pas ouvert depuis** : la date du repère, mise à jour à chaque ouverture du dossier
  (`Cache::note_use`, en fond : une petite écriture par niveau, pas par image lue ; lire
  une entrée n'écrit rien, issue n° 74). Un dossier jamais rouvert depuis cette version
  compte depuis la création de son cache.
- Les caches gardés **sur un disque** (`.tri/cache`) ne sont jamais touchés : ils
  voyagent avec lui.

Désactivé par défaut pour l'âge : recalculer prend du temps, à choisir selon la place.

## Données de tri

`store::purge_gone(données, disparu, à_l_essai)` supprime les replis
(`folders/<empreinte>/`, avec leur fichier `folder`) des dossiers disparus. Un `.tri/`
dans un dossier ou sur un disque n'est jamais touché.

## Un dossier introuvable n'a pas forcément disparu

Disque débranché, carte retirée, NAS hors ligne, partage pas monté : `gone(dossier)` ne
le dit disparu que s'il n'existe plus **et** que son plus proche parent existant n'est
ni :

- un dossier de montage (`/Volumes`, `/media`, `/media/<utilisateur>`,
  `/run/media/<utilisateur>`, `/mnt`, `/net`, GVFS, `Library/CloudStorage`) ;
- un dossier **vide** (point de montage d'un NAS en NFS, volume absent) ;
- la racine d'un disque (trop incertain) ;
- ou rien (lecteur Windows absent).

Un dossier relatif n'est jamais dit disparu.

## Quand

- **Automatique** : en fond, 20 s après le lancement de l'interface (l'ouverture du
  dossier d'abord), au plus une fois par `interval_hours` (24 ; 0 : jamais), retenu par
  le fichier `last-purge` du dossier de données. Le bilan va dans le journal.
- **À la main** : « Nettoyer… » dans la section Cache de la fenêtre de réglages : ce qui
  partirait d'abord (essai), puis « Nettoyer » pour de bon.
- **Ligne de commande** : `tri --purge [--dry-run]`.

## API

- `PurgeSettings`, `Keep` (`duration`, `cutoff`) ;
- `gone(dossier)`, `due(données, réglages, maintenant)`, `mark_done(données)` ;
- `run(réglages, cache, données, historique, maintenant, à_l_essai) -> Report` (entrées
  archivées, cache libéré, dossiers, erreurs) ;
- `offload::History::purge`, `offload::archive_path`, `offload::history_path` ;
  `Cache::purge`, `Cache::note_use` ; `store::purge_gone`.
