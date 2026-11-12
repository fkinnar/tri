//! Résumé du dossier ouvert, en bas du panneau des dossiers (issue n° 388, décidée le
//! 09/11/2026) : de quoi le dossier est fait (photos par type, paires RAW+JPEG,
//! développements par logiciel), où en est le tri, les conflits de notes et où vivent les
//! données de tri. Sur **tout** le dossier, pas la vue filtrée ([`tri::folder::Summary`]) :
//! cliquer une ligne ne change pas les autres.
//!
//! Chaque ligne est un filtre (12/11/2026) : un clic montre **ce filtre seul**, à la place
//! des autres ; un second clic sur la ligne allumée les retire tous. Pas encore de
//! Ctrl+clic pour combiner (avec les filtres complexes, #138). La ligne des paires n'est
//! pas un filtre tant que les paires ne sont pas regroupées (#392).

use super::*;
use tri::develop::Engine;
use tri::notes::{ColorRule, StarsRule};

/// Clé de la section dépliée dans l'état d'affichage (`panel_folded` garde les sections
/// repliées ; celle-ci est repliée par défaut, d'où une clé « ouverte »).
pub(super) const OPEN: &str = "folders-summary-open";

/// Une ligne du résumé, comme filtre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Line {
    /// Toutes les photos : aucun filtre.
    Photos,
    /// Les photos d'une extension.
    Extension(String),
    /// Les développements, de tous les logiciels actifs (`None`) ou d'un seul.
    Developments(Option<Engine>),
    /// Sans aucune note.
    Untouched,
    /// Avec des étoiles.
    Starred,
    /// Acceptées.
    Accepted,
    /// Rejetées.
    Rejected,
    /// Avec un libellé couleur, n'importe lequel (`None`) ou celui-ci.
    Colored(Option<Color>),
    /// Notes en conflit.
    Conflicts,
}

/// Ce que les lignes du résumé changent : les filtres de la vue, sans l'ordre, le rendu ni
/// la position.
#[derive(Clone, Debug, PartialEq)]
struct Filters {
    kind: KindFilter,
    notes: NotesRule,
    blur: bool,
    well_exposed: bool,
    developed: bool,
    developed_checked: Option<Vec<Engine>>,
    inverted: bool,
    conflicts: bool,
    extension: Option<String>,
    stack_only: bool,
}

impl Filters {
    /// Aucun filtre.
    fn none() -> Filters {
        Filters {
            kind: KindFilter::All,
            notes: NotesRule::default(),
            blur: false,
            well_exposed: false,
            developed: false,
            developed_checked: None,
            inverted: false,
            conflicts: false,
            extension: None,
            stack_only: false,
        }
    }

    /// Le filtre de `line`, seul.
    fn of(line: &Line) -> Filters {
        let mut f = Filters::none();
        match line {
            Line::Photos => {}
            Line::Extension(e) => f.extension = Some(e.clone()),
            Line::Developments(engine) => {
                f.developed = true;
                f.developed_checked = engine.map(|e| vec![e]);
            }
            Line::Untouched => {
                f.notes = NotesRule {
                    stars: Some(StarsRule::Exactly(0)),
                    flags: Some(vec![Flag::None]),
                    colors: Some(ColorRule {
                        any_of: Vec::new(),
                        none: true,
                    }),
                }
            }
            Line::Starred => f.notes.stars = Some(StarsRule::AtLeast(1)),
            Line::Accepted => f.notes.flags = Some(vec![Flag::Accepted]),
            Line::Rejected => f.notes.flags = Some(vec![Flag::Rejected]),
            Line::Colored(color) => {
                f.notes.colors = Some(ColorRule {
                    any_of: match color {
                        Some(c) => vec![*c],
                        None => Color::ALL.to_vec(),
                    },
                    none: false,
                })
            }
            Line::Conflicts => f.conflicts = true,
        }
        f
    }
}

