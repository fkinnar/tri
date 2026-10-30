# Réglages : fichier de configuration

Règle du projet (`CLAUDE.md`, 16/10/2026) : **pas de valeurs codées en dur**. Tout « potard »
(taille, délai, capacité, qualité, seuil, nombre de workers, couleur…) est un réglage, avec
sa valeur par défaut dans le code, et peut être changé dans un fichier de configuration
sans recompiler.

## Le fichier

Format **TOML** (choisi le 16/10/2026 : commentaires possibles, sections lisibles, standard
de l'écosystème Rust ; le JSON reste réservé à la frontière FFI prévue par l'architecture).

| Système | Emplacement par défaut |
|---|---|
| Linux et autres Unix | `$XDG_CONFIG_HOME/tri/config.toml` (si absolu), sinon `~/.config/tri/config.toml` |
| macOS | `~/Library/Application Support/tri/config.toml` |
| Windows | `%APPDATA%\tri\config.toml` |

Ou n'importe quel fichier avec `--config FICHIER`, pour `tri` comme pour `tri-gui`. Les
options de la ligne de commande (`--cache`, `--side`, `--workers`…) l'emportent sur le
fichier.

**Toutes les clés, leurs valeurs par défaut et leur rôle** : [`config.example.toml`](../config.example.toml)
(décrites en anglais : [`config.example.en.toml`](../config.example.en.toml), mêmes clés et
valeurs ; la fenêtre de réglages montre les descriptions dans la langue de l'interface).
Il suffit d'y copier les lignes à changer : toute clé absente garde sa valeur par défaut, et
un fichier absent équivaut aux réglages par défaut. Deux tests (moteur et interface)
vérifient que ce fichier d'exemple reste **exactement** égal aux valeurs du code : on ne
peut pas changer une valeur par défaut sans mettre à jour sa documentation.

| Section | Contenu | Lu par |
|---|---|---|
| `[preview]` | Tailles (vignette, aperçu « auto » ou fixe), règle `half_size`, seuils d'acceptation de l'aperçu boîtier | moteur |
| `[render]` | Développement LibRaw : balance des blancs, luminosité, luminosité automatique | moteur |
| `[cache]` | Dossier, qualité JPEG, taille maximale des 1:1 (éviction des moins récemment utilisés) | moteur |
| `[scheduler]` | Workers légers (« auto » = moitié des cœurs), vignettes d'abord, analyse d'exposition en fond | moteur |
| `[xmp]` | Convention d'un nouveau sidecar (`darktable` / `adobe`), une ou plusieurs couleurs | moteur |
| `[places]` | Emplacements du panneau des dossiers : masquer Time Machine, motifs à exclure, volumes fixés (sans éjection), intervalle de surveillance des volumes | moteur |
| `[exposure]` | Seuils de saturation et d'ombres bouchées (IL), cases des histogrammes, taille du masque | moteur |
| `[offload]` | Déchargement des cartes : racine, vérification, fichiers déjà déchargés, historique | moteur |
| `[remote]` | Copie en local d'un serveur que le système ne sait pas monter (FTP, SFTP) : destination, profondeur, lots | moteur |
| `[i18n]` | Langue des textes : `auto` (système), `fr`, `en` (`i18n.md`) | moteur |
| `[sharpness]` | Carte de netteté : seuil, épaississement, points isolés (`sharpness.md`) | moteur |
| `[files]` | Actions sur les fichiers : autorisées ou non, paires qui suivent, corbeille ou suppression définitive, éditeur externe (`files.md`) | moteur |
| `[data]` | Dossier des données (historique des déchargements, dernier dossier ouvert) | moteur |
| `[gui]` | Thème, profils de raccourcis, délai des séquences de touches (`gg`), fenêtre, politique des rendus RAW, budget de chargement | interface |
| `[gui.grid]` | Dimensions de la grille, vignettes en mémoire, taille des notes, opacité d'une photo rejetée, cadre du survol | interface |
| `[gui.view]` | Attente avant un 100 % RAW, grandes textures en mémoire, pas du déplacement clavier, taille des notes | interface |
| `[gui.folders]` | Panneau des dossiers : affiché, largeur, retrait, dossiers cachés | interface |
| `[gui.histogram]` | Histogramme (affiché, échelle IL / linéaire, taille du grand histogramme, IL montrés), zones (affichées, opacité), seuils des badges | interface |
| `[gui.metadata]` | Panneau des métadonnées : affiché, largeur, histogramme en grille et sa hauteur | interface |
| `[gui.sharpness]` | Carte de netteté affichée au démarrage, couleurs de la rotation, opacité | interface |
| `[gui.compare]` | Comparaison : images au plus, espace, cadre de l'image active | interface |
| `[gui.colors]` | Couleurs `"#rrggbb"` (sélection, cadre du survol, barres de progression, badge, erreurs, fond du plein écran, étoiles, drapeaux, couleurs de notation) | interface |
| `[keys]` | Raccourcis par action, qui remplacent ceux des profils (voir [gui.md](gui.md#actions-et-raccourcis-actionsrs)) | interface |

### Erreurs

Une clé inconnue dans une section (faute de frappe), une valeur du mauvais type ou hors
domaine est **signalée**, avec la ligne et la liste des clés valides :

```
attention : configuration ~/.config/tri/config.toml : TOML parse error at line 2, column 1
  |
2 | thumbnial = 300
  | ^^^^^^^^^
unknown field `thumbnial`, expected one of `thumbnail`, `standard`, … ; réglages par défaut utilisés
```

Le programme démarre quand même, avec **tous** les réglages par défaut (pas un mélange : un
fichier à moitié pris en compte serait plus trompeur). Une faute de frappe ne doit pas
empêcher de trier ses photos. L'interface affiche « configuration invalide » en rouge dans la
barre d'état, le message complet au survol ; la ligne de commande l'écrit sur la sortie
d'erreur. Un fichier demandé avec `--config` mais introuvable est signalé de même ; un
fichier absent à l'emplacement par défaut, non.

## Fenêtre de réglages

Dans l'interface, `Ctrl+,` ouvre une fenêtre qui modifie ce fichier sans perdre ses
commentaires ni ses autres clés (voir `config_edit.md` et `gui.md`).

## Module `tri::settings` (moteur)

- **`EngineSettings`** : les sections du moteur (`PreviewSection`, `RenderSection`,
  `CacheSection`, `SchedulerSection`). Les sections inconnues du moteur (`[gui]`) sont
  ignorées ici, pour que la ligne de commande lise le même fichier que l'interface.
  - `preview(screen_side)` → `preview::Settings` : résout la taille « auto » (côté long de
    l'écran ; `auto_fallback` sans écran, en ligne de commande) ;
  - `scheduler(raw_previews)` → `scheduler::Config` : résout « auto » (moitié des cœurs) ;
    la politique des rendus RAW est un paramètre car elle dépend de l'usage (l'interface les
    calcule à la demande, la démo `--previews` tout en fond) ;
  - `open_cache()` : cache au dossier configuré (sinon celui du système), avec la qualité
    configurée.
- **`Auto<T>`** : `"auto"` ou un entier positif, avec un message d'erreur clair pour toute
  autre valeur (`"automatique"`, `-1`, `1.5`).
- **`load::<T>(path)`** → `Loaded { settings, path, warning }` : générique sur le type du
  fichier entier, pour que l'interface lise le même fichier avec son propre type.
- **`parse::<T>(text)`**, **`default_path()`**.

Les valeurs par défaut des sections ne sont pas recopiées : elles viennent des types du
moteur (`preview::Settings::default()`, `RenderOptions::default()`,
`cache::DEFAULT_JPEG_QUALITY`, `scheduler::Config::default()`). Une seule source.

Pourquoi des types de fichier (`PreviewSection`…) distincts des types internes
(`preview::Settings`…) : le fichier contient des valeurs à résoudre (« auto ») et des choix
de présentation (pas de `half_size`, réglé par niveau) ; le contrat du fichier est ainsi
découplé de l'API interne, dans l'esprit de la frontière JSON prévue pour la FFI.

### Réglages et cache

Chaque réglage de calcul entre dans la clé de cache **des seuls niveaux qu'il influence**
(`preview::params`) : changer `auto_bright` recalcule les rendus RAW (aperçus standard et
1:1), pas les vignettes ni les aperçus boîtier. Deux exceptions, assumées et documentées dans
le code :

- `cache.jpeg_quality` n'entre pas dans la clé : les entrées déjà écrites représentent la
  même image ; vider le cache (`tri --clear-cache`) pour les réécrire ;
- le repli `half_size` des vignettes de RAW **sans** aperçu embarqué (rare) n'y met pas le
  rendu, pour ne pas invalider toutes les vignettes à chaque réglage de rendu.

## Interface : `gui/src/settings.rs`

`AppSettings` reprend les quatre sections du moteur et y ajoute `[gui]` (`GuiSettings`,
`GridSettings`, `ViewSettings`, `Colors`). Ici, **toute** section inconnue est une erreur
(`[previw]` est signalé). `Color` se lit depuis `"#rrggbb"` ; sélection et survol absents =
couleurs du thème. Une liste de profils de raccourcis vide revient aux profils par défaut
(sinon le clavier ne ferait plus rien).

## Pas encore réglable

Mis à jour à la revue du 28/10/2026 (l'éviction du cache 1:1, `[cache] zoom_max_mb`, et
les raccourcis, `[keys]`, sont réglables depuis longtemps).

- Les priorités du scheduler (classes 0 à 5) : c'est l'algorithme, pas un potard. De même
  les poids de la recherche floue (`fuzzy`) et le nombre d'essais de verrouillage d'un
  volume à éjecter sous Windows (`places`, d'après l'exemple de Microsoft).
- L'intervalle de surveillance d'un programme externe qui travaille (`darktable-cli`,
  RapidRAW : 50 ms, puis 20 ms pour ses dernières lignes) : à régler seulement s'il
  pesait, ce qui n'est pas le cas.
- Le dessin de l'interface : tailles de police, marges, espacements, épaisseurs de trait
  et une partie des couleurs (fonds et textes en surimpression, histogramme, poignées du
  recadrage, icônes). Ils relèveront d'un futur système de thèmes plutôt que d'une clé
  par endroit (voir le rapport de revue du 28/10/2026).

## État d'affichage (`gui-state.toml`)

À côté du dernier dossier ouvert, dans le dossier de données, l'interface garde ce qu'on
affiche ou masque d'une touche (collimateur, carte de netteté, zones, histogramme,
panneaux, barres, avance automatique) : ces valeurs l'emportent sur la configuration à
partir du deuxième lancement. `[gui] remember_display = false` pour toujours partir de la
configuration ; effacer `gui-state.toml` pour y revenir une fois.
