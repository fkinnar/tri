# Module `tri::process` : programmes externes

`tri` lance des programmes externes : outils du système (`diskutil`, `udisksctl`,
`lsblk`, PowerShell pour l'éjection sous Windows, `explorer`, `xdg-open`…), logiciels
de développement (`darktable-cli`, RapidRAW), éditeur externe, `curl` pour les mises à
jour. Tous passent par `process::command(programme)`, qui crée la commande **sans
fenêtre de console sous Windows** (`CREATE_NO_WINDOW`) : une application graphique
qui lance un programme en mode console en ouvrait une (issue n° 159, 26/10/2026 :
l'éjection d'une carte ouvrait une fenêtre PowerShell ; l'éditeur, une fenêtre CMD).
Ailleurs, une commande ordinaire.
