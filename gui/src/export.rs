//! Boîte « Exporter » (19/10/2026) : choix du profil d'export et du dossier, puis export
//! en fond, progression dans la barre d'état. Le travail est fait par le moteur
//! ([`tri::export`], darktable) ; ici, seulement le choix et le suivi.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;

use eframe::egui::{self, Key, Modifiers, Pos2};
use tri::develop::{Darktable, DevelopError};
use tri::export::{Format, Profile, Report};

/// Image exportée : on exporte ce qu'on regarde (idée de l'utilisateur, 19/10/2026).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Render {
    /// Développée par darktable (son `.xmp`).
    Darktable,
    /// Rendu de base par LibRaw (« rendu RAW »).
    LibRaw,
    /// JPEG boîtier (aperçu embarqué du RAW).
    Embedded,
}

impl Render {
    /// Ce qu'on regarde : en plein écran, la source affichée ; en grille (`None`),
    /// darktable s'il est là, sinon LibRaw.
    pub fn of(viewing: Option<crate::view::Source>, darktable: bool) -> Render {
        use crate::view::Source;
        match viewing {
            Some(Source::Raw) => Render::LibRaw,
            Some(Source::Embedded) => Render::Embedded,
            Some(Source::Developed) | None if darktable => Render::Darktable,
            _ => Render::LibRaw,
        }
    }
}

/// Exporte-t-on des images déjà développées (JPEG boîtier, JPEG, TIFF…) plutôt que des
/// rendus de RAW ? Décide des profils proposés ([`tri::export::AppliesTo`]).
pub fn exports_flat(render: Render, images: &[PathBuf]) -> bool {
    export_kind(render, images) == tri::export::Kind::Flat
}

/// Ce que contient l'export de `images` vu en `render` : RAW, images déjà développées, ou
/// les deux (paires RAW + JPEG comprises, voir [`tri::export::Batch`]).
pub fn export_kind(render: Render, images: &[PathBuf]) -> tri::export::Kind {
    let embedded = render == Render::Embedded;
    tri::export::Batch::new(images, embedded).kind(embedded)
}

/// Profil proposé : `preferred` (le dernier choisi, ou celui d'une version), sinon celui
/// des réglages pour ce type d'export (`default_raw`, `default_flat`), sinon le premier ;
/// sa place dans `profiles`.
fn proposed(
    profiles: &[Profile],
    preferred: Option<&str>,
    settings: &tri::export::ExportSettings,
    kind: tri::export::Kind,
) -> usize {
    let default = if kind == tri::export::Kind::Flat {
        &settings.default_flat
    } else {
        &settings.default_raw
    };
    preferred
        .and_then(|id| profiles.iter().position(|p| p.id == id))
        .or_else(|| profiles.iter().position(|p| p.id == *default))
        .unwrap_or(0)
}

/// Boîte ouverte.
pub struct ExportDialog {
    /// Images à exporter (paires déjà réduites à leur RAW, voir [`tri::export::sources`]).
    images: Vec<PathBuf>,
    profiles: Vec<Profile>,
    /// Profil choisi.
    choice: usize,
    /// Dossier, en cours de saisie : celui du profil (voir [`Profile::folder`]), suivi
    /// tant qu'on ne l'a pas modifié à la main.
    input: String,
    /// Dossier modifié à la main : il ne suit plus le profil choisi.
    edited: bool,
    /// JPEG sans recompression (case de la boîte) et le profil dont elle vient : elle
    /// reprend la valeur du profil quand on en change (voir [`Profile::passthrough`]).
    passthrough: (bool, usize),
    field: crate::path_field::PathField,
    at: Pos2,
    /// Première image : le champ prend le focus.
    fresh: bool,
    /// darktable trouvé ; sinon export par LibRaw (rendu de base), voir
    /// [`tri::export::export_libraw`].
    darktable: bool,
    /// Image exportée : selon ce qu'on regarde, modifiable ici (voir [`Render::of`]).
    render: Render,
    /// Version exportée (recadrage), sinon l'image entière.
    cut: Option<tri::export::Cut>,
    /// Ce qui part, pour le dire sous le titre.
    batch: tri::export::Batch,
    /// Identifiant de la liste des profils (image précédente) : focus au départ, Entrée y
    /// exporte.
    combo: Option<egui::Id>,
}

