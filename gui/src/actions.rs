//! Actions nommées et raccourcis clavier.
//!
//! Une touche ne déclenche jamais directement une fonction : elle est traduite en
//! [`Action`], identifiée par un nom stable (`core.nav.next`...). La section `[keys]` du
//! fichier de configuration associe des touches à ces noms ; les plugins auront leur propre
//! espace (`plugin.<nom>.*`). Un raccourci peut être une séquence de touches (`g g`, comme
//! dans vim) : voir [`Sequencer`]. Voir `doc/en/gui.md`.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use tri::i18n::Language;

use eframe::egui::{Event, Key, Modifiers};
use tri::xmp::{Color, Flag};

/// Thème d'une action, pour l'aide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    /// Se déplacer d'une image à l'autre, ouvrir, revenir.
    Navigation,
    /// Sélection multiple.
    Selection,
    /// Plein écran : zoom, source, déplacement, histogramme, zones.
    View,
    /// Étoiles, couleurs, drapeau.
    Rating,
    /// Filtres.
    Filters,
    /// Panneaux et barres.
    Panels,
    /// Renommer, supprimer, afficher, éditeur externe.
    Files,
    /// Annuler, rétablir, historique.
    Edit,
    /// Recadrage (versions).
    Crop,
    /// Aide, quitter.
    App,
}

impl Group {
    /// Les thèmes, dans l'ordre de l'aide.
    pub const ALL: [Group; 10] = [
        Group::Navigation,
        Group::Selection,
        Group::View,
        Group::Crop,
        Group::Rating,
        Group::Filters,
        Group::Panels,
        Group::Files,
        Group::Edit,
        Group::App,
    ];

    /// Titre dans l'aide, dans la langue de l'interface.
    pub fn label(self) -> String {
        t!(&format!("group-{self:?}").to_lowercase())
    }
}

/// Mots-clés de recherche, invisibles, d'un réglage (`gui.profile`), d'une section
/// (`gui.zoom`) ou d'une action (`core.view.zoom`) : message `kw-<id>` des catalogues
/// (points et soulignés en tirets), vide s'il n'y en a pas. Ils aident la recherche de
/// l'aide et des réglages quand on ne sait qu'« à peu près » ce qu'on cherche (« vim »
/// trouve le jeu de raccourcis ; 22/10/2026).
pub fn keywords(id: &str) -> String {
    let key = format!("kw-{}", id.replace(['.', '_'], "-"));
    if tri::i18n::has(tri::i18n::language(), &key) {
        tri::i18n::translate(&key, None)
    } else {
        String::new()
    }
}

/// Identifiant actuel d'une action renommée : les anciens restent acceptés (un `[keys]`
/// écrit avant continue de marcher). 07/11/2026 : recadrage et versions ne font qu'un,
/// `core.view.crop` → `core.version.new`, `core.view.version_next` / `_prev` →
/// `core.version.next` / `prev`, `core.crop.*` → `core.version.*`.
pub fn renamed_id(id: &str) -> std::borrow::Cow<'_, str> {
    match id {
        "core.view.crop" => "core.version.new".into(),
        "core.view.version_next" => "core.version.next".into(),
        "core.view.version_prev" => "core.version.prev".into(),
        _ => match id.strip_prefix("core.crop.") {
            Some(rest) => format!("core.version.{rest}").into(),
            None => id.into(),
        },
    }
}

/// Direction (recadrage au clavier).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    /// Gauche.
    Left,
    /// Droite.
    Right,
    /// Haut.
    Up,
    /// Bas.
    Down,
}

/// Filtres sur le drapeau proposés à l'utilisateur.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FlagFilter {
    /// Seulement les photos acceptées.
    Accepted,
    /// Masquer les photos rejetées.
    NotRejected,
    /// Seulement les photos rejetées.
    Rejected,
}

impl FlagFilter {
    /// Les trois filtres.
    pub const ALL: [FlagFilter; 3] = [
        FlagFilter::Accepted,
        FlagFilter::NotRejected,
        FlagFilter::Rejected,
    ];

    /// Drapeaux gardés par ce filtre.
    pub fn flags(self) -> Vec<Flag> {
        match self {
            FlagFilter::Accepted => vec![Flag::Accepted],
            FlagFilter::NotRejected => vec![Flag::None, Flag::Accepted],
            FlagFilter::Rejected => vec![Flag::Rejected],
        }
    }

    /// Nom court (barre de filtres, barre d'état).
    pub fn label(self) -> String {
        match self {
            FlagFilter::Accepted => t!("flag-filter-accepted"),
            FlagFilter::NotRejected => t!("flag-filter-not-rejected"),
            FlagFilter::Rejected => t!("flag-filter-rejected"),
        }
    }
}

/// Ce que l'utilisateur demande, indépendamment de la touche (ou du clic) utilisée.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// Image précédente.
    NavPrev,
    /// Image suivante.
    NavNext,
    /// Ligne du dessus (grille).
    NavUp,
    /// Ligne du dessous (grille).
    NavDown,
    /// Première image.
    NavFirst,
    /// Dernière image.
    NavLast,
    /// Une page plus haut (grille).
    NavPageUp,
    /// Une page plus bas (grille).
    NavPageDown,
    /// Filtre de type suivant : tout → RAW → plates → tout.
    FilterCycle,
    /// Filtre : n étoiles, selon la comparaison choisie (0 : étoiles indifférentes).
    FilterStars(u8),
    /// Comparaison du filtre d'étoiles suivante : au moins, exactement, au plus (#165).
    FilterStarsOp,
    /// Filtre sur le drapeau (activer / désactiver).
    FilterFlag(FlagFilter),
    /// Filtre : ajouter ou retirer une couleur.
    FilterColor(Color),
    /// Effacer tous les filtres (type compris).
    FilterClear,
    /// Filtre : masquer les photos jugées floues.
    FilterBlurry,
    /// Filtre : seulement les photos bien exposées (ni brûlées ni bouchées).
    FilterWellExposed,
    /// Filtre : seulement la pile courante, toute dépliée (issue n° 52).
    FilterStack,
    /// Inverser les filtres : ce qu'ils retirent (les floues, avec « nettes »…).
    FilterInvert,
    /// Filtre « développées » suivant : un développement, un logiciel, rien.
    FilterDeveloped,
    /// Ordre suivant : nom, date de prise de vue, date de modification.
    SortNext,
    /// Inverser le sens de l'ordre.
    SortReverse,
    /// Trier selon cette clé (nom, prise de vue…), sans passer par les autres.
    SortBy(tri::sort::SortKey),
    /// Montrer tout, les RAW seulement ou les JPEG (images plates) seulement.
    FilterKind(tri::filter::KindFilter),
    /// Rendu des vignettes suivant : JPEG boîtier, dernier développement, un logiciel,
    /// LibRaw (#114).
    GridRenderNext,
    /// Ce rendu affiché, directement (07/11/2026 : depuis la palette, sans passer par les
    /// autres, comme `SortBy`).
    Render(crate::grid_render::GridRender),
    /// Afficher / masquer la barre de filtres.
    ToggleFilterBar,
    /// Afficher / masquer le bandeau de notation.
    ToggleRatingBar,
    /// Mode concentration : afficher / masquer les infos et notes dessinées sur l'image.
    ToggleImageInfo,
    /// Couleur de fond suivante (grille, plein écran) : thème, puis `[gui.colors]
    /// backgrounds`.
    BackgroundNext,
    /// Afficher / masquer les infos des vignettes (nom, format, notes, badges).
    ToggleGridInfo,
    /// Avance automatique à l'image suivante après une note : oui / non.
    ToggleAutoAdvance,
    /// Comparer les images sélectionnées côte à côte.
    Compare,
    /// Afficher / masquer la carte de netteté.
    TogglePeaking,
    /// Carte de netteté : couleur suivante.
    PeakingColor,
    /// Netteté plus souple (curseur de sévérité à gauche : moins de photos floues, plus de
    /// contours marqués nets).
    SharpnessLenient,
    /// Netteté plus stricte (curseur de sévérité à droite).
    SharpnessStrict,
    /// Afficher / masquer le collimateur de mise au point.
    ToggleFocusPoint,
    /// Superposition suivante (aucune, grilles, celles de l'utilisateur).
    OverlayNext,
    /// Orientation des lignes de la superposition (œil de la spirale d'un coin à l'autre).
    OverlayOrient,
    /// Sélectionner / désélectionner l'image courante.
    SelectToggle,
    /// Mode visuel : la plage suit l'image courante.
    SelectVisual,
    /// Tout sélectionner.
    SelectAll,
    /// Tout désélectionner.
    SelectClear,
    /// Renommer l'image (sa paire et ses sidecars suivent).
    FileRename,
    /// Mettre l'image à la corbeille (après confirmation).
    FileDelete,
    /// Afficher l'image dans le gestionnaire de fichiers.
    FileReveal,
    /// Ouvrir l'image dans l'éditeur externe.
    FileEdit,
    /// Générer les sidecars de notes manquants (un logiciel ou tous) de la sélection ou
    /// du dossier (31/10/2026, suite de #253).
    Sidecars,
    /// Inscrire les notes dans les images elles-mêmes (JPEG, PNG, WebP, TIFF), de la
    /// sélection ou du dossier (issue n° 276, 07/11/2026).
    EmbedNotes,
    /// Annuler la dernière modification.
    Undo,
    /// Rétablir la dernière modification annulée.
    Redo,
    /// Liste de l'historique (retour à un pas donné).
    History,
    /// Déplier / replier la pile de l'image sélectionnée.
    StackToggle,
    /// `((` : ouvrir le niveau suivant — la pile de rafale repliée, sinon les
    /// développements de la photo (harmonisation du 31/10/2026).
    StackOpen,
    /// `))` : fermer le niveau le plus intérieur — les développements, sinon la rafale.
    StackClose,
    /// Déplier toutes les piles, ou les replier toutes.
    StacksAll,
    /// Fenêtre de regroupement (seuil, écart de temps).
    GroupingWindow,
    /// Mode pile en plein écran : ← / → dans la pile, ↑ / ↓ de pile en pile.
    StackMode,
    /// Déplier / replier les développements de la photo (pile de la photo et de ses
    /// développements darktable / RapidRAW, 30/10/2026).
    DevToggle,
    /// Développement suivant de la photo (en boucle).
    DevNext,
    /// Développement précédent de la photo.
    DevPrev,
    /// Replier toutes les piles de développements (31/10/2026).
    DevFoldAll,
    /// Zoom : palier supérieur.
    ZoomIn,
    /// Zoom : palier inférieur (jusqu'à l'ajusté).
    ZoomOut,
    /// Zoom : ajusté.
    ZoomFit,
    /// Zoom : 100 %.
    Zoom100,
    /// Faire de l'image la couverture de sa pile (sa préférée).
    StackChoose,
    /// Première image de la pile (issue n° 62).
    StackFirst,
    /// Dernière image de la pile.
    StackLast,
    /// En entrant dans une pile : sa couverture, ou sa première image (`[gui.view]
    /// stack_enter`, bouton du bandeau en mode pile).
    StackEnterToggle,
    /// Clavier au panneau de gauche (après le leader, `Ctrl+B` par défaut).
    FocusLeft,
    /// Clavier au panneau de droite.
    FocusRight,
    /// Clavier au panneau suivant.
    FocusNext,
    /// Panneau des dossiers : dossier du dessus.
    FolderUp,
    /// Panneau des dossiers : dossier du dessous.
    FolderDown,
    /// Panneau des dossiers : déplier (déjà déplié : premier sous-dossier).
    FolderExpand,
    /// Panneau des dossiers : replier (déjà replié : dossier parent).
    FolderCollapse,
    /// Panneau des dossiers : ouvrir le dossier pointé (le clavier revient aux images).
    FolderOpen,
    /// Panneau des dossiers : premier dossier.
    FolderFirst,
    /// Panneau des dossiers : dernier dossier.
    FolderLast,
    /// Panneau des dossiers : décharger le support amovible pointé.
    FolderOffload,
    /// Panneau des dossiers : éjecter le support amovible pointé.
    FolderEject,
    /// Panneau des dossiers : créer un dossier dans le dossier pointé.
    FolderCreate,
    /// Panneau des dossiers : renommer le dossier pointé.
    FolderRename,
    /// Panneau des dossiers : supprimer le dossier pointé (confirmé).
    FolderDelete,
    /// Panneau des dossiers : ajouter le dossier pointé aux favoris, ou l'en retirer.
    FolderFavorite,
    /// Panneau des dossiers : nommer le favori pointé (vide : nom du dossier).
    FolderFavoriteName,
    /// Panneau des dossiers : se connecter à un partage réseau (`\\nas\photo`,
    /// `smb://nas/photo`), tapé dans le champ de recherche (28/10/2026).
    FolderConnect,
    /// Panneau des dossiers : oublier le partage réseau mémorisé pointé.
    FolderForget,
    /// Panneau des dossiers : ouvrir le dossier pointé dans un logiciel de
    /// développement (darktable, 28/10/2026).
    FolderOpenIn(tri::develop::Engine),
    /// Panneau des dossiers : copier le dossier pointé vers… (une boîte confirme).
    FolderCopy,
    /// Panneau des dossiers : déplacer le dossier pointé vers… (une boîte confirme).
    FolderMove,
    /// Panneau des métadonnées : ligne du dessus.
    MetaUp,
    /// Panneau des métadonnées : ligne du dessous.
    MetaDown,
    /// Panneau des métadonnées : copier la valeur de la ligne pointée.
    MetaCopy,
    /// Panneau des métadonnées : ouvrir ce qui est pointé (l'histogramme en grand).
    MetaOpen,
    /// Panneau des métadonnées : étendre la sélection vers le haut.
    MetaSelectUp,
    /// Panneau des métadonnées : étendre la sélection vers le bas.
    MetaSelectDown,
    /// Panneau des métadonnées : copier tout le panneau.
    MetaCopyAll,
    /// Panneau de droite : section suivante (Tab, `)` en vim).
    PanelSectionNext,
    /// Panneau de droite : section précédente (Maj+Tab, `(` en vim).
    PanelSectionPrev,
    /// Panneau de droite : actionner le réglage pointé (cocher, choisir, replier une
    /// section, curseur au centre).
    PanelActivate,
    /// Panneau de droite : réglage pointé vers la gauche (curseur, liste ; replier une
    /// section).
    PanelLeft,
    /// Panneau de droite : réglage pointé vers la droite (curseur, liste ; déplier une
    /// section).
    PanelRight,
    /// Panneau des dossiers : première ligne de la section suivante.
    FolderSectionNext,
    /// Panneau des dossiers : première ligne de la section précédente.
    FolderSectionPrev,
    /// Copier les images vers un dossier.
    FileCopy,
    /// Déplacer les images vers un dossier.
    FileMove,
    /// Supprimer toute la pile de l'image (`dw`, comme un mot dans vim).
    FileDeleteStack,
    /// La photo entière, avec tous ses développements (Ctrl+Suppr / `dd`).
    FileDeletePhoto,
    /// Envoyer la sélection par un plugin d'envoi ([`tri::send`]) ; l'identifiant complet
    /// de l'action, dans l'espace de noms des plugins (`plugin.airdrop.send`).
    Send(&'static str),
    /// Copier toute la pile de l'image vers un dossier (`yw`).
    FileCopyStack,
    /// Exporter la sélection (profil d'export, darktable), voir [`tri::export`].
    FileExport,
    /// Rapport d'export : les exports terminés et leurs dossiers ([`tri::export_log`]).
    ExportReport,
    /// Plein écran : recadrer l'image (créer une version, [`tri::versions`]).
    Crop,
    /// Recadrage : déplacer le cadre.
    CropMove(Dir),
    /// Recadrage : tirer le côté droit (gauche / droite) ou le bas (haut / bas).
    CropSize(Dir),
    /// Recadrage : rapport n° (dans [`tri::versions::Ratio::PRESETS`]).
    CropRatio(u8),
    /// Recadrage : portrait ↔ paysage.
    CropRotate,
    /// Recadrage : enregistrer la version.
    CropSave,
    /// Recadrage : préréglage suivant.
    CropPreset,
    /// Recadrage : préréglage précédent.
    CropPresetPrev,
    /// Version : rapport suivant (ceux fournis, puis ceux ajoutés ; 07/11/2026).
    VersionRatioNext,
    /// Version : rapport précédent.
    VersionRatioPrev,
    /// Version : taper un rapport à soi (la liste s'ouvre sur le champ).
    VersionRatioCustom,
    /// Version : préréglage d'export suivant (« aucun » compris).
    VersionExportNext,
    /// Version : préréglage d'export précédent.
    VersionExportPrev,
    /// Version : modifier le nom (le champ prend le clavier).
    VersionName,
    /// Version : enregistrer comme préréglage (la disquette).
    VersionSavePreset,
    /// Version : revenir au cadre d'origine, l'image entière (07/11/2026).
    VersionReset,
    /// Plein écran : version suivante de l'image (l'original, puis chaque version).
    VersionNext,
    /// Plein écran : version précédente (dans l'autre sens, 06/11/2026).
    VersionPrev,
    /// Rechercher : en grille, filtre par nom de fichier ; dans le panneau des dossiers,
    /// un dossier (aussi `/`, sauf en plein écran où il vaut « ajusté »).
    Search,
    /// Mode concentration : plein écran du système, panneaux et barres masqués.
    FocusMode,
    /// Développer l'image affichée dans son logiciel (celui de son développement, ou le
    /// préféré, `[develop] default`), puis l'afficher.
    Develop,
    /// Développer l'image affichée dans ce logiciel-là (plugin de rendu), puis l'afficher
    /// (26/10/2026 : une image développée dans darktable et dans RapidRAW).
    DevelopWith(tri::develop::Engine),
    /// Développer en fond toutes les images du dossier qui ont été développées, chacune
    /// dans son logiciel (issue n° 69).
    DevelopFolder,
    /// Afficher / masquer le panneau des dossiers.
    ToggleFolderPanel,
    /// Relire les volumes (disque branché) et les dossiers du panneau.
    RefreshFolders,
    /// Plein écran : afficher / masquer l'histogramme.
    ToggleHistogram,
    /// Histogramme : basculer entre IL et linéaire.
    HistogramScale,
    /// Grand histogramme, en boîte au milieu de l'écran.
    HistogramLarge,
    /// Grand histogramme : canal suivant (tous, rouge, vert, bleu).
    HistogramChannel,
    /// Grand histogramme ouvert : canal rouge seul.
    HistogramRed,
    /// Grand histogramme ouvert : canal vert seul.
    HistogramGreen,
    /// Grand histogramme ouvert : canal bleu seul.
    HistogramBlue,
    /// Grand histogramme ouvert : tous les canaux.
    HistogramAll,
    /// Grand histogramme : la luminance seule (28/10/2026).
    HistogramLuminance,
    /// Grand histogramme ouvert : afficher / masquer la photo d'analyse à sa droite.
    HistogramPhoto,
    /// Grand histogramme : courbe du JPEG boîtier d'un RAW, montrée / masquée (28/10/2026).
    HistogramCamera,
    /// Plein écran : afficher / masquer les zones saturées / bouchées.
    ToggleClipping,
    /// Afficher / masquer le panneau des métadonnées.
    ToggleMetadata,
    /// Ouvrir l'image sélectionnée en plein écran.
    Open,
    /// Plein écran : basculer entre ajusté à la fenêtre et 100 %.
    ToggleZoom,
    /// Plein écran : basculer entre JPEG du boîtier et rendu RAW.
    ToggleSource,
    /// Image zoomée : voir plus à gauche.
    PanLeft,
    /// Image zoomée : voir plus à droite.
    PanRight,
    /// Image zoomée : voir plus haut.
    PanUp,
    /// Image zoomée : voir plus bas.
    PanDown,
    /// Fermer ce qui est au premier plan (aide, plein écran...) / revenir en arrière.
    Back,
    /// Afficher / masquer l'aide des raccourcis.
    Help,
    /// Loupe à 100 % sous le pointeur, tant qu'on tient sa touche (`loupe` de `keys.toml`,
    /// `[gui.zoom] loupe_key`) : pour l'aide, la palette et l'aide « ici » seulement ;
    /// elle ne se lie pas dans `[keys]` ([`Action::bindable`]).
    Loupe,
    /// Passer en revue les développements (case de la section Vue), cochée ou non.
    ReviewDevelopments,
    /// Autoriser ou non les actions sur les fichiers (case « Actions sur les fichiers »).
    FileActionsAllow,
    /// Loupe : palier de grossissement suivant (la touche de loupe tenue, ou pour la
    /// prochaine fois).
    LoupeZoomIn,
    /// Loupe : palier précédent.
    LoupeZoomOut,
    /// Zoom du plein écran tapé en chiffres (`<leader> z 200`, `20`, `2`).
    ZoomScaleEntry,
    /// Loupe : grossissement tapé en chiffres (`<leader> m 200`, `20`, `2`).
    LoupeScaleEntry,
    /// Afficher / masquer l'aide « ici » : les raccourcis de l'endroit où l'on est.
    HelpHere,
    /// Palette de commandes : chercher une action ou un réglage et le lancer.
    Palette,
    /// Fenêtre de réglages.
    Settings,
    /// Rechercher une nouvelle version (sur demande, page Releases).
    CheckUpdate,
    /// Signaler un problème : formulaire de GitHub prérempli (issue n° 7), leader puis `b`.
    ReportIssue,
    /// Noter : 0 à 5 étoiles.
    Stars(u8),
    /// Ajouter ou retirer une couleur.
    Color(Color),
    /// Accepter, rejeter, ou retirer le drapeau.
    Flag(Flag),
    /// Quitter l'application.
    Quit,
}

