# Index de recherche des palettes (`tri::index`)

Étape 1 de l'issue #561 : chercher des palettes dans **tous les dossiers**, pas seulement
celui qui est ouvert. Les étapes suivantes (grille des résultats, « Retrouver… » pour un
dossier déplacé, racines surveillées) s'appuieront sur ce module.

## Principe : un cache de recherche, pas une bibliothèque

L'utilisateur ne veut pas de « bibliothèque » visible de l'extérieur : un catalogue à la
Lightroom casse dès qu'on déplace un dossier hors de l'application (dans darktable, d'un
disque interne vers un disque de stockage…). D'où :

- **Seulement des données recalculables** : les palettes. Les notes, versions, etc. restent
  dans les sidecars XMP. L'index peut être effacé à tout moment : les recherches sont
  partielles jusqu'aux prochains calculs. Rien n'est écrit à côté des photos.
- **Identité par le contenu, pas par le chemin.** Chaque photo a une **empreinte**
  (`fingerprint`) : sa taille et un hachage BLAKE3 (tronqué à 16 octets) de ses
  `fingerprint_kb` premiers et derniers Ko (32 par défaut). Une palette ne dépend que du
  contenu et de ses paramètres (`preview::stamp` : réglages des palettes et, pour le rendu
  d'un logiciel, l'état du développement dans le sidecar). Une photo déplacée garde donc sa
  palette ; son nouveau chemin est simplement noté quand `tri` la revoit.
- **Rien n'est suivi** (déplacements, copies, suppressions), dans `tri` ou ailleurs :
  - déplacée : l'ancien chemin ne mène plus à rien, ce qui se verra à l'affichage d'un
    résultat ;
  - copiée : deux chemins, une empreinte, donc deux résultats ;
  - modifiée : une autre empreinte ; l'ancienne entrée devient orpheline.

## La base

SQLite (`rusqlite`, SQLite compilée avec le projet), fichier `index.sqlite` du **dossier
des données** de `tri` (`[data] dir`), pas du cache, qu'on vide. Journal WAL : on lit
pendant qu'on écrit. Deux tables :

| Table | Clé | Contenu |
|---|---|---|
| `paths` | chemin (octets du système ; UTF-16 sous Windows) | taille, date de modification (ns, celle des clés du cache), empreinte (vide tant qu'aucune palette n'a été calculée) |
| `palettes` | (empreinte, source) | tampon des paramètres, palette (format du cache, `Palette::to_bytes`) |

La taille et la date gardées avec chaque chemin permettront de retrouver la vignette d'un
résultat dans le cache **sans relire le fichier** (rapide sur un NAS ou un disque
débranché). Une base d'un autre schéma (`user_version`) ou illisible est refaite.

## Toutes les photos vues (#561, schéma 2)

Pour « Toutes les photos » (une racine virtuelle au-dessus de tous les emplacements, où les
filtres font office de recherche), l'index note **chaque photo vue**, pas seulement celles
qui ont une palette : `index::seen(dossier, niveaux, images)` à l'ouverture d'un dossier
(interface) et au calcul d'un dossier (« Calculer »). Chemin, taille et date seulement :
l'empreinte n'est lue qu'au calcul d'une palette. Les photos de ces niveaux qui n'y sont
plus sont oubliées (supprimées, ou déplacées : revues ailleurs, elles y seront notées) ;
une photo modifiée (autre taille ou date) perd son empreinte. Les développements
(`IMG@dt-01.RAF`) ne sont pas des fichiers : pas notés, ils se retrouvent en relisant le
dossier. `Index::photos(sous)` les liste, sans parcourir de dossier. Le passage au
schéma 2 refait la base une fois : les palettes déjà en cache s'y renotent d'elles-mêmes.

### Ce que « Toutes les photos » connaît d'un dossier (26/11/2026)

Idée de l'utilisateur : rien ne disait quels dossiers avaient été vus. Table `dirs` (chaque
dossier vu par `Index::seen`, et jusqu'à quelle profondeur), ajoutée sans refaire la base
et remplie à sa création à partir des dossiers des photos déjà notées ; elle suit un
dossier déplacé (`Index::relocate`). `Index::seen_dirs` → `SeenDirs::state(dossier,
sous-dossiers connus)` : **vu** (lui et tous ses sous-dossiers connus), **en partie** (vu
sans tous ses sous-dossiers, ou pas vu mais un dossier dessous), **pas vu**.

