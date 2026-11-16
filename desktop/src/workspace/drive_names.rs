//! Names given to drives with Rename, shown wherever the drive appears: the
//! Overview, the sidebar, the path bar. Reset Name returns to the drive's own.
//! The drive itself is not relabelled. Kept in the config dir by a key that
//! survives unplugging, unmounting and restarts.

use std::collections::HashMap;

use gpui::{Context, Window};

use super::operations::{DialogKind, OperationDialog};
use super::overview::Storage;
use super::Workspace;
use crate::api::Api;
use crate::devices::Volume;

/// Which drive a name belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DriveKey(String);

impl DriveKey {
    /// A drive of a provider, by its root name: this computer's or a server's.
    pub fn root(api: &Api, name: &str) -> Self {
        match api.server_url() {
            Some(url) => Self(format!("drive:{url} {name}")),
            None => Self(format!("drive:{name}")),
        }
    }

    /// A disk, phone or share, by its filesystem UUID or network location.
    pub fn volume(volume: &Volume) -> Self {
        Self(format!("volume:{}", volume.key))
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub(crate) struct DriveNames(HashMap<String, String>);

impl DriveNames {
    pub fn load() -> Self {
        std::fs::read(crate::persist::DRIVE_NAMES.path()).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }

    fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The given name, or the drive's own.
    pub fn name<'a>(&'a self, key: &DriveKey, own: &'a str) -> &'a str {
        self.0.get(&key.0).map_or(own, String::as_str)
    }

    pub fn is_renamed(&self, key: &DriveKey) -> bool {
        self.0.contains_key(&key.0)
    }

    /// An empty name, or the drive's own, forgets the given one.
    fn set(&mut self, key: &DriveKey, name: &str, own: &str) {
        let name = name.trim();
        if name.is_empty() || name == own {
            self.0.remove(&key.0);
        } else {
            self.0.insert(key.0.clone(), name.to_string());
        }
    }
}

impl Workspace {
    /// An Overview entry's key and own name; a server with no drives to show has none.
    pub(crate) fn drive_key(&self, storage: &Storage) -> Option<(DriveKey, String)> {
        match storage {
            Storage::Drive(drive) => Some((DriveKey::root(&self.home.api, &drive.name), drive.display_name().to_string())),
            Storage::Volume(volume) => Some((DriveKey::volume(volume), volume.name.clone())),
            Storage::ServerDrive { api, drive, .. } => Some((DriveKey::root(api, &drive.name), drive.display_name().to_string())),
            // The URL as `Api` keeps it, so the key matches once the server answers.
            Storage::Remembered { server, drive, .. } => {
                Some((DriveKey(format!("drive:{} {}", server.url.trim_end_matches('/'), drive.name)), drive.display_name().to_string()))
            }
            Storage::Server { .. } => None,
        }
    }

    /// An Overview entry's card or row, under the name the user gave it.
    pub(crate) fn drive_info(&self, storage: &Storage) -> crate::components::DriveInfo {
        let mut info = storage.info();
        if let Some((key, _)) = self.drive_key(storage) {
            if let Some(name) = self.drive_names.0.get(&key.0) {
                info.name = name.clone().into();
            }
        }
        info
    }

    /// A volume's name as the user sees it.
    pub(crate) fn volume_name<'a>(&'a self, volume: &'a Volume) -> &'a str {
        self.drive_names.name(&DriveKey::volume(volume), &volume.name)
    }

    /// A provider's drive or place, as the user sees it.
    pub(crate) fn root_name<'a>(&'a self, api: &Api, root: &'a crate::api::MountPoint) -> &'a str {
        self.drive_names.name(&DriveKey::root(api, &root.name), root.display_name())
    }

    pub(crate) fn open_drive_rename(&mut self, key: DriveKey, own: String, window: &mut Window, cx: &mut Context<Self>) {
        self.open_dialog(OperationDialog { target_id: self.active_pane.0, kind: DialogKind::RenameDrive { key, own } }, window, cx);
    }

    /// Rename, and Reset Name once the drive has a name of its own.
    pub(crate) fn drive_name_items(&self, key: &DriveKey, own: &str, cx: &mut Context<Self>) -> Vec<crate::components::MenuItem> {
        use crate::components::MenuItem;
        let (rename_key, rename_own) = (key.clone(), own.to_string());
        let mut items = vec![MenuItem::action("drive-rename", "Rename…").icon("pencil").on_select(cx.listener(move |this, _, window, cx| {
            this.place_menu = None;
            this.open_drive_rename(rename_key.clone(), rename_own.clone(), window, cx);
        }))];
        if self.drive_names.is_renamed(key) {
            let (key, own) = (key.clone(), own.to_string());
            items.push(MenuItem::action("drive-reset-name", "Reset Name").icon("x").on_select(cx.listener(move |this, _, _, cx| {
                this.place_menu = None;
                this.rename_drive(&key, None, &own, cx);
            })));
        }
        items
    }

    /// Stores a drive's name; `None` resets it to the drive's own.
    pub(crate) fn rename_drive(&mut self, key: &DriveKey, name: Option<&str>, own: &str, cx: &mut Context<Self>) {
        self.drive_names.set(key, name.unwrap_or(""), own);
        self.persist(&crate::persist::DRIVE_NAMES, self.drive_names.encode(), cx, |this, result, cx| {
            if let Err(error) = result {
                this.report(Err(format!("Could not save the drive's name: {error:#}")), cx);
            }
        });
        cx.notify();
    }
}
