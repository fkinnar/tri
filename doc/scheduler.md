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
- **Événements** (callback appelé hors verrou) : `Ready`, `Failed`, `Progress`, tous avec
  la génération du dossier. L'interface ignore ceux d'une autre génération.
- **Annulation** : un nouveau dossier annule tout ; changer d'image annule un 1:1 en cours
  pour une autre image (effectif seulement entre deux étapes LibRaw, voir `raw.md`).
- **`Producer`** : trait du calcul ; `CachedPreviews { cache, sizes }` est la vraie
  implémentation, et un faux producteur permettra de tester l'ordonnancement sans LibRaw.

**État :** tests unitaires faits (priorités, routage lourd/léger, progression). Les tests
d'intégration du scheduler (avec faux producteur et sur vrai RAW) restent à écrire.
