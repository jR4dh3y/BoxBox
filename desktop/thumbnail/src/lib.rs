#[cfg(target_os = "linux")]
mod media;
mod render;

pub use render::{decode_png, decode_supported, dimensions_supported, encode_png};
use render::{MAX_SOURCE_BYTES, RenderError};
use std::{
    fs::File,
    io::{BufReader, Cursor},
};

#[derive(Debug)]
pub enum ThumbnailError {
    UnsupportedFormat,
    InvalidInput,
    LimitExceeded,
    Io(std::io::Error),
}

impl std::fmt::Display for ThumbnailError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedFormat => formatter.write_str("unsupported thumbnail format"),
            Self::InvalidInput => formatter.write_str("invalid thumbnail input"),
            Self::LimitExceeded => formatter.write_str("thumbnail input exceeds size limits"),
            Self::Io(error) => write!(formatter, "thumbnail I/O error: {error}"),
        }
    }
}

impl std::error::Error for ThumbnailError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RenderError> for ThumbnailError {
    fn from(error: RenderError) -> Self {
        match error {
            RenderError::Invalid => Self::InvalidInput,
            RenderError::Limit => Self::LimitExceeded,
        }
    }
}

/// Renders a local image, video, audio file, or PDF as bounded PNG thumbnail bytes.
pub fn thumbnail_path(
    path: &std::path::Path,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, ThumbnailError> {
    if !(16..=512).contains(&width) || !(16..=512).contains(&height) {
        return Err(ThumbnailError::InvalidInput);
    }
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let kind = match extension.as_str() {
        "bmp" | "gif" | "ico" | "jpg" | "jpeg" | "png" | "qoi" | "tif" | "tiff" | "webp" => 0,
        "mp4" | "m4v" | "mov" | "mkv" | "webm" | "avi" | "mpeg" | "mpg" | "ts" | "m2ts"
        | "flv" => 1,
        "mp3" | "wav" | "flac" | "aac" | "ac3" | "aiff" | "ape" | "wv" | "amr" | "ogg"
        | "oga" | "opus" | "m4a" | "wma" => 2,
        "pdf" => 3,
        _ => return Err(ThumbnailError::UnsupportedFormat),
    };
    let file = File::open(path).map_err(ThumbnailError::Io)?;
    let metadata = file.metadata().map_err(ThumbnailError::Io)?;
    if !metadata.is_file() {
        return Err(ThumbnailError::InvalidInput);
    }
    if kind == 0 {
        if metadata.len() > MAX_SOURCE_BYTES {
            return Err(ThumbnailError::LimitExceeded);
        }
        return render::render_png(BufReader::new(file), width, height)
            .map_err(ThumbnailError::from);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        let fd = u32::try_from(file.as_raw_fd()).map_err(|_| ThumbnailError::InvalidInput)?;
        let frame = media::frame(kind, fd, width, height).map_err(ThumbnailError::from)?;
        render::render_png(Cursor::new(frame), width, height).map_err(ThumbnailError::from)
    }
    #[cfg(not(target_os = "linux"))]
    Err(ThumbnailError::UnsupportedFormat)
}

/// Decodes bounded remote thumbnail bytes and normalizes them to PNG.
pub fn thumbnail_bytes(
    bytes: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, ThumbnailError> {
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(ThumbnailError::LimitExceeded);
    }
    render::render_png(Cursor::new(bytes), width, height).map_err(ThumbnailError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_decoding_reserves_output_and_encoding_converts_float_images() {
        let png = encode_png(&image::DynamicImage::new_rgb32f(64, 64)).unwrap();
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(8 << 10);
        assert!(matches!(decode_png(Cursor::new(&png), limits.clone()), Err(image::ImageError::Limits(_))));
        assert!(matches!(decode_supported(Cursor::new(&png), limits), Err(image::ImageError::Limits(_))));
        let decoded = decode_png(Cursor::new(&png), image::Limits::default()).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 64));
    }

    #[test]
    fn images_render_and_bad_input_is_refused() {
        let path = std::env::temp_dir().join(format!("boxbox-thumbnail-test-{}.png", std::process::id()));
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(32, 16).write_to(&mut encoded, image::ImageFormat::Png).unwrap();
        std::fs::write(&path, encoded.get_ref()).unwrap();
        let png = thumbnail_path(&path, 32, 32).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        assert!(matches!(thumbnail_path(&path, 8, 32), Err(ThumbnailError::InvalidInput)));
        File::options().write(true).open(&path).unwrap().set_len(MAX_SOURCE_BYTES + 1).unwrap();
        assert!(matches!(thumbnail_path(&path, 32, 32), Err(ThumbnailError::LimitExceeded)));
        std::fs::remove_file(path).unwrap();
        assert!(matches!(thumbnail_path(std::path::Path::new("notes.txt"), 32, 32), Err(ThumbnailError::UnsupportedFormat)));
    }
}
