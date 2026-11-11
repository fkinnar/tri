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

## La mémoire : `notes.json`

Un fichier par dossier, indexé par nom de fichier, parmi les données du dossier
([store.md](store.md), 11/11/2026) : dans son `.tri/` quand l'interrupteur « garder les
données de tri ici » le couvre (voyage avec le dossier), sinon dans le dossier de données
de `tri` (`folders/<empreinte>/notes.json` ; `reconcile::set_data_dir` au démarrage) ;
avant le 11/11/2026, toujours écrit dans `.tri/`, il est déplacé à l'ouverture. Créé
seulement quand une image a des notes, retiré quand il redevient vide ; dossier `.tri`
caché sous Windows (attribut), ignoré du parcours des images, des sous-dossiers et de la
surveillance (nom en point).

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
- **Un conflit ne se tranche jamais en silence** (bug du 07/11/2026 : « Relire les notes
  depuis les images » le tranchait en faveur de l'image) : noter (`edit`), annuler
  (`write_only`), relire depuis les images (`import`), inscrire dans les images
  (`embed`), régénérer les sidecars (`generate`) refusent une image en conflit (« à
  trancher d'abord ») ; l'interface ouvre alors la boîte. Seul `resolve` tranche.
- « Relire les notes depuis les images » (`NoteBook::import`) recopie dans les sidecars
  une note fusionnée depuis l'image.

## Ligne de commande

`tri <dossier> --conflicts` : fusions faites, et chaque image en conflit avec ses
sources et leurs notes.

## Interface

- Une fusion est dite dans la barre d'état (« Notes fusionnées depuis d'autres logiciels
  pour N images »).
- Une image en conflit porte un triangle orange en haut à gauche de sa vignette
  (`marks::conflict_sign`) ; un clic, ou une note posée sur elle, ouvre la boîte « Notes
  en désaccord » : un tableau, une ligne par source (XMP, RapidRAW, RawTherapee, ART,
  « Dans l'image ») et les notes d'avant, la date de modification de chaque source
  (`Conflict::modified`, à l'heure locale : un indice), une colonne par champ. Seuls les champs en
  désaccord (titre en gras) se choisissent, d'un clic sur la valeur voulue ; un champ où
  les lignes diffèrent sans conflit (une seule source l'a changé) est présélectionné sur
  la valeur fusionnée, et modifiable aussi (retour du 07/11/2026 : une couleur différente
  qu'on ne pouvait pas choisir) ; les autres sont déjà fusionnés (`Conflict::merged`, retour du 07/11/2026 : une couleur ajoutée dans
  l'image ne doit pas dépendre du choix des étoiles). Lignes alternées, colonnes de
  largeur fixe, cellules dessinées à la main (valeurs centrées). La flèche épaisse d'une
  ligne (`marks::take_row_arrow`) y choisit tous les champs en désaccord, sans valider ;
  la ligne « Résultat », à l'accent (elle ne se clique pas), montre les notes qui seront
  gardées ; « Valider » en bas à droite. « Même choix pour les N autres images en conflit du dossier » : la même
  sorte de source, champ par champ. Un pas d'historique, annulable.

- Filtre « En conflit » (`f c`, `core.filter.conflicts`) dans la section Filtres du
  panneau de droite, sous les autres (filtre secondaire : pas dans la barre du haut),
  seulement quand il y a des conflits (`notes::ConflictFilter`).
- Le triangle et les icônes d'écrêtage de la vignette sont sur une rangée, centrés sur
  la hauteur des marques d'en face.

## Fichiers renommés, déplacés, supprimés

`files::rename`, `files::transfer` (copier ou déplacer) et `files::delete` font suivre la
mémoire (`reconcile::carry`, `reconcile::forget`) : l'entrée de l'image change de nom et
de dossier, ses sources renommées comme elles. Sans cela, une image dont une source est
en retard (l'image non réécrite) passerait en conflit à sa nouvelle place. Un dossier
copié ou déplacé emporte son `.tri` avec lui. L'annulation passe par les mêmes
fonctions.

## Automatisation (07/11/2026)

- **Écriture d'office** (`[xmp] embed`) : `never` (défaut), `internal` (disques internes
  et fixés ; pas un disque amovible ni un partage réseau, `places::outside_roots`),
  `everywhere`. À chaque note (et annulation, choix d'un conflit), les notes vont aussi
  dans l'image (`NoteBook::embeds`). Jamais une image en conflit.
- **Réglage du dossier**, prioritaire (« ce support amovible-là, oui ») : selon les
  réglages, toujours ou jamais ; gardé dans `.tri/notes.json` (champ `embed`,
  `Memory::embed`), donc il voyage avec le dossier ; le dossier seul, pas ses
  sous-dossiers (exceptions posées en connaissance de cause). Clic droit sur le dossier,
  « Notes dans les images » ; un repère dans le panneau des dossiers (à l'accent :
  toujours ; gris barré : jamais). `NoteBook::folder_embed`, `set_folder_embed`.
- **Fusion à la volée** : la surveillance relit le dossier quand une image ou un sidecar
  bouge ; la fusion se fait à la lecture, et un conflit fait apparaître le triangle.
- **Propagation** : une fusion venue d'une source est récrite aussitôt dans les autres
  (sidecars existants ; l'image si elle reçoit les notes d'office), là où `tri` écrit de
  lui-même (`NoteBook::writes_automatically`, `propagate`).
- Après ses propres écritures dans les images, `tri` recale la surveillance
  (`own_writes_done`) : pas de relecture du dossier comme pour un changement venu
  d'ailleurs.
- Abandonné pour l'instant : un dossier « notes dans les fichiers, sans sidecar ».
