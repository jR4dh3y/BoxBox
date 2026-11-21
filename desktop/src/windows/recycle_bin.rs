//! Shared Windows Recycle Bin. A rooted fixture keeps its own Trash.

use std::{collections::HashSet, fs, path::Path, sync::Mutex};

use anyhow::{ensure, Context as _, Result};
use windows::{
    core::{Interface, PCWSTR},
    Win32::{
        Foundation::PROPERTYKEY,
        System::{
            Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
            SystemServices::SFGAO_FOLDER,
        },
        UI::Shell::*,
    },
};

use crate::api::TrashItem;

const ORIGINAL_FOLDER: PROPERTYKEY = PROPERTYKEY {
    fmtid: PSGUID_DISPLACED,
    pid: PID_DISPLACED_FROM,
};
const DELETED_AT: PROPERTYKEY = PROPERTYKEY {
    fmtid: PSGUID_DISPLACED,
    pid: PID_DISPLACED_DATE,
};
const QUIET: FILEOPERATION_FLAGS =
    FILEOPERATION_FLAGS(FOF_NOERRORUI.0 | FOF_SILENT.0 | FOFX_EARLYFAILURE.0);

// A move and its before/after snapshots must be one operation within BoxBox.
static RECYCLE: Mutex<()> = Mutex::new(());

fn record(item: &IShellItem) -> Result<TrashItem> {
    let (id, name, folder, time, is_dir) = unsafe {
        let properties: IShellItem2 = item.cast()?;
        (
            super::shell_string(item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING)?)?,
            super::shell_string(item.GetDisplayName(SIGDN_PARENTRELATIVE)?)?,
            super::shell_string(properties.GetString(&ORIGINAL_FOLDER)?)?,
            properties.GetFileTime(&DELETED_AT)?,
            item.GetAttributes(SFGAO_FOLDER)? == SFGAO_FOLDER,
        )
    };
    let name = original_name(Path::new(&id)).unwrap_or_else(|| full_name(name, &id));
    let original = Path::new(&folder).join(&name);
    ensure!(
        original.is_absolute() && original.parent() == Some(Path::new(&folder)),
        "invalid Recycle Bin location"
    );
    let ticks = (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    let seconds = i64::try_from(ticks / 10_000_000)? - 11_644_473_600;
    let deleted_at = chrono::DateTime::from_timestamp(seconds, 0)
        .map(|date| {
            date.with_timezone(&chrono::Local)
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string()
        })
        .unwrap_or_default();
    Ok(TrashItem {
        id: format!("win:{id}"),
        name,
        original_path: original.to_string_lossy().into_owned(),
        deleted_at,
        is_dir,
        can_restore: can_restore(&original),
    })
}

/// The original name, from the bin's `$I` record beside the `$R` copy. The
/// Shell's label can hide an extension, so `report.txt.txt` shows as `report.txt`.
fn original_name(copy: &Path) -> Option<String> {
    let name = copy.file_name()?.to_str()?.strip_prefix("$R")?;
    let path = parse_index(&fs::read(copy.with_file_name(format!("$I{name}"))).ok()?)?;
    Path::new(&path).file_name()?.to_str().map(str::to_owned)
}

/// A `$I` record: version, size and deletion time (8 bytes each), then the
/// original path in UTF-16. Version 1 pads the path to 260 units; version 2
/// gives its length, with the trailing NUL, in 4 bytes.
fn parse_index(bytes: &[u8]) -> Option<String> {
    let version = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
    let path = match version {
        1 => bytes.get(24..24 + 520)?,
        2 => {
            let units = u32::from_le_bytes(bytes.get(24..28)?.try_into().ok()?) as usize;
            bytes.get(28..28 + units.checked_mul(2)?)?
        }
        _ => return None,
    };
    let units: Vec<u16> = path.chunks_exact(2).map(|unit| u16::from_le_bytes([unit[0], unit[1]])).take_while(|unit| *unit != 0).collect();
    String::from_utf16(&units).ok().filter(|path| !path.is_empty())
}

/// The name when the `$I` record cannot be read. The Shell's label drops a known
/// extension when Explorer hides them (`report` for `report.txt`), but the
/// bin's own copy (`$R1A2B3C.txt`, the parsing path) keeps it.
fn full_name(label: String, parsing: &str) -> String {
    let Some(extension) = Path::new(parsing).extension().and_then(|value| value.to_str()) else {
        return label;
    };
    let suffix = format!(".{extension}");
    let has = label.len() >= suffix.len()
        && label.is_char_boundary(label.len() - suffix.len())
        && label[label.len() - suffix.len()..].eq_ignore_ascii_case(&suffix);
    if has { label } else { label + &suffix }
}