impl Action {
    /// Toutes les actions, pour les listes (aide, fichier de raccourcis).
    pub const ALL: [Action; 229] = [
        Action::NavPrev,
        Action::NavNext,
        Action::NavUp,
        Action::NavDown,
        Action::NavFirst,
        Action::NavLast,
        Action::NavPageUp,
        Action::NavPageDown,
        Action::FilterCycle,
        Action::FilterStars(0),
        Action::FilterStars(1),
        Action::FilterStars(2),
        Action::FilterStars(3),
        Action::FilterStars(4),
        Action::FilterStars(5),
        Action::FilterStarsOp,
        Action::FilterFlag(FlagFilter::Accepted),
        Action::FilterFlag(FlagFilter::NotRejected),
        Action::FilterFlag(FlagFilter::Rejected),
        Action::FilterColor(Color::Red),
        Action::FilterColor(Color::Yellow),
        Action::FilterColor(Color::Green),
        Action::FilterColor(Color::Blue),
        Action::FilterColor(Color::Purple),
        Action::FilterClear,
        Action::FilterBlurry,
        Action::FilterWellExposed,
        Action::FilterStack,
        Action::FilterInvert,
        Action::FilterDeveloped,
        Action::SortNext,
        Action::SortReverse,
        Action::SortBy(tri::sort::SortKey::Name),
        Action::SortBy(tri::sort::SortKey::Captured),
        Action::SortBy(tri::sort::SortKey::Modified),
        Action::SortBy(tri::sort::SortKey::Stars),
        Action::SortBy(tri::sort::SortKey::Color),
        Action::SortBy(tri::sort::SortKey::Flag),
        Action::FilterKind(tri::filter::KindFilter::All),
        Action::FilterKind(tri::filter::KindFilter::RawOnly),
        Action::FilterKind(tri::filter::KindFilter::FlatOnly),
        Action::GridRenderNext,
        Action::Render(crate::grid_render::GridRender::Embedded),
        Action::Render(crate::grid_render::GridRender::Cover),
        Action::Render(crate::grid_render::GridRender::Engine(
            tri::develop::Engine::Darktable,
        )),
        Action::Render(crate::grid_render::GridRender::Engine(
            tri::develop::Engine::RapidRaw,
        )),
        Action::Render(crate::grid_render::GridRender::Engine(
            tri::develop::Engine::RawTherapee,
        )),
        Action::Render(crate::grid_render::GridRender::Engine(
            tri::develop::Engine::Art,
        )),
        Action::Render(crate::grid_render::GridRender::LibRaw),
        Action::ToggleFilterBar,
        Action::ToggleRatingBar,
        Action::ToggleImageInfo,
        Action::BackgroundNext,
        Action::ToggleGridInfo,
        Action::ToggleAutoAdvance,
        Action::SelectToggle,
        Action::SelectVisual,
        Action::Compare,
        Action::TogglePeaking,
        Action::PeakingColor,
        Action::SharpnessLenient,
        Action::SharpnessStrict,
        Action::ToggleFocusPoint,
        Action::OverlayNext,
        Action::OverlayOrient,
        Action::SelectAll,
        Action::SelectClear,
        Action::FileRename,
        Action::FileDelete,
        Action::FileReveal,
        Action::FileEdit,
        Action::Sidecars,
        Action::EmbedNotes,
        Action::FileCopy,
        Action::FileMove,
        Action::FileDeleteStack,
        Action::FileDeletePhoto,
        Action::FileCopyStack,
        Action::FileExport,
        Action::ExportReport,
        Action::Crop,
        Action::CropMove(Dir::Left),
        Action::CropMove(Dir::Right),
        Action::CropMove(Dir::Up),
        Action::CropMove(Dir::Down),
        Action::CropSize(Dir::Left),
        Action::CropSize(Dir::Right),
        Action::CropSize(Dir::Up),
        Action::CropSize(Dir::Down),
        Action::CropRatio(0),
        Action::CropRatio(1),
        Action::CropRatio(2),
        Action::CropRatio(3),
        Action::CropRatio(4),
        Action::CropRatio(5),
        Action::CropRatio(6),
        Action::CropRotate,
        Action::CropSave,
        Action::CropPreset,
        Action::CropPresetPrev,
        Action::VersionRatioNext,
        Action::VersionRatioPrev,
        Action::VersionRatioCustom,
        Action::VersionExportNext,
        Action::VersionExportPrev,
        Action::VersionName,
        Action::VersionSavePreset,
        Action::VersionReset,
        Action::VersionNext,
        Action::VersionPrev,
        Action::Send("plugin.airdrop.send"),
        Action::Undo,
        Action::Redo,
        Action::History,
        Action::StackToggle,
        Action::StackOpen,
        Action::StackClose,
        Action::StacksAll,
        Action::GroupingWindow,
        Action::StackMode,
        Action::DevToggle,
        Action::DevNext,
        Action::DevPrev,
        Action::DevFoldAll,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ZoomFit,
        Action::Zoom100,
        Action::StackChoose,
        Action::StackFirst,
        Action::StackLast,
        Action::StackEnterToggle,
        Action::FocusLeft,
        Action::FocusRight,
        Action::FocusNext,
        Action::FolderUp,
        Action::FolderDown,
        Action::FolderExpand,
        Action::FolderCollapse,
        Action::FolderOpen,
        Action::FolderFirst,
        Action::FolderLast,
        Action::FolderOffload,
        Action::FolderEject,
        Action::FolderCreate,
        Action::FolderRename,
        Action::FolderDelete,
        Action::FolderFavorite,
        Action::FolderFavoriteName,
        Action::FolderConnect,
        Action::FolderForget,
        Action::FolderOpenIn(tri::develop::Engine::Darktable),
        Action::FolderOpenIn(tri::develop::Engine::RawTherapee),
        Action::FolderOpenIn(tri::develop::Engine::Art),
        Action::FolderCopy,
        Action::FolderMove,
        Action::MetaUp,
        Action::MetaDown,
        Action::MetaCopy,
        Action::MetaOpen,
        Action::MetaSelectUp,
        Action::MetaSelectDown,
        Action::MetaCopyAll,
        Action::PanelSectionNext,
        Action::PanelSectionPrev,
        Action::PanelActivate,
        Action::PanelLeft,
        Action::PanelRight,
        Action::FolderSectionNext,
        Action::FolderSectionPrev,
        Action::Search,
        Action::FocusMode,
        Action::Develop,
        Action::DevelopWith(tri::develop::Engine::Darktable),
        Action::DevelopWith(tri::develop::Engine::RapidRaw),
        Action::DevelopWith(tri::develop::Engine::RawTherapee),
        Action::DevelopWith(tri::develop::Engine::Art),
        Action::DevelopFolder,
        Action::ToggleFolderPanel,
        Action::RefreshFolders,
        Action::ToggleHistogram,
        Action::HistogramScale,
        Action::HistogramLarge,
        Action::HistogramChannel,
        Action::HistogramRed,
        Action::HistogramGreen,
        Action::HistogramBlue,
        Action::HistogramAll,
        Action::HistogramLuminance,
        Action::HistogramPhoto,
        Action::HistogramCamera,
        Action::ToggleClipping,
        Action::ToggleMetadata,
        Action::Open,
        Action::ToggleZoom,
        Action::Loupe,
        Action::LoupeZoomIn,
        Action::LoupeZoomOut,
        Action::LoupeScaleEntry,
        Action::ZoomScaleEntry,
        Action::ToggleSource,
        Action::PanLeft,
        Action::PanRight,
        Action::PanUp,
        Action::PanDown,
        Action::Back,
        Action::Help,
        Action::HelpHere,
        Action::ReviewDevelopments,
        Action::FileActionsAllow,
        Action::Palette,
        Action::Settings,
        Action::CheckUpdate,
        Action::ReportIssue,
        Action::Quit,
        Action::Stars(0),
        Action::Stars(1),
        Action::Stars(2),
        Action::Stars(3),
        Action::Stars(4),
        Action::Stars(5),
        Action::Color(Color::Red),
        Action::Color(Color::Yellow),
        Action::Color(Color::Green),
        Action::Color(Color::Blue),
        Action::Color(Color::Purple),
        Action::Flag(Flag::Accepted),
        Action::Flag(Flag::Rejected),
        Action::Flag(Flag::None),
    ];

    /// Action correspondant à un identifiant (fichier de raccourcis, scripts de capture).
    pub fn from_id(id: &str) -> Option<Action> {
        let id = renamed_id(id);
        Action::ALL.into_iter().find(|a| a.id() == id)
    }

    /// Faux pour la loupe : sa touche se règle à part (`loupe` de `keys.toml`,
    /// `[gui.zoom] loupe_key`), elle n'a pas de ligne dans `[keys]` ni dans `keys.toml`.
    pub fn bindable(self) -> bool {
        self != Action::Loupe
    }

    /// Plugin d'envoi d'une action [`Action::Send`] (`airdrop` pour
    /// `plugin.airdrop.send`).
    pub fn sender(self) -> Option<Box<dyn tri::send::Sender>> {
        let Action::Send(id) = self else { return None };
        let name = id.strip_prefix("plugin.")?.strip_suffix(".send")?;
        tri::send::sender(name)
    }

    /// Vrai pour les actions qui renomment, copient, déplacent ou suppriment des images ou
    /// des dossiers : interdites sans `[files] allow_actions` (30/10/2026).
    pub fn changes_files(self) -> bool {
        use Action::*;
        matches!(
            self,
            FileRename
                | FileDelete
                | FileCopy
                | FileMove
                | FileDeleteStack
                | FileDeletePhoto
                | FileCopyStack
                | FolderRename
                | FolderDelete
                | FolderCopy
                | FolderMove
        )
    }

    /// Faux pour l'action d'un plugin indisponible sur cette machine (AirDrop hors
    /// macOS) : ni dans l'aide, ni dans les réglages, ni en bouton.
    pub fn available(self) -> bool {
        match self {
            Action::Send(_) => self.sender().is_some_and(|s| s.available()),
            _ => true,
        }
    }

    /// Libellé compréhensible (aide, bulles, boutons), dans la langue de l'interface :
    /// identifiant `action-<identifiant de l'action>` des catalogues (`gui/locales`).
    pub fn label(self) -> String {
        let id = format!("action-{}", self.id().replace('.', "-"));
        t!(&id, os = crate::i18n::os())
    }

    /// Textes où la recherche de l'aide cherche, **dans toutes les langues** (retour du
    /// 06/11/2026 : on cherche souvent avec le mot anglais, « crop », « peaking »,
    /// « stack ») : libellé, mots-clés cachés, thème et mots-clés du thème
    /// (`kw-group-<thème>` : « noter » trouve les étoiles).
    pub fn search_texts(self) -> Vec<String> {
        let label_id = format!("action-{}", self.id().replace('.', "-"));
        let keywords_id = format!("kw-{}", self.id().replace(['.', '_'], "-"));
        let group_id = format!("group-{:?}", self.group()).to_lowercase();
        let group_keywords = format!("kw-{group_id}");
        let mut args = tri::i18n::Args::new();
        args.set("os", tri::i18n::value(crate::i18n::os()));
        let mut texts = Vec::new();
        for language in Language::ALL {
            for id in [label_id.as_str(), &keywords_id, &group_id, &group_keywords]
                .into_iter()
                .chain(self.interface_labels().iter().copied())
            {
                if tri::i18n::has(language, id) {
                    texts.push(tri::i18n::translate_in(language, id, Some(&args)));
                }
            }
        }
        texts
    }

