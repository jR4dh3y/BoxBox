//! Direct local filesystem implementation for the GPUI desktop client.

mod archive;
mod copy;
pub mod network;
mod recent;
mod search;
mod trash;

use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use anyhow::{bail, ensure, Context as _, Result};
use chrono::{DateTime, Utc};

use crate::api::{DriveStats, FileInfo, MountPoint, RootKind, TrashItem};

#[derive(Clone)]
pub(super) struct Mount {
    pub(super) point: MountPoint,
    pub(super) path: PathBuf,
}

/// Local files exposed through the same virtual paths as the remote file API.
pub struct LocalFiles {
    mounts: Vec<Mount>,
    trash_data_home: PathBuf,
    volume_trash: bool,
}

impl LocalFiles {
    /// Open the host filesystem, or use BOXBOX_ROOT as an isolated local root.
    pub fn new() -> Result<Self> {
        if let Some(root) = std::env::var_os("BOXBOX_ROOT") {
            return Self::with_root(root.into());
        }

        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is not set")?;
        let home = canonical_directory(&home).context("cannot open the home directory")?;
        let filesystem = canonical_directory(Path::new("/"))?;
        let mut mounts = vec![Mount {
            point: MountPoint {
                name: "files".into(),
                label: Some("Files".into()),
                kind: RootKind::Drive,
            },
            path: filesystem,
        }];
        add_mount(
            &mut mounts,
            "home",
            Some("Home"),
            RootKind::Drive,
            home.clone(),
        );
        for (name, label, config_key, fallback) in [
            ("desktop", "Desktop", "XDG_DESKTOP_DIR", "Desktop"),
            ("documents", "Documents", "XDG_DOCUMENTS_DIR", "Documents"),
            ("downloads", "Downloads", "XDG_DOWNLOAD_DIR", "Downloads"),
            ("music", "Music", "XDG_MUSIC_DIR", "Music"),
            ("pictures", "Pictures", "XDG_PICTURES_DIR", "Pictures"),
            ("videos", "Videos", "XDG_VIDEOS_DIR", "Videos"),
            ("templates", "Templates", "XDG_TEMPLATES_DIR", "Templates"),
        ] {
            let candidate = user_directory(&home, config_key)
                .unwrap_or_else(|| home.join(fallback));
            if let Ok(path) = canonical_directory(&candidate) {
                if path.starts_with(&home) {
                    add_mount(&mut mounts, name, Some(label), RootKind::Place, path);
                }
            }
        }

        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"));
        Ok(Self {
            mounts,
            trash_data_home: data_home,
            volume_trash: true,
        })
    }

    /// Build a single `home` mount rooted at `root`; intended for isolated fixtures.
    pub fn with_root(root: PathBuf) -> Result<Self> {
        let root = canonical_directory(&root).context("cannot open local root")?;
        Ok(Self {
            mounts: vec![Mount {
                point: MountPoint {
                    name: "home".into(),
                    label: Some("Home".into()),
                    kind: RootKind::Drive,
                },
                path: root.clone(),
            }],
            trash_data_home: root.join(".local/share"),
            volume_trash: false,
        })
    }

    /// The mounts, with Recent first among the places and Network last. Those two
    /// are listings, not folders.
    pub fn roots(&self) -> Result<Vec<MountPoint>> {
        let place = |name: &str, label: &str| MountPoint { name: name.into(), label: Some(label.into()), kind: RootKind::Place };
        let (drives, places): (Vec<_>, Vec<_>) = self.mounts.iter().map(|mount| mount.point.clone()).partition(|point| point.kind == RootKind::Drive);
        Ok(drives.into_iter().chain([place(RECENT, "Recent")]).chain(places).chain([place(NETWORK, "Network")]).collect())
    }

    pub fn drives(&self) -> Result<Vec<DriveStats>> {
        self.mounts
            .iter()
            .filter(|mount| mount.point.kind != RootKind::Place)
            .map(drive_stats)
            .collect()
    }

    pub fn list(&self, path: &str) -> Result<Vec<FileInfo>> {
        match path {
            RECENT => return Ok(self.recent()),
            NETWORK => return network_entries(),
            _ => {}
        }
        let (mount, directory) = self.resolve_directory(path)?;
        let mut entries = fs::read_dir(&directory)
            .with_context(|| format!("cannot list {}", path))?
            .map(|entry| entry.map_err(anyhow::Error::from))
            .collect::<Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        entries
            .into_iter()
            .map(|entry| self.info_for_entry(mount, path, &entry.path()))
            .collect()
    }

