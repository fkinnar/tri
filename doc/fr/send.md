# Module `tri::send` : plugins d'envoi

Envoyer des images quelque part : AirDrop d'abord (macOS, l'usage de l'utilisateur), puis
d'autres destinations au besoin (dossier synchronisé, KDE Connect…). Prévu « après la
V1 », avancé au 19/10/2026 pour éprouver l'architecture des plugins.

## Le trait et la liste

`Sender` : `id` (nom court), `name` (affiché), `available` (sur cette machine), `send`
(des fichiers). `senders()` les liste, `sender(id)` en trouve un. Chaque plugin est
écrit en Rust et compilé avec `tri` : traits statiques et liste, comme décidé.

Son action dans l'interface est dans l'**espace de noms des plugins** :
`plugin.<id>.send` (`plugin.airdrop.send`). Elle n'a pas de touche par défaut : on la
règle dans `[keys]` ou la fenêtre de réglages. Elle a un bouton (icône de partage) dans
la boîte du clic droit, et apparaît dans l'aide, seulement là où le plugin est
disponible. Elle porte sur la sélection (ou l'image courante).

## AirDrop

La feuille de partage de macOS (`NSSharingService`, service « Envoyer par AirDrop »)
appelée depuis Rust avec `objc2-app-kit` : le système montre les appareils à portée,
l'utilisateur choisit, l'envoi se fait en fond. Interface AppKit : **fil principal
obligatoire** ; ailleurs, `send` refuse proprement (vérifié par un test, les tests
tournant dans des fils à part). Vérifié à la main le 19/10/2026 : la feuille s'ouvre,
macOS accepte les fichiers. Hors macOS : indisponible.

Les fichiers sont envoyés tels quels (le RAW pour un RAW). Plus tard, avec les profils
de sortie de l'« après-développement » : envoyer une version (JPEG redimensionné…).
