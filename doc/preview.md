# Module `tri::preview`

Calcul synchrone des images mises en cache (voir `CLAUDE.md`, « Aperçus et cache »).

- `Sizes { thumbnail: 400, standard: 2560 }` : plus grand côté de chaque niveau réduit.
- `compute(file, level, sizes, cancel)` :
  - vignette RAW : aperçu embarqué décodé *sans* sa balise EXIF, réduit, puis tourné selon
    `RawFile::orientation()` ; à défaut d'aperçu, rendu `half_size` ;
  - aperçu standard RAW : `half_size` si `half_size_suffices(size, side)`, sinon rendu
    complet, puis `fit` ;
  - aperçu boîtier RAW (`Embedded`) : JPEG embarqué décodé (libjpeg-turbo, réduit à l'IDCT
    sans passer sous la taille standard), réduit à la taille standard, redressé. Refusé
    (`PreviewError::NoEmbeddedPreview`) s'il fait moins de la moitié de la taille demandée :
    certains boîtiers n'embarquent qu'une vignette de 160 px. Pour une image plate, ce
    niveau est identique à l'aperçu standard (le scheduler ne le demande pas) ;
  - 1:1 RAW : rendu complet annulable ;
  - images plates : `flat::decode` puis `fit` (sauf pour le 1:1).
- `params(level, sizes)` : chaîne stable incluse dans la clé de cache (taille, balance des
  blancs, `RULES_VERSION`) ; changer un réglage entraîne un recalcul.
- `ensure(cache, …)` : ne calcule que si l'entrée manque ; renvoie `Cached` ou `Computed`.
- `PreviewError` : `Raw`, `Flat`, `Io`, `Cancelled`.