    pub fn create_directory(&self, parent: &str, name: &str) -> Result<FileInfo> {
        validate_name(name)?;
        let (mount, directory) = self.resolve_directory(parent)?;
        let target = directory.join(name);
        self.ensure_mutable_target(&target, true)?;
        fs::create_dir(&target).with_context(|| format!("cannot create directory {name:?}"))?;
        self.info_for_path(mount, parent, &target)
    }

    pub fn rename(&self, old_path: &str, new_path: &str) -> Result<FileInfo> {
        let (_source_mount, source) = self.resolve_entry(old_path)?;
        let (destination_mount, destination_parent, destination_name) =
            self.resolve_destination(new_path)?;
        let metadata = fs::symlink_metadata(&source)
            .with_context(|| format!("cannot inspect {old_path}"))?;
        let is_tree = metadata.is_dir();
        self.ensure_mutable_target(&source, is_tree)?;
        let destination = destination_parent.join(destination_name);
        self.ensure_mutable_target(&destination, is_tree)?;
        ensure!(source != destination, "source and destination are the same path");
        if is_tree {
            let source_real = fs::canonicalize(&source)?;
            ensure!(
                !destination.starts_with(&source_real),
                "cannot move a directory into itself"
            );
        }
        copy::rename_noreplace(&source, &destination)
            .with_context(|| format!("cannot rename {old_path} to {new_path}"))?;
        self.info_for_path(destination_mount, parent_of_virtual(new_path), &destination)
    }

    /// Permanently delete one entry. `recursive` must be true to remove a real directory tree.
    pub fn delete(&self, path: &str, recursive: bool) -> Result<()> {
        let (_, entry) = self.resolve_entry(path)?;
        let metadata = fs::symlink_metadata(&entry)
            .with_context(|| format!("cannot inspect {path}"))?;
        let is_tree = metadata.is_dir();
        ensure!(!is_tree || recursive, "directory deletion requires confirmation");
        self.ensure_mutable_target(&entry, is_tree)?;
        if is_tree {
            fs::remove_dir_all(&entry).with_context(|| format!("cannot delete {path}"))
        } else {
            fs::remove_file(&entry).with_context(|| format!("cannot delete {path}"))
        }
    }

    /// Copy a file into `directory`, or write a directory archive as `<name>.zip`.
    pub fn download(&self, item: &FileInfo, directory: &Path) -> Result<PathBuf> {
        let (mount, source_entry) = self.resolve_entry(&item.path)?;
        let source = fs::canonicalize(&source_entry)
            .with_context(|| format!("cannot resolve {}", item.path))?;
        ensure!(
            source.starts_with(&mount.path),
            "{} resolves outside its mount",
            item.path
        );
        let source_meta = fs::metadata(&source)?;
        let directory = canonical_directory(directory).context("download destination is not a directory")?;
        validate_name(&item.name)?;
        let name = source_entry
            .file_name()
            .and_then(|name| name.to_str())
            .context("file name is not valid UTF-8")?;
        ensure!(item.name == name, "file item name does not match its path");
        if source_meta.is_dir() {
            ensure!(
                !directory.starts_with(&source),
                "archive destination cannot be inside the source directory"
            );
            let archive_name = format!("{name}.zip");
            let target = directory.join(&archive_name);
            archive::write_zip(&source, &target, name, &mount.path)?;
            Ok(target)
        } else {
            ensure!(source_meta.is_file(), "unsupported file type at {}", item.path);
            let target = directory.join(name);
            copy::copy_no_overwrite(&source, &target, Some(&mount.path))?;
            Ok(target)
        }
    }

    /// Resolve an existing virtual path to a canonical native path for previews and thumbnails.
    /// One entry's details, as a listing of its folder would give them.
    pub fn stat(&self, path: &str) -> Result<FileInfo> {
        // A drive or place itself, named as the sidebar names it.
        if let Some(mount) = self.mounts.iter().find(|mount| mount.point.name == path) {
            let metadata = fs::metadata(&mount.path).with_context(|| format!("cannot inspect {path}"))?;
            let mod_time = metadata.modified().map(format_time).unwrap_or_default();
            return Ok(FileInfo { name: mount.point.display_name().to_string(), path: path.to_string(), size: metadata.len(), is_dir: true, mod_time });
        }
        let (mount, real) = self.resolve_entry(path)?;
        let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
        self.info_for_path(mount, parent, &real).with_context(|| format!("cannot inspect {path}"))
    }

