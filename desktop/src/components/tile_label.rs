//! A grid tile's name in two centred lines at most, as Dolphin draws it.
//!
//! GPUI's `line_clamp` keeps every word past the clamp on the last line and
//! clips it, which on centred text shows that line's middle. Here the first
//! line ends at a word break that fits, and a second line too long to fit loses
//! its middle, so the end of the name (`(1).pdf`) stays readable.

use gpui::{px, Font, SharedString, TextRun, Window};

/// Where each character starts, as `(byte index, x)`, and the text's width.
type Measure<'a> = dyn Fn(&str) -> (Vec<(usize, f32)>, f32) + 'a;

const ELLIPSIS: &str = "…";

/// The lines of `name` in `width` pixels of `font` at `size`.
pub fn lines(name: &str, width: f32, font: &Font, size: f32, window: &Window) -> (SharedString, Option<SharedString>) {
    let measure = |text: &str| {
        let run = TextRun { len: text.len(), font: font.clone(), color: gpui::black(), background_color: None, underline: None, strikethrough: None };
        let line = window.text_system().shape_line(SharedString::from(text.to_string()), px(size), &[run], None);
        let starts = line.runs.iter().flat_map(|run| run.glyphs.iter().map(|glyph| (glyph.index, f32::from(glyph.position.x)))).collect();
        (starts, f32::from(line.width))
    };
    let (first, second) = split(name, width, &measure);
    (first.into(), second.map(Into::into))
}

/// The first line, and the rest when it does not fit on one line.
fn split(name: &str, width: f32, measure: &Measure) -> (String, Option<String>) {
    let (starts, total) = measure(name);
    if total <= width {
        return (name.to_string(), None);
    }
    // The first character that does not fit whole, or the second, so a line is never empty.
    let ends = starts.iter().skip(1).map(|&(_, x)| x).chain(std::iter::once(total));
    let overflow = starts
        .iter()
        .zip(ends)
        .find(|&(_, end)| end > width)
        .map_or(name.len(), |(&(index, _), _)| index)
        .max(name.char_indices().nth(1).map_or(name.len(), |(index, _)| index));
    let first_end = word_break(name, overflow).unwrap_or(overflow);
    let first = name[..first_end].trim_end();
    let rest = name[first_end..].trim_start();
    (first.to_string(), (!rest.is_empty()).then(|| fit_middle(rest, width, measure)))
}

/// The last break before `end` that leaves a word's worth on the line: after a
/// space, or after `-`, `_` or `.` inside a long name.
fn word_break(name: &str, end: usize) -> Option<usize> {
    let line = &name[..end];
    let after = |separator: char| line.rmatch_indices(separator).map(|(index, _)| index + separator.len_utf8()).next();
    let space = after(' ');
    let punctuation = ['-', '_', '.'].into_iter().filter_map(after).max();
    space.into_iter().chain(punctuation).max().filter(|&index| index > 0 && index <= end)
}

/// `text` whole if it fits, else its start and end around "…".
fn fit_middle(text: &str, width: f32, measure: &Measure) -> String {
    let (starts, total) = measure(text);
    if total <= width {
        return text.to_string();
    }
    let half = (width - measure(ELLIPSIS).1).max(0.) / 2.;
    // The start keeps characters that end by `half`; the end keeps those that begin
    // `half` before the end.
    let ends = starts.iter().skip(1).copied().chain(std::iter::once((text.len(), total)));
    let head = ends.take_while(|&(_, x)| x <= half).last().map_or(0, |(index, _)| index);
    let tail = starts.iter().find(|&&(_, x)| total - x <= half).map_or(text.len(), |&(index, _)| index);
    format!("{}{ELLIPSIS}{}", text[..head].trim_end(), text[tail.max(head)..].trim_start())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every character one pixel wide.
    fn mono(text: &str) -> (Vec<(usize, f32)>, f32) {
        let starts: Vec<(usize, f32)> = text.char_indices().enumerate().map(|(n, (index, _))| (index, n as f32)).collect();
        let count = starts.len() as f32;
        (starts, count)
    }

    #[test]
    fn a_short_name_is_one_line() {
        assert_eq!(split("soil.hash", 12., &mono), ("soil.hash".into(), None));
    }

    #[test]
    fn the_first_line_ends_at_a_word() {
        assert_eq!(split("Soil Mechanics", 12., &mono), ("Soil".into(), Some("Mechanics".into())));
    }

    #[test]
    fn a_long_second_line_keeps_its_end() {
        let (first, second) = split("Soil Mechanics CivilEnggForAll GATE Study Material (1).pdf", 20., &mono);
        assert_eq!(first, "Soil Mechanics");
        let second = second.unwrap();
        assert!(second.chars().count() <= 20, "{second}");
        assert!(second.starts_with("CivilEngg"), "{second}");
        assert!(second.ends_with("(1).pdf"), "{second}");
        assert!(second.contains('…'));
    }

    #[test]
    fn one_long_word_breaks_where_it_overflows() {
        let (first, second) = split("abcdefghijklmnopqrstuvwxyz", 10., &mono);
        assert_eq!(first, "abcdefghij");
        assert_eq!(second.as_deref(), Some("klmn…wxyz"));
    }

    #[test]
    fn a_name_breaks_after_a_separator() {
        assert_eq!(split("holiday_photos_2024.zip", 16., &mono).0, "holiday_photos_");
    }
}
