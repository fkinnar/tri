# Module `tri::develop` : rendu par un logiciel de développement

Idée de l'utilisateur (19/10/2026) : voir dans `tri` ses photos **telles qu'on les a
développées** ailleurs, darktable d'abord. Le rendu est **demandé** : `Maj+B` ou le
bouton (diaphragme) du bandeau sous l'image. Ensuite, `b` passe du JPEG boîtier au rendu
RAW, puis au **développé**, et revient au boîtier ; tant que l'image n'a pas été
développée, `b` ne propose que boîtier et RAW et ne lance jamais darktable (retour de
l'utilisateur, 19/10/2026 : en tri, le rendu de base suffit presque toujours, inutile
d'attendre 10 s pour rien).

## Un plugin compilé qui pilote un programme externe

Comme les autres plugins (superpositions, envoi), le code est en Rust et compilé avec
`tri` ; il appelle `darktable-cli` en **processus séparé** : darktable ne peut pas faire
tomber `tri`, et sa licence reste la sienne. RawTherapee (`rawtherapee-cli`, profils
`.pp3`) et ART suivraient le même modèle.

- `find_darktable(réglage)` : `[develop] darktable` = `"auto"` (le `PATH`, puis les
  emplacements habituels : `/Applications/darktable.app/…` sur macOS, `/usr/bin` sous
  Linux, `Program Files` sous Windows), un chemin, ou `""` (jamais). Sous Linux, s'il
  n'est pas trouvé : une **AppImage** de darktable (`*darktable*.appimage` dans
  `~/AppImages`, `~/Applications`, `~/.local/bin`, `~/bin`, les téléchargements, `/opt` ;
  la plus récente), lancée avec `darktable-cli` en premier argument : l'AppImage exécute
  alors ce binaire au lieu de l'interface (documenté par darktable, « invocation per
  OS »). C'est l'installation de l'utilisateur sous Mint (`~/AppImages/darktable.appimage`,
  19/10/2026). Un chemin d'AppImage est aussi accepté dans le réglage. Sinon, le
  **Flatpak** `org.darktable.Darktable` (installation système ou
  utilisateur, `flatpak info`), lancé par `flatpak run --command=darktable-cli
  org.darktable.Darktable` (`Cli` : programme et arguments placés devant ceux de `tri`) ;
  `"flatpak"` le choisit d'office. Constaté le 19/10/2026 sur Linux Mint : le Gestionnaire
  de logiciels installe darktable en Flatpak, sans `darktable-cli` dans le `PATH` (et son
  lanceur exporté ouvre l'interface, pas `darktable-cli`). Le Flatpak doit pouvoir lire
  les images et le cache de `tri` (dossier personnel) : à vérifier pour des images hors
  du dossier personnel.
- `darktable_sidecar(image)` : `IMAGE.RAF.xmp`, **seulement s'il porte un vrai
  développement** (`darktable:history_end` > 0) : un sidecar écrit par `tri` seul (notes)
  n'en a pas.
- `Darktable::render` : `darktable-cli image sidecar sortie --width --height --hq
  --core --configdir <cache>/darktable --library :memory: --conf
  write_sidecar_files=never`, annulable (le programme est arrêté).

## Rien n'est écrit chez l'utilisateur

Configuration propre à `tri` (dans son cache), bibliothèque en mémoire : ni la
configuration ni la bibliothèque de darktable ne sont touchées, et darktable peut rester
ouvert (pas de verrou). `write_sidecar_files=never` : le sidecar n'est jamais réécrit,
quelle que soit la configuration ; un test le vérifie sur un vrai rendu (sur une copie).

## Cache et calcul

Niveau `Level::Developed` : le JPEG de darktable, à la taille de l'aperçu standard,
rangé **tel quel** dans le cache. Sa clé comprend l'empreinte du sidecar (taille, date) :
modifier le développement dans darktable refait le rendu. Dans l'interface, un développé
déjà affiché dont le sidecar a changé (surveillance du dossier, ou au moment de `b` /
`Maj+B`) est oublié : `b` revient au rendu de base et `Maj+B` le refait (retour du
19/10/2026 : un recadrage modifié n'était pas vu). Calcul **sur demande
explicite seulement** (`Maj+B`, image affichée ; une image déjà en cache reste dans la
rotation de `b`, même d'une séance à l'autre), par le worker lourd (darktable occupe toute la
machine), jamais tout le dossier en fond. Mesuré le 19/10/2026 (RAF X-T4, Mac Apple
Silicon) : 8 à 10 s par image, dont ~6 s de démarrage de darktable.

## Dans l'interface

En attendant le rendu, le rendu RAW (ou le boîtier) avec « développement darktable en
cours… » ; sans développement pour l'image, la raison à la place. Pas de 1:1 développé :
on zoome dans le rendu. Le collimateur et les zones saturées / bouchées, placés sur le
capteur entier, sont masqués sur un développé (recadré). Sans darktable, la source n'est
pas proposée et `Maj+B` le dit.

À venir : `darktable-cli` sait traiter un dossier en un seul lancement (démarrage
amorti) — pour l'envoi de JPEG développés de l'« après-développement ».
