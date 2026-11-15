//! Paired BoxBox servers. A server pairs with this computer once, by the code
//! its Settings → Account show; the credential it returns lives in the
//! keyring, under the saved connection's id. These calls block (HTTP, D-Bus,
//! perhaps a keyring unlock prompt), so they run off the UI thread.

use anyhow::Result;

use crate::api::{Api, Unauthorized};
use crate::settings_state::SavedConnection;

pub(crate) enum Opened {
    Ready(Api),
    /// No credential, or the server refused it; a new pairing code fixes both.
    NeedsPairing(String),
}

/// What the server lists among its paired clients.
fn client_name() -> String {
    let host = std::fs::read_to_string("/proc/sys/kernel/hostname").map(|name| name.trim().to_string()).unwrap_or_default();
    if host.is_empty() { "BoxBox for Linux".into() } else { format!("BoxBox on {host}") }
}

/// The saved connection with its keyring credential, once the server accepts it.
pub(crate) fn open(connection: &SavedConnection) -> Result<Opened> {
    let Some(credential) = crate::secrets::lookup(&connection.id)? else {
        return Ok(Opened::NeedsPairing("This computer is not paired with the server yet.".into()));
    };
    let api = Api::new(&connection.url, Some(credential));
    match api.roots() {
        Ok(_) => Ok(Opened::Ready(api)),
        Err(error) => match error.downcast_ref::<Unauthorized>() {
            Some(Unauthorized(message)) => Ok(Opened::NeedsPairing(format!("The server no longer accepts this computer ({message}). Pair it again."))),
            None => Err(error),
        },
    }
}

/// Redeems a pairing code and keeps the credential, returning the open server.
pub(crate) fn pair(connection: &SavedConnection, code: &str) -> Result<Api> {
    let credential = Api::redeem(&connection.url, code.trim(), &client_name())?;
    crate::secrets::store(&connection.id, &format!("BoxBox server “{}”", connection.name), &credential)?;
    Ok(Api::new(&connection.url, Some(credential)))
}

/// Asks the server to forget this computer, then drops the credential. The
/// server may be gone, so only the keyring has to succeed.
pub(crate) fn forget(connection: &SavedConnection) -> Result<()> {
    if let Some(credential) = crate::secrets::lookup(&connection.id)? {
        if let Err(error) = Api::new(&connection.url, Some(credential)).revoke_self() {
            log::info!("{} did not revoke this computer: {error:#}", connection.url);
        }
    }
    crate::secrets::delete(&connection.id)
}
