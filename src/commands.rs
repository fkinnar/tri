//! Les commandes de `tri` en ligne de commande, décrites **une seule fois** (issue n° 441) :
//! `tri --help`, le mode d'emploi des erreurs, l'aide de l'interface (F1) et, plus tard, la
//! documentation d'une éventuelle API viennent tous de cette liste.
//!
//! Chaque commande : un thème, sa syntaxe (les mots entre `<…>` sont traduits), ses
//! options avec une description chacune, et un exemple que l'interface remplit avec le
//! dossier ouvert ou les images sélectionnées. Les textes sont dans les catalogues
//! (`cli-cmd-<id>`, `cli-opt-<option>`, ou `cli-opt-<id>-<option>` quand une option dit
//! autre chose dans une commande). Un test vérifie que chaque option lue par le programme
//! est décrite ici, et que chaque texte existe dans toutes les langues.

use std::path::{Path, PathBuf};

use crate::t;

/// Thème d'une commande, pour les ranger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    /// Lister un dossier, calculer ses aperçus.
    Folder,
    /// Notes : noter, conventions de sidecars, notes dans les images, conflits.
    Notes,
    /// Renommer, supprimer, copier, déplacer, montrer, ouvrir.
    Files,
    /// Exporter, harmoniser.
    Export,
    /// Décharger une carte.
    Offload,
    /// Exposition, métadonnées, netteté, piles.
    Analysis,
    /// Cache et données de `tri`.
    Cache,
    /// Le reste (mise à jour).
    Other,
}

impl Theme {
    /// Dans l'ordre de l'aide.
    pub const ALL: [Theme; 8] = [
        Theme::Folder,
        Theme::Notes,
        Theme::Files,
        Theme::Export,
        Theme::Offload,
        Theme::Analysis,
        Theme::Cache,
        Theme::Other,
    ];

    /// Identifiant (`cli-theme-<id>`).
    pub fn id(self) -> &'static str {
        match self {
            Theme::Folder => "folder",
            Theme::Notes => "notes",
            Theme::Files => "files",
            Theme::Export => "export",
            Theme::Offload => "offload",
            Theme::Analysis => "analysis",
            Theme::Cache => "cache",
            Theme::Other => "other",
        }
    }

    /// Nom traduit.
    pub fn name(self) -> String {
        t!(&format!("cli-theme-{}", self.id()))
    }
}

/// Une commande.
#[derive(Clone, Copy, Debug)]
pub struct Command {
    /// Identifiant (`cli-cmd-<id>`).
    pub id: &'static str,
    /// Thème, pour ranger l'aide.
    pub theme: Theme,
    /// Syntaxe : `[…]` facultatif, `a|b` l'un ou l'autre, `…` répétable ; les mots entre
    /// `<…>` sont traduits (`cli-ph-<mot>`).
    pub syntax: &'static str,
    /// Options qui font la commande (`--copy`, `--move`) : décrites par la commande.
    pub main: &'static [&'static str],
    /// Les autres options de la syntaxe, chacune décrite.
    pub options: &'static [&'static str],
    /// Exemple : `<folder>` (le dossier), `<images>` (les images), `<image>` (la
    /// première) remplis par [`Command::example`] ; les autres mots traduits.
    pub example: &'static str,
}

/// Option commune à toutes les commandes.
pub const COMMON: &[&str] = &["--config"];

/// Les options de choix de fichiers d'un dossier.
const KIND_DEPTH: [&str; 3] = ["--raw", "--flat", "--depth"];

macro_rules! opts {
    ($($o:expr),* $(,)?) => { &[$($o),*] };
}

