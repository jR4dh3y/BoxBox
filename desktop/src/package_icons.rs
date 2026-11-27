//! A Linux package's own application icon, as Windows Explorer shows an
//! `.exe`'s: the `.DirIcon` inside an AppImage, or the icon a deb or rpm
//! installs for its desktop entry. Read in process and streamed, never run or
//! unpacked to disk. Reading a large deb or rpm decompresses all of it, so this
//! runs off the UI thread and each file version is read once.

mod appimage;
mod archive;
mod deb;
mod rpm;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

use crate::api::{Api, FileInfo};

#[cfg(test)]
pub(crate) use deb::tests::fixture as deb_fixture;

/// The most an icon file may hold; a larger one is not an icon.
const ICON_LIMIT: u64 = 4 << 20;

/// A local package file at one version: a changed size or time reads it again.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Package {
    /// The virtual path.
    pub path: String,
    pub bytes: u64,
    pub modified: Option<i64>,
}

impl Package {
    /// `item` when it is a package whose icon can be read: a local AppImage, deb or rpm.
    pub fn of(api: &Api, item: &FileInfo) -> Option<Self> {
        (api.is_local() && !item.is_dir && Format::of(&item.name).is_some())
            .then(|| Self { path: item.path.clone(), bytes: item.size, modified: item.mod_time })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    AppImage,
    Deb,
    Rpm,
}

impl Format {
    fn of(name: &str) -> Option<Self> {
        let (_, extension) = name.rsplit_once('.')?;
        [("appimage", Self::AppImage), ("deb", Self::Deb), ("rpm", Self::Rpm)]
            .into_iter()
            .find_map(|(known, format)| extension.eq_ignore_ascii_case(known).then_some(format))
    }
}

/// The icon file's bytes, PNG or SVG. `None` when the package has none.
pub type Icon = Option<Arc<[u8]>>;

type Key = (PathBuf, u64, Option<i64>);

/// Read icons by file version. Waiting on a cell makes a second request for
/// the same file (another zoom step) share the first read instead of repeating it.
#[derive(Default)]
struct Cache {
    cells: HashMap<Key, Arc<OnceLock<Icon>>>,
    /// Bytes held by ready cells; past `CACHE_BUDGET` all are dropped.
    bytes: usize,
}

const CACHE_BUDGET: usize = 32 << 20;

static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(Mutex::default);

fn key(api: &Api, package: &Package) -> Option<Key> {
    let real = api.local_path(&package.path).ok().flatten()?;
    Some((real, package.bytes, package.modified))
}

/// `package`'s icon, read now or by an earlier call. Blocks on the filesystem.
pub fn icon(api: &Api, package: &Package) -> Icon {
    let key = key(api, package)?;
    let format = Format::of(&package.path)?;
    let cell = CACHE.lock().ok()?.cells.entry(key.clone()).or_default().clone();
    let mut made = false;
    let icon = cell
        .get_or_init(|| {
            made = true;
            read(&key.0, format, crate::file_icons::LARGEST_BUCKET)
        })
        .clone();
    if made {
        let mut cache = CACHE.lock().ok()?;
        cache.bytes += icon.as_ref().map_or(0, |bytes| bytes.len());
        if cache.bytes > CACHE_BUDGET {
            *cache = Cache::default();
        }
    }
    icon
}

impl Package {
    /// A deb or rpm: reading one decompresses all of it, seconds for a large one.
    pub fn is_slow(&self) -> bool {
        matches!(Format::of(&self.path), Some(Format::Deb | Format::Rpm))
    }
}

type Job = Box<dyn FnOnce() + Send>;

/// `icon` on a thread of its own, one package at a time. A large deb takes
/// seconds; on GPUI's shared pool a few of them would hold every worker and
/// stall listings and menus behind them.
pub async fn queued(api: Arc<Api>, package: Package) -> Icon {
    static QUEUE: OnceLock<Mutex<std::sync::mpsc::Sender<Job>>> = OnceLock::new();
    let queue = QUEUE.get_or_init(|| {
        let (sender, jobs) = std::sync::mpsc::channel::<Job>();
        let started = std::thread::Builder::new().name("package icons".into()).spawn(move || {
            for job in jobs {
                job();
            }
        });
        if let Err(error) = started {
            log::warn!("package icons thread: {error}");
        }
        Mutex::new(sender)
    });
    let (reply, answer) = async_channel::bounded(1);
    let job: Job = Box::new(move || {
        let _ = reply.send_blocking(icon(&api, &package));
    });
    queue.lock().ok()?.send(job).ok()?;
    answer.recv().await.ok().flatten()
}

/// `package`'s icon when an earlier call has read it, without waiting.
pub fn ready(api: &Api, package: &Package) -> Icon {
    let key = key(api, package)?;
    CACHE.lock().ok()?.cells.get(&key)?.get()?.clone()
}

/// Picks the icon for `size` pixels, the largest the views draw, so one read serves every zoom.
fn read(path: &Path, format: Format, size: u32) -> Icon {
    let result = match format {
        Format::AppImage => appimage::icon(path, size),
        Format::Deb => archive::icon(|visit| deb::walk(path, visit), size),
        Format::Rpm => archive::icon(|visit| rpm::walk(path, visit), size),
    };
    match result {
        Ok(Some(bytes)) => Some(bytes.into()),
        Ok(None) => {
            log::debug!("{}: no icon in the package", path.display());
            None
        }
        Err(error) => {
            log::debug!("{}: {error:#}", path.display());
            None
        }
    }
}

/// `target` as seen from the folder `dir`, both relative to the package root,
/// with `.` and `..` folded. An absolute target starts at the root.
fn resolve(dir: &str, target: &str) -> String {
    let mut parts: Vec<&str> = match target.starts_with('/') {
        true => Vec::new(),
        false => dir.split('/').filter(|part| !part.is_empty()).collect(),
    };
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Files under `dir`, `depth` folders deep, with `extension` in any case.
    pub(super) fn find(dir: &Path, extension: &str, depth: usize) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
        let mut found = Vec::new();
        for path in entries.flatten().map(|entry| entry.path()) {
            if path.is_dir() && depth > 0 {
                found.extend(find(&path, extension, depth - 1));
            } else if path.extension().is_some_and(|found| found.eq_ignore_ascii_case(extension)) {
                found.push(path);
            }
        }
        found
    }

