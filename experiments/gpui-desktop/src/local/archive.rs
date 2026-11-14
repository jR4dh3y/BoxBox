use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context as _, Result};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::copy;

pub(super) fn write_zip(
    source: &Path,
    destination: &Path,
    root_name: &str,
    confinement: &Path,
) -> Result<()> {
    validate_zip_component(root_name)?;
    let parent = destination
        .parent()
        .context("archive destination has no parent")?;
    let (stage, file) = copy::create_temp_file(parent, "archive")?;
    let result = (|| {
        let mut zip = ZipWriter::new(file);
        let root = format!("{root_name}/");
        zip.add_directory(&root, directory_options(None))?;
        let canonical = fs::canonicalize(source)?;
        ensure!(canonical.starts_with(confinement), "archive source escapes its mount");
        let mut visited = HashSet::from([canonical.clone()]);
        append_directory(&mut zip, &canonical, &root, confinement, &mut visited)?;
        let file = zip.finish()?;
        file.sync_all()?;
        drop(file);
        copy::rename_noreplace(&stage, destination)
    })();
    if result.is_err() {
        let _ = fs::remove_file(stage);
    }
    result.with_context(|| format!("cannot create archive {}", destination.display()))
}

/// A ZIP at `destination` holding each source at its top level, folders with all
/// they contain. Each source comes with the mount it must stay inside.
pub(super) fn write_zip_of(sources: &[(PathBuf, PathBuf)], destination: &Path) -> Result<()> {
    let parent = destination.parent().context("archive destination has no parent")?;
    let (stage, file) = copy::create_temp_file(parent, "archive")?;
    let result = (|| {
        let mut zip = ZipWriter::new(file);
        let mut visited = HashSet::new();
        for (source, confinement) in sources {
            let mut pending = Vec::new();
            append_entry(&mut zip, source, "", confinement, &mut visited, &mut pending)?;
            drain(&mut zip, pending, confinement, &mut visited)?;
        }
        let file = zip.finish()?;
        file.sync_all()?;
        drop(file);
        copy::rename_noreplace(&stage, destination)
    })();
    if result.is_err() {
        let _ = fs::remove_file(stage);
    }
    result.with_context(|| format!("cannot create archive {}", destination.display()))
}

fn append_directory(
    zip: &mut ZipWriter<File>,
    directory: &Path,
    archive_directory: &str,
    confinement: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<()> {
    drain(zip, vec![(directory.to_path_buf(), archive_directory.to_owned())], confinement, visited)
}

/// Adds every folder in `pending`, and the folders found inside them.
fn drain(zip: &mut ZipWriter<File>, mut pending: Vec<(PathBuf, String)>, confinement: &Path, visited: &mut HashSet<PathBuf>) -> Result<()> {
    while let Some((directory, archive_directory)) = pending.pop() {
        let canonical_directory = fs::canonicalize(&directory)?;
        ensure!(
            canonical_directory.starts_with(confinement),
            "{} escapes its mount",
            directory.display()
        );
        let entries = fs::read_dir(&canonical_directory)
            .with_context(|| format!("cannot read directory {}", canonical_directory.display()))?;
        for entry in entries {
            append_entry(zip, &entry?.path(), &archive_directory, confinement, visited, &mut pending)?;
        }
    }
    Ok(())
}

/// Adds one entry under `archive_directory`; a folder is queued in `pending`.
fn append_entry(
    zip: &mut ZipWriter<File>,
    source: &Path,
    archive_directory: &str,
    confinement: &Path,
    visited: &mut HashSet<PathBuf>,
    pending: &mut Vec<(PathBuf, String)>,
) -> Result<()> {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("file name is not valid UTF-8"))?;
    validate_zip_component(name)?;
    let metadata = fs::symlink_metadata(source)?;
    let archive_name = format!("{archive_directory}{name}");
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(source)?;
        zip.start_file(&archive_name, symlink_options())?;
        zip.write_all(path_bytes(&target))?;
    } else if metadata.is_dir() {
        let canonical = fs::canonicalize(source)?;
        ensure!(canonical.starts_with(confinement), "{} escapes its mount", source.display());
        let archive_child = format!("{archive_name}/");
        zip.add_directory(&archive_child, directory_options(Some(&metadata)))?;
        if visited.insert(canonical.clone()) {
            pending.push((canonical, archive_child));
        }
    } else if metadata.is_file() {
        let canonical = fs::canonicalize(source)?;
        ensure!(canonical.starts_with(confinement), "{} escapes its mount", source.display());
        let mut input = open_no_follow(&canonical)?;
        let opened = input.metadata()?;
        ensure!(opened.is_file(), "{} is not a regular file", source.display());
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            ensure!(
                opened.dev() == metadata.dev() && opened.ino() == metadata.ino(),
                "{} changed while opening",
                source.display()
            );
        }
        zip.start_file(&archive_name, file_options(&metadata))?;
        io::copy(&mut input, zip)?;
    } else {
        bail!("unsupported filesystem entry: {}", source.display());
    }
    Ok(())
}

fn validate_zip_component(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name != "."
            && name != ".."
            && !name.contains(['/', '\\', '\0']),
        "unsafe ZIP path component {name:?}"
    );
    Ok(())
}

fn directory_options(metadata: Option<&fs::Metadata>) -> SimpleFileOptions {
    let mut options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o755);
    if let Some(metadata) = metadata {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            options = options.unix_permissions(metadata.permissions().mode() & 0o777);
        }
        #[cfg(not(unix))]
        let _ = metadata;
    }
    options
}

fn file_options(metadata: &fs::Metadata) -> SimpleFileOptions {
    let mut options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        options = options.unix_permissions(metadata.permissions().mode() & 0o777);
    }
    #[cfg(not(unix))]
    let _ = metadata;
    options
}

fn symlink_options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o120777)
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
fn open_no_follow(path: &Path) -> Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    Ok(File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?)
}

#[cfg(not(unix))]
fn open_no_follow(path: &Path) -> Result<File> {
    Ok(File::open(path)?)
}
