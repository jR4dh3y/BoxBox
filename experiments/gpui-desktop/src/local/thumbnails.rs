//! The shared freedesktop thumbnail cache (Thumbnail Managing Standard 0.9), so
//! thumbnails other file managers made show at once and ours are reused by them.

use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use md5::{Digest, Md5};

/// The standard's sizes, smallest first.
const FLAVORS: [(&str, u32); 4] = [("normal", 128), ("large", 256), ("x-large", 512), ("xx-large", 1024)];
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// A PNG that covers `width` x `height`, from the cache when it is current.
pub fn thumbnail(path: &Path, width: u32, height: u32) -> Result<Vec<u8>> {
    match cache_root() {
        Some(root) if !path.starts_with(&root) => cached(&root, path, width.max(height)),
        // A thumbnail of a thumbnail is never cached, as the standard asks.
        _ => render(path, width, height),
    }
}

fn cache_root() -> Option<PathBuf> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .filter(|path| Path::new(path).is_absolute())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".cache")))?;
    Some(cache.join("thumbnails"))
}

fn render(path: &Path, width: u32, height: u32) -> Result<Vec<u8>> {
    boxbox_thumbnail::thumbnail_path(path, width, height).with_context(|| format!("cannot create thumbnail for {}", path.display()))
}

fn cached(root: &Path, path: &Path, size: u32) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path).with_context(|| format!("cannot read {}", path.display()))?;
    let uri = file_uri(path);
    let name = format!("{:x}.png", Md5::digest(uri.as_bytes()));
    let flavor = FLAVORS.iter().position(|&(_, pixels)| pixels >= size).unwrap_or(FLAVORS.len() - 1);
    // A larger thumbnail serves a smaller tile too.
    for (directory, _) in &FLAVORS[flavor..] {
        let Ok(bytes) = fs::read(root.join(directory).join(&name)) else { continue };
        if is_current(&bytes, &uri, &metadata) {
            return Ok(bytes);
        }
    }
    let (directory, pixels) = FLAVORS[flavor];
    let mut bytes = render(path, pixels, pixels)?;
    let attributes = [
        ("Thumb::URI", uri),
        ("Thumb::MTime", metadata.mtime().to_string()),
        ("Thumb::Size", metadata.size().to_string()),
        ("Software", "BoxBox".to_string()),
    ];
    if insert_text(&mut bytes, &attributes) {
        if let Err(error) = store(&root.join(directory), &name, &bytes) {
            log::debug!("cannot cache the thumbnail of {}: {error:#}", path.display());
        }
    }
    Ok(bytes)
}

/// `g_filename_to_uri`: the bytes GLib leaves unescaped, so our names match other apps'.
fn file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"!$&'()*+,-./:=@_~".contains(&byte) {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

/// The standard's validity check: the same file, unchanged since it was thumbnailed.
fn is_current(png: &[u8], uri: &str, metadata: &fs::Metadata) -> bool {
    let text = text_chunks(png);
    let value = |key: &str| text.iter().find(|(name, _)| name == key).map(|(_, value)| value.as_str());
    value("Thumb::URI") == Some(uri)
        && value("Thumb::MTime").and_then(|mtime| mtime.parse::<i64>().ok()) == Some(metadata.mtime())
        && value("Thumb::Size").is_none_or(|size| size.parse::<u64>().ok() == Some(metadata.size()))
}

/// The `tEXt` chunks before the image data.
fn text_chunks(png: &[u8]) -> Vec<(String, String)> {
    let mut text = Vec::new();
    if !png.starts_with(PNG_SIGNATURE) {
        return text;
    }
    let mut offset = PNG_SIGNATURE.len();
    while let Some(header) = png.get(offset..offset + 8) {
        let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let kind = &header[4..8];
        let Some(data) = png.get(offset + 8..offset + 8 + length) else { break };
        match kind {
            b"IDAT" | b"IEND" => break,
            b"tEXt" => {
                if let Some(split) = data.iter().position(|&byte| byte == 0) {
                    let latin1 = |bytes: &[u8]| bytes.iter().map(|&byte| byte as char).collect::<String>();
                    text.push((latin1(&data[..split]), latin1(&data[split + 1..])));
                }
            }
            _ => {}
        }
        offset += 12 + length;
    }
    text
}

/// Adds `tEXt` chunks after IHDR. Text is ASCII here: the URI is escaped and the rest are numbers.
fn insert_text(png: &mut Vec<u8>, attributes: &[(&str, String)]) -> bool {
    const IHDR_END: usize = 8 + 8 + 13 + 4;
    if !png.starts_with(PNG_SIGNATURE) || png.get(12..16) != Some(b"IHDR") || png.len() < IHDR_END {
        return false;
    }
    let mut chunks = Vec::new();
    for (key, value) in attributes {
        let mut data = Vec::with_capacity(key.len() + 1 + value.len());
        data.extend_from_slice(key.as_bytes());
        data.push(0);
        data.extend_from_slice(value.as_bytes());
        let mut crc = crc32fast::Hasher::new();
        crc.update(b"tEXt");
        crc.update(&data);
        chunks.extend_from_slice(&(data.len() as u32).to_be_bytes());
        chunks.extend_from_slice(b"tEXt");
        chunks.extend_from_slice(&data);
        chunks.extend_from_slice(&crc.finalize().to_be_bytes());
    }
    png.splice(IHDR_END..IHDR_END, chunks);
    true
}

/// Writes beside the final name and renames, so readers never see half a file.
fn store(directory: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    fs::DirBuilder::new().recursive(true).mode(0o700).create(directory)?;
    let temporary = directory.join(format!(".{name}.{}.{:?}", std::process::id(), std::thread::current().id()));
    let written = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .and_then(|mut file| file.write_all(bytes));
    match written.and_then(|()| fs::rename(&temporary, directory.join(name))) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_the_standard_and_glib() {
        // The standard's own example.
        let uri = file_uri(Path::new("/home/jens/photos/me.png"));
        assert_eq!(format!("{:x}", Md5::digest(uri.as_bytes())), "c6ee772d9e49320e97ec29a7eb5b1697");
        assert_eq!(file_uri(Path::new("/a b/ü#%.png")), "file:///a%20b/%C3%BC%23%25.png");
    }

    #[test]
    fn a_thumbnail_is_reused_until_its_file_changes() {
        let directory = std::env::temp_dir().join(format!("boxbox-thumbnails-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let root = directory.join("cache");
        let photo = directory.join("photo.png");
        fs::create_dir_all(&directory).unwrap();
        image::RgbImage::from_pixel(600, 400, image::Rgb([200, 30, 30])).save(&photo).unwrap();

        let first = cached(&root, &photo, 160).unwrap();
        let name = format!("{:x}.png", Md5::digest(file_uri(&photo).as_bytes()));
        let stored = root.join("large").join(&name);
        assert_eq!(fs::read(&stored).unwrap(), first);
        assert_eq!(image::load_from_memory(&first).unwrap().width(), 256);
        let mode = fs::metadata(&stored).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);

        // A planted marker proves the second request reads the cache.
        let mut marked = first.clone();
        assert!(insert_text(&mut marked, &[("Marker", "cached".into())]));
        fs::write(&stored, &marked).unwrap();
        assert_eq!(cached(&root, &photo, 160).unwrap(), marked);

        // A newer file makes the entry stale.
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        fs::File::options().write(true).open(&photo).unwrap().set_modified(later).unwrap();
        let fresh = cached(&root, &photo, 160).unwrap();
        assert!(!text_chunks(&fresh).iter().any(|(key, _)| key == "Marker"));
        fs::remove_dir_all(&directory).unwrap();
    }
}
