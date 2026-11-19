//! The icon a package's desktop entries name: their `Icon=`, found in the
//! hicolor theme, pixmaps or AppDir root it ships. A deb or rpm streams its
//! files past once per pass; the first pass keeps the likely icons, so a
//! second pass runs only when the chosen one was not kept.

use std::collections::HashMap;
use std::io::{self, Read};

use super::{resolve, ICON_LIMIT};

/// One file or symbolic link in the package, its path relative to the root.
pub(super) enum Member<'a> {
    File { path: &'a str, size: u64, data: &'a mut dyn Read },
    Link { path: &'a str, target: &'a str },
}

pub(super) enum Flow {
    Continue,
    Stop,
}

/// Streams a package's members to the visitor until it stops.
pub(super) type Visitor<'v> = &'v mut dyn FnMut(Member) -> io::Result<Flow>;

/// The most a desktop entry may hold.
const DESKTOP_LIMIT: u64 = 64 << 10;
/// The most icon bytes a first pass keeps.
const KEEP_BUDGET: u64 = 8 << 20;
/// Links followed to reach an icon.
const HOPS: usize = 8;

/// A path as the reader saw it, relative to the package root.
pub(super) fn normalize(path: &str) -> &str {
    path.trim_start_matches("./").trim_start_matches('/')
}

/// What a pass over a package learnt.
#[derive(Default)]
pub(super) struct Survey {
    /// Desktop entries that name the package's icon, files or links, in package order.
    entries: Vec<String>,
    /// The text of each small desktop file, by path.
    desktop_files: HashMap<String, String>,
    /// PNG and SVG files, by path, with their size.
    images: HashMap<String, u64>,
    links: HashMap<String, String>,
    /// The bytes of images in icon folders, within `KEEP_BUDGET`.
    kept: HashMap<String, Vec<u8>>,
    kept_bytes: u64,
}

impl Survey {
    /// Notes `member`. `entry` tells which desktop files name the icon. Reads
    /// a file's data only for a desktop file, or an icon to keep when `keep`.
    pub(super) fn add(&mut self, member: Member, entry: fn(&str) -> bool, keep: bool) -> io::Result<()> {
        match member {
            Member::File { path, size, data } => {
                if entry(path) {
                    self.entries.push(path.to_string());
                }
                if path.ends_with(".desktop") && size <= DESKTOP_LIMIT {
                    let mut text = String::new();
                    data.take(DESKTOP_LIMIT).read_to_string(&mut text).ok();
                    self.desktop_files.insert(path.to_string(), text);
                } else if extension(path).is_some() {
                    self.images.insert(path.to_string(), size);
                    if keep && in_icon_folder(path) && self.kept_bytes + size <= KEEP_BUDGET {
                        let mut bytes = Vec::new();
                        data.take(size).read_to_end(&mut bytes)?;
                        self.kept_bytes += size;
                        self.kept.insert(path.to_string(), bytes);
                    }
                }
            }
            Member::Link { path, target } => {
                if entry(path) {
                    self.entries.push(path.to_string());
                }
                self.links.insert(path.to_string(), target.to_string());
            }
        }
        Ok(())
    }

    /// The path of the image file to read for `size` pixels: the best fitting
    /// icon any desktop entry names.
    pub(super) fn choose(&self, size: u32) -> Option<String> {
        let names = self.entries.iter().filter_map(|entry| follow(&self.desktop_files, &self.links, entry)).filter_map(|(_, text)| desktop_icon(text));
        let mut best: Option<((u8, u32), String)> = None;
        for name in names {
            // `Icon=` may carry an extension, or be an absolute path in the package.
            let stem = name.strip_suffix(".png").or_else(|| name.strip_suffix(".svg")).unwrap_or(name);
            for path in self.images.keys().chain(self.links.keys()) {
                let Some(extension) = extension(path) else { continue };
                let pixels = match name.starts_with('/') {
                    true if normalize(name) == path => None,
                    true => continue,
                    false => match place(path, stem) {
                        Some(pixels) => pixels,
                        None => continue,
                    },
                };
                let Some((file, bytes)) = follow(&self.images, &self.links, path) else { continue };
                let rank = rank(extension, pixels, size);
                // Equal ranks fall to the first path, as the maps have no order.
                if *bytes <= ICON_LIMIT && best.as_ref().is_none_or(|best| (rank, &file) < (best.0, &best.1)) {
                    best = Some((rank, file));
                }
            }
        }
        best.map(|(_, file)| file)
    }
}

