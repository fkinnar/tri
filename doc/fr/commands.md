# Module `tri::commands` : l'aide de la ligne de commande

Issue n° 441. Les commandes de `tri` sont décrites **une seule fois**, en données, dans
`src/commands.rs` (`COMMANDS`) : `tri --help`, le mode d'emploi court affiché sur une erreur
d'arguments, l'aide de l'interface (F1) et, le jour venu, la documentation d'une API en
viennent tous. Pas de `clap` : la lecture des arguments (`src/main.rs`) reste écrite à la
main ; un test garantit qu'elles ne se décalent pas.

## Une commande

`Command` : `id`, `theme` (`Theme` : dossier et aperçus, notes, fichiers, export,
déchargement, analyses, cache et données, divers ; l'ordre de l'aide), `syntax`, `main`
(les options qui font la commande, `--copy` et `--move`), `options` (les autres, chacune
décrite) et `example`.

- **Syntaxe** : `[…]` facultatif, `a|b` l'un ou l'autre, `…` répétable. Les mots entre
  `<…>` sont traduits (`cli-ph-<mot>` : `<folder>` devient `<dossier>`).
- **Textes** dans les catalogues du moteur (`locales/*.ftl`) : `cli-theme-<id>`,
  `cli-cmd-<id>` (une phrase), `cli-opt-<option>` ; une option qui dit autre chose dans une
  commande a son propre texte, `cli-opt-<id>-<option>` (`--to` du déchargement : la racine
  des séances ; `--side` de `--adjust` : la taille de l'image écrite), pris en premier
  (`Command::option_id`).
- **Exemple** : `Command::example(dossier, images)` remplit `<folder>` (le dossier),
  `<images>` (les images) et `<image>` (la première), chemins entre guillemets au besoin
  (`shell_quote` : simples sous Unix, doubles sous Windows) ; les autres mots restent,
  traduits (`<destination>`). C'est ce que l'interface copie dans le presse-papier.

## Sorties

- `usage()` : la syntaxe de chaque commande, et un renvoi vers `tri --help`.
- `help()` : par thème, chaque commande avec sa description, ses options et un exemple ;
  puis l'option commune `--config`. `tri --help` ou `tri -h`.
- `Command::form()` : la forme courte (parties obligatoires de la syntaxe, sans `tri` ni
  le dossier de tête), montrée à droite dans l'aide de l'interface.
- `Command::search_text()` : ce que la recherche de l'aide de l'interface parcourt.

## Tests

- Chaque option citée par une syntaxe est dans `main` ou `options`, et inversement.
- Chaque texte (thèmes, commandes, options, mots traduits) existe dans toutes les langues.
- Chaque thème a une commande, les identifiants sont uniques ; exemples remplis, chemins
  entre guillemets.
- `src/main.rs` : chaque option lue par `parse()` est citée par une commande (une option
  ajoutée sans aide fait échouer le test).
- `tests/cli.rs` : `tri --help` et `-h` réussissent et citent chaque commande.
