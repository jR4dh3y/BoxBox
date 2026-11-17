//! The picture that follows the pointer while files are dragged to another app,
//! as file managers show it: the file's thumbnail or icon, on a second card when
//! there are several.

use gpui::RenderImage;
use resvg::tiny_skia::{FillRule, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};

/// The longest edge of the picture, in logical pixels.
const EDGE: f32 = 64.;
/// How far the card behind is offset when several files are dragged.
const STACK: f32 = 6.;

/// Premultiplied BGRA pixels, as `wl_shm`'s ARGB8888 lays them out in memory.
pub struct DragIcon {
    pub width: u32,
    pub height: u32,
    /// Device pixels per logical pixel; the compositor scales the buffer down by it.
    pub scale: u32,
    pub bgra: Vec<u8>,
}

/// `image` (GPUI's straight BGRA) fitted into the drag picture at `scale`.
pub fn compose(image: &RenderImage, count: usize, scale: u32) -> Option<DragIcon> {
    let size = image.size(0);
    let (width, height) = (u32::try_from(size.width.0).ok()?, u32::try_from(size.height.0).ok()?);
    let source = pixmap(image.as_bytes(0)?, width, height)?;
    let fit = (EDGE * scale as f32) / width.max(height) as f32;
    let fit = fit.min(scale as f32);
    let (drawn_width, drawn_height) = (width as f32 * fit, height as f32 * fit);
    let stack = if count > 1 { STACK * scale as f32 } else { 0. };
    let (canvas_width, canvas_height) = ((drawn_width + stack).ceil() as u32, (drawn_height + stack).ceil() as u32);
    let mut canvas = Pixmap::new(canvas_width.max(1), canvas_height.max(1))?;
    if count > 1 {
        let card = Rect::from_xywh(stack, stack, drawn_width, drawn_height)?;
        let path = PathBuilder::from_rect(card);
        let mut fill = Paint::default();
        fill.set_color_rgba8(240, 240, 240, 230);
        canvas.fill_path(&path, &fill, FillRule::Winding, Transform::identity(), None);
        let mut edge = Paint::default();
        edge.set_color_rgba8(0, 0, 0, 90);
        canvas.stroke_path(&path, &edge, &Stroke { width: scale as f32, ..Stroke::default() }, Transform::identity(), None);
    }
    let paint = PixmapPaint { quality: FilterQuality::Bicubic, ..PixmapPaint::default() };
    canvas.draw_pixmap(0, 0, source.as_ref(), &paint, Transform::from_scale(fit, fit), None);
    let mut bgra = canvas.take();
    for pixel in bgra.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Some(DragIcon { width: canvas_width.max(1), height: canvas_height.max(1), scale, bgra })
}

/// Straight BGRA into tiny-skia's premultiplied RGBA.
fn pixmap(bgra: &[u8], width: u32, height: u32) -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(width, height)?;
    if bgra.len() != pixmap.data().len() {
        return None;
    }
    for (to, from) in pixmap.data_mut().chunks_exact_mut(4).zip(bgra.chunks_exact(4)) {
        let alpha = u16::from(from[3]);
        let premultiply = |value: u8| ((u16::from(value) * alpha + 127) / 255) as u8;
        to.copy_from_slice(&[premultiply(from[2]), premultiply(from[1]), premultiply(from[0]), from[3]]);
    }
    Some(pixmap)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, bgra: [u8; 4]) -> RenderImage {
        let pixels = image::RgbaImage::from_fn(width, height, |_, _| image::Rgba(bgra));
        RenderImage::new(vec![image::Frame::new(pixels)])
    }

    #[test]
    fn a_large_thumbnail_is_fitted_and_premultiplied() {
        // Half-transparent pure red, in BGRA.
        let icon = compose(&solid(256, 128, [0, 0, 255, 128]), 1, 1).unwrap();
        assert_eq!((icon.width, icon.height), (64, 32));
        let middle = ((16 * icon.width + 32) * 4) as usize;
        assert_eq!(&icon.bgra[middle..middle + 4], &[0, 0, 128, 128]);
    }

    #[test]
    fn several_files_add_a_card_and_scale_follows_the_display() {
        let icon = compose(&solid(48, 48, [255, 255, 255, 255]), 3, 2).unwrap();
        // A small icon is not enlarged past its own pixels at this scale.
        assert_eq!((icon.width, icon.height), (96 + 12, 96 + 12));
        assert_eq!(icon.scale, 2);
    }
}
