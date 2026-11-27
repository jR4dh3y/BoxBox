use std::{collections::HashMap, path::PathBuf};

use gpui::Context;

use super::{PaneTarget, Workspace};

#[derive(Default)]
pub(crate) struct DirectoryWatches {
    #[cfg(target_os = "linux")]
    events: Option<std::fs::File>,
    #[cfg(target_os = "linux")]
    paths: HashMap<usize, (PathBuf, i32)>,
    #[cfg(windows)]
    paths: HashMap<usize, (PathBuf, ChangeNotification)>,
}

impl DirectoryWatches {
    pub(crate) fn watch(&mut self, pane: usize, path: PathBuf) {
        #[cfg(target_os = "linux")]
        {
            use std::os::{fd::{AsRawFd, FromRawFd}, unix::ffi::OsStrExt};
            if self.paths.get(&pane).is_some_and(|(current, _)| *current == path) { return; }
            self.remove(pane);
            if self.events.is_none() {
                // The File owns this nonblocking descriptor and closes it with the workspace.
                let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
                if fd < 0 { log::warn!("Could not watch folders: {}", std::io::Error::last_os_error()); return; }
                self.events = Some(unsafe { std::fs::File::from_raw_fd(fd) });
            }
            let Ok(name) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return; };
            let Some(events) = &self.events else { return; };
            let mask = libc::IN_CREATE | libc::IN_DELETE | libc::IN_MOVED_FROM | libc::IN_MOVED_TO
                | libc::IN_CLOSE_WRITE | libc::IN_ATTRIB | libc::IN_DELETE_SELF | libc::IN_MOVE_SELF;
            let watch = unsafe { libc::inotify_add_watch(events.as_raw_fd(), name.as_ptr(), mask) };
            if watch >= 0 { self.paths.insert(pane, (path, watch)); }
            else { log::warn!("Could not watch folder: {}", std::io::Error::last_os_error()); }
        }
        #[cfg(windows)]
        {
            if self.paths.get(&pane).is_some_and(|(current, _)| current == &path) { return; }
            self.remove(pane);
            if let Some(watch) = ChangeNotification::new(&path) {
                self.paths.insert(pane, (path, watch));
            }
        }
    }

    /// Whether `pane` already watches `path`.
    #[cfg(windows)]
    fn watches(&self, pane: usize, path: &std::path::Path) -> bool {
        self.paths.get(&pane).is_some_and(|(current, _)| current == path)
    }

    /// Takes a watch made off the window thread.
    #[cfg(windows)]
    fn install(&mut self, pane: usize, path: PathBuf, watch: ChangeNotification) {
        self.remove(pane);
        self.paths.insert(pane, (path, watch));
    }

    pub(crate) fn remove(&mut self, pane: usize) {
        #[cfg(windows)]
        self.paths.remove(&pane);
        #[cfg(target_os = "linux")]
        if let Some((_, descriptor)) = self.paths.remove(&pane) {
            if !self.paths.values().any(|(_, watch)| *watch == descriptor) {
                use std::os::fd::AsRawFd;
                if let Some(events) = &self.events {
                    unsafe { libc::inotify_rm_watch(events.as_raw_fd(), descriptor); }
                }
            }
        }
    }

    pub(crate) fn changed(&mut self, active: &[usize]) -> Vec<usize> {
        let stale: Vec<_> = self.paths.keys().copied().filter(|id| !active.contains(id)).collect();
        for id in stale { self.remove(id); }
        let mut changed = std::collections::HashSet::new();
        let mut ended: Vec<usize> = Vec::new();
        #[cfg(target_os = "linux")]
        if let Some(events) = &mut self.events {
            use std::io::Read;
            let mut buffer = [0u8; 16384];
            // Bound work per tick even if another process is continuously changing files.
            for _ in 0..16 {
                let size = match events.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(size) => size,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) => { log::warn!("Folder watch failed: {error}"); break; }
                };
                let mut offset = 0;
                while offset + 16 <= size {
                    let word = |start| i32::from_ne_bytes(buffer[start..start + 4].try_into().unwrap());
                    let descriptor = word(offset);
                    let mask = word(offset + 4) as u32;
                    let length = word(offset + 12) as u32 as usize;
                    // The watched folder is gone or moved; the reload watches whatever has the path now.
                    let gone = mask & (libc::IN_DELETE_SELF | libc::IN_MOVE_SELF | libc::IN_IGNORED) != 0;
                    for (pane, (_, watch)) in &self.paths {
                        if *watch == descriptor || mask & libc::IN_Q_OVERFLOW != 0 { changed.insert(*pane); }
                        if *watch == descriptor && gone { ended.push(*pane); }
                    }
                    offset += 16 + length;
                }
            }
        }
        #[cfg(windows)]
        for (&pane, (_, watch)) in &self.paths {
            if let Some(rearmed) = watch.changed() {
                changed.insert(pane);
                if !rearmed {
                    ended.push(pane);
                }
            }
        }
        for pane in ended { self.remove(pane); }
        changed.into_iter().collect()
    }
}

