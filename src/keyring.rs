//! Trousseau du bureau sous Linux (Secret Service, par D-Bus) : retenir le mot de passe
//! d'un serveur ([`crate::servers::Server::save_login`]) là où GVFS le cherche.
//!
//! Parlé directement, sans `secret-tool` : cet outil (paquet `libsecret-tools`) manque
//! sur Linux Mint et sur bien des installations. Session `plain` (permise par la
//! spécification, la seule sans chiffrement à négocier) : le mot de passe ne passe que
//! par le bus de session de l'utilisateur.

use std::collections::HashMap;

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

const SERVICE: &str = "org.freedesktop.secrets";
const SERVICE_PATH: &str = "/org/freedesktop/secrets";

/// Pourquoi le mot de passe n'a pas été retenu.
#[derive(Debug)]
pub(crate) enum KeyringError {
    /// Pas de bus de session, ou aucun trousseau ne répond.
    Unavailable(String),
    /// Aucun trousseau par défaut.
    NoDefault,
    /// Déverrouillage refusé ou annulé par l'utilisateur.
    Dismissed,
    /// Autre erreur du service.
    Failed(String),
}

impl From<zbus::Error> for KeyringError {
    fn from(e: zbus::Error) -> Self {
        KeyringError::Failed(e.to_string())
    }
}

impl From<zbus::zvariant::Error> for KeyringError {
    fn from(e: zbus::zvariant::Error) -> Self {
        KeyringError::Failed(e.to_string())
    }
}

/// Enregistre `secret` dans le trousseau par défaut, sous `label` et `attributes`
/// (remplace un élément aux mêmes attributs). Bloquant : peut attendre que
/// l'utilisateur déverrouille le trousseau ; à appeler hors du fil de l'interface.
pub(crate) fn store(
    label: &str,
    attributes: &HashMap<String, String>,
    secret: &str,
) -> Result<(), KeyringError> {
    let conn = Connection::session().map_err(|e| KeyringError::Unavailable(e.to_string()))?;
    let service = Proxy::new(
        &conn,
        SERVICE,
        SERVICE_PATH,
        "org.freedesktop.Secret.Service",
    )?;
    let (_, session): (OwnedValue, OwnedObjectPath) = service
        .call("OpenSession", &("plain", Value::from("")))
        .map_err(|e| match e {
            zbus::Error::MethodError(..) | zbus::Error::FDO(_) => {
                KeyringError::Unavailable(e.to_string())
            }
            e => e.into(),
        })?;
    let result = store_in(&conn, &service, &session, label, attributes, secret);
    if let Ok(session) = Proxy::new(&conn, SERVICE, &*session, "org.freedesktop.Secret.Session") {
        let _ = session.call_method("Close", &());
    }
    result
}

fn store_in(
    conn: &Connection,
    service: &Proxy,
    session: &OwnedObjectPath,
    label: &str,
    attributes: &HashMap<String, String>,
    secret: &str,
) -> Result<(), KeyringError> {
    let collection: OwnedObjectPath = service.call("ReadAlias", &("default",))?;
    if collection.as_str() == "/" {
        return Err(KeyringError::NoDefault);
    }
    let (_, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) =
        service.call("Unlock", &(vec![&*collection],))?;
    prompt_through(conn, &prompt)?;

    let collection = Proxy::new(
        conn,
        SERVICE,
        &*collection,
        "org.freedesktop.Secret.Collection",
    )?;
    let mut properties: HashMap<&str, Value> = HashMap::new();
    properties.insert("org.freedesktop.Secret.Item.Label", Value::from(label));
    properties.insert(
        "org.freedesktop.Secret.Item.Attributes",
        Value::from(attributes.clone()),
    );
    let value = (
        &**session,
        Vec::<u8>::new(),
        secret.as_bytes().to_vec(),
        "text/plain",
    );
    let (_, prompt): (OwnedObjectPath, OwnedObjectPath) =
        collection.call("CreateItem", &(properties, value, true))?;
    prompt_through(conn, &prompt)
}

/// Montre la demande du trousseau (`prompt`, « / » : rien à demander) et attend la
/// réponse de l'utilisateur.
fn prompt_through(conn: &Connection, prompt: &ObjectPath) -> Result<(), KeyringError> {
    if prompt.as_str() == "/" {
        return Ok(());
    }
    let proxy = Proxy::new(conn, SERVICE, prompt, "org.freedesktop.Secret.Prompt")?;
    let mut completed = proxy.receive_signal("Completed")?;
    proxy.call_method("Prompt", &("",))?;
    let message = completed.next().ok_or(KeyringError::Dismissed)?;
    let (dismissed, _): (bool, OwnedValue) = message.body().deserialize()?;
    if dismissed {
        Err(KeyringError::Dismissed)
    } else {
        Ok(())
    }
}
