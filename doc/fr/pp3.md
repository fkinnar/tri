# Module `tri::pp3` : notes dans les profils de RawTherapee et d'ART

Ajouté le 31/10/2026, sur le modèle de [rrdata.md](rrdata.md) : RawTherapee range ses
retouches et ses notes dans un **profil** `IMAGE.RAF.pp3` à côté de l'image ; ART, qui en
dérive, fait de même dans `IMAGE.RAF.arp`, au même format. Un seul module sert aux deux ;
seule l'extension change.

## Le format

Un fichier de la forme INI : des sections `[Nom]`, des lignes `Clé=Valeur`, parfois des
commentaires (`#`, `;`). Les notes sont dans `[General]` :

| Clé | Valeur | Dans `tri` |
|---|---|---|
| `Rank` | 0 à 5 | les étoiles |
| `ColorLabel` | 0 aucune, 1 rouge, 2 jaune, 3 vert, 4 bleu, 5 violet | la couleur (une seule : la première de `tri`) |
| `InTrash` | `true` / `false` | `true` = rejetée (la corbeille du logiciel) |

Pas d'équivalent pour « acceptée » : ce drapeau reste dans le XMP. Comme partout, **rien
ne se perd** : seules ces trois clés changent (remplacées sur place, ajoutées si elles
manquent, la section créée en tête si elle manque), tout le reste, réglages, ordre des
lignes, commentaires et fins de ligne (`\r\n` sous Windows), est réécrit tel quel.

## Fonctions

- `parse(text)` / `apply(text, notes)` : lecture et réécriture du contenu ;
- `path(image, extension)` : `IMG.RAF.pp3` ou `IMG.RAF.arp` ;
- `is_development(text)` / `file_is_development(path)` : vrai s'il y a une section autre
  que `[Version]` et `[General]`. RawTherapee écrit un profil réduit à celles-ci quand on
  note une image dans son explorateur sans l'ouvrir ; `tri` aussi (`create`). Un tel
  profil n'est **pas un développement** : pas d'image `IMG@rt.RAF` dans la pile, rien à
  rendre (`develop::profile_sidecar`), comme pour le `.rrdata` de notes seules ;
- `development_content(text)` : le profil sans `[General]`, pour la clé de cache du rendu
  (noter ne refait pas le rendu, `develop::render_stamp`) ;
- `create`, `read`, `write` : le fichier, relu juste avant d'écrire, pas réécrit si rien
  ne change.

## Où il sert

- [sidecars.md](sidecars.md) : les formats `PP3` et `ARP` (`sidecars::Profile`,
  identifiants `pp3` / `arp`, logiciels `rawtherapee` / `art`), créés quand on note selon
  `[sidecars.rawtherapee]` / `[sidecars.art]`, ou à la demande ;
- [developments.md](developments.md) : `developments::find` reconnaît `IMG.RAF.pp3` et
  `IMG.RAF.arp` comme développement principal (pas de clones dans ces logiciels) ;
- [develop.md](develop.md) : le rendu par `rawtherapee-cli` / `ART-cli`.

Vérifié sur les docs et pages de manuel (RawPedia, `ART.1`) ; à confirmer sur de vrais
profils des deux logiciels (testeurs).
