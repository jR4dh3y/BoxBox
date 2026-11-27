//! Finds icons in the desktop's icon theme, as GTK does: the theme the settings
//! portal reports, the themes it inherits, then hicolor and pixmaps
//! (freedesktop Icon Theme Specification). Each theme's folders are listed once
//! into an index, so a lookup touches no disk. Building it blocks on D-Bus and
//! the filesystem, so it runs off the UI thread.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
#[cfg(target_os = "linux")]
use std::time::Duration;

/// The size an app icon is looked up at; Open With draws them at 24px, on HiDPI at 48.
const APP_SIZE: u32 = 48;

pub struct IconTheme {
    /// Every icon folder of every theme in the chain: (theme index, folder).
    dirs: Vec<(usize, Dir)>,
    /// Icon name → (index into `dirs`, is SVG), in no order.
    names: HashMap<String, Vec<(usize, bool)>>,
    pixmaps: Vec<PathBuf>,
}

#[derive(Clone)]
struct Dir {
    path: PathBuf,
    size: u32,
    min: u32,
    max: u32,
    scale: u32,
}

impl Dir {
    /// How far the folder's icons are from `size`: 0 when they cover it.
    fn distance(&self, size: u32) -> u32 {
        if self.min <= size && size <= self.max {
            0
        } else if size < self.min {
            self.min - size
        } else {
            size - self.max
        }
    }
}

impl IconTheme {
    /// The desktop's theme, read once per run.
    pub fn shared() -> Arc<IconTheme> {
        static THEME: OnceLock<Arc<IconTheme>> = OnceLock::new();
        THEME.get_or_init(|| Arc::new(Self::load(current_name().as_deref(), &data_dirs(), &home().join(".icons")))).clone()
    }

    /// `name` and its parents in `~/.icons` and each data dir's `icons`.
    fn load(name: Option<&str>, data: &[PathBuf], dot_icons: &Path) -> Self {
        let _span = crate::profile::span("icon theme index");
        let mut bases = vec![dot_icons.to_path_buf()];
        bases.extend(data.iter().map(|dir| dir.join("icons")));
        // (theme, its index.theme), the theme first and hicolor last.
        let mut chain: Vec<(String, String)> = Vec::new();
        let mut queue: Vec<String> = name.into_iter().map(str::to_string).collect();
        queue.insert(0, "hicolor".into());
        while let Some(theme) = queue.pop() {
            if chain.iter().any(|(seen, _)| *seen == theme) || (theme == "hicolor" && !queue.is_empty()) {
                continue;
            }
            let Some(text) = bases.iter().find_map(|base| std::fs::read_to_string(base.join(&theme).join("index.theme")).ok()) else { continue };
            // Parents are searched in order, depth first, so push them reversed.
            let parents = Ini::parse(&text).get("Icon Theme", "Inherits").to_string();
            queue.extend(parents.split(',').map(str::trim).filter(|parent| !parent.is_empty() && *parent != "hicolor").rev().map(str::to_string));
            chain.push((theme, text));
        }
        let mut dirs = Vec::new();
        let mut names: HashMap<String, Vec<(usize, bool)>> = HashMap::new();
        for (index, (theme, text)) in chain.iter().enumerate() {
            for dir in theme_dirs(&Ini::parse(text)) {
                for base in &bases {
                    let path = base.join(theme).join(&dir.path);
                    let Ok(entries) = std::fs::read_dir(&path) else { continue };
                    let at = dirs.len();
                    for entry in entries.flatten() {
                        let file = entry.file_name();
                        let Some(file) = file.to_str() else { continue };
                        let (stem, svg) = match file.rsplit_once('.') {
                            Some((stem, "svg")) => (stem, true),
                            Some((stem, "png")) => (stem, false),
                            _ => continue,
                        };
                        names.entry(stem.to_string()).or_default().push((at, svg));
                    }
                    dirs.push((index, Dir { path, ..dir.clone() }));
                }
            }
        }
        Self { dirs, names, pixmaps: data.iter().map(|dir| dir.join("pixmaps")).collect() }
    }

    /// The file for icon `name` drawn at `size` pixels: from the first theme of
    /// the chain that has it, the folder nearest that size, an SVG on a tie.
    pub fn find_sized(&self, name: &str, size: u32) -> Option<PathBuf> {
        let candidates = self.names.get(name)?;
        let (dir, svg) = candidates.iter().copied().min_by_key(|(dir, svg)| {
            let (theme, folder) = &self.dirs[*dir];
            (*theme, folder.distance(size), folder.scale != 1, !svg, folder.size.abs_diff(size))
        })?;
        Some(self.dirs[dir].1.path.join(format!("{name}.{}", if svg { "svg" } else { "png" })))
    }

    /// The first of `names` the theme has, at `size`.
    pub fn find_first(&self, names: &[impl AsRef<str>], size: u32) -> Option<PathBuf> {
        names.iter().find_map(|name| self.find_sized(name.as_ref(), size))
    }

    /// A desktop entry's `Icon`: an absolute path, or a name to look up.
    pub fn find(&self, icon: &str) -> Option<PathBuf> {
        if icon.is_empty() {
            return None;
        }
        let as_path = Path::new(icon);
        if as_path.is_absolute() {
            return as_path.is_file().then(|| as_path.to_path_buf());
        }
        self.find_sized(icon, APP_SIZE).or_else(|| {
            self.pixmaps.iter().find_map(|dir| ["png", "svg"].iter().map(|extension| dir.join(format!("{icon}.{extension}"))).find(|path| path.is_file()))
        })
    }
}

