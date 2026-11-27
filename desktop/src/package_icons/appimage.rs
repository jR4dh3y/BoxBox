//! A type 2 AppImage's icon: an ELF runtime followed by a squashfs image,
//! whose root `.DirIcon`, often a link, is the PNG or SVG the desktop shows.
//! Without a usable one, the AppDir's root desktop entry names the icon.

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{bail, ensure, Context as _};
use backhand::{FilesystemReader, InnerNode};

use super::archive::{normalize, Member, Survey};
use super::{resolve, ICON_LIMIT};

/// Links followed to reach the icon.
const HOPS: usize = 8;

pub(super) fn icon(path: &Path, size: u32) -> anyhow::Result<Option<Vec<u8>>> {
    let mut file = BufReader::new(File::open(path)?);
    let offset = image_offset(&mut file)?;
    let image = FilesystemReader::from_reader_with_offset(file, offset)?;
    if let Some(bytes) = dir_icon(&image)? {
        return Ok(Some(bytes));
    }
    let mut survey = Survey::default();
    for node in image.files() {
        let path = node.fullpath.to_string_lossy();
        let path = normalize(&path);
        match &node.inner {
            // Only desktop files are read.
            InnerNode::File(file) if path.ends_with(".desktop") => {
                survey.add(Member::File { path, size: file.file_len() as u64, data: &mut image.file(file).reader() }, is_root_entry, false)?
            }
            InnerNode::File(file) => survey.add(Member::File { path, size: file.file_len() as u64, data: &mut io::empty() }, is_root_entry, false)?,
            InnerNode::Symlink(link) => survey.add(Member::Link { path, target: &link.link.to_string_lossy() }, is_root_entry, false)?,
            _ => {}
        }
    }
    match survey.choose(size) {
        Some(chosen) => read(&image, &chosen),
        None => Ok(None),
    }
}

fn is_root_entry(path: &str) -> bool {
    !path.contains('/') && path.ends_with(".desktop")
}

/// The image's `.DirIcon`, through links. `None` when missing or dangling,
/// as one linked to the path it was built at.
fn dir_icon(image: &FilesystemReader) -> anyhow::Result<Option<Vec<u8>>> {
    let mut name = String::from(".DirIcon");
    for _ in 0..HOPS {
        let Some(node) = image.files().find(|node| normalize(&node.fullpath.to_string_lossy()) == name) else { return Ok(None) };
        match &node.inner {
            InnerNode::File(_) => return read(image, &name),
            InnerNode::Symlink(link) => {
                let dir = name.rsplit_once('/').map_or("", |(dir, _)| dir);
                name = resolve(dir, &link.link.to_string_lossy());
            }
            _ => return Ok(None),
        }
    }
    Ok(None)
}

