//! One line of text cut short with "…".

use gpui::Styled;

/// For text in a box whose width comes from its parent, such as a table cell
/// or a `flex_1` row. GPUI 0.2 keeps a no-wrap text's first measured size,
/// taken before its width is known, so `truncate()` clips without an
/// ellipsis. Wrapped text is measured again at each width, so a one-line clamp
/// gets its "…". Text sized by its own content keeps `truncate()`: probed at
/// its smallest width, this would shrink it to "…".
pub trait Ellipsis: Styled + Sized {
    fn ellipsis(self) -> Self {
        self.whitespace_normal().text_ellipsis().line_clamp(1)
    }
}

impl<T: Styled> Ellipsis for T {}
