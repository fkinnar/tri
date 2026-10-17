# Module `tri::offload`

Déchargement d'une carte (ou de tout volume) vers un dossier local, puis tri dans le
même outil (demandé le 17/10/2026 : « vider la carte, trier, développer »). Sans
interface : la ligne de commande et l'interface graphique s'en servent tels quels.

## Choix (17/10/2026)

Les défauts suivent l'usage de l'utilisateur, tout est réglable ; principe : couvrir les
besoins de tous, pas seulement les siens.

- **Destination** : `[offload] destination` (`~/Pictures` par défaut, `~` accepté) +
  dossier de la séance. Proposé : `AAAA/AAAA-MM-JJ_` (`suggest_folder`, date de prise de
  vue de la plus ancienne image, lue dans ses métadonnées en heure locale ; à défaut, sa
  date de modification en UTC), à compléter (« _Self ») ou à remplacer entièrement. Un
  `_` laissé seul en fin de nom est retiré. Le nom ne peut ni être absolu ni remonter
  (`..`) : on ne copie que sous la racine. Plus tard : gabarits (placeholders).
- **Noms d'origine**, à plat : les sous-dossiers de la carte (`DCIM/100_FUJI`…) ne sont
  pas reproduits. Un nom déjà pris par un *autre* fichier reçoit un suffixe
  (`DSCF0001-2.RAF`, compteur du boîtier remis à zéro) ; rien n'est jamais écrasé.
  Plus tard : renommage par gabarits.
- **Doublons** : les fichiers déjà déchargés sont ignorés, même vers un autre dossier,
  grâce à un **historique** (`offload-history.tsv` dans le dossier de données :
  `~/.local/share/tri`, `~/Library/Application Support/tri`, `%APPDATA%\tri` ; ou
  `[offload] history`). Pas dans le cache, qu'on peut vider sans rien perdre. Un fichier
  y est identifié par nom, taille et date (les numéros repartent à zéro : le nom seul ne
  suffit pas). Une copie identique déjà présente à l'arrivée est aussi ignorée (et
  inscrite). RAW et JPEG du boîtier ne sont pas censés changer : ce critère suffit.
- **Vérification** (`[offload] verify`) : empreinte BLAKE3 calculée pendant la copie,
  puis copie relue et comparée ; date de modification d'origine conservée ; écriture
  atomique (fichier temporaire caché, puis renommage).
- **Choix faits avant la copie, enchaînés à la fin** (retour du 17/10/2026 : devoir
  choisir entre « ouvrir » et « éjecter » n'avait pas de sens) : cases à cocher, défauts
  dans `[offload]` :
  - **RAW seulement** (`raw_only`, non) : les images plates (JPEG du boîtier) restent sur
    la carte (`Report::left`) ; elles ne sont pas « déjà déchargées » pour la suite ;
  - **ouvrir le dossier à la fin** (`open_after`, oui) ;
  - **éjecter la carte à la fin** (`eject_after`, oui), sauf si la copie a été
    interrompue ;
  - **supprimer de la carte les fichiers copiés et vérifiés** (`delete_after`, non) :
    `delete_sources`, seulement eux ; chaque copie est encore contrôlée — présente, même
    taille — juste avant ; pas après une interruption.

  Ordre : suppression, ouverture, éjection ; le bilan reste affiché, avec des boutons
  pour ce qui n'a pas été coché.
- Seules les images (RAW et plates) sont copiées ; vidéos et fichiers du boîtier sont
  comptés (`Plan::ignored`) et laissés.

## API

- `History::open(fichier)`, `History::open_configured(&réglages)`, `History::in_memory()` ;
- `plan(carte, &historique) -> Plan` : images (`Item` : source, taille, date, `known`),
  autres fichiers ; `Plan::to_copy`, `Plan::bytes` ;
- `suggest_folder(&plan, skip_known)`, `destination(racine, nom)` ;
- `run(&plan, destination, &réglages, &mut historique, &annulation, progression) ->
  Report` (copiés, ignorés, erreurs, interrompu, durée) ; `Progress` : fichiers et octets ;
- `delete_sources(&report)`.

## Ligne de commande

```
tri --offload <carte> [--to <racine>] [--folder <nom>] [--raw-only] [--dry-run] [--previews] [--delete-copied] [--eject]
```

En ligne de commande, rien ne se fait sur la carte sans option explicite (`--eject`,
`--delete-copied` ; `open_after`, `eject_after`, `delete_after` ne valent que pour
l'interface). `--dry-run` liste ce qui serait copié ; `--previews` calcule ensuite vignettes, aperçus et
histogrammes de la séance. Mesuré (17/10/2026, image disque locale, Mac Apple Silicon) :
4 RAF (109 Mo) copiés et vérifiés en 0,23 s ; second passage : « rien à copier ».

## Interface

Icône « décharger » à côté de l'icône d'éjection de chaque volume non fixé du panneau des
dossiers : boîte (modale, près du clic) avec inventaire en fond, racine et nom modifiables
(destination complète affichée), cases à cocher ci-dessus, copie en fond avec barre de
progression et « Interrompre », puis suites enchaînées et bilan (la séance s'ouvre dans
la grille et ses calculs démarrent). Voir [gui.md](gui.md).
