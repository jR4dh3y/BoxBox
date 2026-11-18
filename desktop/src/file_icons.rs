//! Icons for files and folders from the desktop's icon theme, as Thunar and
//! Nautilus draw them: a folder's (or a special folder's, such as Downloads),
//! and a file's by its MIME type from its name. Lookup and rasterising block on
//! the filesystem, so they run off the UI thread; views only read the result.

#[cfg(target_os = "linux")]
use std::path::Path;
use std::sync::Arc;

use anyhow::Context as _;
use gpui::RenderImage;

#[cfg(target_os = "linux")]
use crate::icon_theme::IconTheme;

/// What decides an entry's icon. A folder of 100 000 files has a few dozen keys.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IconKey {
    /// A folder: `folder`, or a special folder's icon such as `folder-download`.
    Folder(&'static str),
    /// A file, by its lowercased extension (`py`, `tar.gz`), or its whole name
    /// when it has none (`makefile`).
    File(String),
}

impl IconKey {
    /// Cheap enough for every drawn row: no lookups, one small string.
    pub fn for_entry(name: &str, is_dir: bool, special: Option<&'static str>) -> Self {
        if is_dir {
            return Self::Folder(special.unwrap_or("folder"));
        }
        let lower = name.to_lowercase();
        let parts: Vec<&str> = lower.trim_start_matches('.').rsplitn(3, '.').collect();
        Self::File(match parts.as_slice() {
            [extension, "tar", _] => format!("tar.{extension}"),
            [extension, _, ..] => extension.to_string(),
            _ => lower,
        })
    }

    /// The type a file's name gives it.
    #[cfg(target_os = "linux")]
    fn mime(&self) -> Option<String> {
        let Self::File(key) = self else { return None };
        // A whole name such as `makefile`, else an extension.
        crate::local::mime::from_name(Path::new(key)).or_else(|| crate::local::mime::from_name(Path::new(&format!("x.{key}"))))
    }

    /// The icon names to try, best first.
    #[cfg(target_os = "linux")]
    fn names(&self) -> Vec<String> {
        match self {
            Self::Folder("folder") => vec!["folder".into()],
            Self::Folder(special) => vec![special.to_string(), "folder".into()],
            Self::File(_) => {
                let mut names = self.mime().map(|mime| crate::local::mime::icon_names(&mime)).unwrap_or_default();
                names.extend(["text-x-generic".to_string(), "unknown".to_string()]);
                names
            }
        }
    }
}

/// The pixel sizes icons are drawn at, so zooming reuses them.
const BUCKETS: [u32; 9] = [16, 24, 32, 48, 64, 96, 128, 192, LARGEST_BUCKET];
pub const LARGEST_BUCKET: u32 = 256;

/// The bucket for an icon drawn at `size` logical pixels on a `scale` display.
pub fn bucket(size: f32, scale: f32) -> u32 {
    let wanted = (size * scale).ceil() as u32;
    BUCKETS.iter().copied().find(|bucket| *bucket >= wanted).unwrap_or(LARGEST_BUCKET)
}

/// The theme's icon for `key`, `size` pixels square. `None` when the theme has
/// none, and the view draws its own glyph.
#[cfg(target_os = "linux")]
pub fn render(key: &IconKey, size: u32) -> Option<Arc<RenderImage>> {
    let theme = IconTheme::shared();
    let names = key.names();
    let path = theme.find_first(&names, size)?;
    rasterize(&path, size).map_err(|error| log::debug!("{}: {error:#}", path.display())).ok()
}

#[cfg(windows)]
pub fn render(key: &IconKey, size: u32) -> Option<Arc<RenderImage>> {
    crate::windows::icons::render(key, size).ok().map(bgra)
}

/// Types whose files are programs or packages, drawn by what they are, not by
/// the app that would open them: a launcher or AppImage is its own app.
#[cfg(target_os = "linux")]
const OWN_ICON_TYPES: [&str; 9] = [
    "application/x-desktop",
    "application/x-executable",
    "application/x-sharedlib",
    // Older and newer shared-mime-info name shell scripts differently.
    "application/x-shellscript",
    "text/x-shellscript",
    "application/vnd.appimage",
    "application/x-iso9660-appimage",
    "application/vnd.debian.binary-package",
    "application/x-rpm",
];

