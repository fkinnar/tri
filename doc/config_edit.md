# Module `tri::config_edit` : modifier la configuration depuis une interface

Décidé le 17/10/2026, fait le 18/10/2026 : les réglages sont devenus nombreux
(`config.example.toml`), il faut pouvoir les changer sans ouvrir le fichier à la main.
L'interface (fenêtre de réglages, `Ctrl+,`, voir `gui.md`) s'appuie sur ce module, qui
ne dépend d'aucun affichage (un futur TUI pourrait faire de même).

## `Schema` : la liste des réglages vient du fichier d'exemple

`Schema::parse(texte)` lit `doc/config.example.toml` : sections (`gui.colors`), clés,
valeurs par défaut et **commentaires**. Pourquoi l'exemple plutôt qu'une liste écrite
dans le code :

- un test vérifie déjà que l'exemple reste égal aux valeurs par défaut du code
  (`settings.md`) ; la liste est donc juste, et tout nouveau réglage (qui doit naître
  dans l'exemple, règle du projet) apparaît de lui-même dans la fenêtre ;
- les commentaires de l'exemple sont la documentation de chaque réglage : la fenêtre les
  montre tels quels.

Un commentaire au-dessus d'un groupe de clés (« Badges de la grille : … » au-dessus de
`badge_over` et `badge_under`) décrit le groupe : `Group { doc, entries }`. Seul le
dernier paragraphe d'un commentaire compte (l'en-tête du fichier, séparé par une ligne
vide, n'est pas la description de `[preview]`). Les sections sans clé (`[keys]`, dont
les clés sont des identifiants d'action choisis par l'utilisateur) sont omises, ainsi
que les clés seulement citées en commentaire (`# selection = "#1e6f7f"`, réglage
facultatif sans valeur par défaut) : à ajouter à la main dans le fichier.

Limite connue : les commentaires de l'exemple sont en français ; un exemple anglais
serait à ajouter pour les descriptions en anglais.

## `ConfigFile` : modifier sans rien perdre

`ConfigFile::open(chemin)` (fichier absent : vide, créé à l'enregistrement), `get`,
`set`, `remove` (retour à la valeur par défaut), `text`, `save`. Le fichier est tenu par
`toml_edit`, qui garde le texte d'origine autour de ce qu'on change : commentaires,
ordre, mise en page, clés inconnues de l'interface. Une valeur remplacée garde le
commentaire de sa ligne (`thumbnail = 600 # à moi`). Une sous-section créée
(`[gui.colors]`) ne crée pas de `[gui]` vide. L'enregistrement passe par un fichier
temporaire renommé : jamais de fichier à moitié écrit.

Les valeurs : `Value::{Bool, Int, Float, Str, Toml}` ; `Toml` pour les listes et tables,
éditées comme du texte TOML (`[1280.0, 800.0]`). `parse_color` reconnaît `"#rrggbb"`.

## Validation : à l'appelant

Le module ne connaît pas le type complet des réglages (l'interface ajoute les siens à
ceux du moteur). L'appelant valide le texte modifié avec `tri::settings::parse::<T>` :
la fenêtre de réglages refuse une valeur qui rendrait le fichier illisible (mauvais type,
valeur hors de l'énumération), remet l'ancienne et montre le message de `serde` (qui
liste les valeurs possibles).
