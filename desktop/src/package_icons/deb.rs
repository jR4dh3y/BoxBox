//! A deb's files: an `ar` archive whose `data.tar`, plain or compressed with
//! gzip, xz or zstd, holds what it installs.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{bail, ensure, Context as _};

use super::archive::{normalize, Flow, Member, Visitor};

pub(super) fn walk(path: &Path, visit: Visitor) -> anyhow::Result<()> {
    let mut file = BufReader::new(File::open(path)?);
    let mut magic = [0; 8];
    file.read_exact(&mut magic)?;
    ensure!(&magic == b"!<arch>\n", "not an ar archive");
    loop {
        // name[16] mtime[12] uid[6] gid[6] mode[8] size[10] magic[2]
        let mut header = [0; 60];
        file.read_exact(&mut header).context("no data.tar")?;
        let name = std::str::from_utf8(&header[..16])?.trim_end().trim_end_matches('/');
        let size: u64 = std::str::from_utf8(&header[48..58])?.trim().parse().context("member size")?;
        if let Some(compression) = name.strip_prefix("data.tar") {
            let data = (&mut file).take(size);
            return match compression {
                "" => walk_tar(data, visit),
                ".gz" => walk_tar(flate2::read::MultiGzDecoder::new(data), visit),
                ".xz" => walk_tar(liblzma::read::XzDecoder::new(data), visit),
                ".zst" => walk_tar(zstd::stream::read::Decoder::new(data)?, visit),
                other => bail!("data.tar{other} is not supported"),
            };
        }
        // Members start on an even offset.
        file.seek_relative(i64::try_from(size + size % 2)?)?;
    }
}

fn walk_tar(reader: impl Read, visit: Visitor) -> anyhow::Result<()> {
    let mut archive = tar::Archive::new(reader);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().into_owned();
        let path = normalize(&path);
        let target = entry.link_name()?.map(|target| target.to_string_lossy().into_owned());
        let flow = match (entry.header().entry_type(), target) {
            (tar::EntryType::Regular | tar::EntryType::Continuous, _) => {
                let size = entry.size();
                visit(Member::File { path, size, data: &mut entry })?
            }
            (tar::EntryType::Symlink, Some(target)) => visit(Member::Link { path, target: &target })?,
            // A hard link names another member from the root.
            (tar::EntryType::Link, Some(target)) => visit(Member::Link { path, target: &format!("/{}", normalize(&target)) })?,
            _ => Flow::Continue,
        };
        if let Flow::Stop = flow {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use std::io::Write as _;

    use super::*;

    /// A deb with `files` and symbolic `links` in its gzip data.tar.
    pub(crate) fn fixture(files: &[(&str, &[u8])], links: &[(&str, &str)]) -> Vec<u8> {
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        for (path, data) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            tar.append_data(&mut header, path, *data).unwrap();
        }
        for (path, target) in links {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            tar.append_link(&mut header, path, target).unwrap();
        }
        let data = tar.into_inner().unwrap().finish().unwrap();
        let mut deb = b"!<arch>\n".to_vec();
        for (name, body) in [("debian-binary", &b"2.0\n"[..]), ("control.tar.gz", &[0; 3][..]), ("data.tar.gz", &data[..])] {
            writeln!(deb, "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`", name, 0, 0, 0, 100644, body.len()).unwrap();
            deb.extend_from_slice(body);
            if body.len() % 2 == 1 {
                deb.push(b'\n');
            }
        }
        deb
    }

    pub(crate) const PNG: &[u8] = b"\x89PNG\r\n\x1a\n not decoded here";

    #[test]
    fn a_deb_gives_its_desktop_entrys_icon() {
        let dir = std::env::temp_dir().join(format!("boxbox-deb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.deb");
        let desktop = b"[Desktop Entry]\nName=App\nIcon=app\n";
        std::fs::write(&path, fixture(
            &[
                ("./usr/share/icons/hicolor/32x32/apps/app.png", b"small"),
                ("./opt/App/icon.png", PNG),
                ("./usr/share/applications/app.desktop", desktop),
            ],
            &[("./usr/share/icons/hicolor/512x512/apps/app.png", "../../../../../../opt/App/icon.png")],
        ))
        .unwrap();
        let icon = super::super::archive::icon(|visit| walk(&path, visit), 256).unwrap();
        assert_eq!(icon.as_deref(), Some(PNG));
        std::fs::write(&path, b"!<arch>\n").unwrap();
        assert!(super::super::archive::icon(|visit| walk(&path, visit), 256).is_err());
        std::fs::remove_dir_all(dir).ok();
    }

    /// Real debs on this machine, when there are any. Run with `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_debs() {
        let home = std::env::var("HOME").unwrap();
        for dir in [format!("{home}/Downloads"), format!("{home}/.cache/paru/clone")] {
            for path in super::super::tests::find(Path::new(&dir), "deb", 3) {
                let started = std::time::Instant::now();
                let icon = super::super::archive::icon(|visit| walk(&path, visit), 256);
                super::super::tests::report(&path, icon, started);
            }
        }
    }
}
