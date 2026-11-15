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

/// `color` turned around the hue circle as far as `accent` is from BoxBox's
/// default blue, keeping its own chroma and tone: the tinted icons keep their
/// distinct hues, and the whole set follows the accent, also one taken from
/// the wallpaper. The default accent leaves every colour as it is.
pub(crate) fn follow_accent(color: u32, accent: u32) -> u32 {
    const DEFAULT: u32 = crate::settings_state::AccentColor::Blue.hex();
    if accent == DEFAULT {
        return color;
    }
    let hct = |hex: u32| Hct::new(Argb::new(255, (hex >> 16) as u8, (hex >> 8) as u8, hex as u8));
    let (own, from, to) = (hct(color), hct(DEFAULT), hct(accent));
    let hue = (own.get_hue() + to.get_hue() - from.get_hue()).rem_euclid(360.);
    let shifted = Argb::from(Hct::from(hue, own.get_chroma(), own.get_tone()));
    u32::from(shifted.red) << 16 | u32::from(shifted.green) << 8 | u32::from(shifted.blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tints_turn_with_the_accent_and_stay_apart() {
        let blue = crate::settings_state::AccentColor::Blue.hex();
        assert_eq!(follow_accent(0xf87171, blue), 0xf87171);
        let (red, green) = (follow_accent(0xf87171, 0xf39c12), follow_accent(0x4ade80, 0xf39c12));
        assert_ne!(red, 0xf87171, "another accent moves the colour");
        assert_ne!(red, green, "kinds keep apart");
    }
}
