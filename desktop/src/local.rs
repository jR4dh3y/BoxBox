//! Direct local filesystem implementation for the GPUI desktop client.

mod archive;
mod copy;
#[cfg(target_os = "linux")]
mod network;
#[cfg(target_os = "linux")]
mod recent;
mod search;
mod trash;
pub(crate) mod mime;
#[cfg_attr(windows, path = "windows_thumbnails.rs")]
pub mod thumbnails;
#[cfg(target_os = "linux")]
mod thumbnailers;

#[cfg(unix)]
use std::ffi::CString;
use std::borrow::Cow;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, ensure, Context as _, Result};

use crate::api::{DriveStats, FileInfo, MountPoint, RootKind, TrashItem};

#[derive(Clone)]
pub(super) struct Mount {
    pub(super) point: MountPoint,
    pub(super) path: PathBuf,
}

/// Local files exposed through the same virtual paths as the remote file API.
pub struct LocalFiles {
    pub(super) mounts: Vec<Mount>,
    pub(super) trash_data_home: PathBuf,
    pub(super) volume_trash: bool,
    #[cfg(windows)]
    pub(super) host_filesystem: bool,
}

impl LocalFiles {
    /// Open the host filesystem, or use BOXBOX_ROOT as an isolated local root.
    pub fn new() -> Result<Self> {
        if let Some(root) = std::env::var_os("BOXBOX_ROOT") {
            return Self::with_root(root.into());
        }

        #[cfg(windows)]
        return crate::windows::local_files();

        #[cfg(target_os = "linux")]
        Self::linux()
    }

