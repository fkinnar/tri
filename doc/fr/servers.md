# Module `tri::servers`

Rejoindre un partage SMB par son adresse, comme « Se connecter au serveur » du Finder
(demandé le 28/10/2026 : sous Windows, dans WinBoat, les photos de l'utilisateur sont sur
un partage `\\truc\machin` qu'aucun moyen ne permettait d'atteindre). Avant, `tri` ne
voyait que les partages déjà montés (section « Réseau » du panneau des dossiers, voir
[places.md](places.md)).

## Adresses (`Server::parse`)

- `\\serveur\partage[\dossiers]` (Windows ; barres obliques acceptées : `\\nas/photo`) ;
- `smb://[utilisateur@]serveur/partage[/dossiers]`, partout (`%20` décodé).

Il faut au moins un serveur et un partage. Les dossiers sous le partage
(`\\nas\photo\2026\séance`) s'ouvrent une fois connecté ; seul le partage est mémorisé.
Deux adresses désignent le même partage si leurs premiers noms de serveur et leurs partages
sont égaux, casse ignorée (`NAS` = `nas.local` = `nas._smb._tcp.local`, nom donné par
Bonjour ; une adresse IP est comparée entière).

## Connexion (`Server::connect`) : le système s'en charge

Pas de client SMB dans `tri` : le système monte, et demande lui-même le mot de passe
(rien à stocker de notre côté).

| Système | Comment | Mot de passe |
| --- | --- | --- |
| Windows | chemin UNC lu directement (`\\nas\photo`) | identifiants de Windows (ceux enregistrés par l'Explorateur) |
| macOS | `osascript -e 'mount volume "smb://…"'`, monté dans `/Volumes` | boîte du système, trousseau |
| Linux | `gio mount smb://…` (GVFS, comme Nemo et Nautilus) | trousseau GNOME s'il est déjà enregistré |

Un partage déjà monté n'est pas remonté : son dossier est retrouvé dans les montages
(`Mounts` : sortie de `mount` sur macOS, `/proc/mounts` pour CIFS et les montages GVFS
sous Linux). Sous Linux, `gio mount` ne peut pas demander de mot de passe sans terminal :
un partage protégé se rejoint une fois depuis le gestionnaire de fichiers (« Se
connecter à un serveur », mot de passe retenu), ensuite `tri` le rejoint seul. La
connexion peut prendre plusieurs secondes : l'interface l'attend dans un fil à part.

## Partages mémorisés

Fichier `servers` du dossier de données (`[data] dir`) : une adresse `smb://` par ligne
(`remembered`, `remember`, `forget`). Un partage rejoint y est ajouté ; il apparaît dans
la section « Réseau » (`Server::place`) :

- sous Windows, son chemin UNC, un dossier comme un autre (`PlaceKind::Network`) ;
- ailleurs, tant qu'il n'est pas monté, une ligne grisée `PlaceKind::Server` (chemin =
  son adresse) qu'un clic rejoint ; monté, il apparaît comme les autres partages montés.

## Dans l'interface

- Icône « serveur » de l'en-tête du panneau des dossiers, ou `c` quand le panneau a le
  clavier (`core.folders.connect`) : le champ de recherche s'ouvre avec `\\` (Windows) ou
  `smb://` ; tant que l'adresse est incomplète, il montre le format attendu.
- Une adresse complète tapée dans la recherche (y compris sans passer par l'icône) :
  première ligne « Se connecter à « photo (nas) » », Entrée ou clic.
- Clic droit sur un partage mémorisé : « Se connecter » (s'il ne l'est pas), « Oublier ce
  partage » (`core.folders.forget`, Maj+C).
- Sous Windows, une lettre de lecteur attribuée à un partage (`Z:`) est rangée dans
  « Réseau ».

## Plus tard

Un vrai voisinage réseau (découverte des serveurs, Bonjour / mDNS, et liste des partages
d'un serveur : `net view`, `smbutil view`, `smbclient -L`) : après la bêta.