/// Une ligne à dessiner.
struct Row {
    label: String,
    count: usize,
    /// Détail (une extension, un logiciel) : en retrait.
    indent: bool,
    /// Son filtre ; `None` : pas cliquable.
    line: Option<Line>,
    /// Couleur propre (avertissement, grisé).
    color: Option<Color32>,
    /// Bulle propre (sinon : celle du filtre).
    tip: Option<String>,
}

impl TriApp {
    /// Les filtres de la vue, tels que le résumé les compare.
    fn summary_filters(&self) -> Filters {
        Filters {
            kind: self.filter,
            notes: self.notes_rule.clone(),
            blur: self.blur_rule.is_some(),
            well_exposed: self.well_exposed,
            developed: self.developed_on,
            developed_checked: self.developed_checked.clone(),
            inverted: self.filter_inverted,
            conflicts: self.conflicts_only,
            extension: self.extension_only.clone(),
            stack_only: self.stack_only.is_some(),
        }
    }

    /// Vrai si la vue montre exactement le filtre de `line` (la ligne est allumée).
    /// « Photos » n'est jamais allumée : c'est l'absence de filtre.
    pub(super) fn summary_line_on(&self, line: &Line) -> bool {
        *line != Line::Photos && self.summary_filters() == Filters::of(line)
    }

    /// Clic sur une ligne du résumé : ce filtre seul, à la place des autres ; déjà seul,
    /// plus aucun filtre.
    pub(super) fn summary_click(&mut self, line: &Line) {
        let wanted = if self.summary_line_on(line) {
            Filters::none()
        } else {
            Filters::of(line)
        };
        if self.stack_only.is_some() {
            self.leave_stack_only();
        }
        self.filter = wanted.kind;
        self.notes_rule = wanted.notes;
        self.blur_rule = None;
        self.well_exposed = wanted.well_exposed;
        self.developed_on = wanted.developed;
        self.developed_checked = wanted.developed_checked;
        self.filter_inverted = wanted.inverted;
        self.conflicts_only = wanted.conflicts;
        self.extension_only = wanted.extension;
        self.apply_filter();
    }

    /// Section « Ce dossier » : titre repliable, puis les comptes, chacun un filtre.
    pub(super) fn folder_summary(&mut self, ui: &mut Ui, f: &crate::settings::FolderSettings) {
        let open = self.panel_folded.contains(OPEN);
        if section_title(ui, &t!("summary-title"), open, f) {
            if open {
                self.panel_folded.remove(OPEN);
            } else {
                self.panel_folded.insert(OPEN.to_string());
            }
        }
        if !open {
            return;
        }
        let rows = self.summary_rows(ui.visuals());
        let accent = marks::accent_mark(ui.ctx());
        let mut clicked: Option<Line> = None;
        egui::Grid::new("folder_summary")
            .num_columns(2)
            .spacing(vec2(12.0, 2.0))
            .show(ui, |ui| {
                for row in rows {
                    let on = row.line.as_ref().is_some_and(|l| self.summary_line_on(l));
                    let mut text = egui::RichText::new(&row.label);
                    let mut count = egui::RichText::new(row.count.to_string());
                    if let Some(c) = row.color {
                        text = text.color(c);
                        count = count.color(c);
                    }
                    // Allumée : le filtre montré.
                    if on {
                        text = text.color(accent).underline();
                        count = count.color(accent);
                    }
                    let sense = if row.line.is_some() {
                        Sense::click()
                    } else {
                        Sense::hover()
                    };
                    let label = ui
                        .horizontal(|ui| {
                            if row.indent {
                                ui.add_space(12.0);
                            }
                            ui.add(egui::Label::new(text).sense(sense))
                        })
                        .inner;
                    let number = ui.add(egui::Label::new(count).sense(sense));
                    let tip = row.tip.clone().or_else(|| {
                        row.line.as_ref().map(|_| {
                            if on {
                                t!("summary-filter-off")
                            } else {
                                t!("summary-filter-on")
                            }
                        })
                    });
                    let (label, number) = match &tip {
                        Some(tip) => (label.on_hover_text(tip), number.on_hover_text(tip)),
                        None => (label, number),
                    };
                    if row.line.is_some() {
                        let label = label.on_hover_cursor(egui::CursorIcon::PointingHand);
                        let number = number.on_hover_cursor(egui::CursorIcon::PointingHand);
                        if label.clicked() || number.clicked() {
                            clicked = row.line.clone();
                        }
                    }
                    ui.end_row();
                }
            });
        // Où vivent les données de tri (cache, filtres, curseurs…) de ce dossier (#378).
        let dir = &self.folder.dir;
        let place = match tri::store::holder(dir) {
            Some(root) if root == *dir => t!("summary-data-folder"),
            Some(root) => t!("summary-data-disk", path = root.display().to_string()),
            None => t!("summary-data-machine"),
        };
        ui.weak(place).on_hover_text(t!("folder-menu-cache-tip"));
        if let Some(line) = clicked {
            self.summary_click(&line);
        }
    }

