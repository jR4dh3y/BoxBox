//! Paired BoxBox servers, whose drives sit beside this computer's in the
//! Overview and the sidebar. A server pairs with this computer once, by the
//! code its Settings → Account show; the credential it returns lives in the
//! keyring, under the saved connection's id. Connecting blocks (HTTP, D-Bus,
//! perhaps a keyring unlock prompt), so it runs off the UI thread.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use gpui::Context;

use crate::api::{Api, DriveStats, Unauthorized};
use crate::settings_state::{SavedConnection, SettingsDialog};
use crate::ui::SettingsField;
use crate::workspace::{PaneTarget, SettingsSection, Workspace};

pub(crate) enum ServerState {
    Connecting { since: Instant },
    Ready { api: Arc<Api>, drives: Vec<DriveStats> },
    /// No credential, or the server refused it; a new pairing code fixes both.
    NeedsPairing(String),
    /// The server could not be reached or failed.
    Offline(String),
}

/// A server's drive as it was last seen: what the Overview and the sidebar
/// show of it while the server connects again, so they keep their places.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct RememberedDrive {
    pub name: String,
    pub label: Option<String>,
    pub device: Option<String>,
    pub fs_type: Option<String>,
}

impl RememberedDrive {
    fn of(drive: &DriveStats) -> Self {
        Self { name: drive.name.clone(), label: drive.label.clone(), device: drive.device.clone(), fs_type: drive.fs_type.clone() }
    }

    pub fn display_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

/// Each saved connection's drives as last seen, by its id.
pub(crate) type Remembered = HashMap<String, Vec<RememberedDrive>>;

/// Reads the drives remembered from the last run. Call off the UI thread; a
/// missing or unreadable file remembers nothing.
pub(crate) fn load_remembered() -> Remembered {
    let _span = crate::profile::span("load remembered drives");
    std::fs::read(crate::persist::SERVER_DRIVES.path()).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

/// A drive the user opened before its server answered. It opens when the
/// server does, unless the pane went somewhere else first.
pub(crate) struct Wanted {
    pub server: String,
    /// None for the server's first drive.
    pub drive: Option<String>,
    pub pane: PaneTarget,
}

/// Each saved connection's state, by its id.
#[derive(Default)]
pub(crate) struct Servers {
    states: HashMap<String, ServerState>,
    remembered: Remembered,
    wanted: Option<Wanted>,
}

impl Servers {
    pub fn state(&self, id: &str) -> Option<&ServerState> {
        self.states.get(id)
    }

    /// The drives a server had when last seen; empty when none are known.
    pub fn remembered(&self, id: &str) -> &[RememberedDrive] {
        self.remembered.get(id).map_or(&[], Vec::as_slice)
    }

    pub fn set_remembered(&mut self, remembered: Remembered) {
        self.remembered = remembered;
    }

    /// Whether this drive is waiting to open; `None` asks about the first.
    pub fn is_wanted(&self, server: &str, drive: Option<&str>) -> bool {
        self.wanted.as_ref().is_some_and(|wanted| wanted.server == server && wanted.drive.as_deref() == drive)
    }

    /// Drops a waiting open: the user went elsewhere.
    pub fn forget_wanted(&mut self) {
        self.wanted = None;
    }

    #[cfg(test)]
    pub fn set(&mut self, id: &str, state: ServerState) {
        self.states.insert(id.to_string(), state);
    }
}

enum Connected {
    Ready(Arc<Api>, Vec<DriveStats>),
    NeedsPairing(String),
}

/// What the server lists among its paired clients.
fn client_name() -> String {
    let host = std::fs::read_to_string("/proc/sys/kernel/hostname").map(|name| name.trim().to_string()).unwrap_or_default();
    if host.is_empty() { "BoxBox for Linux".into() } else { format!("BoxBox on {host}") }
}

/// The saved connection with its keyring credential, and its drives once the server accepts it.
fn connect(connection: &SavedConnection) -> Result<Connected> {
    let Some(credential) = crate::secrets::lookup(&connection.id)? else {
        return Ok(Connected::NeedsPairing("This computer is not paired with the server yet.".into()));
    };
    let api = Api::new(&connection.url, Some(credential));
    let drives = api.drives();
    listed(Arc::new(api), drives)
}

/// What a server's drive listing means: its drives, or a refused credential
/// that a new pairing fixes, or a failure.
fn listed(api: Arc<Api>, drives: Result<Vec<DriveStats>>) -> Result<Connected> {
    match drives {
        Ok(drives) => Ok(Connected::Ready(api, drives)),
        Err(error) => match error.downcast_ref::<Unauthorized>() {
            Some(Unauthorized(message)) => Ok(Connected::NeedsPairing(format!("The server no longer accepts this computer ({message}). Pair it again."))),
            None => Err(error),
        },
    }
}

/// Redeems a pairing code and keeps the credential in the keyring.
pub(crate) fn pair(connection: &SavedConnection, code: &str) -> Result<()> {
    let credential = Api::redeem(&connection.url, code.trim(), &client_name())?;
    crate::secrets::store(&connection.id, &format!("BoxBox server “{}”", connection.name), &credential)
}

/// Asks the server to forget this computer, then drops the credential. The
/// server may be gone, so only the keyring has to succeed.
fn forget(connection: &SavedConnection) -> Result<()> {
    if let Some(credential) = crate::secrets::lookup(&connection.id)? {
        if let Err(error) = Api::new(&connection.url, Some(credential)).revoke_self() {
            log::info!("{} did not revoke this computer: {error:#}", connection.url);
        }
    }
    crate::secrets::delete(&connection.id)
}

impl Workspace {
    /// Connects every saved server in the background, so their drives appear
    /// once each answers.
    pub(crate) fn connect_servers(&mut self, cx: &mut Context<Self>) {
        for connection in self.desktop_settings.connections.clone() {
            self.connect_server(connection, false, cx);
        }
    }

    /// Opens a server's drive in the active pane: now when the server has
    /// answered, else as soon as it does. A server that failed is tried again.
    pub(crate) fn open_server_drive_when_ready(&mut self, connection: SavedConnection, drive: Option<String>, cx: &mut Context<Self>) {
        if let Some(ServerState::Ready { api, drives }) = self.servers.states.get(&connection.id) {
            let drive = drive.or_else(|| drives.first().map(|drive| drive.name.clone())).unwrap_or_default();
            self.open_server_drive(self.active_pane, api.clone(), drive, cx);
            return;
        }
        self.servers.wanted = Some(Wanted { server: connection.id.clone(), drive, pane: self.active_pane });
        cx.notify();
        self.connect_server(connection, false, cx);
    }

    /// Connects a saved server. When `open`, the user asked for it: its first
    /// drive opens, or Settings asks for a pairing code, or the failure shows.
    pub(crate) fn connect_server(&mut self, connection: SavedConnection, open: bool, cx: &mut Context<Self>) {
        if open {
            self.open_server_drive_when_ready(connection, None, cx);
            return;
        }
        if let Some(ServerState::Connecting { .. }) = self.servers.states.get(&connection.id) {
            return;
        }
        self.servers.states.insert(connection.id.clone(), ServerState::Connecting { since: Instant::now() });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let connecting = connection.clone();
            let result = cx.background_executor().spawn(async move { connect(&connecting) }).await;
            this.update(cx, |this, cx| this.connected(connection, result, cx)).ok();
        })
        .detach();
    }

