//! Mise en forme du Markdown des notes de version (issue n° 3, 22/10/2026) : la boîte
//! « Mise à jour » montrait les `**` et les tirets tels quels. Seulement ce qu'écrit le
//! CHANGELOG — titres, listes à tirets (lignes suivantes en retrait), paragraphes, gras
//! et code en ligne — sans dépendance : une bibliothèque Markdown complète pour egui
//! serait bien plus lourde que le besoin.

use eframe::egui;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{FontId, TextStyle, Ui};

/// Un bloc du texte.
#[derive(Debug, PartialEq, Eq)]
pub enum Block {
    /// Titre (`## …`).
    Heading(String),
    /// Élément de liste (`- …` ou `* …`), lignes de suite comprises.
    Bullet(String),
    /// Paragraphe, lignes jointes.
    Paragraph(String),
}

/// Un morceau d'une ligne.
#[derive(Debug, PartialEq, Eq)]
pub enum Span {
    /// Texte ordinaire.
    Text(String),
    /// `**gras**`.
    Bold(String),
    /// `` `code` ``.
    Code(String),
}

/// Blocs d'un texte Markdown. Les lignes de HTML (`<details>`…) sont ignorées.
pub fn blocks(text: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut open = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('<') {
            open = false;
            continue;
        }
        if trimmed.starts_with('#') {
            out.push(Block::Heading(
                trimmed.trim_start_matches('#').trim().into(),
            ));
            open = false;
        } else if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            out.push(Block::Bullet(rest.trim().into()));
            open = true;
        } else if let Some(Block::Bullet(s) | Block::Paragraph(s)) = out.last_mut().filter(|_| open)
        {
            s.push(' ');
            s.push_str(trimmed);
        } else {
            out.push(Block::Paragraph(trimmed.into()));
            open = true;
        }
    }
    out
}

/// Morceaux d'une ligne : `**gras**` et `` `code` ``. Une marque sans sa fermeture reste
/// du texte.
pub fn spans(line: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut rest = line;
    while !rest.is_empty() {
        let found = [("**", true), ("`", false)]
            .into_iter()
            .filter_map(|(mark, bold)| {
                let start = rest.find(mark)?;
                let end = rest[start + mark.len()..].find(mark)? + start + mark.len();
                Some((start, end, mark.len(), bold))
            })
            .min_by_key(|&(start, ..)| start);
        let Some((start, end, len, bold)) = found else {
            text.push_str(rest);
            break;
        };
        text.push_str(&rest[..start]);
        if !text.is_empty() {
            out.push(Span::Text(std::mem::take(&mut text)));
        }
        let inner = rest[start + len..end].to_string();
        out.push(if bold {
            Span::Bold(inner)
        } else {
            Span::Code(inner)
        });
        rest = &rest[end + len..];
    }
    if !text.is_empty() {
        out.push(Span::Text(text));
    }
    out
}

/// Affiche `text` mis en forme, en couleur atténuée (gras : couleur du texte fort).
pub fn show(ui: &mut Ui, text: &str) {
    for block in blocks(text) {
        match block {
            Block::Heading(s) => {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(s).strong());
            }
            Block::Paragraph(s) => {
                ui.add(egui::Label::new(job(ui, &s)).wrap());
            }
            Block::Bullet(s) => {
                ui.horizontal_top(|ui| {
                    ui.weak("•");
                    ui.add(egui::Label::new(job(ui, &s)).wrap());
                });
            }
        }
        ui.add_space(2.0);
    }
}

/// Une ligne mise en forme.
fn job(ui: &Ui, line: &str) -> LayoutJob {
    let visuals = ui.visuals();
    let body = TextStyle::Body.resolve(ui.style());
    let mono = FontId::monospace(body.size * 0.92);
    let weak = visuals.weak_text_color();
    let strong = visuals.strong_text_color();
    let mut job = LayoutJob::default();
    for span in spans(line) {
        let (text, format) = match span {
            Span::Text(t) => (t, TextFormat::simple(body.clone(), weak)),
            Span::Bold(t) => (t, TextFormat::simple(body.clone(), strong)),
            Span::Code(t) => (
                t,
                TextFormat {
                    background: visuals.code_bg_color,
                    ..TextFormat::simple(mono.clone(), visuals.text_color())
                },
            ),
        };
        job.append(&text, 0.0, format);
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changelog_blocks_and_spans() {
        let text = "Fourth test version.\n\n- **Crop**: fixed\n  on two lines\n- `tri --check-update`\n\n<details>\n## Next\n";
        assert_eq!(
            blocks(text),
            [
                Block::Paragraph("Fourth test version.".into()),
                Block::Bullet("**Crop**: fixed on two lines".into()),
                Block::Bullet("`tri --check-update`".into()),
                Block::Heading("Next".into()),
            ]
        );
        assert_eq!(
            spans("**Crop**: see `r`, a ** alone"),
            [
                Span::Bold("Crop".into()),
                Span::Text(": see ".into()),
                Span::Code("r".into()),
                Span::Text(", a ** alone".into()),
            ]
        );
    }
}
