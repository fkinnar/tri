# Feuille de route de tri

*[English version](ROADMAP.md)*

> **Figée le 21/10/2026.** La liste de la bêta 1 ne s'allonge plus : toute idée nouvelle
> va dans « après la V1 », sauf les bugs. Les retours se font par les *issues* du dépôt ;
> chaque sujet ci-dessous a la sienne, en lien à côté de son nom.

## Déjà là (alpha 7)

Grille et plein écran (rendu boîtier, rendu RAW, 1:1, zoom continu, loupe), notation XMP
compatible darktable / Lightroom / digiKam, filtres (notes, netteté, exposition,
inversion, recherche par nom), piles de photos semblables, netteté (score, carte),
histogramme et zones d'exposition, comparaison, sélection multiple, fichiers et dossiers
(renommer, corbeille, copier / déplacer, glisser-déposer, paires RAW+JPEG au choix),
annulation, déchargement de carte, export par préréglages, versions recadrées, développé
darktable, superpositions, réglages dans l'interface, raccourcis classiques et vim,
sélection à la souris (Maj+clic, Ctrl+clic), recherche de mise à jour et signalement
de problème, tri par nom ou date, favoris nommés, français et
anglais. Paquets macOS (`.dmg`), Linux (`.tar.gz`, `.deb`, AppImage) et Windows (`.msi`, `.zip`), et
builds de nuit (Release « nightly »).

## Bêta 1 (publique)