/// Résultat d'une image de la boîte.
pub enum Outcome {
    /// Boîte toujours ouverte.
    Open,
    /// Annulée.
    Cancelled,
    /// Export demandé ; `folder` : celui du profil, ou un autre pour cette fois.
    Start {
        images: Vec<PathBuf>,
        profile: Box<Profile>,
        folder: String,
        render: Render,
        cut: Option<tri::export::Cut>,
    },
}

/// Identifiant du champ du dossier.
pub fn destination_field_id() -> egui::Id {
    egui::Id::unique("export_destination")
}

/// Nom affiché d'un profil : le sien, sinon le nom traduit des profils fournis, sinon
/// l'identifiant.
pub fn profile_name(p: &Profile) -> String {
    if !p.name.trim().is_empty() {
        return p.name.clone();
    }
    match p.id.as_str() {
        "edit" => t!("export-profile-edit"),
        "asis" => t!("export-profile-asis"),
        "web" => t!("export-profile-web"),
        id => id.to_string(),
    }
}

/// Résumé d'un profil : « TIFF 16 bits · ProPhoto (linéaire) · pleine définition ».
pub fn profile_summary(p: &Profile) -> String {
    // Copie telle quelle (sans recompression, espace d'origine, pleine taille) : la
    // qualité est celle du fichier d'origine, pas celle du profil (retour du 19/10/2026).
    let as_is = p.is_as_is();
    let format = match p.format {
        Format::Jpeg if as_is => "JPEG".to_string(),
        Format::Jpeg => t!("export-format-jpeg", quality = p.quality),
        Format::Tiff => t!("export-format-tiff", bits = p.bits),
        Format::Png => t!("export-format-png", bits = p.bits),
    };
    let color = match p.color.trim() {
        "" => t!("export-color-image"),
        "srgb" => "sRGB".into(),
        "adobergb" => "Adobe RGB".into(),
        "rec709" => t!("export-color-linear", name = "Rec709"),
        "rec2020" => t!("export-color-linear", name = "Rec2020"),
        "prophoto" => t!("export-color-linear", name = "ProPhoto"),
        "display-p3" => "Display P3".into(),
        file => Path::new(file)
            .file_name()
            .map_or_else(|| file.to_string(), |n| n.to_string_lossy().into_owned()),
    };
    let size = match p.max_side {
        0 => t!("export-size-full"),
        side => t!("export-size-max", side = side),
    };
    let sharpen = match p.sharpen {
        tri::export::Sharpen::None => String::new(),
        tri::export::Sharpen::Screen => format!(" · {}", t!("export-sharpen-screen")),
        tri::export::Sharpen::Print => format!(" · {}", t!("export-sharpen-print")),
    };
    format!("{format} · {color} · {size}{sharpen}")
}

impl ExportDialog {
    /// Boîte pour `images`, profil `preferred` proposé (identifiant ; voir [`proposed`]),
    /// avec son dossier ; `render` : l'image exportée (voir [`Render::of`]).
    pub fn new(
        images: &[PathBuf],
        settings: &tri::export::ExportSettings,
        preferred: Option<&str>,
        darktable: bool,
        render: Render,
        at: Pos2,
    ) -> ExportDialog {
        // RAW, images déjà développées, JPEG de paires (exportés aussi, retour du
        // 19/10/2026) ; seulement les profils qui s'appliquent à tout (`applies_to`).
        let embedded = render == Render::Embedded;
        let batch = tri::export::Batch::new(images, embedded);
        let kind = batch.kind(embedded);
        let profiles: Vec<Profile> = settings
            .profiles
            .0
            .iter()
            .filter(|p| kind.allows(p))
            .cloned()
            .collect();
        let choice = proposed(&profiles, preferred, settings, kind);
        let images: Vec<PathBuf> = batch.raws.iter().chain(&batch.flats).cloned().collect();
        ExportDialog {
            choice,
            input: profiles
                .get(choice)
                .map(|p| p.folder.clone())
                .unwrap_or_default(),
            edited: false,
            passthrough: (profiles.get(choice).is_some_and(|p| p.passthrough), choice),
            darktable,
            render,
            cut: None,
            batch,
            combo: None,
            images,
            profiles,
            field: Default::default(),
            at,
            fresh: true,
        }
    }

