//! Windows filesystem and Shell integration.

use std::ffi::{OsStr, OsString};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use anyhow::{ensure, Context as _, Result};
use windows_sys::core::{w, GUID};
use windows_sys::Win32::{
    Storage::FileSystem::*,
    System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED},
    System::SystemServices::FILE_READ_ONLY_VOLUME,
    UI::Shell::*,
    UI::WindowsAndMessaging::SW_SHOWNORMAL,
};

pub(crate) mod context_menu;
pub(crate) mod icons;
pub(crate) mod live_resize;
pub(crate) mod mail;
pub(crate) mod recent;
pub(crate) mod recycle_bin;

use crate::api::{DriveStats, RootKind};
use crate::local::{add_mount, canonical_directory, LocalFiles, Mount};

pub(super) struct SystemLibrary(windows_sys::Win32::Foundation::HMODULE);

impl SystemLibrary {
    pub(super) fn open(name: windows_sys::core::PCWSTR) -> Result<Self> {
        use windows_sys::Win32::System::LibraryLoader::{
            LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
        };
        let module = unsafe { LoadLibraryExW(name, null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32) };
        if module.is_null() {
            return Err(std::io::Error::last_os_error())
                .context("a Windows system component is unavailable");
        }
        Ok(Self(module))
    }

    pub(super) fn symbol(
        &self,
        name: &std::ffi::CStr,
    ) -> Result<unsafe extern "system" fn() -> isize> {
        unsafe {
            windows_sys::Win32::System::LibraryLoader::GetProcAddress(self.0, name.as_ptr().cast())
        }
        .with_context(|| {
            format!(
                "Windows system component does not export {}",
                name.to_string_lossy()
            )
        })
    }
}

impl Drop for SystemLibrary {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::FreeLibrary(self.0) };
    }
}

pub(super) fn window_id(window: &gpui::Window) -> Option<isize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
        _ => None,
    }
}

pub(super) fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref().encode_wide().chain([0]).collect()
}

pub(super) fn shell_path(path: &Path) -> Vec<u16> {
    let native = wide(path);
    let slash = u16::from(b'\\');
    let extended_prefix = [slash, slash, u16::from(b'?'), slash];
    let [first, second, question, third] = extended_prefix;
    let unc_prefix = [
        first,
        second,
        question,
        third,
        b'U'.into(),
        b'N'.into(),
        b'C'.into(),
        slash,
    ];
    if let Some(tail) = native.strip_prefix(&unc_prefix) {
        [slash, slash]
            .into_iter()
            .chain(tail.iter().copied())
            .collect()
    } else if let Some(tail) = native.strip_prefix(&extended_prefix) {
        tail.to_vec()
    } else {
        native
    }
}

pub(super) fn shell_path_buf(path: &Path) -> PathBuf {
    let mut text = shell_path(path);
    text.pop();
    PathBuf::from(OsString::from_wide(&text))
}

pub(super) fn known_folder(id: &GUID) -> Option<PathBuf> {
    let mut path = null_mut();
    // The shell allocates a NUL-terminated string with the COM allocator.
    let result = unsafe { SHGetKnownFolderPath(id, 0, null_mut(), &mut path) };
    let folder = if result >= 0 && !path.is_null() {
        let len = (0..).find(|&i| unsafe { *path.add(i) } == 0)?;
        Some(PathBuf::from(OsString::from_wide(unsafe {
            std::slice::from_raw_parts(path, len)
        })))
    } else {
        None
    };
    unsafe { CoTaskMemFree(path.cast()) };
    folder
}

pub(super) fn local_files() -> Result<LocalFiles> {
    let home = known_folder(&FOLDERID_Profile).context("cannot locate the Windows profile")?;
    let home = canonical_directory(&home).context("cannot open the Windows profile")?;
    let mut mounts = Vec::new();
    mounts.extend(drive_mounts());
    add_mount(&mut mounts, "home", Some("Home"), RootKind::Place, home);
    for (name, label, id) in [
        ("desktop", "Desktop", FOLDERID_Desktop),
        ("documents", "Documents", FOLDERID_Documents),
        ("downloads", "Downloads", FOLDERID_Downloads),
        ("music", "Music", FOLDERID_Music),
        ("pictures", "Pictures", FOLDERID_Pictures),
        ("videos", "Videos", FOLDERID_Videos),
    ] {
        if let Some(path) = known_folder(&id).and_then(|p| canonical_directory(&p).ok()) {
            add_mount(&mut mounts, name, Some(label), RootKind::Place, path);
        }
    }
    Ok(LocalFiles {
        mounts,
        trash_data_home: crate::persist::state_dir(),
        volume_trash: false,
        host_filesystem: true,
    })
}

pub(super) fn drive_mask() -> u32 {
    unsafe { GetLogicalDrives() }
}