    /// The virtual path for a real one, under the mount that holds it most closely.
    pub fn virtual_path(&self, real: &Path) -> Option<String> {
        let mount = self.mounts.iter().filter(|mount| real.starts_with(&mount.path)).max_by_key(|mount| mount.path.components().count())?;
        let relative = real.strip_prefix(&mount.path).ok()?;
        let mut path = mount.point.name.clone();
        for part in relative.components() {
            path.push('/');
            path.push_str(&part.as_os_str().to_string_lossy());
        }
        Some(path)
    }

    pub fn local_path(&self, path: &str) -> Result<PathBuf> {
        let (mount, candidate) = self.candidate_path(path)?;
        let resolved = fs::canonicalize(&candidate)
            .with_context(|| format!("cannot resolve {path}"))?;
        ensure!(resolved.starts_with(&mount.path), "{path} resolves outside its mount");
        Ok(resolved)
    }

    pub fn read_text(&self, path: &str, max_bytes: usize) -> Result<String> {
        let (mount, candidate) = self.candidate_path(path)?;
        let resolved = fs::canonicalize(&candidate)
            .with_context(|| format!("cannot read {path}"))?;
        ensure!(resolved.starts_with(&mount.path), "{path} resolves outside its mount");
        let metadata = fs::metadata(&resolved)?;
        ensure!(metadata.is_file(), "{path} is not a regular file");
        ensure!(metadata.len() <= max_bytes as u64, "{path} exceeds the text size limit");
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = options
            .open(&resolved)
            .with_context(|| format!("cannot read {path}"))?;
        let opened = file.metadata()?;
        ensure!(opened.is_file(), "{path} is not a regular file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            ensure!(
                opened.dev() == metadata.dev() && opened.ino() == metadata.ino(),
                "{path} changed while opening"
            );
        }
        let mut contents = Vec::with_capacity(metadata.len() as usize);
        file.take(max_bytes.saturating_add(1) as u64)
            .read_to_end(&mut contents)?;
        ensure!(contents.len() <= max_bytes, "{path} exceeds the text size limit");
        String::from_utf8(contents).with_context(|| format!("{path} is not UTF-8 text"))
    }

    pub fn create_file(&self, parent: &str, name: &str, content: &str) -> Result<FileInfo> {
        validate_name(name)?;
        let (mount, directory) = self.resolve_directory(parent)?;
        let target = directory.join(name);
        self.ensure_mutable_target(&target, false)?;
        copy::write_new_file(&target, content.as_bytes())?;
        self.info_for_path(mount, parent, &target)
    }

    /// Copy one entry to an explicit, non-existing virtual destination path.
    pub fn copy(&self, source: &str, destination: &str) -> Result<()> {
        let (source_mount, source_entry) = self.resolve_entry(source)?;
        let source_meta = fs::symlink_metadata(&source_entry)?;
        let (_, parent, name) = self.resolve_destination(destination)?;
        let target = parent.join(name);
        let is_tree = source_meta.is_dir();
        self.ensure_mutable_target(&target, is_tree)?;
        if is_tree {
            let source_real = fs::canonicalize(&source_entry)?;
            ensure!(
                !target.starts_with(&source_real),
                "cannot copy a directory into itself"
            );
        }
        copy::copy_no_overwrite(&source_entry, &target, Some(&source_mount.path))
            .with_context(|| format!("cannot copy {source} to {destination}"))?;
        Ok(())
    }

    /// Import a selected local file or directory into an existing virtual directory.
    pub fn import_file(&self, source: &Path, parent: &str) -> Result<FileInfo> {
        let source_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .context("source name is not valid UTF-8")?;
        validate_name(source_name)?;
        let (mount, directory) = self.resolve_directory(parent)?;
        let target = directory.join(source_name);
        let metadata = fs::symlink_metadata(source)?;
        let is_tree = metadata.is_dir();
        self.ensure_mutable_target(&target, is_tree)?;
        if is_tree {
            let source_real = fs::canonicalize(source)?;
            ensure!(
                !target.starts_with(&source_real),
                "cannot import a directory into itself"
            );
        }
        copy::copy_no_overwrite(source, &target, None)
            .with_context(|| format!("cannot import {}", source.display()))?;
        self.info_for_path(mount, parent, &target)
    }

    pub fn trash(&self) -> Result<Vec<TrashItem>> {
        trash::list(&self.mounts, &self.trash_data_home, self.volume_trash)
    }

    pub fn move_to_trash(&self, path: &str) -> Result<TrashItem> {
        let (_, entry) = self.resolve_entry(path)?;
        let metadata = fs::symlink_metadata(&entry)
            .with_context(|| format!("cannot inspect {path}"))?;
        self.ensure_mutable_target(&entry, metadata.is_dir())?;
        trash::move_to_trash(
            &self.mounts,
            &self.trash_data_home,
            &entry,
            path,
            self.volume_trash,
        )
    }

    /// Recently used local files, each at its own path, newest first.
    fn recent(&self) -> Vec<FileInfo> {
        recent::files(&self.trash_data_home, 200)
            .into_iter()
            .filter_map(|real| self.stat(&self.virtual_path(&real)?).ok())
            .collect()
    }

    pub fn trash_has_items(&self) -> bool {
        trash::has_items(&self.trash_data_home, self.volume_trash)
    }

    pub fn empty_trash(&self) -> Result<usize> {
        trash::empty(&self.mounts, &self.trash_data_home, self.volume_trash)
    }

    /// The home Trash's `files` folder, which changes whenever Trash does.
    pub fn trash_files(&self) -> PathBuf {
        self.trash_data_home.join("Trash/files")
    }

    pub fn restore_trash(&self, id: &str) -> Result<()> {
        trash::restore(&self.mounts, &self.trash_data_home, self.volume_trash, id)
    }

    pub fn delete_trash(&self, id: &str) -> Result<()> {
        trash::delete(&self.mounts, &self.trash_data_home, self.volume_trash, id)
    }

    fn resolve_directory(&self, path: &str) -> Result<(&Mount, PathBuf)> {
        let (mount, candidate) = self.candidate_path(path)?;
        let directory = fs::canonicalize(&candidate)
            .with_context(|| format!("cannot resolve directory {path}"))?;
        ensure!(directory.starts_with(&mount.path), "{path} resolves outside its mount");
        ensure!(directory.is_dir(), "{path} is not a directory");
        Ok((mount, directory))
    }

    fn resolve_entry(&self, path: &str) -> Result<(&Mount, PathBuf)> {
        let (mount, segments) = self.parse_virtual_path(path)?;
        ensure!(!segments.is_empty(), "cannot mutate a mount root");
        let (name, parent_segments) = segments.split_last().context("invalid path")?;
        let parent_path = append_segments(&mount.path, parent_segments);
        let parent = fs::canonicalize(&parent_path)
            .with_context(|| format!("cannot resolve parent of {path}"))?;
        ensure!(parent.starts_with(&mount.path), "parent of {path} resolves outside its mount");
        ensure!(parent.is_dir(), "parent of {path} is not a directory");
        Ok((mount, parent.join(name)))
    }

    fn resolve_destination(&self, path: &str) -> Result<(&Mount, PathBuf, String)> {
        let (mount, segments) = self.parse_virtual_path(path)?;
        ensure!(!segments.is_empty(), "cannot mutate a mount root");
        let (name, parent_segments) = segments.split_last().context("invalid path")?;
        let parent_path = append_segments(&mount.path, parent_segments);
        let parent = fs::canonicalize(&parent_path)
            .with_context(|| format!("cannot resolve destination parent for {path}"))?;
        ensure!(parent.starts_with(&mount.path), "destination parent escapes its mount");
        ensure!(parent.is_dir(), "destination parent is not a directory");
        Ok((mount, parent, (*name).to_owned()))
    }

    fn candidate_path(&self, path: &str) -> Result<(&Mount, PathBuf)> {
        let (mount, segments) = self.parse_virtual_path(path)?;
        Ok((mount, append_segments(&mount.path, &segments)))
    }

    fn parse_virtual_path<'mount, 'path>(
        &'mount self,
        path: &'path str,
    ) -> Result<(&'mount Mount, Vec<&'path str>)> {
        ensure!(!path.is_empty(), "empty virtual path");
        ensure!(!path.starts_with('/') && !path.ends_with('/'), "malformed virtual path {path:?}");
        ensure!(!path.contains('\0'), "malformed virtual path {path:?}");
        let mut segments = path.split('/');
        let root_name = segments.next().context("empty virtual path")?;
        let mount = self
            .mounts
            .iter()
            .find(|mount| mount.point.name == root_name)
            .with_context(|| format!("unknown local mount {root_name:?}"))?;
        let rest = segments.collect::<Vec<_>>();
        for segment in &rest {
            ensure!(
                !segment.is_empty() && *segment != "." && *segment != "..",
                "malformed virtual path {path:?}"
            );
            validate_name(segment)?;
        }
        Ok((mount, rest))
    }

    fn ensure_mutable_target(&self, path: &Path, tree: bool) -> Result<()> {
        let path = lexical_absolute(path)?;
        for mount in &self.mounts {
            if path == mount.path || (tree && mount.path.starts_with(&path)) {
                bail!("cannot mutate mount root or a directory containing a mount root");
            }
        }
        Ok(())
    }

    fn info_for_entry(&self, mount: &Mount, parent: &str, path: &Path) -> Result<FileInfo> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("file name is not valid UTF-8")?;
        self.info_for_path(mount, parent, path)
            .with_context(|| format!("cannot inspect entry {name:?}"))
    }

    fn info_for_path(&self, mount: &Mount, parent: &str, path: &Path) -> Result<FileInfo> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("file name is not valid UTF-8")?
            .to_owned();
        let entry_metadata = fs::symlink_metadata(path)?;
        let metadata = if entry_metadata.file_type().is_symlink() {
            match fs::canonicalize(path) {
                Ok(target) if target.starts_with(&mount.path) => fs::metadata(target)?,
                _ => entry_metadata,
            }
        } else {
            entry_metadata
        };
        let mod_time = metadata
            .modified()
            .map(format_time)
            .unwrap_or_default();
        let virtual_path = if parent.is_empty() {
            format!("{}/{}", mount.point.name, name)
        } else {
            format!("{parent}/{name}")
        };
        Ok(FileInfo {
            name,
            path: virtual_path,
            size: metadata.len(),
            is_dir: metadata.is_dir(),
            mod_time,
        })
    }
}