    /// Libellés des contrôles de l'interface qui font cette action (case « Empiler
    /// automatiquement » des piles…) : la recherche les trouve sous ces mots aussi (retour
    /// du 06/11/2026, la palette ne trouvait pas « Empiler automatiquement »).
    pub fn interface_labels(self) -> &'static [&'static str] {
        use Action::*;
        match self {
            StackMode => &["panel-grid-stacks-on", "panel-grid-stacks"],
            StacksAll => &["panel-grid-unfold-all", "panel-grid-fold-all"],
            GroupingWindow => &["panel-grid-grouping", "panel-grid-similarity"],
            GridRenderNext | Render(_) => &["panel-grid-render"],
            SortNext | SortBy(_) => &["panel-grid-sort"],
            ToggleSource => &["panel-view-source"],
            ReviewDevelopments => &["panel-view-review", "panel-view-developments"],
            FileActionsAllow => &["panel-grid-files-allow", "panel-grid-files"],
            FilterBlurry => &["panel-filter-sharp"],
            FilterWellExposed => &["panel-filter-exposed"],
            FilterDeveloped => &["panel-filter-developed"],
            FilterCycle | FilterKind(_) => &["filter-bar-raw-tip", "filter-bar-flat-tip"],
            // « Recadrer » : l'ancien nom de la nouvelle version (07/11/2026), trouvé en tête.
            Crop => &["version-crop-alias"],
            // En grille, * met toutes les vignettes à l'écran ; leur taille est un contrôle
            // à part dans la palette (07/11/2026).
            Zoom100 => &["grid-fit"],
            _ => &[],
        }
    }

    /// Thème de l'action (aide).
    pub fn group(self) -> Group {
        use Action::*;
        match self {
            NavPrev | NavNext | NavUp | NavDown | NavFirst | NavLast | NavPageUp | NavPageDown
            | Open | Back | Search => Group::Navigation,
            ToggleZoom | Loupe | LoupeZoomIn | LoupeZoomOut | LoupeScaleEntry | ZoomScaleEntry
            | ReviewDevelopments | ZoomIn | ZoomOut | ZoomFit | Zoom100 | ToggleSource
            | PanLeft | PanRight | PanUp | PanDown | ToggleHistogram | HistogramScale
            | HistogramLarge | HistogramChannel | HistogramRed | HistogramGreen | HistogramBlue
            | HistogramAll | HistogramLuminance | HistogramPhoto | HistogramCamera
            | ToggleClipping | TogglePeaking | PeakingColor | SharpnessLenient
            | SharpnessStrict | ToggleFocusPoint | OverlayNext | OverlayOrient | FocusMode
            | Develop | DevelopWith(_) | DevelopFolder => Group::View,
            Crop | CropMove(_) | CropSize(_) | CropRatio(_) | CropRotate | CropSave
            | CropPreset | CropPresetPrev | VersionNext | VersionPrev | VersionRatioNext
            | VersionRatioPrev | VersionRatioCustom | VersionExportNext | VersionExportPrev
            | VersionName | VersionSavePreset | VersionReset => Group::Crop,
            Stars(_) | Color(_) | Flag(_) | ToggleAutoAdvance => Group::Rating,
            FilterCycle | FilterStars(_) | FilterStarsOp | FilterFlag(_) | FilterColor(_)
            | FilterClear | FilterBlurry | FilterWellExposed | FilterStack | FilterInvert
            | FilterDeveloped | SortNext | SortReverse | SortBy(_) | FilterKind(_)
            | GridRenderNext | Render(_) => Group::Filters,
            ToggleFilterBar | ToggleRatingBar | ToggleImageInfo | BackgroundNext
            | ToggleGridInfo | ToggleFolderPanel | ToggleMetadata | RefreshFolders => Group::Panels,
            FileActionsAllow | FileRename | FileDelete | FileReveal | FileEdit | FileCopy
            | FileMove | Sidecars | EmbedNotes | FileDeleteStack | FileDeletePhoto
            | FileCopyStack | FileExport | ExportReport | Send(_) => Group::Files,
            Undo | Redo | History => Group::Edit,
            StackToggle | StackOpen | StackClose | StacksAll | GroupingWindow | StackMode
            | StackChoose | StackFirst | StackLast | StackEnterToggle | DevToggle | DevNext
            | DevPrev | DevFoldAll => Group::Selection,
            FocusLeft | FocusRight | FocusNext | FolderUp | FolderDown | FolderExpand
            | FolderCollapse | FolderOpen | FolderFirst | FolderLast | FolderOffload
            | FolderEject | FolderCreate | FolderRename | FolderDelete | FolderFavorite
            | FolderFavoriteName | FolderConnect | FolderForget | FolderOpenIn(_) | FolderCopy
            | FolderMove | MetaUp | MetaDown | MetaCopy | MetaOpen | MetaSelectUp
            | MetaSelectDown | MetaCopyAll | PanelSectionNext | PanelSectionPrev
            | PanelActivate | PanelLeft | PanelRight | FolderSectionNext | FolderSectionPrev => {
                Group::Panels
            }
            SelectToggle | SelectVisual | SelectAll | SelectClear | Compare => Group::Selection,
            Help | HelpHere | Palette | Settings | CheckUpdate | ReportIssue | Quit => Group::App,
        }
    }

    /// Vrai si une touche tenue répète l'action : déplacements, zoom, défilement. Les
    /// autres ne partent qu'une fois par appui (#160 : `e` tenu ouvrait l'éditeur en
    /// boucle).
    pub fn repeats(self) -> bool {
        use Action::*;
        matches!(
            self,
            NavPrev
                | NavNext
                | NavUp
                | NavDown
                | NavPageUp
                | NavPageDown
                | PanLeft
                | PanRight
                | PanUp
                | PanDown
                | ZoomIn
                | ZoomOut
                | FolderUp
                | FolderDown
                | MetaUp
                | MetaDown
                | MetaSelectUp
                | MetaSelectDown
                | PanelLeft
                | PanelRight
                | CropMove(_)
                | CropSize(_)
                | SharpnessLenient
                | SharpnessStrict
        )
    }

    /// Vrai pour les actions encore actives quand une fenêtre (l'aide) est au premier plan.
    pub fn works_over_overlay(self) -> bool {
        matches!(
            self,
            Action::Back | Action::Help | Action::HelpHere | Action::Quit
        )
    }

    /// Identifiant stable, utilisé par le fichier de raccourcis.
    pub fn id(self) -> &'static str {
        match self {
            Action::NavPrev => "core.nav.prev",
            Action::NavNext => "core.nav.next",
            Action::NavUp => "core.nav.up",
            Action::NavDown => "core.nav.down",
            Action::NavFirst => "core.nav.first",
            Action::NavLast => "core.nav.last",
            Action::NavPageUp => "core.nav.page_up",
            Action::NavPageDown => "core.nav.page_down",
            Action::FilterCycle => "core.filter.cycle",
            Action::FilterStars(0) => "core.filter.stars.0",
            Action::FilterStars(1) => "core.filter.stars.1",
            Action::FilterStars(2) => "core.filter.stars.2",
            Action::FilterStars(3) => "core.filter.stars.3",
            Action::FilterStars(4) => "core.filter.stars.4",
            Action::FilterStars(_) => "core.filter.stars.5",
            Action::FilterStarsOp => "core.filter.stars.op",
            Action::FilterFlag(FlagFilter::Accepted) => "core.filter.flag.accepted",
            Action::FilterFlag(FlagFilter::NotRejected) => "core.filter.flag.not_rejected",
            Action::FilterFlag(FlagFilter::Rejected) => "core.filter.flag.rejected",
            Action::FilterColor(Color::Red) => "core.filter.color.red",
            Action::FilterColor(Color::Yellow) => "core.filter.color.yellow",
            Action::FilterColor(Color::Green) => "core.filter.color.green",
            Action::FilterColor(Color::Blue) => "core.filter.color.blue",
            Action::FilterColor(Color::Purple) => "core.filter.color.purple",
            Action::FilterClear => "core.filter.clear",
            Action::FilterBlurry => "core.filter.blurry",
            Action::FilterWellExposed => "core.filter.well_exposed",
            Action::FilterStack => "core.filter.stack",
            Action::FilterInvert => "core.filter.invert",
            Action::FilterDeveloped => "core.filter.developed",
            Action::SortNext => "core.filter.sort",
            Action::GridRenderNext => "core.grid.render",
            Action::Render(r) => {
                use crate::grid_render::GridRender;
                use tri::develop::Engine;
                match r {
                    GridRender::Embedded => "core.view.render.embedded",
                    GridRender::Cover => "core.view.render.cover",
                    GridRender::Engine(Engine::Darktable) => "core.view.render.darktable",
                    GridRender::Engine(Engine::RapidRaw) => "core.view.render.rapidraw",
                    GridRender::Engine(Engine::RawTherapee) => "core.view.render.rawtherapee",
                    GridRender::Engine(Engine::Art) => "core.view.render.art",
                    GridRender::LibRaw => "core.view.render.libraw",
                }
            }
            Action::SortReverse => "core.filter.sort_reverse",
            Action::SortBy(tri::sort::SortKey::Name) => "core.filter.sort_by.name",
            Action::SortBy(tri::sort::SortKey::Captured) => "core.filter.sort_by.captured",
            Action::SortBy(tri::sort::SortKey::Modified) => "core.filter.sort_by.modified",
            Action::SortBy(tri::sort::SortKey::Stars) => "core.filter.sort_by.stars",
            Action::SortBy(tri::sort::SortKey::Color) => "core.filter.sort_by.color",
            Action::SortBy(tri::sort::SortKey::Flag) => "core.filter.sort_by.flag",
            Action::FilterKind(tri::filter::KindFilter::All) => "core.filter.kind.all",
            Action::FilterKind(tri::filter::KindFilter::RawOnly) => "core.filter.kind.raw",
            Action::FilterKind(tri::filter::KindFilter::FlatOnly) => "core.filter.kind.flat",
            Action::ToggleFilterBar => "core.view.filter_bar",
            Action::ToggleRatingBar => "core.view.rating_bar",
            Action::ToggleImageInfo => "core.view.image_info",
            Action::BackgroundNext => "core.view.background",
            Action::ToggleGridInfo => "core.view.grid_info",
            Action::ToggleAutoAdvance => "core.rate.auto_advance",
            Action::Compare => "core.view.compare",
            Action::TogglePeaking => "core.view.peaking",
            Action::PeakingColor => "core.view.peaking_color",
            Action::SharpnessLenient => "core.view.sharpness_lenient",
            Action::SharpnessStrict => "core.view.sharpness_strict",
            Action::ToggleFocusPoint => "core.view.focus_point",
            Action::OverlayNext => "core.view.overlay",
            Action::OverlayOrient => "core.view.overlay_orient",
            Action::SelectToggle => "core.select.toggle",
            Action::SelectVisual => "core.select.visual",
            Action::SelectAll => "core.select.all",
            Action::SelectClear => "core.select.clear",
            Action::FileRename => "core.file.rename",
            Action::FileDelete => "core.file.delete",
            Action::FileCopy => "core.file.copy",
            Action::FileExport => "core.file.export",
            Action::ExportReport => "core.file.export.report",
            Action::Crop => "core.version.new",
            Action::CropMove(Dir::Left) => "core.version.move.left",
            Action::CropMove(Dir::Right) => "core.version.move.right",
            Action::CropMove(Dir::Up) => "core.version.move.up",
            Action::CropMove(Dir::Down) => "core.version.move.down",
            Action::CropSize(Dir::Left) => "core.version.size.left",
            Action::CropSize(Dir::Right) => "core.version.size.right",
            Action::CropSize(Dir::Up) => "core.version.size.up",
            Action::CropSize(Dir::Down) => "core.version.size.down",
            Action::CropRatio(0) => "core.version.ratio.free",
            Action::CropRatio(1) => "core.version.ratio.original",
            Action::CropRatio(2) => "core.version.ratio.square",
            Action::CropRatio(3) => "core.version.ratio.5_4",
            Action::CropRatio(4) => "core.version.ratio.3_2",
            Action::CropRatio(5) => "core.version.ratio.16_9",
            Action::CropRatio(_) => "core.version.ratio.4_3",
            Action::CropRotate => "core.version.rotate",
            Action::CropSave => "core.version.save",
            Action::CropPreset => "core.version.preset",
            Action::CropPresetPrev => "core.version.preset_prev",
            Action::VersionRatioNext => "core.version.ratio_next",
            Action::VersionRatioPrev => "core.version.ratio_prev",
            Action::VersionRatioCustom => "core.version.ratio_custom",
            Action::VersionExportNext => "core.version.export_next",
            Action::VersionExportPrev => "core.version.export_prev",
            Action::VersionName => "core.version.name",
            Action::VersionSavePreset => "core.version.save_preset",
            Action::VersionReset => "core.version.reset",
            Action::VersionNext => "core.version.next",
            Action::VersionPrev => "core.version.prev",
            Action::Undo => "core.edit.undo",
            Action::Redo => "core.edit.redo",
            Action::History => "core.edit.history",
            Action::StackToggle => "core.stack.toggle",
            Action::StackOpen => "core.stack.open",
            Action::StackClose => "core.stack.close",
            Action::StacksAll => "core.stack.all",
            Action::GroupingWindow => "core.stack.settings",
            Action::StackMode => "core.stack.mode",
            Action::DevToggle => "core.dev.toggle",
            Action::DevNext => "core.dev.next",
            Action::DevFoldAll => "core.dev.fold_all",
            Action::DevPrev => "core.dev.prev",
            Action::ZoomIn => "core.view.zoom_in",
            Action::ZoomOut => "core.view.zoom_out",
            Action::ZoomFit => "core.view.zoom_fit",
            Action::Zoom100 => "core.view.zoom_100",
            Action::StackChoose => "core.stack.choose",
            Action::StackFirst => "core.stack.first",
            Action::StackLast => "core.stack.last",
            Action::StackEnterToggle => "core.stack.enter",
            Action::FocusLeft => "core.panel.left",
            Action::FocusRight => "core.panel.right",
            Action::FocusNext => "core.panel.next",
            Action::FolderUp => "core.folders.up",
            Action::FolderDown => "core.folders.down",
            Action::FolderExpand => "core.folders.expand",
            Action::FolderCollapse => "core.folders.collapse",
            Action::FolderOpen => "core.folders.open",
            Action::FolderFirst => "core.folders.first",
            Action::FolderLast => "core.folders.last",
            Action::FolderOffload => "core.folders.offload",
            Action::FolderEject => "core.folders.eject",
            Action::FolderCreate => "core.folders.create",
            Action::FolderRename => "core.folders.rename",
            Action::FolderDelete => "core.folders.delete",
            Action::FolderFavorite => "core.folders.favorite",
            Action::FolderFavoriteName => "core.folders.favorite_name",
            Action::FolderConnect => "core.folders.connect",
            Action::FolderForget => "core.folders.forget",
            Action::FolderOpenIn(tri::develop::Engine::Darktable) => "plugin.darktable.open_folder",
            Action::FolderOpenIn(tri::develop::Engine::RapidRaw) => "plugin.rapidraw.open_folder",
            Action::FolderOpenIn(tri::develop::Engine::RawTherapee) => {
                "plugin.rawtherapee.open_folder"
            }
            Action::FolderOpenIn(tri::develop::Engine::Art) => "plugin.art.open_folder",
            Action::FolderCopy => "core.folders.copy",
            Action::FolderMove => "core.folders.move",
            Action::MetaUp => "core.metadata.up",
            Action::MetaDown => "core.metadata.down",
            Action::MetaCopy => "core.metadata.copy",
            Action::MetaOpen => "core.metadata.open",
            Action::MetaSelectUp => "core.metadata.select_up",
            Action::MetaSelectDown => "core.metadata.select_down",
            Action::MetaCopyAll => "core.metadata.copy_all",
            Action::PanelSectionNext => "core.metadata.section_next",
            Action::PanelSectionPrev => "core.metadata.section_prev",
            Action::PanelActivate => "core.metadata.activate",
            Action::PanelLeft => "core.metadata.left",
            Action::PanelRight => "core.metadata.right",
            Action::FolderSectionNext => "core.folders.section_next",
            Action::FolderSectionPrev => "core.folders.section_prev",
            Action::FileMove => "core.file.move",
            Action::FileDeleteStack => "core.file.delete_stack",
            Action::FileDeletePhoto => "core.file.delete_photo",
            Action::FileCopyStack => "core.file.copy_stack",
            Action::Send(id) => id,
            Action::FileReveal => "core.file.reveal",
            Action::FileEdit => "core.file.edit",
            Action::Sidecars => "core.file.sidecars",
            Action::EmbedNotes => "core.file.embed_notes",
            Action::ToggleFolderPanel => "core.view.folders",
            Action::Search => "core.search",
            Action::FocusMode => "core.view.focus_mode",
            Action::Develop => "core.develop.render",
            Action::DevelopWith(tri::develop::Engine::Darktable) => "plugin.darktable.develop",
            Action::DevelopWith(tri::develop::Engine::RapidRaw) => "plugin.rapidraw.develop",
            Action::DevelopWith(tri::develop::Engine::RawTherapee) => "plugin.rawtherapee.develop",
            Action::DevelopWith(tri::develop::Engine::Art) => "plugin.art.develop",
            Action::DevelopFolder => "core.develop.folder",
            Action::RefreshFolders => "core.view.refresh_folders",
            Action::ToggleHistogram => "core.view.histogram",
            Action::HistogramScale => "core.view.histogram_scale",
            Action::HistogramLarge => "core.view.histogram_large",
            Action::HistogramChannel => "core.view.histogram_channel",
            Action::HistogramRed => "core.histogram.red",
            Action::HistogramGreen => "core.histogram.green",
            Action::HistogramBlue => "core.histogram.blue",
            Action::HistogramAll => "core.histogram.all",
            Action::HistogramLuminance => "core.histogram.luminance",
            Action::HistogramPhoto => "core.histogram.photo",
            Action::HistogramCamera => "core.histogram.camera",
            Action::ToggleClipping => "core.view.clipping",
            Action::ToggleMetadata => "core.view.metadata",
            Action::Open => "core.view.open",
            Action::ToggleZoom => "core.view.zoom",
            Action::ToggleSource => "core.view.source",
            Action::PanLeft => "core.view.pan_left",
            Action::PanRight => "core.view.pan_right",
            Action::PanUp => "core.view.pan_up",
            Action::PanDown => "core.view.pan_down",
            Action::Back => "core.view.back",
            Action::Help => "core.app.help",
            Action::HelpHere => "core.app.help_here",
            Action::Loupe => "core.view.loupe",
            Action::LoupeZoomIn => "core.view.loupe_zoom_in",
            Action::LoupeZoomOut => "core.view.loupe_zoom_out",
            Action::LoupeScaleEntry => "core.view.loupe_scale",
            Action::ZoomScaleEntry => "core.view.zoom_scale",
            Action::ReviewDevelopments => "core.view.review_developments",
            Action::FileActionsAllow => "core.file.allow_actions",
            Action::Palette => "core.app.palette",
            Action::Settings => "core.app.settings",
            Action::CheckUpdate => "core.app.check_update",
            Action::ReportIssue => "core.app.report_issue",
            Action::Quit => "core.app.quit",
            Action::Stars(0) => "core.rate.star.0",
            Action::Stars(1) => "core.rate.star.1",
            Action::Stars(2) => "core.rate.star.2",
            Action::Stars(3) => "core.rate.star.3",
            Action::Stars(4) => "core.rate.star.4",
            Action::Stars(_) => "core.rate.star.5",
            Action::Color(Color::Red) => "core.rate.color.red",
            Action::Color(Color::Yellow) => "core.rate.color.yellow",
            Action::Color(Color::Green) => "core.rate.color.green",
            Action::Color(Color::Blue) => "core.rate.color.blue",
            Action::Color(Color::Purple) => "core.rate.color.purple",
            Action::Flag(Flag::Accepted) => "core.flag.accept",
            Action::Flag(Flag::Rejected) => "core.flag.reject",
            Action::Flag(Flag::None) => "core.flag.none",
        }
    }
}

/// Ce qui déclenche un raccourci : une touche, ou un caractère tapé. Les caractères servent
/// aux touches qu'egui ne nomme pas (« ° » de l'AZERTY) ; une touche nommée par egui est
/// toujours une [`Trigger::Key`], pour ne pas réagir deux fois à la même frappe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Touche.
    Key(Key),
    /// Caractère tapé.
    Char(char),
}

impl Trigger {
    /// Vrai pour les touches qui ne tapent aucun texte (Échap, F1 à F12) : elles restent
    /// des raccourcis même pendant la saisie dans un champ.
    pub fn types_no_text(self) -> bool {
        use Key::*;
        matches!(
            self,
            Trigger::Key(Escape | F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12)
        )
    }
}

/// Une combinaison : une touche avec ou sans Maj et Ctrl, ou un caractère. Cmd et Alt sont
/// réservés au système et ne déclenchent rien ici ; Ctrl + flèches aussi, par convention
/// (sur macOS, cela change de bureau).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    /// Touche ou caractère.
    pub trigger: Trigger,
    /// Maj enfoncée (toujours faux pour un caractère : Maj fait partie du caractère).
    pub shift: bool,
    /// Ctrl enfoncée.
    pub ctrl: bool,
}

/// Panneau qui a le clavier (façon tmux, demandé le 18/10/2026) : ses raccourcis passent
/// d'abord ; une touche qu'il n'utilise pas garde son effet habituel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    /// Les images (grille, plein écran, comparaison).
    #[default]
    Images,
    /// Le panneau des dossiers.
    Folders,
    /// Le panneau des métadonnées.
    Metadata,
    /// Le grand histogramme (boîte ouverte).
    Histogram,
    /// Le recadrage (plein écran).
    Crop,
}

impl Action {
    /// Panneau où l'action vaut ; `None` : partout.
    pub fn scope(self) -> Option<Scope> {
        use Action::*;
        match self {
            FolderUp | FolderDown | FolderExpand | FolderCollapse | FolderOpen | FolderFirst
            | FolderLast | FolderOffload | FolderEject | FolderCreate | FolderRename
            | FolderDelete | FolderFavorite | FolderFavoriteName | FolderConnect | FolderForget
            | FolderOpenIn(_) | FolderCopy | FolderMove | FolderSectionNext | FolderSectionPrev => {
                Some(Scope::Folders)
            }
            MetaUp | MetaDown | MetaCopy | MetaOpen | MetaSelectUp | MetaSelectDown
            | MetaCopyAll | PanelSectionNext | PanelSectionPrev | PanelActivate | PanelLeft
            | PanelRight => Some(Scope::Metadata),
            CropMove(_) | CropSize(_) | CropRatio(_) | CropRotate | CropSave | CropPreset
            | CropPresetPrev | VersionRatioNext | VersionRatioPrev | VersionRatioCustom
            | VersionExportNext | VersionExportPrev | VersionName | VersionSavePreset
            | VersionReset => Some(Scope::Crop),
            // Échelle et canal suivant : `s` et `c` dans le grand histogramme (retour du
            // 06/11/2026), ailleurs `<leader> i s` / `<leader> i c`.
            HistogramRed | HistogramGreen | HistogramBlue | HistogramAll | HistogramLuminance
            | HistogramPhoto | HistogramCamera | HistogramScale | HistogramChannel => {
                Some(Scope::Histogram)
            }
            _ => None,
        }
    }
}

/// Deux actions se disputent-elles vraiment une touche (pas une action de panneau qui
/// prend le pas sur une action générale, voulu) ?
fn conflict(a: Action, b: Action) -> bool {
    a.scope() == b.scope()
}

impl Chord {
    /// Combinaison d'une touche pressée, `None` si Cmd ou Alt est enfoncé. Sous Linux et
    /// Windows, egui signale Ctrl aussi comme `command` : seul `mac_cmd`, ou `command` sans
    /// Ctrl, désigne Cmd.
    pub fn from_event(key: Key, modifiers: Modifiers) -> Option<Chord> {
        // Exceptions (décidées le 18/10/2026) : Cmd+Z / Cmd+Maj+Z / Cmd+C sur macOS,
        // réflexes de tout utilisateur de Mac, valent Ctrl+Z / Ctrl+Maj+Z / Ctrl+C
        // (annuler, rétablir, copier).
        if cfg!(target_os = "macos")
            && modifiers.mac_cmd
            && !modifiers.alt
            && matches!(key, Key::Z | Key::C)
        {
            return Some(Chord {
                trigger: Trigger::Key(key),
                shift: modifiers.shift,
                ctrl: true,
            });
        }
        if modifiers.alt || modifiers.mac_cmd || (modifiers.command && !modifiers.ctrl) {
            return None;
        }
        // Un modificateur seul n'est pas un raccourci (07/11/2026 : sous Linux, egui-winit
        // envoie aussi Maj et Ctrl comme touches ; Maj, tapé avant un chiffre de la rangée
        // du haut d'un clavier AZERTY, fermait la saisie de `<leader> m` et le chiffre
        // changeait la note). macOS ne les envoie pas.
        if matches!(
            key,
            Key::ShiftLeft
                | Key::ShiftRight
                | Key::ControlLeft
                | Key::ControlRight
                | Key::AltLeft
                | Key::AltRight
                | Key::SuperLeft
                | Key::SuperRight
        ) {
            return None;
        }
        Some(Chord {
            trigger: Trigger::Key(key),
            shift: modifiers.shift,
            ctrl: modifiers.ctrl,
        })
    }

