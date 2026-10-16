# Module `tri::scheduler`

File de travail en tâche de fond qui remplit le cache pour un dossier.

- **Workers** : un seul worker *lourd* pour les rendus LibRaw (aperçus standard et 1:1 des
  RAW), plus `Config::light_workers` workers *légers* (par défaut la moitié des cœurs) pour
  les vignettes et les images plates.
- **Niveaux de fond par type de fichier** : vignette, **aperçu boîtier** (JPEG embarqué,
  RAW seulement) et aperçu standard ; le 1:1 n'est calculé qu'à la demande. L'aperçu
  boîtier est léger (worker léger), l'aperçu standard d'un RAW lourd (LibRaw).
- **Priorités**, recalculées à chaque prise de tâche (`priority`) : 1:1 demandé > aperçus
  boîtier de l'image affichée, de n+1, de n-1 (presque instantanés) > leurs aperçus standard
  > vignettes dans l'ordre du dossier > autres aperçus boîtier > autres aperçus standard,
  par distance (en avant d'abord).
- **API** : `set_folder(files) -> génération`, `set_focus(index)`,
  `request(index, level)`, `set_paused(bool)`, `progress(level)`, `wait_idle(timeout)`.
  `Drop` annule et arrête les threads.
- **Demandes hors travail de fond** (`request`) : 1:1 (`Full`, `EmbeddedFull`) et rendus
  RAW (`Standard`) de l'image affichée **et de ses voisines** (préchargement quand on
  navigue zoomé). Elles ne comptent pas dans la progression (`on_demand`). Priorité : les
  1:1 d'abord (image affichée, puis suivante, puis précédente). Changer d'image abandonne
  celles qui ne sont plus voisines, annule un 1:1 en cours pour une image éloignée, et le
  **signale par `Event::Dropped`** : l'interface sait ainsi qu'elle doit redemander si l'on
  revient. (Sans ce signal, un 1:1 abandonné n'était jamais redemandé : bug corrigé le
  16/10/2026.) Une demande déjà en cache donne simplement un `Ready`.
