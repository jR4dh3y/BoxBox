use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{bail, ensure, Context as _, Result};

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

/// Copy an entry tree without following symlinks and without replacing `destination`.
pub(super) fn copy_no_overwrite(
    source: &Path,
    destination: &Path,
    confinement: Option<&Path>,
) -> Result<()> {
    let parent = destination
        .parent()
        .context("destination has no parent directory")?;
    ensure!(parent.is_dir(), "destination parent is not a directory");
    let stage = match fs::symlink_metadata(source) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let stage = unique_path(parent, "copy")?;
            create_symlink(source, &stage, &metadata)?;
            stage
        }
        Ok(metadata) if metadata.is_dir() => {
            let canonical = fs::canonicalize(source)?;
            ensure_confined(&canonical, confinement)?;
            let stage = create_temp_dir(parent, "copy")?;
            let mut visited = std::collections::HashSet::new();
            visited.insert(canonical.clone());
            if let Err(error) = copy_directory_contents(&canonical, &stage, confinement, &mut visited)
            {
                let _ = fs::remove_dir_all(&stage);
                return Err(error);
            }
            fs::set_permissions(&stage, metadata.permissions())?;
            stage
        }
        Ok(_) => {
            let stage = unique_path(parent, "copy")?;
            let mut buffer = Vec::new();
            copy_regular_file(source, &stage, confinement, &mut buffer)?;
            stage
        }
        Err(error) => return Err(error).with_context(|| format!("cannot inspect {}", source.display())),
    };

    if let Err(error) = rename_noreplace(&stage, destination) {
        let _ = remove_entry(&stage);
        return Err(error);
    }
    Ok(())
}

pub(super) fn write_new_file(destination: &Path, bytes: &[u8]) -> Result<()> {
    let parent = destination
        .parent()
        .context("destination has no parent directory")?;
    let (stage, mut file) = create_temp_file(parent, "new")?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        rename_noreplace(&stage, destination)
    })();
    if result.is_err() {
        let _ = fs::remove_file(stage);
    }
    result
}

pub(super) fn remove_entry(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTypeExt as _;
        if metadata.file_type().is_symlink_dir() {
            return fs::remove_dir(path).with_context(|| format!("cannot remove {}", path.display()));
        }
    }
    if metadata.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("cannot remove {}", path.display()))
    } else {
        fs::remove_file(path).with_context(|| format!("cannot remove {}", path.display()))
    }
}

pub(super) fn rename_noreplace(source: &Path, destination: &Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt as _;
        let source = CString::new(source.as_os_str().as_bytes())?;
        let destination = CString::new(destination.as_os_str().as_bytes())?;
        // SAFETY: both C strings are NUL-terminated and syscall receives the Linux renameat2 ABI.
        let result = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                destination.as_ptr(),
                1u32, // RENAME_NOREPLACE
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error()).with_context(|| {
                format!(
                    "cannot move {} to {} without replacing an existing entry",
                    source.to_string_lossy(),
                    destination.to_string_lossy()
                )
            })
        }
    }
    #[cfg(windows)]
    return crate::windows::rename_noreplace(source, destination);

    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (source, destination);
        bail!("atomic no-replace moves are unsupported on this platform")
    }
}

fn copy_directory_contents(
    source: &Path,
    destination: &Path,
    confinement: Option<&Path>,
    visited: &mut std::collections::HashSet<PathBuf>,
) -> Result<()> {
    let mut pending = vec![(source.to_path_buf(), destination.to_path_buf())];
    let mut directories = Vec::new();
    let mut buffer = Vec::new();
    while let Some((source_dir, destination_dir)) = pending.pop() {
        let source_dir = fs::canonicalize(&source_dir)?;
        ensure_confined(&source_dir, confinement)?;
        let entries =
            fs::read_dir(&source_dir).with_context(|| format!("cannot read {}", source_dir.display()))?;
        for entry in entries {
            let entry = entry?;
            let source_entry = entry.path();
            let destination_entry = destination_dir.join(entry.file_name());
            let metadata = fs::symlink_metadata(&source_entry)?;
            if metadata.file_type().is_symlink() {
                create_symlink(&source_entry, &destination_entry, &metadata)?;
            } else if metadata.is_dir() {
                let canonical = fs::canonicalize(&source_entry)?;
                ensure_confined(&canonical, confinement)?;
                fs::create_dir(&destination_entry)?;
                directories.push((destination_entry.clone(), metadata.permissions()));
                if visited.insert(canonical.clone()) {
                    pending.push((canonical, destination_entry));
                }
            } else if metadata.is_file() {
                copy_regular_file(&source_entry, &destination_entry, confinement, &mut buffer)?;
            } else {
                bail!("unsupported filesystem entry: {}", source_entry.display());
            }
        }
    }
    for (directory, permissions) in directories.into_iter().rev() {
        fs::set_permissions(directory, permissions)?;
    }
    Ok(())
}

const COPY_CHUNK: usize = 1024 * 1024;

