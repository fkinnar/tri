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

## Protocoles (`Protocol`, 29/10/2026)

Demandé par l'utilisateur : « les protocoles les plus classiques, y compris le FTP ».
L'adresse donne le protocole ; ce qui suit le serveur dépend de lui (`Server::share`, ce
qu'on monte) :

| Adresse | Ce qu'on monte | Linux | macOS | Windows |
| --- | --- | --- | --- | --- |
| `smb://`, `\\` | le partage (1er dossier) | ✅ GVFS | ✅ | ✅ UNC |
| `afp://` | le partage | ✅ | ✅ | ❌ |
| `ftp://` | tout le serveur | ✅ | ✅ (lecture seule) | ❌ |
| `sftp://` | tout le serveur | ✅ | ❌ | ❌ |
| `dav://`, `davs://` (`webdav(s)://`) | tout le chemin | ✅ | ✅ (`http(s)://`) | ✅ UNC `\\nas@SSL\…` (service WebClient) |
| `nfs://` | tout le chemin (l'export) | ✅ | ✅ | ❌ |

`[utilisateur@]` et `:port` partout (`ftp://moi@nas:2121/`). Les cas ❌ donnent un message
clair (`Protocol::mountable`), sauf FTP et SFTP : **copie en local**, confirmée d'abord
(nombre d'images et taille annoncés, voir [remote.md](remote.md)). Un montage est reconnu s'il couvre le début du
chemin voulu (`sftp://nas/home/moi/photo` dans un montage de `sftp://nas/`), en SMB comme
en AFP pour un même partage, en WebDAV chiffré ou pas.

## Connexion (`Server::connect`) : le système monte, `tri` demande le compte

Pas de client SMB dans `tri` : le système monte. Quand le serveur demande un compte que
le système ne demande pas lui-même, `connect` renvoie `ConnectError::NeedsLogin` et
l'interface ouvre sa **boîte de connexion** (29/10/2026) : utilisateur, domaine (SMB),
mot de passe, « Retenir le mot de passe » ; puis `connect` est rappelé avec le `Login`.

| Système | Comment | Compte |
| --- | --- | --- |
| Windows | chemin UNC lu directement (`\\nas\photo`) ; accès refusé (codes 5, 86, 1326, 1244) : la boîte, puis `WNetAddConnection2` | identifiants de Windows ; « Retenir » : gestionnaire d'identifiants (`CredWrite`, comme `cmdkey /add`) |
| macOS | `osascript -e 'mount volume "smb://…"'`, monté dans `/Volumes` | boîte du système, trousseau (pas de boîte `tri`) |
| Linux | `gio mount smb://…` (GVFS, comme Nemo et Nautilus), questions répondues par `tri` | trousseau GNOME s'il est déjà enregistré ; sinon la boîte ; « Retenir » : `secret-tool` (trousseau du bureau, attributs lus par GVFS ; sans garantie) |

Sous Linux, `gio mount` pose ses questions sur sa sortie (`User [fabrice]: `,
`Domain [WORKGROUP]: `, `Password: `, `LC_ALL=C`) ; `tri` les lit au fil de l'eau et
répond avec le compte saisi. Sans compte, la première question ouvre la boîte ; une
question après le mot de passe est un refus : la boîte revient avec « utilisateur ou mot
de passe refusé ». Une question à choix (clé SFTP inconnue…) arrête avec son texte : se
connecter une fois depuis le gestionnaire de fichiers. Le mot de passe ne passe jamais
dans une ligne de commande et n'est jamais écrit par `tri` (seulement par le système,
quand « Retenir » est coché).

Les erreurs sont expliquées par cause : serveur injoignable (nom, réseau), partage
inconnu (taper le serveur seul pour voir ses partages), déjà connecté sous un autre
compte (Windows, `net use`), compte refusé.

Un partage déjà monté n'est pas remonté : son dossier est retrouvé dans les montages
(`Mounts` : sortie de `mount` sur macOS — `smbfs`, `afpfs`, `ftp`, `webdav`, `nfs` —,
`/proc/mounts` pour CIFS, NFS, sshfs et davfs, et les montages GVFS sous Linux), **en SMB comme en AFP** (retour du 29/10/2026 : un NAS qui annonce les deux
est souvent monté en AFP par Nemo, `afp-volume:host=…,volume=…`, et n'était pas
reconnu). La connexion peut prendre plusieurs secondes : l'interface l'attend dans un
fil à part.

## Partages d'un serveur (`Host`)

Une adresse sans partage, suivie d'une barre (`smb://nas.local/`, `\\nas\`) : `tri`
demande au système les partages de dossiers du serveur (`Host::shares`, partages cachés
`…$` écartés), comme « Partages Windows sur nas » de Nemo (retour du 29/10/2026 :
`smb://nas.local/` suivi d'Entrée ne faisait rien). Pas avant la barre : le nom est
sans doute encore en cours de frappe ; Entrée sur un serveur seul (`\\host.lan`,
retour du même jour sous WinBoat) ajoute la barre et lance la liste, ce qu'indique la
ligne « Entrée : partages de host.lan » sous le champ.

| Système | Comment |
| --- | --- |
| Windows | `NetShareEnum` (pas de sortie de `net view` à lire, traduite selon la langue) |
| macOS | `smbutil view -N` (identifiants du trousseau, sans rien demander), sinon `-g` (invité) |
| Linux | `gio mount smb://nas/` (voisinage anonyme) puis `gio list smb://nas/` |

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
- Un serveur seul (`smb://nas.local/`) : une ligne « Se connecter » par partage (liste
  demandée en fond, « recherche des partages… » en attendant) ; taper le début d'un nom
  (`smb://nas.local/kp`) resserre la liste. ↑ / ↓ et Entrée, ou un clic.
- Clic droit sur un partage mémorisé : « Se connecter » (s'il ne l'est pas), « Oublier ce
  partage » (`core.folders.forget`, Maj+C).
- Sous Windows, une lettre de lecteur attribuée à un partage (`Z:`) est rangée dans
  « Réseau ».

## Plus tard

Voisinage réseau (Bonjour / mDNS) : fait, voir [neighbourhood.md](neighbourhood.md). La
boîte de connexion n'a pas encore été essayée sur de vrais serveurs (Windows, Mint) ;
« Retenir » sous Linux dépend du trousseau du bureau.
