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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_a_plain_command_with_its_program() {
        let command = command("echo");
        assert_eq!(command.get_program(), "echo");
        assert_eq!(command.get_args().count(), 0);
    }
}