Dans le panneau des dossiers (relu en fond, au plus toutes les 10 s) : un dossier pas vu
a son **nom en gris**, un dossier en partie un **◐** après son nom (bulles d'aide).
**« Ajouter à Toutes les photos »** au clic droit de ce qui n'est pas tout à fait vu
(action `core.folders.add_all_photos`) : un parcours en fond, une fois, comme une racine
surveillée mais sans la surveiller ensuite.

## Empreintes de similitude (schéma 3)

Les empreintes des piles (`similarity::Signature`, ~70 octets) sont notées avec chaque
chemin (`paths.signature`, et `signature_stamp` : leurs paramètres), dès qu'elles sont
prêtes (`index::note_signature`, planificateur et « Calculer »). Elles suivent le
fichier : un fichier modifié les perd, un dossier réparé les garde. « Toutes les
photos » les charge d'un coup à l'ouverture (`Index::signatures`) : « semblables »
partout sans relire le cache photo par photo (100 000 photos : ~7 Mo). Le passage au
schéma 3 refait la base une fois.

## Remplissage

Toute palette **prête**, calculée ou trouvée dans le cache, est signalée par
`index::note` :
- par le planificateur (`scheduler::finish`), donc à l'ouverture d'un dossier, que ses
  palettes soient calculées ou déjà en cache ;
- par les calculs choisis (`compute::run`, clic droit › Calculer, `tri --compute`).

Un **fil à part** (`index::start`, `tri-index`) écrit par lots (au plus `batch`, une
transaction) : le calcul ne l'attend jamais. Un chemin déjà connu à la même taille et la
même date, avec la palette aux mêmes paramètres, ne coûte qu'une lecture de la base ;
sinon l'empreinte est calculée (au plus 2 × 32 Ko lus) et la palette relue dans le cache.
`index::flush` attend que tout soit écrit (ligne de commande, avant de quitter).
L'interface démarre l'index avec le moteur (jamais dans les tests) ; `--compute` aussi,
quand il calcule des palettes. Le dossier ouvert au lancement, lu avant le fil
d'écriture, est noté quand le moteur démarre.

**Cartes d'appareil jamais indexées** (retour du 22/11/2026 : des fichiers bruts, avant
leur copie ailleurs) : rien sous un dossier `DCIM` (`locate::is_camera_card`).

## Retrouver un dossier déplacé (`tri::locate`)

Dans « Toutes les photos », la complétion depuis le disque (`folder::complete_from_disk`)
rend aussi les dossiers **injoignables** ; leurs photos restent, **grisées**, avec la
raison au survol : **disque débranché** (le premier parent qui existe est un dossier de
montage, `/Volumes`, `/media/<utilisateur>`… : `locate::unplugged`, il suffit de le
rebrancher) ou **introuvable** (déplacé ou renommé sur un disque présent).

**Retrouver son dossier** (`core.file.locate`, `g l`, clic droit, ou l'icône dossier
et loupe **au centre de chaque vignette grise**, retour du 22/11/2026 : à la souris
aussi ; un disque débranché y montre un disque, sans clic) cherche en fond (`locate::find`) sous les disques, le dossier personnel et les
favoris :
1. l'ancien chemin posé sur chaque racine, de sa fin vers son début
   (`…/2026-11-18_Sarah`, `…/2026/2026-11-18_Sarah`…) : un dossier déplacé d'un disque
   à l'autre garde souvent sa structure, et ces essais ne coûtent rien ;
2. puis les dossiers du même nom, à la profondeur des sous-dossiers de la grille
   (`[gui.folders] subfolders_depth`, retour du 22/11/2026 : le même réglage), dossiers
   cachés et cartes écartés.

Chaque candidat est **vérifié** dès qu'il se présente (`locate::verify`) : trois photos
témoins de l'ancien dossier (première, milieu, dernière par nom, `Index::photos_in`)
doivent y être, à la même taille, et au même contenu quand l'index en a l'empreinte. Pas
trouvé : une boîte demande où il est (complétion par Tab, « Parcourir… »), vérifié de
même (« Ce dossier n'a pas les mêmes photos »). Retrouvé, **tout suit**
(`locate::relocate`) : l'index (`Index::relocate`, sous-dossiers compris, empreintes et
palettes gardées), le cache (`Cache::relocate`) et les données de tri gardées sur la
machine (`store::relocate`) ; « Toutes les photos » est relue. Un dossier déplacé ou
renommé **dans `tri`** emmène aussi son index.

