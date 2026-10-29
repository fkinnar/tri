# Module `tri::remote`

Copie en local d'un dossier d'un serveur que le système ne sait pas monter : FTP et SFTP
sous Windows, SFTP sous macOS (voir `Protocol::mountable` dans [servers.md](servers.md)).
Choix de l'utilisateur (29/10/2026) : copier en local, mais annoncer d'abord le nombre
d'images et la taille, « pour éventuellement changer d'avis avant de télécharger 60 To de
données ».

## Sans dépendance

Les outils livrés avec Windows 10+ et macOS :

| Protocole | Outil | Liste d'un dossier | Copie |
| --- | --- | --- | --- |
| FTP | `curl` | `curl ftp://serveur/dossier/` (format `ls -l` ou DOS d'IIS) | un `curl -o` par fichier |
| SFTP | `sftp` (OpenSSH) | `ls -l` en mode lot (`-b -`) | des `get` par lots (`[remote] batch`, une connexion chacun) |

`sftp` tourne avec `BatchMode=yes` : les clés SSH de l'utilisateur (agent compris), rien
n'est demandé. `curl` reçoit l'utilisateur de l'adresse, mot de passe vide. La boîte de
connexion (mot de passe) est l'étape suivante.

## Déroulé (`scan`, `download`)

1. `scan` parcourt le dossier et ses sous-dossiers (jusqu'à `[remote] depth`, dossiers
   cachés `.…` écartés) et ne garde que les **images et sidecars XMP** (`media::kind_of`,
   `.xmp`) : `Listing` (fichiers, `images()`, `bytes()`). Annulable entre deux dossiers.
2. L'interface annonce « 1 234 images, 38,2 Go à copier », la destination modifiable
   (`default_destination` : `[remote] destination/serveur/dossier`, `~/Pictures` par
   défaut, complétion par Tab), et que les notes posées ensuite restent sur la copie.
   Annuler : rien n'est téléchargé.
3. `download` copie en gardant les sous-dossiers ; un fichier déjà là, à la même taille,
   est sauté (reprise d'une copie interrompue). Progression (fichiers, octets) après
   chaque fichier ou lot ; « En arrière-plan » referme la boîte, la progression passe dans
   la barre d'état ; annulable. À la fin, le dossier copié s'ouvre.

## Réglages `[remote]`

| Clé | Défaut | Rôle |
| --- | --- | --- |
| `destination` | `"~/Pictures"` | dossier des copies, sous `serveur/dossier` |
| `depth` | `8` | profondeur des sous-dossiers parcourus |
| `batch` | `20` | fichiers par lancement de `sftp` |

## Tests

Lecture des listes (Unix, DOS, liens et `.`/`..` écartés), filtre images / sidecars,
destination et URL (encodage, port), fichier déjà copié sauté sans réseau ; interface :
boîte de confirmation avec un faux serveur, annulation sans rien télécharger. Jamais
essayé contre un vrai serveur FTP / SFTP à ce jour.
