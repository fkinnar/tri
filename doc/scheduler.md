# Module `tri::scheduler`

File de travail en tâche de fond qui remplit le cache pour un dossier.

- **Workers** : un seul worker *lourd* pour les rendus LibRaw (aperçus standard et 1:1 des
  RAW), plus `Config::light_workers` workers *légers* (par défaut la moitié des cœurs) pour
  les vignettes et les images plates.
- **Priorités**, recalculées à chaque prise de tâche (`priority`) : 1:1 demandé > aperçu de
  l'image affichée > n+1 > n-1 > vignettes dans l'ordre du dossier > autres aperçus par
  distance (en avant d'abord).
- **API** : `set_folder(files) -> génération`, `set_focus(index)`, `request_full()`,
  `set_paused(bool)`, `progress(level)`, `wait_idle(timeout)`. `Drop` annule et arrête
  les threads.
- **Événements** : `Ready`, `Failed`, `Progress`, tous avec la génération du dossier.
  Garanties du callback :
  - appelé **hors du verrou** d'état : il peut rappeler le scheduler ;
  - **jamais deux fois en même temps**, et **dans l'ordre** où les événements se sont
    produits. Les workers déposent leurs événements dans une file commune (`outbox`), sous
    le verrou ; un seul thread à la fois la vide (`deliver`). Sans cela, deux workers
    livrant en parallèle pouvaient faire arriver « 6/6 » avant « 5/6 » : la progression
    reculait (bug trouvé par les tests, voir plus bas) ;
  - `set_folder` purge les événements non livrés de l'ancien dossier. Seul un événement en
    cours de livraison peut encore arriver, et l'interface ignore ceux d'une autre
    génération.
- **`wait_idle`** ne rend la main qu'une fois **tous les événements livrés** : rien en
  attente, rien en cours, file d'événements vide, aucun callback en cours. Un callback qui
  panique ne bloque pas `wait_idle` (garde `Drop`).
- **Annulation** : un nouveau dossier annule tout ; changer d'image annule un 1:1 en cours
  pour une autre image (effectif seulement entre deux étapes LibRaw, voir `raw.md`).
- **`Producer`** : trait du calcul ; `CachedPreviews { cache, sizes }` est la vraie
  implémentation, et un faux producteur permettra de tester l'ordonnancement sans LibRaw.

## Tests

Tests unitaires : priorités, routage lourd/léger, fraction de progression.

Tests d'intégration (`tests/scheduler.rs`), avec un **faux producteur** qui enregistre les
tâches, simule durées, échecs, entrées déjà en cache, et mesure la concurrence par type de
worker :

- chaque fichier traité une fois par niveau, progression complète, pas de 1:1 sans demande ;
- événements dans l'ordre, progression strictement croissante (35 fichiers, 4 workers) ;
- entrées en cache signalées sans recalcul ; échecs signalés et comptés ;
- ordre exact du worker lourd autour de l'image affichée (5, 6, 4, 7, 3, 8, 2, 9, 1, 0) ;
- rendus RAW jamais simultanés, tâches légères en parallèle ;
- 1:1 à la demande, annulé quand l'image affichée change (< 2 s), puis abouti pour la
  nouvelle ;
- nouveau dossier : l'ancien est abandonné, aucun de ses résultats sous la nouvelle
  génération ;
- pause / reprise ; image affichée hors limites ignorée ; arrêt rapide à la destruction ;
- avec `TRI_TEST_RAW` : vrai producteur sur un RAW et un JPEG, tailles et orientation des
  entrées, second passage servi par le cache (0,66 s puis ~40 µs).

Ces tests ont révélé deux courses, corrigées : `wait_idle` pouvait rendre la main avant la
livraison des derniers événements, et des événements de progression pouvaient arriver dans
le désordre. Pour s'assurer qu'aucune ne revient, le fichier de tests a été relancé
300 à 500 fois de suite, sans échec.