    /// Affiche la boîte. Flèches haut / bas sur la liste : préréglage ; Tab : contrôle
    /// suivant ; Entrée : exporter.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let n = self.images.len();
        let id = destination_field_id();
        // Flèches haut / bas : changent de préréglage quand la liste a le clavier, et
        // seulement là (retour du 20/10/2026 : depuis le dossier, elles changeaient le
        // préréglage, et depuis la liste, elles passaient au dossier). Tab passe d'un
        // contrôle à l'autre.
        let on_combo = self.combo.is_some_and(|id| ctx.memory(|m| m.has_focus(id)));
        let (up, down) = if on_combo {
            ctx.input_mut(|i| {
                (
                    i.consume_key(Modifiers::NONE, Key::ArrowUp),
                    i.consume_key(Modifiers::NONE, Key::ArrowDown),
                )
            })
        } else {
            (false, false)
        };
        // Entrée sur la liste des profils : exporter (sans ouvrir la liste).
        let enter_on_combo =
            on_combo && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        let last = self.profiles.len().saturating_sub(1);
        if up {
            self.choice = self.choice.saturating_sub(1);
        }
        if down {
            self.choice = (self.choice + 1).min(last);
        }
        let (mut confirm, mut cancel) = (false, false);
        let modal = crate::app::modal_at("export_dialog", self.at)
            .title(match &self.cut {
                Some(cut) => t!("export-title-version", name = cut.name.as_str()),
                None => t!("export-title", n = n),
            })
            .show(ctx, |ui| {
                ui.add_space(6.0);
                // Ce qui part (retour du 19/10/2026 : on ne savait pas quelle image).
                ui.weak(self.contents());
                ui.add_space(4.0);
                ui.label(t!("export-profile"));
                // Liste déroulante (retour du 19/10/2026 : avec beaucoup de profils, une
                // liste rendait la boîte trop haute) ; flèches haut / bas sans l'ouvrir.
                let style = ui.style().clone();
                let label = |p: &Profile, selected: bool| {
                    // Le nom, puis les réglages en petit et en gris (moins d'attraction).
                    let body = egui::TextStyle::Body.resolve(&style);
                    let small = egui::FontId::proportional(body.size * 0.85);
                    let strong = if selected {
                        style.visuals.selection.stroke.color
                    } else {
                        style.visuals.text_color()
                    };
                    let weak = strong.gamma_multiply(0.6);
                    let mut text = egui::text::LayoutJob::default();
                    text.append(
                        &profile_name(p),
                        0.0,
                        egui::TextFormat::simple(body, strong),
                    );
                    text.append(
                        &profile_summary(p),
                        10.0,
                        egui::TextFormat {
                            valign: egui::Align::Center,
                            ..egui::TextFormat::simple(small, weak)
                        },
                    );
                    text
                };
                let current = self
                    .profiles
                    .get(self.choice)
                    .map(|p| label(p, false))
                    .unwrap_or_default();
                let combo = egui::ComboBox::from_id_salt("export_profile")
                    .selected_text(current)
                    .width(420.0)
                    .show_ui(ui, |ui| {
                        for (i, p) in self.profiles.iter().enumerate() {
                            let selected = self.choice == i;
                            if ui.selectable_label(selected, label(p, selected)).clicked() {
                                self.choice = i;
                            }
                        }
                    })
                    .response
                    .on_hover_text(t!("export-profile-tip"));
                // Au départ, le clavier est à la liste des profils (flèches haut / bas ;
                // Tab : le dossier), retour du 19/10/2026 : rien ne le disait.
                if std::mem::take(&mut self.fresh) {
                    combo.request_focus();
                }
                self.combo = Some(combo.id);
                // Les flèches restent à la liste : egui ne s'en sert pas pour déplacer le
                // clavier vers le contrôle voisin.
                ui.memory_mut(|m| {
                    m.set_focus_lock_filter(
                        combo.id,
                        egui::EventFilter {
                            vertical_arrows: true,
                            ..Default::default()
                        },
                    );
                });
                // Le dossier suit le profil, tant qu'on ne l'a pas modifié.
                if !self.edited
                    && let Some(p) = self.profiles.get(self.choice)
                    && self.input != p.folder
                {
                    self.input = p.folder.clone();
                    self.field.replaced();
                }
                ui.add_space(6.0);
                ui.label(t!("export-destination"));
                let field = self.field.show(ui, id, &mut self.input, 420.0);
                if field.changed() {
                    self.edited = true;
                }
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                // Où ira la première image (relatif : à côté de chaque image).
                if let Some(first) = self.images.first() {
                    let dest = tri::export::folder_for(&self.input, first);
                    let mut text = t!("export-goes-to", dest = crate::files::tilde(&dest));
                    if !dest.is_dir() {
                        text.push_str(&format!(" ({})", t!("export-create")));
                    }
                    ui.weak(text);
                }
                ui.add_space(6.0);
                // L'image exportée est celle qu'on regarde (retour du 19/10/2026 : pas de
                // choix dans la boîte, Ctrl+E sur l'image voulue) ; dite ci-dessous.
                if !self.darktable && self.render == Render::LibRaw {
                    ui.weak(t!("export-no-darktable"));
                }
                // Sans darktable : ce que LibRaw et le JPEG boîtier ne savent pas faire.
                let local = match self.render {
                    Render::Darktable => None,
                    Render::LibRaw => Some(tri::export::Local::LibRaw),
                    Render::Embedded => Some(tri::export::Local::Embedded),
                };
                let unsupported = local
                    .zip(self.profiles.get(self.choice))
                    .and_then(|(local, p)| tri::export::libraw_check(p, local).err());
                // Sans recompression : JPEG, depuis un JPEG (boîtier, ou image plate).
                if self.passthrough.1 != self.choice {
                    let profile = self.profiles.get(self.choice);
                    self.passthrough = (profile.is_some_and(|p| p.passthrough), self.choice);
                }
                let jpeg = self
                    .profiles
                    .get(self.choice)
                    .is_some_and(|p| p.format == tri::export::Format::Jpeg);
                if jpeg && local.is_some() {
                    ui.checkbox(&mut self.passthrough.0, t!("export-passthrough"))
                        .on_hover_text(t!("export-passthrough-tip"));
                }
                if let Some(reason) = &unsupported {
                    ui.colored_label(ui.visuals().error_fg_color, reason);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let ok = n > 0 && !self.profiles.is_empty() && unsupported.is_none();
                    confirm = ui
                        .add_enabled(ok, egui::Button::new(t!("export-button")))
                        .clicked()
                        || ((enter || enter_on_combo) && ok);
                    cancel = ui.button(t!("cancel")).clicked();
                    ui.weak(t!("enter-or-escape"));
                });
            });
        if confirm {
            return Outcome::Start {
                images: std::mem::take(&mut self.images),
                profile: Box::new(tri::export::Profile {
                    passthrough: self.passthrough.0,
                    ..self.profiles[self.choice].clone()
                }),
                folder: self.input.trim().to_string(),
                render: self.render,
                cut: self.cut.clone(),
            };
        }
        if cancel || modal.should_close() {
            Outcome::Cancelled
        } else {
            Outcome::Open
        }
    }

    /// Ce qui part, en clair : « 1 RAW, développé par darktable · 1 JPEG de l'appareil
    /// (nom suivi de -camera) ».
    fn contents(&self) -> String {
        let b = &self.batch;
        let mut parts = Vec::new();
        if !b.raws.is_empty() {
            let n = b.raws.len();
            parts.push(match self.render {
                Render::Embedded => t!("export-contents-embedded", n = n),
                Render::LibRaw => t!("export-contents-libraw", n = n),
                Render::Darktable => t!("export-contents-darktable", n = n),
            });
        }
        let alone = b.flats.len() - b.paired.len();
        if alone > 0 {
            parts.push(t!("export-contents-flat", n = alone));
        }
        if !b.paired.is_empty() {
            parts.push(t!("export-contents-paired", n = b.paired.len()));
        }
        parts.join(" · ")
    }

    /// Exporter la version `cut` (recadrage) plutôt que l'image entière.
    pub fn with_cut(mut self, cut: tri::export::Cut) -> ExportDialog {
        self.cut = Some(cut);
        // Une version recadrée n'est jamais « telle quelle » : pas de JPEG tel quel. Le
        // profil choisi est retrouvé par son identifiant dans la liste réduite.
        let id = self.profiles.get(self.choice).map(|p| p.id.clone());
        self.profiles.retain(|p| !p.is_as_is());
        // « JPEG tel quel » retiré : le premier profil JPEG (Web par défaut), plus naturel
        // pour un recadrage qu'un TIFF de retouche.
        self.choice = id
            .and_then(|id| self.profiles.iter().position(|p| p.id == id))
            .or_else(|| {
                self.profiles
                    .iter()
                    .position(|p| p.format == tri::export::Format::Jpeg)
            })
            .unwrap_or(0);
        if let Some(p) = self.profiles.get(self.choice) {
            self.input = p.folder.clone();
            self.passthrough = (p.passthrough, self.choice);
        }
        self
    }

    #[cfg(test)]
    /// Profil choisi (retenu pour la prochaine boîte).
    pub fn choice(&self) -> usize {
        self.choice
    }

    /// Image exportée choisie.
    #[cfg(test)]
    pub fn render(&self) -> Render {
        self.render
    }

    /// « JPEG tel quel » proposé.
    #[cfg(test)]
    pub fn lists_as_is(&self) -> bool {
        self.profiles.iter().any(Profile::is_as_is)
    }

    /// Case « sans recompression ».
    #[cfg(test)]
    pub fn passthrough(&self) -> bool {
        self.passthrough.0
    }
}