**Photos déplacées, renommées ou copiées par `tri`** (`files::rename`, `files::transfer`,
comme les notes, `reconcile::carry`) : l'index les suit (`index::carry`,
`Index::carry`), même vers un dossier jamais ouvert. Déplacée, une photo garde tout ;
copiée, la copie prend l'empreinte de contenu de l'original (ses palettes suivent).
Une photo déplacée **hors de `tri`**, seule, quitte « Toutes les photos » : son dossier
est là, relu, et elle n'y est plus ; les racines surveillées la retrouveront à sa
nouvelle place. Un dossier **qui n'existe plus** n'est jamais noté « vu vide » (rouvert
au lancement après un renommage, il effaçait tout ce que l'index en savait) : ses photos
restent, grisées.

**Notes gardées** (schéma 4, retour du 22/11/2026 : « sinon on ne voit pas qu'elles ne
sont plus là ») : étoiles, drapeau et couleurs de chaque photo joignable, en un entier
(`paths.notes`), signalés quand ils sont lus ou changés (`index::note_notes`, depuis
`TriApp::notes_changed`). Une photo injoignable montre ces notes (`NoteBook::set_known`,
`Index::notes_in`) : les filtres la trouvent, grisée. Passer d'un schéma 2 ou 3 au 4
ajoute les colonnes (`ALTER TABLE`) : rien n'est perdu.

**Dossier reconnu à l'ouverture** (`locate::recognise`, idée du 22/11/2026) : ouvrir un
dossier que l'index ne connaît pas (`TEST4`) dont les photos témoins (même nom, même
taille, `Index::with_sizes`, puis `locate::verify`) sont celles d'un dossier connu
**qui n'existe plus** (`TEST3`) : c'est lui, renommé ou déplacé hors de `tri`. Tout suit
(`locate::relocate`) avant que sa vue ne soit lue, dans le fil d'ouverture : vue retenue,
notes gardées par `tri`, calculs du cache, index ; un message le dit. Un ancien dossier
encore là n'est pas pris (une copie n'est pas un renommage).

## Racines surveillées (`tri::roots`, 22/11/2026)

Des dossiers que `tri` parcourt de lui-même, en fond (`roots::walk`, fil `tri-roots`),
pour que « Toutes les photos » sache ce qu'ils contiennent sans qu'on les ouvre, et pour
réparer l'index quand des photos ont bougé hors de `tri`. Choisis par `[index] roots`
ou au clic droit d'un dossier, **Surveiller ce dossier** (`core.folders.watch`, `w`
dans le panneau des dossiers), qui réécrit la configuration ; un œil le marque dans
l'arborescence.

**Quand** (choix de l'utilisateur, 22/11/2026 : « en prenant soin de rien gêner ») : au
lancement (`walk_at_launch`), après `walk_idle_minutes` sans rien faire, et pour un
disque amovible qu'on branche (`walk_plugged`), **jamais une carte d'appareil** (un
`DCIM` : des fichiers bruts, avant leur copie ailleurs). Une pause entre deux dossiers
(`walk_pause_ms`) ; une ligne dans la barre d'état, interruptible.

**Ce qu'il fait**, dossier par dossier (`walk_depth` niveaux, dossiers cachés et cartes
écartés) :
- un dossier inconnu : peut-être un dossier connu renommé ou déplacé
  (`locate::recognise`) ;
- une photo inconnue : peut-être une photo **déplacée seule** (même nom, même taille,
  même contenu si l'empreinte est connue, ancien chemin disparu) : l'index la suit
  (`Index::carry`) avec ses notes, palettes et empreintes, la mémoire des notes de `tri`
  aussi (`reconcile::carry`). C'est ainsi qu'est retrouvée une photo déplacée seule hors
  de `tri` (choix de l'utilisateur : pas de vignette grise pour elle) ;
- les photos vues sont notées ; celles qui ont disparu ne sont oubliées **qu'à la fin**
  du parcours (une photo partie d'un dossier parcouru avant celui où elle est arrivée
  doit encore être connue), et jamais si le parcours est interrompu.

## Réglages `[index]`

| Clé | Défaut | Rôle |
|---|---|---|
| `enabled` | `true` | noter les palettes dans l'index |
| `fingerprint_kb` | 32 | Ko lus au début et à la fin d'un fichier pour son empreinte |
| `batch` | 256 | palettes notées par écriture, au plus |

## Mesures (22/11/2026)

100 000 palettes synthétiques (6 couleurs, couverture de 96 × 96 points bruités), Mac
Apple Silicon, release (`cargo test --release --test index -- --ignored --nocapture`) :

| | |
|---|---|
| palette | 2,4 Ko en moyenne (les vraies : ~1,2 Ko, mesuré sur 131 photos, #562) |
| base | 429 Mo |
| écriture, lots de 256 | 48 s (~0,5 ms par palette, en fond) |
| lecture de tout | 0,34 s |
| « couleurs proches » sur tout | 96 ms |
| « contient cette couleur » sur tout | 68 ms |

La recherche en mémoire suffit (moins de 0,1 s, plus le chargement, une fois). La taille
de la base est le point à surveiller : couverture compressée ou moins fine dans l'index, à
décider sur de vraies photos.

## Tests

- `src/index.rs` : photo déplacée (un chemin de plus, même palette), palette recalculée à
  d'autres réglages (remplacée), empreinte (seulement les bouts du fichier), base d'un
  autre schéma refaite, chemins non ASCII.
- `tests/index.rs` : de bout en bout, calcul réel de palettes, index rempli par le fil
  d'écriture, rien de neuf au second calcul, photo déplacée retrouvée ; la mesure
  ci-dessus (`#[ignore]`).
