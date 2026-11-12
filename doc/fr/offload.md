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
  `_` laissé seul en fin de nom est retiré ; un nom **vide** décharge dans la racine même
  (#281 : « décharger ici » dans un dossier qu'on vient de créer). Le nom ne peut ni être
  absolu ni remonter
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
- **Destination pleine** (retour du 12/11/2026 : le déchargement échouait sans rien
  dire, chaque fichier suivant en erreur). Une copie qui échoue faute de place
  (`is_out_of_space` : disque plein ou quota dépassé) n'est pas une erreur ordinaire :
  `run` s'arrête sur ce fichier et appelle `full` avec un `OutOfSpace` (fichiers déjà
  copiés, fichiers et octets restant à copier, place libre, `free_space` : `statvfs`
  sous macOS et Linux, `GetDiskFreeSpaceExW` sous Windows). `true` réessaie le **même**
  fichier, `false` arrête là (`Report::out_of_space`) : le reste demeure sur la carte, ni
  copié ni en erreur, et rien n'est supprimé de la carte ni éjecté. Dans l'interface, la
  boîte revient au premier plan (même en arrière-plan) avec ce qui est fait, ce qui reste
  et la place libre, et y reste sans limite de temps (Échap ne la ferme pas) jusqu'à
  « Réessayer » (après avoir fait de la place) ou « Abandonner ». Avant la copie, la boîte
  compare la taille des fichiers à copier à la place libre et prévient si elle ne suffit
  pas ; le bouton devient alors « Décharger quand même » (en connaissance de cause). En
  ligne de commande, pas de question : un message, l'arrêt, pas d'éjection, et un code
  de sortie d'erreur.
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

## Plusieurs cartes à la suite (25/10/2026, issue n° 37)

La boîte restée ouverte sur le bilan d'une carte (éjectée ou non), une nouvelle carte
branchée : la boîte repart sur elle (`OffloadDialog::next_card`, appelé par la
surveillance des volumes, `TriApp::set_volumes`), vers **le même dossier de séance**
(racine et nom repris du déchargement précédent), avec les mêmes choix ; il ne reste
qu'à lancer la copie. Seuls comptent les supports éjectables (pas un disque fixé,
`[places] fixed`). Les aperçus de chaque séance se calculent à part : l'application
garde une liste de leurs schedulers.

Retours du soir même : la boîte n'est pas obligée d'être restée ouverte. Bilan refermé,
ou copie faite en arrière-plan : l'application garde le dernier déchargement fini
(`TriApp::last_offload` : séance et choix de la boîte), et une carte d'appareil (avec un
dossier `DCIM`) branchée ensuite ouvre la boîte sur elle, vers cette séance
(`OffloadDialog::propose_session`). Refermée sans décharger, plus rien n'est proposé
ensuite. La nouvelle carte peut être montée **au même chemin** que la précédente (deux
cartes du même boîtier ont le même nom de volume, `/media/…/disk`) : elle vient
d'apparaître, c'est une autre carte.

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

## Aperçus pendant la copie (25/10/2026, issue n° 40)

On décharge une carte pour travailler dessus : attendre la fin de la copie pour calculer
les aperçus, c'est du temps perdu, alors que la copie (lectures et écritures) laisse le
processeur presque libre. Chaque photo copiée et vérifiée (ou trouvée identique dans la
destination) rejoint aussitôt un calcul en fond : vignettes, rendus boîtier,
histogrammes, netteté, regroupement, comme à l'ouverture d'un dossier.

- **Depuis la copie, pas depuis la carte** : la photo vient d'être écrite, le système
  l'a encore en mémoire, et la lecture de la carte n'est pas ralentie.
- **Un scheduler à part** (`Scheduler::add_files` : photos ajoutées une à une, sans rien
  interrompre), aux réglages d'aperçu du moteur de l'interface : mêmes clés que le
  dossier ouvert ensuite. Il est gardé par l'application, pas par la boîte (elle peut se
  fermer avant la fin) ; il s'arrête quand la séance s'ouvre (le moteur du dossier reprend
  ce qui reste, le fait est lu dans le cache) ou une fois tout calculé.
- **`[cache] location`** est appliqué à la séance avant la copie : son cache se remplit
  directement à sa place.
- **« Garder les données de tri sur ce disque »** (11/11/2026, issue n° 378,
  [store.md](store.md)) : case cochée d'office quand la destination est sur un disque
  amovible (sinon « …dans le dossier de la séance », non cochée) ; l'interrupteur est
  posé avant la copie, le cache et les données de la séance vont directement sur le
  disque. Décochée : `[cache] location` ne s'applique pas non plus.
- **Réglage** `[offload] previews` (oui par défaut), case « Calculer les aperçus pendant
  la copie » de la boîte. En ligne de commande, `--previews`.
- Limite connue : en arrière-plan, pendant qu'on regarde un autre dossier, deux workers
  LibRaw peuvent tourner en même temps (celui du dossier affiché, celui de la séance).

## API

- `History::open(fichier)`, `History::open_configured(&réglages)`, `History::in_memory()` ;
- `plan(carte, &historique) -> Plan` : images (`Item` : source, taille, date, `known`),
  autres fichiers ; `Plan::to_copy`, `Plan::bytes` ;
- `suggest_folder(&plan, skip_known)`, `destination(racine, nom)` ;
- `run(&plan, destination, &réglages, &mut historique, &annulation, progression,
  copie) -> Report` (`copie` : chaque copie dès qu'elle est faite, pour les aperçus) (copiés, ignorés, erreurs, interrompu, durée) ; `Progress` : fichiers (sans ceux laissés sur la carte) et octets (un fichier passé, déjà déchargé ou déjà présent, compte d'un coup : la barre suit les fichiers) ;
- `delete_sources(&report)`.

## Ligne de commande

```
tri --offload <carte> [--to <racine>] [--folder <nom>] [--raw-only] [--dry-run] [--previews] [--delete-copied] [--eject]
```

En ligne de commande, rien ne se fait sur la carte sans option explicite (`--eject`,
`--delete-copied` ; `open_after`, `eject_after`, `delete_after` ne valent que pour
l'interface). `--dry-run` liste ce qui serait copié ; `--previews` calcule vignettes, aperçus et
analyses de la séance pendant la copie, puis attend la fin des calculs. Mesuré (17/10/2026, image disque locale, Mac Apple Silicon) :
4 RAF (109 Mo) copiés et vérifiés en 0,23 s ; second passage : « rien à copier ».

## Interface

Icône « décharger » à côté de l'icône d'éjection de chaque volume non fixé du panneau des
dossiers : boîte (modale, au centre) avec inventaire en fond, racine et nom modifiables
(destination complète affichée), cases à cocher ci-dessus, copie en fond avec barre de
progression et « Interrompre », puis suites enchaînées et bilan (la séance s'ouvre dans
la grille et ses calculs démarrent). Voir [gui.md](gui.md).