/// Ce qu'un export doit produire (images, profil, dossier, version, accentuation).
pub struct Order {
    /// Images.
    pub images: Vec<PathBuf>,
    /// Profil.
    pub profile: Profile,
    /// Dossier (celui du profil, ou un autre pour cette fois).
    pub folder: String,
    /// Version (recadrage), sinon l'image entière.
    pub cut: Option<tri::export::Cut>,
    /// Paramètres de l'accentuation de sortie.
    pub sharpening: tri::export::Sharpening,
    /// Rendu LibRaw (`[render]`), le même qu'à l'écran.
    pub render: tri::raw::RenderOptions,
    /// Suffixe du JPEG d'une paire exporté avec son RAW.
    pub pair_suffix: String,
}

/// Export en cours, dans un fil à part (darktable : plusieurs secondes par image).
pub struct Job {
    pub done: Arc<AtomicUsize>,
    pub total: usize,
    pub dest: PathBuf,
    pub profile: String,
    /// Version exportée (recadrage), sinon l'image entière.
    pub version: Option<String>,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<Report, DevelopError>>>,
}

impl Job {
    /// Lance l'export : par darktable (`dt`), sinon depuis `local` ; `ctx` est redessiné à
    /// chaque fichier écrit.
    pub fn start(
        dt: Option<Darktable>,
        local: tri::export::Local,
        order: Order,
        ctx: &egui::Context,
    ) -> Job {
        let Order {
            images,
            profile,
            folder,
            cut,
            sharpening,
            render,
            pair_suffix,
        } = order;
        // Dossier montré : celui de la première image (relatif : un par dossier d'images).
        let dest = images
            .first()
            .map(|first| tri::export::folder_for(&folder, first))
            .unwrap_or_default();
        let done = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let total = images.len();
        let name = profile_name(&profile);
        let version = cut.as_ref().map(|c| c.name.clone());
        let handle = {
            let (done, cancel, ctx) = (done.clone(), cancel.clone(), ctx.clone());
            std::thread::spawn(move || {
                let progress = |n: usize, _: usize, _: &Path| {
                    done.store(n, Ordering::Relaxed);
                    ctx.request_repaint();
                };
                let req = tri::export::Request::new(&profile, &folder)
                    .cut(cut.as_ref())
                    .sharpening(sharpening)
                    .render(render);
                let embedded = local == tri::export::Local::Embedded && dt.is_none();
                let batch = tri::export::Batch::new(&images, embedded);
                let renderer = match &dt {
                    Some(dt) => tri::export::Renderer::Darktable(dt),
                    None => tri::export::Renderer::Local(local),
                };
                let result =
                    tri::export::run(&batch, renderer, &req, &pair_suffix, &cancel, progress);
                ctx.request_repaint();
                result
            })
        };
        Job {
            done,
            total,
            dest,
            profile: name,
            version,
            cancel,
            handle: Some(handle),
        }
    }

