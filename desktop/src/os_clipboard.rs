//! Files on the system clipboard, so copy and paste work with other file
//! managers. A copy is offered as `x-special/gnome-copied-files` (GNOME,
//! Thunar, Nemo), `text/uri-list` with `application/x-kde-cutselection`
//! (KDE), and plain paths for terminals.
//!
//! On Wayland this owns the clipboard through the data-control protocol
//! (wlroots compositors, Niri, Hyprland, KDE). GNOME has no such protocol,
//! and X11 offers files in GNOME's format. X11 reads use `xclip`. Every call here
//! blocks, so it runs off the UI thread.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use anyhow::{Context as _, Result};

#[path = "os_clipboard/wayland.rs"]
mod wayland_clipboard;

static X11_CLIPBOARD: Mutex<Option<x11_clipboard::Clipboard>> = Mutex::new(None);

const GNOME: &str = "x-special/gnome-copied-files";
const URI_LIST: &str = "text/uri-list";
const KDE_CUT: &str = "application/x-kde-cutselection";
/// More than enough for a file list; a larger clipboard is not files.
const MAX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileOp {
    Copy,
    Cut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Files {
    pub op: FileOp,
    pub paths: Vec<PathBuf>,
}

/// `file://` URI with every byte outside the unreserved set and `/` escaped.
pub fn file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut uri = String::from("file://");
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

pub fn path_from_uri(uri: &str) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let rest = uri.trim().strip_prefix("file://")?;
    // `file://host/path`: only the local host is ours.
    let path = match rest.find('/') {
        Some(0) => rest,
        Some(index) if &rest[..index] == "localhost" => &rest[index..],
        _ => return None,
    };
    let mut bytes = Vec::with_capacity(path.len());
    let raw = path.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' && index + 2 < raw.len() {
            let hex = std::str::from_utf8(&raw[index + 1..index + 3]).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            bytes.push(raw[index]);
            index += 1;
        }
    }
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

/// Every format a copy is offered in, as (MIME type, bytes).
pub fn encode(files: &Files) -> Vec<(&'static str, Vec<u8>)> {
    let uris: Vec<String> = files.paths.iter().map(|path| file_uri(path)).collect();
    let verb = match files.op {
        FileOp::Copy => "copy",
        FileOp::Cut => "cut",
    };
    let mut formats = vec![
        (GNOME, format!("{verb}\n{}", uris.join("\n")).into_bytes()),
        (URI_LIST, uris.iter().map(|uri| format!("{uri}\r\n")).collect::<String>().into_bytes()),
        ("text/plain;charset=utf-8", files.paths.iter().map(|path| path.to_string_lossy()).collect::<Vec<_>>().join("\n").into_bytes()),
    ];
    if files.op == FileOp::Cut {
        formats.push((KDE_CUT, b"1".to_vec()));
    }
    formats
}

/// Reads files from the formats a clipboard offered, preferring GNOME's.
pub fn decode(gnome: Option<&[u8]>, uri_list: Option<&[u8]>, kde_cut: Option<&[u8]>) -> Option<Files> {
    if let Some(text) = gnome.and_then(|bytes| std::str::from_utf8(bytes).ok()) {
        let mut lines = text.lines();
        let op = match lines.next()?.trim() {
            "copy" => FileOp::Copy,
            "cut" => FileOp::Cut,
            _ => return None,
        };
        let paths: Vec<PathBuf> = lines.filter_map(path_from_uri).collect();
        return (!paths.is_empty()).then_some(Files { op, paths });
    }
    let text = std::str::from_utf8(uri_list?).ok()?;
    let paths: Vec<PathBuf> = text.lines().filter(|line| !line.starts_with('#')).filter_map(path_from_uri).collect();
    let cut = kde_cut.is_some_and(|flag| flag.first() == Some(&b'1'));
    (!paths.is_empty()).then_some(Files { op: if cut { FileOp::Cut } else { FileOp::Copy }, paths })
}

pub(crate) fn wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty())
}

/// Publishes text without depending on the window still having Wayland focus.
pub(crate) fn publish_wayland_text(text: &str) -> Result<()> {
    wayland_clipboard::publish(
        ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING", "STRING", "TEXT"]
            .map(|mime| (mime, text.as_bytes().to_vec()))
            .into(),
    )
}

