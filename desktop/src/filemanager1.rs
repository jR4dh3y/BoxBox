//! `org.freedesktop.FileManager1` on the session bus, which browsers and
//! editors call for "Show in folder". The window serves it only when no other
//! file manager owns the name, so the user's default is never displaced.
//! Each call becomes a launch, as a second start of the app would be.

use anyhow::{Context as _, Result};
use zbus::blocking::Connection;
use zbus::fdo::{RequestNameFlags, RequestNameReply};

const NAME: &str = "org.freedesktop.FileManager1";

struct FileManager {
    launches: async_channel::Sender<String>,
}

impl FileManager {
    fn open(&self, uris: Vec<String>) {
        for uri in uris {
            let _ = self.launches.send_blocking(uri);
        }
    }
}

#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl FileManager {
    #[zbus(name = "ShowFolders")]
    fn show_folders(&self, uris: Vec<String>, _startup_id: String) {
        self.open(uris);
    }

    /// Opens each item's folder.
    #[zbus(name = "ShowItems")]
    fn show_items(&self, uris: Vec<String>, _startup_id: String) {
        self.open(uris);
    }

    #[zbus(name = "ShowItemProperties")]
    fn show_item_properties(&self, uris: Vec<String>, _startup_id: String) {
        self.open(uris);
    }
}

/// Serves the interface while the returned connection lives; `None` when another
/// file manager has the name.
pub fn serve(launches: async_channel::Sender<String>) -> Result<Option<Connection>> {
    let connection = zbus::blocking::connection::Builder::session()?
        .serve_at("/org/freedesktop/FileManager1", FileManager { launches })?
        .build()
        .context("cannot reach the session bus")?;
    match connection.request_name_with_flags(NAME, RequestNameFlags::DoNotQueue.into())? {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => Ok(Some(connection)),
        _ => Ok(None),
    }
}