/// The folders of a theme's `index.theme`, except symbolic ones: those are
/// single-colour glyphs meant to be recoloured.
fn theme_dirs(index: &Ini) -> Vec<Dir> {
    let listed = index.get("Icon Theme", "Directories").to_string() + "," + index.get("Icon Theme", "ScaledDirectories");
    listed
        .split(',')
        .map(str::trim)
        .filter(|path| !path.is_empty() && !path.contains("symbolic"))
        .filter_map(|path| {
            let number = |name: &str| index.get(path, name).parse::<u32>().ok();
            let size = number("Size")?;
            let threshold = number("Threshold").unwrap_or(2);
            let (min, max) = match index.get(path, "Type") {
                "Fixed" => (size, size),
                "Scalable" => (number("MinSize").unwrap_or(size), number("MaxSize").unwrap_or(size)),
                _ => (size.saturating_sub(threshold), size + threshold),
            };
            Some(Dir { path: PathBuf::from(path), size, min, max, scale: number("Scale").unwrap_or(1) })
        })
        .collect()
}

/// A desktop-style file, parsed once: hicolor's index has hundreds of sections.
struct Ini<'a>(HashMap<&'a str, HashMap<&'a str, &'a str>>);

impl<'a> Ini<'a> {
    fn parse(text: &'a str) -> Self {
        let mut sections: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
        let mut section = "";
        for line in text.lines().map(str::trim) {
            if let Some(name) = line.strip_prefix('[').and_then(|line| line.strip_suffix(']')) {
                section = name;
            } else if let Some((key, value)) = line.split_once('=').filter(|_| !line.starts_with('#')) {
                sections.entry(section).or_default().entry(key.trim()).or_insert(value.trim());
            }
        }
        Self(sections)
    }

    /// `key` in `[section]`, or "".
    fn get(&self, section: &str, key: &str) -> &'a str {
        self.0.get(section).and_then(|keys| keys.get(key)).copied().unwrap_or("")
    }
}

/// The theme name: from the settings portal, which every desktop's portal
/// backend answers, else GTK's and KDE's settings files.
#[cfg(windows)]
fn current_name() -> Option<String> { None }

#[cfg(target_os = "linux")]
fn current_name() -> Option<String> {
    portal_name().or_else(|| {
        let config = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|path| path.is_absolute()).unwrap_or_else(|| home().join(".config"));
        let read = |file: &str, section: &str, name: &str| {
            std::fs::read_to_string(config.join(file)).ok().map(|text| Ini::parse(&text).get(section, name).to_string()).filter(|value| !value.is_empty())
        };
        read("gtk-4.0/settings.ini", "Settings", "gtk-icon-theme-name")
            .or_else(|| read("gtk-3.0/settings.ini", "Settings", "gtk-icon-theme-name"))
            .or_else(|| read("kdeglobals", "Icons", "Theme"))
    })
}

#[cfg(target_os = "linux")]
fn portal_name() -> Option<String> {
    let connection = zbus::blocking::connection::Builder::session().ok()?.method_timeout(Duration::from_secs(1)).build().ok()?;
    let reply = connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "ReadOne",
            &("org.gnome.desktop.interface", "icon-theme"),
        )
        .ok()?;
    let value: zbus::zvariant::OwnedValue = reply.body().deserialize().ok()?;
    String::try_from(value).ok().filter(|name| !name.is_empty())
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

fn data_dirs() -> Vec<PathBuf> {
    let home_data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|path| path.is_absolute()).unwrap_or_else(|| home().join(".local/share"));
    let system = std::env::var("XDG_DATA_DIRS").ok().filter(|value| !value.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    std::iter::once(home_data).chain(system.split(':').map(PathBuf::from).filter(|path| path.is_absolute())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_finds_its_own_icon_before_a_parent_and_hicolor() {
        let root = std::env::temp_dir().join(format!("boxbox-icons-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let write = |path: &str, text: &str| {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(
            "icons/Child/index.theme",
            "[Icon Theme]\nInherits=Parent\nDirectories=16x16/apps,48x48/apps\n[16x16/apps]\nSize=16\nContext=Applications\nType=Fixed\n[48x48/apps]\nSize=48\nContext=Applications\nType=Fixed\n",
        );
        write("icons/Child/16x16/apps/editor.svg", "");
        write("icons/Child/48x48/apps/editor.svg", "");
        write("icons/Parent/index.theme", "[Icon Theme]\nDirectories=scalable/apps\n[scalable/apps]\nSize=48\nType=Scalable\nMinSize=8\nMaxSize=512\nContext=Applications\n");
        write("icons/Parent/scalable/apps/player.svg", "");
        write("icons/hicolor/index.theme", "[Icon Theme]\nDirectories=48x48/apps\n[48x48/apps]\nSize=48\nContext=Applications\nType=Threshold\n");
        write("icons/hicolor/48x48/apps/editor.png", "");
        write("icons/hicolor/48x48/apps/legacy.png", "");
        write("pixmaps/old.xpm.png", "");

        let theme = IconTheme::load(Some("Child"), &[root.clone()], &root.join("none"));
        let found = |name: &str| theme.find(name).map(|path| path.strip_prefix(&root).unwrap().to_path_buf());
        assert_eq!(found("editor"), Some(PathBuf::from("icons/Child/48x48/apps/editor.svg")));
        assert_eq!(found("player"), Some(PathBuf::from("icons/Parent/scalable/apps/player.svg")));
        assert_eq!(found("legacy"), Some(PathBuf::from("icons/hicolor/48x48/apps/legacy.png")));
        assert_eq!(found("old.xpm"), Some(PathBuf::from("pixmaps/old.xpm.png")));
        assert_eq!(found("missing"), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
