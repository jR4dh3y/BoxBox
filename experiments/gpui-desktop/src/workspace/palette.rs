use image::{DynamicImage, imageops::FilterType};
use material_colors::{color::{Argb, Lab}, hct::Hct, quantize::{Quantizer, QuantizerMap, QuantizerWu}, score::Score};

// Match frontend/src/lib/theme/wallpaperPalette.ts: fixed Wu centers, Lab assignment, HCT scoring.
pub(crate) fn wallpaper_accent(image: &DynamicImage) -> Option<u32> {
    let pixels = image.resize_exact(48, 48, FilterType::Triangle).to_rgba8();
    let opaque: Vec<_> = pixels.pixels().filter(|pixel| pixel[3] >= 128)
        .map(|pixel| Argb::new(255, pixel[0], pixel[1], pixel[2])).collect();
    if opaque.is_empty() { return None; }
    let centers: Vec<_> = QuantizerWu::quantize(&opaque, 64).color_to_count.keys()
        .map(|color| (*color, Lab::from(*color))).collect();
    let counts = QuantizerMap::quantize(&opaque, 64).color_to_count;
    let mut candidates = counts.clone();
    candidates.clear();
    for (color, count) in counts {
        let point = Lab::from(color);
        let nearest = centers.iter().min_by(|(_, left), (_, right)| {
            distance(point, *left).total_cmp(&distance(point, *right))
        });
        if let Some((color, _)) = nearest { *candidates.entry(*color).or_insert(0) += count; }
    }
    candidates.retain(|color, _| {
        let hct = Hct::new(*color);
        hct.get_chroma() >= 12. && (12. ..=98.).contains(&hct.get_tone())
    });
    if candidates.is_empty() { return None; }
    let seed = Score::score(&candidates, Some(1), Some(Argb::new(0, 0, 0, 0)), None).first().copied()?;
    let source = Hct::new(seed);
    let accent = Argb::from(Hct::from(source.get_hue(), source.get_chroma().clamp(48., 72.), 65.));
    Some(u32::from(accent.red) << 16 | u32::from(accent.green) << 8 | u32::from(accent.blue))
}

fn distance(left: Lab, right: Lab) -> f64 {
    (left.l - right.l).powi(2) + (left.a - right.a).powi(2) + (left.b - right.b).powi(2)
}
