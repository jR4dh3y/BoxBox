use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, ensure, Context as _, Result};
use chrono::Utc;

use crate::api::TrashItem;

use super::{copy, Mount};

const MAX_INFO_BYTES: usize = 64 * 1024;

#[derive(Clone)]
struct TrashRoot {
    path: PathBuf,
    relative_base: Option<PathBuf>,
}

struct Record {
    trash_root: TrashRoot,
    stored_name: PathBuf,
    data_path: PathBuf,
    info_path: PathBuf,
    original_path: PathBuf,
    deleted_at: String,
    is_dir: bool,
}

pub(super) fn list(
    mounts: &[Mount],
    data_home: &Path,
    include_volume_trash: bool,
) -> Result<Vec<TrashItem>> {
    let mut items = Vec::new();
    for root in discover_roots(data_home, include_volume_trash)? {
        let files = root.path.join("files");
        let info = root.path.join("info");
        for entry in fs::read_dir(&info).with_context(|| format!("cannot read {}", info.display()))? {
            let entry = entry?;
            let info_path = entry.path();
            let metadata = fs::symlink_metadata(&info_path)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                continue;
            }
            let Some(info_name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(stored_name) = info_name.strip_suffix(".trashinfo") else {
                continue;
            };
            validate_leaf(Path::new(stored_name))?;
            let data_name = PathBuf::from(stored_name);
            let data_path = files.join(&data_name);
            let data_metadata = match fs::symlink_metadata(&data_path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            let (original_path, deleted_at) = parse_info(&info_path, root.relative_base.as_deref())?;
            let name = original_path
                .file_name()
                .and_then(|name| name.to_str())
                .context("Trash original name is not valid UTF-8")?
                .to_owned();
            items.push(TrashItem {
                id: item_id(&root.path, &data_name),
                name,
                original_path: original_path.to_string_lossy().into_owned(),
                deleted_at,
                is_dir: data_metadata.is_dir(),
                can_restore: can_restore(mounts, &original_path),
            });
        }
    }
    items.sort_by(|left, right| left.deleted_at.cmp(&right.deleted_at).then(left.name.cmp(&right.name)));
    Ok(items)
}

pub(super) fn move_to_trash(
    mounts: &[Mount],
    data_home: &Path,
    source: &Path,
    virtual_path: &str,
    include_volume_trash: bool,
) -> Result<TrashItem> {
    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("cannot inspect {virtual_path}"))?;
    let source_parent = source.parent().context("trash source has no parent")?;
    let source_parent = fs::canonicalize(source_parent)?;
    let source = source_parent.join(source.file_name().context("trash source has no name")?);
    let original_path = source.clone();
    ensure!(original_path.is_absolute(), "trash source must be absolute");
    reject_line_breaks(&original_path)?;
    ensure!(
        mounts.iter().any(|mount| source.starts_with(&mount.path)),
        "trash source is outside all local mounts"
    );

    let global_root = TrashRoot {
        path: data_home.join("Trash"),
        relative_base: None,
    };
    ensure!(
        !metadata.is_dir() || !global_root.path.starts_with(&source),
        "cannot move a directory containing its Trash location"
    );
    match store_entry(&global_root, &source, &original_path) {
        Ok(record) => return Ok(item_for_record(mounts, record)?),
        Err(error) if include_volume_trash && is_cross_device(&error) => {}
        Err(error) => return Err(error),
    }

    let mountpoint = mountpoint_for(&source_parent).unwrap_or_else(|| filesystem_root(&source_parent));
    let local_root = local_trash_root(&mountpoint)?;
    ensure!(
        !metadata.is_dir() || !local_root.path.starts_with(&source),
        "cannot move a directory containing its Trash location"
    );
    let relative_original = source
        .strip_prefix(&mountpoint)
        .context("trash source is outside its filesystem mount")?;
    ensure!(
        relative_original.components().all(|component| matches!(component, Component::Normal(_))),
        "malformed path for Trash"
    );
    let record = store_entry(&local_root, &source, relative_original)
        .with_context(|| format!("cannot move {virtual_path} to the filesystem Trash"))?;
    item_for_record(mounts, record)
}

pub(super) fn restore(
    mounts: &[Mount],
    data_home: &Path,
    include_volume_trash: bool,
    id: &str,
) -> Result<()> {
    let record = find_record(data_home, include_volume_trash, id)?;
    let destination = validate_original(mounts, &record.original_path)?;
    match copy::rename_noreplace(&record.data_path, &destination) {
        Ok(()) => {}
        Err(error) if is_cross_device(&error) => {
            copy::copy_no_overwrite(&record.data_path, &destination, None)?;
            copy::remove_entry(&record.data_path)?;
        }
        Err(error) => return Err(error).context("cannot restore Trash item without overwriting an entry"),
    }
    fs::remove_file(&record.info_path).with_context(|| {
        format!("restored item but could not remove {}", record.info_path.display())
    })
}