    /// Combinaison d'une touche seule, sans modificateur.
    pub const fn key(key: Key) -> Chord {
        plain(key)
    }

    /// Combinaison d'un caractère tapé.
    pub const fn char(c: char) -> Chord {
        Chord {
            trigger: Trigger::Char(c),
            shift: false,
            ctrl: false,
        }
    }
}

const fn plain(key: Key) -> Chord {
    Chord {
        trigger: Trigger::Key(key),
        shift: false,
        ctrl: false,
    }
}

#[cfg(test)]
const fn shifted(key: Key) -> Chord {
    Chord {
        trigger: Trigger::Key(key),
        shift: true,
        ctrl: false,
    }
}

#[cfg(test)]
const fn ctrl(key: Key) -> Chord {
    Chord {
        trigger: Trigger::Key(key),
        shift: false,
        ctrl: true,
    }
}

/// Jeu de raccourcis, au choix (`[gui] profile`, décidé le 18/10/2026 : un utilisateur
/// d'outils classiques n'a que faire des touches de vim, et inversement ; un nouveau
/// raccourci ne s'écrit qu'une fois s'il est le même pour les deux, dans les listes
/// communes). Dans le fichier de configuration : `"classic"`, `"vim"` ou `"both"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    /// Flèches, Début / Fin, Page préc. / suiv., Maj + flèches dans une image zoomée,
    /// Suppr, `Ctrl+C` / `Ctrl+X` copier / déplacer vers, `Ctrl+Z` / `Ctrl+Y`.
    #[default]
    Classic,
    /// hjkl, gg / G, Ctrl+U / Ctrl+D, Maj + hjkl dans une image zoomée, `x`, `y`, `dw` /
    /// `yw`, `u` / `Ctrl+R`, `j` / `k` dans les panneaux.
    Vim,
    /// Les deux ensemble (ils ne se recouvrent pas).
    Both,
}

/// Événements qu'egui-winit livre pour la touche `key` tenue avec `modifiers` (pour les
/// tests : `egui_kittest` envoie toujours la touche elle-même). Comme `egui-winit` 0.37
/// (`is_cut_command`…) : Ctrl / Cmd seuls + X, C, V deviennent couper, copier, coller,
/// sans la touche ; sous Windows aussi Maj+Suppr, Ctrl+Inser, Maj+Inser. Coller suppose un
/// presse-papiers non vide (vide, egui-winit n'envoie rien).
#[cfg(test)]
pub(crate) fn winit_events(key: Key, modifiers: Modifiers, windows: bool) -> Vec<Event> {
    let command_only = modifiers.command && !modifiers.alt && !modifiers.shift;
    let event = if (command_only && key == Key::X)
        || (windows && modifiers.shift && key == Key::Delete)
    {
        Event::Cut
    } else if (command_only && key == Key::C) || (windows && modifiers.ctrl && key == Key::Insert) {
        Event::Copy
    } else if (command_only && key == Key::V) || (windows && modifiers.shift && key == Key::Insert)
    {
        Event::Paste("texte".into())
    } else {
        return [true, false]
            .map(|pressed| Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            })
            .to_vec();
    };
    vec![event]
}

/// Touche d'un événement du presse-papiers d'egui-winit (voir
/// [`Keymap::chords_repeated`]) : Maj tenue, celle de Windows (Maj+Suppr pour couper,
/// Maj+Inser pour coller) ; sinon X, C ou V avec Ctrl / Cmd.
fn clipboard_key(event: &Event, modifiers: Modifiers) -> Option<Key> {
    let shift = modifiers.shift && !modifiers.command;
    Some(match event {
        Event::Cut if shift => Key::Delete,
        Event::Cut => Key::X,
        Event::Copy if modifiers.command => Key::C,
        Event::Copy => Key::Insert,
        Event::Paste(_) if shift => Key::Insert,
        Event::Paste(_) => Key::V,
        _ => return None,
    })
}

/// Raccourcis par défaut, lus dans `keys.toml` (issue n° 280 : plus aucun raccourci écrit
/// dans le code). Le fichier est livré à côté du programme ([`resource_file`]) pour
/// pouvoir changer les défauts sans recompiler ; une copie est intégrée au programme, en
/// secours.
#[derive(Clone, Debug, Default)]
pub struct Defaults {
    /// Communs aux deux profils.
    common: Vec<(Vec<Chord>, Action)>,
    /// Profil classique.
    classic: Vec<(Vec<Chord>, Action)>,
    /// Profil vim.
    vim: Vec<(Vec<Chord>, Action)>,
    /// Touches tapées après le préfixe des panneaux.
    leader: Vec<(Vec<Chord>, Action)>,
    /// Préfixe des panneaux (`leader`, en tête du fichier), tel qu'écrit : « Ctrl+b ».
    prefix: String,
    /// Touche de loupe (`loupe`, en tête du fichier).
    loupe: crate::settings::LoupeKey,
    /// Noms des sous-menus du menu des suites (`[menus]`) : après le leader ou non, début
    /// de séquence, nom.
    menus: Vec<(bool, Vec<Chord>, String)>,
}

/// Copie de `gui/resources/keys.toml` intégrée au programme.
const EMBEDDED_KEYS: &str = include_str!("../resources/keys.toml");

/// Nom du fichier des raccourcis par défaut.
const KEYS_FILE: &str = "keys.toml";

/// Raccourcis par défaut chargés (une fois, au premier usage) et les problèmes à
/// signaler.
static DEFAULTS: std::sync::OnceLock<(Defaults, Vec<String>)> = std::sync::OnceLock::new();

impl Defaults {
    /// Lit `text` (syntaxe de `keys.toml`) ; renvoie aussi les problèmes : action ou
    /// touche inconnue (l'entrée est ignorée), section inconnue. `Err` si le fichier
    /// n'est pas du TOML lisible.
    pub fn parse(text: &str) -> Result<(Defaults, Vec<String>), String> {
        let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut defaults = Defaults::default();
        let mut warnings = Vec::new();
        for (section, entries) in &table {
            // La touche de loupe, en tête du fichier.
            if section == "loupe"
                && let Some(loupe) = entries.as_str()
            {
                match crate::settings::LoupeKey::parse(loupe) {
                    Ok(key) => defaults.loupe = key,
                    Err(e) => {
                        warnings.push(t!("keys-file-error", section = "", id = "loupe", error = e))
                    }
                }
                continue;
            }
            // Le préfixe des panneaux, en tête du fichier.
            if section == "leader"
                && let Some(prefix) = entries.as_str()
            {
                match parse_chord(prefix) {
                    Ok(_) => defaults.prefix = prefix.to_string(),
                    Err(e) => warnings.push(t!(
                        "keys-file-error",
                        section = "",
                        id = "leader",
                        error = e
                    )),
                }
                continue;
            }
            // Noms des sous-menus du menu des suites.
            if section == "menus"
                && let Some(entries) = entries.as_table()
            {
                for (spec, name) in entries {
                    let (lead, rest) = match spec.trim().strip_prefix("<leader>") {
                        Some(rest) => (true, rest.trim()),
                        None => (false, spec.trim()),
                    };
                    let keys = if rest.is_empty() {
                        Ok(Vec::new())
                    } else {
                        parse_keys(rest)
                    };
                    match (keys, name.as_str()) {
                        (Ok(keys), Some(name)) => {
                            defaults.menus.push((lead, keys, name.to_string()))
                        }
                        (Err(e), _) => warnings.push(t!(
                            "keys-file-error",
                            section = "menus",
                            id = spec.as_str(),
                            error = e
                        )),
                        (Ok(_), None) => warnings.push(t!("keys-file-section", section = "menus")),
                    }
                }
                continue;
            }
            let list = match section.as_str() {
                "common" => &mut defaults.common,
                "classic" => &mut defaults.classic,
                "vim" => &mut defaults.vim,
                "after_leader" => &mut defaults.leader,
                _ => {
                    warnings.push(t!("keys-file-section", section = section.as_str()));
                    continue;
                }
            };
            let Some(entries) = entries.as_table() else {
                warnings.push(t!("keys-file-section", section = section.as_str()));
                continue;
            };
            // Dans l'ordre du fichier : à touches égales, la première l'emporte.
            for (id, specs) in entries {
                let Some(action) = Action::from_id(id).filter(|a| a.bindable()) else {
                    let error = t!("keys-file-unknown-action");
                    warnings.push(t!(
                        "keys-file-error",
                        section = section.as_str(),
                        id = id.as_str(),
                        error = error
                    ));
                    continue;
                };
                let specs: Vec<&str> = match specs.as_array() {
                    Some(a) => a.iter().filter_map(|v| v.as_str()).collect(),
                    None => specs.as_str().into_iter().collect(),
                };
                for spec in specs {
                    match parse_keys(spec) {
                        Ok(keys) => list.push((keys, action)),
                        Err(e) => warnings.push(t!(
                            "keys-file-error",
                            section = section.as_str(),
                            id = id.as_str(),
                            error = e
                        )),
                    }
                }
            }
        }
        Ok((defaults, warnings))
    }

    /// Les raccourcis par défaut : `keys.toml` à côté du programme s'il y en a un, sinon
    /// la copie intégrée (aussi si le fichier est illisible : le problème est signalé).
    fn load() -> (Defaults, Vec<String>) {
        let embedded = || Defaults::parse(EMBEDDED_KEYS).expect("keys.toml intégré valide");
        // Les tests ne dépendent pas d'un fichier posé à côté de leur exécutable.
        if cfg!(test) {
            return embedded();
        }
        let Some(path) = resource_file() else {
            return embedded();
        };
        let read = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| Defaults::parse(&text));
        match read {
            Ok(loaded) => loaded,
            Err(e) => {
                let (defaults, mut warnings) = embedded();
                warnings.insert(
                    0,
                    t!(
                        "keys-file-unreadable",
                        path = path.display().to_string(),
                        error = e
                    ),
                );
                (defaults, warnings)
            }
        }
    }

    /// Les raccourcis par défaut chargés (au premier appel).
    pub fn get() -> &'static Defaults {
        &DEFAULTS.get_or_init(Defaults::load).0
    }

    /// Préfixe des panneaux par défaut (« Ctrl+b »).
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Touche de loupe par défaut (« m »).
    pub fn loupe(&self) -> crate::settings::LoupeKey {
        self.loupe
    }

    /// Actions liées après le préfixe des panneaux (`[after_leader]`).
    pub fn leader_actions(&self) -> impl Iterator<Item = Action> + '_ {
        self.leader.iter().map(|(_, a)| *a)
    }

    /// Problèmes rencontrés en chargeant les raccourcis par défaut.
    pub fn warnings() -> &'static [String] {
        &DEFAULTS.get_or_init(Defaults::load).1
    }
}

/// `keys.toml` livré avec le programme, s'il existe : à côté de l'exécutable (Windows,
/// archive), dans `../Resources` (paquet macOS `tri.app`), ou `../share/tri` (Linux :
/// `/usr/bin` → `/usr/share/tri`).
pub fn resource_file() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [
        dir.join(KEYS_FILE),
        dir.join("../Resources").join(KEYS_FILE),
        dir.join("../share/tri").join(KEYS_FILE),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

impl Profile {
    /// Raccourcis propres au profil (section `[classic]` ou `[vim]` de `keys.toml`), sans
    /// les communs.
    fn own(self) -> &'static [(Vec<Chord>, Action)] {
        let d = Defaults::get();
        match self {
            Profile::Vim => &d.vim,
            Profile::Classic | Profile::Both => &d.classic,
        }
    }

    /// Raccourcis du profil, notation et filtres compris (`keys.toml`).
    pub fn defaults(self) -> impl Iterator<Item = &'static (Vec<Chord>, Action)> {
        let d = Defaults::get();
        let own: Vec<&'static Vec<(Vec<Chord>, Action)>> = match self {
            Profile::Vim => vec![&d.vim],
            // « both » n'existe plus (revue des raccourcis du 06/11/2026) : classique.
            Profile::Classic | Profile::Both => vec![&d.classic],
        };
        own.into_iter()
            .chain([&d.common])
            .flat_map(|list| list.iter())
    }
}

/// Table raccourci → action.
#[derive(Clone, Debug)]
pub struct Keymap {
    bindings: Vec<(Vec<Chord>, Action)>,
    /// Préfixe des panneaux ([`Keymap::with_leader`]) : l'aide écrit `<leader>` à sa place.
    leader: Option<Chord>,
    /// Panneau qui a le clavier ([`Keymap::set_focus`]).
    focus: Scope,
    /// Touche de loupe en vigueur (`keys.toml`, sinon `[gui.zoom] loupe_key`), refusée si
    /// elle est déjà liée à une action ([`Keymap::with_loupe`]).
    loupe: crate::settings::LoupeKey,
    /// Noms des sous-menus du menu des suites (`[menus]` de `keys.toml`), le leader en
    /// tête de ceux qui le suivent.
    menus: Vec<(Vec<Chord>, String)>,
    /// Raccourcis propres au jeu choisi (`[classic]` ou `[vim]`) et ceux de `[keys]` :
    /// l'aide ne montre qu'eux pour une action qui en a ([`Keymap::keys_for`]).
    own: Vec<(Vec<Chord>, Action)>,
    /// Jeu vim choisi : l'aide écrit `/` seul pour la recherche ([`Keymap::keys_for`]).
    vim: bool,
}

/// Résultat d'une touche passée à [`Keymap::lookup`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lookup {
    /// La séquence correspond exactement à un raccourci.
    Action(Action),
    /// La séquence est un raccourci, et le début d'un plus long (`y` et `y w`) : attendre
    /// un court instant la suite, sinon agir seule.
    Ambiguous(Action),
    /// La séquence est le début d'un raccourci plus long : attendre la suite.
    Prefix,
    /// Aucun raccourci ne commence ainsi.
    None,
}

impl Keymap {
    /// Union de plusieurs profils (le premier l'emporte en cas de conflit).
    pub fn from_profiles(profiles: &[Profile]) -> Keymap {
        let mut bindings: Vec<(Vec<Chord>, Action)> = Vec::new();
        for profile in profiles {
            for (keys, action) in profile.defaults() {
                if !bindings
                    .iter()
                    .any(|(k, a)| k == keys && conflict(*a, *action))
                {
                    bindings.push((keys.clone(), *action));
                }
            }
        }
        let menus = Defaults::get()
            .menus
            .iter()
            .filter(|(lead, _, _)| !lead)
            .map(|(_, keys, name)| (keys.clone(), name.clone()))
            .collect();
        let own = profiles
            .iter()
            .flat_map(|p| p.own().iter().cloned())
            .collect();
        Keymap {
            bindings,
            leader: None,
            focus: Scope::Images,
            loupe: crate::settings::LoupeKey::None,
            menus,
            own,
            vim: profiles.contains(&Profile::Vim),
        }
    }

    /// Action d'une touche seule (sans séquence en cours), `None` si aucune ou si Ctrl, Cmd
    /// ou Alt est enfoncé. L'application passe par [`Sequencer`] ; sert aux tests.
    #[cfg(test)]
    pub fn action(&self, key: Key, modifiers: Modifiers) -> Option<Action> {
        match self.lookup(&[Chord::from_event(key, modifiers)?]) {
            Lookup::Action(a) | Lookup::Ambiguous(a) => Some(a),
            Lookup::Prefix | Lookup::None => None,
        }
    }

    /// Applique la section `[keys]` du fichier de configuration : pour chaque action citée
    /// (par son identifiant), ses raccourcis **remplacent** ceux des profils (liste vide :
    /// plus aucun). Renvoie aussi les problèmes à signaler : action ou touche inconnue,
    /// raccourci attribué à deux actions (le premier dans l'ordre du fichier, trié par
    /// identifiant, est gardé ; un raccourci du fichier l'emporte sur celui d'un profil),
    /// raccourci qui ne peut pas se déclencher car il commence une séquence plus longue.
    pub fn with_overrides(
        mut self,
        overrides: &BTreeMap<String, Vec<String>>,
    ) -> (Keymap, Vec<String>) {
        let mut warnings = Vec::new();
        let mut custom: Vec<(Vec<Chord>, Action, &str)> = Vec::new();
        let mut replaced: Vec<Action> = Vec::new();
        for (id, specs) in overrides {
            let Some(action) = Action::from_id(id).filter(|a| a.bindable()) else {
                warnings.push(format!("[keys] : action inconnue « {id} »"));
                continue;
            };
            replaced.push(action);
            for spec in specs {
                match parse_keys(spec) {
                    Ok(keys) => custom.push((keys, action, spec)),
                    Err(e) => warnings.push(t!("keys-error", id = id, error = e)),
                }
            }
        }
        self.bindings.retain(|(_, a)| !replaced.contains(a));
        let mut added: Vec<(Vec<Chord>, Action)> = Vec::new();
        for (keys, action, spec) in custom {
            if let Some((_, other)) = added
                .iter()
                .find(|(k, a)| *k == keys && conflict(*a, action))
            {
                if *other != action {
                    warnings.push(t!(
                        "keys-conflict",
                        spec = spec,
                        first = other.id(),
                        second = action.id()
                    ));
                }
                continue;
            }
            if let Some(i) = self
                .bindings
                .iter()
                .position(|(k, a)| *k == keys && conflict(*a, action))
            {
                warnings.push(t!(
                    "keys-replaces",
                    spec = spec,
                    action = action.id(),
                    other = self.bindings[i].1.id()
                ));
                self.bindings.remove(i);
            }
            added.push((keys, action));
        }
        self.own.retain(|(_, a)| !replaced.contains(a));
        self.own.extend(added.iter().cloned());
        self.bindings.extend(added);
        // Une touche qui commence aussi une séquence (`y`, `y w`) n'est plus un problème :
        // elle attend un court instant la suite (voir [`Sequencer`]).
        (self, warnings)
    }