| Sujet | Détail |
| --- | --- |
| Retours des testeurs Windows [#124](https://github.com/fkinnar/tri/issues/124) | Essais sur de vraies machines (chemins, lecteurs, éjection, corbeille, clavier AZERTY, écrans à haute densité), bugs trouvés |
| Manuel des testeurs [#125](https://github.com/fkinnar/tri/issues/125) | Captures refaites avec de vraies photos (RAW, collimateur, histogramme) |
| Gros dossiers sur disque lent [#126](https://github.com/fkinnar/tri/issues/126) | Lecture du dossier en fond, progression visible (la fenêtre se figeait le temps de lister des milliers de fichiers et leurs sidecars sur un disque dur externe) |
| Tri par note, couleur [#127](https://github.com/fkinnar/tri/issues/127) | À concevoir avec les piles et le jugement de netteté, qui s'appuient sur l'ordre de prise de vue |
| Vider le cache depuis l'interface [#128](https://github.com/fkinnar/tri/issues/128) | Tout, un niveau, ou le dossier ouvert (aujourd'hui : `tri <dossier> --clear-cache`), avec la place occupée |
| Développement par RapidRAW [#114](https://github.com/fkinnar/tri/issues/114) | Rendu et export par `rapidraw export` (ligne de commande depuis RapidRAW 1.6), avec les réglages de ses sidecars `.rrdata`, comme avec darktable |

La bêta n'est **pas signée** : macOS et Windows avertissent au premier lancement, la
marche à suivre est dans le README.

## V1

| Sujet | Détail |
| --- | --- |
| Signature [#129](https://github.com/fkinnar/tri/issues/129) | macOS : Developer ID et notarisation ; Windows : SignPath Foundation (gratuit pour l'open source). Plus d'avertissement au premier lancement |
| Collimateur des autres marques [#130](https://github.com/fkinnar/tri/issues/130) | Tous les RAW que lit LibRaw sont déjà gérés (affichage, histogramme, métadonnées) ; seul le repère du collimateur est pour l'instant propre à Fujifilm. À ajouter : Canon, Nikon, Sony, OM, Panasonic, vérifiés sur des échantillons de raw.pixls.us |
| Notes dans les autres logiciels [#131](https://github.com/fkinnar/tri/issues/131) | Vérifier digiKam, RawTherapee, ART ; Lightroom et Capture One (testeurs) |
| Netteté dans la comparaison [#132](https://github.com/fkinnar/tri/issues/132) | Carte de netteté en mode comparaison côte à côte |

## Après la V1

| Sujet | Détail |
| --- | --- |
| Mots-clés [#133](https://github.com/fkinnar/tri/issues/133) | Hiérarchiques, compatibles avec les autres logiciels ; interface à maquetter d'abord |
| Fusion de dossiers [#134](https://github.com/fkinnar/tri/issues/134) | Déplacer un dossier dans un dossier du même nom |
| Recadrage vers les logiciels de développement [#135](https://github.com/fkinnar/tri/issues/135) | Écrire le cadre d'une version dans le sidecar, au format de Lightroom (`crs:Crop…`), darktable, Capture One |
| Déchargement [#136](https://github.com/fkinnar/tri/issues/136) | Gabarits de noms de dossier et de fichiers, cartes de plusieurs jours |
| Réglages de visualisation [#137](https://github.com/fkinnar/tri/issues/137) | Exposition, noir et blanc, contraste, uniformisation d'exposition (non destructifs) |
| Filtres complexes [#138](https://github.com/fkinnar/tri/issues/138) | Fenêtre ou panneau dédié |
| Filtres sur les EXIF [#4](https://github.com/fkinnar/tri/issues/4) | Appareil, objectif, focale, ouverture, vitesse, ISO… : au moins ce que montre le panneau des métadonnées (issue n° 4) |
| Vim plus complet [#139](https://github.com/fkinnar/tri/issues/139) | Nombres devant les actions (`5j`, `3x`) |
| Netteté par IA [#140](https://github.com/fkinnar/tri/issues/140) | Visages, yeux nets sur un portrait |
| Jeu d'icônes [#141](https://github.com/fkinnar/tri/issues/141) | Icônes dessinées (SVG) à la place des formes simples |
| Architecture cible [#142](https://github.com/fkinnar/tri/issues/142) | LibRaw en bibliothèque dynamique, interface en terminal (TUI) |
| Autres logiciels de développement [#115](https://github.com/fkinnar/tri/issues/115), [#143](https://github.com/fkinnar/tri/issues/143) | RawTherapee (`rawtherapee-cli`), ART, vkdt (`vkdt-cli`) : rendu et export comme avec darktable |
| Style darktable [#144](https://github.com/fkinnar/tri/issues/144) | Appliquer un style darktable à une image (`darktable-cli --style`) |
| Plugins externes [#145](https://github.com/fkinnar/tri/issues/145) | Un programme à part, dans n'importe quel langage (Python pour l'IA…), qui échange du JSON avec `tri` : analyses, filtres, envois. **Exemple fourni : notation en pas à pas**, deux raccourcis pour tout : note +1 / −1, couleur suivante / précédente (ordre réglable), rejeté → neutre → accepté, et le filtre étoiles / couleur au cran suivant / précédent ; rien ne bouge au bout de la séquence |
| HEIC par le système [#146](https://github.com/fkinnar/tri/issues/146) | Décodeurs de macOS (ImageIO) et de Windows (extension HEVC) : rien de breveté livré avec `tri` |
| Mise à jour automatique [#147](https://github.com/fkinnar/tri/issues/147) | Télécharger et installer la nouvelle version (après la signature) |
| Autres paquets [#148](https://github.com/fkinnar/tri/issues/148) | Flatpak (Flathub), winget / Microsoft Store, Homebrew : plus de systèmes, plus visible |
| Déchargement des vidéos [#149](https://github.com/fkinnar/tri/issues/149) | Aujourd'hui ignorées sur la carte |
| Analyses d'image [#150](https://github.com/fkinnar/tri/issues/150) | Histogramme de luminance des images plates, HEIC 10 bits, exposition calculée pendant le rendu RAW |
| Idées des issues | Aperçu à l'importation [#42](https://github.com/fkinnar/tri/issues/42), groupe à partir d'une image [#45](https://github.com/fkinnar/tri/issues/45), exclure des images similaires des filtres netteté / exposition [#46](https://github.com/fkinnar/tri/issues/46), plusieurs écrans [#53](https://github.com/fkinnar/tri/issues/53), mode visionneuse [#67](https://github.com/fkinnar/tri/issues/67), diaporama [#68](https://github.com/fkinnar/tri/issues/68), collections et catalogues optionnels [#117](https://github.com/fkinnar/tri/issues/117), serveur MCP [#121](https://github.com/fkinnar/tri/issues/121), API REST [#122](https://github.com/fkinnar/tri/issues/122), impression [#161](https://github.com/fkinnar/tri/issues/161), sauvegarde [#162](https://github.com/fkinnar/tri/issues/162), ouvrir un dossier dans darktable [#174](https://github.com/fkinnar/tri/issues/174) |

## Comment aider

- **Tester** : surtout sous Windows, et avec d'autres marques d'appareils que Fujifilm.
  Paquets sur la [page Releases](https://github.com/fkinnar/tri/releases), sur une
  **copie** de vos photos.
- **Signaler** : par les [*issues*](https://github.com/fkinnar/tri/issues) du dépôt
  (système, appareil, ce qui était attendu et ce qui s'est passé).
- **Contribuer** : compilation dans [doc/fr/build.md](doc/fr/build.md), architecture et
  modules dans [doc/fr/](doc/fr/README.md). Rust, egui, LibRaw. Une branche `fix/…` ou
  `feat/…` et une pull request vers `main`, vérifiée par la CI sur les trois systèmes.
