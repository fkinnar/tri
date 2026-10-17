# Module `tri::offload`

Déchargement d'une carte (ou de tout volume) vers un dossier local, puis tri dans le
même outil (demandé le 17/10/2026 : « vider la carte, trier, développer »). Sans
interface : la ligne de commande et l'interface graphique s'en servent tels quels.

## Choix (17/10/2026)

Les défauts suivent l'usage de l'utilisateur, tout est réglable ; principe : couvrir les
besoins de tous, pas seulement les siens.

- **Destination** : racine + dossier de la séance. La racine est **mémorisée** d'un
  déchargement à l'autre (`root` / `remember_root`, fichier `offload-root` du dossier de
  données) ; `[offload] destination` (`~/Pictures`, `~` accepté) ne sert qu'au premier.
  Dossier de la séance proposé : `AAAA-MM-JJ_` (`suggest_folder`, date de prise de vue
  de la plus ancienne image à copier, lue dans ses métadonnées en heure locale ; à
  défaut, sa date de modification en UTC ; carte déjà déchargée : la plus ancienne de la
  carte), à compléter (« _Self ») ou à remplacer entièrement. L'année, si on la veut,
  va dans la racine (`…/Sources/2026`) : retour du 17/10/2026, la proposition
  `AAAA/AAAA-MM-JJ_` mélangeait les deux, et restait vide pour une carte déjà
  déchargée. Un
  `_` laissé seul en fin de nom est retiré. Le nom ne peut ni être absolu ni remonter
  (`..`) : on ne copie que sous la racine. Plus tard : gabarits (placeholders).
- **Noms d'origine**, à plat : les sous-dossiers de la carte (`DCIM/100_FUJI`…) ne sont
  pas reproduits. Un nom déjà pris par un *autre* fichier reçoit un suffixe
  (`DSCF0001-2.RAF`, compteur du boîtier remis à zéro) ; rien n'est jamais écrasé.
  Plus tard : renommage par gabarits.
- **Doublons**, jugés sur le **dossier de destination** (retour du 17/10/2026 : une même
  carte doit pouvoir être déchargée deux fois, à deux endroits) : une copie identique
  déjà présente (même contenu, empreinte comparée) est ignorée ; la boîte annonce avant
  la copie combien d'images sont déjà dans la destination choisie (`already_in` : même
  nom, même taille ; recalculé quand le dossier change). En option (`[offload]
  skip_known`, non par défaut), un **historique global** (`offload-history.tsv` dans le
  dossier de données, ou `[offload] history`) fait ignorer ce qui a déjà été déchargé
  n'importe où ; un fichier y est identifié par nom, taille et date (les numéros
  repartent à zéro : le nom seul ne suffit pas). L'historique est tenu dans tous les
  cas.
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
    `delete_sources` — ceux copiés pendant ce déchargement **et** ceux déjà présents à
    l'identique dans la destination (`Report::already_there`, empreinte comparée : carte
    déchargée une seconde fois au même endroit ; ajouté le 17/10/2026, ils restaient
    sur la carte), jamais ceux que seul l'historique global connaît (vérifiés nulle
    part), ni les erreurs, ni les vidéos ; chaque copie est encore contrôlée — présente, même
    taille — juste avant ; pas après une interruption.

  - **supprimer aussi les JPEG laissés sur la carte** (`delete_left`, non ; proposé
    seulement avec « RAW seulement » et la suppression cochées) : `delete_left`, pour ceux
    qui ne gardent que les RAW (ils ne sont pas copiés : perdus).

  Ordre : suppression, ouverture, éjection. Si le système refuse l'éjection (volume
  utilisé par une autre application : vu le 17/10/2026 avec ForkLift ouvert sur la
  carte, « Unmount was dissented by PID … ForkLift »), le message s'affiche avec un
  bouton « Réessayer l'éjection » dans la barre d'état ; le bilan reste affiché, avec des boutons
  pour ce qui n'a pas été coché.
- Seules les images (RAW et plates) sont copiées ; vidéos et fichiers du boîtier sont
  comptés (`Plan::ignored`) et laissés.

## En arrière-plan (17/10/2026)

Pour trier une séance pendant qu'une autre carte se décharge : pendant la copie,
« Continuer en arrière-plan » (ou Échap, ou un clic à côté) referme la boîte ; la copie
continue, la progression s'affiche dans la barre d'état (un clic rouvre la boîte, avec
« Interrompre »). À la fin, en arrière-plan, suppression et éjection cochées se font
comme prévu, mais la séance n'est **pas ouverte d'office** — ce serait changer de dossier
sous les yeux de l'utilisateur — : la barre d'état propose « Ouvrir la séance » (et
« ignorer »), avec le bilan. Boîte restée affichée jusqu'au bout : ouverture automatique
comme avant. Un seul déchargement à la fois : l'icône « décharger » disparaît des volumes
tant qu'un déchargement existe.

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