/// Toutes les commandes, dans l'ordre de l'aide (par thème, voir [`Theme::ALL`]).
pub const COMMANDS: &[Command] = &[
    Command {
        id: "list",
        theme: Theme::Folder,
        syntax: "tri <folder> [--raw|--flat] [--depth N]",
        main: &[],
        options: &KIND_DEPTH,
        example: "tri <folder>",
    },
    Command {
        id: "previews",
        theme: Theme::Folder,
        syntax: "tri <folder> --previews [--raw|--flat] [--depth N] [--side N] [--thumb N] [--workers N] [--mixed] [--cache <folder>]",
        main: &["--previews"],
        options: opts![
            "--raw",
            "--flat",
            "--depth",
            "--side",
            "--thumb",
            "--workers",
            "--mixed",
            "--cache"
        ],
        example: "tri <folder> --previews",
    },
    Command {
        id: "rate",
        theme: Theme::Notes,
        syntax: "tri <image>… [--stars N] [--color <color>[,…]|+<color>,-<color>…|none] [--flag accept|reject|none] [--dry-run]",
        main: &[],
        options: opts!["--stars", "--color", "--flag", "--dry-run"],
        example: "tri <images> --stars 3 --dry-run",
    },
    Command {
        id: "convert-xmp",
        theme: Theme::Notes,
        syntax: "tri <folder> --convert-xmp darktable|adobe [--dry-run] [--raw|--flat] [--depth N]",
        main: &["--convert-xmp"],
        options: opts!["--dry-run", "--raw", "--flat", "--depth"],
        example: "tri <folder> --convert-xmp adobe --dry-run",
    },
    Command {
        id: "sidecars",
        theme: Theme::Notes,
        syntax: "tri <folder> --sidecars rapidraw|xmp|all [--rewrite] [--dry-run] [--raw|--flat] [--depth N]",
        main: &["--sidecars"],
        options: opts!["--rewrite", "--dry-run", "--raw", "--flat", "--depth"],
        example: "tri <folder> --sidecars xmp --dry-run",
    },
    Command {
        id: "embed-notes",
        theme: Theme::Notes,
        syntax: "tri <folder> --embed-notes [--dry-run] [--raw|--flat] [--depth N]",
        main: &["--embed-notes"],
        options: opts!["--dry-run", "--raw", "--flat", "--depth"],
        example: "tri <folder> --embed-notes --dry-run",
    },
    Command {
        id: "import-notes",
        theme: Theme::Notes,
        syntax: "tri <folder> --import-notes [--dry-run] [--raw|--flat] [--depth N]",
        main: &["--import-notes"],
        options: opts!["--dry-run", "--raw", "--flat", "--depth"],
        example: "tri <folder> --import-notes --dry-run",
    },
    Command {
        id: "conflicts",
        theme: Theme::Notes,
        syntax: "tri <folder> --conflicts [--raw|--flat] [--depth N]",
        main: &["--conflicts"],
        options: &KIND_DEPTH,
        example: "tri <folder> --conflicts",
    },
    Command {
        id: "rename",
        theme: Theme::Files,
        syntax: "tri --rename <image> <name> [--dry-run]",
        main: &["--rename"],
        options: opts!["--dry-run"],
        example: "tri --rename <image> <name> --dry-run",
    },
    Command {
        id: "delete",
        theme: Theme::Files,
        syntax: "tri --delete <image>… [--permanent] [--dry-run]",
        main: &["--delete"],
        options: opts!["--permanent", "--dry-run"],
        example: "tri --delete <images> --dry-run",
    },
    Command {
        id: "copy",
        theme: Theme::Files,
        syntax: "tri --copy|--move <image>…|<folder> --to <folder> [--dry-run]",
        main: &["--copy", "--move"],
        options: opts!["--to", "--dry-run"],
        example: "tri --copy <images> --to <destination> --dry-run",
    },
    Command {
        id: "reveal",
        theme: Theme::Files,
        syntax: "tri --reveal <image>",
        main: &["--reveal"],
        options: &[],
        example: "tri --reveal <image>",
    },
    Command {
        id: "edit",
        theme: Theme::Files,
        syntax: "tri --edit <image>…",
        main: &["--edit"],
        options: &[],
        example: "tri --edit <images>",
    },
    Command {
        id: "export",
        theme: Theme::Export,
        syntax: "tri --export <preset> <image>… [--to <folder>] [--libraw|--embedded] [--passthrough] [--dry-run]",
        main: &["--export"],
        options: opts![
            "--to",
            "--libraw",
            "--embedded",
            "--passthrough",
            "--dry-run"
        ],
        example: "tri --export web <images> --dry-run",
    },
    Command {
        id: "adjust",
        theme: Theme::Export,
        syntax: "tri --adjust <reference> <image>… --to <folder> [--ev EV|auto] [--neutral auto|x,y,w,h] [--kelvin K[,tint]|daylight|cloudy|shade|tungsten|fluorescent|flash] [--creative-ev EV] [--warmth EV] [--tint EV] [--contrast %] [--saturation %] [--mono neutral|red|orange|yellow|green|blue|r,g,b] [--embedded] [--side N]",
        main: &["--adjust"],
        options: opts![
            "--to",
            "--ev",
            "--neutral",
            "--kelvin",
            "--creative-ev",
            "--warmth",
            "--tint",
            "--contrast",
            "--saturation",
            "--mono",
            "--embedded",
            "--side"
        ],
        example: "tri --adjust <images> --to <destination> --ev auto",
    },
    Command {
        id: "offload",
        theme: Theme::Offload,
        syntax: "tri --offload <card> [--to <root>] [--folder <name>] [--raw-only] [--dry-run] [--previews] [--delete-copied [--delete-left]] [--eject]",
        main: &["--offload"],
        options: opts![
            "--to",
            "--folder",
            "--raw-only",
            "--dry-run",
            "--previews",
            "--delete-copied",
            "--delete-left",
            "--eject"
        ],
        example: "tri --offload <card> --dry-run",
    },
    Command {
        id: "exposure",
        theme: Theme::Analysis,
        syntax: "tri <folder> --exposure [--raw|--flat] [--depth N]",
        main: &["--exposure"],
        options: &KIND_DEPTH,
        example: "tri <folder> --exposure",
    },
    Command {
        id: "metadata",
        theme: Theme::Analysis,
        syntax: "tri <folder> --metadata [--raw|--flat] [--depth N]",
        main: &["--metadata"],
        options: &KIND_DEPTH,
        example: "tri <folder> --metadata",
    },
    Command {
        id: "sharpness",
        theme: Theme::Analysis,
        syntax: "tri <folder> --sharpness [--raw|--flat] [--depth N]",
        main: &["--sharpness"],
        options: &KIND_DEPTH,
        example: "tri <folder> --sharpness",
    },
    Command {
        id: "groups",
        theme: Theme::Analysis,
        syntax: "tri <folder> --groups [--threshold X]",
        main: &["--groups"],
        options: opts!["--threshold"],
        example: "tri <folder> --groups",
    },
    Command {
        id: "clear-cache",
        theme: Theme::Cache,
        syntax: "tri [<folder>] --clear-cache [<level>] [--depth N] [--cache <folder>]",
        main: &["--clear-cache"],
        options: opts!["--depth", "--cache"],
        example: "tri <folder> --clear-cache",
    },
    Command {
        id: "cache-usage",
        theme: Theme::Cache,
        syntax: "tri --cache-usage [--cache <folder>]",
        main: &["--cache-usage"],
        options: opts!["--cache"],
        example: "tri --cache-usage",
    },
    Command {
        id: "cache-here",
        theme: Theme::Cache,
        syntax: "tri <folder> --cache-here|--cache-user [--cache <folder>]",
        main: &["--cache-here", "--cache-user"],
        options: opts!["--cache"],
        example: "tri <folder> --cache-here",
    },
    Command {
        id: "reset",
        theme: Theme::Cache,
        syntax: "tri <folder> --reset cache,data,notes,versions [--depth N] [--dry-run] [--cache <folder>]",
        main: &["--reset"],
        options: opts!["--depth", "--dry-run", "--cache"],
        example: "tri <folder> --reset cache --dry-run",
    },
    Command {
        id: "check-update",
        theme: Theme::Other,
        syntax: "tri --check-update",
        main: &["--check-update"],
        options: &[],
        example: "tri --check-update",
    },
];

