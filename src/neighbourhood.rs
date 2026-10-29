//! Voisinage réseau : les serveurs qui s'annoncent sur le réseau local (Bonjour / mDNS,
//! comme le Finder, Nemo ou l'Explorateur), proposés dans la section « Réseau » du
//! panneau des dossiers sans avoir à taper leur adresse (demandé le 29/10/2026 pour la
//! bêta). Écoute en fond par `mdns-sd` (pur Rust, sans runtime asynchrone) ; ce module
//! ne fait que rassembler les annonces par serveur. Voir `doc/en/neighbourhood.md`.

use std::collections::BTreeMap;
use std::sync::mpsc;

use serde::Deserialize;

use crate::places::{Place, PlaceKind};
use crate::servers::Protocol;

/// Section `[neighbourhood]` : voisinage réseau.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NeighbourhoodSettings {
    /// Écouter les serveurs qui s'annoncent (mDNS). Désactivé : seuls les serveurs
    /// mémorisés et montés sont dans « Réseau ».
    pub enabled: bool,
}

impl Default for NeighbourhoodSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Types de services écoutés, et leur protocole, dans l'ordre de préférence pour
/// rejoindre un serveur qui en annonce plusieurs : les partages d'abord (SMB, AFP).
const SERVICES: &[(&str, Protocol)] = &[
    ("_smb._tcp.local.", Protocol::Smb),
    ("_afpovertcp._tcp.local.", Protocol::Afp),
    ("_sftp-ssh._tcp.local.", Protocol::Sftp),
    ("_webdavs._tcp.local.", Protocol::Davs),
    ("_webdav._tcp.local.", Protocol::Dav),
    ("_ftp._tcp.local.", Protocol::Ftp),
    ("_nfs._tcp.local.", Protocol::Nfs),
];

/// Port habituel de `protocol` : omis de l'adresse.
fn usual_port(protocol: Protocol) -> u16 {
    match protocol {
        Protocol::Smb => 445,
        Protocol::Afp => 548,
        Protocol::Ftp => 21,
        Protocol::Sftp => 22,
        Protocol::Dav => 80,
        Protocol::Davs => 443,
        Protocol::Nfs => 2049,
    }
}

/// Un service annoncé.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
    /// Nom complet de l'annonce (`NAS._smb._tcp.local.`), unique.
    pub fullname: String,
    /// Nom donné par le serveur (`NAS`).
    pub name: String,
    /// Nom d'hôte (`nas.local`).
    pub host: String,
    /// Protocole.
    pub protocol: Protocol,
    /// Port.
    pub port: u16,
    /// Chemin annoncé (`path` du TXT : WebDAV, NFS, FTP), sans `/` autour.
    pub path: String,
}

/// Un changement dans le voisinage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Service annoncé (ou annoncé à nouveau, modifié).
    Found(Service),
    /// Service retiré (`fullname`).
    Removed(String),
}

/// Un serveur du voisinage, avec ses services.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Neighbour {
    /// Nom donné par le serveur.
    pub name: String,
    /// Nom d'hôte.
    pub host: String,
    /// Services, du préféré au moins préféré ([`SERVICES`]).
    pub services: Vec<Service>,
}

