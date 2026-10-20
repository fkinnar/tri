# Historique

## À venir

- **Dossiers** : copier ou déplacer un dossier entier (`Ctrl+C` ou `y` / `Ctrl+X` dans
  le panneau des dossiers, clic droit, annulable) ; `y` copiait les images
  sélectionnées au lieu du dossier. **Glisser-déposer** d'un dossier ou d'images de la
  grille sur un dossier, toujours confirmé (Option sur macOS, Ctrl ailleurs : copier).
  Panneau rangé en sections titrées : disques amovibles, réseau (partages montés, GVFS
  sous Linux), dossiers personnels (Bureau, Documents, Téléchargements, Images, comme le
  Finder), favoris, puis le système. Le clic droit « Ajouter aux favoris » ouvrait la boîte de
  suppression : corrigé.
- **Plein écran** : après un développement darktable (Maj+B), les autres images ne
  zoomaient plus (« pas encore développé ») ; une image sans développement se montre
  maintenant dans la source d'avant, avec son 1:1.
- **Piles** : le curseur de similarité ne déplie plus les piles pendant le réglage (on
  voit les piles obtenues) ; Maj tenue pour les voir dépliées.
- **Rapport d'export** (`Ctrl+Maj+E`, ou le bouton à côté du bilan d'un export) : les
  exports terminés, gardés d'une session à l'autre, avec un lien vers chaque dossier de
  destination ; effaçable.
- **Vocabulaire** : « préréglage d'export », comme « préréglage de recadrage »
  (« profil » reste aux profils couleur).
- **Réglages** : une seule entrée pour les préréglages d'export et une pour ceux de
  recadrage, avec une liste, « Nouveau… » et « Supprimer ».
- **Recadrage** : « Enregistrer comme préréglage » devient une petite disquette à côté
  de la liste des préréglages ; Enregistrer et Annuler à droite.
- **Boîte d'export** : les flèches ne changent le préréglage que sur sa liste ; Tab passe
  au dossier.

## 1.0.0-alpha.1 — 19/10/2026

Première version de test (dépôt privé ; la bêta sera publique). Paquets prêts à
installer pour macOS (`.dmg`) et Linux (`.tar.gz`), sans HEIC ; Windows à venir.

- **Dossiers** : panneau des dossiers (volumes, favoris, recherche `/`), surveillance du
  dossier ouvert, déchargement de cartes (copie vérifiée, doublons, éjection).
- **Grille** : vignettes en tâche de fond avec cache persistant, filtres (type, étoiles,
  couleurs, drapeau, netteté, nom), sélection multiple, piles d'images semblables
  (seuil réglable en direct), comparaison côte à côte.
- **Plein écran** : JPEG boîtier ou vrai rendu RAW (LibRaw), zoom continu jusqu'à 400 %,
  loupe, repère du collimateur (Fujifilm), carte de netteté, zones saturées / bouchées,
  superpositions (grilles, spirale d'or, texte, logo, format 4:5…).
- **Notes** : étoiles, couleurs, accepté / rejeté, en XMP (conventions darktable et
  Adobe, rien d'autre n'est touché), annulables.
- **Analyses** : histogramme RAW en IL, score de netteté jugé dans le dossier et dans la
  pile, regroupement par similarité.
- **Fichiers** : renommer, corbeille, copier / déplacer vers…, éditeur externe, AirDrop
  (macOS).
- **Développé dans darktable** : `Maj+B` (ou le bouton sous l'image) fait
  le rendu darktable-cli, en cache, rien d'écrit chez vous ; `b` le montre ensuite.
- **Export** : `Ctrl+E`, JPEG / TIFF / PNG par profils d'export (« Retouche » TIFF 16 bits
  ProPhoto, « Web » JPEG sRGB), développés par darktable (installé, en AppImage ou en
  Flatpak) ; sans darktable, rendu de base par LibRaw ; pas de ProPhoto en 8 bits.
- **Interface** : tout au clavier (jeux « classique » et « vim », raccourcis réglables),
  fenêtre de réglages, aide avec recherche, français et anglais, thèmes sombre, clair et
  gris neutre ; icône, barre du haut, mode concentration (F11).
