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

use crate::api::{DriveStats, RootKind};
use crate::local::{add_mount, canonical_directory, LocalFiles, Mount};

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

fn known_folder(id: &GUID) -> Option<PathBuf> {
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
    let mask = unsafe { GetLogicalDrives() };
    for (index, letter) in ('A'..='Z').enumerate() {
        if mask & (1 << index) == 0 {
            continue;
        }
        let root = format!("{letter}:\\");
        if let Ok(path) = canonical_directory(Path::new(&root)) {
            let label = volume(&path).and_then(|info| info.label).map_or_else(
                || format!("{letter}:"),
                |label| format!("{label} ({letter}:)"),
            );
            add_mount(
                &mut mounts,
                &letter.to_ascii_lowercase().to_string(),
                Some(&label),
                RootKind::Drive,
                path,
            );
        }
    }
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
    })
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

pub(super) fn with_shell<T>(run: impl FnOnce() -> Result<T>) -> Result<T> {
    let result = unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) };
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