- **`Config::raw_previews`** : `Background` (défaut : rendus RAW de tout le dossier en
  fond, après vignettes et aperçus boîtier) ou `OnDemand` (aucun rendu RAW en fond ; utilisé
  par l'interface). Les aperçus des images plates sont toujours calculés en fond.
- **Événements** : `Ready`, `Failed`, `Progress`, tous avec la génération du dossier.
  Garanties du callback :
  - appelé **hors du verrou** d'état : il peut rappeler le scheduler ;
  - **jamais deux fois en même temps**, et **dans l'ordre** où les événements se sont
    produits. Les workers déposent leurs événements dans une file commune (`outbox`), sous
    le verrou ; un seul thread à la fois la vide (`deliver`). Sans cela, deux workers
    livrant en parallèle pouvaient faire arriver « 6/6 » avant « 5/6 » : la progression
    reculait (bug trouvé par les tests, voir plus bas) ;
  - `set_folder` purge les événements non livrés de l'ancien dossier. Seul un événement en
    cours de livraison peut encore arriver, et l'interface ignore ceux d'une autre
    génération.
- **`wait_idle`** ne rend la main qu'une fois **tous les événements livrés** : rien en
  attente, rien en cours, file d'événements vide, aucun callback en cours. Un callback qui
  panique ne bloque pas `wait_idle` (garde `Drop`).
- **Annulation** : un nouveau dossier annule tout ; changer d'image annule un 1:1 en cours
  pour une autre image (effectif seulement entre deux étapes LibRaw, voir `raw.md`).
- **`Producer`** : trait du calcul ; `CachedPreviews { cache, sizes }` est la vraie
  implémentation, et un faux producteur permettra de tester l'ordonnancement sans LibRaw.

## Politique « le plus rapide d'abord »

`Config::thumbnails_first` (par défaut `true`) : les aperçus boîtier *de fond* attendent la
fin des vignettes, et les aperçus standard *de fond* (rendus RAW) attendent la fin des
vignettes **et** des aperçus boîtier. L'image affichée et ses voisines n+1/n-1 passent
toujours. Ainsi la grille, puis tout le dossier en plein écran, sont disponibles au plus vite,
avant de lancer les rendus LibRaw qui occupent tous les cœurs. Chaque tâche terminée réveille les workers, pour qu'ils reprennent les aperçus
dès que la dernière vignette est faite.

## Mesures

Dossier réel : 472 images (236 RAF X-T4 + 236 JPEG du boîtier), Mac Apple Silicon à
8 cœurs, build release, cache vide. Commande :
`tri <dossier> --previews --cache <dossier vide> [--workers N] [--mixed]`.

| Politique | Workers légers | Vignettes (grille complète) | Total |
|---|---|---|---|
| mélangé, avant optimisation de `fit` | 4 | 91 s | 210 s |
| mélangé | 2 | 59 s | 206 s |
| mélangé | 4 | 39 s | **173 s** |
| mélangé | 8 | 36 s | 237 s |
| vignettes d'abord | 4 | 29 s | 227 s |
| vignettes d'abord | 8 | **19 s** | 252 s |

Après passage à libjpeg-turbo (feature `turbojpeg`), plusieurs mesures par configuration
pour estimer la variance (machine plus ou moins chaude) :

| Politique | Workers légers | Vignettes | Total |
|---|---|---|---|
| vignettes d'abord (défaut) | 4 | **14–19 s** | **179–183 s** |
| vignettes d'abord | 8 | 12 s | 190 s |
| mélangé | 4 | 27–38 s | 187–209 s |

Avec un décodage plus rapide, « vignettes d'abord » gagne sur les deux plans : c'est le
défaut, l'autre politique reste disponible (`Config::thumbnails_first`, `--mixed`). Le
total est dominé par les 236 aperçus RAW du worker lourd (~0,5 s chacun).

Second passage (tout en cache) : ~10 ms.

### Pourquoi l'aperçu boîtier (16/10/2026)

Les rendus RAW ne peuvent pas aller plus vite : la décompression d'un RAF X-T4 occupe déjà
tous les cœurs (LibRaw la parallélise), et rien ne se gagne ailleurs :

| Essai | Résultat |
|---|---|
| aperçus à 1280 px au lieu de 2560 | 139 s au lieu de 176 s (−21 %), cache 4× plus léger |
| 1 / 2 / 4 / 8 rendus `half_size` en parallèle | 600 / 573 / 1 046 / 1 440 ms par RAF |

D'où l'aperçu boîtier : le JPEG embarqué (4416×2944 sur X-T4) réduit à la taille standard.
Mesuré sur le dossier de test : **vignettes à 14 s, tout le dossier consultable en plein
écran à 35 s**, rendus RAW terminés à 180 s (total inchangé). Coût : un niveau de cache de
plus (197 Mo pour 236 RAF à 2560 px). Piste : ~0,25 s par aperçu boîtier, surtout la
réduction Lanczos3 de 4416 à 2560 px ; un filtre plus simple serait à mesurer. Taille du cache : 13 Mo de vignettes (28 Kio en
moyenne), 397 Mo d'aperçus standard à 2560 px (861 Kio en moyenne).

Lecture :

- **la concurrence avec LibRaw domine**. Une vignette coûte 140 à 250 ms seule, mais plus de
  300 ms par worker pendant qu'un rendu LibRaw occupe tous les cœurs. D'où le gain de la
  politique « vignettes d'abord » sur la grille ;
- elle allonge en revanche le total : le worker lourd ne fait rien pendant la phase des
  vignettes, et on perd le recouvrement entre les deux phases ;
- plus de workers légers n'aide que si LibRaw ne tourne pas en même temps.

## Tests

Tests unitaires : priorités, routage lourd/léger, fraction de progression.

Tests d'intégration (`tests/scheduler.rs`), avec un **faux producteur** qui enregistre les
tâches, simule durées, échecs, entrées déjà en cache, et mesure la concurrence par type de
worker :

- chaque fichier traité une fois par niveau, progression complète, pas de 1:1 sans demande ;
- événements dans l'ordre, progression strictement croissante (35 fichiers, 4 workers) ;
- entrées en cache signalées sans recalcul ; échecs signalés et comptés ;
- ordre exact du worker lourd autour de l'image affichée (5, 6, 4, 7, 3, 8, 2, 9, 1, 0) ;
- rendus RAW jamais simultanés, tâches légères en parallèle ;
- 1:1 à la demande, annulé quand l'image affichée s'éloigne (< 2 s), signalé par
  `Dropped`, puis abouti pour la nouvelle ; 1:1 des voisines préchargés et gardés d'un pas
  à l'autre ; chaque abandon de demande signalé ;
- nouveau dossier : l'ancien est abandonné, aucun de ses résultats sous la nouvelle
  génération ;
- « vignettes d'abord » : avant la dernière vignette, seuls l'image affichée et ses voisines
  ont un aperçu ; tout finit par être calculé ;
- aperçus boîtier pour les RAW seulement (progression = nombre de RAW) ; rendus RAW de fond
  jamais avant la fin des aperçus boîtier ; l'aperçu boîtier de l'image affichée passe avant
  toutes les vignettes ;
- pause / reprise ; image affichée hors limites ignorée ; arrêt rapide à la destruction ;
- avec `TRI_TEST_RAW` : vrai producteur sur un RAW et un JPEG, tailles et orientation des
  entrées, second passage servi par le cache (0,66 s puis ~40 µs).

Ces tests ont révélé deux courses, corrigées : `wait_idle` pouvait rendre la main avant la
livraison des derniers événements, et des événements de progression pouvaient arriver dans
le désordre. Pour s'assurer qu'aucune ne revient, le fichier de tests a été relancé
300 à 500 fois de suite, sans échec.