/// The regular file at `path` in the image.
fn read(image: &FilesystemReader, path: &str) -> anyhow::Result<Option<Vec<u8>>> {
    let Some(node) = image.files().find(|node| normalize(&node.fullpath.to_string_lossy()) == path) else { return Ok(None) };
    let InnerNode::File(file) = &node.inner else { bail!("/{path} is not a file") };
    ensure!(file.file_len() as u64 <= ICON_LIMIT, "/{path} is too large for an icon");
    let mut bytes = Vec::with_capacity(file.file_len());
    image.file(file).reader().read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

/// Where the squashfs image starts: the end of the ELF runtime's section headers.
fn image_offset(file: &mut (impl Read + Seek)) -> anyhow::Result<u64> {
    let mut elf = [0; 64];
    file.read_exact(&mut elf).context("not an ELF file")?;
    ensure!(elf[..4] == *b"\x7fELF", "not an ELF file");
    let little = match elf[5] {
        1 => true,
        2 => false,
        _ => bail!("unknown ELF byte order"),
    };
    let u16_at = |at: usize| {
        let bytes = [elf[at], elf[at + 1]];
        u64::from(if little { u16::from_le_bytes(bytes) } else { u16::from_be_bytes(bytes) })
    };
    let u32_at = |at: usize| {
        let bytes = [elf[at], elf[at + 1], elf[at + 2], elf[at + 3]];
        u64::from(if little { u32::from_le_bytes(bytes) } else { u32::from_be_bytes(bytes) })
    };
    let u64_at = |at: usize| {
        let bytes: [u8; 8] = std::array::from_fn(|index| elf[at + index]);
        if little { u64::from_le_bytes(bytes) } else { u64::from_be_bytes(bytes) }
    };
    // e_shoff + e_shentsize * e_shnum, at the offsets of each class.
    let end = match elf[4] {
        1 => u32_at(0x20) + u16_at(0x2e) * u16_at(0x30),
        2 => u64_at(0x28).checked_add(u16_at(0x3a) * u16_at(0x3c)).context("ELF section headers")?,
        _ => bail!("unknown ELF class"),
    };
    let mut magic = [0; 4];
    file.seek(SeekFrom::Start(end))?;
    file.read_exact(&mut magic).context("no squashfs image")?;
    ensure!(magic == *b"hsqs", "no squashfs image after the runtime");
    Ok(end)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use backhand::{FilesystemWriter, NodeHeader};

    use super::*;

    /// An ELF64 header whose section headers end at byte 64, then a squashfs
    /// image with `files` and `links`.
    fn fixture(files: &[(&str, &[u8])], links: &[(&str, &str)]) -> Vec<u8> {
        let mut elf = vec![0; 64];
        elf[..6].copy_from_slice(b"\x7fELF\x02\x01");
        elf[0x3a] = 64;
        elf[0x3c] = 1;
        let mut image = FilesystemWriter::default();
        for (path, data) in files {
            if let Some((dir, _)) = path.rsplit_once('/') {
                image.push_dir_all(dir, NodeHeader::default()).unwrap();
            }
            image.push_file(Cursor::new(data.to_vec()), path, NodeHeader::default()).unwrap();
        }
        for (path, target) in links {
            image.push_symlink(*target, path, NodeHeader::default()).unwrap();
        }
        let mut out = Cursor::new(elf);
        image.write_with_offset(&mut out, 64).unwrap();
        out.into_inner()
    }

    #[test]
    fn an_appimage_gives_its_dir_icon_through_links() {
        let dir = std::env::temp_dir().join(format!("boxbox-appimage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("App.AppImage");
        let png = super::super::deb::tests::PNG;
        std::fs::write(&path, fixture(
            &[("usr/share/icons/hicolor/256x256/apps/app.png", png), ("app.desktop", b"[Desktop Entry]\nIcon=app\n")],
            &[(".DirIcon", "app.png"), ("app.png", "usr/share/icons/hicolor/256x256/apps/app.png")],
        ))
        .unwrap();
        assert_eq!(icon(&path, 256).unwrap().as_deref(), Some(png));

        // A dangling or looping .DirIcon falls back to the desktop entry, a link
        // here, and its icon in the AppDir's theme.
        let svg = b"<svg xmlns='http://www.w3.org/2000/svg' width='4' height='4'/>";
        std::fs::write(&path, fixture(
            &[("usr/share/applications/app.desktop", b"[Desktop Entry]\nIcon=app\n"), ("usr/share/icons/hicolor/32x32/apps/app.png", png), ("usr/share/icons/hicolor/scalable/apps/app.svg", svg)],
            &[(".DirIcon", "/home/runner/build/App.AppDir/app.png"), ("app.desktop", "usr/share/applications/app.desktop"), ("app.png", "usr/share/icons/hicolor/32x32/apps/app.png")],
        ))
        .unwrap();
        assert_eq!(icon(&path, 256).unwrap().as_deref(), Some(&svg[..]));
        std::fs::write(&path, fixture(&[("app.desktop", b"")], &[(".DirIcon", ".DirIcon")])).unwrap();
        assert_eq!(icon(&path, 256).unwrap(), None);
        std::fs::write(&path, b"#!/bin/sh\n").unwrap();
        assert!(icon(&path, 256).is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    /// AppImages on this machine, when there are any. Run with `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_appimages() {
        let home = std::env::var("HOME").unwrap();
        for dir in [format!("{home}/Applications"), format!("{home}/Downloads"), "/opt".to_string()] {
            for path in super::super::tests::find(Path::new(&dir), "appimage", 2) {
                let started = std::time::Instant::now();
                super::super::tests::report(&path, icon(&path, 256), started);
            }
        }
    }
}
