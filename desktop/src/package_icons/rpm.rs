//! An rpm's files: a 96-byte lead, a signature header padded to 8 bytes, the
//! main header, then a `newc` cpio payload compressed as the main header's
//! PAYLOADCOMPRESSOR tag says.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use anyhow::{bail, ensure, Context as _};

use super::archive::{normalize, Flow, Member, Visitor};

const PAYLOAD_COMPRESSOR: u32 = 1125;
const STRING: u32 = 6;

pub(super) fn walk(path: &Path, visit: Visitor) -> anyhow::Result<()> {
    let mut file = BufReader::new(File::open(path)?);
    let mut lead = [0; 96];
    file.read_exact(&mut lead)?;
    ensure!(lead[..4] == [0xed, 0xab, 0xee, 0xdb], "not an rpm");
    let (_, signature) = header(&mut file, None)?;
    file.seek_relative(i64::try_from((8 - signature % 8) % 8)?)?;
    let (compressor, _) = header(&mut file, Some(PAYLOAD_COMPRESSOR))?;
    // Packages from before the tag are gzip.
    match compressor.as_deref().unwrap_or("gzip") {
        "gzip" => walk_cpio(flate2::read::MultiGzDecoder::new(file), visit),
        "xz" => walk_cpio(liblzma::read::XzDecoder::new(file), visit),
        "lzma" => walk_cpio(liblzma::read::XzDecoder::new_stream(file, liblzma::stream::Stream::new_lzma_decoder(u64::MAX)?), visit),
        "zstd" => walk_cpio(zstd::stream::read::Decoder::new(file)?, visit),
        other => bail!("{other} payloads are not supported"),
    }
}