#[cfg(windows)]
struct ChangeNotification(windows_sys::Win32::Foundation::HANDLE);

// SAFETY: a change notification handle is a kernel object; any thread may wait on or close it.
#[cfg(windows)]
unsafe impl Send for ChangeNotification {}

#[cfg(windows)]
impl ChangeNotification {
    fn new(path: &std::path::Path) -> Option<Self> {
        use windows_sys::Win32::{Foundation::INVALID_HANDLE_VALUE, Storage::FileSystem::*};
        let path = crate::windows::wide(path);
        let mask = FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_DIR_NAME
            | FILE_NOTIFY_CHANGE_ATTRIBUTES | FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_SIZE;
        let handle = unsafe { FindFirstChangeNotificationW(path.as_ptr(), 0, mask) };
        (handle != INVALID_HANDLE_VALUE).then(|| Self(handle))
    }

    fn changed(&self) -> Option<bool> {
        use windows_sys::Win32::{
            Foundation::{WAIT_FAILED, WAIT_OBJECT_0},
            Storage::FileSystem::FindNextChangeNotification,
            System::Threading::WaitForSingleObject,
        };
        match unsafe { WaitForSingleObject(self.0, 0) } {
            WAIT_OBJECT_0 => Some(unsafe { FindNextChangeNotification(self.0) } != 0),
            WAIT_FAILED => Some(false),
            _ => None,
        }
    }
}

#[cfg(windows)]
impl Drop for ChangeNotification {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Storage::FileSystem::FindCloseChangeNotification(self.0) };
    }
}

impl Workspace {
    /// Watches the folder a pane's listing came from. On Windows the watch is made
    /// off the window thread, since opening one on a slow share waits on the network;
    /// it is kept only if the pane still shows that folder.
    pub(crate) fn watch_pane_folder(&mut self, target: PaneTarget, path: PathBuf, cx: &mut Context<Self>) {
        #[cfg(target_os = "linux")]
        {
            let _ = cx;
            self.directory_watches.watch(target.0, path);
        }
        #[cfg(windows)]
        {
            if self.directory_watches.watches(target.0, &path) {
                return;
            }
            // The folder it showed before is not this pane's any more.
            self.directory_watches.remove(target.0);
            cx.spawn(async move |this, cx| {
                let watch = cx.background_executor().spawn({
                    let path = path.clone();
                    async move { ChangeNotification::new(&path) }
                }).await;
                this.update(cx, |this, _| {
                    let shown = this.pane_by_id(target.0).is_some_and(|pane| pane.real_path.as_deref() == Some(path.as_path()));
                    if let (true, Some(watch)) = (shown, watch) {
                        this.directory_watches.install(target.0, path, watch);
                    }
                })
                .ok();
            })
            .detach();
        }
    }

    pub(crate) fn poll_directory_changes(&mut self, cx: &mut Context<Self>) {
        if self.operation_status.is_busy() { return; }
        let mut watched = self.all_pane_ids();
        watched.extend([super::trash::TRASH_WATCH, super::bookmarks::BOOKMARKS_WATCH]);
        for id in self.directory_watches.changed(&watched) {
            if id == super::trash::TRASH_WATCH {
                self.refresh_trash_state(cx);
                continue;
            }
            if id == super::bookmarks::BOOKMARKS_WATCH {
                self.load_bookmarks(cx);
                continue;
            }
            self.load_pane(PaneTarget(id), cx);
            // A finished search is brought up to date; one still walking is left to finish,
            // or a busy folder would restart it for ever.
            if self.pane_by_id(id).is_some_and(|pane| matches!(pane.search, crate::pane::Search::Done)) {
                self.schedule_search_for(id, std::time::Duration::ZERO, cx);
            }
        }
    }

    pub(crate) fn start_directory_watches(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
            if this.update(cx, |this, cx| this.poll_directory_changes(cx)).is_err() { break; }
        }).detach();
    }
}