    /// L'export fini, pour le rapport d'export ([`tri::export_log`]).
    pub fn entry(&self, result: &Result<Report, DevelopError>) -> tri::export_log::Entry {
        let entry = tri::export_log::Entry::now(&self.profile, self.version.as_deref(), self.total);
        match result {
            Ok(report) => entry.with_report(report),
            Err(DevelopError::Cancelled) => tri::export_log::Entry {
                stopped: Some(tri::export_log::Stop::Cancelled),
                ..entry
            },
            Err(e) => tri::export_log::Entry {
                stopped: Some(tri::export_log::Stop::Failed(e.to_string())),
                ..entry
            },
        }
    }

    /// Arrête darktable (les fichiers déjà écrits restent).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Résultat, une fois fini.
    pub fn finished(&mut self) -> Option<Result<Report, DevelopError>> {
        if !self.handle.as_ref()?.is_finished() {
            return None;
        }
        let handle = self.handle.take()?;
        Some(handle.join().unwrap_or_else(|_| {
            Err(DevelopError::Failed(
                tri::develop::Engine::Darktable,
                "export".into(),
            ))
        }))
    }
}

/// Ce que demande le rapport d'export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReportAction {
    /// Montrer ce fichier (et donc son dossier) dans le gestionnaire de fichiers.
    Reveal(PathBuf),
    /// Effacer le journal.
    Clear,
    /// Fermer le rapport.
    Close,
}

