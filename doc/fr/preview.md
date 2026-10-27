# Module `tri::preview`

Calcul synchrone des images mises en cache (voir `CLAUDE.md`, « Aperçus et cache »).

- `Sizes { thumbnail: 400, standard: 2560 }` : plus grand côté de chaque niveau réduit.
- `Settings` : tout ce qui règle le calcul, lu dans le fichier de configuration (voir
  [settings.md](settings.md)) : `sizes`, `render` (`RenderOptions` : balance des blancs,
  luminosité, luminosité automatique ; `half_size` y est ignoré, choisi par niveau),
  `half_size_margin` (1,0), `embedded_min_ratio` (0,5), `min_embedded_full` (1024 px).
- `compute(file, level, settings, cancel)` :
  - vignette RAW : aperçu embarqué décodé *sans* sa balise EXIF, réduit, puis tourné selon
    `RawFile::orientation()` ; à défaut d'aperçu, rendu `half_size` ;
  - aperçu standard RAW : `half_size` si `half_size_suffices(size, side, margin)` (moitié
    du côté long ≥ `half_size_margin` × côté demandé), sinon rendu complet, puis `fit` ;
  - aperçu boîtier RAW (`Embedded`) : JPEG embarqué décodé (libjpeg-turbo, réduit à l'IDCT
    sans passer sous la taille standard), réduit à la taille standard, redressé. Refusé
    (`PreviewError::NoEmbeddedPreview`) s'il fait moins de `embedded_min_ratio` (la moitié)
    de la taille demandée : certains boîtiers n'embarquent qu'une vignette de 160 px. Pour une image plate, ce
    niveau est identique à l'aperçu standard (le scheduler ne le demande pas) ;
  - 1:1 du JPEG boîtier (`EmbeddedFull`) : JPEG embarqué à sa taille d'origine, redressé ;
    refusé sous `min_embedded_full` (1024 px) ;
  - 1:1 RAW : rendu complet annulable ;
  - images plates : `flat::decode` puis `fit` (sauf pour le 1:1).
- `params(level, settings)` : chaîne stable incluse dans la clé de cache (`RULES_VERSION` et
  les seuls réglages qui influencent ce niveau : taille, rendu, seuils) ; changer un réglage
  recalcule les niveaux concernés, et eux seuls (testé par
  `params_follow_only_the_settings_that_matter`).
- `ensure(cache, …)` : ne calcule que si l'entrée manque ; renvoie `Cached` ou `Computed`.
  Pour `Level::Exposure`, calcule l'analyse (`compute_exposure` : données du capteur pour
  un RAW, image décodée pour une image plate) et l'écrit en binaire (`put_bytes`).
- `exposure(cache, file, settings)` : analyse lue dans le cache (`None` si absente ; une
  entrée illisible est supprimée pour être recalculée). Sa clé contient tous les réglages
  `[exposure]`.
- `camera_exposure(cache, file, settings)` (27/10/2026) : exposition du **JPEG boîtier**
  d'un RAW, d'après son aperçu boîtier en cache (`Level::Embedded`) ; `None` pour une
  image plate ou tant que l'aperçu manque. Sans cache propre (quelques dizaines de ms) :
  l'interface la garde en mémoire et la superpose à l'histogramme du capteur.
- `PreviewError` : `Raw`, `Flat`, `Io`, `Cancelled`, `NotAnImage` (`compute` sur
  `Level::Exposure`).