pub(super) fn delete(
    mounts: &[Mount],
    data_home: &Path,
    include_volume_trash: bool,
    id: &str,
) -> Result<()> {
    let record = find_record(data_home, include_volume_trash, id)?;
    ensure!(
        record.original_path.is_absolute()
            && mounts
                .iter()
                .any(|mount| record.original_path.starts_with(&mount.path)),
        "Trash item is outside all local mounts"
    );
    copy::remove_entry(&record.data_path)
        .with_context(|| format!("cannot permanently delete {}", record.data_path.display()))?;
    fs::remove_file(&record.info_path)
        .with_context(|| format!("deleted Trash entry but could not remove {}", record.info_path.display()))
}

fn store_entry(root: &TrashRoot, source: &Path, path_in_info: &Path) -> Result<Record> {
    let (files, info) = ensure_trash_dirs(root)?;
    let name = source
        .file_name()
        .context("Trash source has no name")?;
    let name_string = name.to_str().context("Trash source name is not valid UTF-8")?;
    validate_leaf(Path::new(name))?;
    for suffix in 0..1024u32 {
        let stored_name = if suffix == 0 {
            name.to_os_string()
        } else {
            format!("{name_string}.{suffix}").into()
        };
        let data_path = files.join(&stored_name);
        let info_path = info.join(format!("{}.trashinfo", stored_name.to_string_lossy()));
        if path_exists(&data_path)? || path_exists(&info_path)? {
            continue;
        }
        let encoded_path = encode_path(path_in_info);
        let deleted_at = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let body = format!("[Trash Info]\nPath={encoded_path}\nDeletionDate={deleted_at}\n");
        match write_info_new(&info_path, body.as_bytes()) {
            Ok(()) => {}
            Err(error) if is_already_exists(&error) => continue,
            Err(error) => return Err(error),
        }
        if let Err(error) = copy::rename_noreplace(source, &data_path) {
            let _ = fs::remove_file(&info_path);
            return Err(error);
        }
        let metadata = fs::symlink_metadata(&data_path)?;
        let original_path = if let Some(base) = &root.relative_base {
            base.join(path_in_info)
        } else {
            path_in_info.to_path_buf()
        };
        ensure!(original_path.is_absolute(), "Trash path must be absolute");
        return Ok(Record {
            trash_root: root.clone(),
            stored_name: PathBuf::from(stored_name),
            data_path,
            info_path,
            original_path,
            deleted_at,
            is_dir: metadata.is_dir(),
        });
    }
bail!("no unique name is available in {}", files.display())
}

fn path_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn is_already_exists(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.kind() == std::io::ErrorKind::AlreadyExists)
    })
}