#[cfg(target_os = "linux")]
fn copy_in_kernel(input: &File, output: &File, expected: u64) -> Result<u64> {
    use std::os::unix::io::AsRawFd as _;
    let (in_fd, out_fd) = (input.as_raw_fd(), output.as_raw_fd());
    let mut copied = 0;
    while copied < expected {
        let count = (expected - copied).min(COPY_CHUNK as u64) as usize;
        crate::progress::check()?;
        // SAFETY: valid open file descriptors; null offsets advance each fd's offset.
        let written = unsafe {
            libc::copy_file_range(in_fd, std::ptr::null_mut(), out_fd, std::ptr::null_mut(), count, 0)
        };
        if written > 0 {
            copied += written as u64;
            crate::progress::advance(written as u64);
            continue;
        }
        if written == 0 {
            break;
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        if copied == 0
            && matches!(
                error.raw_os_error(),
                Some(libc::EXDEV | libc::ENOSYS | libc::EINVAL | libc::EOPNOTSUPP | libc::EPERM)
            )
        {
            break;
        }
        return Err(error.into());
    }
    Ok(copied)
}

fn copy_regular_file(
    source: &Path,
    destination: &Path,
    confinement: Option<&Path>,
    buffer: &mut Vec<u8>,
) -> Result<()> {
    let canonical = fs::canonicalize(source)
        .with_context(|| format!("cannot resolve {}", source.display()))?;
    ensure_confined(&canonical, confinement)?;
    let metadata = fs::metadata(&canonical)?;
    ensure!(metadata.is_file(), "{} is not a regular file", source.display());
    let mut input_options = OpenOptions::new();
    input_options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        input_options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut input = input_options.open(&canonical)?;
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

    let result = {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        // Private until the source's permissions apply, after the last byte.
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut output = options
            .open(destination)
            .with_context(|| format!("cannot create {}", destination.display()))?;
        (|| -> Result<()> {
            #[cfg(target_os = "linux")]
            let copied = copy_in_kernel(&input, &output, opened.len())?;
            #[cfg(not(target_os = "linux"))]
            let copied = 0;

            // Check for EOF with read, even after a full kernel copy. Another
            // kernel copy at the file-size limit can raise SIGXFSZ before checking EOF.
            let needed = (opened.len() - copied).clamp(4096, COPY_CHUNK as u64) as usize;
            if buffer.len() < needed {
                buffer.resize(needed, 0);
            }
            let chunk = &mut buffer[..needed];
            loop {
                crate::progress::check()?;
                let read = match io::Read::read(&mut input, chunk) {
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    result => result?,
                };
                if read == 0 {
                    break;
                }
                output.write_all(&chunk[..read])?;
                crate::progress::advance(read as u64);
            }
            output.flush()?;
            output.set_permissions(metadata.permissions())?;
            Ok(())
        })()
    };
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

fn ensure_confined(path: &Path, confinement: Option<&Path>) -> Result<()> {
    if let Some(root) = confinement {
        ensure!(path.starts_with(root), "{} resolves outside its mount", path.display());
    }
    Ok(())
}

pub(super) fn create_temp_file(parent: &Path, purpose: &str) -> Result<(PathBuf, File)> {
    for _ in 0..128 {
        let path = unique_path(parent, purpose)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).with_context(|| format!("cannot create {}", path.display())),
        }
    }
    bail!("could not reserve a temporary file in {}", parent.display())
}

fn create_temp_dir(parent: &Path, purpose: &str) -> Result<PathBuf> {
    for _ in 0..128 {
        let path = unique_path(parent, purpose)?;
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).with_context(|| format!("cannot create {}", path.display())),
        }
    }
    bail!("could not reserve a temporary directory in {}", parent.display())
}

fn unique_path(parent: &Path, purpose: &str) -> Result<PathBuf> {
    let parent_name = parent
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("directory");
    let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(".{parent_name}.boxbox-{purpose}-{}-{id}", std::process::id())))
}

fn create_symlink(source: &Path, link: &Path, metadata: &fs::Metadata) -> Result<()> {
    let target = fs::read_link(source)?;
    #[cfg(unix)]
    let result = {
        let _ = metadata;
        std::os::unix::fs::symlink(target, link)
    };
    #[cfg(windows)]
    let result = {
        use std::os::windows::fs::FileTypeExt as _;
        if metadata.file_type().is_symlink_dir() {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    };
    result.with_context(|| format!("cannot create symlink {}", link.display()))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn copy_reads_past_zero_metadata_size() {
        let root = std::env::temp_dir().join(format!("boxbox-copy-proc-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let source = Path::new("/proc/self/cmdline");
        let target = root.join("cmdline");
        let expected = fs::read(source).unwrap();
        assert!(!expected.is_empty());
        assert_eq!(fs::metadata(source).unwrap().len(), 0);
        copy_no_overwrite(source, &target, None).unwrap();
        assert_eq!(fs::read(target).unwrap(), expected);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn copy_finishes_at_a_chunk_boundary() {
        let root = std::env::temp_dir().join(format!("boxbox-copy-chunk-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source");
        let target = root.join("target");
        let expected = vec![0xa5; COPY_CHUNK];
        fs::write(&source, &expected).unwrap();
        copy_no_overwrite(&source, &target, None).unwrap();
        assert_eq!(fs::read(target).unwrap(), expected);
        fs::remove_dir_all(root).unwrap();
    }
}
