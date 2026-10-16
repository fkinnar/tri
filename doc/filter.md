# Module `tri::filter`

Les filtres décident quelles images afficher, et dans quel ordre ou regroupement.

## Pourquoi des groupes

Les images circulent sous forme de **`Groups`** (`Vec<Vec<MediaFile>>`), pas d'une liste
plate. Cela prépare la détection de séries ou de rafales (façon Capture One) sans changer
l'interface des filtres plus tard. Sans regroupement, tout tient dans un groupe unique.

Invariant : **un groupe n'est jamais vide**. Les filtres qui retirent des images suppriment
aussi les groupes qu'ils ont vidés.

## Le trait `Filter`

```rust
pub trait Filter {
    fn apply(&self, groups: Groups) -> Groups;
}
```

Un filtre prend des groupes et en renvoie d'autres. Il peut retirer des images (filtre par
type, et plus tard par étoiles ou couleur), les réordonner ou les regrouper (détection de
séries). Les filtres natifs et ceux des futurs plugins implémentent **ce même trait**, sans
traitement spécial pour les filtres intégrés.

Le trait est *dyn-compatible* : une suite de filtres se stocke en `Vec<Box<dyn Filter>>`.

`apply` prend les groupes **par valeur** : un filtre peut les transformer sans copier les
`MediaFile`.

## Fonctions utilitaires

- **`ungrouped(files)`** : met la sortie de `media::scan` dans un groupe unique (ou aucun
  groupe si la liste est vide, pour respecter l'invariant).
- **`apply_all(&filters, groups)`** : applique une suite de filtres dans l'ordre.
- **`retain(groups, keep)`** : garde dans chaque groupe les images qui vérifient `keep`, puis
  retire les groupes vides. C'est la brique commune des filtres qui ne font que trier sur un
  critère.

## Filtres natifs

### `KindFilter`

```rust
pub enum KindFilter { All, RawOnly, FlatOnly }   // défaut : All
```

Filtre par nature de fichier. Cas d'usage : une carte SD en RAW+JPEG doit pouvoir s'afficher
en entier, en RAW seuls, ou en JPEG seuls. `accepts(kind)` donne la décision pour une seule
image (utile côté interface, par exemple pour un compteur).

## Exemple

```rust
use tri::filter::{Filter, KindFilter, apply_all, ungrouped};
use tri::media::scan;

let groups = ungrouped(scan("/carte/DCIM", usize::MAX)?);
let raws = KindFilter::RawOnly.apply(groups);

// ou une suite de filtres :
let filters: Vec<Box<dyn Filter>> = vec![Box::new(KindFilter::FlatOnly)];
let shown = apply_all(&filters, ungrouped(scan("/carte/DCIM", usize::MAX)?));
```