/// Remplace les mots `<…>` de `text` par leur traduction (`cli-ph-<mot>`), ou par
/// `fill(mot)` s'il en donne une.
fn placeholders(text: &str, fill: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        out.push_str(&rest[..start]);
        let word = &rest[start + 1..start + end];
        match fill(word) {
            Some(filled) => out.push_str(&filled),
            None => {
                out.push('<');
                out.push_str(&t!(&format!("cli-ph-{word}")));
                out.push('>');
            }
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

/// Un chemin tel qu'on le tape dans un terminal : entre guillemets s'il contient autre
/// chose que des caractères sûrs (simples sous Unix, doubles sous Windows).
pub fn shell_quote(path: &Path) -> String {
    let text = path.display().to_string();
    let safe = |c: char| c.is_alphanumeric() || "/\\._-~:+,@".contains(c);
    if !text.is_empty() && text.chars().all(safe) {
        text
    } else if cfg!(windows) {
        format!("\"{text}\"")
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

impl Command {
    /// La commande d'identifiant `id`.
    pub fn get(id: &str) -> Option<&'static Command> {
        COMMANDS.iter().find(|c| c.id == id)
    }

    /// Syntaxe traduite.
    pub fn syntax(&self) -> String {
        placeholders(self.syntax, &|_| None)
    }

    /// Sa forme courte, traduite : les parties obligatoires de la syntaxe, sans `tri` ni
    /// le dossier de tête (`--offload <carte>`, `--previews`) ; le dossier seul pour la
    /// liste d'un dossier.
    pub fn form(&self) -> String {
        let mut depth = 0;
        let mut required = String::new();
        for c in self.syntax.chars() {
            match c {
                '[' => depth += 1,
                ']' => depth -= 1,
                c if depth == 0 => required.push(c),
                _ => {}
            }
        }
        let mut words: Vec<&str> = required.split_whitespace().collect();
        if words.first() == Some(&"tri") {
            words.remove(0);
        }
        if words.len() > 1 && words[0] == "<folder>" {
            words.remove(0);
        }
        placeholders(&words.join(" "), &|_| None)
    }

    /// Ce que fait la commande, en une phrase.
    pub fn description(&self) -> String {
        t!(&format!("cli-cmd-{}", self.id))
    }

    /// Identifiant du texte de l'option `option` dans cette commande : le sien
    /// (`cli-opt-<id>-<option>`) s'il existe, sinon le commun (`cli-opt-<option>`).
    pub fn option_id(&self, option: &str) -> String {
        let name = option.trim_start_matches('-');
        let own = format!("cli-opt-{}-{name}", self.id);
        if crate::i18n::has(crate::i18n::language(), &own) {
            own
        } else {
            format!("cli-opt-{name}")
        }
    }

    /// Les options et leur description.
    pub fn options(&self) -> Vec<(&'static str, String)> {
        self.options
            .iter()
            .map(|&o| (o, t!(&self.option_id(o))))
            .collect()
    }

    /// L'exemple, rempli : `folder` pour `<folder>`, `images` pour `<images>` (et la
    /// première pour `<image>`), chemins entre guillemets au besoin ; sans eux, le mot
    /// traduit reste.
    pub fn example(&self, folder: Option<&Path>, images: &[PathBuf]) -> String {
        placeholders(self.example, &|word| match word {
            "folder" => folder.map(shell_quote),
            "images" if !images.is_empty() => Some(
                images
                    .iter()
                    .map(|p| shell_quote(p))
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            "image" | "images" => images.first().map(|p| shell_quote(p)),
            _ => None,
        })
    }

    /// Texte cherché par l'aide de l'interface : syntaxe, description, options.
    pub fn search_text(&self) -> String {
        let mut text = format!("{} {}", self.syntax(), self.description());
        for (option, description) in self.options() {
            text.push(' ');
            text.push_str(option);
            text.push(' ');
            text.push_str(&description);
        }
        text
    }
}

/// Mode d'emploi court (une erreur dans les arguments) : la syntaxe de chaque commande.
pub fn usage() -> String {
    let mut out = format!("{}\n", t!("cli-usage-title"));
    for command in COMMANDS {
        out.push_str("  ");
        out.push_str(&command.syntax());
        out.push('\n');
    }
    out.push_str(&t!("cli-usage-more"));
    out
}

/// Aide complète (`tri --help`) : par thème, chaque commande avec sa description, ses
/// options et un exemple.
pub fn help() -> String {
    let mut out = format!("{}\n", t!("cli-help-intro"));
    for theme in Theme::ALL {
        out.push('\n');
        out.push_str(&theme.name().to_uppercase());
        out.push('\n');
        for command in COMMANDS.iter().filter(|c| c.theme == theme) {
            out.push_str(&format!(
                "\n  {}\n      {}\n",
                command.syntax(),
                command.description()
            ));
            let options = command.options();
            let width = options.iter().map(|(o, _)| o.len()).max().unwrap_or(0);
            for (option, description) in options {
                out.push_str(&format!("      {option:<width$}  {description}\n"));
            }
            out.push_str(&format!(
                "      {} {}\n",
                t!("cli-help-example"),
                command.example(None, &[])
            ));
        }
    }
    out.push('\n');
    for option in COMMON {
        out.push_str(&format!(
            "{} {option} <{}>  {}\n",
            t!("cli-help-common"),
            t!("cli-ph-file"),
            t!(&format!("cli-opt-{}", option.trim_start_matches('-')))
        ));
    }
    out
}

/// Les options citées par la syntaxe de `command` (mots commençant par `--`).
pub fn syntax_options(syntax: &str) -> Vec<&str> {
    syntax
        .split(|c: char| c.is_whitespace() || "[]|".contains(c))
        .filter(|w| w.starts_with("--"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{self, Language};

    #[test]
    fn every_option_of_a_syntax_is_described_by_its_command() {
        for command in COMMANDS {
            let mut cited = syntax_options(command.syntax);
            cited.sort();
            cited.dedup();
            let mut listed: Vec<&str> = command
                .main
                .iter()
                .chain(command.options)
                .copied()
                .collect();
            listed.sort();
            listed.dedup();
            assert_eq!(cited, listed, "{}", command.id);
        }
    }

    #[test]
    fn every_text_exists_in_every_language() {
        let mut missing = Vec::new();
        for language in Language::ALL {
            let has = |id: &str| i18n::has(language, id);
            for theme in Theme::ALL {
                if !has(&format!("cli-theme-{}", theme.id())) {
                    missing.push(format!("theme {} ({})", theme.id(), language.code()));
                }
            }
            for command in COMMANDS {
                if !has(&format!("cli-cmd-{}", command.id)) {
                    missing.push(format!("cli-cmd-{} ({})", command.id, language.code()));
                }
                for option in command.options.iter().chain(COMMON) {
                    let name = option.trim_start_matches('-');
                    let own = format!("cli-opt-{}-{name}", command.id);
                    if !has(&own) && !has(&format!("cli-opt-{name}")) {
                        missing.push(format!("{own} ({})", language.code()));
                    }
                }
                for text in [command.syntax, command.example] {
                    let mut rest = text;
                    while let Some(start) = rest.find('<') {
                        let end = start + rest[start..].find('>').unwrap();
                        let word = &rest[start + 1..end];
                        let filled = ["folder", "image", "images"].contains(&word);
                        if !(filled && text == command.example) && !has(&format!("cli-ph-{word}")) {
                            missing.push(format!("cli-ph-{word} ({})", language.code()));
                        }
                        rest = &rest[end + 1..];
                    }
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "{missing:#?}");
    }

    #[test]
    fn every_theme_has_a_command_and_ids_are_unique() {
        for theme in Theme::ALL {
            assert!(COMMANDS.iter().any(|c| c.theme == theme), "{theme:?}");
        }
        let mut ids: Vec<&str> = COMMANDS.iter().map(|c| c.id).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }

    #[test]
    fn examples_are_filled_with_the_folder_and_the_images() {
        let copy = Command::get("copy").unwrap();
        let images = [PathBuf::from("/p/a.RAF"), PathBuf::from("/p/b c.RAF")];
        let text = copy.example(Some(Path::new("/p")), &images);
        let quoted = if cfg!(windows) {
            "\"/p/b c.RAF\""
        } else {
            "'/p/b c.RAF'"
        };
        assert!(
            text.starts_with(&format!("tri --copy /p/a.RAF {quoted} --to <")),
            "{text}"
        );
        let list = Command::get("list").unwrap();
        assert_eq!(list.example(Some(Path::new("/p")), &[]), "tri /p");
        assert_eq!(
            Command::get("reveal").unwrap().example(None, &images),
            "tri --reveal /p/a.RAF"
        );
    }

    #[test]
    fn the_form_keeps_the_required_parts() {
        let form = |id: &str| Command::get(id).unwrap().form();
        assert_eq!(form("previews"), "--previews");
        assert_eq!(form("check-update"), "--check-update");
        assert!(
            form("offload").starts_with("--offload <"),
            "{}",
            form("offload")
        );
        // La liste d'un dossier : le dossier seul.
        assert!(form("list").starts_with('<'), "{}", form("list"));
        // Les options facultatives n'y sont pas.
        assert!(!form("rate").contains("--stars"), "{}", form("rate"));
    }

    #[test]
    fn quoting_keeps_simple_paths_bare() {
        assert_eq!(shell_quote(Path::new("/a/b-c_d.RAF")), "/a/b-c_d.RAF");
        if !cfg!(windows) {
            assert_eq!(shell_quote(Path::new("/a/l'été")), r"'/a/l'\''été'");
        }
    }
}
