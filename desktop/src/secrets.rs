//! Server credentials in the user's keyring, through the freedesktop Secret
//! Service on the session bus (GNOME Keyring, KWallet, KeePassXC). Every call
//! blocks, and may wait for the user to unlock the keyring, so it runs off the
//! UI thread.

use std::collections::HashMap;

use anyhow::{bail, Context as _, Result};
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

const SERVICE: &str = "org.freedesktop.secrets";
const SERVICE_PATH: &str = "/org/freedesktop/secrets";
const SERVICE_IFACE: &str = "org.freedesktop.Secret.Service";
const COLLECTION_IFACE: &str = "org.freedesktop.Secret.Collection";
const ITEM_IFACE: &str = "org.freedesktop.Secret.Item";
const PROMPT_IFACE: &str = "org.freedesktop.Secret.Prompt";
/// The schema attribute libsecret writes, so keyring browsers group the items.
const SCHEMA: &str = "app.boxbox.Server";

/// `(oayays)`: session, parameters, value, content type.
type Secret = (OwnedObjectPath, Vec<u8>, Vec<u8>, String);

struct Keyring {
    connection: Connection,
    service: Proxy<'static>,
    session: OwnedObjectPath,
}

fn attributes(server: &str) -> HashMap<&'static str, &str> {
    HashMap::from([("xdg:schema", SCHEMA), ("server", server)])
}

impl Keyring {
    fn open() -> Result<Self> {
        let connection = Connection::session().context("cannot reach the session bus")?;
        let service = Proxy::new(&connection, SERVICE, SERVICE_PATH, SERVICE_IFACE)?;
        // "plain" keeps the secret unencrypted on the session bus, which only this user can read.
        let (_, session): (OwnedValue, OwnedObjectPath) =
            service.call("OpenSession", &("plain", Value::from(""))).context("no keyring service is running")?;
        Ok(Self { connection, service, session })
    }

    /// Runs a prompt the service asks for, such as unlocking the keyring, and
    /// waits for the user. Returns the prompt's result, or fails if dismissed.
    fn prompt(&self, path: &ObjectPath) -> Result<OwnedValue> {
        let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).path(path.to_owned())?.interface(PROMPT_IFACE)?.member("Completed")?.build();
        let mut signals = MessageIterator::for_match_rule(rule, &self.connection, Some(1))?;
        Proxy::new(&self.connection, SERVICE, path.to_owned(), PROMPT_IFACE)?.call::<_, _, ()>("Prompt", &("",))?;
        let message = signals.next().context("the keyring closed without answering")??;
        let (dismissed, result): (bool, OwnedValue) = message.body().deserialize()?;
        if dismissed {
            bail!("the keyring stayed locked");
        }
        Ok(result)
    }

    fn unlock(&self, objects: &[OwnedObjectPath]) -> Result<()> {
        let (_, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) = self.service.call("Unlock", &(objects,))?;
        if prompt.as_str() != "/" {
            self.prompt(&prompt)?;
        }
        Ok(())
    }

    fn items(&self, server: &str) -> Result<Vec<OwnedObjectPath>> {
        let (unlocked, locked): (Vec<OwnedObjectPath>, Vec<OwnedObjectPath>) = self.service.call("SearchItems", &(attributes(server),))?;
        if !locked.is_empty() {
            self.unlock(&locked)?;
        }
        Ok(unlocked.into_iter().chain(locked).collect())
    }

    /// The default collection ("login" on most desktops), unlocked.
    fn collection(&self) -> Result<OwnedObjectPath> {
        let collection: OwnedObjectPath = self.service.call("ReadAlias", &("default",))?;
        if collection.as_str() == "/" {
            bail!("the keyring has no default collection");
        }
        self.unlock(std::slice::from_ref(&collection))?;
        Ok(collection)
    }
}

/// Saves the credential for a server connection, replacing any earlier one.
pub fn store(server: &str, label: &str, credential: &str) -> Result<()> {
    let keyring = Keyring::open()?;
    let collection = keyring.collection()?;
    let properties: HashMap<&str, Value> = HashMap::from([
        ("org.freedesktop.Secret.Item.Label", Value::from(label)),
        ("org.freedesktop.Secret.Item.Attributes", Value::from(attributes(server))),
    ]);
    let secret: Secret = (keyring.session.clone(), Vec::new(), credential.as_bytes().to_vec(), "text/plain".into());
    let (_, prompt): (OwnedObjectPath, OwnedObjectPath) = Proxy::new(&keyring.connection, SERVICE, collection, COLLECTION_IFACE)?
        .call("CreateItem", &(properties, secret, true))
        .context("the keyring did not save the credential")?;
    if prompt.as_str() != "/" {
        keyring.prompt(&prompt)?;
    }
    Ok(())
}

/// The saved credential for a server connection, if there is one.
pub fn lookup(server: &str) -> Result<Option<String>> {
    let keyring = Keyring::open()?;
    let items = keyring.items(server)?;
    if items.is_empty() {
        return Ok(None);
    }
    let secrets: HashMap<OwnedObjectPath, Secret> = keyring.service.call("GetSecrets", &(&items, &keyring.session))?;
    let Some((_, _, value, _)) = items.iter().find_map(|item| secrets.get(item)) else { return Ok(None) };
    Ok(Some(String::from_utf8(value.clone()).context("the saved credential is not text")?))
}

/// Forgets a server connection's credential.
pub fn delete(server: &str) -> Result<()> {
    let keyring = Keyring::open()?;
    for item in keyring.items(server)? {
        let prompt: OwnedObjectPath = Proxy::new(&keyring.connection, SERVICE, item, ITEM_IFACE)?.call("Delete", &())?;
        if prompt.as_str() != "/" {
            keyring.prompt(&prompt)?;
        }
    }
    Ok(())
}