    #[cfg(target_os = "linux")]
    fn linux() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is not set")?;
        let home = canonical_directory(&home).context("cannot open the home directory")?;
        let filesystem = canonical_directory(Path::new("/"))?;
        let mut mounts = vec![Mount {
            point: MountPoint {
                name: "files".into(),
                label: Some("System".into()),
                kind: RootKind::Drive,
            },
            path: filesystem,
        }];
        // Home is a folder on the system drive, not a drive of its own.
        add_mount(&mut mounts, "home", Some("Home"), RootKind::Place, home.clone());
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
        let mut mounts = vec![Mount {
            point: MountPoint {
                name: "home".into(),
                label: Some("Home".into()),
                kind: RootKind::Drive,
            },
            path: root.clone(),
        }];
        // A fixture's own Desktop and Templates folders stand in for the user's.
        for (name, label) in [("desktop", "Desktop"), ("templates", "Templates")] {
            if let Ok(folder) = canonical_directory(&root.join(label)) {
                add_mount(&mut mounts, name, Some(label), RootKind::Place, folder);
            }
        }
        Ok(Self {
            mounts,
            trash_data_home: root.join(".local/share"),
            volume_trash: false,
            #[cfg(windows)]
            host_filesystem: false,
        })
    }

    fn mount(&self, name: &str) -> Option<Cow<'_, Mount>> {
        if let Some(mount) = self.mounts.iter().find(|mount| mount.point.name == name) {
            #[cfg(windows)]
            if self.host_filesystem && mount.point.kind == RootKind::Drive &&
                !crate::windows::has_drive(name, crate::windows::drive_mask()) {
                return None;
            }
            return Some(Cow::Borrowed(mount));
        }
        #[cfg(windows)]
        if self.host_filesystem {
            return crate::windows::additional_mount(name).map(Cow::Owned);
        }
        None
    }

    #[cfg(windows)]
    fn current_mounts(&self) -> Vec<Mount> {
        if !self.host_filesystem {
            return self.mounts.clone();
        }
        let mut current = crate::windows::drive_mounts();
        current.extend(self.mounts.iter().filter(|mount| mount.point.kind == RootKind::Place).cloned());
        current
    }

    /// The mounts, with Home then Recent first among the places and Network last.
    /// Recent and Network are listings, not folders.
    pub fn roots(&self) -> Result<Vec<MountPoint>> {
        let place = |name: &str, label: &str| MountPoint { name: name.into(), label: Some(label.into()), kind: RootKind::Place };
        #[cfg(windows)]
        let mounted = self.current_mounts();
        #[cfg(not(windows))]
        let mounted = &self.mounts;
        let (drives, places): (Vec<_>, Vec<_>) = mounted.iter().map(|mount| mount.point.clone()).partition(|point| point.kind == RootKind::Drive);
        let (home, places): (Vec<_>, Vec<_>) = places.into_iter().partition(|point| point.name == "home");
        let roots = drives.into_iter().chain(home);
        #[cfg(target_os = "linux")]
        let roots = roots.chain([place(RECENT, "Recent")]).chain(places).chain([place(NETWORK, "Network")]);
        #[cfg(windows)]
        let roots = roots.chain(self.host_filesystem.then(|| place(RECENT, "Recent"))).chain(places)
            .chain(self.host_filesystem.then(|| place(NETWORK, "Network")));
        Ok(roots.collect())
    }

    pub fn drives(&self) -> Result<Vec<DriveStats>> {
        #[cfg(windows)]
        let mounted = self.current_mounts();
        #[cfg(not(windows))]
        let mounted = &self.mounts;
        mounted
            .iter()
            .filter(|mount| mount.point.kind != RootKind::Place)
            .filter_map(|mount| match drive_stats(mount) {
                #[cfg(windows)]
                Err(error) => {
                    log::debug!("Could not read {}: {error:#}", mount.path.display());
                    None
                }
                result => Some(result),
            })
            .collect()
    }

    pub fn list(&self, path: &str) -> Result<Vec<FileInfo>> {
        match path {
            RECENT => return Ok(self.recent()),
            #[cfg(target_os = "linux")]
            NETWORK => return network_entries(),
            #[cfg(windows)]
            NETWORK if self.host_filesystem => return Ok(crate::windows::drive_mounts().into_iter()
                .filter(|mount| crate::windows::is_network_drive(&mount.path))
                .filter_map(|mount| self.stat(&mount.point.name).ok()).collect()),
            _ => {}
        }
        let (mount, directory) = self.resolve_directory(path)?;
        let entries = fs::read_dir(&directory)
            .with_context(|| format!("cannot list {}", path))?
            .map(|entry| entry.map_err(anyhow::Error::from))
            .collect::<Result<Vec<_>>>()?;
        let mut items = map_entries(&entries, |entry| self.info_for_entry(&mount, path, entry))
            .into_iter()
            // Deleted since the folder was read, as happens in a busy Downloads. A name that is
            // not UTF-8 has no virtual path to reach it, so it is left out, not the folder.
            .filter(|result| !matches!(result, Err(error) if is_not_found(error) || error.is::<NotUtf8Name>()))
            .collect::<Result<Vec<_>>>()?;
        // By the names already read: `DirEntry::file_name` allocates on every call.
        items.sort_unstable_by(|left, right| left.name.cmp(&right.name));
        Ok(items)
    }

    pub fn create_directory(&self, parent: &str, name: &str) -> Result<FileInfo> {
        validate_name(name)?;
        let (mount, directory) = self.resolve_directory(parent)?;
        let target = directory.join(name);
        self.ensure_mutable_target(&target, true)?;
        fs::create_dir(&target).with_context(|| format!("cannot create directory {name:?}"))?;
        self.info_for_path(&mount, parent, &target)
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
        self.info_for_path(&destination_mount, parent_of_virtual(new_path), &destination)
    }

    /// Permanently delete one entry. `recursive` must be true to remove a real directory tree.
    pub fn delete(&self, path: &str, recursive: bool) -> Result<()> {
        let (_, entry) = self.resolve_entry(path)?;
        let metadata = fs::symlink_metadata(&entry)
            .with_context(|| format!("cannot inspect {path}"))?;
        let is_tree = metadata.is_dir();
        ensure!(!is_tree || recursive, "directory deletion requires confirmation");
        self.ensure_mutable_target(&entry, is_tree)?;
        copy::remove_entry(&entry).with_context(|| format!("cannot delete {path}"))
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
        if let Some(mount) = self.mount(path) {
            let metadata = fs::metadata(&mount.path).with_context(|| format!("cannot inspect {path}"))?;
            let mod_time = metadata.modified().ok().and_then(crate::api::mod_time);
            return Ok(FileInfo { name: mount.point.display_name().to_string(), path: path.to_string(), size: metadata.len(), is_dir: true, hidden: false, mod_time });
        }
        let (mount, real) = self.resolve_entry(path)?;
        let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
        self.info_for_path(&mount, parent, &real).with_context(|| format!("cannot inspect {path}"))
    }

    /// The virtual path for a real one, under the mount that holds it most closely.
    pub fn virtual_path(&self, real: &Path) -> Option<String> {
        #[cfg(windows)]
        let canonical = fs::canonicalize(real).ok()?;
        #[cfg(windows)]
        let real = canonical.as_path();
        let mount = self.mounts.iter().filter(|mount| real.starts_with(&mount.path)).max_by_key(|mount| mount.path.components().count()).map(Cow::Borrowed);
        #[cfg(windows)]
        let mount = mount.or_else(|| {
            if !self.host_filesystem { return None; }
            self.mount(&crate::windows::mount_name(real)?)
        });
        let mount = mount?;
        let relative = real.strip_prefix(&mount.path).ok()?;
        let mut path = mount.point.name.clone();
        for part in relative.components() {
            path.push('/');
            path.push_str(&part.as_os_str().to_string_lossy());
        }
        Some(path)
    }

    /// The folder that holds place `name`, such as `home` for Downloads, by the
    /// path Home or a drive gives it. `None` for Home, a drive or a listing.
    pub fn place_parent(&self, name: &str) -> Option<String> {
        let place = self.mounts.iter().find(|mount| mount.point.name == name && mount.point.kind == RootKind::Place && name != "home")?;
        let parent = place.path.parent()?;
        let holder = self
            .mounts
            .iter()
            .filter(|mount| mount.point.name == "home" || mount.point.kind == RootKind::Drive)
            .filter(|mount| parent.starts_with(&mount.path))
            .max_by_key(|mount| mount.path.components().count())?;
        let mut path = holder.point.name.clone();
        for part in parent.strip_prefix(&holder.path).ok()?.components() {
            path.push('/');
            path.push_str(&part.as_os_str().to_string_lossy());
        }
        Some(path)
    }

    /// Like `virtual_path`, but a final link or junction names itself, not its
    /// target, so moving the entry moves the link. Only Windows resolves links there.
    pub fn virtual_entry_path(&self, real: &Path) -> Option<String> {
        #[cfg(windows)]
        if let (Some(parent), Some(name)) = (real.parent(), real.file_name()) {
            return Some(format!("{}/{}", self.virtual_path(parent)?, name.to_string_lossy()));
        }
        self.virtual_path(real)
    }

    /// The home folder and the XDG user folders by every virtual path that
    /// reaches them, with the theme icon each one has.
    pub fn special_folders(&self) -> Vec<(String, &'static str)> {
        let icon = |name: &str| match name {
            "home" => Some("user-home"),
            "desktop" => Some("user-desktop"),
            "documents" => Some("folder-documents"),
            "downloads" => Some("folder-download"),
            "music" => Some("folder-music"),
            "pictures" => Some("folder-pictures"),
            "videos" => Some("folder-videos"),
            "templates" => Some("folder-templates"),
            _ => None,
        };
        let mut places: Vec<(&Path, &'static str)> =
            self.mounts.iter().filter_map(|mount| Some((mount.path.as_path(), icon(&mount.point.name)?))).collect();
        let home = self.mounts.iter().find(|mount| mount.point.name == "home").map(|mount| mount.path.clone());
        let public = home.as_ref().and_then(|home| user_directory(home, "XDG_PUBLICSHARE_DIR").or_else(|| Some(home.join("Public"))));
        if let Some(public) = &public {
            places.push((public, "folder-publicshare"));
        }
        // Each place under each mount: `home/Downloads`, `files/home/me/Downloads`, `downloads`.
        let mut found = Vec::new();
        for mount in &self.mounts {
            for (path, icon) in &places {
                if let Ok(rest) = path.strip_prefix(&mount.path) {
                    let rest = rest.iter().map(|part| part.to_string_lossy()).collect::<Vec<_>>().join("/");
                    found.push((if rest.is_empty() { mount.point.name.clone() } else { format!("{}/{rest}", mount.point.name) }, *icon));
                }
            }
        }
        found
    }

    pub fn local_path(&self, path: &str) -> Result<PathBuf> {
        let (mount, candidate) = self.candidate_path(path)?;
        let resolved = fs::canonicalize(&candidate)
            .with_context(|| format!("cannot resolve {path}"))?;
        ensure!(self.exposes(&mount, &resolved), "{path} resolves outside the folders BoxBox shows");
        Ok(resolved)
    }

    /// The real path `path` names, without following links or touching the disk.
    pub fn lexical_path(&self, path: &str) -> Option<PathBuf> {
        self.candidate_path(path).ok().map(|(_, real)| real)
    }

    /// Whether `real`, reached through `mount`, lies in a root this provider
    /// shows. On the host, System holds every path, so a link in Home to
    /// `/mnt/media` opens; on Windows any present drive does, including one
    /// attached after launch. A `BOXBOX_ROOT` fixture stays inside its one root.
    fn exposes(&self, mount: &Mount, real: &Path) -> bool {
        if real.starts_with(&mount.path) || self.mounts.iter().any(|shown| real.starts_with(&shown.path)) {
            return true;
        }
        #[cfg(windows)]
        if self.host_filesystem {
            return crate::windows::mount_name(real).is_some();
        }
        false
    }

    /// The entry itself: its parent is resolved, but a final link is not followed.
    pub fn entry_path(&self, path: &str) -> Result<PathBuf> {
        if !path.contains('/') {
            return self.local_path(path);
        }
        Ok(self.resolve_entry(path)?.1)
    }

    pub fn create_file(&self, parent: &str, name: &str, content: &str) -> Result<FileInfo> {
        validate_name(name)?;
        let (mount, directory) = self.resolve_directory(parent)?;
        let target = directory.join(name);
        self.ensure_mutable_target(&target, false)?;
        copy::write_new_file(&target, content.as_bytes())?;
        self.info_for_path(&mount, parent, &target)
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

    /// A ZIP at `destination` holding the entries at `paths`.
    pub fn compress(&self, paths: &[String], destination: &str) -> Result<()> {
        let sources = paths
            .iter()
            .map(|path| self.resolve_entry(path).map(|(mount, entry)| (entry, mount.path.clone())))
            .collect::<Result<Vec<_>>>()?;
        let (_, parent, name) = self.resolve_destination(destination)?;
        archive::write_zip_of(&sources, &parent.join(name))
    }

    /// A symbolic link at `link` to the entry at `target`, in the same folder, so
    /// the link holds just the entry's name and survives the folder moving.
    pub fn create_link(&self, target: &str, link: &str) -> Result<()> {
        let (_, entry) = self.resolve_entry(target)?;
        let (_, parent, name) = self.resolve_destination(link)?;
        // Beside its target a link holds just the name, so the pair can move together;
        // elsewhere it holds the full path, as Thunar's do.
        let points_to = match entry.parent() == Some(parent.as_path()) {
            true => PathBuf::from(entry.file_name().context("the target has no name")?),
            false => entry.clone(),
        };
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(points_to, parent.join(name));
        #[cfg(windows)]
        let result = if entry.is_dir() {
            std::os::windows::fs::symlink_dir(points_to, parent.join(name))
        } else {
            std::os::windows::fs::symlink_file(points_to, parent.join(name))
        };
        result.with_context(|| format!("cannot create a link to {target}"))
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
        self.info_for_path(&mount, parent, &target)
    }

    pub fn trash(&self) -> Result<Vec<TrashItem>> {
        let items = trash::list(&self.mounts, &self.trash_data_home, self.volume_trash)?;
        #[cfg(windows)]
        if self.host_filesystem {
            let mut combined = items;
            combined.extend(crate::windows::recycle_bin::list()?);
            combined.sort_by(|left, right| right.deleted_at.cmp(&left.deleted_at));
            return Ok(combined);
        }
        Ok(items)
    }

    pub fn move_to_trash(&self, path: &str) -> Result<TrashItem> {
        let (_, entry) = self.resolve_entry(path)?;
        let metadata = fs::symlink_metadata(&entry)
            .with_context(|| format!("cannot inspect {path}"))?;
        self.ensure_mutable_target(&entry, metadata.is_dir())?;
        #[cfg(windows)]
        if self.host_filesystem {
            return crate::windows::recycle_bin::recycle(&entry);
        }
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
        #[cfg(target_os = "linux")]
        let recent = recent::files(&self.trash_data_home, 200);
        #[cfg(windows)]
        let recent = if self.host_filesystem { crate::windows::recent::files(200) } else { Vec::new() };
        recent.into_iter()
            .filter_map(|real| self.stat(&self.virtual_path(&real)?).ok())
            .collect()
    }

    pub fn trash_has_items(&self) -> bool {
        #[cfg(windows)]
        if self.host_filesystem && crate::windows::recycle_bin::has_items() {
            return true;
        }
        trash::has_items(&self.trash_data_home, self.volume_trash)
    }

    pub fn empty_trash(&self) -> Result<usize> {
        #[cfg(windows)]
        let recycled = if self.host_filesystem { crate::windows::recycle_bin::purge(None)? } else { 0 };
        #[cfg(not(windows))]
        let recycled = 0;
        Ok(trash::empty(&self.mounts, &self.trash_data_home, self.volume_trash)? + recycled)
    }

    /// The home Trash's `files` folder, which changes whenever Trash does.
    pub fn trash_files(&self) -> PathBuf {
        self.trash_data_home.join("Trash/files")
    }

    pub fn restore_trash(&self, id: &str) -> Result<()> {
        #[cfg(windows)]
        if self.host_filesystem && id.starts_with("win:") {
            return crate::windows::recycle_bin::restore(id);
        }
        trash::restore(&self.mounts, &self.trash_data_home, self.volume_trash, id)
    }

    pub fn delete_trash(&self, id: &str) -> Result<()> {
        #[cfg(windows)]
        if self.host_filesystem && id.starts_with("win:") {
            return crate::windows::recycle_bin::purge(Some(id)).map(|_| ());
        }
        trash::delete(&self.mounts, &self.trash_data_home, self.volume_trash, id)
    }

    fn resolve_directory(&self, path: &str) -> Result<(Cow<'_, Mount>, PathBuf)> {
        let (mount, candidate) = self.candidate_path(path)?;
        let directory = fs::canonicalize(&candidate)
            .with_context(|| format!("cannot resolve directory {path}"))?;
        ensure!(self.exposes(&mount, &directory), "{path} resolves outside the folders BoxBox shows");
        ensure!(directory.is_dir(), "{path} is not a directory");
        Ok((mount, directory))
    }

    fn resolve_entry(&self, path: &str) -> Result<(Cow<'_, Mount>, PathBuf)> {
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

    fn resolve_destination(&self, path: &str) -> Result<(Cow<'_, Mount>, PathBuf, String)> {
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

    fn candidate_path(&self, path: &str) -> Result<(Cow<'_, Mount>, PathBuf)> {
        let (mount, segments) = self.parse_virtual_path(path)?;
        let candidate = append_segments(&mount.path, &segments);
        Ok((mount, candidate))
    }

    fn parse_virtual_path<'path>(
        &self,
        path: &'path str,
    ) -> Result<(Cow<'_, Mount>, Vec<&'path str>)> {
        ensure!(!path.is_empty(), "empty virtual path");
        ensure!(!path.starts_with('/') && !path.ends_with('/'), "malformed virtual path {path:?}");
        ensure!(!path.contains('\0'), "malformed virtual path {path:?}");
        let mut segments = path.split('/');
        let root_name = segments.next().context("empty virtual path")?;
        let mount = self.mount(root_name)
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

}

/// An entry whose name is not UTF-8, which a virtual path cannot name.
#[derive(Debug)]
struct NotUtf8Name;

impl std::fmt::Display for NotUtf8Name {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("file name is not valid UTF-8")
    }
}

impl std::error::Error for NotUtf8Name {}

impl LocalFiles {
    /// Stats relative to the open folder, which skips resolving the whole path
    /// again: 26 ms instead of 37 ms for 20,000 entries.
    fn info_for_entry(&self, mount: &Mount, parent: &str, entry: &fs::DirEntry) -> Result<FileInfo> {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            log::info!("{} is left out: its name is not UTF-8", path.display());
            return Err(NotUtf8Name.into());
        };
        let metadata = entry.metadata().with_context(|| format!("cannot inspect entry {name:?}"))?;
        self.info_from_metadata(mount, parent, &path, metadata)
            .with_context(|| format!("cannot inspect entry {name:?}"))
    }

    fn info_for_path(&self, mount: &Mount, parent: &str, path: &Path) -> Result<FileInfo> {
        self.info_from_metadata(mount, parent, path, fs::symlink_metadata(path)?)
    }

    /// `entry_metadata` describes the entry itself; a link inside the mount reports its target.
    fn info_from_metadata(&self, mount: &Mount, parent: &str, path: &Path, entry_metadata: fs::Metadata) -> Result<FileInfo> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("file name is not valid UTF-8")?
            .to_owned();
        let metadata = if entry_metadata.file_type().is_symlink() {
            match fs::canonicalize(path) {
                Ok(target) if target.starts_with(&mount.path) => fs::metadata(target)?,
                _ => entry_metadata,
            }
        } else {
            entry_metadata
        };
        let mod_time = metadata.modified().ok().and_then(crate::api::mod_time);
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
            hidden: hidden_attribute(&metadata),
            mod_time,
        })
    }
}

