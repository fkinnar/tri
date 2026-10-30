# Module `tri::rrdata` — notes dans les sidecars de RapidRAW

RapidRAW range les réglages et les notes d'une image dans un fichier JSON à côté d'elle :
`IMAGE.RAF.rrdata` pour le développement principal, `IMAGE.RAF.<id>.rrdata` pour chaque
copie virtuelle (en interne, RapidRAW la désigne par `IMAGE.RAF?vc=<id>`). Vérifié dans
son code source le 30/10/2026.

Chaque développement garde ses notes là où son logiciel les lit (décidé le 30/10/2026) :
un développement RapidRAW, principal ou copie, a donc les siennes dans son `.rrdata`.

## Champs

| Champ | Contenu | Dans `tri` |
|---|---|---|
| `rating` | 0 à 5 | étoiles |
| `flag` | `"pick"`, `"reject"`, absent sinon | accepté, rejeté, sans drapeau |
| `tags` | liste ; la couleur y est l'étiquette `"color:red"` (une seule : `red`, `yellow`, `green`, `blue`, `purple`) | couleur (la première, s'il y en a plusieurs) |
| `adjustments` | les réglages | rendu d'une copie virtuelle (`rapidraw export --adjustments`) |

## Ne rien perdre

Comme pour les XMP : seuls `rating`, `flag` et l'étiquette de couleur changent ; le reste
(réglages, mots-clés, champs inconnus) est réécrit tel quel, dans le même ordre (la
feature `preserve_order` de `serde_json`). Le fichier est relu juste avant d'écrire, et
n'est pas réécrit si les notes n'ont pas changé.

## Fonctions

- `parse(json)` / `read(path)` : les notes.
- `apply(json, notes)` / `write(path, notes)` : les notes écrites, le reste gardé.
- `adjustments(json)` : les réglages, pour rendre une copie virtuelle.

## Fichiers

Les copies virtuelles suivent l'image copiée, déplacée ou renommée, comme le
`.rrdata` principal (`xmp::SidecarIndex::rapidraw_copies`, voir `files.md`).
