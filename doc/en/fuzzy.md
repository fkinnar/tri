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