/// Native hidden flags supplement dot-prefixed names.
#[cfg(windows)]
fn hidden_attribute(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_SYSTEM};
    metadata.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
}

#[cfg(not(windows))]
fn hidden_attribute(_: &fs::Metadata) -> bool {
    false
}

/// Whether this process may add, rename and remove entries in `folder`.
#[cfg(unix)]
pub(crate) fn writable(folder: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt as _;
    let Ok(path) = CString::new(folder.as_os_str().as_bytes()) else { return false };
    // SAFETY: the C path is NUL-terminated and outlives the call.
    unsafe { libc::access(path.as_ptr(), libc::W_OK | libc::X_OK) == 0 }
}

#[cfg(windows)]
pub(crate) use crate::windows::writable;

/// A readable location for Properties and Trash.
pub(crate) fn display_location(path: &Path) -> String {
    #[cfg(windows)]
    let plain = crate::windows::shell_path_buf(path);
    #[cfg(windows)]
    let path = plain.as_path();
    let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let home = std::env::var_os(home_var).filter(|value| !value.is_empty());
    match home.as_deref().and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~{}{rest}", std::path::MAIN_SEPARATOR, rest = rest.display()),
        None => path.display().to_string(),
    }
}

/// Folders this large read their entries on several threads.
const PARALLEL_ENTRIES: usize = 1024;
const ENTRY_THREADS: usize = 4;

