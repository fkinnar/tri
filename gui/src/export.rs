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

/// Exporte-t-on des JPEG (le JPEG boîtier, ou des JPEG d'origine) plutôt qu'un rendu ?
pub fn exports_jpeg(render: Render, images: &[PathBuf]) -> bool {
    let jpeg = |p: &PathBuf| {
        p.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
    };
    render == Render::Embedded || (!images.is_empty() && images.iter().all(jpeg))
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
    let format = match p.format {
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
    format!("{format} · {color} · {size}")
}

impl ExportDialog {
    /// Boîte pour `images`, profil `choice` présélectionné, avec son dossier ; `render` :
    /// l'image proposée (voir [`Render::of`]).
    pub fn new(
        images: &[PathBuf],
        settings: &tri::export::ExportSettings,
        choice: usize,
        darktable: bool,
        render: Render,
        at: Pos2,
    ) -> ExportDialog {
        let images = tri::export::sources(images);
        let profiles = settings.profiles.0.clone();
        let choice = choice.min(profiles.len().saturating_sub(1));
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
            images,
            profiles,
            field: Default::default(),
            at,
            fresh: true,
        }
    }

    /// Affiche la boîte. Flèches haut / bas : profil ; Entrée : exporter.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let n = self.images.len();
        let id = destination_field_id();
        // Flèches haut / bas : changent de profil même depuis le champ (Tab y complète le
        // chemin).
        let (up, down) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
            )
        });
        let last = self.profiles.len().saturating_sub(1);
        if up {
            self.choice = self.choice.saturating_sub(1);
        }
        if down {
            self.choice = (self.choice + 1).min(last);
        }
        let (mut confirm, mut cancel) = (false, false);
        let modal = crate::app::modal_at("export_dialog", self.at)
            .title(t!("export-title", n = n))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.label(t!("export-profile"));
                for (i, p) in self.profiles.iter().enumerate() {
                    // Le nom, puis les réglages en petit et en gris (retour du 19/10/2026 :
                    // moins d'attraction pour l'œil).
                    let mut text = egui::text::LayoutJob::default();
                    let style = ui.style();
                    let body = egui::TextStyle::Body.resolve(style);
                    let small = egui::FontId::proportional(body.size * 0.85);
                    let selected = self.choice == i;
                    let strong = if selected {
                        style.visuals.selection.stroke.color
                    } else {
                        style.visuals.text_color()
                    };
                    let weak = strong.gamma_multiply(0.6);
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
                    let row = ui.selectable_label(selected, text);
                    if row.clicked() {
                        self.choice = i;
                    }
                }
                // Le dossier suit le profil, tant qu'on ne l'a pas modifié.
                if !self.edited
                    && let Some(p) = self.profiles.get(self.choice)
                    && self.input != p.folder
                {
                    self.input = p.folder.clone();
                    self.field.replaced();
                }
                ui.weak(t!("export-profile-hint"));
                ui.add_space(6.0);
                ui.label(t!("export-destination"));
                let field = self.field.show(ui, id, &mut self.input, 420.0);
                if std::mem::take(&mut self.fresh) {
                    field.request_focus();
                }
                if field.changed() {
                    self.edited = true;
                }
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                ui.weak(t!("export-folder-hint"));
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
                ui.weak(match self.render {
                    Render::Darktable => t!("export-how"),
                    Render::LibRaw => t!("export-how-libraw"),
                    Render::Embedded => t!("export-how-embedded"),
                });
                if let Some(reason) = &unsupported {
                    ui.colored_label(ui.visuals().error_fg_color, reason);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let ok = n > 0 && !self.profiles.is_empty() && unsupported.is_none();
                    confirm = ui
                        .add_enabled(ok, egui::Button::new(t!("export-button")))
                        .clicked()
                        || (enter && ok);
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
            };
        }
        if cancel || modal.should_close() {
            Outcome::Cancelled
        } else {
            Outcome::Open
        }
    }

    /// Profil choisi (retenu pour la prochaine boîte).
    pub fn choice(&self) -> usize {
        self.choice
    }

    /// Image exportée choisie.
    #[cfg(test)]
    pub fn render(&self) -> Render {
        self.render
    }

    /// Case « sans recompression ».
    #[cfg(test)]
    pub fn passthrough(&self) -> bool {
        self.passthrough.0
    }
}

/// Export en cours, dans un fil à part (darktable : plusieurs secondes par image).
pub struct Job {
    pub done: Arc<AtomicUsize>,
    pub total: usize,
    pub dest: PathBuf,
    pub profile: String,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<Report, DevelopError>>>,
}

impl Job {
    /// Lance l'export : par darktable (`dt`), sinon depuis `local` ; `ctx` est redessiné à
    /// chaque fichier écrit.
    pub fn start(
        dt: Option<Darktable>,
        local: tri::export::Local,
        images: Vec<PathBuf>,
        profile: Profile,
        folder: String,
        ctx: &egui::Context,
    ) -> Job {
        // Dossier montré : celui de la première image (relatif : un par dossier d'images).
        let dest = images
            .first()
            .map(|first| tri::export::folder_for(&folder, first))
            .unwrap_or_default();
        let done = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let total = images.len();
        let name = profile_name(&profile);
        let handle = {
            let (done, cancel, ctx) = (done.clone(), cancel.clone(), ctx.clone());
            std::thread::spawn(move || {
                let progress = |n: usize, _: usize, _: &Path| {
                    done.store(n, Ordering::Relaxed);
                    ctx.request_repaint();
                };
                let result = match &dt {
                    Some(dt) => {
                        tri::export::export(dt, &images, &profile, &folder, &cancel, progress)
                    }
                    None => tri::export::export_libraw(
                        &images, &profile, &folder, local, &cancel, progress,
                    ),
                };
                ctx.request_repaint();
                result
            })
        };
        Job {
            done,
            total,
            dest,
            profile: name,
            cancel,
            handle: Some(handle),
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
        Some(
            handle
                .join()
                .unwrap_or_else(|_| Err(DevelopError::Failed("export".into()))),
        )
    }
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
    }
}
