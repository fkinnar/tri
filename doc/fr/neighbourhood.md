# Module `tri::neighbourhood` — voisinage réseau

Les serveurs qui s'annoncent sur le réseau local (Bonjour / mDNS, comme dans le Finder,
Nemo ou l'Explorateur) apparaissent dans la section « Réseau » du panneau des dossiers,
sans avoir à taper leur adresse (demandé le 29/10/2026 pour la bêta).

## Écoute (`Browser`)

`Browser::start(notify)` écoute en fond, par le crate `mdns-sd` (pur Rust, sans runtime
asynchrone), ces types de services, dans l'ordre de préférence pour rejoindre un serveur
qui en annonce plusieurs : `_smb._tcp`, `_afpovertcp._tcp`, `_sftp-ssh._tcp`,
`_webdavs._tcp`, `_webdav._tcp`, `_ftp._tcp`, `_nfs._tcp`. Chaque annonce ou retrait
devient un `Event` (`Found(Service)`, `Removed(nom complet)`) ; `notify` est appelé à
chaque changement (l'interface se redessine), `Browser::poll` les applique. L'écoute
s'arrête quand le `Browser` est libéré (fermeture de l'application).

L'écoute ouvre une socket multicast (port 5353) : sous Windows, le pare-feu peut demander
une autorisation la première fois. `[neighbourhood] enabled = false` la coupe.

## Serveurs (`Neighbourhood`, `Neighbour`)

`Neighbourhood` garde les services annoncés et les rassemble par serveur (même nom
d'hôte, casse ignorée) : `neighbours()` les donne par nom, chacun avec ses services du
préféré au moins préféré. Le nom affiché est celui de l'annonce préférée (l'annonce SMB
porte souvent le nom choisi pour le NAS, l'annonce SFTP celui de la machine).

`Neighbour::address(os)` : le serveur seul pour SMB et AFP (`smb://nas.local/`,
`\\nas.local\` sous Windows) — ses partages se listent dans la recherche des dossiers
([servers.md](servers.md), « Partages d'un serveur ») — ; sinon le chemin annoncé
(`path` du TXT : WebDAV, NFS, FTP), port compris s'il n'est pas l'habituel.
`Neighbour::place(os)` en fait un emplacement `PlaceKind::Neighbour` (« Photos (SMB,
AFP) »), ajouté par `places::Tree::set_neighbours`.

## Dans l'interface

L'écoute démarre à la première image (jamais dans les tests). Un clic sur un serveur
ouvre son adresse dans la recherche des dossiers : ses partages s'y listent, Entrée
rejoint celui sous le curseur (boîte de connexion si le serveur demande un compte).
Un serveur qui quitte le réseau disparaît de la liste.

## Tests

Unitaires (`src/neighbourhood.rs`) : annonces rassemblées par serveur, protocole préféré
en tête, adresses relues par `servers::Host` / `Server`, retrait. Interface : un faux
voisinage listé sous « Réseau », clic qui liste les partages (faux), retrait. Rien
n'écoute le vrai réseau dans les tests.
