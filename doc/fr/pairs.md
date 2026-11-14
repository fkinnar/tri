# Paires RAW+JPEG regroupées (`tri::pairs`)

Issue n° 392, décidée le 09/11/2026. Une carte réglée en RAW+JPEG donne deux fichiers par
photo (`DSCF0001.RAF` et `DSCF0001.JPG`). Le JPEG n'est qu'un développement fait dans le
boîtier : **regroupée**, la paire est **une seule vignette** dans la grille, et le JPEG est
rangé avec les développements du RAW (darktable, RapidRAW…), avec la puce « Boîtier ».
On le voit en dépliant la pile de développements (Maj+V), comme un clone darktable.

Rien ne change sur le disque : le JPEG reste un fichier à part. Ses notes sont déjà
partagées avec le RAW ; copier, déplacer, renommer et supprimer emportent déjà la paire
(`[files] pairs`).

## Décision par dossier

- **Réglage global** `[files] group_pairs` : `ask` (par défaut), `separate` ou `grouped`.
- **Décision du dossier**, retenue avec ses données de tri ([store.md](store.md)) : elle
  suit le dossier quand « garder les données de tri ici » est activé. Elle l'emporte
  toujours sur le réglage global.
- Avec `ask`, l'ouverture d'un dossier qui a des paires et pas encore de décision ouvre
  la boîte « Regrouper RAW et JPEG ? » (la grille reste séparée en attendant) :
  « Regrouper », « Garder séparées », et la case « Ne plus demander », qui écrit la
  réponse dans le réglage global. Fermée sans répondre : on redemandera.
- Pour changer d'avis : menu du dossier ouvert (clic droit, groupe Fichiers), ou la
  palette, action `core.dev.pairs` (sans raccourci par défaut).
- Sous-dossiers affichés avec le dossier : chaque image suit la décision de son propre
  dossier.

## Filtres

Dans un dossier regroupé, les cases RAW et JPEG du panneau de droite deviennent
**trois** : RAW (les RAW sans JPEG), RAW+JPEG (les paires regroupées), JPEG (les JPEG sans
RAW). Combinables, une au moins cochée ; la touche du type (`core.filter.cycle`) passe de
tout à chacune seule. Un développement suit sa photo ; retenues avec les filtres du
dossier.

## Moteur

- `Grouping` : les trois valeurs du réglage.
- `decided(data, folder)` / `decide(data, folder, Some(bool) | None)` : la décision du
  dossier, rangée dans ses valeurs (`tuning.json`, clé `pairs_grouped`).
- `grouped(setting, data, folder)` : la décision, sinon le réglage.
- `PairKinds`, `PairFilter` : les trois cases et leur filtre (trait `Filter`).
- `camera_jpegs(files)` : les images plates qui ont un RAW de même nom (sans tenir compte
  de la casse) dans le même dossier, avec ce RAW. Aussi utilisé par le résumé du dossier
  ([folder.md](folder.md)).

## Interface

Les piles de développements regroupent les images qui ont la même photo : pour le JPEG
d'une paire regroupée, cette photo est son RAW (`TriApp::stack_photo`). La couverture de
la pile reste le RAW.

- **Résumé du dossier** : dans un dossier regroupé, la ligne « Paires RAW+JPEG » est un
  filtre (la case RAW+JPEG seule) ; séparées, sa bulle dit comment regrouper.
- **Déchargement** : une case « Regrouper RAW et JPEG (n paires) », montrée seulement si
  ce qui sera copié a des paires (donc pas avec « RAW seulement »), cochée d'office avec
  `group_pairs = "grouped"` ; sa réponse est retenue pour le dossier de la séance une fois
  la copie faite (`Plan::pairs`). Avec « demander », la case non touchée laisse la
  question pour l'ouverture.
- **Panneau des dossiers** : une marque (deux feuilles décalées) après le nom d'un
  dossier regroupé, lue une fois par dossier.

À venir (même issue et n° 391) : le JPEG en pleine résolution comme rendu boîtier, et la
couverture par défaut.