/// Whether this process may add, rename and remove entries in `folder`.
pub(crate) fn writable(folder: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt as _;
    let Ok(path) = CString::new(folder.as_os_str().as_bytes()) else { return false };
    // SAFETY: the C path is NUL-terminated and outlives the call.
    unsafe { libc::access(path.as_ptr(), libc::W_OK | libc::X_OK) == 0 }
}

/// The listing of recently used files.
pub const RECENT: &str = "recent";
/// The listing of servers on the local network.
pub const NETWORK: &str = "network";

/// Servers as folders whose paths are the addresses that mount them.
fn network_entries() -> Result<Vec<FileInfo>> {
    Ok(network::browse()?
        .into_iter()
        .map(|server| FileInfo { name: format!("{} ({})", server.name, server.kind), path: server.uri, size: 0, is_dir: true, mod_time: String::new() })
        .collect())
}

fn canonical_directory(path: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path)
        .with_context(|| format!("cannot resolve directory {}", path.display()))?;
    ensure!(canonical.is_dir(), "{} is not a directory", path.display());
    Ok(canonical)
}

fn add_mount(mounts: &mut Vec<Mount>, name: &str, label: Option<&str>, kind: RootKind, path: PathBuf) {
    if mounts.iter().any(|mount| mount.point.name == name || mount.path == path) {
        return;
    }
    mounts.push(Mount {
        point: MountPoint {
            name: name.to_owned(),
            label: label.map(str::to_owned),
            kind,
        },
        path,
    });
}

