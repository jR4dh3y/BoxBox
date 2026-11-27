//! Read-only text the mouse can select, for values worth copying, such as a
//! file's location in the Details panel. Its owner keeps the one selection.

use std::ops::Range;

use gpui::{HighlightStyle, Pixels, Point, SharedString, StyledText, TextLayout};

use crate::theme::*;

/// What is selected in one piece of read-only text.
#[derive(Clone, Debug, PartialEq)]
pub struct TextSelection {
    id: SharedString,
    text: SharedString,
    anchor: usize,
    head: usize,
    /// The left button is still down, so moving the mouse extends it.
    pub dragging: bool,
}

impl TextSelection {
    /// From a press at byte `at`; a double-click takes the whole text.
    pub fn start(id: SharedString, text: SharedString, at: usize, whole: bool) -> Self {
        let (anchor, head) = if whole { (0, text.len()) } else { (at, at) };
        Self { id, text, anchor, head, dragging: !whole }
    }

    pub fn extend(&mut self, to: usize) {
        self.head = to;
    }

    pub fn is_of(&self, id: &str, text: &str) -> bool {
        self.id.as_ref() == id && self.text.as_ref() == text
    }

    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    /// The selected text, when there is any.
    pub fn selected(&self) -> Option<&str> {
        Some(&self.text[self.range()]).filter(|text| !text.is_empty())
    }
}

/// The text with `range` highlighted as a selection.
pub fn selectable_text(text: SharedString, range: Option<Range<usize>>) -> StyledText {
    let highlight = HighlightStyle { background_color: Some(c(SELECTION).into()), ..Default::default() };
    StyledText::new(text).with_highlights(range.filter(|range| !range.is_empty()).map(|range| (range, highlight)))
}

/// The byte in `text` nearest to `position`, on a character boundary.
pub fn index_at(layout: &TextLayout, text: &str, position: Point<Pixels>) -> usize {
    let index = layout.index_for_position(position).unwrap_or_else(|nearest| nearest).min(text.len());
    (0..=index).rev().find(|&index| text.is_char_boundary(index)).unwrap_or(0)
}