/// « il y a 3 min » : âge d'un export, en secondes.
fn ago(seconds: u64) -> String {
    let n = |unit: u64| seconds / unit;
    match seconds {
        0..60 => t!("export-report-now"),
        60..3_600 => t!("export-report-minutes", n = n(60)),
        3_600..86_400 => t!("export-report-hours", n = n(3_600)),
        86_400..2_592_000 => t!("export-report-days", n = n(86_400)),
        2_592_000..31_536_000 => t!("export-report-months", n = n(2_592_000)),
        _ => t!("export-report-years", n = n(31_536_000)),
    }
}

/// Rapport d'export (`Ctrl+Maj+E`, idée de l'utilisateur du 20/10/2026) : les exports
/// terminés, les plus récents en haut, dans une liste à hauteur limitée ; pour chacun, le
/// préréglage, ce qui a été écrit, et ses dossiers (un clic les montre dans le
/// gestionnaire de fichiers) ; les fichiers et les échecs se déplient. `now` : secondes
/// depuis 1970.
pub fn report_dialog(
    ctx: &egui::Context,
    entries: &[tri::export_log::Entry],
    now: u64,
) -> Option<ReportAction> {
    let mut action = None;
    let size = ctx.content_rect().size();
    let at = ctx.content_rect().center() - egui::vec2(280.0, size.y * 0.3);
    let modal = crate::app::modal_at("export_report", at)
        .title(t!("export-report-title"))
        .show(ctx, |ui| {
            ui.set_width(560.0);
            ui.add_space(4.0);
            if entries.is_empty() {
                ui.weak(t!("export-report-empty"));
            }
            egui::ScrollArea::vertical()
                .max_height(size.y * 0.55)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (i, entry) in entries.iter().enumerate().rev() {
                        if i + 1 < entries.len() {
                            ui.separator();
                        }
                        if let Some(a) = report_entry(ui, i, entry, now) {
                            action = Some(a);
                        }
                    }
                });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                // Effacer la liste : petit balai à droite, loin de Fermer (retour du
                // 20/10/2026).
                if !entries.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let size = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
                        let label = t!("export-report-clear");
                        let (rect, response) = crate::marks::icon_button(ui, size, &label, &label);
                        let color = ui.style().interact(&response).fg_stroke.color;
                        crate::marks::broom(ui.painter(), rect.shrink(size * 0.18), color);
                        if response.clicked() {
                            action = Some(ReportAction::Clear);
                        }
                    });
                }
            });
        });
    if modal.should_close() {
        action = Some(ReportAction::Close);
    }
    action
}