    /// Combinaisons tapées dans les événements d'une image d'egui, dans l'ordre.
    ///
    /// Seuls les caractères liés à un raccourci (« ° ») sont gardés : les autres
    /// accompagnent une touche déjà traitée, et interrompraient une séquence. Une touche
    /// suivie d'un tel caractère est ignorée : « ° » arrive aussi comme Maj + `Minus`
    /// (touche physique, egui ne connaissant pas « ° »), qui interromprait `f °`. egui-winit
    /// livre la touche et son caractère dans la même image (même événement winit).
    ///
    /// Chacune avec vrai si elle vient de la répétition d'une touche tenue (un caractère
    /// suit la touche qui le produit).
    ///
    /// egui-winit remplace les raccourcis du presse-papiers par `Cut`, `Copy` et `Paste`,
    /// sans la touche : Ctrl / Cmd + X, C, V, et sous Windows Maj+Suppr, Ctrl+Inser,
    /// Maj+Inser (issue n° 194 : Maj+Suppr ne supprimait pas la pile). Ils sont retraduits
    /// en touches d'après `modifiers`, les modificateurs tenus.
    pub fn chords_repeated(&self, events: &[Event], modifiers: Modifiers) -> Vec<(Chord, bool)> {
        let bound_char = |e: &Event| match e {
            Event::Text(text) => {
                let mut chars = text.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) if self.uses_char(c) => Some(c),
                    _ => None,
                }
            }
            _ => None,
        };
        // Ce qui suit la touche `n` : premier texte ou appui de touche (en sautant
        // relâchements et changements de modificateurs).
        let next_is_bound_char = |n: usize| {
            events[n + 1..]
                .iter()
                .find(|e| matches!(e, Event::Text(_) | Event::Key { pressed: true, .. }))
                .and_then(bound_char)
                .is_some()
        };
        let mut repeated = false;
        events
            .iter()
            .enumerate()
            .filter_map(|(n, e)| match e {
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    repeat,
                    ..
                } => {
                    repeated = *repeat;
                    if next_is_bound_char(n) {
                        return None;
                    }
                    Chord::from_event(*key, *modifiers).map(|c| (c, *repeat))
                }
                Event::Text(_) => bound_char(e).map(|c| (Chord::char(c), repeated)),
                Event::Cut | Event::Copy | Event::Paste(_) => clipboard_key(e, modifiers)
                    .and_then(|k| Chord::from_event(k, modifiers))
                    .map(|c| (c, false)),
                _ => None,
            })
            .collect()
    }

    /// Vrai si le caractère `c` sert dans un raccourci : seuls ceux-là sont passés au
    /// [`Sequencer`] (les autres caractères tapés interrompraient une séquence).
    pub fn uses_char(&self, c: char) -> bool {
        self.bindings
            .iter()
            .any(|(k, _)| k.contains(&Chord::char(c)))
    }

    /// Donne le clavier au panneau `focus` : ses raccourcis l'emportent, les actions
    /// d'un autre panneau sont ignorées.
    pub fn set_focus(&mut self, focus: Scope) {
        self.focus = focus;
    }

    /// Vrai si `key` seule (sans modificateur) déclenche une action ou commence une
    /// séquence, dans le panneau des images.
    pub fn binds(&self, key: Key) -> bool {
        let mut images = self.clone();
        images.focus = Scope::Images;
        images.lookup(&[plain(key)]) != Lookup::None
    }

    fn lookup(&self, keys: &[Chord]) -> Lookup {
        // Une séquence du leader atteint les actions de tous les panneaux (`<leader> i r` :
        // un canal de l'histogramme, sans l'ouvrir en grand).
        let by_leader = |k: &[Chord]| k.len() > 1 && Some(k[0]) == self.leader;
        let usable =
            |k: &[Chord], a: &Action| a.scope().is_none_or(|s| s == self.focus) || by_leader(k);
        // Une touche du panneau actif l'emporte sur une action générale, qui l'emporte sur
        // celle d'un autre panneau (par le leader : `<leader> r d` développe dans darktable
        // depuis les images, ouvre le dossier dans darktable depuis les dossiers).
        let rank = |a: &Action| match a.scope() {
            Some(s) if s == self.focus => 2,
            None => 1,
            Some(_) => 0,
        };
        let exact = self
            .bindings
            .iter()
            .filter(|(k, a)| k == keys && usable(k, a))
            .max_by_key(|(_, a)| rank(a))
            .map(|&(_, a)| a);
        let longer = |same_scope: Option<Action>| {
            self.bindings.iter().any(|(k, a)| {
                k.len() > keys.len()
                    && k.starts_with(keys)
                    && usable(k, a)
                    && same_scope.is_none_or(|s| s.scope() == a.scope())
            })
        };
        // Une touche du panneau ne cède qu'à une séquence du même panneau.
        if let Some(action) = exact.filter(|a| a.scope().is_some_and(|s| s == self.focus)) {
            return if longer(Some(action)) {
                Lookup::Ambiguous(action)
            } else {
                Lookup::Action(action)
            };
        }
        match (exact, longer(None)) {
            (Some(action), true) => Lookup::Ambiguous(action),
            (Some(action), false) => Lookup::Action(action),
            (None, true) => Lookup::Prefix,
            (None, false) => Lookup::None,
        }
    }

    /// Ajoute les raccourcis après le leader `leader` (« Ctrl+b » par défaut, comme tmux) :
    /// section `[after_leader]` de `keys.toml` (panneaux, développer, réglages
    /// secondaires).
    pub fn with_leader(mut self, leader: &str) -> Result<Keymap, String> {
        let lead = parse_chord(leader)?;
        self.leader = Some(lead);
        // Touches suivantes : section `[after_leader]` de `keys.toml`.
        for (keys, action) in &Defaults::get().leader {
            let mut sequence = vec![lead];
            sequence.extend(keys.iter().copied());
            self.bindings.push((sequence, *action));
        }
        for (_, keys, name) in Defaults::get().menus.iter().filter(|(l, _, _)| *l) {
            let mut sequence = vec![lead];
            sequence.extend(keys.iter().copied());
            self.menus.push((sequence, name.clone()));
        }
        Ok(self)
    }

    /// Première action (hors `ignore`) dont un raccourci commence par `chord` : un leader
    /// sur cette touche lui volerait son raccourci (refusé, revue du 06/11/2026).
    pub fn first_use(&self, chord: Chord, ignore: &[Action]) -> Option<Action> {
        self.bindings
            .iter()
            .find(|(k, a)| k.first() == Some(&chord) && !ignore.contains(a))
            .map(|(_, a)| *a)
    }

    /// Fixe la touche de loupe en vigueur.
    pub fn with_loupe(mut self, loupe: crate::settings::LoupeKey) -> Keymap {
        self.loupe = loupe;
        self
    }

    /// Touche de loupe en vigueur.
    pub fn loupe(&self) -> crate::settings::LoupeKey {
        self.loupe
    }

    /// Touches d'une action utilisables pour faire défiler un texte pendant qu'un champ a
    /// le clavier (fenêtre d'aide) : une seule combinaison, qui ne tape pas de texte
    /// (avec Ctrl, ou flèches, pages, Début / Fin). Avec ses modificateurs egui.
    pub fn scroll_keys(&self, action: Action) -> Vec<(Key, Modifiers)> {
        self.bindings
            .iter()
            .filter(|(_, a)| *a == action)
            .filter_map(|(keys, _)| match keys.as_slice() {
                [chord] => match chord.trigger {
                    Trigger::Key(key)
                        if chord.ctrl
                            || matches!(
                                key,
                                Key::ArrowUp
                                    | Key::ArrowDown
                                    | Key::PageUp
                                    | Key::PageDown
                                    | Key::Home
                                    | Key::End
                            ) =>
                    {
                        let modifiers = Modifiers {
                            shift: chord.shift,
                            ctrl: chord.ctrl,
                            ..Modifiers::NONE
                        };
                        Some((key, modifiers))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    /// Raccourcis d'une action tels qu'on les écrit dans `[keys]` (« Maj+P », « g g »),
    /// relus par [`parse_keys`] : pour la fenêtre de réglages.
    pub fn specs_for(&self, action: Action) -> Vec<String> {
        let mut specs: Vec<String> = Vec::new();
        for (keys, _) in self.bindings.iter().filter(|(_, a)| *a == action) {
            let spec = keys_spec(keys);
            if !specs.contains(&spec) {
                specs.push(spec);
            }
        }
        specs
    }

    /// Raccourcis associés à une action, lisibles (« Flèche gauche », « Maj+g », « gg »...),
    /// pour l'aide : ceux du jeu choisi s'il en a, sinon les communs. Une touche liée avec et sans Maj n'apparaît qu'une fois (« 3 », pas
    /// « 3, Maj+3 »). Une séquence qui commence par le leader l'écrit `<leader>` (« <leader>
    /// r », #175) : il est réglable, l'aide le montre à part.
    pub fn keys_for(&self, action: Action) -> Vec<String> {
        // La loupe : sa touche, maintenue.
        if action == Action::Loupe {
            use crate::settings::LoupeKey;
            return match self.loupe {
                LoupeKey::Key(key) => vec![key_label(key)],
                LoupeKey::Shift => vec![t!("key-shift-prefix").trim_end_matches('+').to_string()],
                LoupeKey::Ctrl => vec!["Ctrl".to_string()],
                LoupeKey::Alt => vec!["Alt".to_string()],
                LoupeKey::None | LoupeKey::Default => Vec::new(),
            };
        }
        // Celles du jeu choisi (ou de `[keys]`) seulement, s'il y en a : en vim, « x »
        // supprime, sans Suppr (retour du 06/11/2026 : plus lisible) ; sinon les communes.
        let own = |k: &Vec<Chord>| self.own.iter().any(|(o, a)| o == k && *a == action);
        let has_own = self.bindings.iter().any(|(k, a)| *a == action && own(k));
        let mut mine: Vec<&Vec<Chord>> = self
            .bindings
            .iter()
            .filter(|(k, a)| *a == action && (!has_own || own(k)))
            .map(|(k, _)| k)
            .collect();
        // Flèches et hjkl pour la même action (`<leader> h` / `<leader> Flèche gauche`) :
        // celles du jeu choisi seulement (retour du 06/11/2026).
        if mine.iter().any(|k| !self.other_set_variant(k)) {
            mine.retain(|k| !self.other_set_variant(k));
        }
        let mut keys: Vec<String> = Vec::new();
        // En grille, « ajusté » (`/`) lance la recherche (`TriApp::perform`) : l'aide
        // l'écrit pour la recherche, seul en vim, comme dans vim (06/11/2026).
        if action == Action::Search {
            let custom = self.own.iter().any(|(_, a)| *a == Action::Search);
            let fit: Vec<String> = self
                .bindings
                .iter()
                .filter(|(k, a)| *a == Action::ZoomFit && k.len() == 1)
                .map(|(k, _)| self.sequence_label(k))
                .collect();
            if self.vim && !custom && !fit.is_empty() {
                return fit;
            }
            keys.extend(fit);
        }
        for k in &mine {
            let unshifted: Vec<Chord> = k.iter().map(|c| Chord { shift: false, ..*c }).collect();
            let shift_variant = k.iter().any(|c| c.shift) && mine.contains(&&unshifted);
            let label = self.sequence_label(k);
            if !shift_variant && !keys.contains(&label) {
                keys.push(label);
            }
        }
        keys
    }
}

/// Une suite possible d'un début de séquence, pour le menu des suites (façon which-key de
/// nvim) : la touche, et l'action qu'elle fait ou le sous-menu qu'elle ouvre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Next {
    /// Touche suivante.
    pub chord: Chord,
    /// Son nom lisible (« r », « Maj+H », « Espace »).
    pub key: String,
    /// Action faite par cette touche (rien : seulement un sous-menu).
    pub action: Option<Action>,
    /// D'autres touches peuvent suivre : nom du sous-menu (`[menus]` de `keys.toml`,
    /// traduit), sinon le thème le plus fréquent de ses actions (« Histogramme »).
    pub menu: Option<String>,
}

impl Keymap {
    /// Suites possibles du début de séquence `prefix`, dans le panneau actif, telles que
    /// [`Sequencer::feed`] les traiterait (même classement des panneaux que `lookup`).
    /// Une touche liée avec et sans Maj à la même action n'apparaît qu'une fois. Dans
    /// l'ordre de `keys.toml`, les sous-menus à la fin.
    pub fn continuations(&self, prefix: &[Chord]) -> Vec<Next> {
        let mut chords: Vec<Chord> = Vec::new();
        for (k, _) in &self.bindings {
            if k.len() > prefix.len() && k.starts_with(prefix) && !chords.contains(&k[prefix.len()])
            {
                chords.push(k[prefix.len()]);
            }
        }
        let mut next: Vec<Next> = Vec::new();
        for chord in chords {
            let mut keys = prefix.to_vec();
            keys.push(chord);
            let (action, more) = match self.lookup(&keys) {
                Lookup::Action(a) => (Some(a), false),
                Lookup::Ambiguous(a) => (Some(a), true),
                Lookup::Prefix => (None, true),
                Lookup::None => continue,
            };
            if action.is_some_and(|a| !a.available()) {
                continue;
            }
            let menu = more
                .then(|| {
                    self.menu_name(&keys)
                        .or_else(|| self.menu_group(&keys).map(Group::label))
                })
                .flatten();
            if action.is_none() && menu.is_none() {
                continue;
            }
            next.push(Next {
                chord,
                key: chord_label(&chord),
                action,
                menu,
            });
        }
        // Flèche et lettre hjkl pour la même action : celle du jeu choisi (retour du
        // 06/11/2026 : « h / Flèche gauche » en vim).
        let kept: Vec<Option<Action>> = next
            .iter()
            .filter(|n| !self.other_set_variant(&[n.chord]))
            .map(|n| n.action)
            .collect();
        next.retain(|n| {
            n.action.is_none() || !self.other_set_variant(&[n.chord]) || !kept.contains(&n.action)
        });
        // Maj + touche qui fait la même chose que la touche seule : en double.
        let plain: Vec<(Chord, Option<Action>)> = next
            .iter()
            .filter(|n| !n.chord.shift)
            .map(|n| (n.chord, n.action))
            .collect();
        next.retain(|n| {
            !n.chord.shift
                || n.action.is_none()
                || !plain.contains(&(
                    Chord {
                        shift: false,
                        ..n.chord
                    },
                    n.action,
                ))
        });
        // Dans l'ordre de `keys.toml` (retour du 06/11/2026 : par touche, « ° , . » du
        // violet passaient avant les étoiles, loin des autres couleurs), les sous-menus à
        // la fin.
        let order = |chord: Chord| {
            self.bindings
                .iter()
                .position(|(k, _)| {
                    k.len() > prefix.len() && k.starts_with(prefix) && k[prefix.len()] == chord
                })
                .unwrap_or(usize::MAX)
        };
        next.sort_by_key(|n| (n.action.is_none(), order(n.chord)));
        next
    }

    /// Vrai si la séquence `keys` est la variante de l'autre jeu : des flèches en vim,
    /// `hjkl` en classique. L'aide la tait quand l'action en a une du jeu choisi.
    fn other_set_variant(&self, keys: &[Chord]) -> bool {
        keys.iter().any(|c| match c.trigger {
            Trigger::Key(Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown) => {
                self.vim
            }
            Trigger::Key(Key::H | Key::J | Key::K | Key::L) if !c.ctrl => !self.vim,
            _ => false,
        })
    }

    /// Nom donné au sous-menu `keys` par `[menus]` : `key-submenu-<nom>` des catalogues s'il
    /// existe, sinon le nom tel quel (un texte de l'utilisateur).
    fn menu_name(&self, keys: &[Chord]) -> Option<String> {
        let (_, name) = self.menus.iter().find(|(k, _)| k == keys)?;
        let id = format!("key-submenu-{name}");
        Some(if tri::i18n::has(tri::i18n::language(), &id) {
            tri::i18n::translate(&id, None)
        } else {
            name.clone()
        })
    }

    /// Thème le plus fréquent des actions disponibles qui commencent par `keys` (plus
    /// longues), dans le panneau actif.
    fn menu_group(&self, keys: &[Chord]) -> Option<Group> {
        let mut counts: Vec<(Group, usize)> = Vec::new();
        for (k, a) in &self.bindings {
            if k.len() > keys.len() && k.starts_with(keys) && a.available() {
                let found =
                    matches!(self.lookup(k), Lookup::Action(x) | Lookup::Ambiguous(x) if x == *a);
                if !found {
                    continue;
                }
                match counts.iter_mut().find(|(g, _)| *g == a.group()) {
                    Some((_, n)) => *n += 1,
                    None => counts.push((a.group(), 1)),
                }
            }
        }
        // À égalité, le premier rencontré (ordre de `keys.toml`).
        let best = counts.iter().map(|(_, n)| *n).max()?;
        counts.into_iter().find(|(_, n)| *n == best).map(|(g, _)| g)
    }

    /// Titre du menu des suites : le début de séquence, le leader écrit `<leader>`.
    pub fn prefix_label(&self, prefix: &[Chord]) -> String {
        match prefix.split_first() {
            Some((first, rest)) if Some(*first) == self.leader => {
                let mut label = t!("key-leader");
                for c in rest {
                    label.push(' ');
                    label.push_str(&chord_label(c));
                }
                label
            }
            _ => prefix.iter().map(chord_label).collect::<Vec<_>>().join(" "),
        }
    }
}

impl Keymap {
    /// Actions liées à la touche que décrit `query` quand elle se lit comme une touche
    /// avec modificateurs (« ctrl f », « Ctrl+F », « maj p ») : un de leurs raccourcis est
    /// cette touche seule, avec au moins ces modificateurs (« ctrl f » : Ctrl+f et
    /// Ctrl+Maj+f).
    /// `None` si `query` ne se lit pas ainsi, ou si aucune action n'y est liée : la
    /// recherche porte alors sur les mots (retour du 06/11/2026 : le « f » de « ctrl f »,
    /// trouvé tel quel dans presque tous les libellés, remontait des actions sans rapport).
    pub fn actions_for_typed_keys(&self, query: &str) -> Option<Vec<Action>> {
        let shift_words: Vec<String> = Language::ALL
            .iter()
            .map(|&l| {
                crate::i18n::ensure();
                tri::i18n::translate_in(l, "key-shift-prefix", None)
                    .trim_end_matches('+')
                    .to_lowercase()
            })
            .chain(["shift".to_string()])
            .collect();
        let (mut ctrl, mut shift, mut key) = (false, false, None);
        for token in query.split(|c: char| c.is_whitespace() || c == '+') {
            let lower = token.to_lowercase();
            if token.is_empty() {
                continue;
            } else if matches!(lower.as_str(), "ctrl" | "control" | "ctl") {
                ctrl = true;
            } else if shift_words.contains(&lower) {
                shift = true;
            } else if key.is_none() {
                key = Some(parse_chord(&lower).ok()?);
            } else {
                return None;
            }
        }
        let key = key?;
        if !ctrl && !shift {
            return None;
        }
        let mut found: Vec<Action> = Vec::new();
        for (keys, action) in &self.bindings {
            // Un raccourci d'une seule touche (`f Maj+p` est un filtre, pas Maj+p).
            let hit = matches!(keys.as_slice(), [c]
                if c.trigger == key.trigger && (!ctrl || c.ctrl) && (!shift || c.shift));
            if hit && action.available() && !found.contains(action) {
                found.push(*action);
            }
        }
        (!found.is_empty()).then_some(found)
    }

    /// Nom lisible d'une séquence, le leader écrit `<leader>` (« <leader> r », #175).
    fn sequence_label(&self, keys: &[Chord]) -> String {
        match keys.split_first() {
            Some((first, rest)) if !rest.is_empty() && Some(*first) == self.leader => {
                format!("{} {}", t!("key-leader"), sequence_label(rest))
            }
            _ => sequence_label(keys),
        }
    }

    /// Nom lisible du leader en vigueur (« Ctrl+b »), vide s'il n'y en a pas.
    pub fn leader_label(&self) -> String {
        self.leader.as_ref().map(chord_label).unwrap_or_default()
    }

    /// Nom lisible d'un raccourci écrit dans `[keys]` (« gg » pour « g g »), le leader
    /// écrit `<leader>` (fenêtre de réglages) ; illisible : tel quel.
    pub fn spec_label(&self, spec: &str) -> String {
        parse_keys(spec).map_or_else(|_| spec.to_string(), |keys| self.sequence_label(&keys))
    }
}

/// Touche désignée par `name` : nom affiché par l'aide (« Flèche gauche », « Échap »,
/// « g »), ou nom d'egui (« ArrowLeft », « Escape », « PageUp », « 1 »...), sans tenir
/// compte de la casse.
pub fn key_named(name: &str) -> Option<Key> {
    let lower = name.to_lowercase();
    // Noms de toutes les langues : une configuration reste valable si la langue change.
    Key::ALL
        .iter()
        .copied()
        .find(|&k| {
            Language::ALL
                .iter()
                .any(|&l| key_label_in(l, k).to_lowercase() == lower)
        })
        .or_else(|| Key::from_name(name))
        .or_else(|| {
            Key::ALL
                .iter()
                .copied()
                .find(|k| k.name().to_lowercase() == lower)
        })
}

/// Retire le préfixe `prefix` (« Maj+ ») de `s`, sans tenir compte de la casse.
fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    // Un « + » seul est une touche, pas un modificateur vide.
    (s.len() > prefix.len() && s.get(..prefix.len())?.eq_ignore_ascii_case(prefix))
        .then(|| &s[prefix.len()..])
}

/// Une combinaison écrite dans le fichier de configuration : « Maj+P », « Ctrl+u »,
/// « Flèche gauche », « PageUp », « ° ». Une majuscule seule vaut Maj + la lettre (« G »,
/// comme dans vim). Un caractère qui n'est pas une touche connue d'egui devient un
/// [`Trigger::Char`] (sans modificateur).
fn parse_chord(text: &str) -> Result<Chord, String> {
    let (mut rest, mut shift, mut ctrl) = (text, false, false);
    // « Maj+ », « Shift+ » : préfixes de toutes les langues.
    let shift_prefixes: Vec<String> = Language::ALL
        .iter()
        .map(|&l| {
            crate::i18n::ensure();
            tri::i18n::translate_in(l, "key-shift-prefix", None)
        })
        .chain(["Shift+".to_string()])
        .collect();
    loop {
        if let Some(r) = shift_prefixes
            .iter()
            .find_map(|p| strip_prefix_ignore_case(rest, p))
        {
            (rest, shift) = (r, true);
        } else if let Some(r) = strip_prefix_ignore_case(rest, "Ctrl+") {
            (rest, ctrl) = (r, true);
        } else {
            break;
        }
    }
    let mut chars = rest.chars();
    let single = match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    };
    if single.is_some_and(|c| c.is_ascii_uppercase()) {
        shift = true;
    }
    // « + - * / ( ) = » : des caractères, pas des touches (le zoom, les piles) ; la touche
    // dépend de la disposition (« + » demande Maj sur un AZERTY, « ( » sur un QWERTY), le
    // caractère est le même partout, pavé numérique compris.
    if let Some(c) = single.filter(|c| "+-*/()=:".contains(*c))
        && !shift
        && !ctrl
    {
        return Ok(Chord::char(c));
    }
    match key_named(rest) {
        Some(key) => Ok(Chord {
            trigger: Trigger::Key(key),
            shift,
            ctrl,
        }),
        None => match single {
            Some(c) if !shift && !ctrl => Ok(Chord::char(c)),
            _ => Err(t!("keys-unknown-key", text = text)),
        },
    }
}

/// Un raccourci écrit dans le fichier de configuration : une combinaison, ou une séquence
/// séparée par des espaces (« g g »). Les noms en deux mots (« Flèche gauche ») sont
/// reconnus.
pub fn parse_keys(text: &str) -> Result<Vec<Chord>, String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Err(t!("keys-empty"));
    }
    let mut chords = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if i + 1 < words.len()
            && let Ok(chord) = parse_chord(&format!("{} {}", words[i], words[i + 1]))
        {
            chords.push(chord);
            i += 2;
            continue;
        }
        chords.push(parse_chord(words[i])?);
        i += 1;
    }
    Ok(chords)
}