impl Neighbour {
    /// Adresse pour le rejoindre sous `os`, par son service préféré : le serveur seul
    /// pour SMB et AFP (ses partages se listent), sinon le chemin annoncé.
    pub fn address(&self, os: &str) -> String {
        let service = &self.services[0];
        let port = if service.port == usual_port(service.protocol) {
            String::new()
        } else {
            format!(":{}", service.port)
        };
        match service.protocol {
            Protocol::Smb if os == "windows" && port.is_empty() => {
                format!(r"\\{}\", self.host)
            }
            Protocol::Smb | Protocol::Afp => {
                format!("{}://{}{port}/", service.protocol.scheme(), self.host)
            }
            protocol => format!(
                "{}://{}{port}/{}",
                protocol.scheme(),
                self.host,
                crate::servers::percent_encode_path(&service.path)
            ),
        }
    }

    /// Emplacement du panneau des dossiers : son adresse comme chemin, à ouvrir dans la
    /// recherche des dossiers.
    pub fn place(&self, os: &str) -> Place {
        let protocols: Vec<&str> = self.services.iter().map(|s| s.protocol.label()).collect();
        Place {
            name: format!("{} ({})", self.name, protocols.join(", ")),
            path: self.address(os).into(),
            kind: PlaceKind::Neighbour,
        }
    }
}

/// Services annoncés, rassemblés par serveur.
#[derive(Clone, Debug, Default)]
pub struct Neighbourhood {
    services: BTreeMap<String, Service>,
}

impl Neighbourhood {
    /// Applique `event` ; vrai si le voisinage a changé.
    pub fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Found(service) => {
                let old = self
                    .services
                    .insert(service.fullname.clone(), service.clone());
                old.as_ref() != Some(&service)
            }
            Event::Removed(fullname) => self.services.remove(&fullname).is_some(),
        }
    }

    /// Serveurs, par nom ; un serveur qui annonce plusieurs protocoles n'apparaît
    /// qu'une fois (même nom d'hôte, casse ignorée).
    pub fn neighbours(&self) -> Vec<Neighbour> {
        let mut by_host: BTreeMap<String, Neighbour> = BTreeMap::new();
        for service in self.services.values() {
            let neighbour = by_host
                .entry(service.host.to_lowercase())
                .or_insert_with(|| Neighbour {
                    name: service.name.clone(),
                    host: service.host.clone(),
                    services: Vec::new(),
                });
            if !neighbour
                .services
                .iter()
                .any(|s| s.protocol == service.protocol)
            {
                neighbour.services.push(service.clone());
            }
        }
        let rank = |p: Protocol| SERVICES.iter().position(|(_, q)| *q == p);
        let mut list: Vec<Neighbour> = by_host.into_values().collect();
        for neighbour in &mut list {
            neighbour.services.sort_by_key(|s| rank(s.protocol));
            // Le nom donné avec le service préféré (une annonce SFTP porte souvent le nom
            // de la machine, l'annonce SMB celui choisi pour le partage).
            neighbour.name = neighbour.services[0].name.clone();
        }
        list.sort_by_key(|n| n.name.to_lowercase());
        list
    }
}

/// Écoute du voisinage en fond : les changements arrivent par [`Browser::poll`].
/// Arrêtée quand le `Browser` est libéré.
pub struct Browser {
    daemon: mdns_sd::ServiceDaemon,
    events: mpsc::Receiver<Event>,
}

impl Browser {
    /// Commence l'écoute ; `notify` est appelé à chaque changement (rafraîchir
    /// l'interface). Ouvre une socket multicast (port 5353) : le pare-feu du système peut
    /// le demander à l'utilisateur.
    pub fn start(notify: impl Fn() + Send + Sync + 'static) -> Result<Browser, String> {
        let daemon = mdns_sd::ServiceDaemon::new().map_err(|e| e.to_string())?;
        let (tx, events) = mpsc::channel();
        let notify = std::sync::Arc::new(notify);
        for (ty, protocol) in SERVICES {
            let receiver = daemon.browse(ty).map_err(|e| e.to_string())?;
            let (tx, notify, protocol) = (tx.clone(), notify.clone(), *protocol);
            std::thread::spawn(move || {
                // Fin quand le démon s'arrête (canal fermé) ou l'interface est partie.
                while let Ok(event) = receiver.recv() {
                    let Some(event) = convert(event, protocol) else {
                        continue;
                    };
                    if tx.send(event).is_err() {
                        break;
                    }
                    notify();
                }
            });
        }
        Ok(Browser { daemon, events })
    }

    /// Applique à `neighbourhood` les changements arrivés ; vrai s'il a changé.
    pub fn poll(&self, neighbourhood: &mut Neighbourhood) -> bool {
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            changed |= neighbourhood.apply(event);
        }
        changed
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}

/// Un événement de `mdns-sd` en [`Event`] ; les autres (recherche commencée…) ignorés.
fn convert(event: mdns_sd::ServiceEvent, protocol: Protocol) -> Option<Event> {
    match event {
        mdns_sd::ServiceEvent::ServiceResolved(info) => {
            let path = info.get_property_val_str("path").unwrap_or_default();
            Some(Event::Found(service(
                info.get_fullname(),
                info.ty_domain.as_str(),
                info.get_hostname(),
                info.get_port(),
                protocol,
                path,
            )))
        }
        mdns_sd::ServiceEvent::ServiceRemoved(_, fullname) => Some(Event::Removed(fullname)),
        _ => None,
    }
}