    /// A server answered, or failed to. A drive the user opened while it
    /// connected opens now; a failure they were waiting on is shown.
    fn connected(&mut self, connection: SavedConnection, result: Result<Connected>, cx: &mut Context<Self>) {
        // Removed while it connected.
        if !self.desktop_settings.connections.iter().any(|saved| saved.id == connection.id) {
            self.servers.states.remove(&connection.id);
            return;
        }
        let wanted = self.servers.wanted.take_if(|wanted| wanted.server == connection.id);
        let state = match result {
            Ok(Connected::Ready(api, drives)) => {
                self.remember(&connection.id, &drives, cx);
                if let Some(wanted) = wanted {
                    // A remembered drive the server no longer has opens its first.
                    let drive = wanted.drive.filter(|name| drives.iter().any(|drive| &drive.name == name));
                    let drive = drive.or_else(|| drives.first().map(|drive| drive.name.clone())).unwrap_or_default();
                    self.open_server_drive(wanted.pane, api.clone(), drive, cx);
                }
                ServerState::Ready { api, drives }
            }
            Ok(Connected::NeedsPairing(reason)) => {
                if wanted.is_some() {
                    self.ask_to_pair(connection.clone(), reason.clone(), cx);
                }
                ServerState::NeedsPairing(reason)
            }
            Err(error) => {
                let message = format!("{error:#}");
                if wanted.is_some() {
                    let shown = format!("Could not open {}: {message}", connection.name);
                    match self.settings_open {
                        true => self.settings_panel.error = Some(shown),
                        false => self.report(Err(shown), cx),
                    }
                }
                ServerState::Offline(message)
            }
        };
        self.servers.states.insert(connection.id, state);
        cx.notify();
    }

