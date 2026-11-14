//! The sidebar's devices: disks from UDisks2 (`udisks`), and phones and network
//! shares from GVfs (`gvfs`). Both are D-Bus services, and every call here
//! blocks, so it runs on a watcher thread or the background executor.

pub mod gvfs;
mod udisks;

use std::ffi::CString;
use std::path::{Path, PathBuf};

use anyhow::Result;

pub use gvfs::{Answer, Prompt, Prompts};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Disk,
    Phone,
    Network,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Space {
    pub total: u64,
    pub free: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Mounted {
        point: PathBuf,
        /// None when the filesystem does not report a size, as some phones do.
        space: Option<Space>,
        read_only: bool,
    },
    Unmounted,
    /// An encrypted volume that needs its passphrase.
    Locked,
}

/// Where a volume comes from, which decides how it is mounted and removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A UDisks block object. `crypto` is the encrypted block under an unlocked volume.
    Block { object: String, crypto: Option<String> },
    /// A mounted GVfs location, by its daemon's bus name and object.
    GvfsMount { daemon: String, object: String },
    /// A device a GVfs volume monitor found, such as a phone, not mounted yet.
    GvfsVolume { uri: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    /// Stable while the device is present.
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub state: State,
    pub source: Source,
    /// Removable media or a drive that can be ejected; otherwise it can only be unmounted.
    pub ejectable: bool,
}

impl Volume {
    pub fn mount_point(&self) -> Option<&Path> {
        match &self.state {
            State::Mounted { point, .. } => Some(point),
            State::Unmounted | State::Locked => None,
        }
    }

    /// Whether the row offers to eject or unmount.
    pub fn removable(&self) -> bool {
        self.ejectable || self.mount_point().is_some()
    }
}

/// One service's volumes, or why it could not list them.
pub enum Update {
    Disks(Result<Vec<Volume>, String>),
    Gvfs(Result<Vec<Volume>, String>),
}

/// Sends each service's list now and after every change, until the receiver is dropped.
pub fn watch(sender: async_channel::Sender<Update>) {
    udisks::watch(sender.clone());
    gvfs::watch(sender);
}

/// Mounts an unmounted volume and returns where. `passphrase` unlocks a locked one;
/// GVfs asks for anything else it needs through `prompts`.
pub fn mount(volume: &Volume, passphrase: Option<&str>, prompts: Prompts) -> Result<PathBuf> {
    match &volume.source {
        Source::Block { object, .. } => match (&volume.state, passphrase) {
            (State::Locked, Some(passphrase)) => udisks::unlock(object, passphrase),
            (State::Locked, None) => anyhow::bail!("{} needs its passphrase", volume.name),
            _ => udisks::mount(object),
        },
        Source::GvfsVolume { uri } => gvfs::mount(uri, prompts),
        Source::GvfsMount { .. } => volume.mount_point().map(Path::to_path_buf).ok_or_else(|| anyhow::anyhow!("{} is not mounted", volume.name)),
    }
}

/// Ejects a removable volume's drive, or unmounts a fixed one or a GVfs location.
pub fn remove(volume: &Volume) -> Result<()> {
    match &volume.source {
        Source::Block { object, crypto } if volume.ejectable => udisks::eject(object, crypto.as_deref()),
        Source::Block { object, crypto } => udisks::unmount(object, crypto.as_deref()),
        Source::GvfsMount { daemon, object } => gvfs::unmount(daemon, object),
        Source::GvfsVolume { .. } => Ok(()),
    }
}

/// A mounted filesystem's size and whether it is mounted read-only.
fn filesystem(point: &Path) -> Option<(Option<Space>, bool)> {
    use std::os::unix::ffi::OsStrExt as _;
    let path = CString::new(point.as_os_str().as_bytes()).ok()?;
    // SAFETY: statvfs fills the record, and the path is NUL-terminated.
    let mut raw: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut raw) } != 0 {
        return None;
    }
    let block = (if raw.f_frsize == 0 { raw.f_bsize } else { raw.f_frsize }) as u64;
    let total = (raw.f_blocks as u64).saturating_mul(block);
    let free = (raw.f_bavail as u64).saturating_mul(block).min(total);
    Some(((total > 0).then_some(Space { total, free }), raw.f_flag & libc::ST_RDONLY != 0))
}

fn mounted(point: PathBuf, read_only_device: bool) -> State {
    let (space, read_only) = filesystem(&point).unwrap_or((None, false));
    State::Mounted { point, space, read_only: read_only || read_only_device }
}

/// The service's own words for a failed call, without the D-Bus error name in front.
fn reason(error: zbus::Error) -> anyhow::Error {
    match error {
        zbus::Error::MethodError(_, Some(description), _) => anyhow::anyhow!(description),
        other => anyhow::Error::new(other),
    }
}