/// Un [`Service`] d'après une annonce : nom tiré du nom complet (`NAS._smb._tcp.local.`
/// moins le type), nom d'hôte sans le point final.
fn service(
    fullname: &str,
    ty_domain: &str,
    host: &str,
    port: u16,
    protocol: Protocol,
    path: &str,
) -> Service {
    let name = fullname
        .strip_suffix(ty_domain)
        .map(|n| n.trim_end_matches('.'))
        .filter(|n| !n.is_empty())
        .unwrap_or(fullname);
    Service {
        fullname: fullname.to_string(),
        name: name.replace("\\032", " ").replace("\\ ", " "),
        host: host.trim_end_matches('.').to_string(),
        protocol,
        port,
        path: path.trim_matches('/').to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(name: &str, ty: &str, host: &str, port: u16, path: &str) -> Event {
        let protocol = SERVICES.iter().find(|(t, _)| *t == ty).unwrap().1;
        Event::Found(service(
            &format!("{name}.{ty}"),
            ty,
            host,
            port,
            protocol,
            path,
        ))
    }

    #[test]
    fn announces_are_gathered_by_server_with_the_preferred_protocol_first() {
        let mut hood = Neighbourhood::default();
        assert!(hood.apply(found("nas", "_sftp-ssh._tcp.local.", "Nas.local.", 22, "")));
        assert!(hood.apply(found(
            "Photos NAS",
            "_smb._tcp.local.",
            "nas.local.",
            445,
            ""
        )));
        assert!(hood.apply(found(
            "Box",
            "_webdavs._tcp.local.",
            "box.local.",
            8443,
            "/dav/photo/"
        )));
        assert!(
            !hood.apply(found(
                "Box",
                "_webdavs._tcp.local.",
                "box.local.",
                8443,
                "/dav/photo/"
            )),
            "annonce répétée : rien ne change"
        );
        let list = hood.neighbours();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Box");
        assert_eq!(list[0].address("linux"), "davs://box.local:8443/dav/photo");
        let nas = &list[1];
        assert_eq!(nas.name, "Photos NAS", "le nom de l'annonce SMB");
        assert_eq!(
            nas.services.iter().map(|s| s.protocol).collect::<Vec<_>>(),
            vec![Protocol::Smb, Protocol::Sftp]
        );
        assert_eq!(nas.address("macos"), "smb://nas.local/");
        assert_eq!(nas.address("windows"), r"\\nas.local\");
        let place = nas.place("linux");
        assert_eq!(place.name, "Photos NAS (SMB, SFTP)");
        assert_eq!(place.kind, PlaceKind::Neighbour);
        // L'annonce retirée : le serveur aussi, une fois tous ses services partis.
        assert!(hood.apply(Event::Removed("Photos NAS._smb._tcp.local.".into())));
        assert_eq!(hood.neighbours()[1].address("linux"), "sftp://Nas.local/");
        assert!(!hood.apply(Event::Removed("inconnu".into())));
    }

    #[test]
    fn the_address_of_a_server_parses_back_to_it() {
        let mut hood = Neighbourhood::default();
        hood.apply(found(
            "Disque",
            "_afpovertcp._tcp.local.",
            "disque.local.",
            548,
            "",
        ));
        hood.apply(found(
            "Ftp",
            "_ftp._tcp.local.",
            "ftp.local.",
            2121,
            "Séance 1",
        ));
        for neighbour in hood.neighbours() {
            for os in ["linux", "macos", "windows"] {
                let address = neighbour.address(os);
                let host = crate::servers::Host::parse(&address);
                let server = crate::servers::Server::parse(&address);
                assert!(host.is_some() || server.is_some(), "{address}");
            }
        }
        assert_eq!(
            hood.neighbours()[1].address("linux"),
            "ftp://ftp.local:2121/S%C3%A9ance%201"
        );
    }
}