/// One stat per entry dominates a large folder; spread over threads, 20,000 entries
/// took 7 ms instead of 25 ms. A thread costs more than a small folder gains. Each
/// thread builds the final values, so no per-entry buffer outlives the listing.
fn map_entries<T: Send>(entries: &[fs::DirEntry], read: impl Fn(&fs::DirEntry) -> T + Sync) -> Vec<T> {
    let threads = std::thread::available_parallelism().map_or(1, |count| count.get().min(ENTRY_THREADS));
    if entries.len() < PARALLEL_ENTRIES || threads < 2 {
        return entries.iter().map(read).collect();
    }
    let chunk = entries.len().div_ceil(threads);
    let read = &read;
    std::thread::scope(|scope| {
        let workers: Vec<_> = entries
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(read).collect::<Vec<_>>()))
            .collect();
        workers.into_iter().flat_map(|worker| worker.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic))).collect()
    })
}

/// The listing of recently used files.
pub const RECENT: &str = "recent";
/// The listing of servers on the local network.
pub const NETWORK: &str = "network";

/// Servers as folders whose paths are the addresses that mount them.
#[cfg(target_os = "linux")]
fn network_entries() -> Result<Vec<FileInfo>> {
    Ok(network::browse()?
        .into_iter()
        .map(|server| FileInfo { name: format!("{} ({})", server.name, server.kind), path: server.uri, size: 0, is_dir: true, hidden: false, mod_time: None })
        .collect())
}

pub(super) fn canonical_directory(path: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path)
        .with_context(|| format!("cannot resolve directory {}", path.display()))?;
    ensure!(canonical.is_dir(), "{} is not a directory", path.display());
    Ok(canonical)
}

pub(super) fn add_mount(mounts: &mut Vec<Mount>, name: &str, label: Option<&str>, kind: RootKind, path: PathBuf) {
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
    crate::windows::validate_name(name)?;
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

fn is_not_found(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| cause.downcast_ref::<std::io::Error>().is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound))
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
    #[cfg(windows)]
    crate::windows::drive_stats(mount)
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

#[cfg(all(unix, not(target_os = "linux")))]
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
