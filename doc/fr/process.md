# Module `tri::process` : programmes externes

`tri` lance des programmes externes : outils du système (`diskutil`, `udisksctl`,
`lsblk`, PowerShell pour l'éjection sous Windows, `explorer`, `xdg-open`…), logiciels
de développement (`darktable-cli`, RapidRAW), éditeur externe, `curl` pour les mises à
jour. Tous passent par `process::command(programme)`, qui crée la commande **sans
fenêtre de console sous Windows** (`CREATE_NO_WINDOW`) : une application graphique
qui lance un programme en mode console en ouvrait une (issue n° 159, 26/10/2026 :
l'éjection d'une carte ouvrait une fenêtre PowerShell ; l'éditeur, une fenêtre CMD).
Ailleurs, une commande ordinaire.

## Exécutable « occupé » (`process::spawn`)

Les logiciels de développement sont lancés par `process::spawn(&mut commande)`. Sous
Linux, un exécutable qu'on vient d'écrire peut être refusé un instant avec « Text file
busy » (`ETXTBSY`) : un processus créé au même moment ailleurs dans `tri` a hérité du
fichier encore ouvert en écriture, jusqu'à son `exec`. `spawn` réessaie alors jusqu'à
5 fois, en attendant 5 ms, puis 10, 20, 40 et 80 ms (155 ms en tout) ; toute autre erreur
revient tout de suite. Vu sur les faux outils des tests (CI Ubuntu, 11/11/2026, test
`rawtherapee_and_art_export_into_a_folder_then_tri_shrinks`) ; possible aussi pour un
outil tout juste installé ou extrait d'une AppImage. Test :
`a_busy_executable_is_launched_once_freed`.
