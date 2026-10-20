# Module `tri::fuzzy` : recherche floue

À la façon de fzf : chaque mot tapé doit se retrouver dans le texte lettre par lettre,
dans l'ordre, éventuellement avec des trous (« pnd » trouve « panneau des dossiers »),
sans tenir compte des accents ni de la casse ; les mots se cherchent indépendamment
(« dehors 2026 » trouve « 2026/2026-10-17_Self_Dehors »). `score(requête, texte)` :
`None` sans correspondance, plus grand pour des lettres consécutives et des débuts de
mots ; requête vide : `Some(0)`.

Né dans l'aide de l'interface (17/10/2026), déplacé dans le moteur le 19/10/2026 pour
servir aussi :

- **`NameFilter`** : filtre de la grille par nom de fichier (`/` ou Ctrl+F), sur le même
  trait `Filter` que les autres filtres (combinable avec étoiles, couleurs…) ; porte sur le
  nom seul, pas le dossier ;
- la **recherche de dossiers** (`places::DirSearch`, voir `places.md`) ;
- l'aide et la fenêtre de réglages (l'interface le réexporte en `actions::fuzzy_score`).
