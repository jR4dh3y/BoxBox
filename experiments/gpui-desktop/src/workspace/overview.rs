//! The Overview: every drive the window can open, in one list that the cards,
//! the table, the sidebar and the status bar share. This computer's drives and
//! devices come first, then each paired server's drives, as if they were local.

use std::sync::Arc;

use gpui::{Context, Window};

use crate::api::{Api, DriveStats, MountPoint};
use crate::components::{DriveInfo, DriveState, Usage};
use crate::devices::{Kind, State, Volume};
use crate::pane::Load;
use crate::settings_state::SavedConnection;
use crate::workspace::servers::ServerState;
use crate::workspace::Workspace;

/// The window's own provider, whose drives and places the sidebar always shows.
pub(crate) struct Home {
    pub api: Arc<Api>,
    pub roots: Load<Vec<MountPoint>>,
    pub drives: Load<Vec<DriveStats>>,
}

impl Home {
    pub fn new(api: Arc<Api>) -> Self {
        Self { api, roots: Load::Loading, drives: Load::Loading }
    }
}

#[derive(Clone)]
pub(crate) enum Storage {
    /// A drive of the window's own provider.
    Drive(DriveStats),
    /// A disk, phone or share the system reports.
    Volume(Volume),
    /// A drive on a paired server.
    ServerDrive { server: SavedConnection, api: Arc<Api>, drive: DriveStats },
    /// A saved server whose drives are not known: connecting, unpaired or offline,
    /// with the server's own words when it gave any.
    Server { server: SavedConnection, status: &'static str, reason: Option<String> },
}

pub(crate) fn volume_glyph(volume: &Volume) -> &'static str {
    match (&volume.state, volume.kind) {
        (State::Locked, _) => "lock",
        (_, Kind::Disk) => "hard-drive",
        (_, Kind::Phone) => "smartphone",
        (_, Kind::Network) => "globe",
    }
}

fn usage(drive: &DriveStats) -> DriveState {
    DriveState::Usage(Usage { total: drive.total_bytes, used: drive.used_bytes, free: drive.free_bytes, pct: drive.used_pct })
}

impl Storage {
    pub fn info(&self) -> DriveInfo {
        match self {
            Storage::Drive(drive) => DriveInfo {
                name: drive.display_name().to_string().into(),
                glyph: "hard-drive",
                state: usage(drive),
                read_only: drive.read_only,
                origin: None,
                device: drive.device.clone(),
                fs_type: drive.fs_type.clone(),
            },
            Storage::Volume(volume) => {
                let status = |status: &'static str| DriveState::Status { status: status.into(), size: volume.size };
                let (state, read_only) = match &volume.state {
                    State::Mounted { space: Some(space), read_only, .. } if space.total > 0 => {
                        let used = space.total.saturating_sub(space.free);
                        let pct = used as f64 * 100. / space.total as f64;
                        (DriveState::Usage(Usage { total: space.total, used, free: space.free, pct }), *read_only)
                    }
                    State::Mounted { read_only, .. } => (status("Connected"), *read_only),
                    State::Unmounted => (status("Not mounted"), false),
                    State::Locked => (status("Encrypted"), false),
                };
                DriveInfo {
                    name: volume.name.clone().into(),
                    glyph: volume_glyph(volume),
                    state,
                    read_only,
                    origin: None,
                    device: volume.device.clone(),
                    fs_type: volume.fs_type.clone(),
                }
            }
            Storage::ServerDrive { server, drive, .. } => DriveInfo {
                name: drive.display_name().to_string().into(),
                glyph: "server",
                state: usage(drive),
                read_only: drive.read_only,
                origin: Some(server.name.clone().into()),
                device: drive.device.clone(),
                fs_type: drive.fs_type.clone(),
            },
            Storage::Server { server, status, .. } => DriveInfo {
                name: server.name.clone().into(),
                glyph: "server",
                state: DriveState::Status { status: (*status).into(), size: None },
                read_only: false,
                origin: None,
                device: Some(server.url.clone()),
                fs_type: None,
            },
        }
    }

    /// Whether the pane is on this entry: the same provider, at or under its drive.
    pub fn holds(&self, workspace: &Workspace, api: &Arc<Api>, path: &str) -> bool {
        let under = |root: &str| path == root || path.starts_with(&format!("{root}/"));
        match self {
            Storage::Drive(drive) => Arc::ptr_eq(api, &workspace.home.api) && under(&drive.name),
            Storage::ServerDrive { api: server, drive, .. } => Arc::ptr_eq(api, server) && under(&drive.name),
            Storage::Volume(_) | Storage::Server { .. } => false,
        }
    }
}

impl Workspace {
    /// This computer's drives, its disks, phones and shares, then each saved
    /// server's drives, or one entry for a server that has none to show yet.
    /// None until this computer's drives load.
    pub(crate) fn storage(&self) -> Option<Vec<Storage>> {
        let Load::Loaded(drives) = &self.home.drives else { return None };
        let local = self.home.api.is_local();
        let mut storage: Vec<Storage> = drives.iter().cloned().map(Storage::Drive).collect();
        if local {
            storage.extend(self.devices.volumes().cloned().map(Storage::Volume));
        }
        for server in &self.desktop_settings.connections {
            let (status, reason) = match self.servers.state(&server.id) {
                Some(ServerState::Ready { api, drives }) if !drives.is_empty() => {
                    storage.extend(drives.iter().map(|drive| Storage::ServerDrive { server: server.clone(), api: api.clone(), drive: drive.clone() }));
                    continue;
                }
                Some(ServerState::Ready { .. }) => ("No drives shared", None),
                None | Some(ServerState::Connecting) => ("Connecting…", None),
                Some(ServerState::NeedsPairing(reason)) => ("Not paired", Some(reason.clone())),
                Some(ServerState::Offline(reason)) => ("Can’t reach the server", Some(reason.clone())),
            };
            storage.push(Storage::Server { server: server.clone(), status, reason });
        }
        Some(storage)
    }

    pub(crate) fn open_storage(&mut self, storage: Storage, window: &mut Window, cx: &mut Context<Self>) {
        match storage {
            Storage::Drive(drive) => {
                let home = self.home.api.clone();
                self.navigate_to(self.active_pane, home, drive.name, cx);
            }
            Storage::Volume(volume) => self.open_volume(volume, window, cx),
            Storage::ServerDrive { api, drive, .. } => self.navigate_to(self.active_pane, api, drive.name, cx),
            Storage::Server { server, .. } => self.connect_server(server, true, cx),
        }
    }
}