/// Une séquence telle qu'on l'écrit dans `[keys]` : ses combinaisons séparées par des
/// espaces (« g g », « Ctrl+b h »).
pub fn keys_spec(keys: &[Chord]) -> String {
    // Comme le libellé, mais Maj toujours écrit (« Maj+? » : le libellé l'omet).
    let spec = |c: &Chord| match c.trigger {
        Trigger::Key(Key::Questionmark) if c.shift => {
            let unshifted = Chord { shift: false, ..*c };
            t!("key-shift-prefix") + &chord_label(&unshifted)
        }
        _ => chord_label(c),
    };
    keys.iter().map(spec).collect::<Vec<_>>().join(" ")
}

/// Le chiffre d'une touche (rangée du haut, avec ou sans Maj selon le clavier, ou pavé
/// numérique), sans Ctrl ; `None` pour les autres.
pub fn digit(chord: Chord) -> Option<char> {
    if chord.ctrl {
        return None;
    }
    match chord.trigger {
        Trigger::Key(key) => {
            let name = key.name();
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_digit() => Some(c),
                _ => None,
            }
        }
        Trigger::Char(c) if c.is_ascii_digit() => Some(c),
        Trigger::Char(_) => None,
    }
}

/// Nom lisible d'une combinaison (« Maj+Flèche gauche », « Ctrl+u », « g », « ° »).
pub(crate) fn chord_label(c: &Chord) -> String {
    let name = match c.trigger {
        Trigger::Key(key) => key_label(key),
        Trigger::Char(ch) => ch.to_string(),
    };
    let mut label = String::new();
    if c.ctrl {
        label.push_str("Ctrl+");
    }
    if c.shift && c.trigger != Trigger::Key(Key::Questionmark) {
        label.push_str(&t!("key-shift-prefix"));
    }
    label + &name
}

/// Nom lisible d'une séquence : « gg » comme on la tape quand ce ne sont que des lettres,
/// sinon « Échap puis g ».
fn sequence_label(keys: &[Chord]) -> String {
    let labels: Vec<String> = keys.iter().map(chord_label).collect();
    if labels.iter().all(|l| l.chars().count() == 1) {
        labels.concat()
    } else {
        labels.join(&t!("key-sequence-then"))
    }
}

/// Nom lisible d'une touche, dans la langue de l'interface, pour l'affichage. Les flèches
/// en symbole (←, →, ↑, ↓) quand une police du système les a ([`crate::fonts`] ; la
/// police d'egui n'en a pas), sinon en toutes lettres. Les noms écrits restent ceux de
/// [`key_label_in`] (lus dans `[keys]` dans toutes les langues).
pub fn key_label(key: Key) -> String {
    if crate::fonts::arrows()
        && let Some(arrow) = crate::fonts::arrow(key)
    {
        return arrow.to_string();
    }
    key_label_in(tri::i18n::language(), key)
}

/// [`key_label`] dans la langue `language`.
fn key_label_in(language: Language, key: Key) -> String {
    let id = match key {
        Key::ArrowLeft => Some("key-arrow-left"),
        Key::ArrowRight => Some("key-arrow-right"),
        Key::ArrowUp => Some("key-arrow-up"),
        Key::ArrowDown => Some("key-arrow-down"),
        Key::Home => Some("key-home"),
        Key::End => Some("key-end"),
        Key::PageUp => Some("key-page-up"),
        Key::PageDown => Some("key-page-down"),
        Key::Escape => Some("key-escape"),
        Key::Enter => Some("key-enter"),
        Key::Space => Some("key-space"),
        Key::Delete => Some("key-delete"),
        Key::Backspace => Some("key-backspace"),
        _ => None,
    };
    if let Some(id) = id {
        crate::i18n::ensure();
        return tri::i18n::translate_in(language, id, None);
    }
    match key {
        Key::Questionmark => "?".into(),
        Key::Comma => ",".into(),
        Key::Period => ".".into(),
        other => {
            let name = other.name();
            // Lettres en minuscules, comme on les tape (g et Maj+G sont deux touches).
            if name.len() == 1 {
                name.to_lowercase()
            } else {
                name.to_string()
            }
        }
    }
}

impl Default for Keymap {
    /// Le profil classique.
    fn default() -> Self {
        Keymap::from_profiles(&[Profile::Classic])
    }
}

/// Reconnaissance des séquences de touches (`g g`), comme le `timeoutlen` de vim : une
/// touche qui commence une séquence est mise en attente ; la suivante doit arriver avant
/// `timeout`. Si elle ne complète aucune séquence, elle est interprétée seule (`g` puis
/// `j` : ligne du dessous).
#[derive(Clone, Debug)]
pub struct Sequencer {
    pending: Vec<Chord>,
    since: Option<Instant>,
    timeout: Duration,
    /// Action d'une touche qui commence aussi une séquence (`y` avant `y w`), faite si la
    /// suite ne vient pas dans le délai `ambiguous`.
    waiting: Option<Action>,
    ambiguous: Duration,
    /// Menu des suites affiché ([`Sequencer::hold`]) : la séquence attend sans délai la
    /// touche suivante, le temps de lire le menu.
    held: bool,
}

impl Sequencer {
    /// Séquenceur avec un délai maximal `timeout` entre deux touches d'une séquence (et
    /// le même pour une touche ambiguë, voir [`Sequencer::with_ambiguous`]).
    pub fn new(timeout: Duration) -> Sequencer {
        Sequencer {
            pending: Vec::new(),
            since: None,
            timeout,
            waiting: None,
            ambiguous: timeout,
            held: false,
        }
    }

    /// Délai d'attente de la suite après une touche qui est un raccourci *et* le début
    /// d'un autre (`y`, `y w`) : plus court que celui des séquences, la touche seule ne
    /// doit pas sembler lente.
    pub fn with_ambiguous(mut self, ambiguous: Duration) -> Sequencer {
        self.ambiguous = ambiguous;
        self
    }

    fn window(&self) -> Duration {
        if self.waiting.is_some() {
            self.ambiguous
        } else {
            self.timeout
        }
    }

    /// Vrai si une séquence est commencée (et pas encore expirée à `now`).
    pub fn is_pending(&self, now: Instant) -> bool {
        self.since
            .is_some_and(|since| self.held || now.duration_since(since) <= self.window())
    }

    /// Début de séquence en attente à `now` (`<leader>`, `f`, `<leader> r`…) et l'instant
    /// de sa dernière touche, pour le menu des suites. Rien pour une touche ambiguë
    /// (`y` avant `y w`) : elle agit seule après un court délai.
    pub fn prefix(&self, now: Instant) -> Option<(&[Chord], Instant)> {
        if self.waiting.is_some() || !self.is_pending(now) {
            return None;
        }
        Some((&self.pending, self.since?))
    }

    /// Le menu des suites est affiché : la séquence n'expire plus, elle attend la touche
    /// suivante (ou [`Sequencer::cancel`]).
    pub fn hold(&mut self) {
        if self.since.is_some() && self.waiting.is_none() {
            self.held = true;
        }
    }

    /// Vrai si le menu des suites tient la séquence.
    pub fn is_held(&self) -> bool {
        self.held && self.since.is_some()
    }

    /// Abandonne la séquence en cours (Échap dans le menu des suites).
    pub fn cancel(&mut self) {
        self.pending.clear();
        self.since = None;
        self.waiting = None;
        self.held = false;
    }

    /// Instant où l'action d'une touche ambiguë sera faite, faute de suite : l'interface
    /// se redessine alors pour appeler [`Sequencer::expire`].
    pub fn deadline(&self) -> Option<Instant> {
        self.waiting?;
        Some(self.since? + self.ambiguous)
    }

    /// L'action d'une touche ambiguë dont le délai est passé à `now` : à faire.
    pub fn expire(&mut self, now: Instant) -> Option<Action> {
        if self.waiting.is_none() || self.is_pending(now) {
            return None;
        }
        self.pending.clear();
        self.since = None;
        self.waiting.take()
    }

    /// Traite une touche pressée à `now` ; renvoie les actions déclenchées, dans l'ordre :
    /// celle d'une touche ambiguë en attente que cette touche ne continue pas, puis la
    /// sienne.
    pub fn feed(&mut self, keymap: &Keymap, chord: Chord, now: Instant) -> Vec<Action> {
        let mut done: Vec<Action> = self.expire(now).into_iter().collect();
        if !self.is_pending(now) {
            self.pending.clear();
        }
        let mut keys = std::mem::take(&mut self.pending);
        keys.push(chord);
        let since = self.since.take();
        let waiting = self.waiting.take();
        let held = std::mem::take(&mut self.held);
        match keymap.lookup(&keys) {
            Lookup::Prefix => {
                self.pending = keys;
                self.since = Some(now);
                // Menu déjà affiché : le sous-menu (`<leader> r`) suit tout de suite.
                self.held = held;
            }
            Lookup::Ambiguous(action) => {
                self.pending = keys;
                self.since = Some(now);
                self.waiting = Some(action);
            }
            Lookup::Action(action) => done.push(action),
            // Touche liée à aucun raccourci, même seule, au milieu d'une séquence : ignorée,
            // la séquence reste en attente. « ° » arrive sur un AZERTY comme Maj + `Minus`
            // (touche physique, inconnue ici) *puis*, parfois dans une image suivante, comme
            // caractère : sans cela, la touche annulait `f` et le caractère notait au lieu
            // de filtrer (retour du 17/10/2026, macOS et Linux).
            Lookup::None if keys.len() > 1 && keymap.lookup(&[chord]) == Lookup::None => {
                keys.pop();
                self.pending = keys;
                self.since = since;
                self.waiting = waiting;
                self.held = held;
            }
            // Séquence interrompue par une touche qui a un sens seule : la touche ambiguë
            // en attente est faite, puis celle-ci compte seule.
            Lookup::None if keys.len() > 1 => {
                done.extend(waiting);
                done.extend(self.feed(keymap, chord, now));
            }
            Lookup::None => {}
        }
        done
    }

