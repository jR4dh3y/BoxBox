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
