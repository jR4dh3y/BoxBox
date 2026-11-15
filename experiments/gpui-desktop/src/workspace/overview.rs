//! The Overview: every drive and device a pane can open, in one list that the
//! cards, the table, the sidebar and the status bar share.

use gpui::{Context, Window};

use crate::api::DriveStats;
use crate::components::{DriveInfo, DriveState, Usage};
use crate::devices::{Kind, State, Volume};
use crate::pane::{Load, Pane};
use crate::settings_state::SavedConnection;
use crate::workspace::Workspace;

/// A drive the provider lists, a disk, phone or share the system reports, or a
/// paired BoxBox server.
#[derive(Clone)]
pub(crate) enum Storage {
    Drive(DriveStats),
    Volume(Volume),
    Server(SavedConnection),
}

pub(crate) fn volume_glyph(volume: &Volume) -> &'static str {
    match (&volume.state, volume.kind) {
        (State::Locked, _) => "lock",
        (_, Kind::Disk) => "hard-drive",
        (_, Kind::Phone) => "smartphone",
        (_, Kind::Network) => "globe",
    }
}

impl Storage {
    pub fn info(&self) -> DriveInfo {
        match self {
            Storage::Drive(drive) => DriveInfo {
                name: drive.display_name().to_string().into(),
                glyph: "hard-drive",
                state: DriveState::Usage(Usage { total: drive.total_bytes, used: drive.used_bytes, free: drive.free_bytes, pct: drive.used_pct }),
                read_only: drive.read_only,
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
                    device: volume.device.clone(),
                    fs_type: volume.fs_type.clone(),
                }
            }
            // Its drives show once it is open, in its own tab.
            Storage::Server(connection) => DriveInfo {
                name: connection.name.clone().into(),
                glyph: "server",
                state: DriveState::Status { status: "BoxBox server".into(), size: None },
                read_only: false,
                device: Some(connection.url.clone()),
                fs_type: None,
            },
        }
    }
}

impl Workspace {
    /// The pane's drives, then, on this computer, its disks, phones and shares,
    /// and the saved servers. None until the drives load.
    pub(crate) fn storage(&self, pane: &Pane) -> Option<Vec<Storage>> {
        let Load::Loaded(drives) = &pane.drives else { return None };
        let local = pane.api.is_local();
        let volumes = local.then(|| self.devices.volumes()).into_iter().flatten().cloned().map(Storage::Volume);
        let servers = local.then_some(&self.desktop_settings.connections).into_iter().flatten().cloned().map(Storage::Server);
        Some(drives.iter().cloned().map(Storage::Drive).chain(volumes).chain(servers).collect())
    }

    pub(crate) fn open_storage(&mut self, storage: Storage, window: &mut Window, cx: &mut Context<Self>) {
        match storage {
            Storage::Drive(drive) => self.navigate(drive.name, cx),
            Storage::Volume(volume) => self.open_volume(volume, window, cx),
            Storage::Server(connection) => self.connect_server(connection, cx),
        }
    }
}
