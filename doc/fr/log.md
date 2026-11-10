# Journal de l'application (`tri::log`)

Issue n° 398 (10/11/2026). Un rendu darktable échouait dans trois dossiers avec pour seul
message « 1 échec (Darktable : darktable-cli) » : ni l'image, ni la raison (darktable-cli
n'avait rien écrit sur sa sortie d'erreur). La cause, un `.xmp` abîmé, n'a été trouvée qu'en
effaçant tous les sidecars. Chez un utilisateur, un tel signalement ne se diagnostique pas.

D'où le principe : **le message de la barre d'état reste court, le détail complet va au
journal**.

## Le fichier

`tri.log`, dans le dossier de données (`[data] dir` ; sinon `~/.local/share/tri` sous
Linux, `~/Library/Application Support/tri` sous macOS, `%APPDATA%\tri` sous Windows).
L'interface et la ligne de commande y écrivent toutes les deux.

Une entrée = une ligne d'en-tête, puis son détail en retrait de deux espaces :

```
2026-11-10 15:03:12.345 ERROR [develop] #4711-17 Darktable failed: can't open xmp
  image: /Volumes/SD/DCIM/DSCF0001.RAF
  command: darktable-cli DSCF0001.RAF DSCF0001.RAF.xmp … --library :memory:
  exit: exit status: 1
  output:
  [xmp] can't open xmp file …
```

- **date et heure locales**, à la milliseconde ;
- **gravité** : `TRACE`, `INFO`, `WARN`, `ERROR`, `CRASH` (en anglais et fixes : le
  journal se joint aux signalements) ;
- **source** : la partie de `tri` (`develop`, `export`, `preview`, `settings`, `gui`,
  `panic`, `gpu`…) ;
- **identifiant** `<processus>-<numéro>` : la barre d'état s'en sert pour ouvrir le
  journal sur une erreur ;
- **message** sur une ligne ; les messages en anglais pour la même raison, sauf ceux de
  l'interface, écrits tels qu'ils ont été montrés.

Le fichier se lit tel quel ; `tri::log::parse` le relit (une ligne qui n'est pas un
en-tête complète le détail de l'entrée d'avant).

## Ce qui y va

| Source | Gravité | Détail |
|---|---|---|
| Logiciel de développement en échec (rendu ou export : darktable, RapidRAW, RawTherapee, ART) | `ERROR` | image, ligne de commande complète, code de sortie, **tous** ses messages (`develop::tool_failed`) |
| Export d'une image en échec | `ERROR` | image, raison |
| Vignette, aperçu ou rendu raté (scheduler) | `WARN` | niveau, image, raison |
| Configuration invalide | `WARN` | le message de lecture (extrait du fichier) |
| Toute erreur montrée dans la barre d'état | `ERROR` | le message montré (source `gui`) |
| Plantage (`panic`) | `CRASH` | version, système, fil, endroit, pile d'appels |
| Erreur de la carte graphique | `CRASH` | message de wgpu ; une rafale (5 s) n'en laisse qu'une |
| Trace détaillée (`[log] trace`) | `TRACE` | étapes du chargement des images, images lentes |

## Taille

Au-delà de `[log] max_kb` (1 024 Ko), `tri.log` devient `tri.1.log`, l'ancien
`tri.1.log` devient `tri.2.log`…, et l'on garde `[log] keep` anciens fichiers (3). Le
journal ne fait jamais échouer une action : une écriture impossible est ignorée.

Sans dossier de données, ou sans `tri::log::init` (tests, outils), rien n'est écrit.

## Plantages

Jusqu'au 10/11/2026, les plantages allaient dans un `crash.log` à part. Au premier
lancement après, son contenu est repris dans `tri.log` (une entrée `CRASH` « Earlier
crashes ») et le fichier est supprimé.

## Dans l'interface

- **Clic sur une erreur** de la barre d'état : la fenêtre du journal s'ouvre sur cette
  erreur et sur les entrées que le moteur a écrites juste avant (son détail), mises en
  avant et dépliées ; le message est effacé (comme le clic le faisait déjà).
- **Fenêtre du journal** (palette : « Journal de l'application », `core.app.log`, sans
  raccourci par défaut ; bouton « Journal » de l'aide) : les entrées les plus récentes
  d'abord (500 au plus, la recherche remonte plus loin), gravités montrées (erreurs ;
  avertissements et erreurs, par défaut ; tout), recherche floue (message, source,
  détail, date), détail déplié au clic, « Copier » une entrée, « Tout copier » (les
  entrées montrées), « Afficher dans le gestionnaire de fichiers », « Actualiser ».
- **Signaler un problème** : les dernières entrées (`[gui] report_log_entries`, 20) sont
  proposées dans une boîte, **modifiables** : les chemins contiennent des noms de dossiers
  personnels. « Ouvrir le formulaire avec ces lignes » les joint au formulaire de GitHub
  (champ « Journal », paramètre `logs`, ~6 000 caractères au plus une fois encodés : les
  plus anciennes lignes tombent d'abord) ; « Sans le journal » ouvre le formulaire seul.
  Avec `report_log_entries = 0`, le formulaire s'ouvre directement.

## Trace

`[log] trace = true` écrit au journal ce que `TRI_GUI_TRACE=1` écrit sur la sortie
d'erreur (étapes du chargement des images, images de l'interface plus lentes que
`[gui] trace_slow_ms`). Pour un utilisateur à qui l'on demande de reproduire un problème
sans terminal ; volumineux, à couper ensuite. Se règle à chaud dans la fenêtre de réglages.

## API

- `init(dir, settings)` : active le journal (appelé par `tri` et `tri-gui` au démarrage) ;
  `set_settings` : nouveaux réglages ;
- `error`, `warning`, `info`, `trace`, `write` : une entrée, renvoie son identifiant ;
- `read(dir)` : toutes les entrées, les plus récentes d'abord ; `tail(dir, n)` : les `n`
  dernières en texte ;
- `last_number`, `id_of` : rattacher une erreur montrée aux entrées écrites avant elle ;
- `set_thread_dir` : un journal propre au fil courant, pour les tests de l'interface
  (parallèles, chacun dans son dossier temporaire).
