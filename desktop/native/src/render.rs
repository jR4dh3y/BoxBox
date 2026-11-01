use std::io::{BufRead, Seek};

use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer, images::Image};
use image::{
    ExtendedColorType, ImageDecoder, ImageError, ImageReader, Limits, codecs::jpeg::JpegEncoder,
};

pub const MAX_SOURCE_BYTES: u64 = 64 << 20;
const MAX_PIXELS: u64 = 32_000_000;

#[derive(Debug, PartialEq)]
pub enum RenderError {
    Invalid,
    Limit,
}

fn image_error(error: ImageError) -> RenderError {
    match error {
        ImageError::Limits(_) => RenderError::Limit,
        _ => RenderError::Invalid,
    }
}

pub fn fit(width: u32, height: u32, max_width: u32, max_height: u32) -> (u32, u32) {
    if width <= max_width && height <= max_height {
        (width, height)
    } else if u64::from(width) * u64::from(max_height) > u64::from(height) * u64::from(max_width) {
        (max_width, (height * max_width / width).max(1))
    } else {
        ((width * max_height / height).max(1), max_height)
    }
}

pub fn render<R: BufRead + Seek>(
    source: R,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, RenderError> {
    if !(16..=512).contains(&width) || !(16..=512).contains(&height) {
        return Err(RenderError::Invalid);
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 << 20);
    let mut reader = ImageReader::new(source)
        .with_guessed_format()
        .map_err(|_| RenderError::Invalid)?;
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(image_error)?;
    let (source_width, source_height) = decoder.dimensions();
    if source_width == 0
        || source_height == 0
        || u64::from(source_width) * u64::from(source_height) > MAX_PIXELS
    {
        return Err(RenderError::Limit);
    }
    let decoded = image::DynamicImage::from_decoder(decoder).map_err(image_error)?;
    let (target_width, target_height) = fit(source_width, source_height, width, height);
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::CatmullRom));
    let mut resizer = Resizer::new();
    let rgb = if decoded.color().has_alpha() {
        let source = decoded.into_rgba8();
        let mut target = Image::new(target_width, target_height, PixelType::U8x4);
        resizer
            .resize(&source, &mut target, &options)
            .map_err(|_| RenderError::Invalid)?;
        // Match the existing JPEG thumbnail's black matte for transparency.
        target
            .buffer()
            .chunks_exact(4)
            .flat_map(|pixel| {
                let alpha = u16::from(pixel[3]);
                [pixel[0], pixel[1], pixel[2]]
                    .map(|channel| ((u16::from(channel) * alpha + 127) / 255) as u8)
            })
            .collect::<Vec<_>>()
    } else {
        let source = decoded.into_rgb8();
        let mut target = Image::new(target_width, target_height, PixelType::U8x3);
        resizer
            .resize(&source, &mut target, &options)
            .map_err(|_| RenderError::Invalid)?;
        target.into_vec()
    };
    let mut output = Vec::new();
    JpegEncoder::new_with_quality(&mut output, 82)
        .encode(&rgb, target_width, target_height, ExtendedColorType::Rgb8)
        .map_err(image_error)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    #[test]
    fn alpha_is_matted_and_images_are_aspect_fitted() {
        let mut input = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(640, 400, Rgba([255, 0, 0, 128])))
            .write_to(&mut input, ImageFormat::Png)
            .unwrap();
        input.set_position(0);
        let result = render(input, 224, 144).unwrap();
        let decoded = image::load_from_memory(&result).unwrap().into_rgb8();
        assert_eq!(decoded.dimensions(), (224, 140));
        let pixel = decoded.get_pixel(100, 50);
        assert!((120..=136).contains(&pixel[0]));
        assert!(pixel[1] < 8 && pixel[2] < 8);
    }

    #[test]
    fn rejects_invalid_sources_and_sizes() {
        assert_eq!(
            render(Cursor::new(b"not an image"), 224, 144),
            Err(RenderError::Invalid)
        );
        assert_eq!(
            render(Cursor::new(b"not an image"), 0, 144),
            Err(RenderError::Invalid)
        );
        assert_eq!(fit(3000, 2000, 224, 144), (216, 144));
        assert_eq!(fit(2000, 3000, 224, 144), (96, 144));
        assert_eq!(fit(32, 16, 224, 144), (32, 16));
    }

    #[test]
    fn supports_every_advertised_raster_decoder() {
        for format in [
            ImageFormat::Jpeg,
            ImageFormat::Png,
            ImageFormat::Gif,
            ImageFormat::WebP,
            ImageFormat::Bmp,
            ImageFormat::Tiff,
            ImageFormat::Ico,
            ImageFormat::Qoi,
        ] {
            let mut input = Cursor::new(Vec::new());
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(64, 32, Rgba([20, 40, 80, 255])))
                .write_to(&mut input, format)
                .unwrap();
            input.set_position(0);
            let bytes =
                render(input, 32, 32).unwrap_or_else(|error| panic!("{format:?}: {error:?}"));
            assert_eq!(
                image::load_from_memory(&bytes)
                    .unwrap()
                    .into_rgb8()
                    .dimensions(),
                (32, 16),
                "{format:?}"
            );
        }
    }

    #[test]
    fn rejects_oversized_headers_before_decoding_pixels() {
        for (width, height) in [(8193_u32, 1_u32), (8000, 5000)] {
            let mut encoded = Cursor::new(Vec::new());
            DynamicImage::new_rgb8(1, 1)
                .write_to(&mut encoded, ImageFormat::Png)
                .unwrap();
            let mut bytes = encoded.into_inner();
            bytes[16..20].copy_from_slice(&width.to_be_bytes());
            bytes[20..24].copy_from_slice(&height.to_be_bytes());
            let mut crc = u32::MAX;
            for byte in &bytes[12..29] {
                crc ^= u32::from(*byte);
                for _ in 0..8 {
                    crc = (crc >> 1) ^ (0xedb88320 & (0_u32.wrapping_sub(crc & 1)));
                }
            }
            bytes[29..33].copy_from_slice(&(!crc).to_be_bytes());
            assert_eq!(
                render(Cursor::new(bytes), 224, 144),
                Err(RenderError::Limit)
            );
        }
    }
}
