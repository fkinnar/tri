//! Programmes externes lancés par `tri` (outils du système, logiciels de développement,
//! éditeur) : une seule façon de les créer, pour qu'aucun n'ouvre de fenêtre de console
//! sous Windows (#159 : l'éjection ouvrait une fenêtre PowerShell). Voir
//! `doc/en/process.md`.

use std::ffi::OsStr;
use std::io;
use std::process::{Child, Command};
use std::time::Duration;

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

/// Essais de plus quand l'exécutable est « occupé » ([`spawn`]), et l'attente avant le
/// premier (doublée à chaque essai : 5, 10, 20, 40, 80 ms, 155 ms en tout). Pas un
/// réglage : une parade à une course du système, pas un potard (`doc/en/settings.md`).
const BUSY_RETRIES: u32 = 5;
const BUSY_WAIT: Duration = Duration::from_millis(5);

/// Lance `command`. Un exécutable « occupé » (`ETXTBSY`, Linux : il vient d'être écrit, et
/// un processus créé au même moment ailleurs dans `tri` a hérité un instant du fichier
/// ouvert en écriture, jusqu'à son `exec`) est réessayé quelques fois : l'erreur ne dure
/// que le temps de ce lancement voisin. Vu sur les faux outils des tests, écrits puis
/// lancés pendant que d'autres tests lancent les leurs (CI Ubuntu, 11/11/2026) ; possible
/// aussi pour un outil tout juste installé ou extrait (AppImage).
pub fn spawn(command: &mut Command) -> io::Result<Child> {
    let mut wait = BUSY_WAIT;
    for _ in 0..BUSY_RETRIES {
        match command.spawn() {
            Err(e) if e.kind() == io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(wait);
                wait *= 2;
            }
            result => return result,
        }
    }
    command.spawn()
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

    #[cfg(unix)]
    #[test]
    fn a_busy_executable_is_launched_once_freed() {
        // CI Ubuntu, 11/11/2026 : « Text file busy (os error 26) » en lançant un faux
        // outil que le test venait d'écrire. Ici, le script reste ouvert en écriture
        // 30 ms : Linux refuse de l'exécuter pendant ce temps (ETXTBSY) ; `spawn` attend.
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("tool.sh");
        let mut file = std::fs::File::create(&script).unwrap();
        file.write_all(b"#!/bin/sh\nexit 0\n").unwrap();
        file.flush().unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            drop(file);
        });
        let status = spawn(&mut command(&script)).unwrap().wait().unwrap();
        assert!(status.success());
        release.join().unwrap();
    }

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
