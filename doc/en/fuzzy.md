# Module `tri::fuzzy`: fuzzy search

In the manner of fzf: each typed word must be found in the text letter by letter, in
order, possibly with gaps ("fdp" finds "folders panel"), ignoring accents and case;
words are searched independently ("dehors 2026" finds "2026/2026-10-17_Self_Dehors").
`score(query, text)`: `None` with no match, higher for consecutive letters and word
starts; empty query: `Some(0)`.

Born in the interface's help (2026-10-17), moved into the engine on 2026-10-19 to also
serve:

- **`NameFilter`**: grid filter by file name (`/` or Ctrl+F), on the same `Filter` trait
  as the other filters (combinable with stars, colours…); applies to the name only, not
  the folder;
- **folder search** (`places::DirSearch`, see `places.md`);
- the help and the settings window, through **`find(query, fields)`** (2026-10-22): each
  word within one of an entry's fields (label, identifier, description, keywords…), and
  `Match { exact, score }`, `exact` when each word appears in one piece; **`relevant`**
  keeps only the exact ones if there are any, so that scattered letters do not drown the
  right result.
- **Typos** (2026-11-06, help redesign): a typed word not found letter by letter is
  looked for within a few typos (extra, missing or replaced letter, two letters swapped:
  restricted Damerau-Levenshtein distance), in a word of the text or at its start (still
  typing); no typo under 4 letters, one up to 7, two beyond (`typos_allowed`). Always
  scores lower than a match without a typo. `Match` gains **`close`** (each word as is
  or within a few typos of a word): `relevant` keeps the exact ones, else the close
  ones, else the rest ("noter" brought up 44 actions with scattered letters).
- **Single letter** (2026-11-06, user feedback): a typed word of one letter is not looked
  for inside words; it only finds a single letter of the text (a key, "x", "f f", or a
  one-letter word).
- **Plural** (2026-11-06): a final "s" or "x" (beyond 3 letters) does not count when
  judging a word exact ("rendus vignette" finds "Rendu des vignettes").
- **Start of a word** (2026-11-07, `starts_words`): a strict match, each typed word at
  the start of a word of the text (without accents or case, singular as plural). For
  what must not come up on a resemblance: the control rows of the palette ("nettes" is
  inside "vignettes", not at its start; "rendu" looks like "réduire").
- **Letters to highlight** (2026-11-09, `matched_chars`, #382): for each typed word, its
  passage in one piece if it is there (at the start of a word preferably; singular too),
  otherwise its letters in order. A word found only through a typo is not highlighted.
  Used by the `?` help to show what matched.