/// Un export du rapport ; `i` : son rang (identifiants des parties dépliables).
fn report_entry(
    ui: &mut egui::Ui,
    i: usize,
    entry: &tri::export_log::Entry,
    now: u64,
) -> Option<ReportAction> {
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        ui.strong(&entry.preset);
        if let Some(version) = &entry.version {
            ui.label(t!("export-report-version", name = version.as_str()));
        }
        ui.weak(t!(
            "export-report-summary",
            written = entry.written.len(),
            total = entry.total
        ));
        if entry.passthrough > 0 {
            ui.weak(t!("export-report-passthrough", n = entry.passthrough));
        }
        ui.weak(ago(entry.age(now)));
    });
    match &entry.stopped {
        Some(tri::export_log::Stop::Cancelled) => {
            ui.weak(t!("export-cancelled"));
        }
        Some(tri::export_log::Stop::Failed(message)) => {
            ui.colored_label(ui.visuals().error_fg_color, message);
        }
        None => {}
    }
    for (dir, files) in entry.folders() {
        let first = files[0].clone();
        let link = ui
            .link(crate::files::tilde(&dir))
            .on_hover_text(t!("export-report-reveal"));
        if link.clicked() {
            action = Some(ReportAction::Reveal(first));
        }
        egui::CollapsingHeader::new(t!("export-report-files", n = files.len()))
            .id_salt(("export_report_files", i, &dir))
            .show(ui, |ui| {
                for file in files {
                    let name = file
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                    if ui
                        .link(name)
                        .on_hover_text(t!("export-report-reveal"))
                        .clicked()
                    {
                        action = Some(ReportAction::Reveal(file.clone()));
                    }
                }
            });
    }
    if !entry.errors.is_empty() {
        let title = egui::RichText::new(t!("export-report-errors", n = entry.errors.len()))
            .color(ui.visuals().error_fg_color);
        egui::CollapsingHeader::new(title)
            .id_salt(("export_report_errors", i))
            .show(ui, |ui| {
                for (image, error) in &entry.errors {
                    let name = image
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                    ui.label(format!("{name} : {error}"));
                }
            });
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::Source;

    #[test]
    fn what_is_exported_is_what_is_looked_at() {
        // Grille : darktable s'il est là, sinon LibRaw.
        assert_eq!(Render::of(None, true), Render::Darktable);
        assert_eq!(Render::of(None, false), Render::LibRaw);
        // Plein écran : la source affichée.
        assert_eq!(Render::of(Some(Source::Embedded), true), Render::Embedded);
        assert_eq!(Render::of(Some(Source::Raw), true), Render::LibRaw);
        assert_eq!(Render::of(Some(Source::Developed), true), Render::Darktable);
        assert_eq!(Render::of(Some(Source::Developed), false), Render::LibRaw);
        // « JPEG tel quel » (`applies_to = "flat"`) : seulement pour des images développées.
        let raw = [PathBuf::from("a/DSCF1.RAF")];
        let jpeg = [PathBuf::from("a/DSCF1.JPG")];
        assert!(!exports_flat(Render::LibRaw, &raw));
        assert!(!exports_flat(Render::Darktable, &raw));
        assert!(exports_flat(Render::Embedded, &raw));
        assert!(exports_flat(Render::LibRaw, &jpeg));
        let settings = tri::export::ExportSettings::default();
        let dialog = |render, images: &[PathBuf]| {
            ExportDialog::new(images, &settings, None, true, render, Pos2::ZERO)
        };
        assert!(!dialog(Render::Darktable, &raw).lists_as_is());
        assert!(dialog(Render::Embedded, &raw).lists_as_is());
    }

    #[test]
    fn arrows_change_the_preset_only_on_the_list() {
        crate::i18n::ensure();
        let images = [PathBuf::from("a/DSCF1.RAF")];
        let settings = tri::export::ExportSettings::default();
        let dialog = ExportDialog::new(&images, &settings, None, true, Render::LibRaw, Pos2::ZERO);
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(700.0, 500.0))
            .with_accessibility_check(false)
            .build_ui_state(
                |ui, d: &mut ExportDialog| {
                    d.show(ui.ctx());
                },
                dialog,
            );
        h.run_steps(3);
        let start = h.state().choice;
        assert!(h.state().profiles.len() > 1, "au moins deux préréglages");
        // Sur la liste : la flèche change de préréglage, la liste garde le clavier.
        let key = if start == 0 {
            Key::ArrowDown
        } else {
            Key::ArrowUp
        };
        h.key_press(key);
        h.run_steps(2);
        let moved = h.state().choice;
        assert_ne!(moved, start);
        let combo = h.state().combo;
        assert_eq!(
            h.ctx.memory(|m| m.focused()),
            combo,
            "clavier resté à la liste"
        );
        // Tab : le dossier ; les flèches n'y changent plus le préréglage.
        h.key_press(Key::Tab);
        h.run_steps(2);
        assert_eq!(h.ctx.memory(|m| m.focused()), Some(destination_field_id()));
        h.key_press(Key::ArrowUp);
        h.key_press(Key::ArrowDown);
        h.run_steps(2);
        assert_eq!(
            h.state().choice,
            moved,
            "préréglage inchangé depuis le dossier"
        );
    }
}
