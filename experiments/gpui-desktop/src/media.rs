use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};

use crate::api::{Api, FileInfo};

pub const MAX_TEXT_PREVIEW_BYTES: usize = 1 << 20;
const MAX_STAGED_NAME_BYTES: usize = 240;
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Resolve a file to a local real path or stage a private remote copy for desktop use.
pub fn prepare_file(api: &Api, item: &FileInfo) -> Result<PathBuf> {
    ensure_file(item)?;
    if let Some(path) = local_file(api, &item.path)? {
        return Ok(path);
    }

    let staging_dir = create_staging_dir()?;
    let mut staged_item = item.clone();
    staged_item.name = safe_staged_name(&item.name);
    let target = staging_dir.join(&staged_item.name);
    let result = (|| {
        if target.try_exists()? {
            bail!("staging target already exists");
        }
        let downloaded = api
            .download(&staged_item, &staging_dir)
            .with_context(|| format!("cannot stage remote file {}", item.path))?;
        if downloaded != target || !downloaded.is_file() {
            bail!("provider returned an unexpected staged file path");
        }
        Ok(downloaded)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging_dir);
    }
    result
}

/// Prepare a file and ask the platform's default application to open it.
pub fn open_file(api: &Api, item: &FileInfo) -> Result<PathBuf> {
    let path = prepare_file(api, item)?;
    open_default_application(&path)?;
    Ok(path)
}

/// Return the whole UTF-8 text when it fits the preview bound; oversized files are rejected.
pub fn read_text_preview(api: &Api, item: &FileInfo) -> Result<String> {
    ensure_file(item)?;
    api.read_text(&item.path, MAX_TEXT_PREVIEW_BYTES)
        .with_context(|| format!("cannot read text preview for {}", item.path))
}

/// Return a PNG thumbnail, rendering local files natively and requesting remote thumbnails directly.
pub fn thumbnail(api: &Api, item: &FileInfo, width: u32, height: u32) -> Result<Vec<u8>> {
    let _span = crate::profile::span("thumbnail").with(|| format!("{} {width}x{height}", item.path));
    ensure_file(item)?;
    if let Some(path) = local_file(api, &item.path)? {
        return boxbox_thumbnail::thumbnail_path(&path, width, height)
            .with_context(|| format!("cannot create thumbnail for {}", item.path));
    }
    let bytes = api
        .thumbnail(&item.path, width, height)
        .with_context(|| format!("cannot request remote thumbnail for {}", item.path))?;
    boxbox_thumbnail::thumbnail_bytes(&bytes, width, height)
        .with_context(|| format!("cannot decode remote thumbnail for {}", item.path))
}

fn ensure_file(item: &FileInfo) -> Result<()> {
    if item.is_dir {
        bail!("operation requires a file, not a directory");
    }
    Ok(())
}

fn local_file(api: &Api, virtual_path: &str) -> Result<Option<PathBuf>> {
    api.local_path(virtual_path)
}

fn create_staging_dir() -> Result<PathBuf> {
    let temp_dir = std::env::temp_dir();
    for _ in 0..32 {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = temp_dir.join(format!(
            "boxbox-open-{}-{timestamp:x}-{sequence:x}",
            std::process::id()
        ));
        match create_private_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("cannot create private staging directory"),
        }
    }
    bail!("cannot allocate a unique private staging directory")
}

fn create_private_dir(path: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

fn safe_staged_name(name: &str) -> String {
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension))
            if !extension.is_empty()
                && extension.len() <= 16
                && extension.bytes().all(|byte| byte.is_ascii_alphanumeric()) =>
        {
            (stem, extension.to_ascii_lowercase())
        }
        _ => (name, String::new()),
    };
    let mut stem = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || "._-".contains(character) {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    stem = stem.trim_matches('.').to_owned();
    if stem.is_empty() {
        stem.push_str("file");
    }
    let extension_bytes = if extension.is_empty() {
        0
    } else {
        extension.len() + 1
    };
    let max_stem_bytes = MAX_STAGED_NAME_BYTES.saturating_sub(extension_bytes);
    stem.truncate(max_stem_bytes);
    if extension.is_empty() {
        stem
    } else {
        format!("{stem}.{extension}")
    }
}

fn open_default_application(path: &Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .with_context(|| format!("cannot open {} in its default application", path.display()))?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .with_context(|| format!("cannot open {} in its default application", path.display()))?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe")
            .arg(path)
            .spawn()
            .with_context(|| format!("cannot open {} in its default application", path.display()))?;
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    bail!("opening files is unsupported on this platform");
    Ok(())
}
