//! Tells when file associations change, so file type icons from the Shell are
//! made again: a new default in Open With or Settings, or an app installed or
//! removed. The registry holds them, so one thread waits on its change events.

use std::ptr::null;
use std::time::Duration;

use windows_sys::core::{w, PCWSTR};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegNotifyChangeKeyValue, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_NOTIFY,
    REG_NOTIFY_CHANGE_LAST_SET, REG_NOTIFY_CHANGE_NAME,
};
use windows_sys::Win32::System::Threading::{CreateEventW, WaitForMultipleObjects, WaitForSingleObject, INFINITE};

/// Installers write many keys at once; one signal covers the burst.
const SETTLE: Duration = Duration::from_millis(300);

/// A user's choice per extension, then the classes it and installed apps name.
fn keys() -> [(HKEY, PCWSTR); 3] {
    [
        (HKEY_CURRENT_USER, w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts")),
        (HKEY_CURRENT_USER, w!(r"Software\Classes")),
        (HKEY_LOCAL_MACHINE, w!(r"Software\Classes")),
    ]
}

struct Watch {
    key: HKEY,
    event: HANDLE,
}

impl Watch {
    fn open(root: HKEY, path: PCWSTR) -> Option<Self> {
        let mut key = std::ptr::null_mut();
        if unsafe { RegOpenKeyExW(root, path, 0, KEY_NOTIFY, &mut key) } != ERROR_SUCCESS {
            return None;
        }
        // Auto-reset, so a wait consumes the signal it returns.
        let event = unsafe { CreateEventW(null(), 0, 0, null()) };
        if event.is_null() {
            unsafe { RegCloseKey(key) };
            return None;
        }
        let watch = Self { key, event };
        watch.arm().then_some(watch)
    }

    /// One notification per call; it is tied to this thread, which never exits while watching.
    fn arm(&self) -> bool {
        let filter = REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET;
        unsafe { RegNotifyChangeKeyValue(self.key, 1, filter, self.event, 1) == ERROR_SUCCESS }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.key);
            CloseHandle(self.event);
        }
    }
}

/// Sends once after each burst of changes, until the receiver is gone.
pub(crate) fn watch(changed: async_channel::Sender<()>) {
    let started = std::thread::Builder::new().name("associations".into()).spawn(move || {
        let watches: Vec<Watch> = keys().into_iter().filter_map(|(root, path)| Watch::open(root, path)).collect();
        if watches.is_empty() {
            log::warn!("Could not watch file associations: {}", std::io::Error::last_os_error());
            return;
        }
        let events: Vec<HANDLE> = watches.iter().map(|watch| watch.event).collect();
        loop {
            let woke = unsafe { WaitForMultipleObjects(events.len() as u32, events.as_ptr(), 0, INFINITE) };
            let Some(first) = woke.checked_sub(WAIT_OBJECT_0).map(|index| index as usize).filter(|index| *index < watches.len()) else {
                log::warn!("Stopped watching file associations: {}", std::io::Error::last_os_error());
                break;
            };
            std::thread::sleep(SETTLE);
            // Re-arm each key that fired, the one that woke us included.
            for (index, watch) in watches.iter().enumerate() {
                let fired = index == first || unsafe { WaitForSingleObject(watch.event, 0) } == WAIT_OBJECT_0;
                if fired && !watch.arm() {
                    log::warn!("Stopped watching a file association key: {}", std::io::Error::last_os_error());
                }
            }
            if changed.send_blocking(()).is_err() {
                break;
            }
        }
    });
    if let Err(error) = started {
        log::warn!("Could not watch file associations: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Registry::{RegCreateKeyExW, RegDeleteTreeW, KEY_WRITE, REG_OPTION_VOLATILE};

    #[test]
    fn a_new_class_is_noticed() {
        let (changed, changes) = async_channel::unbounded();
        watch(changed);
        // Let the thread arm its keys before writing.
        std::thread::sleep(Duration::from_millis(200));
        let name = super::super::wide(format!(r"Software\Classes\BoxBox.AssociationTest.{}", std::process::id()));
        let mut key = std::ptr::null_mut();
        let created = unsafe {
            RegCreateKeyExW(HKEY_CURRENT_USER, name.as_ptr(), 0, null(), REG_OPTION_VOLATILE, KEY_WRITE, null(), &mut key, std::ptr::null_mut())
        };
        assert_eq!(created, ERROR_SUCCESS);
        unsafe { RegCloseKey(key) };
        let (seen_tx, seen_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || seen_tx.send(changes.recv_blocking().is_ok()));
        let seen = seen_rx.recv_timeout(Duration::from_secs(5)).unwrap_or(false);
        unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, name.as_ptr()) };
        assert!(seen, "creating a class under HKCU\\Software\\Classes should signal");
    }
}