fn write_info_new(path: &Path, contents: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("cannot create {}", path.display()))?;
    let result = file.write_all(contents).and_then(|_| file.sync_all());
    if let Err(error) = result {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

fn item_for_record(mounts: &[Mount], record: Record) -> Result<TrashItem> {
    let name = record
        .original_path
        .file_name()
        .and_then(|name| name.to_str())
        .context("Trash original name is not valid UTF-8")?
        .to_owned();
    Ok(TrashItem {
        id: item_id(&record.trash_root.path, &record.stored_name),
        name,
        original_path: record.original_path.to_string_lossy().into_owned(),
        deleted_at: record.deleted_at,
        is_dir: record.is_dir,
        can_restore: can_restore(mounts, &record.original_path),
    })
}

fn find_record(data_home: &Path, include_volume_trash: bool, id: &str) -> Result<Record> {
    ensure!(id.starts_with("v1:"), "invalid Trash item id");
    for root in discover_roots(data_home, include_volume_trash)? {
        let files = root.path.join("files");
        let info = root.path.join("info");
        for entry in fs::read_dir(&info).with_context(|| format!("cannot read {}", info.display()))? {
            let entry = entry?;
            let info_path = entry.path();
            let info_metadata = fs::symlink_metadata(&info_path)?;
            if info_metadata.file_type().is_symlink() || !info_metadata.is_file() {
                continue;
            }
            let Some(info_name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(stored_name) = info_name.strip_suffix(".trashinfo") else {
                continue;
            };
            validate_leaf(Path::new(stored_name))?;
            let stored_name_path = PathBuf::from(stored_name);
            if item_id(&root.path, &stored_name_path) != id {
                continue;
            }
            let data_path = files.join(&stored_name_path);
            let metadata = fs::symlink_metadata(&data_path)
                .with_context(|| format!("cannot find Trash data {}", data_path.display()))?;
            let (original_path, deleted_at) = parse_info(&info_path, root.relative_base.as_deref())?;
            return Ok(Record {
                trash_root: root,
                stored_name: stored_name_path,
                data_path,
                info_path,
                original_path,
                deleted_at,
                is_dir: metadata.is_dir(),
            });
        }
    }
    bail!("Trash item not found")
}

fn discover_roots(data_home: &Path, include_volume_trash: bool) -> Result<Vec<TrashRoot>> {
    let mut roots = Vec::new();
    let user_root = TrashRoot {
        path: data_home.join("Trash"),
        relative_base: None,
    };
    if existing_trash_root(&user_root)? {
        roots.push(user_root);
    }
    if include_volume_trash {
        let uid = current_uid();
        for mountpoint in mountpoints() {
            for path in [
                mountpoint.join(format!(".Trash-{uid}")),
                mountpoint.join(".Trash").join(uid.to_string()),
            ] {
                let root = TrashRoot {
                    path,
                    relative_base: Some(mountpoint.clone()),
                };
                if existing_trash_root(&root)? && !roots.iter().any(|known| known.path == root.path) {
                    roots.push(root);
                }
            }
        }
    }
    Ok(roots)
}

fn existing_trash_root(root: &TrashRoot) -> Result<bool> {
    if root.relative_base.is_some()
        && root
            .path
            .parent()
            .is_some_and(|parent| parent.file_name().is_some_and(|name| name == ".Trash"))
    {
        let shared = root.path.parent().context("missing .Trash parent")?;
        let metadata = match fs::symlink_metadata(shared) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || mode(&metadata) & 0o1002 != 0o1002
        {
            return Ok(false);
        }
    }
    let metadata = match fs::symlink_metadata(&root.path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        !metadata.file_type().is_symlink()
            && metadata.is_dir()
            && owned_by_current_user(&metadata)
            && mode(&metadata) & 0o077 == 0,
        "unsafe Trash directory {}",
        root.path.display()
    );
    let files = root.path.join("files");
    let info = root.path.join("info");
    for directory in [&files, &info] {
        let metadata = match fs::symlink_metadata(directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        ensure!(
            !metadata.file_type().is_symlink()
                && metadata.is_dir()
                && owned_by_current_user(&metadata)
                && mode(&metadata) & 0o077 == 0,
            "unsafe Trash directory {}",
            directory.display()
        );
    }
    Ok(true)
}

fn ensure_trash_dirs(root: &TrashRoot) -> Result<(PathBuf, PathBuf)> {
    if root.relative_base.is_some()
        && root
            .path
            .parent()
            .is_some_and(|parent| parent.file_name().is_some_and(|name| name == ".Trash"))
    {
        let shared = root.path.parent().context("missing .Trash parent")?;
        let metadata = fs::symlink_metadata(shared)?;
        ensure!(
            !metadata.file_type().is_symlink()
                && metadata.is_dir()
                && mode(&metadata) & 0o1002 == 0o1002,
            "unsafe shared Trash directory {}",
            shared.display()
        );
    }
    if root.relative_base.is_none() {
        ensure_directory_chain(root.path.parent().context("Trash path has no parent")?)?;
    }
    ensure_owned_dir(&root.path)?;
    let files = root.path.join("files");
    let info = root.path.join("info");
    ensure_owned_dir(&files)?;
    ensure_owned_dir(&info)?;
    Ok((files, info))
}
fn ensure_directory_chain(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                !metadata.file_type().is_symlink() && metadata.is_dir(),
                "unsafe directory in Trash path {}",
                path.display()
            );
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = path.parent().context("Trash data directory has no parent")?;
    ensure!(parent != path, "cannot create Trash data directory");
    ensure_directory_chain(parent)?;
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    match builder.create(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(path)?;
            ensure!(
                !metadata.file_type().is_symlink() && metadata.is_dir(),
                "unsafe directory in Trash path {}",
                path.display()
            );
            Ok(())
        }
        Err(error) => Err(error).with_context(|| format!("cannot create {}", path.display())),
    }
}


fn ensure_owned_dir(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                !metadata.file_type().is_symlink() && metadata.is_dir() && owned_by_current_user(&metadata),
                "unsafe Trash directory {}",
                path.display()
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt as _;
                builder.mode(0o700);
            }
            builder
                .create(path)
                .with_context(|| format!("cannot create Trash directory {}", path.display()))?;
            let metadata = fs::symlink_metadata(path)?;
            ensure!(
                !metadata.file_type().is_symlink() && metadata.is_dir() && owned_by_current_user(&metadata),
                "unsafe Trash directory {}",
                path.display()
            );
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn local_trash_root(mountpoint: &Path) -> Result<TrashRoot> {
    let shared = mountpoint.join(".Trash");
    if let Ok(metadata) = fs::symlink_metadata(&shared) {
        if !metadata.file_type().is_symlink()
            && metadata.is_dir()
            && mode(&metadata) & 0o1002 == 0o1002
        {
            return Ok(TrashRoot {
                path: shared.join(current_uid().to_string()),
                relative_base: Some(mountpoint.to_path_buf()),
            });
        }
    }
    Ok(TrashRoot {
        path: mountpoint.join(format!(".Trash-{}", current_uid())),
        relative_base: Some(mountpoint.to_path_buf()),
    })
}

fn parse_info(path: &Path, relative_base: Option<&Path>) -> Result<(PathBuf, String)> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink()
            && metadata.is_file()
            && owned_by_current_user(&metadata)
            && mode(&metadata) & 0o077 == 0,
        "unsafe Trash metadata {}",
        path.display()
    );
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    let opened = file.metadata()?;
    ensure!(
        opened.is_file()
            && owned_by_current_user(&opened)
            && mode(&opened) & 0o077 == 0,
        "unsafe Trash metadata {}",
        path.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        ensure!(
            metadata.dev() == opened.dev() && metadata.ino() == opened.ino(),
            "Trash metadata changed while opening"
        );
    }
    let mut bytes = Vec::with_capacity(opened.len().min(MAX_INFO_BYTES as u64) as usize);
    file.take((MAX_INFO_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= MAX_INFO_BYTES, "Trash metadata is too large");
    let text = std::str::from_utf8(&bytes).context("Trash metadata is not UTF-8")?;
    let mut encoded_path = None;
    let mut deleted_at = String::new();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("Path=") {
            ensure!(encoded_path.is_none(), "duplicate Path in Trash metadata");
            encoded_path = Some(value);
        } else if let Some(value) = line.strip_prefix("DeletionDate=") {
            deleted_at = value.to_owned();
        }
    }
    let decoded = decode_path(encoded_path.context("Trash metadata has no Path")?)?;
    reject_nul_or_newline(&decoded)?;
    let decoded = PathBuf::from(os_string(decoded));
    ensure!(decoded.file_name().is_some(), "Trash path has no entry name");
    let original = if let Some(base) = relative_base {
        ensure!(!decoded.is_absolute(), "relative Trash metadata contains an absolute Path");
        ensure!(
            decoded.components().all(|component| matches!(component, Component::Normal(_))),
            "malformed relative Trash Path"
        );
        base.join(decoded)
    } else {
        ensure!(decoded.is_absolute(), "absolute Trash metadata contains a relative Path");
        ensure!(
            decoded.components().all(|component| !matches!(component, Component::ParentDir | Component::CurDir)),
            "malformed absolute Trash Path"
        );
        decoded
    };
    Ok((original, deleted_at))
}