    /// Lists every server's drives again, as Refresh on the Overview asks: a
    /// connected server keeps its `Api`, which open panes compare by identity;
    /// one that failed or needs pairing is tried from scratch.
    pub(crate) fn refresh_servers(&mut self, cx: &mut Context<Self>) {
        for connection in self.desktop_settings.connections.clone() {
            let api = match self.servers.states.get(&connection.id) {
                Some(ServerState::Connecting { .. }) => continue,
                Some(ServerState::Ready { api, .. }) => api.clone(),
                _ => {
                    self.servers.states.remove(&connection.id);
                    self.connect_server(connection, false, cx);
                    continue;
                }
            };
            cx.spawn(async move |this, cx| {
                let listing = api.clone();
                let result = cx.background_executor().spawn(async move { listing.drives() }).await;
                this.update(cx, |this, cx| {
                    // Removed, or reconnected with another credential, meanwhile.
                    let current = matches!(this.servers.states.get(&connection.id), Some(ServerState::Ready { api: now, .. }) if Arc::ptr_eq(now, &api));
                    if !current {
                        return;
                    }
                    this.connected(connection, listed(api, result), cx);
                })
                .ok();
            })
            .detach();
        }
    }

    /// What `connect` gives when the server lists `drives`, without the network.
    #[cfg(test)]
    pub(crate) fn server_answered(&mut self, connection: SavedConnection, api: Arc<Api>, drives: Vec<DriveStats>, cx: &mut Context<Self>) {
        self.connected(connection, Ok(Connected::Ready(api, drives)), cx);
    }

    /// Keeps the drives a server just listed for the next start-up, when they
    /// changed. Only a server that answered is remembered, never a failure.
    pub(crate) fn remember(&mut self, id: &str, drives: &[DriveStats], cx: &mut Context<Self>) {
        let drives: Vec<_> = drives.iter().map(RememberedDrive::of).collect();
        if self.servers.remembered.get(id) == Some(&drives) {
            return;
        }
        self.servers.remembered.insert(id.to_string(), drives);
        self.save_remembered(cx);
    }

    fn save_remembered(&self, cx: &mut Context<Self>) {
        let Ok(bytes) = serde_json::to_vec(&self.servers.remembered) else { return };
        self.persist(&crate::persist::SERVER_DRIVES, bytes, cx, |_, result, _| {
            if let Err(error) = result {
                log::debug!("{error}");
            }
        });
    }

    /// Connects again from scratch, as after pairing gave a new credential.
    pub(crate) fn reconnect_server(&mut self, connection: SavedConnection, open: bool, cx: &mut Context<Self>) {
        self.servers.states.remove(&connection.id);
        self.connect_server(connection, open, cx);
    }

    /// Opens a server's drive in a pane, closing Settings if it was the way there.
    pub(crate) fn open_server_drive(&mut self, target: PaneTarget, api: Arc<Api>, drive: String, cx: &mut Context<Self>) {
        self.settings_open = false;
        self.settings_panel.dialog = None;
        self.navigate_to(target, api, drive, cx);
    }

    /// Settings, at the pairing dialog, which is where codes are entered.
    pub(crate) fn ask_to_pair(&mut self, connection: SavedConnection, reason: String, cx: &mut Context<Self>) {
        self.settings_open = true;
        self.settings_section = SettingsSection::Connections;
        self.open_settings_dialog(SettingsDialog::Pair { connection, reason }, &[(SettingsField::Code, String::new())], cx);
    }

    /// Drops a removed server: its panes go back to the Overview, and the
    /// server and the keyring forget this computer.
    pub(crate) fn forget_server(&mut self, connection: SavedConnection, cx: &mut Context<Self>) {
        if self.servers.remembered.remove(&connection.id).is_some() {
            self.save_remembered(cx);
        }
        if let Some(ServerState::Ready { api, .. }) = self.servers.states.remove(&connection.id) {
            let inside: Vec<_> = self.tabs.iter().chain(self.split_panes.values()).filter(|pane| Arc::ptr_eq(&pane.api, &api)).map(|pane| super::PaneTarget(pane.id)).collect();
            for target in inside {
                self.navigate_in(target, String::new(), cx);
            }
        }
        cx.spawn(async move |this, cx| {
            let name = connection.name.clone();
            let result = cx.background_executor().spawn(async move { forget(&connection) }).await;
            if let Err(error) = result {
                this.update(cx, |this, cx| {
                    this.settings_panel.error = Some(format!("Removed {name}, but its credential is still in the keyring: {error:#}"));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
}