pub(super) fn has_drive(name: &str, mask: u32) -> bool {
    matches!(name.as_bytes(), [letter] if letter.is_ascii_lowercase() && mask & (1 << (letter - b'a')) != 0)
}

pub(super) fn drive_mounts() -> Vec<Mount> {
    let mask = drive_mask();
    ('a'..='z')
        .filter_map(|letter| {
            let name = letter.to_string();
            has_drive(&name, mask)
                .then(|| additional_mount(&name))
                .flatten()
        })
        .collect()
}

pub(super) fn additional_mount(name: &str) -> Option<Mount> {
    use crate::api::MountPoint;
    use std::path::{Component, Prefix};

    let root = if has_drive(name, drive_mask()) {
        PathBuf::from(format!("{}:\\", name.to_ascii_uppercase()))
    } else {
        let path = Path::new(name);
        let mut parts = path.components();
        let Component::Prefix(prefix) = parts.next()? else {
            return None;
        };
        if !matches!(prefix.kind(), Prefix::UNC(..))
            || parts.any(|part| !matches!(part, Component::RootDir))
        {
            return None;
        }
        return Some(Mount {
            point: MountPoint {
                name: name.trim_end_matches('\\').to_owned(),
                label: None,
                kind: RootKind::Drive,
            },
            path: canonical_directory(path).ok()?,
        });
    };
    let path = canonical_directory(&root).ok()?;
    let letter = name.to_ascii_uppercase();
    let label = volume(&path).and_then(|info| info.label).map_or_else(
        || format!("{letter}:"),
        |label| format!("{label} ({letter}:)"),
    );
    Some(Mount {
        point: MountPoint {
            name: name.to_owned(),
            label: Some(label),
            kind: RootKind::Drive,
        },
        path,
    })
}

pub(super) fn mount_name(real: &Path) -> Option<String> {
    use std::path::{Component, Prefix};
    let Component::Prefix(prefix) = real.components().next()? else {
        return None;
    };
    match prefix.kind() {
        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
            Some(char::from(letter).to_ascii_lowercase().to_string())
        }
        Prefix::UNC(host, share) | Prefix::VerbatimUNC(host, share) => {
            Some(format!(r"\\{}\{}", host.to_str()?, share.to_str()?))
        }
        _ => None,
    }
}

pub(super) fn is_network_drive(path: &Path) -> bool {
    use windows_sys::Win32::System::WindowsProgramming::DRIVE_REMOTE;
    let path = shell_path(path);
    (unsafe { GetDriveTypeW(path.as_ptr()) }) == DRIVE_REMOTE
}

pub(super) fn validate_name(name: &str) -> Result<()> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let reserved_port = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        });
    ensure!(
        !name.ends_with(['.', ' '])
            && !name
                .chars()
                .any(|ch| ch < ' ' || r#"\":<>|?*"#.contains(ch))
            && !matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            )
            && !reserved_port,
        "invalid Windows file name {name:?}"
    );
    Ok(())
}

pub(crate) fn writable(folder: &Path) -> bool {
    std::fs::OpenOptions::new()
        .access_mode(FILE_ADD_FILE | FILE_ADD_SUBDIRECTORY)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(folder)
        .is_ok()
}

pub(super) fn rename_noreplace(source: &Path, destination: &Path) -> Result<()> {
    let from = wide(source);
    let to = wide(destination);
    // Omitting MOVEFILE_REPLACE_EXISTING keeps the destination if it exists.
    ensure_win32(unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) }).with_context(|| {
        format!(
            "cannot move {} to {}",
            source.display(),
            destination.display()
        )
    })
}

pub(super) fn ensure_win32(result: i32) -> io::Result<()> {
    if result != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(super) fn file_id(path: &Path) -> Option<(u32, u64)> {
    let file = std::fs::OpenOptions::new()
        .access_mode(0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .ok()?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    ensure_win32(unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) }).ok()?;
    let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    Some((info.dwVolumeSerialNumber, index))
}

pub(super) fn same_device(source: &Path, destination: &Path) -> bool {
    file_id(source)
        .is_some_and(|(volume, _)| file_id(destination).is_some_and(|(other, _)| volume == other))
}

fn disk_space(path: &Path) -> io::Result<(u64, u64)> {
    let path = wide(path);
    let (mut free, mut total) = (0, 0);
    ensure_win32(unsafe { GetDiskFreeSpaceExW(path.as_ptr(), &mut free, &mut total, null_mut()) })?;
    Ok((total, free.min(total)))
}

pub(super) fn free_space(path: &Path) -> Option<u64> {
    disk_space(path).ok().map(|(_, free)| free)
}

struct Volume {
    label: Option<String>,
    filesystem: Option<String>,
    read_only: bool,
}