fn can_restore(mounts: &[Mount], original: &Path) -> bool {
    validate_original(mounts, original).is_ok()
}

fn validate_original(mounts: &[Mount], original: &Path) -> Result<PathBuf> {
    let destination = validate_original_parent(mounts, original)?;
    match fs::symlink_metadata(&destination) {
        Ok(_) => bail!("restore destination already exists"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(destination),
        Err(error) => Err(error.into()),
    }
}

fn validate_original_parent(mounts: &[Mount], original: &Path) -> Result<PathBuf> {
    ensure!(original.is_absolute(), "Trash restore path is not absolute");
    reject_line_breaks(original)?;
    let parent = original.parent().context("Trash restore path has no parent")?;
    let name = original.file_name().context("Trash restore path has no name")?;
    let canonical_parent = fs::canonicalize(parent)
        .with_context(|| format!("Trash restore directory {} is unavailable", parent.display()))?;
    ensure!(
        mounts.iter().any(|mount| canonical_parent.starts_with(&mount.path)),
        "Trash restore path is outside all local mounts"
    );
    Ok(canonical_parent.join(name))
}

fn encode_path(path: &Path) -> String {
    let bytes = path_bytes(path);
    let mut encoded = String::with_capacity(bytes.len());
    for byte in bytes {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            byte => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn decode_path(value: &str) -> Result<Vec<u8>> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            ensure!(index + 2 < bytes.len(), "truncated percent escape in Trash Path");
            let high = hex_value(bytes[index + 1]).context("invalid percent escape in Trash Path")?;
            let low = hex_value(bytes[index + 2]).context("invalid percent escape in Trash Path")?;
            decoded.push(high << 4 | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    Ok(decoded)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn item_id(root: &Path, name: &Path) -> String {
    let mut bytes = path_bytes(root).to_vec();
    bytes.push(0);
    bytes.extend_from_slice(path_bytes(name));
    let mut result = String::with_capacity(3 + bytes.len() * 2);
    result.push_str("v1:");
    for byte in bytes {
        result.push_str(&format!("{byte:02x}"));
    }
    result
}

fn validate_leaf(name: &Path) -> Result<()> {
    ensure!(
        name.components().count() == 1
            && name.file_name().is_some()
            && !name.as_os_str().is_empty(),
        "malformed Trash entry id"
    );
    ensure!(
        !name.to_string_lossy().contains(['/', '\0', '\n', '\r']),
        "malformed Trash entry id"
    );
    Ok(())
}

fn reject_line_breaks(path: &Path) -> Result<()> {
    reject_nul_or_newline(path_bytes(path))
}

fn reject_nul_or_newline(bytes: &[u8]) -> Result<()> {
    ensure!(
        !bytes.iter().any(|byte| matches!(byte, 0 | b'\n' | b'\r')),
        "Trash path contains a NUL or newline"
    );
    Ok(())
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> &[u8] {
    use std::os::unix::ffi::OsStrExt as _;
    path.as_os_str().as_bytes()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> &[u8] {
    path.to_str().unwrap_or_default().as_bytes()
}

#[cfg(unix)]
fn os_string(bytes: Vec<u8>) -> std::ffi::OsString {
    use std::os::unix::ffi::OsStringExt as _;
    std::ffi::OsString::from_vec(bytes)
}

#[cfg(not(unix))]
fn os_string(bytes: Vec<u8>) -> std::ffi::OsString {
    std::ffi::OsString::from(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(unix)]
fn current_uid() -> u32 {
    unsafe { libc::geteuid() }
}

#[cfg(not(unix))]
fn current_uid() -> u32 {
    0
}

#[cfg(unix)]
fn owned_by_current_user(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    metadata.uid() == current_uid()
}

#[cfg(not(unix))]
fn owned_by_current_user(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    metadata.permissions().mode()
}

#[cfg(not(unix))]
fn mode(_metadata: &fs::Metadata) -> u32 {
    0
}

fn is_cross_device(error: &anyhow::Error) -> bool {
    #[cfg(unix)]
    {
        error.chain().any(|cause| {
            cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.raw_os_error() == Some(libc::EXDEV))
        })
    }
    #[cfg(not(unix))]
    {
        let _ = error;
        false
    }
}

#[cfg(unix)]
fn mountpoints() -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string("/proc/self/mountinfo") else {
        return vec![PathBuf::from("/")];
    };
    let mut paths = text
        .lines()
        .filter_map(|line| line.split_whitespace().nth(4).map(unescape_mount_field))
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if !paths.iter().any(|path| path == Path::new("/")) {
        paths.push(PathBuf::from("/"));
    }
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(not(unix))]
fn mountpoints() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(unix)]
fn mountpoint_for(path: &Path) -> Option<PathBuf> {
    mountpoints()
        .into_iter()
        .filter(|mountpoint| path.starts_with(mountpoint))
        .max_by_key(|mountpoint| mountpoint.components().count())
}

#[cfg(not(unix))]
fn mountpoint_for(_path: &Path) -> Option<PathBuf> {
    None
}

fn filesystem_root(path: &Path) -> PathBuf {
    let mut root = path.to_path_buf();
    while let Some(parent) = root.parent() {
        if parent == root {
            break;
        }
        root = parent.to_path_buf();
    }
    root
}

#[cfg(unix)]
fn unescape_mount_field(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 3 < bytes.len() {
            match &bytes[index + 1..index + 4] {
                b"040" => decoded.push(b' '),
                b"011" => decoded.push(b'\t'),
                b"012" => decoded.push(b'\n'),
                b"134" => decoded.push(b'\\'),
                _ => {
                    decoded.push(bytes[index]);
                    index += 1;
                    continue;
                }
            }
            index += 4;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}