/// The icon file for `size` pixels, read with one or two passes of `walk`.
pub(super) fn icon(walk: impl Fn(Visitor) -> anyhow::Result<()>, size: u32) -> anyhow::Result<Option<Vec<u8>>> {
    let mut survey = Survey::default();
    walk(&mut |member| {
        survey.add(member, is_desktop_entry, true)?;
        Ok(Flow::Continue)
    })?;
    let Some(chosen) = survey.choose(size) else { return Ok(None) };
    if let Some(bytes) = survey.kept.remove(&chosen) {
        return Ok(Some(bytes));
    }
    let mut bytes = None;
    walk(&mut |member| match member {
        Member::File { path, data, .. } if path == chosen => {
            let mut read = Vec::new();
            data.take(ICON_LIMIT).read_to_end(&mut read)?;
            bytes = Some(read);
            Ok(Flow::Stop)
        }
        _ => Ok(Flow::Continue),
    })?;
    Ok(bytes)
}

/// The file `path` names in `files`, through links.
fn follow<'a, V>(files: &'a HashMap<String, V>, links: &HashMap<String, String>, path: &str) -> Option<(String, &'a V)> {
    let mut path = path.to_string();
    for _ in 0..HOPS {
        if let Some(value) = files.get(&path) {
            return Some((path, value));
        }
        let target = links.get(&path)?;
        let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        path = resolve(dir, target);
    }
    None
}

fn in_icon_folder(path: &str) -> bool {
    path.starts_with("usr/share/pixmaps/") || path.starts_with("usr/share/icons/")
}

fn is_desktop_entry(path: &str) -> bool {
    path.strip_prefix("usr/share/applications/").is_some_and(|name| !name.contains('/') && name.ends_with(".desktop"))
}

/// The `Icon=` of a desktop entry's main group.
fn desktop_icon(text: &str) -> Option<&str> {
    let mut main = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            main = line == "[Desktop Entry]";
        } else if let Some(value) = line.strip_prefix("Icon").map(str::trim_start).filter(|_| main).and_then(|rest| rest.strip_prefix('=')) {
            return Some(value.trim()).filter(|value| !value.is_empty());
        }
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Extension {
    Png,
    Svg,
}

fn extension(path: &str) -> Option<Extension> {
    let (_, extension) = path.rsplit_once('.')?;
    match extension.to_ascii_lowercase().as_str() {
        "png" => Some(Extension::Png),
        "svg" => Some(Extension::Svg),
        _ => None,
    }
}

/// How well an icon fits `size` pixels, best lowest: a scalable SVG, then the
/// smallest PNG at least that size, then one of unknown size, then the largest smaller one.
fn rank(extension: Extension, pixels: Option<u32>, size: u32) -> (u8, u32) {
    match (extension, pixels) {
        (Extension::Svg, _) => (0, 0),
        (Extension::Png, Some(pixels)) if pixels >= size => (1, pixels - size),
        (Extension::Png, None) => (2, 0),
        (Extension::Png, Some(pixels)) => (3, size - pixels),
    }
}

