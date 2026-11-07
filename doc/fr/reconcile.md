# Module `reconcile` : notes venues de plusieurs sources

`src/reconcile.rs` (issue n° 368, 07/11/2026).

Une photo peut porter ses notes à plusieurs endroits : `.xmp` (darktable, Lightroom…),
`.rrdata` (RapidRAW), `.pp3` / `.arp` (RawTherapee, ART), l'image elle-même
([embedded.md](embedded.md)). Chaque logiciel n'en modifie qu'un. Avant, `tri` lisait le
sidecar le plus récent : un changement fait ailleurs pouvait disparaître en silence à la
note suivante.

## Ce que chaque source a changé

Pour chaque image et **chaque source**, `tri` retient ce qu'il y a lu (ou écrit) la
dernière fois, avec la date et la taille du fichier ([`Stamp`]), et les notes qu'il en a
retenues. À la lecture (`reconcile`) :

- une source dont la date et la taille n'ont pas bougé n'a rien changé ;
- sinon, ses champs qui diffèrent de ce qu'on y avait lu sont **ses** changements : un
  JPEG retouché ailleurs sans toucher aux notes ne change rien, même si ses notes sont en
  retard ;
- **fusion champ par champ** (étoiles, drapeau, chaque couleur) : changé par une source,
  ou par plusieurs de la même façon → adopté (`Outcome::Merged`, avec les sources) ;
  différemment → **conflit** (`Outcome::Conflict`) ;
- une fusion de couleurs qu'aucune source ne porte (deux couleurs ajoutées chacune d'un
  côté) → conflit : on n'en invente pas en mode une seule couleur ;
- une **source inconnue** (première lecture, sidecar créé par un autre logiciel) en
  désaccord → conflit : on demande, sans deviner par les dates (cartes en FAT à 2 s
  près, horloges des NAS). En attendant, les notes les plus récentes sont montrées ;
- une image **muette** (qui peut porter des notes mais n'en a pas) est retenue sans
  compter dans les accords : une note qui y apparaît ensuite (Explorateur Windows,
  XnView…) est un changement connu, fusionné.

Après une écriture par `tri` (noter, annuler, trancher), ce qu'il vient d'écrire fait foi
(`notes::remember`) : une source restée en retard (l'image, non réécrite) n'est pas un
désaccord.

## La mémoire : `.tri/notes.json`

À côté des photos, un fichier par dossier, indexé par nom de fichier (voyage avec le
dossier, facile à effacer pour tester). Créé seulement quand une image a des notes,
retiré quand il redevient vide ; dossier `.tri` caché sous Windows (attribut), ignoré du
parcours des images, des sous-dossiers et de la surveillance (nom en point). Dossier en
lecture seule : repli dans le dossier de données de `tri` (`notes/<empreinte>.json`,
`reconcile::set_data_dir` au démarrage).

Pas dans le XMP : darktable réécrit ses XMP depuis sa base et risque d'effacer nos
champs.

## Dans `notes.rs`

- Chaque lecture (`read_notes`) prend toutes les sources : les sidecars, et l'image
  (`embedded`) si elle peut porter des notes.
- Une image en conflit montre ses notes d'avant ; `NoteBook::edit` y est **refusé**
  (noter écraserait une source sans le savoir) ; `NoteBook::resolve` tranche (notes
  écrites dans tous les sidecars, la paire suit, et retenues).
- `NoteBook::conflict`, `conflicts`, `take_merged` (fusions faites depuis le dernier
  appel, pour un message discret).
- « Relire les notes depuis les images » (`NoteBook::import`) tranche en faveur de
  l'image, et recopie dans les sidecars une note fusionnée depuis elle.

## Ligne de commande

`tri <dossier> --conflicts` : fusions faites, et chaque image en conflit avec ses
sources et leurs notes.

## Interface

- Une fusion est dite dans la barre d'état (« Notes fusionnées depuis d'autres logiciels
  pour N images »).
- Une image en conflit porte un triangle orange en haut à gauche de sa vignette
  (`marks::conflict_sign`) ; un clic, ou une note posée sur elle, ouvre la boîte « Notes
  en désaccord » : un tableau, une ligne par source (XMP, RapidRAW, RawTherapee, ART,
  « Dans l'image ») et les notes d'avant, une colonne par champ. Seuls les champs en
  désaccord (en gras) se choisissent, d'un clic sur la valeur voulue ; les autres sont
  déjà fusionnés (`Conflict::merged`, retour du 07/11/2026 : une couleur ajoutée dans
  l'image ne doit pas dépendre du choix des étoiles). Lignes alternées, colonnes de
  largeur fixe, cellules dessinées à la main (valeurs centrées). La flèche épaisse d'une
  ligne (`marks::take_row_arrow`) y choisit tous les champs en désaccord, sans valider ;
  la ligne « Résultat », à l'accent (elle ne se clique pas), montre les notes qui seront
  gardées ; « Valider » en bas à droite. « Même choix pour les N autres images en conflit du dossier » : la même
  sorte de source, champ par champ. Un pas d'historique, annulable.

## À venir

Un filtre « en conflit », puis l'automatisation (écrire
partout, fusion à la volée sur les changements vus par la surveillance). Renommer ou
déplacer une image n'emporte pas encore son entrée : elle est relue comme la première
fois.