/// Offers `files` until another app takes the clipboard. Returns once it is offered.
pub fn publish(files: &Files) -> Result<()> {
    // Tests run inside the developer's session; never touch its clipboard.
    if cfg!(test) {
        return Ok(());
    }
    let formats = encode(files);
    if wayland() {
        let mut formats = formats;
        if let Some((_, text)) = formats.iter().find(|(mime, _)| *mime == "text/plain;charset=utf-8") {
            let text = text.clone();
            formats.extend(["text/plain", "UTF8_STRING", "STRING", "TEXT"].map(|mime| (mime, text.clone())));
        }
        return wayland_clipboard::publish(formats);
    }
    // The owner window stays alive while other apps can request the files.
    let clipboard = x11_clipboard::Clipboard::new()?;
    let target = clipboard.setter.get_atom(GNOME)?;
    let (_, bytes) = formats.into_iter().next().context("nothing to copy")?;
    let mut published = X11_CLIPBOARD.lock().expect("X11 clipboard owner");
    clipboard.store(clipboard.setter.atoms.clipboard, target, bytes)?;
    *published = Some(clipboard);
    Ok(())
}

pub fn clear() -> Result<()> {
    if cfg!(test) {
        return Ok(());
    }
    if wayland() {
        use wl_clipboard_rs::copy::{clear, ClipboardType, Seat};
        return clear(ClipboardType::Regular, Seat::All).context("cannot clear the Wayland clipboard");
    }
    use x11rb::protocol::xproto::ConnectionExt as _;
    let mut published = X11_CLIPBOARD.lock().expect("X11 clipboard owner");
    let (connection, _) = x11rb::connect(None)?;
    let clipboard = connection.intern_atom(false, b"CLIPBOARD")?.reply()?.atom;
    connection.set_selection_owner(x11rb::NONE, clipboard, x11rb::CURRENT_TIME)?.check()?;
    *published = None;
    Ok(())
}

/// The files on the system clipboard, if it holds any.
pub fn read() -> Result<Option<Files>> {
    if cfg!(test) {
        return Ok(None);
    }
    let fetch = |mime: &str| -> Option<Vec<u8>> {
        let mut bytes = Vec::new();
        if wayland() {
            use wl_clipboard_rs::paste::{get_contents, ClipboardType, MimeType, Seat};
            let (pipe, _) = get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Specific(mime)).ok()?;
            pipe.take(MAX_BYTES).read_to_end(&mut bytes).ok()?;
        } else {
            let output = Command::new("xclip").args(["-selection", "clipboard", "-o", "-t", mime]).stderr(Stdio::null()).output().ok()?;
            if !output.status.success() {
                return None;
            }
            bytes = output.stdout;
        }
        Some(bytes)
    };
    let available: Vec<String> = if wayland() {
        use wl_clipboard_rs::paste::{get_mime_types, ClipboardType, Error, Seat};
        match get_mime_types(ClipboardType::Regular, Seat::Unspecified) {
            Ok(types) => types.into_iter().collect(),
            Err(Error::ClipboardEmpty | Error::NoSeats) => return Ok(None),
            Err(error) => return Err(error).context("cannot read the clipboard"),
        }
    } else {
        match fetch("TARGETS") {
            Some(targets) => String::from_utf8_lossy(&targets).lines().map(str::to_string).collect(),
            None => return Ok(None),
        }
    };
    let has = |mime: &str| available.iter().any(|offered| offered.eq_ignore_ascii_case(mime));
    let gnome = has(GNOME).then(|| fetch(GNOME)).flatten();
    let uri_list = (gnome.is_none() && has(URI_LIST)).then(|| fetch(URI_LIST)).flatten();
    let kde_cut = has(KDE_CUT).then(|| fetch(KDE_CUT)).flatten();
    Ok(decode(gnome.as_deref(), uri_list.as_deref(), kde_cut.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_round_trip_odd_names() {
        let path = PathBuf::from("/home/me/My Files/naïve #1 100%.txt");
        let uri = file_uri(&path);
        assert_eq!(uri, "file:///home/me/My%20Files/na%C3%AFve%20%231%20100%25.txt");
        assert_eq!(path_from_uri(&uri), Some(path));
        assert_eq!(path_from_uri("file://localhost/tmp/a"), Some(PathBuf::from("/tmp/a")));
        assert_eq!(path_from_uri("file://other/tmp/a"), None);
        assert_eq!(path_from_uri("https://example.com/a"), None);
    }

    #[test]
    fn a_cut_reads_back_from_either_format() {
        let files = Files { op: FileOp::Cut, paths: vec![PathBuf::from("/tmp/a b"), PathBuf::from("/tmp/c")] };
        let formats = encode(&files);
        let get = |mime: &str| formats.iter().find(|(offered, _)| *offered == mime).map(|(_, bytes)| bytes.as_slice());
        assert_eq!(decode(get(GNOME), None, None), Some(files.clone()));
        assert_eq!(decode(None, get(URI_LIST), get(KDE_CUT)), Some(files));
        assert_eq!(decode(Some(b"copy"), None, None), None);
    }
}
