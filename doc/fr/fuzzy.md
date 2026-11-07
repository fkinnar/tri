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
- l'aide et la fenêtre de réglages, par **`find(requête, champs)`** (22/10/2026) :
  chaque mot dans un des champs d'une entrée (libellé, identifiant, description,
  mots-clés…), et `Match { exact, score }`, `exact` quand chaque mot y apparaît d'un seul
  tenant ; **`relevant`** ne garde que les exactes s'il y en a, pour que des lettres
  éparpillées ne noient pas le bon résultat.
- **Fautes de frappe** (06/11/2026, refonte de l'aide) : un mot tapé introuvable lettre à
  lettre est cherché à quelques fautes près (lettre en trop, en moins, remplacée, deux
  lettres inversées : distance de Damerau-Levenshtein restreinte), dans un mot du texte ou
  à son début (on tape encore) ; aucune faute sous 4 lettres, une jusqu'à 7, deux
  au-delà (`typos_allowed`). Score toujours plus bas qu'une correspondance sans faute.
  `Match` gagne **`close`** (chaque mot tel quel ou à quelques fautes près d'un mot) :
  `relevant` garde les exactes, sinon les proches, sinon le reste (« noter » remontait 44
  actions aux lettres éparpillées).
- **Lettre seule** (06/11/2026, retour de l'utilisateur) : un mot tapé d'une seule lettre
  n'est pas cherché dans les mots ; il ne trouve qu'une lettre seule du texte (une
  touche, « x », « f f », ou un mot d'une lettre).
- **Pluriel** (06/11/2026) : un « s » ou « x » final (au-delà de 3 lettres) ne compte pas
  pour juger un mot exact (« rendus vignette » trouve « Rendu des vignettes »).
- **Début de mot** (07/11/2026, `starts_words`) : correspondance stricte, chaque mot tapé
  au début d'un mot du texte (sans accents ni casse, au singulier comme au pluriel). Pour
  ce qui ne doit pas sortir sur une ressemblance : les lignes de contrôle de la palette
  (« nettes » est dans « vignettes », pas au début ; « rendu » ressemble à « réduire »).