/// Where an icon named `name` may be installed, with its pixel size when the path says.
fn place(path: &str, name: &str) -> Option<Option<u32>> {
    let (dir, file) = path.rsplit_once('/').unwrap_or(("", path));
    let (stem, _) = file.rsplit_once('.')?;
    if stem != name {
        return None;
    }
    // An AppDir keeps its icon at the root.
    if dir == "usr/share/pixmaps" || dir.is_empty() {
        return Some(None);
    }
    // usr/share/icons/<theme>/<size>/apps
    let mut parts = dir.strip_prefix("usr/share/icons/")?.split('/');
    let (Some(_), Some(theme_size), Some("apps"), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return None };
    match theme_size {
        "scalable" => Some(None),
        // `256x256@2` holds 512 pixel icons.
        size => {
            let (logical, scale) = size.split_once('@').unwrap_or((size, "1"));
            Some(logical.split_once('x')?.0.parse::<u32>().ok().zip(scale.parse::<u32>().ok()).map(|(logical, scale)| logical * scale))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_entries_name_their_icon() {
        assert_eq!(desktop_icon("[Desktop Entry]\nName=A\nIcon[de]=b\nIcon = app\n"), Some("app"));
        assert_eq!(desktop_icon("[Desktop Action new]\nIcon=other\n[Desktop Entry]\nName=A\n"), None);
        assert_eq!(desktop_icon("[Desktop Entry]\nIcon=\n"), None);
        assert!(is_desktop_entry("usr/share/applications/app.desktop"));
        assert!(!is_desktop_entry("usr/share/applications/kde/app.desktop"));
    }

    /// A package whose desktop entries name `names`.
    fn survey(names: &[&str], images: &[&str], links: &[(&str, &str)]) -> Survey {
        let entries: Vec<String> = (0..names.len()).map(|index| format!("usr/share/applications/{index}.desktop")).collect();
        Survey {
            desktop_files: entries.iter().zip(names).map(|(entry, name)| (entry.clone(), format!("[Desktop Entry]\nIcon={name}\n"))).collect(),
            entries,
            images: images.iter().map(|path| (path.to_string(), 1000)).collect(),
            links: links.iter().map(|(path, target)| (path.to_string(), target.to_string())).collect(),
            ..Survey::default()
        }
    }

    fn choose(survey: &Survey, size: u32) -> Option<String> {
        survey.choose(size)
    }

    #[test]
    fn the_best_size_wins() {
        let images = ["usr/share/icons/hicolor/48x48/apps/app.png", "usr/share/icons/hicolor/512x512/apps/app.png", "usr/share/icons/hicolor/256x256/apps/app.png", "usr/share/pixmaps/app.png", "usr/share/icons/hicolor/1024x1024/apps/other.png"];
        assert_eq!(choose(&survey(&["app"], &images, &[]), 256).as_deref(), Some("usr/share/icons/hicolor/256x256/apps/app.png"));
        assert_eq!(choose(&survey(&["app"], &images, &[]), 600).as_deref(), Some("usr/share/pixmaps/app.png"));
        assert_eq!(choose(&survey(&["app"], &images[..1], &[]), 64).as_deref(), Some("usr/share/icons/hicolor/48x48/apps/app.png"));
        let with_svg = [images.as_slice(), &["usr/share/icons/hicolor/scalable/apps/app.svg"]].concat();
        assert_eq!(choose(&survey(&["app.png"], &with_svg, &[]), 256).as_deref(), Some("usr/share/icons/hicolor/scalable/apps/app.svg"));
        assert_eq!(choose(&survey(&["missing"], &images, &[]), 256), None);
        let retina = ["usr/share/icons/hicolor/256x256@2/apps/app.png", "usr/share/icons/hicolor/384x384/apps/app.png"];
        assert_eq!(choose(&survey(&["app"], &retina, &[]), 500).as_deref(), Some(retina[0]));
        assert_eq!(choose(&survey(&["app"], &["app.png", "usr/share/icons/hicolor/32x32/apps/app.png"], &[]), 256).as_deref(), Some("app.png"));
    }

    #[test]
    fn absolute_names_and_links_reach_the_file() {
        let images = ["opt/App/resources/icon.png", "opt/App/logo.svg"];
        let links = [("usr/share/pixmaps/app.png", "/opt/App/resources/icon.png"), ("opt/App/loop.png", "loop.png")];
        assert_eq!(choose(&survey(&["app"], &images, &links), 256).as_deref(), Some("opt/App/resources/icon.png"));
        assert_eq!(choose(&survey(&["/opt/App/logo.svg"], &images, &links), 256).as_deref(), Some("opt/App/logo.svg"));
        assert_eq!(choose(&survey(&["/opt/App/loop.png"], &images, &links), 256), None);

        // The desktop entry itself may be a link, as Discord's is.
        let mut linked = survey(&[], &["usr/share/discord/discord.png"], &[("usr/share/pixmaps/discord.png", "../discord/discord.png"), ("usr/share/applications/discord.desktop", "../discord/discord.desktop")]);
        linked.entries.push("usr/share/applications/discord.desktop".into());
        linked.desktop_files.insert("usr/share/discord/discord.desktop".into(), "[Desktop Entry]\nIcon=discord\n".into());
        assert_eq!(choose(&linked, 256).as_deref(), Some("usr/share/discord/discord.png"));
    }
}
