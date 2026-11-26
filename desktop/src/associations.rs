//! Tells when file associations change, so icons drawn from the apps that open
//! each type are made again: a `mimeapps.list` written (by BoxBox's Open With,
//! `gio mime`, `xdg-mime` or another file manager) or an app installed or
//! removed. One inotify descriptor on a thread of its own, which blocks on it.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt as _;
use std::path::PathBuf;
use std::time::Duration;

/// Writes come in bursts (a package installs dozens of launchers); one signal covers them.
const SETTLE: Duration = Duration::from_millis(300);

/// The folders whose files decide associations: the config folders hold
/// `mimeapps.list`, the applications folders the launchers and their own lists.
fn folders() -> Vec<(PathBuf, bool)> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    let config_dirs = std::env::var("XDG_CONFIG_DIRS").ok().filter(|dirs| !dirs.is_empty()).unwrap_or_else(|| "/etc/xdg".into());
    let configs = config_home.into_iter().chain(config_dirs.split(':').map(PathBuf::from).filter(|path| path.is_absolute()));
    configs.map(|dir| (dir, false)).chain(crate::open_with::application_dirs().into_iter().map(|dir| (dir, true))).collect()
}

/// Whether a changed file in a watched folder can change an association. A config
/// folder sees every app's settings, so only its lists count there.
fn matters(name: &[u8], applications: bool) -> bool {
    let name = String::from_utf8_lossy(name);
    name.ends_with("mimeapps.list") || (applications && (name.ends_with(".desktop") || name == "mimeinfo.cache"))
}

/// Sends once after each burst of changes, until the receiver is gone.
pub fn watch(changed: async_channel::Sender<()>) {
    let started = std::thread::Builder::new().name("associations".into()).spawn(move || {
        // Blocking: the thread waits in `read` until something changes.
        let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
        if fd < 0 {
            log::warn!("Could not watch file associations: {}", std::io::Error::last_os_error());
            return;
        }
        let mask = libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO | libc::IN_MOVED_FROM | libc::IN_CREATE | libc::IN_DELETE;
        let mut watched = std::collections::HashMap::new();
        for (dir, applications) in folders() {
            let Ok(name) = CString::new(dir.as_os_str().as_bytes()) else { continue };
            let watch = unsafe { libc::inotify_add_watch(fd, name.as_ptr(), mask) };
            if watch >= 0 {
                watched.insert(watch, applications);
            }
        }
        let mut buffer = [0u8; 4096];
        loop {
            let read = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
            let Ok(read) = usize::try_from(read) else { break };
            if !events(&buffer[..read]).any(|(watch, name)| watched.get(&watch).is_some_and(|applications| matters(name, *applications))) {
                continue;
            }
            std::thread::sleep(SETTLE);
            // A launcher edited in place leaves its folder's time, which the app list checks, alone.
            crate::open_with::forget_installed();
            if changed.send_blocking(()).is_err() {
                break;
            }
            // Drop what piled up while settling; the signal already covers it.
            drain(fd, &mut buffer);
        }
        unsafe { libc::close(fd) };
    });
    if let Err(error) = started {
        log::warn!("Could not watch file associations: {error}");
    }
}

/// `(watch, file name)` of each `inotify_event` in `bytes`.
fn events(bytes: &[u8]) -> impl Iterator<Item = (i32, &[u8])> {
    const HEADER: usize = std::mem::size_of::<libc::inotify_event>();
    let mut offset = 0;
    std::iter::from_fn(move || {
        let header = bytes.get(offset..offset + HEADER)?;
        let watch = i32::from_ne_bytes(header[0..4].try_into().ok()?);
        let len = u32::from_ne_bytes(header[12..16].try_into().ok()?) as usize;
        let name = bytes.get(offset + HEADER..offset + HEADER + len)?;
        offset += HEADER + len;
        Some((watch, name.split(|byte| *byte == 0).next().unwrap_or_default()))
    })
}

fn drain(fd: i32, buffer: &mut [u8]) {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    while unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) } > 0 {}
    unsafe { libc::fcntl(fd, libc::F_SETFL, flags) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_lists_and_launchers_count() {
        assert!(matters(b"mimeapps.list", false));
        assert!(matters(b"niri-mimeapps.list", false));
        assert!(!matters(b"settings.json", false), "a config folder sees every app's files");
        assert!(matters(b"org.xarchiver.Xarchiver.desktop", true));
        assert!(matters(b"mimeinfo.cache", true));
        assert!(!matters(b"org.app.desktop", false));
    }

    #[test]
    fn writing_a_default_is_noticed() {
        let dir = std::env::temp_dir().join(format!("boxbox-associations-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // The parser reads what the kernel writes, so drive a real descriptor.
        let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
        let name = CString::new(dir.as_os_str().as_bytes()).unwrap();
        let watch = unsafe { libc::inotify_add_watch(fd, name.as_ptr(), libc::IN_CLOSE_WRITE) };
        std::fs::write(dir.join("mimeapps.list"), "[Default Applications]\napplication/zip=xarchiver.desktop\n").unwrap();
        let mut buffer = [0u8; 4096];
        let read = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
        let seen: Vec<(i32, Vec<u8>)> = events(&buffer[..read as usize]).map(|(w, name)| (w, name.to_vec())).collect();
        unsafe { libc::close(fd) };
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(seen, [(watch, b"mimeapps.list".to_vec())]);
    }
}