/// The icon of the app that opens files like `key`, `size` pixels square, or
/// `None` when no app opens them or the type is drawn by what it is.
#[cfg(target_os = "linux")]
pub fn render_app(key: &IconKey, size: u32) -> Option<Arc<RenderImage>> {
    let mime = key.mime()?;
    // The exact type: scripts such as Python's are kinds of `x-executable`, yet open in an editor.
    if OWN_ICON_TYPES.contains(&mime.as_str()) {
        return None;
    }
    let path = crate::open_with::default_app_icon(&mime, size)?;
    rasterize(&path, size).map_err(|error| log::debug!("{}: {error:#}", path.display())).ok()
}

/// An SVG centred in a `size` pixel square, as GPUI's BGRA pixels. Also the
/// BoxBox logo, which GPUI's own SVG loader refuses.
pub fn svg_image(bytes: &[u8], size: u32) -> anyhow::Result<Arc<RenderImage>> {
    Ok(bgra(svg_pixels(bytes, size)?))
}

fn svg_pixels(bytes: &[u8], size: u32) -> anyhow::Result<image::RgbaImage> {
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default())?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).context("empty icon size")?;
    let natural = tree.size();
    let scale = size as f32 / natural.width().max(natural.height());
    let offset = |length: f32| (size as f32 - length * scale) / 2.;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(offset(natural.width()), offset(natural.height()));
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    // tiny-skia keeps premultiplied pixels; GPUI takes them straight.
    let straight: Vec<u8> = pixmap.pixels().iter().flat_map(|pixel| {
        let color = pixel.demultiply();
        [color.red(), color.green(), color.blue(), color.alpha()]
    }).collect();
    image::RgbaImage::from_raw(size, size, straight).context("icon pixels")
}

fn bgra(mut pixels: image::RgbaImage) -> Arc<RenderImage> {
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]))
}

/// A package's own icon file, SVG or raster, `size` pixels square.
#[cfg(target_os = "linux")]
pub fn package_image(bytes: &[u8], size: u32) -> Option<Arc<RenderImage>> {
    let svg = image::guess_format(bytes).is_err();
    pixels(bytes, svg, size).map(bgra).map_err(|error| log::debug!("package icon: {error:#}")).ok()
}

/// An icon file (a theme's, an app's), as GPUI's BGRA pixels.
#[cfg(target_os = "linux")]
pub(crate) fn rasterize(path: &Path, size: u32) -> anyhow::Result<Arc<RenderImage>> {
    let svg = path.extension().is_some_and(|extension| extension == "svg");
    Ok(bgra(pixels(&std::fs::read(path)?, svg, size)?))
}

/// An SVG drawn at `size`, or a PNG scaled down to it.
#[cfg(target_os = "linux")]
fn pixels(bytes: &[u8], svg: bool, size: u32) -> anyhow::Result<image::RgbaImage> {
    if svg {
        return svg_pixels(bytes, size);
    }
    let decoded = image::load_from_memory(bytes)?.into_rgba8();
    // Never scaled up: a small PNG stays sharp at its own size.
    Ok(match decoded.width().max(decoded.height()) > size {
        true => image::imageops::thumbnail(&decoded, size, size),
        false => decoded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_group_entries_by_type() {
        assert_eq!(IconKey::for_entry("Photo.JPG", false, None), IconKey::File("jpg".into()));
        assert_eq!(IconKey::for_entry("a.tar.gz", false, None), IconKey::File("tar.gz".into()));
        assert_eq!(IconKey::for_entry("Makefile", false, None), IconKey::File("makefile".into()));
        assert_eq!(IconKey::for_entry(".bashrc", false, None), IconKey::File(".bashrc".into()));
        assert_eq!(IconKey::for_entry("Downloads", true, Some("folder-download")), IconKey::Folder("folder-download"));
        assert_eq!(bucket(20., 1.), 24);

    }

    #[test]
    fn launchers_programs_and_packages_keep_their_own_icons() {
        for name in ["tool.desktop", "run.sh", "app.AppImage", "pkg.deb", "pkg.rpm"] {
            let key = IconKey::for_entry(name, false, None);
            assert!(render_app(&key, 32).is_none(), "{name} is drawn by what it is");
        }
        assert!(render_app(&IconKey::Folder("folder"), 32).is_none());
        assert_eq!(bucket(20., 2.), 48);
    }
}
