//! The list's column widths and how dragging a border between two columns
//! changes them, as in Nautilus and Dolphin.

/// Type, Size and Modified as shares of the row; Name takes the rest, so every
/// column scales with the pane.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ColumnShares(pub [f32; 3]);

/// `table-fixed` in FileList.svelte: Type 18%, Size 15% and Modified 25%.
const DEFAULT: [f32; 3] = [0.18, 0.15, 0.25];
/// The narrowest a column gets by dragging, in pixels.
const MIN_COLUMN: f32 = 56.;
const MIN_NAME: f32 = 120.;

impl Default for ColumnShares {
    fn default() -> Self {
        Self(DEFAULT)
    }
}

impl ColumnShares {
    /// The shares to draw: these, unless a hand-edited settings file left Name no room.
    pub fn usable(self) -> Self {
        let valid = self.0.iter().all(|share| share.is_finite() && *share > 0.) && self.0.iter().sum::<f32>() < 0.9;
        if valid { self } else { Self::default() }
    }

    /// Where each border between columns sits, as a share of the row: Name|Type,
    /// Type|Size and Size|Modified.
    pub fn borders(self) -> [f32; 3] {
        let [kind, size, _] = self.0;
        let name = 1. - self.0.iter().sum::<f32>();
        [name, name + kind, name + kind + size]
    }

    /// The shares with `border` (an index into `borders`) moved to `x` pixels
    /// into a row `width` pixels wide. The two columns beside the border trade
    /// width, so only that border moves; neither gets narrower than a minimum.
    pub fn resized(self, border: usize, x: f32, width: f32) -> Self {
        if width <= 0. || border > 2 {
            return self;
        }
        let mut widths = self.0.map(|share| share * width);
        let name = width - widths.iter().sum::<f32>();
        // The left column's start and the pair's combined width.
        let (start, pair, left_min) = match border {
            0 => (0., name + widths[0], MIN_NAME),
            _ => (name + widths[..border - 1].iter().sum::<f32>(), widths[border - 1] + widths[border], MIN_COLUMN),
        };
        let (low, high) = (left_min, pair - MIN_COLUMN);
        if low > high {
            return self;
        }
        let left = (x - start).clamp(low, high);
        widths[border] = pair - left;
        if border > 0 {
            widths[border - 1] = left;
        }
        Self(widths.map(|pixels| pixels / width))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4)
    }

    #[test]
    fn a_border_follows_the_pointer_and_only_its_neighbours_change() {
        let shares = ColumnShares::default();
        // Type|Size starts at 0.42 + 0.18 = 0.60 of 1000px; drag it to 650px.
        let moved = shares.resized(1, 650., 1000.);
        assert!(close(moved.0, [0.23, 0.10, 0.25]), "{:?}", moved.0);
        assert!((moved.borders()[1] - 0.65).abs() < 1e-4);
        assert!((moved.borders()[0] - shares.borders()[0]).abs() < 1e-4, "Name keeps its width");
    }

    #[test]
    fn dragging_name_wider_takes_from_type() {
        let moved = ColumnShares::default().resized(0, 500., 1000.);
        assert!(close(moved.0, [0.10, 0.15, 0.25]), "{:?}", moved.0);
    }

    #[test]
    fn columns_keep_a_minimum() {
        let moved = ColumnShares::default().resized(2, 10_000., 1000.);
        assert!((moved.0[2] * 1000. - MIN_COLUMN).abs() < 1e-3, "{:?}", moved.0);
        let moved = ColumnShares::default().resized(0, 0., 1000.);
        assert!((moved.borders()[0] * 1000. - MIN_NAME).abs() < 1e-3, "{:?}", moved.0);
    }

    #[test]
    fn a_settings_file_that_leaves_name_no_room_is_ignored() {
        assert_eq!(ColumnShares([0.5, 0.3, 0.3]).usable(), ColumnShares::default());
        assert_eq!(ColumnShares([f32::NAN, 0.1, 0.1]).usable(), ColumnShares::default());
        assert_eq!(ColumnShares([0.2, 0.1, 0.2]).usable(), ColumnShares([0.2, 0.1, 0.2]));
    }
}