    /// Les lignes du résumé. Peu coûteux (quelques milliers de recherches en mémoire) :
    /// refait à chaque image affichée, toujours à jour des notes et des développements.
    fn summary_rows(&self, visuals: &egui::Visuals) -> Vec<Row> {
        let summary = self.folder.summary();
        let engines = self.developers.engines();
        let row = |label: String, count: usize, line: Option<Line>| Row {
            label,
            count,
            indent: false,
            line,
            color: None,
            tip: None,
        };
        let mut rows = vec![Row {
            tip: Some(t!("summary-photos-tip")),
            ..row(t!("summary-photos"), summary.photos, Some(Line::Photos))
        }];
        for t in &summary.types {
            rows.push(Row {
                indent: true,
                ..row(
                    t.extension.clone(),
                    t.count,
                    Some(Line::Extension(t.extension.clone())),
                )
            });
        }
        if summary.pairs > 0 {
            rows.push(Row {
                tip: Some(t!("summary-pairs-tip")),
                ..row(t!("summary-pairs"), summary.pairs, None)
            });
        }
        if !summary.developments.is_empty() {
            let total = summary.developments.iter().map(|(_, n)| n).sum();
            let line = (!engines.is_empty()).then_some(Line::Developments(None));
            rows.push(row(t!("summary-developments"), total, line));
            for &(engine, n) in &summary.developments {
                // Logiciel absent ou désactivé : ses développements ne sont pas dans la
                // grille (#386), grisés ici avec la raison, sans filtre.
                let active = engines.contains(&engine);
                rows.push(Row {
                    indent: true,
                    color: (!active).then_some(visuals.weak_text_color()),
                    tip: (!active).then(|| t!("summary-engine-off", engine = engine.name())),
                    ..row(
                        engine.name().to_string(),
                        n,
                        active.then_some(Line::Developments(Some(engine))),
                    )
                });
            }
        }
        rows.push(row(
            t!("summary-untouched"),
            summary.untouched,
            Some(Line::Untouched),
        ));
        rows.push(row(
            t!("summary-starred"),
            summary.starred,
            Some(Line::Starred),
        ));
        rows.push(row(
            t!("summary-accepted"),
            summary.accepted,
            Some(Line::Accepted),
        ));
        rows.push(row(
            t!("summary-rejected"),
            summary.rejected,
            Some(Line::Rejected),
        ));
        // Libellés couleur (retour du 12/11/2026) : le total, puis chaque couleur présente.
        rows.push(row(
            t!("summary-colored"),
            summary.colored,
            Some(Line::Colored(None)),
        ));
        for &(color, n) in &summary.colors {
            rows.push(Row {
                indent: true,
                ..row(color_name(color), n, Some(Line::Colored(Some(color))))
            });
        }
        // Conflits : seulement s'il y en a, en couleur d'avertissement.
        if summary.conflicts > 0 {
            rows.push(Row {
                color: Some(visuals.warn_fg_color),
                ..row(
                    t!("summary-conflicts"),
                    summary.conflicts,
                    Some(Line::Conflicts),
                )
            });
        }
        rows
    }
}
