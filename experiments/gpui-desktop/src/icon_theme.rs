//! Finds an app's icon in the desktop's icon theme, as GTK does: the theme the
//! settings portal reports, the themes it inherits, then hicolor and pixmaps
//! (freedesktop Icon Theme Specification). Blocks on D-Bus and the filesystem,
//! so it runs off the UI thread.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The size the lookup aims for; the dialog draws icons at 24px, on HiDPI at 48.
const SIZE: u32 = 48;

pub struct IconTheme {
    /// Icon directories (`<base>/<theme>/<dir>`) for apps, best first.
    dirs: Vec<PathBuf>,
    pixmaps: Vec<PathBuf>,
}

struct Dir {
    path: String,
    size: u32,
    min: u32,
    max: u32,
    scale: u32,
}

impl Dir {
    /// How far the directory's icons are from `SIZE`: 0 when they cover it.
    fn distance(&self) -> u32 {
        if self.min <= SIZE && SIZE <= self.max {
            0
        } else if SIZE < self.min {
            self.min - SIZE
        } else {
            SIZE - self.max
        }
    }
}

impl IconTheme {
    /// The desktop's current theme with its parents.
    pub fn current() -> Self {
        Self::load(current_name().as_deref(), &data_dirs(), &home().join(".icons"))
    }

    /// `name` and its parents in `~/.icons` and each data dir's `icons`.
    fn load(name: Option<&str>, data: &[PathBuf], dot_icons: &Path) -> Self {
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
        for (theme, text) in &chain {
            let mut theme_dirs = app_dirs(&Ini::parse(text));
            theme_dirs.sort_by_key(|dir| (dir.distance(), dir.scale != 1, dir.size.abs_diff(SIZE)));
            for dir in theme_dirs {
                dirs.extend(bases.iter().map(|base| base.join(theme).join(&dir.path)).filter(|path| path.is_dir()));
            }
        }
        Self { dirs, pixmaps: data.iter().map(|dir| dir.join("pixmaps")).collect() }
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
        let in_dir = |dir: &PathBuf, extensions: &[&str]| {
            extensions.iter().map(|extension| dir.join(format!("{icon}.{extension}"))).find(|path| path.is_file())
        };
        self.dirs.iter().find_map(|dir| in_dir(dir, &["svg", "png"])).or_else(|| self.pixmaps.iter().find_map(|dir| in_dir(dir, &["png", "svg"])))
    }
}

/// The directories of a theme's `index.theme` that hold app icons.
fn app_dirs(index: &Ini) -> Vec<Dir> {
    let listed = index.get("Icon Theme", "Directories").to_string() + "," + index.get("Icon Theme", "ScaledDirectories");
    listed
        .split(',')
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .filter_map(|path| {
            let context = index.get(path, "Context");
            if context != "Applications" && !(context.is_empty() && path.ends_with("apps")) {
                return None;
            }
            let number = |name: &str| index.get(path, name).parse::<u32>().ok();
            let size = number("Size")?;
            let threshold = number("Threshold").unwrap_or(2);
            let (min, max) = match index.get(path, "Type") {
                "Fixed" => (size, size),
                "Scalable" => (number("MinSize").unwrap_or(size), number("MaxSize").unwrap_or(size)),
                _ => (size.saturating_sub(threshold), size + threshold),
            };
            Some(Dir { path: path.to_string(), size, min, max, scale: number("Scale").unwrap_or(1) })
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