fn user_directory(home: &Path, key: &str) -> Option<PathBuf> {
    let config = fs::read_to_string(home.join(".config/user-dirs.dirs")).ok()?;
    let raw = config
        .lines()
        .filter_map(|line| line.split_once('='))
        .find_map(|(candidate, value)| (candidate.trim() == key).then_some(value.trim()))?;
    let raw = raw.strip_prefix('"')?.strip_suffix('"')?;
    let relative = raw
        .strip_prefix("$HOME/")
        .or_else(|| raw.strip_prefix("${HOME}/"))?;
    let path = home.join(relative);
    path.is_absolute().then_some(path)
}

fn validate_name(name: &str) -> Result<()> {
    ensure!(!name.is_empty(), "name cannot be empty");
    ensure!(!matches!(name, "." | ".."), "invalid file name {name:?}");
    ensure!(!name.contains(['/', '\0']), "invalid file name {name:?}");
    #[cfg(windows)]
    ensure!(!name.contains('\\'), "invalid file name {name:?}");
    Ok(())
}

fn append_segments(root: &Path, segments: &[&str]) -> PathBuf {
    segments
        .iter()
        .fold(root.to_path_buf(), |path, segment| path.join(segment))
}

fn lexical_absolute(path: &Path) -> Result<PathBuf> {
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => clean.push(prefix.as_os_str()),
            Component::RootDir => clean.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                ensure!(clean.pop(), "path escapes its filesystem root");
            }
            Component::Normal(part) => clean.push(part),
        }
    }
    ensure!(clean.is_absolute(), "path must be absolute");
    Ok(clean)
}