    /// [`Sequencer::feed`] pour les tests : au plus une action.
    #[cfg(test)]
    pub fn feed_one(&mut self, keymap: &Keymap, chord: Chord, now: Instant) -> Option<Action> {
        let mut done = self.feed(keymap, chord, now);
        assert!(done.len() <= 1, "{done:?}");
        done.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le profil vim (le profil par défaut, [`Keymap::default`], est le classique).
    fn vim() -> Keymap {
        Keymap::from_profiles(&[Profile::Vim])
    }

    #[test]
    fn every_default_shortcut_reads_back_from_its_spec() {
        crate::i18n::ensure();
        for profile in [Profile::Classic, Profile::Vim] {
            let km = Keymap::from_profiles(&[profile])
                .with_leader("Ctrl+B")
                .unwrap();
            for (keys, action) in &km.bindings {
                let spec = keys_spec(keys);
                assert_eq!(
                    parse_keys(&spec).as_ref(),
                    Ok(keys),
                    "{spec} ({})",
                    action.id()
                );
                assert!(km.specs_for(*action).contains(&spec));
            }
        }
        assert_eq!(Keymap::default().spec_label("g g"), "gg");
    }

    #[test]
    fn old_crop_ids_still_name_their_version_actions() {
        // 07/11/2026 : `core.crop.*` et `core.view.crop` deviennent `core.version.*` ; un
        // `[keys]` écrit avant marche encore.
        assert_eq!(Action::from_id("core.view.crop"), Some(Action::Crop));
        assert_eq!(Action::from_id("core.version.new"), Some(Action::Crop));
        assert_eq!(Action::from_id("core.crop.save"), Some(Action::CropSave));
        assert_eq!(
            Action::from_id("core.crop.ratio.square"),
            Some(Action::CropRatio(2))
        );
        assert_eq!(
            Action::from_id("core.view.version_next"),
            Some(Action::VersionNext)
        );
        assert_eq!(Action::Crop.id(), "core.version.new");
        assert_eq!(Action::CropSave.id(), "core.version.save");
        assert_eq!(Action::from_id("core.crop.nothing"), None);
    }

    #[test]
    fn ids_are_unique_namespaced_and_reversible() {
        let mut ids: Vec<&str> = Action::ALL.iter().map(|a| a.id()).collect();
        // `core.*`, ou `plugin.<nom>.<action>` pour les plugins (voir le `CLAUDE.md`).
        let plugin = |id: &str| id.starts_with("plugin.") && id.split('.').count() == 3;
        assert!(
            ids.iter().all(|id| id.starts_with("core.") || plugin(id)),
            "{ids:?}"
        );
        for a in Action::ALL {
            assert_eq!(Action::from_id(a.id()), Some(a));
        }
        assert_eq!(Action::from_id("plugin.x.y"), None);
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), Action::ALL.len());
    }

    #[test]
    fn each_profile_has_its_own_moves() {
        // « both » n'existe plus (revue du 06/11/2026) : chaque jeu seul.
        let (classic, vim) = (Keymap::default(), vim());
        let none = Modifiers::NONE;
        assert_eq!(classic.action(Key::ArrowRight, none), Some(Action::NavNext));
        assert_eq!(classic.action(Key::L, none), None);
        assert_eq!(vim.action(Key::L, none), Some(Action::NavNext));
        assert_eq!(vim.action(Key::ArrowRight, none), None);
        assert_eq!(vim.action(Key::J, none), Some(Action::NavDown));
        assert_eq!(vim.action(Key::G, none), None, "g seul : début de « gg »");
        assert_eq!(vim.action(Key::G, Modifiers::SHIFT), Some(Action::NavLast));
        assert_eq!(classic.action(Key::G, none), None, "touche libre");
    }

    #[test]
    fn shift_moves_inside_a_zoomed_image() {
        let classic = Keymap::default();
        let shift = Modifiers::SHIFT;
        assert_eq!(classic.action(Key::ArrowLeft, shift), Some(Action::PanLeft));
        assert_eq!(classic.action(Key::ArrowDown, shift), Some(Action::PanDown));
        let km = vim();
        assert_eq!(km.action(Key::H, shift), Some(Action::PanLeft));
        assert_eq!(km.action(Key::J, shift), Some(Action::PanDown));
        assert_eq!(km.action(Key::K, shift), Some(Action::PanUp));
        assert_eq!(km.action(Key::L, shift), Some(Action::PanRight));
        assert_eq!(
            km.action(Key::G, shift),
            Some(Action::NavLast),
            "Maj+G inchangé"
        );
    }

    #[test]
    fn system_modifiers_are_left_alone() {
        let km = Keymap::default();
        assert_eq!(
            km.action(Key::Q, Modifiers::COMMAND),
            None,
            "Cmd+Q reste au système"
        );
        assert_eq!(km.action(Key::Q, Modifiers::MAC_CMD), None);
        assert_eq!(km.action(Key::O, Modifiers::CTRL), None, "Ctrl+O : libre");
        assert_eq!(km.action(Key::L, Modifiers::ALT), None);
        assert_eq!(Chord::from_event(Key::ArrowLeft, Modifiers::ALT), None);
        // Cmd+Z : annuler sur macOS (exception), rien ailleurs.
        let cmd_z = Chord::from_event(Key::Z, Modifiers::MAC_CMD | Modifiers::COMMAND);
        if cfg!(target_os = "macos") {
            assert_eq!(cmd_z, Chord::from_event(Key::Z, Modifiers::CTRL));
        } else {
            assert_eq!(cmd_z, None);
        }
    }

    #[test]
    fn vim_pages_with_ctrl_u_and_ctrl_d_and_u_undoes() {
        let km = vim();
        // Sous Linux et Windows, egui signale Ctrl aussi comme `command`.
        let linux_ctrl = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
        assert_eq!(km.action(Key::U, Modifiers::CTRL), Some(Action::NavPageUp));
        assert_eq!(km.action(Key::D, linux_ctrl), Some(Action::NavPageDown));
        assert_eq!(
            km.action(Key::U, Modifiers::NONE),
            Some(Action::Undo),
            "comme dans vim"
        );
        assert_eq!(km.action(Key::R, linux_ctrl), Some(Action::Redo));
        assert_eq!(km.action(Key::D, Modifiers::NONE), None);
        assert_eq!(km.keys_for(Action::NavPageUp), ["Ctrl+u"]);
    }

    #[test]
    fn rating_keys_work_with_or_without_shift() {
        let km = Keymap::default();
        for (key, action) in [
            (Key::Num0, Action::Stars(0)),
            (Key::Num3, Action::Stars(3)),
            (Key::Num5, Action::Stars(5)),
            (Key::Num6, Action::Color(Color::Red)),
            (Key::Num9, Action::Color(Color::Blue)),
        ] {
            assert_eq!(km.action(key, Modifiers::NONE), Some(action), "{key:?}");
            assert_eq!(
                km.action(key, Modifiers::SHIFT),
                Some(action),
                "Maj+{key:?}"
            );
        }
        assert_eq!(
            km.action(Key::Comma, Modifiers::NONE),
            Some(Action::Color(Color::Purple))
        );
        assert_eq!(
            km.action(Key::P, Modifiers::SHIFT),
            Some(Action::Flag(Flag::Accepted))
        );
        assert_eq!(
            km.action(Key::X, Modifiers::SHIFT),
            Some(Action::Flag(Flag::Rejected))
        );
        assert_eq!(
            km.action(Key::U, Modifiers::SHIFT),
            Some(Action::Flag(Flag::None))
        );
        assert_eq!(
            km.keys_for(Action::Stars(3)),
            ["3"],
            "Maj+3 implicite dans l'aide"
        );
        assert_eq!(
            km.action(Key::Period, Modifiers::NONE),
            Some(Action::Color(Color::Purple)),
            "pavé numérique sous Linux"
        );
        assert_eq!(km.keys_for(Action::Color(Color::Purple)), ["°", ",", "."]);
    }

    #[test]
    fn action_labels_name_no_arrow_key() {
        // Retour du 06/11/2026 (jeu vim) : « flèches gauche / droite » dans un libellé ;
        // les touches du jeu choisi sont montrées à côté, le libellé dit ce que fait
        // l'action.
        crate::i18n::ensure();
        for language in Language::ALL {
            for action in Action::ALL {
                let id = format!("action-{}", action.id().replace('.', "-"));
                let mut args = tri::i18n::Args::new();
                args.set("os", tri::i18n::value(crate::i18n::os()));
                let label = tri::i18n::translate_in(language, &id, Some(&args)).to_lowercase();
                for word in ["flèche", "arrow", "←", "→", "↑", "↓"] {
                    assert!(!label.contains(word), "{id} ({language:?}) : {label}");
                }
            }
        }
    }

    #[test]
    fn interface_labels_exist_in_every_language() {
        crate::i18n::ensure();
        for action in Action::ALL {
            for id in action.interface_labels() {
                for language in Language::ALL {
                    assert!(tri::i18n::has(language, id), "{id} ({language:?})");
                }
            }
        }
    }

    #[test]
    fn arrows_or_hjkl_after_the_leader_follow_the_chosen_set() {
        // Retour du 06/11/2026 : « h / Flèche gauche » dans le menu des suites, en vim.
        let lead = parse_chord("Ctrl+b").unwrap();
        let vim = vim().with_leader("Ctrl+b").unwrap();
        let classic = Keymap::from_profiles(&[Profile::Classic])
            .with_leader("Ctrl+b")
            .unwrap();
        let keys = |km: &Keymap, action: Action| -> Vec<String> {
            km.continuations(&[lead])
                .into_iter()
                .filter(|n| n.action == Some(action))
                .map(|n| n.key)
                .collect()
        };
        assert_eq!(keys(&vim, Action::FocusLeft), ["h"]);
        assert_eq!(
            keys(&classic, Action::FocusLeft),
            [key_label(Key::ArrowLeft)]
        );
        assert_eq!(vim.keys_for(Action::FocusLeft), ["<leader> h"]);
        assert_eq!(
            classic.keys_for(Action::FocusLeft),
            [format!("<leader> {}", key_label(Key::ArrowLeft))]
        );
        // Les deux marchent toujours.
        let mut seq = Sequencer::new(Duration::from_secs(1));
        let t = Instant::now();
        seq.feed_one(&vim, lead, t);
        assert_eq!(
            seq.feed_one(&vim, plain(Key::ArrowLeft), t),
            Some(Action::FocusLeft)
        );
        // Une action sans variante garde sa touche (`<leader> p j` / `<leader> p Bas`).
        let p = [lead, parse_chord("p").unwrap()];
        let lenient: Vec<String> = vim
            .continuations(&p)
            .into_iter()
            .filter(|n| n.action == Some(Action::SharpnessLenient))
            .map(|n| n.key)
            .collect();
        assert_eq!(lenient, ["j"]);
    }

    #[test]
    fn the_help_shows_the_keys_of_the_chosen_set_only() {
        // Retour du 06/11/2026 : en vim, « x » supprime, sans Suppr ni Retour arrière
        // (communs, qui marchent aussi) ; en classique, les communs.
        let vim = vim();
        assert_eq!(vim.keys_for(Action::FileDelete), ["x"]);
        assert_eq!(vim.keys_for(Action::Undo), ["u"]);
        let classic = Keymap::from_profiles(&[Profile::Classic]);
        assert_eq!(classic.keys_for(Action::Undo), ["Ctrl+z"]);
        assert_eq!(classic.keys_for(Action::FileDelete).len(), 2);
        // Les communes marchent toujours.
        assert_eq!(
            vim.action(Key::Delete, Modifiers::NONE),
            Some(Action::FileDelete)
        );
        // Sans touche propre au jeu : les communes.
        assert_eq!(vim.keys_for(Action::Open), classic.keys_for(Action::Open));
        // La recherche : `/` en grille (« ajusté » y cherche), seul en vim.
        assert_eq!(vim.keys_for(Action::Search), ["/"]);
        assert_eq!(classic.keys_for(Action::Search), ["/", "Ctrl+f"]);
        // `[keys]` : ce que l'utilisateur a choisi.
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "core.edit.undo".to_string(),
            vec!["Ctrl+Shift+u".to_string()],
        );
        let (km, _) = vim.with_overrides(&overrides);
        assert_eq!(km.keys_for(Action::Undo).len(), 1);
    }

    #[test]
    fn the_help_writes_leader_for_the_panel_prefix() {
        // #175 : l'aide écrivait « Ctrl+B puis r » ; le leader est réglable, montré à part.
        let km = vim().with_leader("Ctrl+b").unwrap();
        let keys = km.keys_for(Action::Develop);
        assert!(keys.contains(&"<leader> rr".to_string()), "{keys:?}");
        let keys = km.keys_for(Action::FocusLeft);
        assert!(keys.contains(&"<leader> h".to_string()), "{keys:?}");
        assert!(!keys.iter().any(|k| k.contains("Ctrl")), "{keys:?}");
        // Fenêtre de réglages aussi (pastilles de `[keys]`).
        let specs = km.specs_for(Action::FocusNext);
        let labels: Vec<String> = specs.iter().map(|s| km.spec_label(s)).collect();
        assert_eq!(labels, ["<leader> o"]);
    }

    #[test]
    fn the_key_menu_lists_what_can_follow_a_prefix() {
        let km = vim().with_leader("Ctrl+b").unwrap();
        let lead = parse_chord("Ctrl+b").unwrap();
        let next = km.continuations(&[lead]);
        let find = |key: &str| next.iter().find(|n| n.key == key);
        // Une action directe : son action, pas de sous-menu.
        let h = find("h").expect("h");
        assert_eq!(h.action, Some(Action::FocusLeft));
        assert_eq!(h.menu, None);
        // Un sous-menu nommé par `[menus]` de keys.toml.
        let i = find("i").expect("i");
        assert_eq!(i.action, None);
        assert_eq!(
            i.menu.as_deref(),
            Some(t!("key-submenu-histogram").as_str())
        );
        // Caractères d'abord, touches nommées ensuite.
        let space = next.iter().position(|n| n.key == key_label(Key::Space));
        let h = next.iter().position(|n| n.key == "h");
        assert!(h < space, "{next:?}");
        assert_eq!(km.prefix_label(&[lead]), t!("key-leader"));
        // Dans le sous-menu : les réglages de l'histogramme.
        let sub = km.continuations(&[lead, parse_chord("i").unwrap()]);
        assert!(sub.iter().any(|n| n.action == Some(Action::HistogramScale)));
        assert_eq!(
            km.prefix_label(&[lead, parse_chord("i").unwrap()]),
            format!("{} i", t!("key-leader"))
        );
    }

    #[test]
    fn the_key_menu_follows_the_focused_panel() {
        // `<leader> r d` : développer dans darktable depuis les images, ouvrir le dossier
        // dans darktable depuis le panneau des dossiers.
        let mut km = Keymap::default().with_leader("Ctrl+b").unwrap();
        let prefix = [parse_chord("Ctrl+b").unwrap(), parse_chord("r").unwrap()];
        let d = |km: &Keymap| {
            km.continuations(&prefix)
                .into_iter()
                .find(|n| n.key == "d")
                .and_then(|n| n.action)
        };
        let images = d(&km).expect("d");
        km.set_focus(Scope::Folders);
        let folders = d(&km).expect("d");
        assert_ne!(images, folders);
        assert_eq!(folders.scope(), Some(Scope::Folders));
    }

    #[test]
    fn a_held_sequence_waits_for_its_next_key() {
        let km = Keymap::default().with_leader("Ctrl+b").unwrap();
        let lead = parse_chord("Ctrl+b").unwrap();
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let t = Instant::now();
        assert_eq!(seq.feed_one(&km, lead, t), None);
        assert_eq!(seq.prefix(t).map(|(p, _)| p.to_vec()), Some(vec![lead]));
        seq.hold();
        assert!(seq.is_held());
        // Bien après le délai des séquences : toujours en attente.
        let later = t + Duration::from_secs(30);
        assert!(seq.is_pending(later));
        // Le sous-menu reste tenu, puis l'action part.
        assert_eq!(seq.feed_one(&km, parse_chord("i").unwrap(), later), None);
        assert!(seq.is_held());
        assert_eq!(
            seq.feed_one(&km, parse_chord("s").unwrap(), later),
            Some(Action::HistogramScale)
        );
        assert!(!seq.is_held());
        assert!(!seq.is_pending(later));
        // Abandon.
        seq.feed_one(&km, lead, later);
        seq.hold();
        seq.cancel();
        assert!(!seq.is_pending(later));
        assert!(seq.prefix(later).is_none());
    }

    #[test]
    fn an_ambiguous_key_shows_no_menu() {
        // `y` copie seule après un court délai (`y )` : la pile) : pas de menu.
        let km = vim();
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let t = Instant::now();
        assert_eq!(seq.feed_one(&km, parse_chord("y").unwrap(), t), None);
        assert!(seq.prefix(t).is_none());
        seq.hold();
        assert!(!seq.is_held());
    }

    #[test]
    fn menu_names_come_from_the_keys_file() {
        let text =
            "[menus]\n\"f\" = \"filters\"\n\"<leader> x\" = \"Mine\"\n\"Nope+q\" = \"bad\"\n";
        let (d, warnings) = Defaults::parse(text).unwrap();
        assert_eq!(d.menus.len(), 2, "{:?}", d.menus);
        assert_eq!(
            d.menus[0],
            (
                false,
                vec![parse_chord("f").unwrap()],
                "filters".to_string()
            )
        );
        assert!(d.menus[1].0);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn the_focused_panel_takes_its_keys_first_and_lets_the_rest_through() {
        let mut km = vim().with_leader("Ctrl+b").unwrap();
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let now = Instant::now();
        let key = |k| Chord::from_event(k, Modifiers::NONE).unwrap();
        assert_eq!(seq.feed_one(&km, key(Key::J), now), Some(Action::NavDown));
        km.set_focus(Scope::Folders);
        assert_eq!(
            seq.feed_one(&km, key(Key::J), now),
            Some(Action::FolderDown)
        );
        assert_eq!(
            seq.feed_one(&km, key(Key::Enter), now),
            Some(Action::FolderOpen)
        );
        assert_eq!(
            seq.feed_one(&km, key(Key::Num3), now),
            Some(Action::Stars(3)),
            "le reste passe"
        );
        // `y` copie le dossier pointé, pas les images (retour du 20/10/2026).
        let y = seq
            .feed_one(&km, key(Key::Y), now)
            .or_else(|| seq.expire(now + Duration::from_secs(2)));
        assert_eq!(y, Some(Action::FolderCopy));
        let ctrl_x = Chord::from_event(Key::X, Modifiers::CTRL).unwrap();
        assert_eq!(seq.feed_one(&km, ctrl_x, now), Some(Action::FolderMove));
        km.set_focus(Scope::Metadata);
        assert_eq!(seq.feed_one(&km, key(Key::Y), now), Some(Action::MetaCopy));
        let ctrl_c = Chord::from_event(Key::C, Modifiers::CTRL).unwrap();
        assert_eq!(
            seq.feed_one(&km, ctrl_c, now),
            Some(Action::MetaCopy),
            "Ctrl+C aussi"
        );
        // Leader puis direction.
        let lead = Chord::from_event(Key::B, Modifiers::CTRL).unwrap();
        assert_eq!(seq.feed_one(&km, lead, now), None, "en attente");
        assert_eq!(seq.feed_one(&km, key(Key::H), now), Some(Action::FocusLeft));
        let question = key(Key::Questionmark);
        assert_eq!(seq.feed_one(&km, lead, now), None);
        assert_eq!(seq.feed_one(&km, question, now), Some(Action::ReportIssue));
        // Développer (revue du 06/11/2026) : `r` puis `r` le logiciel de l'image, `d`
        // darktable, `p` RapidRAW, `t` RawTherapee, `a` ART.
        use tri::develop::Engine;
        for (k, action) in [
            (Key::R, Action::Develop),
            (Key::D, Action::DevelopWith(Engine::Darktable)),
            (Key::P, Action::DevelopWith(Engine::RapidRaw)),
            (Key::T, Action::DevelopWith(Engine::RawTherapee)),
            (Key::A, Action::DevelopWith(Engine::Art)),
        ] {
            assert_eq!(seq.feed_one(&km, lead, now), None);
            assert_eq!(seq.feed_one(&km, key(Key::R), now), None);
            assert_eq!(seq.feed_one(&km, key(k), now), Some(action), "{k:?}");
        }
        // Le leader atteint les actions des autres panneaux : un canal de l'histogramme
        // sans l'ouvrir en grand ; dans les dossiers, `r d` ouvre le dossier dans
        // darktable plutôt que de développer.
        km.set_focus(Scope::Images);
        for k in [lead, key(Key::I)] {
            assert_eq!(seq.feed_one(&km, k, now), None);
        }
        assert_eq!(
            seq.feed_one(&km, key(Key::R), now),
            Some(Action::HistogramRed)
        );
        km.set_focus(Scope::Folders);
        for k in [lead, key(Key::R)] {
            assert_eq!(seq.feed_one(&km, k, now), None);
        }
        assert_eq!(
            seq.feed_one(&km, key(Key::D), now),
            Some(Action::FolderOpenIn(Engine::Darktable))
        );
        km.set_focus(Scope::Metadata);
        assert_eq!(
            Action::from_id("plugin.rapidraw.develop"),
            Some(Action::DevelopWith(Engine::RapidRaw))
        );
        assert!(vim().with_leader("Ctrl+").is_err());
    }

    #[test]
    fn a_bound_character_triggers_its_action() {
        let km = Keymap::default();
        assert!(km.uses_char('°'));
        assert!(!km.uses_char('g'), "g est une touche, pas un caractère");
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        assert_eq!(
            seq.feed_one(&km, Chord::char('°'), Instant::now()),
            Some(Action::Color(Color::Purple))
        );
    }

    #[test]
    fn the_keys_file_lists_every_action_and_reads_cleanly() {
        // Toutes les actions, même sans raccourci (`[]`) : un fichier à lire d'un coup
        // d'œil, et un point de départ complet pour qui écrit le sien (choix de
        // l'utilisateur, 06/11/2026). Une nouvelle action s'ajoute aussi à `keys.toml`.
        assert!(
            Defaults::warnings().is_empty(),
            "{:?}",
            Defaults::warnings()
        );
        let table: toml::Table = toml::from_str(EMBEDDED_KEYS).unwrap();
        let ids: Vec<&String> = table
            .values()
            .filter_map(|v| v.as_table())
            .flat_map(|t| t.keys())
            .collect();
        let missing: Vec<&str> = Action::ALL
            .iter()
            .filter(|a| a.bindable())
            .map(|a| a.id())
            .filter(|id| !ids.iter().any(|i| i == id))
            .collect();
        assert!(missing.is_empty(), "absentes de keys.toml : {missing:?}");
        // Le fichier livré est celui intégré au programme.
        let shipped = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/keys.toml"),
        )
        .unwrap();
        assert_eq!(shipped, EMBEDDED_KEYS);
    }

    #[test]
    fn a_broken_keys_file_is_reported_entry_by_entry() {
        let text = r#"
            [common]
            "core.view.zoom" = ["z", "Ctrl+Pomme"]
            "core.nope" = ["x"]
            [vim]
            "core.nav.next" = ["l"]
            [autre]
            "core.view.back" = ["Escape"]
        "#;
        let (d, warnings) = Defaults::parse(text).unwrap();
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert_eq!(d.common, vec![(vec![plain(Key::Z)], Action::ToggleZoom)]);
        assert_eq!(d.vim, vec![(vec![plain(Key::L)], Action::NavNext)]);
        assert!(Defaults::parse("[common\n").is_err(), "TOML illisible");
        // Le préfixe des panneaux, en tête du fichier.
        assert_eq!(Defaults::get().prefix(), "Ctrl+b");
        let (d, warnings) = Defaults::parse("leader = \"Ctrl+Pomme\"").unwrap();
        assert_eq!(
            (d.prefix(), warnings.len()),
            ("", 1),
            "préfixe illisible : signalé"
        );
    }

    #[test]
    fn profiles_cover_the_same_actions_without_internal_conflict() {
        // Les deux jeux donnent une touche aux mêmes actions (les touches après le leader
        // valent pour les deux) ; celles sans touche par défaut sont listées avec `[]`
        // dans `keys.toml` (voir `the_keys_file_lists_every_action_and_reads_cleanly`).
        let leader: Vec<Action> = Defaults::get().leader_actions().collect();
        let bound = |p: Profile| -> std::collections::BTreeSet<&'static str> {
            p.defaults()
                .map(|(_, a)| a.id())
                .chain(leader.iter().map(|a| a.id()))
                .collect()
        };
        let (classic, vim) = (bound(Profile::Classic), bound(Profile::Vim));
        assert_eq!(
            classic.symmetric_difference(&vim).collect::<Vec<_>>(),
            Vec::<&&str>::new(),
            "actions liées dans un seul jeu"
        );
        for profile in [Profile::Classic, Profile::Vim] {
            let b: Vec<&(Vec<Chord>, Action)> = profile.defaults().collect();
            // Même touche pour une action d'un panneau et une action générale : voulu.
            for (i, (k, x)) in b.iter().enumerate() {
                assert!(
                    !b[i + 1..].iter().any(|(l, y)| l == k && conflict(*x, *y)),
                    "{profile:?} : doublon {k:?}"
                );
            }
        }
    }

    #[test]
    fn only_the_copy_key_is_also_the_start_of_another() {
        // Une telle touche attend un court instant la suite avant d'agir : voulu pour `y`
        // (`y w`) et pour `(` / `)` (`((` / `))`, 31/10/2026) seulement, pour ne retarder
        // aucune autre touche. Une touche d'un panneau
        // peut commencer une séquence générale (`g` du grand histogramme, `gg`) : dans le
        // panneau, la touche l'emporte ; ailleurs, la séquence marche.
        let km = Keymap::default();
        for (k, a) in &km.bindings {
            let starts = km
                .bindings
                .iter()
                .any(|(l, b)| l.len() > k.len() && l.starts_with(k) && conflict(*a, *b));
            assert!(
                !starts
                    || matches!(
                        *a,
                        Action::FileCopy | Action::StackFirst | Action::StackLast
                    ),
                "{a:?} ({k:?}) est aussi le début d'une séquence"
            );
        }
    }

    #[test]
    fn help_lists_readable_keys() {
        let km = Keymap::default();
        assert_eq!(km.keys_for(Action::NavFirst), ["Début"]);
        assert_eq!(km.keys_for(Action::NavLast), ["Fin"]);
        assert_eq!(km.keys_for(Action::NavPrev), ["Flèche gauche"]);
        assert_eq!(km.keys_for(Action::PanLeft), ["Maj+Flèche gauche"]);
        let vim = vim();
        assert_eq!(vim.keys_for(Action::NavFirst), ["gg"]);
        assert_eq!(vim.keys_for(Action::NavLast), ["Maj+g"]);
        assert_eq!(vim.keys_for(Action::NavPrev), ["h"]);
        assert_eq!(vim.keys_for(Action::PanLeft), ["Maj+h"]);
        assert_eq!(km.keys_for(Action::Help), ["F1"]);
        assert_eq!(km.keys_for(Action::HelpHere), ["?"]);
        assert_eq!(
            km.action(Key::Questionmark, Modifiers::SHIFT),
            Some(Action::HelpHere)
        );
        assert_eq!(
            sequence_label(&[plain(Key::Escape), plain(Key::G)]),
            "Échap puis g"
        );
    }

    #[test]
    fn labels_are_distinct_and_readable() {
        let mut labels: Vec<String> = Action::ALL.iter().map(|a| a.label()).collect();
        assert!(labels.iter().all(|l| !l.contains('.') || l.contains(' ')));
        labels.sort();
        labels.dedup();
        assert_eq!(labels.len(), Action::ALL.len());
    }

    #[test]
    fn first_profile_wins_on_conflict() {
        let km = Keymap::from_profiles(&[Profile::Vim, Profile::Classic]);
        assert_eq!(km.action(Key::Z, Modifiers::NONE), Some(Action::ToggleZoom));
    }

    fn g() -> Chord {
        plain(Key::G)
    }

    #[test]
    fn gg_goes_to_the_first_image() {
        let km = vim();
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let t = Instant::now();
        assert_eq!(seq.feed_one(&km, g(), t), None);
        assert!(seq.is_pending(t));
        assert_eq!(
            seq.feed_one(&km, g(), t + Duration::from_millis(300)),
            Some(Action::NavFirst)
        );
        assert!(!seq.is_pending(t + Duration::from_millis(300)));
    }

    #[test]
    fn a_too_slow_second_g_starts_over() {
        let km = vim();
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let t = Instant::now();
        seq.feed_one(&km, g(), t);
        let late = t + Duration::from_millis(1500);
        assert_eq!(
            seq.feed_one(&km, g(), late),
            None,
            "nouveau début de séquence"
        );
        assert_eq!(
            seq.feed_one(&km, g(), late + Duration::from_millis(100)),
            Some(Action::NavFirst)
        );
    }

    #[test]
    fn an_unbound_key_does_not_break_a_sequence() {
        let km = Keymap::default();
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let t = Instant::now();
        // « f ° » sur un AZERTY, touche et caractère dans deux images différentes.
        assert_eq!(seq.feed_one(&km, plain(Key::F), t), None);
        assert_eq!(
            seq.feed_one(&km, shifted(Key::Minus), t),
            None,
            "touche inconnue ignorée"
        );
        assert!(seq.is_pending(t), "f toujours en attente");
        assert_eq!(
            seq.feed_one(&km, Chord::char('°'), t),
            Some(Action::FilterColor(Color::Purple))
        );
        // Le délai court toujours depuis f : la touche ignorée ne le prolonge pas.
        seq.feed_one(&km, plain(Key::F), t);
        seq.feed_one(&km, shifted(Key::Minus), t + Duration::from_millis(900));
        assert!(!seq.is_pending(t + Duration::from_millis(1100)));
    }

    #[test]
    fn an_interrupted_sequence_lets_the_last_key_act_alone() {
        let km = vim();
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let t = Instant::now();
        seq.feed_one(&km, g(), t);
        assert_eq!(seq.feed_one(&km, plain(Key::J), t), Some(Action::NavDown));
        assert!(!seq.is_pending(t));
        // Une touche seule passe directement, sans attente.
        assert_eq!(seq.feed_one(&km, plain(Key::L), t), Some(Action::NavNext));
    }

    fn chord(key: Key, shift: bool, ctrl: bool) -> Chord {
        Chord {
            trigger: Trigger::Key(key),
            shift,
            ctrl,
        }
    }

    #[test]
    fn key_specs_are_parsed() {
        let ok = |s: &str| parse_keys(s).unwrap();
        assert_eq!(ok("p"), [plain(Key::P)]);
        assert_eq!(ok("Maj+P"), [shifted(Key::P)]);
        assert_eq!(ok("shift+p"), [shifted(Key::P)]);
        assert_eq!(
            ok("G"),
            [shifted(Key::G)],
            "majuscule = Maj, comme dans vim"
        );
        assert_eq!(ok("Ctrl+u"), [ctrl(Key::U)]);
        assert_eq!(ok("Ctrl+Maj+u"), [chord(Key::U, true, true)]);
        assert_eq!(ok("g g"), [plain(Key::G), plain(Key::G)]);
        assert_eq!(ok("Flèche gauche"), [plain(Key::ArrowLeft)]);
        assert_eq!(ok("Maj+Flèche gauche"), [shifted(Key::ArrowLeft)]);
        assert_eq!(ok("ArrowLeft"), [plain(Key::ArrowLeft)]);
        assert_eq!(ok("Échap"), [plain(Key::Escape)]);
        assert_eq!(ok("Escape g"), [plain(Key::Escape), plain(Key::G)]);
        assert_eq!(ok("Page préc."), [plain(Key::PageUp)]);
        assert_eq!(ok("3"), [plain(Key::Num3)]);
        assert_eq!(ok(","), [plain(Key::Comma)]);
        assert_eq!(
            ok("+"),
            [Chord::char('+')],
            "un caractère, quelle que soit la disposition"
        );
        assert_eq!(ok("°"), [Chord::char('°')]);
        assert!(parse_keys("").is_err());
        assert!(parse_keys("Maj+°").is_err(), "Maj fait partie du caractère");
        assert!(parse_keys("Hyper").unwrap_err().contains("Hyper"));
    }

    #[test]
    fn every_help_label_reads_back_as_the_same_keys() {
        let km = Keymap::default();
        for (keys, _) in &km.bindings {
            // « ? » s'affiche sans Maj : selon la disposition, il vient avec ou sans.
            if keys == &[shifted(Key::Questionmark)] {
                continue;
            }
            // « gg » s'écrit « g g » dans le fichier.
            let label = sequence_label(keys);
            let spec = if keys.len() > 1 && label.chars().count() == keys.len() {
                label
                    .chars()
                    .map(String::from)
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                label.replace(" puis ", " ")
            };
            assert_eq!(&parse_keys(&spec).unwrap(), keys, "« {spec} »");
        }
    }

    fn overrides(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        entries
            .iter()
            .map(|(id, keys)| (id.to_string(), keys.iter().map(|k| k.to_string()).collect()))
            .collect()
    }

    #[test]
    fn config_keys_replace_the_profile_keys_of_an_action() {
        let (km, warnings) =
            Keymap::default().with_overrides(&overrides(&[("core.flag.accept", &["g"])]));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            km.action(Key::G, Modifiers::NONE),
            Some(Action::Flag(Flag::Accepted))
        );
        assert_eq!(km.action(Key::P, Modifiers::SHIFT), None, "Maj+P remplacé");
        assert_eq!(km.keys_for(Action::Flag(Flag::Accepted)), ["g"]);

        let (km, _) = Keymap::default().with_overrides(&overrides(&[("core.app.quit", &[])]));
        assert_eq!(
            km.action(Key::Q, Modifiers::NONE),
            None,
            "liste vide : plus de touche"
        );
    }

    #[test]
    fn config_problems_are_reported() {
        let (km, warnings) = Keymap::default().with_overrides(&overrides(&[
            ("core.nope", &["x"]),
            ("core.flag.accept", &["Hyper", "b"]),
            ("core.rate.star.1", &["Maj+P"]),
        ]));
        let all = warnings.join("\n");
        assert!(all.contains("core.nope"), "{all}");
        assert!(all.contains("Hyper"), "{all}");
        // « b » était le fond : le fichier l'emporte, et le dit.
        assert!(all.contains("core.view.background"), "{all}");
        assert_eq!(
            km.action(Key::B, Modifiers::NONE),
            Some(Action::Flag(Flag::Accepted))
        );
        assert_eq!(km.action(Key::P, Modifiers::SHIFT), Some(Action::Stars(1)));
    }

    #[test]
    fn two_actions_on_the_same_key_keep_the_first() {
        let (km, warnings) = Keymap::default().with_overrides(&overrides(&[
            ("core.flag.accept", &["g"]),
            ("core.flag.reject", &["g"]),
        ]));
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(
            km.action(Key::G, Modifiers::NONE),
            Some(Action::Flag(Flag::Accepted))
        );
    }

    #[test]
    fn a_double_parenthesis_opens_or_closes_a_level() {
        // `((` ouvre, `))` ferme (31/10/2026) ; `(` seul reste « première de la pile »,
        // au bout du délai court.
        let km = vim();
        let mut seq =
            Sequencer::new(Duration::from_millis(1000)).with_ambiguous(Duration::from_millis(300));
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        assert!(seq.feed(&km, Chord::char('('), t0).is_empty());
        assert_eq!(
            seq.feed(&km, Chord::char('('), ms(100)),
            [Action::StackOpen]
        );
        assert!(seq.feed(&km, Chord::char(')'), ms(500)).is_empty());
        assert_eq!(
            seq.feed(&km, Chord::char(')'), ms(600)),
            [Action::StackClose]
        );
        assert!(seq.feed(&km, Chord::char('('), ms(2000)).is_empty());
        assert_eq!(seq.expire(ms(2400)), Some(Action::StackFirst));
    }

    #[test]
    fn a_key_that_starts_a_sequence_waits_for_the_rest() {
        // `y` copie, `y )` copie la pile : `y` attend la suite le temps du délai court.
        let km = vim();
        let mut seq =
            Sequencer::new(Duration::from_millis(1000)).with_ambiguous(Duration::from_millis(300));
        let t0 = Instant::now();
        let key = |k| Chord::from_event(k, Modifiers::NONE).unwrap();
        let ms = |n| t0 + Duration::from_millis(n);
        assert!(seq.feed(&km, key(Key::Y), t0).is_empty());
        assert_eq!(seq.deadline(), Some(ms(300)));
        assert_eq!(
            seq.feed(&km, Chord::char(')'), ms(100)),
            [Action::FileCopyStack]
        );
        // Seul : fait au bout du délai court.
        assert!(seq.feed(&km, key(Key::Y), ms(1000)).is_empty());
        assert_eq!(seq.expire(ms(1100)), None, "encore en attente");
        assert_eq!(seq.expire(ms(1400)), Some(Action::FileCopy));
        assert_eq!(seq.deadline(), None);
        // Suivi d'une autre touche : la copie, puis cette touche.
        seq.feed(&km, key(Key::Y), ms(2000));
        assert_eq!(
            seq.feed(&km, key(Key::J), ms(2100)),
            [Action::FileCopy, Action::NavDown]
        );
        // `d )` : une séquence ordinaire (`d` seul ne fait rien).
        assert!(seq.feed(&km, key(Key::D), ms(3000)).is_empty());
        assert_eq!(
            seq.feed(&km, Chord::char(')'), ms(3500)),
            [Action::FileDeleteStack]
        );
        // `)` seul : la fin de la pile (issue n° 62), au bout du délai court depuis que
        // `))` existe (31/10/2026).
        assert!(seq.feed(&km, Chord::char(')'), ms(5000)).is_empty());
        assert_eq!(seq.expire(ms(5400)), Some(Action::StackLast));
        // Plus de signalement : la touche seule reste joignable.
        let (_, warnings) = vim().with_overrides(&overrides(&[("core.flag.accept", &["g"])]));
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    fn key_event(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn a_held_key_repeats_only_moves() {
        // #160 : `e` tenu ouvrait l'éditeur en boucle (Windows) ; → tenu avance toujours.
        let km = Keymap::default();
        let held = |key| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::NONE,
        };
        let events = [
            key_event(Key::E, true, Modifiers::NONE),
            held(Key::E),
            Event::Text("°".into()),
        ];
        // La touche tenue suivie de son caractère : le caractère seul, répété lui aussi.
        let chords = km.chords_repeated(&events, Modifiers::NONE);
        assert_eq!(chords, [(plain(Key::E), false), (Chord::char('°'), true)]);
        assert!(Action::NavNext.repeats() && Action::PanLeft.repeats());
        assert!(!Action::FileEdit.repeats() && !Action::Develop.repeats());
    }

    #[test]
    fn every_shortcut_survives_what_the_system_delivers() {
        // Revue du 28/10/2026, après #194 : les tests tapaient les touches telles quelles,
        // alors qu'egui-winit en remplace certaines par couper / copier / coller. Chaque
        // raccourci de chaque profil passe ici par les événements de la vraie fenêtre,
        // sous Linux, Windows et macOS (Ctrl physique), et doit revenir intact.
        let km = Keymap::default();
        for profile in [Profile::Classic, Profile::Vim] {
            for (chords, action) in profile.defaults() {
                for chord in chords.iter() {
                    let Trigger::Key(key) = chord.trigger else {
                        continue;
                    };
                    for os in ["linux", "windows", "macos"] {
                        let modifiers = Modifiers {
                            shift: chord.shift,
                            ctrl: chord.ctrl,
                            // Ailleurs que sur macOS, « commande » est Ctrl.
                            command: chord.ctrl && os != "macos",
                            ..Default::default()
                        };
                        let events = winit_events(key, modifiers, os == "windows");
                        let got: Vec<Chord> = km
                            .chords_repeated(&events, modifiers)
                            .into_iter()
                            .map(|(c, _)| c)
                            .collect();
                        assert_eq!(
                            got,
                            [*chord],
                            "{profile:?} {os} : {chord:?} ({action:?}) perdu en route"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn clipboard_events_come_back_as_their_keys() {
        // Issue n° 194 : egui-winit livre Maj+Suppr (Windows), Ctrl+C, Ctrl+X comme
        // couper / copier, sans la touche ; la pile n'était pas supprimée.
        let km = Keymap::default();
        let ctrl = Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let one = |e: Event, m: Modifiers| -> Vec<Chord> {
            km.chords_repeated(&[e], m)
                .into_iter()
                .map(|(c, _)| c)
                .collect()
        };
        assert_eq!(one(Event::Cut, Modifiers::SHIFT), [shifted(Key::Delete)]);
        assert_eq!(
            km.action(Key::Delete, Modifiers::SHIFT),
            Some(Action::FileDeleteStack)
        );
        assert_eq!(one(Event::Cut, ctrl), [super::ctrl(Key::X)]);
        assert_eq!(one(Event::Copy, ctrl), [super::ctrl(Key::C)]);
        assert_eq!(
            one(Event::Paste(String::new()), Modifiers::SHIFT),
            [shifted(Key::Insert)]
        );
    }

    #[test]
    fn a_character_replaces_the_key_that_typed_it() {
        let km = Keymap::default();
        // « f ° » sur un AZERTY : f, puis Maj + touche physique Minus et le caractère °.
        let events = [
            key_event(Key::F, true, Modifiers::NONE),
            Event::Text("f".into()),
            key_event(Key::F, false, Modifiers::NONE),
            key_event(Key::Minus, true, Modifiers::SHIFT),
            Event::Text("°".into()),
            key_event(Key::Minus, false, Modifiers::SHIFT),
        ];
        let chords: Vec<Chord> = km
            .chords_repeated(&events, Modifiers::NONE)
            .into_iter()
            .map(|(c, _)| c)
            .collect();
        assert_eq!(chords, [plain(Key::F), Chord::char('°')]);
        let mut seq = Sequencer::new(Duration::from_millis(1000));
        let now = Instant::now();
        let actions: Vec<Action> = chords
            .into_iter()
            .filter_map(|c| seq.feed_one(&km, c, now))
            .collect();
        assert_eq!(actions, [Action::FilterColor(Color::Purple)]);
    }

    #[test]
    fn every_action_has_a_group_and_groups_cover_the_help() {
        for g in Group::ALL {
            assert!(Action::ALL.iter().any(|a| a.group() == g), "{g:?} vide");
        }
    }

    #[test]
    fn fuzzy_search_like_fzf() {
        let s = |q: &str, t: &str| tri::fuzzy::score(q, t);
        assert!(s("pnd", "Afficher / masquer le panneau des dossiers").is_some());
        assert!(
            s("dossiers panneau", "le panneau des dossiers").is_some(),
            "mots en désordre"
        );
        assert!(s("etoile", "3 étoiles").is_some(), "accents ignorés");
        assert!(s("ETOILE", "3 étoiles").is_some(), "casse ignorée");
        assert!(s("xyz", "3 étoiles").is_none());
        assert!(
            s("dossiers zzz", "le panneau des dossiers").is_none(),
            "tous les mots"
        );
        assert_eq!(s("", "n'importe quoi"), Some(0));
        // Lettres consécutives et débuts de mots passent devant les correspondances éparses.
        assert!(s("zoom", "Zoom 100 % / ajusté") > s("zoom", "z o o m éparpillé"));
        assert!(s("fil", "Filtre de type") > s("fil", "Afficher le profil"));
    }

    #[test]
    fn key_names_are_read_in_every_language() {
        assert_eq!(parse_keys("Shift+P"), parse_keys("Maj+P"));
        assert_eq!(parse_keys("Left arrow"), parse_keys("Flèche gauche"));
        assert_eq!(parse_keys("Esc g"), parse_keys("Échap g"));
        assert_eq!(key_label_in(Language::En, Key::ArrowLeft), "Left arrow");
        assert_eq!(key_label_in(Language::Fr, Key::ArrowLeft), "Flèche gauche");
    }
}
