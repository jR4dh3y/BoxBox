//! The Properties dialog's state: what is shown, and the details read off the UI
//! thread (owner, permissions, and the size of folders and selections).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gpui::Context;

use super::operations::PaneTarget;
use super::{Load, Workspace};
use crate::api::{Api, FileInfo};

pub(crate) struct PropertiesState {
    pub items: Vec<FileInfo>,
    pub provider_label: String,
    /// Owner and permissions of a single local item.
    pub ownership: Option<Load<Ownership>>,
    /// Everything inside the folders, plus the selected files.
    pub totals: Load<Totals>,
    cancel: Arc<AtomicBool>,
}

impl Drop for PropertiesState {
    fn drop(&mut self) {
        // Closing the dialog stops a walk of a large folder.
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Ownership {
    pub path: PathBuf,
    pub mode: u32,
    pub owner: String,
    pub group: String,
    /// Only the owner, or root, may change the permissions.
    pub editable: bool,
    pub accessed: Option<i64>,
    pub created: Option<i64>,
    /// Where a symbolic link points; a link has no permissions of its own.
    pub link_target: Option<String>,
}

impl Ownership {
    pub fn is_dir(&self) -> bool {
        self.mode & libc::S_IFMT == libc::S_IFDIR
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Totals {
    pub files: u64,
    pub folders: u64,
    pub bytes: u64,
}

impl Workspace {
    /// Opens Properties for `items` and starts reading what a listing does not hold.
    pub(crate) fn show_properties(&mut self, target: PaneTarget, items: Vec<FileInfo>, cx: &mut Context<Self>) {
        let Some(api) = self.pane_by_id(target.0).map(|pane| pane.api.clone()) else { return };
        if items.is_empty() {
            return;
        }
        let needs_walk = items.len() > 1 || items[0].is_dir;
        let cancel = Arc::new(AtomicBool::new(false));
        self.properties = Some(PropertiesState {
            provider_label: api.server_url().unwrap_or("Local storage").to_string(),
            ownership: (items.len() == 1 && api.is_local()).then_some(Load::Loading),
            totals: if needs_walk { Load::Loading } else { Load::Loaded(Totals { files: 1, folders: 0, bytes: items[0].size }) },
            items: items.clone(),
            cancel: cancel.clone(),
        });
        self.context_menu = None;
        cx.notify();
        let shown = cancel.clone();
        cx.spawn(async move |this, cx| {
            let (ownership, totals) = cx
                .background_executor()
                .spawn(async move {
                    let real: Option<Vec<PathBuf>> = items.iter().map(|item| api.local_path(&item.path).ok().flatten()).collect();
                    let ownership = match (items.as_slice(), &real) {
                        ([_], Some(paths)) => Some(ownership(&paths[0]).map_err(|error| format!("{error:#}"))),
                        _ => None,
                    };
                    let totals = needs_walk.then(|| match &real {
                        Some(paths) => local_totals(paths, &cancel),
                        None => remote_totals(&api, &items, &cancel),
                    });
                    (ownership, totals)
                })
                .await;
            this.update(cx, |this, cx| {
                // Another Properties may have opened meanwhile.
                let Some(state) = this.properties.as_mut().filter(|state| Arc::ptr_eq(&state.cancel, &shown)) else { return };
                if let Some(ownership) = ownership {
                    state.ownership = Some(match ownership {
                        Ok(ownership) => Load::Loaded(ownership),
                        Err(error) => Load::Failed(error),
                    });
                }
                match totals {
                    Some(Some(totals)) => state.totals = Load::Loaded(totals),
                    Some(None) => state.totals = Load::Failed("Could not read every item".into()),
                    None => {}
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// Who a permission is for: the bits of each, as a shift of `rwx` (0o7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Owner = 6,
    Group = 3,
    Others = 0,
}

/// What one class may do, as Nautilus names it. A file's execute bit is the
/// separate "Run as a program"; a folder's is part of its access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Access {
    None,
    Read,
    ReadWrite,
    /// Folders: `r--`, `r-x` and `rwx`.
    List,
    Open,
    Full,
    /// Any other combination, which the menu does not offer.
    Custom,
}

impl Access {
    pub const FILE: [(Self, &'static str); 3] = [(Self::None, "None"), (Self::Read, "Read only"), (Self::ReadWrite, "Read & write")];
    pub const FOLDER: [(Self, &'static str); 4] =
        [(Self::None, "None"), (Self::List, "List files only"), (Self::Open, "Access files"), (Self::Full, "Create and delete")];

    pub fn of(mode: u32, class: Class, folder: bool) -> Self {
        let bits = (mode >> class as u32) & 0o7;
        match (folder, bits) {
            (false, bits) => match bits & 0o6 {
                0 => Self::None,
                0o4 => Self::Read,
                0o6 => Self::ReadWrite,
                _ => Self::Custom,
            },
            (true, 0) => Self::None,
            (true, 0o4) => Self::List,
            (true, 0o5) => Self::Open,
            (true, 0o7) => Self::Full,
            (true, _) => Self::Custom,
        }
    }

    /// `mode` with this access for `class`; a file keeps its execute bit.
    fn apply(self, mode: u32, class: Class) -> u32 {
        let (bits, keep) = match self {
            Self::None => (0, 0o1),
            Self::Read => (0o4, 0o1),
            Self::ReadWrite => (0o6, 0o1),
            Self::List => (0o4, 0),
            Self::Open => (0o5, 0),
            Self::Full => (0o7, 0),
            Self::Custom => return mode,
        };
        let shift = class as u32;
        (mode & !(0o7 << shift)) | (bits << shift) | (mode & (keep << shift))
    }
}

/// "Run as a program": execute for each class that may read, or for none.
pub(crate) fn runs(mode: u32) -> bool {
    mode & 0o111 != 0
}

impl Workspace {
    pub(crate) fn set_access(&mut self, class: Class, access: Access, cx: &mut Context<Self>) {
        self.change_mode(|mode| access.apply(mode, class), cx);
    }

    pub(crate) fn set_runs(&mut self, run: bool, cx: &mut Context<Self>) {
        self.change_mode(|mode| if run { mode | ((mode & 0o444) >> 2) } else { mode & !0o111 }, cx);
    }

    /// Changes the mode at once, as Nautilus and Thunar do, then reads it back.
    fn change_mode(&mut self, change: impl FnOnce(u32) -> u32, cx: &mut Context<Self>) {
        let Some(state) = self.properties.as_ref() else { return };
        let Some(Load::Loaded(shown_mode)) = &state.ownership else { return };
        let (path, mode, shown) = (shown_mode.path.clone(), change(shown_mode.mode) & 0o7777, state.cancel.clone());
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { chmod(&path, mode).and_then(|()| ownership(&path)) }).await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(read) => {
                        if let Some(state) = this.properties.as_mut().filter(|state| Arc::ptr_eq(&state.cancel, &shown)) {
                            state.ownership = Some(Load::Loaded(read));
                        }
                    }
                    Err(error) => this.report(Err(format!("Could not change the permissions: {error:#}")), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Gives everything inside the folder its permissions: folders the same,
    /// files the same without execute, unless they were programs already.
    pub(crate) fn apply_permissions_to_contents(&mut self, cx: &mut Context<Self>) {
        let Some(Load::Loaded(ownership)) = self.properties.as_ref().and_then(|state| state.ownership.as_ref()) else { return };
        let (root, mode) = (ownership.path.clone(), ownership.mode & 0o7777);
        let generation = self.begin_operation("Applying permissions…");
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { apply_below(&root, mode) }).await;
            this.update(cx, |this, cx| {
                let result = result.map(|failed| {
                    if failed > 0 {
                        this.report(Err(format!("{failed} items inside kept their permissions: they are not yours")), cx);
                    }
                });
                this.finish_operation(generation, result, cx);
            })
            .ok();
        })
        .detach();
    }
}

fn chmod(path: &Path, mode: u32) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

/// Returns how many items could not be changed. Links are not followed.
fn apply_below(root: &Path, mode: u32) -> anyhow::Result<usize> {
    use std::os::unix::fs::MetadataExt as _;
    let mut failed = 0;
    let mut pending: Vec<PathBuf> = std::fs::read_dir(root)?.flatten().map(|entry| entry.path()).collect();
    while let Some(path) = pending.pop() {
        let Ok(metadata) = std::fs::symlink_metadata(&path) else { continue };
        let wanted = if metadata.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                pending.extend(entries.flatten().map(|entry| entry.path()));
            }
            mode
        } else if metadata.is_file() {
            // A program stays one, with the folder's execute bits.
            if metadata.mode() & 0o100 != 0 { mode } else { mode & !0o111 }
        } else {
            continue;
        };
        if metadata.mode() & 0o7777 != wanted && chmod(&path, wanted).is_err() {
            failed += 1;
        }
    }
    Ok(failed)
}

fn ownership(path: &Path) -> anyhow::Result<Ownership> {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = std::fs::symlink_metadata(path)?;
    let millis = |time: std::io::Result<std::time::SystemTime>| {
        time.ok().and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok()).map(|since| since.as_millis() as i64)
    };
    // SAFETY: geteuid cannot fail.
    let me = unsafe { libc::geteuid() };
    let link_target = metadata.file_type().is_symlink().then(|| std::fs::read_link(path).map(|target| target.display().to_string()).unwrap_or_default());
    Ok(Ownership {
        path: path.to_path_buf(),
        mode: metadata.mode(),
        owner: user_name(metadata.uid()),
        group: group_name(metadata.gid()),
        editable: link_target.is_none() && (me == 0 || me == metadata.uid()),
        accessed: millis(metadata.accessed()),
        created: millis(metadata.created()),
        link_target,
    })
}

/// The account name for `uid`, or the number when the system has none.
fn user_name(uid: u32) -> String {
    let mut buffer = vec![0 as libc::c_char; 4096];
    // SAFETY: `passwd` and `buffer` outlive the call, which writes only within them.
    unsafe {
        let mut passwd: libc::passwd = std::mem::zeroed();
        let mut found = std::ptr::null_mut();
        if libc::getpwuid_r(uid, &mut passwd, buffer.as_mut_ptr(), buffer.len(), &mut found) == 0 && !found.is_null() {
            return std::ffi::CStr::from_ptr(passwd.pw_name).to_string_lossy().into_owned();
        }
    }
    uid.to_string()
}

fn group_name(gid: u32) -> String {
    let mut buffer = vec![0 as libc::c_char; 4096];
    // SAFETY: as in `user_name`.
    unsafe {
        let mut group: libc::group = std::mem::zeroed();
        let mut found = std::ptr::null_mut();
        if libc::getgrgid_r(gid, &mut group, buffer.as_mut_ptr(), buffer.len(), &mut found) == 0 && !found.is_null() {
            return std::ffi::CStr::from_ptr(group.gr_name).to_string_lossy().into_owned();
        }
    }
    gid.to_string()
}

/// `drwxr-xr-x`: the kind, then read, write and execute for owner, group and others,
/// with the set-id and sticky bits where `ls` shows them.
pub(crate) fn permission_string(mode: u32) -> String {
    let kind = match mode & libc::S_IFMT {
        libc::S_IFDIR => 'd',
        libc::S_IFLNK => 'l',
        _ => '-',
    };
    let bit = |mask: u32, letter: char| if mode & mask != 0 { letter } else { '-' };
    let execute = |mask: u32, special: u32, set: char| match (mode & mask != 0, mode & special != 0) {
        (true, true) => set,
        (false, true) => set.to_ascii_uppercase(),
        (true, false) => 'x',
        (false, false) => '-',
    };
    [
        kind,
        bit(0o400, 'r'),
        bit(0o200, 'w'),
        execute(0o100, 0o4000, 's'),
        bit(0o040, 'r'),
        bit(0o020, 'w'),
        execute(0o010, 0o2000, 's'),
        bit(0o004, 'r'),
        bit(0o002, 'w'),
        execute(0o001, 0o1000, 't'),
    ]
    .into_iter()
    .collect()
}

/// Walks the paths without following links. `None` once cancelled.
fn local_totals(paths: &[PathBuf], cancel: &AtomicBool) -> Option<Totals> {
    let mut totals = Totals::default();
    let mut pending = paths.to_vec();
    while let Some(path) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&path) else { continue };
        if metadata.is_dir() {
            totals.folders += 1;
            if let Ok(entries) = std::fs::read_dir(&path) {
                pending.extend(entries.flatten().map(|entry| entry.path()));
            }
        } else {
            totals.files += 1;
            totals.bytes += metadata.len();
        }
    }
    Some(totals)
}

fn remote_totals(api: &Api, items: &[FileInfo], cancel: &AtomicBool) -> Option<Totals> {
    let mut totals = Totals::default();
    let mut pending = items.to_vec();
    while let Some(item) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        if item.is_dir {
            totals.folders += 1;
            pending.extend(api.list(&item.path).ok()?);
        } else {
            totals.files += 1;
            totals.bytes += item.size;
        }
    }
    Some(totals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_levels_read_and_set_the_mode() {
        assert_eq!(Access::of(0o640, Class::Owner, false), Access::ReadWrite);
        assert_eq!(Access::of(0o640, Class::Group, false), Access::Read);
        assert_eq!(Access::of(0o640, Class::Others, false), Access::None);
        assert_eq!(Access::of(0o620, Class::Group, false), Access::Custom);
        assert_eq!(Access::of(0o750, Class::Group, true), Access::Open);
        // A file keeps whether it runs; a folder's access sets all three bits.
        assert_eq!(Access::Read.apply(0o755, Class::Owner), 0o555);
        assert_eq!(Access::None.apply(0o755, Class::Others), 0o751);
        assert_eq!(Access::Full.apply(0o700, Class::Group), 0o770);
        assert_eq!(Access::List.apply(0o755, Class::Others), 0o754);
    }

    #[test]
    fn permissions_read_like_ls() {
        assert_eq!(permission_string(libc::S_IFDIR | 0o755), "drwxr-xr-x");
        assert_eq!(permission_string(libc::S_IFREG | 0o640), "-rw-r-----");
        assert_eq!(permission_string(libc::S_IFREG | 0o4755), "-rwsr-xr-x");
        assert_eq!(permission_string(libc::S_IFDIR | 0o1777), "drwxrwxrwt");
        assert_eq!(permission_string(libc::S_IFREG | 0o2644), "-rw-r-Sr--");
    }

    #[test]
    fn totals_count_folders_files_and_bytes_below_the_selection() {
        let root = std::env::temp_dir().join(format!("boxbox-totals-{}", std::process::id()));
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("a/one"), [0; 10]).unwrap();
        std::fs::write(root.join("a/b/two"), [0; 5]).unwrap();
        std::fs::write(root.join("three"), [0; 1]).unwrap();
        let totals = local_totals(&[root.join("a"), root.join("three")], &AtomicBool::new(false)).unwrap();
        assert_eq!(totals, Totals { files: 3, folders: 2, bytes: 16 });
        assert!(local_totals(&[root.clone()], &AtomicBool::new(true)).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}