fn parent_of_virtual(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

fn format_time(time: SystemTime) -> String {
    DateTime::<Utc>::from(time).to_rfc3339()
}

fn skip_search_directory(mount: &Mount, path: &Path) -> bool {
    if mount.point.name != "files" || mount.path != Path::new("/") {
        return false;
    }
    ["/proc", "/sys", "/dev", "/run"]
        .iter()
        .any(|prefix| path == Path::new(prefix))
}

fn drive_stats(mount: &Mount) -> Result<DriveStats> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        let path = CString::new(mount.path.as_os_str().as_bytes())?;
        // SAFETY: statvfs initializes the output record and the C path is NUL-terminated.
        let mut raw: libc::statvfs = unsafe { std::mem::zeroed() };
        let result = unsafe { libc::statvfs(path.as_ptr(), &mut raw) };
        if result != 0 {
            return Err(std::io::Error::last_os_error())
                .with_context(|| format!("cannot read filesystem stats for {}", mount.path.display()));
        }
        let block_size = (if raw.f_frsize == 0 { raw.f_bsize } else { raw.f_frsize }) as u64;
        let total_bytes = (raw.f_blocks as u64).saturating_mul(block_size);
        let free_bytes = (raw.f_bavail as u64).saturating_mul(block_size).min(total_bytes);
        let used_bytes = total_bytes.saturating_sub(free_bytes);
        let used_pct = if total_bytes == 0 {
            0.0
        } else {
            used_bytes as f64 * 100.0 / total_bytes as f64
        };
        let read_only = {
            #[cfg(target_os = "linux")]
            {
                raw.f_flag & libc::ST_RDONLY != 0
            }
            #[cfg(not(target_os = "linux"))]
            {
                false
            }
        };
        let (device, fs_type) = mount_details(&mount.path);
        Ok(DriveStats {
            name: mount.point.name.clone(),
            label: mount.point.label.clone(),
            device,
            fs_type,
            total_bytes,
            free_bytes,
            used_bytes,
            used_pct,
            read_only,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = mount;
        bail!("filesystem statistics are unsupported on this platform")
    }
}

#[cfg(target_os = "linux")]
fn mount_details(path: &Path) -> (Option<String>, Option<String>) {
    let Some(mountinfo) = fs::read_to_string("/proc/self/mountinfo").ok() else {
        return (None, None);
    };
    let best = mountinfo
        .lines()
        .filter_map(|line| {
            let (before, after) = line.split_once(" - ")?;
            let fields = before.split_whitespace().collect::<Vec<_>>();
            let tail = after.split_whitespace().collect::<Vec<_>>();
            let mountpoint = fields.get(4).map(|value| unescape_mount_field(value))?;
            let mountpoint = PathBuf::from(mountpoint);
            (path.starts_with(&mountpoint) && !tail.is_empty())
                .then_some((mountpoint.components().count(), tail))
        })
        .max_by_key(|(depth, _)| *depth);
    best.and_then(|(_, fields)| {
        Some((
            fields.get(1).map(|field| (*field).to_owned()),
            fields.first().map(|field| (*field).to_owned()),
        ))
    })
    .unwrap_or((None, None))
}

#[cfg(not(target_os = "linux"))]
fn mount_details(_path: &Path) -> (Option<String>, Option<String>) {
    (None, None)
}

#[cfg(target_os = "linux")]
fn unescape_mount_field(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 3 < bytes.len() {
            let code = &bytes[index + 1..index + 4];
            let decoded = match code {
                b"040" => Some(b' '),
                b"011" => Some(b'\t'),
                b"012" => Some(b'\n'),
                b"134" => Some(b'\\'),
                _ => None,
            };
            if let Some(decoded) = decoded {
                result.push(decoded);
                index += 4;
                continue;
            }
        }
        result.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}