fn be(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Reads past a header, returning the string tag `wanted` and the header's length.
fn header(file: &mut BufReader<File>, wanted: Option<u32>) -> anyhow::Result<(Option<String>, u64)> {
    let mut intro = [0; 16];
    file.read_exact(&mut intro)?;
    ensure!(intro[..3] == [0x8e, 0xad, 0xe8], "not an rpm header");
    let (count, store) = (u64::from(be(&intro[8..12])), u64::from(be(&intro[12..16])));
    ensure!(count <= 1 << 16 && store <= 256 << 20, "rpm header too large");
    let mut offset = None;
    for _ in 0..count {
        let mut entry = [0; 16];
        file.read_exact(&mut entry)?;
        if Some(be(&entry[..4])) == wanted && be(&entry[4..8]) == STRING {
            offset = Some(u64::from(be(&entry[8..12])));
        }
    }
    let mut read = 0;
    let mut value = None;
    if let Some(offset) = offset.filter(|offset| *offset < store) {
        file.seek_relative(i64::try_from(offset)?)?;
        let mut bytes = Vec::new();
        file.by_ref().take(64).read_until(0, &mut bytes)?;
        read = offset + bytes.len() as u64;
        value = Some(String::from_utf8_lossy(bytes.strip_suffix(&[0]).unwrap_or(&bytes)).into_owned());
    }
    file.seek_relative(i64::try_from(store.checked_sub(read).context("rpm header string")?)?)?;
    Ok((value, 16 + count * 16 + store))
}

fn walk_cpio(mut reader: impl Read, visit: Visitor) -> anyhow::Result<()> {
    let skip = |reader: &mut dyn Read, count: u64| io::copy(&mut reader.take(count), &mut io::sink());
    loop {
        let mut header = [0; 110];
        reader.read_exact(&mut header)?;
        ensure!(&header[..6] == b"070701" || &header[..6] == b"070702", "not a newc cpio payload");
        let field = |index: usize| -> anyhow::Result<u64> { Ok(u64::from_str_radix(std::str::from_utf8(&header[6 + 8 * index..14 + 8 * index])?, 16)?) };
        let (mode, size, name_size) = (field(1)?, field(6)?, field(11)?);
        ensure!(name_size <= 4096, "cpio name too long");
        let mut name = vec![0; name_size as usize];
        reader.read_exact(&mut name)?;
        skip(&mut reader, (4 - (110 + name_size) % 4) % 4)?;
        let name = String::from_utf8_lossy(name.strip_suffix(&[0]).unwrap_or(&name)).into_owned();
        if name == "TRAILER!!!" {
            return Ok(());
        }
        let path = normalize(&name);
        let mut data = reader.by_ref().take(size);
        let flow = match mode & 0o170000 {
            0o100000 => visit(Member::File { path, size, data: &mut data })?,
            0o120000 if size <= 4096 => {
                let mut target = String::new();
                data.read_to_string(&mut target)?;
                visit(Member::Link { path, target: &target })?
            }
            _ => Flow::Continue,
        };
        if let Flow::Stop = flow {
            return Ok(());
        }
        io::copy(&mut data, &mut io::sink())?;
        skip(&mut reader, (4 - size % 4) % 4)?;
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn cpio(entries: &[(&str, u32, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let pad = |out: &mut Vec<u8>| out.resize(out.len().next_multiple_of(4), 0);
        for (name, mode, data) in entries.iter().chain([&("TRAILER!!!", 0, &b""[..])]) {
            write!(out, "070701{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}", 1, mode, 0, 0, 1, 0, data.len(), 0, 0, 0, 0, name.len() + 1, 0).unwrap();
            out.extend_from_slice(name.as_bytes());
            out.push(0);
            pad(&mut out);
            out.extend_from_slice(data);
            pad(&mut out);
        }
        out
    }

    /// An rpm whose payload, compressed with `compressor`, holds `entries`.
    fn fixture(compressor: &str, entries: &[(&str, u32, &[u8])]) -> Vec<u8> {
        let mut rpm = vec![0xed, 0xab, 0xee, 0xdb];
        rpm.resize(96, 0);
        // A signature header with one entry and 5 bytes of store, padded to 8.
        rpm.extend_from_slice(&[0x8e, 0xad, 0xe8, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 5]);
        rpm.extend_from_slice(&[0, 0, 1, 12, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 1]);
        rpm.extend_from_slice(&[1, 2, 3, 4, 5, 0, 0, 0]);
        let store = [b"x86_64\0".as_slice(), compressor.as_bytes(), b"\0"].concat();
        rpm.extend_from_slice(&[0x8e, 0xad, 0xe8, 1, 0, 0, 0, 0, 0, 0, 0, 2]);
        rpm.extend_from_slice(&(store.len() as u32).to_be_bytes());
        rpm.extend_from_slice(&[0, 0, 0x03, 0xfe, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 1]);
        rpm.extend_from_slice(&[0, 0, 0x04, 0x65, 0, 0, 0, 6, 0, 0, 0, 7, 0, 0, 0, 1]);
        rpm.extend_from_slice(&store);
        let payload = cpio(entries);
        let compressed = match compressor {
            "gzip" => {
                let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
                encoder.write_all(&payload).unwrap();
                encoder.finish().unwrap()
            }
            "xz" => {
                let mut encoder = liblzma::write::XzEncoder::new(Vec::new(), 1);
                encoder.write_all(&payload).unwrap();
                encoder.finish().unwrap()
            }
            "zstd" => zstd::stream::encode_all(payload.as_slice(), 1).unwrap(),
            _ => unreachable!(),
        };
        rpm.extend_from_slice(&compressed);
        rpm
    }

    #[test]
    fn an_rpm_gives_its_desktop_entrys_icon() {
        use super::super::deb::tests::PNG;
        let dir = std::env::temp_dir().join(format!("boxbox-rpm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.rpm");
        let svg = b"<svg xmlns='http://www.w3.org/2000/svg' width='4' height='4'/>";
        for compressor in ["gzip", "xz", "zstd"] {
            std::fs::write(&path, fixture(compressor, &[
                ("./usr/share/applications/org.app.App.desktop", 0o100644, b"[Desktop Entry]\nIcon=org.app.App\n"),
                ("./usr/share/icons/hicolor/128x128/apps/org.app.App.png", 0o120777, b"../../256x256/apps/org.app.App.png"),
                ("./usr/share/icons/hicolor/256x256/apps/org.app.App.png", 0o100644, PNG),
                ("./usr/share/icons/hicolor/scalable/apps/org.app.App.svg", 0o100644, svg),
            ]))
            .unwrap();
            assert_eq!(super::super::archive::icon(|visit| walk(&path, visit), 256).unwrap().as_deref(), Some(&svg[..]), "{compressor}");
            std::fs::write(&path, fixture(compressor, &[
                ("./usr/share/applications/org.app.App.desktop", 0o100644, b"[Desktop Entry]\nIcon=org.app.App\n"),
                ("./usr/share/icons/hicolor/512x512/apps/org.app.App.png", 0o120777, b"../../256x256/apps/org.app.App.png"),
                ("./usr/share/icons/hicolor/256x256/apps/org.app.App.png", 0o100644, PNG),
            ]))
            .unwrap();
            assert_eq!(super::super::archive::icon(|visit| walk(&path, visit), 256).unwrap().as_deref(), Some(PNG), "{compressor}");
        }
        std::fs::write(&path, fixture("gzip", &[("./usr/bin/app", 0o100755, b"\x7fELF")])).unwrap();
        assert_eq!(super::super::archive::icon(|visit| walk(&path, visit), 256).unwrap(), None);
        std::fs::remove_dir_all(dir).ok();
    }
}
