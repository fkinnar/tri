//! Programmes externes lancés par `tri` (outils du système, logiciels de développement,
//! éditeur) : une seule façon de les créer, pour qu'aucun n'ouvre de fenêtre de console
//! sous Windows (#159 : l'éjection ouvrait une fenêtre PowerShell). Voir
//! `doc/en/process.md`.

use std::ffi::OsStr;
use std::process::Command;

/// `CREATE_NO_WINDOW` : pas de console pour un programme lancé par une application
/// graphique (Windows).
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// La commande `program`, sans fenêtre de console sous Windows ; ailleurs, une commande
/// ordinaire.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Vrai si un programme de nom `name` tourne (sans extension : `darktable`) :
/// `pgrep -x` sous macOS et Linux, `tasklist` sous Windows. Faux si l'outil manque.
pub fn running(name: &str) -> bool {
    if cfg!(windows) {
        let image = format!("{name}.exe");
        command("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {image}"), "/NH"])
            .output()
            .is_ok_and(|o| tasklist_lists(&String::from_utf8_lossy(&o.stdout), &image))
    } else {
        command("pgrep")
            .args(["-x", name])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
}

/// Vrai si la sortie de `tasklist` liste le programme `image` (« darktable.exe ») ;
/// sans résultat, `tasklist` écrit un message (traduit selon la langue de Windows).
fn tasklist_lists(output: &str, image: &str) -> bool {
    output.lines().any(|line| {
        line.split_whitespace()
            .next()
            .is_some_and(|first| first.eq_ignore_ascii_case(image))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_that_does_not_exist_is_not_running() {
        assert!(!running("tri-programme-qui-n-existe-pas"));
    }

    #[test]
    fn the_tasklist_output_is_read_line_by_line() {
        let listed = "\r\ndarktable.exe                 4242 Console    1    350 000 K\r\n";
        assert!(tasklist_lists(listed, "darktable.exe"));
        assert!(tasklist_lists(listed, "DARKTABLE.EXE"));
        let none = "INFO: No tasks are running which match the specified criteria.";
        assert!(!tasklist_lists(none, "darktable.exe"));
        assert!(!tasklist_lists(
            "darktable-cli.exe  12 Console",
            "darktable.exe"
        ));
    }

    #[test]
    fn a_command_is_a_plain_command_with_its_program() {
        let command = command("echo");
        assert_eq!(command.get_program(), "echo");
        assert_eq!(command.get_args().count(), 0);
    }
}
