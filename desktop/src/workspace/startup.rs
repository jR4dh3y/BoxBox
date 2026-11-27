//! The first folder and the drive list, read while GPUI starts.
//!
//! GPUI's start-up holds the UI thread for most of a launch, and work spawned from
//! the window begins only after the first frame. Reading on a thread started before
//! GPUI lets the first frame show the folder instead of an empty pane.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use async_channel::{Receiver, TryRecvError};
use gpui::Context;

use super::{launch_path, PaneTarget, Preferences, StartLocation, Workspace};
use crate::api::{Api, DriveStats, FileInfo, MountPoint};
use crate::pane::View;

/// What `Workspace::new` starts from.
pub struct Startup {
    pub(super) api: Arc<Api>,
    pub(super) preferences: Preferences,
    pub(super) folder_views: super::folder_views::FolderViews,
    pub(super) drive_names: super::drive_names::DriveNames,
    /// The virtual path to open; empty for the drive list.
    pub(super) path: String,
    /// The path came from the start location, not the launch; if it no longer
    /// opens, the window starts on the Overview instead of an error.
    pub(super) fallback: bool,
    pub(super) prefetch: Option<Prefetch>,
}

pub(super) struct Prefetch {
    remembered: Receiver<super::servers::Remembered>,
    listing: Receiver<ListingRead>,
    drives: Receiver<DrivesRead>,
}

/// A folder's entries, `arrange`d, and what the pane needs to know about the folder.
pub(super) struct ListingRead {
    pub(super) items: Result<Vec<FileInfo>>,
    pub(super) real: Option<PathBuf>,
    pub(super) writable: bool,
}

pub(super) struct DrivesRead {
    pub(super) roots: Result<Vec<MountPoint>>,
    pub(super) drives: Result<Vec<DriveStats>>,
}

impl Startup {
    pub(crate) fn integrates_window_controls(&self) -> bool {
        #[cfg(windows)]
        {
            self.preferences.window_controls_in_tab_bar
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// Starts reading the launch folder and the drives at once.
    pub fn prefetch(api: Api, argument: &str) -> Self {
        let mut start = Self::deferred(api, argument);
        let (remembered_sender, remembered) = async_channel::bounded(1);
        let (listing_sender, listing) = async_channel::bounded(1);
        let (drives_sender, drives) = async_channel::bounded(1);
        let api = start.api.clone();
        let path = start.path.clone();
        let view = View {
            sort: Some(start.folder_views.sort(&api, &path, &start.preferences)),
            show_hidden_files: start.preferences.show_hidden_files,
        };
        let reader = std::thread::Builder::new().name("first listing".into()).spawn(move || {
            // A small file, which lays out the servers' drives before they answer.
            let _ = remembered_sender.try_send(super::servers::load_remembered());
            // The drives come first: a large folder would hold them back.
            let _ = drives_sender.try_send(read_drives(&api));
            if !path.is_empty() {
                let _ = listing_sender.try_send(read_listing(&api, &path, view));
            }
            // What this thread and the stat threads freed: 2.4 MB for 20,000 entries.
            crate::heap::release_freed();
        });
        start.prefetch = reader.is_ok().then_some(Prefetch { remembered, listing, drives });
        start
    }

    /// Reads nothing until the window asks, as tests need.
    pub fn deferred(api: Api, argument: &str) -> Self {
        let api = Arc::new(api);
        let preferences = Preferences::load();
        let (path, fallback) = match argument.is_empty() {
            false => (launch_path(&api, argument), false),
            true => (start_path(&api, preferences.start_location), true),
        };
        Self { api, preferences, folder_views: super::folder_views::FolderViews::load(), drive_names: super::drive_names::DriveNames::load(), path, fallback, prefetch: None }
    }
}

/// Where a launch without a folder opens.
pub(super) fn start_path(api: &Api, start: StartLocation) -> String {
    match start {
        StartLocation::Overview => String::new(),
        StartLocation::Home => {
            let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
            std::env::var_os(home_var).and_then(|home| api.virtual_path(std::path::Path::new(&home)))
        }.unwrap_or_default(),
        StartLocation::Last => std::fs::read_to_string(crate::persist::LAST_LOCATION.path()).map(|path| path.trim().to_string()).unwrap_or_default(),
    }
}

/// Runs off the UI thread.
pub(super) fn read_listing(api: &Api, path: &str, view: View) -> ListingRead {
    let items = api.list(path).map(|items| {
        let _span = crate::profile::span("arrange").with(|| format!("{} entries", items.len()));
        crate::pane::arrange(items, view)
    });
    let real = api.local_path(path).ok().flatten();
    // A local listing with no folder behind it, such as Recent, takes nothing.
    let writable = match &real {
        Some(real) => crate::local::writable(real),
        None => !api.is_local(),
    };
    ListingRead { items, real, writable }
}

/// Runs off the UI thread.
pub(super) fn read_drives(api: &Api) -> DrivesRead {
    DrivesRead { roots: api.roots(), drives: api.drives() }
}

impl Workspace {
    /// Applies what the start-up thread read, or waits for it. A read that never
    /// arrives (its thread failed) falls back to the usual load.
    pub(super) fn receive_prefetch(&mut self, prefetch: Prefetch, fallback: bool, cx: &mut Context<Self>) {
        let target = PaneTarget(0);
        receive(self, prefetch.remembered, cx, |this, remembered, cx| {
            this.servers.set_remembered(remembered.unwrap_or_default());
            cx.notify();
        });
        let journey = crate::profile::journey("load drives", String::new);
        receive(self, prefetch.drives, cx, move |this, read, cx| match read {
            // The start-up thread reads this computer's drives.
            Some(read) => this.apply_drives(target, this.home.api.clone(), read, journey, cx),
            None => this.load_drives(target, cx),
        });
        let Some(pane) = self.pane_mut_by_id(target.0) else { return };
        let generation = pane.begin_load();
        if pane.is_at_root() {
            return;
        }
        let view = pane.view();
        let journey = crate::profile::journey("open folder", || format!("{}  (start-up)", pane.path));
        receive(self, prefetch.listing, cx, move |this, read, cx| match read {
            // The remembered folder is gone: start on the Overview, not an error.
            Some(read) if fallback && read.items.is_err() => {
                // Unless the user has opened another folder since.
                if let Some(pane) = this.pane_mut_by_id(target.0).filter(|pane| pane.generation == generation) {
                    pane.start_on_overview();
                }
                cx.notify();
            }
            Some(read) => this.apply_listing(target, generation, view, read, journey, cx),
            None => this.load_pane(target, cx),
        });
    }
}

/// Calls `apply` now when the value is ready, else once it arrives.
fn receive<T: 'static>(
    this: &mut Workspace,
    receiver: Receiver<T>,
    cx: &mut Context<Workspace>,
    apply: impl FnOnce(&mut Workspace, Option<T>, &mut Context<Workspace>) + 'static,
) {
    match receiver.try_recv() {
        Ok(value) => apply(this, Some(value), cx),
        Err(TryRecvError::Closed) => apply(this, None, cx),
        Err(TryRecvError::Empty) => cx
            .spawn(async move |this, cx| {
                let value = receiver.recv().await.ok();
                this.update(cx, |this, cx| apply(this, value, cx)).ok();
            })
            .detach(),
    }
}