fn volume(path: &Path) -> Option<Volume> {
    let mut root = vec![0u16; 32768];
    let path = wide(path);
    ensure_win32(unsafe {
        GetVolumePathNameW(path.as_ptr(), root.as_mut_ptr(), root.len() as u32)
    })
    .ok()?;
    let (mut label, mut filesystem, mut flags) = ([0u16; 256], [0u16; 32], 0);
    ensure_win32(unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            null_mut(),
            null_mut(),
            &mut flags,
            filesystem.as_mut_ptr(),
            filesystem.len() as u32,
        )
    })
    .ok()?;
    let text = |buffer: &[u16]| {
        let end = buffer
            .iter()
            .position(|&ch| ch == 0)
            .unwrap_or(buffer.len());
        (end > 0).then(|| String::from_utf16_lossy(&buffer[..end]))
    };
    Some(Volume {
        label: text(&label),
        filesystem: text(&filesystem),
        read_only: flags & FILE_READ_ONLY_VOLUME != 0,
    })
}

pub(super) fn is_fat(path: &Path) -> bool {
    volume(path)
        .and_then(|info| info.filesystem)
        .is_some_and(|fs| matches!(fs.to_ascii_uppercase().as_str(), "FAT" | "FAT32" | "EXFAT"))
}

pub(super) fn drive_stats(mount: &Mount) -> Result<DriveStats> {
    let (total_bytes, free_bytes) = disk_space(&mount.path)?;
    let used_bytes = total_bytes - free_bytes;
    let details = volume(&mount.path);
    Ok(DriveStats {
        name: mount.point.name.clone(),
        label: mount.point.label.clone(),
        device: Some(
            String::from_utf16_lossy(&shell_path(&mount.path))
                .trim_end_matches('\0')
                .to_owned(),
        ),
        fs_type: details.as_ref().and_then(|info| info.filesystem.clone()),
        total_bytes,
        free_bytes,
        used_bytes,
        used_pct: if total_bytes == 0 {
            0.0
        } else {
            used_bytes as f64 * 100.0 / total_bytes as f64
        },
        read_only: details.is_some_and(|info| info.read_only),
    })
}

pub(super) fn shell_item(path: &Path) -> Result<windows::Win32::UI::Shell::IShellItem> {
    use windows::{core::PCWSTR, Win32::UI::Shell::SHCreateItemFromParsingName};

    let wide = shell_path(path);
    Ok(unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)? })
}

/// Take ownership of a string returned by the Shell's COM allocator.
unsafe fn shell_string(text: windows::core::PWSTR) -> Result<String> {
    let value = unsafe { text.to_string() };
    unsafe { CoTaskMemFree(text.0.cast()) };
    Ok(value?)
}

pub(super) fn with_shell<T: Send>(run: impl FnOnce() -> Result<T> + Send) -> Result<T> {
    // GPUI's background workers are initialized as MTA. Shell dialogs and file
    // operations need an STA and cannot change the caller's existing apartment.
    let result = unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) };
    if result == windows_sys::Win32::Foundation::RPC_E_CHANGED_MODE {
        return std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let result = unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) };
                    initialized_shell(result, run)
                })
                .join()
                .map_err(|_| anyhow::anyhow!("the Windows Shell thread panicked"))?
        });
    }
    initialized_shell(result, run)
}

fn initialized_shell<T>(result: i32, run: impl FnOnce() -> Result<T>) -> Result<T> {
    if result < 0 {
        return Err(io::Error::from_raw_os_error(result))
            .context("cannot initialize the Windows Shell");
    }
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }
    let _com = Com;
    run()
}

pub(super) fn shell_verb(verb: &[u16], path: &Path, owner: isize) -> Result<()> {
    let file = shell_path(path);
    with_shell(|| {
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_INVOKEIDLIST | SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC,
            hwnd: owner as _,
            lpVerb: verb.as_ptr(),
            lpFile: file.as_ptr(),
            nShow: SW_SHOWNORMAL,
            ..Default::default()
        };
        ensure_win32(unsafe { ShellExecuteExW(&mut info) })?;
        Ok(())
    })
}

pub(super) fn show_properties(path: &Path, owner: isize) -> Result<()> {
    shell_verb(&wide("properties"), path, owner)
}

pub(super) fn add_recent(path: &Path) {
    let target = shell_path(path);
    unsafe { SHAddToRecentDocs(SHARD_PATHW as u32, target.as_ptr().cast()) };
}

pub(super) fn open(path: &Path) -> Result<()> {
    let target = shell_path(path);
    with_shell(|| {
        let result = unsafe {
            ShellExecuteW(
                null_mut(),
                w!("open"),
                target.as_ptr(),
                null(),
                null(),
                SW_SHOWNORMAL,
            )
        } as isize;
        ensure!(
            result > 32,
            "Windows could not open {} (Shell error {result})",
            path.display()
        );
        Ok(())
    })
}

#[cfg(test)]
mod tests;
