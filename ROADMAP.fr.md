# Feuille de route de tri

*[English version](ROADMAP.md)*

> **Figée le 21/10/2026.** La liste de la bêta 1 ne s'allonge plus : toute idée nouvelle
> va dans « après la V1 », sauf les bugs. Les retours se font par les *issues* du dépôt.

## Déjà là (alpha 3)

Grille et plein écran (rendu boîtier, rendu RAW, 1:1, zoom continu, loupe), notation XMP
compatible darktable / Lightroom / digiKam, filtres (notes, netteté, exposition,
inversion, recherche par nom), piles de photos semblables, netteté (score, carte),
histogramme et zones d'exposition, comparaison, sélection multiple, fichiers et dossiers
(renommer, corbeille, copier / déplacer, glisser-déposer, paires RAW+JPEG au choix),
annulation, déchargement de carte, export par préréglages, versions recadrées,
développé darktable, superpositions, réglages dans l'interface, raccourcis classiques et
vim, français et anglais. Paquets macOS (`.dmg`), Linux (`.tar.gz`) et Windows (`.msi`,
`.zip`).

## Bêta 1 (publique)

| Sujet | Détail |
|---|---|
| Retours des testeurs Windows | Essais sur de vraies machines (chemins, lecteurs, éjection, corbeille, clavier AZERTY, écrans à haute densité), bugs trouvés |
| Sélection à la souris | Maj+clic / Ctrl+clic dans la grille (retour d'un testeur, à vérifier sous Windows) |
| Manuel des testeurs | Captures refaites avec de vraies photos (RAW, collimateur, histogramme) |
| Rechercher une mise à jour | Sur demande (aide, `tri --check-update`) : nouvelle version sur la page Releases ? Ni notification ni installation automatique |

La bêta n'est **pas signée** : macOS et Windows avertissent au premier lancement, la
marche à suivre est dans le README.

## V1

| Sujet | Détail |
|---|---|
| Signature | macOS : Developer ID et notarisation ; Windows : SignPath Foundation (gratuit pour l'open source). Plus d'avertissement au premier lancement |
| RAW des autres marques | Essais Canon, Nikon, Sony, OM, Panasonic, DNG (raw.pixls.us) ; collimateur pour chacune |
| Notes dans les autres logiciels | Vérifier digiKam, RawTherapee, ART ; Lightroom et Capture One (testeurs) |
| Netteté dans la comparaison | Carte de netteté en mode comparaison côte à côte |

## Après la V1

| Sujet | Détail |
|---|---|
| Mots-clés | Hiérarchiques, compatibles avec les autres logiciels ; interface à maquetter d'abord |
| Fusion de dossiers | Déplacer un dossier dans un dossier du même nom |
| Recadrage vers les logiciels de développement | Écrire le cadre d'une version dans le sidecar, au format de Lightroom (`crs:Crop…`), darktable, Capture One |
| Déchargement | Gabarits de noms de dossier et de fichiers, cartes de plusieurs jours |
| Réglages de visualisation | Exposition, noir et blanc, contraste, uniformisation d'exposition (non destructifs) |
| Après-développement | Profils de sortie, envoi (AirDrop, autres) |
| Filtres complexes | Fenêtre ou panneau dédié |
| Vim plus complet | Nombres devant les actions (`5j`, `3x`) |
| Netteté par IA | Visages, yeux nets sur un portrait |
| Jeu d'icônes | Icônes dessinées (SVG) à la place des formes simples |
| Architecture cible | LibRaw en bibliothèque dynamique, interface en terminal (TUI) |
| Autres logiciels de développement | RawTherapee (`rawtherapee-cli`), ART, vkdt (`vkdt-cli`) : rendu et export comme avec darktable |
| Style darktable | Appliquer un style darktable à une image (`darktable-cli --style`) |
| Plugins externes | Un programme à part, dans n'importe quel langage (Python pour l'IA…), qui échange du JSON avec `tri` : analyses, filtres, envois |
| HEIC par le système | Décodeurs de macOS (ImageIO) et de Windows (extension HEVC) : rien de breveté livré avec `tri` |
| Mise à jour automatique | Télécharger et installer la nouvelle version (après la signature) |
| Autres paquets | AppImage ou Flatpak (Flathub), winget / Microsoft Store, Homebrew : plus de systèmes, plus visible |
| Déchargement des vidéos | Aujourd'hui ignorées sur la carte |
| Analyses d'image | Histogramme de luminance des images plates, HEIC 10 bits, exposition calculée pendant le rendu RAW |

## Comment aider

- **Tester** : surtout sous Windows, et avec d'autres marques d'appareils que Fujifilm.
  Paquets sur la [page Releases](https://github.com/fkinnar/tri/releases), sur une
  **copie** de vos photos.
- **Signaler** : par les [*issues*](https://github.com/fkinnar/tri/issues) du dépôt
  (système, appareil, ce qui était attendu et ce qui s'est passé).
- **Contribuer** : compilation dans [doc/fr/build.md](doc/fr/build.md), architecture et
  modules dans [doc/fr/](doc/fr/README.md). Rust, egui, LibRaw.
