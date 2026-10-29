# Module `tri::watch` : surveillance du dossier ouvert

Demandée pour la V1 (18/10/2026, « un vrai confort ») : des images copiées dans le dossier
ouvert (export, déchargement par un autre outil, prise de vue connectée), retirées ou
modifiées, et des notes posées ailleurs (darktable…) apparaissent d'elles-mêmes, sans F5.

## Scrutation plutôt que notifications

`FolderWatch` relit la liste du dossier (images et sidecars XMP, avec taille et date de
modification ; le reste est ignoré) toutes les `[watch] interval_ms` (1000 ms par
défaut, 0 : pas de surveillance), dans un fil à part. Choix assumé plutôt que le crate
`notify` (FSEvents, inotify…) : aucune dépendance, et le même comportement sur un disque
local, une carte SD ou un partage réseau, où les notifications du système manquent
souvent. Même principe que la surveillance des volumes (`places::Monitor`). Coût : une
lecture du dossier par seconde (quelques millisecondes pour quelques milliers de
fichiers en local). Sur un partage lent, une lecture peut prendre des minutes (GVFS sous
Linux, un aller-retour réseau par fichier) : la suivante attend alors d'autant, pour ne
jamais passer plus de `[watch] max_share` % du temps à relire (10 % : une lecture de
3 s attend 27 s ; `Pace::wait_after`, 29/10/2026).

Dossier ouvert avec ses sous-dossiers (case du panneau, 20/10/2026) :
`FolderWatch::start_depth` / `Snapshot::of_depth` regardent les mêmes niveaux que la
grille, dossiers cachés écartés comme dans `media::scan`.

## Un signal par changement, une fois le dossier stable

Un changement n'est signalé que quand deux lectures successives sont identiques : une
copie de 500 fichiers en cours, ou un fichier dont la taille grandit, ne donne qu'un
signal, à la fin. Le signal (`Changes`) liste les images et les sidecars ajoutés,
retirés ou modifiés depuis le signal précédent.

`FolderWatch::sync()` reprend l'état actuel pour référence sans rien signaler : après un
changement fait par `tri` lui-même (renommage, corbeille, dossier relu), déjà pris en
compte.

## Dans l'interface

- images changées : le dossier est relu, la sélection reste sur la même image ;
  message « dossier relu » dans la barre d'état ;
- sidecars seulement : `NoteBook::reload_sidecars` relit les notes des seules images
  concernées (`IMG.RAF.xmp` → `IMG.RAF`, `IMG.xmp` → `IMG.RAF` et `IMG.JPG`) et renvoie
  celles qui ont vraiment changé : rien quand c'est `tri` qui vient d'écrire. Le filtre
  sur les notes et les couvertures des piles sont alors revus.

La surveillance repart à chaque dossier ouvert, ou si l'intervalle change dans les
réglages. Son état de départ est relevé avec les notes, en fond (`folder::Loading`), puis
donné à `FolderWatch::start_from` : relevé au démarrage de la surveillance
(`start_depth`), c'était un accès disque par fichier dans le fil de l'interface
(issue n° 15).
