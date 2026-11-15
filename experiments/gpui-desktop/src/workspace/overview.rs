//! The Overview: every drive and device a pane can open, in one list that the
//! cards, the table, the sidebar and the status bar share.

use gpui::{Context, Window};

use crate::api::DriveStats;
use crate::components::{DriveInfo, DriveState, Usage};
use crate::devices::{Kind, State, Volume};
use crate::pane::{Load, Pane};
use crate::workspace::Workspace;

/// A drive the provider lists, or a disk, phone or share the system reports.
#[derive(Clone)]
pub(crate) enum Storage {
    Drive(DriveStats),
    Volume(Volume),
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
                let (state, read_only) = match &volume.state {
                    State::Mounted { space: Some(space), read_only, .. } if space.total > 0 => {
                        let used = space.total.saturating_sub(space.free);
                        let pct = used as f64 * 100. / space.total as f64;
                        (DriveState::Usage(Usage { total: space.total, used, free: space.free, pct }), *read_only)
                    }
                    State::Mounted { read_only, .. } => (DriveState::Status("Connected".into()), *read_only),
                    State::Unmounted => (DriveState::Status("Not mounted".into()), false),
                    State::Locked => (DriveState::Status("Encrypted".into()), false),
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
        }
    }
}

impl Workspace {
    /// The pane's drives, then, on this computer, its disks, phones and shares.
    /// None until the drives load.
    pub(crate) fn storage(&self, pane: &Pane) -> Option<Vec<Storage>> {
        let Load::Loaded(drives) = &pane.drives else { return None };
        let volumes = pane.api.is_local().then(|| self.devices.volumes()).into_iter().flatten();
        Some(drives.iter().cloned().map(Storage::Drive).chain(volumes.cloned().map(Storage::Volume)).collect())
    }

    pub(crate) fn open_storage(&mut self, storage: Storage, window: &mut Window, cx: &mut Context<Self>) {
        match storage {
            Storage::Drive(drive) => self.navigate(drive.name, cx),
            Storage::Volume(volume) => self.open_volume(volume, window, cx),
        }
    }
}