fn can_restore(original: &Path) -> bool {
    original.parent().is_some_and(Path::is_dir)
        && fs::symlink_metadata(original)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
}

fn records() -> Result<Vec<(IShellItem, TrashItem)>> {
    let bin: IShellItem =
        unsafe { SHGetKnownFolderItem(&FOLDERID_RecycleBinFolder, KF_FLAG_DEFAULT, None)? };
    let items: IEnumShellItems = unsafe { bin.BindToHandler(None, &BHID_EnumItems)? };
    let mut records = Vec::new();
    loop {
        let mut entry = [None];
        unsafe { items.Next(&mut entry, None)? };
        let [Some(item)] = entry else { break };
        match record(&item) {
            Ok(row) => records.push((item, row)),
            Err(error) => log::debug!("Could not inspect Recycle Bin item: {error:#}"),
        }
    }
    records.sort_by(|(_, left), (_, right)| right.deleted_at.cmp(&left.deleted_at));
    Ok(records)
}

pub(crate) fn list() -> Result<Vec<TrashItem>> {
    super::with_shell(|| Ok(records()?.into_iter().map(|(_, row)| row).collect()))
}

pub(crate) fn has_items() -> bool {
    super::with_shell(|| {
        let mut info = SHQUERYRBINFO {
            cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
            ..Default::default()
        };
        unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info)? };
        Ok(info.i64NumItems > 0)
    })
    .unwrap_or(false)
}

fn operation(flags: FILEOPERATION_FLAGS) -> Result<IFileOperation> {
    let op: IFileOperation =
        unsafe { CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)? };
    unsafe { op.SetOperationFlags(flags)? };
    Ok(op)
}

fn perform(op: &IFileOperation) -> Result<()> {
    unsafe {
        op.PerformOperations()?;
        ensure!(
            !op.GetAnyOperationsAborted()?.as_bool(),
            "Windows cancelled the file operation"
        );
    }
    Ok(())
}

pub(crate) fn recycle(path: &Path) -> Result<TrashItem> {
    let _lock = RECYCLE.lock().unwrap_or_else(|error| error.into_inner());
    let original = super::shell_path_buf(path);
    super::with_shell(|| {
        let before: HashSet<_> = records()?
            .into_iter()
            .filter_map(|(_, row)| (Path::new(&row.original_path) == original).then_some(row.id))
            .collect();
        let op = operation(QUIET | FOFX_RECYCLEONDELETE | FOF_WANTNUKEWARNING)?;
        let item = super::shell_item(path)?;
        unsafe { op.DeleteItem(&item, None)? };
        perform(&op)?;
        records()?
            .into_iter()
            .find_map(|(_, row)| {
                (Path::new(&row.original_path) == original && !before.contains(&row.id))
                    .then_some(row)
            })
            .context("the Recycle Bin did not report the moved item")
    })
}

pub(crate) fn restore(id: &str) -> Result<()> {
    super::with_shell(|| {
        let (item, row) = records()?
            .into_iter()
            .find(|(_, row)| row.id == id)
            .context("Recycle Bin item was removed")?;
        let original = Path::new(&row.original_path);
        ensure!(
            can_restore(original),
            "the original folder is gone or already contains this name"
        );
        let parent = super::shell_item(original.parent().context("invalid original folder")?)?;
        let name = super::wide(&row.name);
        let op = operation(QUIET)?;
        // Keep Windows' collision confirmation if a file arrived since the check.
        unsafe { op.MoveItem(&item, &parent, PCWSTR(name.as_ptr()), None)? };
        perform(&op)
    })
}

/// A single row, or all rows from the Recycle Bin. Only shell-enumerated items are submitted.
pub(crate) fn purge(id: Option<&str>) -> Result<usize> {
    super::with_shell(|| {
        let items: Vec<_> = records()?
            .into_iter()
            .filter(|(_, row)| id.is_none_or(|id| row.id == id))
            .collect();
        if items.is_empty() {
            ensure!(id.is_none(), "Recycle Bin item was removed");
            return Ok(0);
        }
        let op = operation(QUIET | FOF_NOCONFIRMATION)?;
        for (item, _) in &items {
            unsafe { op.DeleteItem(item, None)? };
        }
        perform(&op)?;
        Ok(items.len())
    })
}

