use std::{collections::HashMap, path::PathBuf};

use gpui::Context;

use super::{PaneTarget, Workspace};

#[derive(Default)]
pub(crate) struct DirectoryWatches {
    #[cfg(target_os = "linux")]
    events: Option<std::fs::File>,
    paths: HashMap<usize, (PathBuf, i32)>,
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
        #[cfg(not(target_os = "linux"))]
        let _ = (pane, path);
    }

    fn remove(&mut self, pane: usize) {
        if let Some((_, descriptor)) = self.paths.remove(&pane) {
            #[cfg(target_os = "linux")]
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
                    for (pane, (_, watch)) in &self.paths {
                        if *watch == descriptor || mask & libc::IN_Q_OVERFLOW != 0 { changed.insert(*pane); }
                    }
                    offset += 16 + length;
                }
            }
        }
        changed.into_iter().collect()
    }
}

impl Workspace {
    pub(crate) fn poll_directory_changes(&mut self, cx: &mut Context<Self>) {
        if self.operation_status.is_busy() { return; }
        for id in self.directory_watches.changed(&self.all_pane_ids()) {
            self.load_pane(PaneTarget(id), cx);
            self.schedule_search_for(id, std::time::Duration::ZERO, cx);
        }
    }

    pub(crate) fn start_directory_watches(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
            if this.update(cx, |this, cx| this.poll_directory_changes(cx)).is_err() { break; }
        }).detach();
    }
}