    /// Prints what a real package gave, and checks it draws.
    pub(super) fn report(path: &Path, icon: anyhow::Result<Option<Vec<u8>>>, started: std::time::Instant) {
        let took = started.elapsed();
        let size = std::fs::metadata(path).map_or(0, |metadata| metadata.len() >> 20);
        let drawn = match &icon {
            Ok(Some(bytes)) => {
                let natural = image::load_from_memory(bytes).map(|image| format!("{}x{} raster", image.width(), image.height())).unwrap_or_else(|_| "svg".into());
                let image = crate::file_icons::package_image(bytes, 256).expect("the icon draws");
                format!("{natural}, {} bytes, drawn {}x{}", bytes.len(), image.size(0).width.0, image.size(0).height.0)
            }
            Ok(None) => "no icon".into(),
            Err(error) => format!("error: {error:#}"),
        };
        println!("{} ({size} MiB): {drawn} in {took:?}", path.display());
    }

    #[test]
    fn formats_come_from_the_extension() {
        assert_eq!(Format::of("Krita-5.2.AppImage"), Some(Format::AppImage));
        assert_eq!(Format::of("x.appimage"), Some(Format::AppImage));
        assert_eq!(Format::of("discord-0.0.113.deb"), Some(Format::Deb));
        assert_eq!(Format::of("a.x86_64.RPM"), Some(Format::Rpm));
        assert_eq!(Format::of("deb"), None);
        assert_eq!(Format::of("notes.txt"), None);
    }

    #[test]
    fn links_resolve_inside_the_package() {
        assert_eq!(resolve("usr/share/icons/hicolor/256x256/apps", "../../../../pixmaps/a.png"), "usr/share/pixmaps/a.png");
        assert_eq!(resolve("usr/share/pixmaps", "/opt/App/icon.png"), "opt/App/icon.png");
        assert_eq!(resolve("", "./app.svg"), "app.svg");
        assert_eq!(resolve("", "../../../etc/passwd"), "etc/passwd");
    }

    #[test]
    fn a_package_reads_once_per_version() {
        let root = std::env::temp_dir().join(format!("boxbox-package-icons-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("app.deb"), deb::tests::fixture(&[("./usr/share/applications/app.desktop", "[Desktop Entry]\nIcon=app\n".as_bytes())], &[])).unwrap();
        let api = Api::local_with_root(root.clone()).unwrap();
        let item = api.stat("home/app.deb").unwrap();
        let package = Package::of(&api, &item).unwrap();
        assert_eq!(ready(&api, &package), None);
        assert_eq!(icon(&api, &package), None);
        let key = key(&api, &package).unwrap();
        assert!(CACHE.lock().unwrap().cells.get(&key).is_some_and(|cell| cell.get().is_some()));
        assert!(Package::of(&api, &FileInfo { is_dir: true, ..item }).is_none());
        std::fs::remove_dir_all(root).ok();
    }
}
