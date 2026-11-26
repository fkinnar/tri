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
  de la plus **récente** image à copier, lue dans ses métadonnées en heure locale ; à
  défaut, sa date de modification en UTC ; carte déjà déchargée : la plus récente de la
  carte ; la plus ancienne jusqu'au 12/11/2026, qui datait le dossier d'une séance déjà
  reprise sur une carte de plusieurs jours ; le nom proposé suit les cases « RAW
  seulement » et « Décharger à nouveau » tant qu'on ne l'a pas changé), à
  compléter (« _Self ») ou à remplacer entièrement. L'année, si on la veut,
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
  nom, même taille ; recalculé quand le dossier change). Un **historique global**
  (`offload-history.tsv` dans le dossier de données, ou `[offload] history`) fait aussi
  ignorer ce qui a déjà été déchargé n'importe où, **par défaut** depuis le 12/11/2026
  (`[offload] skip_known = true` ; demande de l'utilisateur : ne pas redécharger ce qu'on
  a déjà repris ; d'abord une option de la configuration seule, non par défaut). La boîte
  le dit : « 1686 images : 1200 déjà déchargées, 486 nouvelles », et propose, s'il y en a,
  une case « Décharger à nouveau les n images déjà déchargées » (décochée) pour décharger
  une carte une seconde fois ailleurs. Un fichier y est identifié par
  nom, taille et date (les numéros repartent à zéro : le nom seul ne suffit pas), sans
  base de données : un fichier texte, une ligne par copie. L'historique est tenu dans
  tous les cas. Il est propre à chaque machine (dossier de données) : une carte
  déchargée sur le Mac n'est pas connue sous Linux.
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
  pas ; le bouton devient alors « Décharger quand même » (en connaissance de cause). Les
  messages restent courts, tailles en Go (retour du 18/11/2026 : long et confus) :
  « Pas assez de place : 12,4 Go à copier, 7,8 Go libres. », la suite dans la bulle du
  bouton. En
  ligne de commande, pas de question : un message, l'arrêt, pas d'éjection, et un code
  de sortie d'erreur.
- **Vérification** (`[offload] verify`) : empreinte BLAKE3 calculée pendant la copie,
  puis copie relue et comparée ; date de modification d'origine conservée ; écriture
  atomique (fichier temporaire caché, puis renommage).
- **Choix faits avant la copie, enchaînés à la fin** (retour du 17/10/2026 : devoir
  choisir entre « ouvrir » et « éjecter » n'avait pas de sens) : cases à cocher, défauts
  dans `[offload]` :
  - **RAW seulement** (`raw_only`, non) : les images plates (JPEG du boîtier) restent sur
    la carte (`Report::left`) ; elles ne sont pas « déjà déchargées » pour la suite.
    Exclusive de « Regrouper RAW et JPEG » (#469) : les deux cases restent en place,
    l'une grisée quand l'autre est cochée, une bulle dit pourquoi ;
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
(`OffloadDialog::propose_session`) — seulement si elle n'a qu'une séance : une carte de
plusieurs séances (la même, rebranchée) garde ses lignes (retour du 26/11/2026). Refermée sans décharger, plus rien n'est proposé
ensuite. La nouvelle carte peut être montée **au même chemin** que la précédente (deux
cartes du même boîtier ont le même nom de volume, `/media/…/disk`) : elle vient
d'apparaître, c'est une autre carte.

## Carte de plusieurs séances (25/11/2026, #507)

Une carte peut garder plusieurs séances ; souvent une seule est à décharger.
`Plan::sessions(&réglages)` coupait d'abord la carte par jour de prise de vue (#600 : aussi aux pauses, voir plus bas) (date du fichier, posée
par le boîtier, en heure locale ; une photo prise avant `[offload] day_cut` heures, 4 par
défaut, compte pour la veille : une soirée qui finit après minuit reste une séance). Une
paire RAW+JPEG reste dans la séance de sa première image. `Plan::subset` restreint
l'inventaire aux images choisies : la copie vérifiée, l'historique et la suppression de la
carte ensuite ne touchent qu'elles, jamais le reste de la carte.

- **Boîte** : avec plusieurs séances, une ligne par séance (case avec le jour et les heures, dossier, nombre d'images et
  taille à copier, ou « déjà déchargée ») à la place du champ « Dossier de la séance ».
  Cochées d'office : celles qui ont du nouveau. **Un dossier par séance cochée**
  (`AAAA-MM-JJ_` à compléter), copiées l'une après l'autre dans le même fil ; les bilans
  sont mis bout à bout (`Report::absorb`), la plus récente séance ouverte à la fin.
- **Ligne de commande** : `tri --offload <carte> --session AAAA-MM-JJ` ne décharge que ce
  jour-là (dossier nommé d'après lui ; `AAAA-MM-JJ_HHMM` quand le jour a plusieurs séances) ; `--dry-run` liste les séances.
- **Choisir les photos** d'une séance (« Choisir les photos… » sur sa ligne,
  `offload_picker.rs`) : une galerie à la place de la boîte (idée de l'utilisateur,
  25/11/2026 : plus simple que la grille de `tri`, qu'il aurait fallu ouvrir sur la carte
  puis rendre telle quelle). Vignettes tirées des JPEG embarqués, lues en fond sur la
  carte sans cache ; une paire RAW+JPEG est une photo ; taille réglable (curseur) ; coche
  sur chaque photo, toutes cochées au départ, « Tout sélectionner » / « Tout
  désélectionner » et le compte. Gestes de la grille : clic = curseur (il ne décoche pas
  tout, contrairement au Finder), Cmd/Ctrl+clic bascule, Maj+clic coche la plage, clic sur
  la coche ; Espace bascule, Maj+Espace le mode visuel (les déplacements cochent),
  Maj + flèches étendent, flèches ou hjkl, Cmd/Ctrl+A tout, avec Maj rien. Double-clic ou
  Entrée : la photo en grand, ajustée (l'aperçu embarqué, pas de 1:1), flèches pour la
  suivante. Échap sort du visuel, de la vue en grand, puis ferme sans rien changer ;
  Valider revient à la boîte (« Changer le choix… »), seule la sélection est copiée.

## Séances coupées où l'on veut (26/11/2026, #600)

Les jours ne sont qu'un cas particulier (décision de l'utilisateur) : la carte est coupée
en séances **là où l'on veut**, et les changements de jour sont une des coupures
proposées. Exemple : le matin à Bruxelles, l'après-midi à Anvers, deux séances le même
jour.

- **Coupures proposées** (`Plan::proposed_cuts`) : à chaque changement de jour
  (`[offload] split_days`, coupé à `day_cut`) et là où la prise de vue s'est arrêtée au
  moins `split_pause_minutes` (120 par défaut, 0 : jamais). Au plus `max_sessions`
  séances (30) : au-delà, les jours puis les plus longues pauses.
- **Une coupure désigne sa première photo** (`Cut::first`, un fichier), pas une heure : une
  carte FAT date ses fichiers à 2 s près, les photos d'une rafale ont souvent la même.
  `Plan::shots` donne les photos dans l'ordre de prise de vue (une paire RAW+JPEG est une
  photo, jamais coupée) ; `Plan::sessions_at(&coupures, day_cut)` coupe la carte.
- **Noms de dossier** : le jour (`AAAA-MM-JJ_`), et l'heure de début quand d'autres
  séances de la carte sont du même jour (`2026-11-25_0900_`, `2026-11-25_1300_`) ; deux
  séances qui commencent à la même minute : `-2`, `-3`… (jamais deux séances dans le même
  dossier).
- **Grouper la sélection** (idée de l'utilisateur : les portraits de deux personnes pris
  en alternance, un dossier chacune) : les photos cochées forment une séance à part, même
  si elles ne se suivent pas ; puis toutes repartent cochées (les coches ont servi à
  choisir). Une séance devient un ensemble de photos (`Plan::sessions_of` : les fichiers
  de chaque séance), montrée à l'heure de sa première photo ; ciseaux et colle marchent
  dans chacune (recoudre joint une séance à celle d'avant dans la galerie).
- **Galerie de toute la carte** (« Séances… » dans la boîte, à côté du dossier ou des
  lignes de séances) : les séances l'une après l'autre, chacune sur une nouvelle ligne sous
  un bandeau (numéro, jour, heures, photos cochées, pourquoi elle commence là : nouveau
  jour, pause, coupure choisie). Au clavier, `c` (action `core.offload.cut`, réglable dans `[keys]` comme toutes les
  touches de la galerie, qui passent par les actions de la grille) commence une séance à
  la photo pointée,
  ou la recoud à la précédente. À la souris, comme sur une table de montage (idée de
  l'utilisateur) : une barre d'outils toujours visible en haut, **ciseaux** (un clic entre
  deux photos coupe, aussi après la dernière photo d'une ligne ; la coupure visée s'allume) et **colle** (un clic sur le bandeau
  d'une séance la recoud à la précédente) ; le pointeur devient l'outil, Alt ou Ctrl (Cmd sur macOS) donne
  l'autre (sous Linux Mint, Alt+clic déplace la fenêtre), Échap revient à la sélection. Le clic droit sur une photo coupe ou recoud
  aussi. Coches, gestes et vue en grand comme
  dans la galerie d'une séance, qui reste sur chaque ligne. Valider refait les lignes de
  la boîte : une séance est cochée si l'une de ses photos l'est, avec son choix de
  photos ; un dossier tapé à la main est gardé si sa séance commence au même endroit.
  Les coupures ne valent que le temps de la boîte.
- Les vignettes de la galerie sont centrées dans la fenêtre (#601).
- **Images de la galerie gardées** (idée de l'utilisateur) : vignettes et grands aperçus
  sont rangés dans le cache du moteur, aux mêmes clés que la grille. La même carte
  rouverte va plus vite, et la copie emporte ces entrées (`preview::carry`, comme une
  copie de `tri`, #592 ; déplacées si la carte est vidée ensuite) : la séance déchargée
  montre tout de suite ses vignettes. `offload::run` donne pour cela (fichier de la
  carte, copie) à chaque copie.

## Oublier une carte (26/11/2026, #602)

« Oublier cette carte » (boîte de déchargement, quand des images sont déjà connues ;
confirmé) et `tri --offload <carte> --forget` : `History::forget` retire de l'historique
les lignes des images de la carte (fichier réécrit d'un bloc, les autres lignes gardées
telles quelles). Les images redeviennent nouvelles ; les copies déjà faites ne sont pas
touchées. Usage : réutiliser une carte de test.

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
- `suggest_folder(&plan, &réglages)`, `destination(racine, nom)` ;
- `run(&plan, destination, &réglages, &mut historique, &annulation, progression,
  copie) -> Report` (`copie` : chaque copie dès qu'elle est faite, pour les aperçus) (copiés, ignorés, erreurs, interrompu, durée) ; `Progress` : fichiers (sans ceux laissés sur la carte) et octets (un fichier passé, déjà déchargé ou déjà présent, compte d'un coup : la barre suit les fichiers), et l'étape en cours (`Phase` : préparation, copie, écriture sur le disque `sync_all`, vérification ; #470, pour distinguer une copie lente d'une copie bloquée). La boîte montre l'étape, le fichier, le débit, et « Rien n'a bougé depuis n s » au-delà de 3 s sans avancement ;
- `Plan::shots`, `Plan::proposed_cuts`, `Plan::sessions`, `Plan::sessions_at`, `Plan::sessions_of` (`Cut`, `CutReason`, `Session`), `History::forget` ;
- `delete_sources(&report)`.

## Ligne de commande

```
tri --offload <carte> [--to <racine>] [--folder <nom>] [--raw-only] [--dry-run] [--previews] [--delete-copied] [--eject] [--session <nom>] [--forget]
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
