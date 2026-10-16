# Tests

## Organisation

- **Tests unitaires** : dans chaque module, sous `#[cfg(test)] mod tests`. Ils testent la
  logique interne, y compris les fonctions privées (ex. `bitmap_from_processed`), sans
  dépendre de fichiers externes.
- **Tests d'intégration** : dans `tests/`, un fichier par module (`tests/raw.rs`…). Ils
  utilisent uniquement l'API publique, comme le ferait l'interface. Les fichiers temporaires
  passent par le crate `tempfile` (supprimés automatiquement).

## Lancer les tests

```sh
cargo test                       # tout ce qui ne nécessite pas de fichier externe
TRI_TEST_RAW=/chemin/photo.RAF cargo test --release -- --include-ignored   # tout
```

Les tests qui ont besoin d'un vrai RAW sont marqués `#[ignore = "nécessite TRI_TEST_RAW"]` :
aucun fichier RAW n'est versionné (trop lourd, et propre à chaque boîtier). `cargo test` les
signale comme ignorés au lieu de les faire passer en silence. Avec `--include-ignored`, ils
échouent clairement si `TRI_TEST_RAW` n'est pas définie. `--release` est conseillé : le
développement RAW est beaucoup plus lent en debug.

## Ce qui est couvert

### `media` (unitaires, `src/media.rs`)

- extensions RAW et plates reconnues quelle que soit la casse ; autres fichiers ignorés ;
- fichiers cachés et AppleDouble (`._x.RAF`) ignorés ;
- listes d'extensions en minuscules et disjointes ;
- tri : le RAW précède son jumeau JPEG.

### `filter` (unitaires, `src/filter.rs`)

- `ungrouped`, `KindFilter` (tout / RAW / plates), suppression des groupes vidés, valeur par
  défaut, chaînage `apply_all`.

### `media` + `filter` (intégration, `tests/media.rs`)

Sur une fausse carte SD temporaire (paires RAW+JPEG, casse mixte, AppleDouble, `.xmp`,
sous-dossiers, dossier nommé `x.RAF`) : contenu et ordre exacts de `scan`, effet de `depth`
(0, 1, 2, 3, illimité), dossier vide, dossier absent, fichier passé comme racine, filtre par
type sur la carte.

### `raw` (unitaires, `src/raw/mod.rs`)

- conversion RGB8, niveaux de gris → RGB, octets superflus ignorés ;
- rejet des données tronquées, du JPEG, du 16 bits, d'un nombre de canaux inattendu ;
- traduction des codes d'erreur LibRaw ;
- valeurs par défaut de `RenderOptions`, version de LibRaw.

### `raw` (intégration, `tests/raw.rs`)

Sans fichier RAW : fichier absent, fichier non RAW, chemin non ASCII transmis intact.

Avec `TRI_TEST_RAW` : aperçu JPEG valide (marqueur `FF D8`), rendu RGB8 cohérent et non noir,
aperçu puis rendu sur le même handle, effet réel de l'option balance des blancs (vérifie le
shim C), `half_size` divise les dimensions par deux, rendu dans un autre thread.