/// Sends once after each burst of changes to any drive's Recycle Bin, made by
/// BoxBox, Explorer or another app, until the receiver is gone. One thread
/// waits on a change notification per drive; a drive added later is not watched.
pub(crate) fn watch(changed: async_channel::Sender<()>) {
    use std::time::Duration;
    use windows_sys::Win32::{
        Foundation::{HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0},
        Storage::FileSystem::{
            FindCloseChangeNotification, FindFirstChangeNotificationW, FindNextChangeNotification,
            FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME,
        },
        System::Threading::{WaitForMultipleObjects, WaitForSingleObject},
    };
    /// Deleting a folder writes many entries; one signal covers them.
    const SETTLE: Duration = Duration::from_millis(300);
    /// How often the thread checks whether anyone still listens.
    const CHECK_MS: u32 = 2_000;

    let started = std::thread::Builder::new().name("recycle bin".into()).spawn(move || {
        let mask = super::drive_mask();
        // Each user's bin is a subfolder named by their SID, so the whole tree is watched.
        let mut handles: Vec<HANDLE> = ('a'..='z')
            .filter(|letter| super::has_drive(&letter.to_string(), mask))
            .map(|letter| format!("{letter}:\\$Recycle.Bin"))
            .filter(|bin| Path::new(bin).is_dir())
            .map(|bin| unsafe {
                FindFirstChangeNotificationW(super::wide(&bin).as_ptr(), 1, FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_DIR_NAME)
            })
            .filter(|handle| *handle != INVALID_HANDLE_VALUE)
            .collect();
        while !handles.is_empty() && !changed.is_closed() {
            let count = u32::try_from(handles.len()).unwrap_or(u32::MAX);
            let index = unsafe { WaitForMultipleObjects(count, handles.as_ptr(), 0, CHECK_MS) }.wrapping_sub(WAIT_OBJECT_0);
            if index as usize >= handles.len() {
                continue;
            }
            std::thread::sleep(SETTLE);
            // Re-arm every bin signalled meanwhile too, so the burst is one signal.
            // A bin that cannot be re-armed (its drive left) is dropped, or it would signal for ever.
            handles.retain(|&handle| {
                if unsafe { WaitForSingleObject(handle, 0) } != WAIT_OBJECT_0 || unsafe { FindNextChangeNotification(handle) } != 0 {
                    return true;
                }
                unsafe { FindCloseChangeNotification(handle) };
                false
            });
            if changed.send_blocking(()).is_err() {
                break;
            }
        }
        for handle in handles {
            unsafe { FindCloseChangeNotification(handle) };
        }
    });
    if let Err(error) = started {
        log::warn!("Could not watch the Recycle Bin: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::{full_name, parse_index};

    fn index(version: u64, path: &str) -> Vec<u8> {
        let units: Vec<u16> = path.encode_utf16().chain([0]).collect();
        let mut bytes = [version, 7, 9].iter().flat_map(|value| value.to_le_bytes()).collect::<Vec<u8>>();
        match version {
            1 => {
                let mut padded = units.clone();
                padded.resize(260, 0);
                bytes.extend(padded.iter().flat_map(|unit| unit.to_le_bytes()));
            }
            _ => {
                bytes.extend((units.len() as u32).to_le_bytes());
                bytes.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
            }
        }
        bytes
    }

    #[test]
    fn the_index_record_gives_the_whole_original_path() {
        let path = r"C:\Users\me\report.txt.txt";
        assert_eq!(parse_index(&index(2, path)).as_deref(), Some(path));
        assert_eq!(parse_index(&index(1, path)).as_deref(), Some(path));
        assert_eq!(parse_index(&index(3, path)), None);
        assert_eq!(parse_index(&index(2, path)[..30]), None);
    }

    #[test]
    fn hidden_extensions_come_back_from_the_bins_copy() {
        let bin = r"C:\$Recycle.Bin\S-1-5-21\$R1A2B3C";
        assert_eq!(full_name("report".into(), &format!("{bin}.txt")), "report.txt");
        assert_eq!(full_name("a.b".into(), &format!("{bin}.txt")), "a.b.txt");
        assert_eq!(full_name("report.TXT".into(), &format!("{bin}.TXT")), "report.TXT");
        assert_eq!(full_name("my.folder".into(), &format!("{bin}.folder")), "my.folder");
        assert_eq!(full_name("Makefile".into(), bin), "Makefile");
    }
}
