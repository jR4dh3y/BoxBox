//! `lib/components/FileGrid.svelte`: grid metrics and one tile. The view owns
//! virtualisation, selection and events.

use std::sync::Arc;

use gpui::{div, img, prelude::*, px, AnyElement, App, ClickEvent, Div, ElementId, ObjectFit, RenderImage, SharedString, Stateful, Window};

use super::file_list::{checkbox, EntryIcon, Selection};
use super::ROUNDED;
use crate::theme::*;
use crate::ui::thin_icon;

/// A grid zoom step: tile width, row height, column gap, outer padding, and the
/// preview box and icon inside a tile. The name and size lines stay the same.
#[derive(Clone, Copy)]
pub struct GridMetrics {
    pub tile_width: f32,
    pub row_height: f32,
    pub gap: f32,
    pub padding: f32,
    pub preview_height: f32,
    /// The longest edge of a thumbnail drawn at this size.
    pub thumbnail: u32,
}

impl GridMetrics {
    /// Step 1 is the web app's `grid h-[164px] gap-x-3` in `p-3`; step 0 its compact one.
    pub const STEPS: [Self; 5] = [
        Self { tile_width: 96., row_height: 136., gap: 8., padding: 8., preview_height: 56., thumbnail: 160 },
        Self { tile_width: 112., row_height: 164., gap: 12., padding: 12., preview_height: 80., thumbnail: 160 },
        Self { tile_width: 144., row_height: 196., gap: 12., padding: 12., preview_height: 112., thumbnail: 160 },
        Self { tile_width: 192., row_height: 244., gap: 12., padding: 12., preview_height: 160., thumbnail: 256 },
        Self { tile_width: 256., row_height: 308., gap: 12., padding: 12., preview_height: 224., thumbnail: 256 },
    ];

    pub fn new(zoom: u8) -> Self {
        Self::STEPS[usize::from(zoom).min(Self::STEPS.len() - 1)]
    }

    /// The edge of an icon: the preview box, less a margin, never wider than the tile.
    pub fn theme_icon(&self) -> f32 {
        (self.preview_height - 8.).min(self.tile_width - 16.)
    }

    /// A tile's height: the row without the padding folded into it.
    pub fn tile_height(&self) -> f32 {
        self.row_height - self.padding
    }

    /// `columnsFor` in `virtualWindow.ts`, for the pane's full width.
    pub fn columns(&self, width: f32) -> usize {
        let inner = width - self.padding * 2.;
        (((inner + self.gap) / (self.tile_width + self.gap)).floor() as usize).max(1)
    }
}

/// An item drawn inside a folder's tile: a picture's thumbnail, or a plain
/// sheet for anything else, so a folder of documents reads as full but quiet.
pub enum PeekCard {
    Image(Arc<RenderImage>),
    Sheet,
}

/// The folder icon of edge `size` in colour `tone`, with `cards` cascaded
/// inside it, front first: the front one lowest and leftmost.
fn peeking(folder: impl IntoElement, cards: Vec<PeekCard>, size: f32, tone: gpui::Rgba) -> Div {
    let (width, height, step) = (size * 0.4, size * 0.3, size * 0.055);
    let count = cards.len();
    // The folder's body, below its tab, in the 24-unit icon: x 2-22, y 6-20.
    let (center_x, center_y) = (size * 0.5, size * 0.56);
    let cards = cards.into_iter().enumerate().rev().map(|(index, card)| {
        // 0 at the front; the middle card sits at the body's centre.
        let depth = index as f32 - (count as f32 - 1.) / 2.;
        let frame = div()
            .absolute()
            .left(px(center_x - width / 2. + depth * step))
            .top(px(center_y - height / 2. - depth * step * 0.8))
            .w(px(width))
            .h(px(height))
            .overflow_hidden()
            .rounded(px(2.));
        match card {
            PeekCard::Image(image) => frame.border_1().border_color(alpha_of(tone, 0.35)).child(img(image).size_full().object_fit(ObjectFit::Cover)),
            PeekCard::Sheet => frame.border_1().border_color(alpha_of(tone, 0.45)).bg(alpha_of(tone, 0.08)),
        }
    });
    div().relative().size(px(size)).flex_none().child(folder).children(cards)
}

type Decorate = Box<dyn FnOnce(Stateful<Div>) -> Stateful<Div>>;
type CheckHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct FileTile {
    id: ElementId,
    name: SharedString,
    is_dir: bool,
    icon: &'static str,
    detail: SharedString,
    thumbnail: Option<Arc<RenderImage>>,
    /// What a folder holds, drawn inside it; empty for a file or an empty folder.
    peek: Vec<PeekCard>,
    /// The app that opens the file, shown small on its thumbnail.
    badge: Option<Arc<RenderImage>>,
    image: EntryIcon,
    tint: Option<u32>,
    emblem: Option<super::Emblem>,
    selected: bool,
    active: bool,
    cut: bool,
    drop_target: bool,
    metrics: GridMetrics,
    selection: Selection,
    checkbox: Option<CheckHandler>,
    decorate: Option<Decorate>,
    editor: Option<AnyElement>,
}

impl FileTile {
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>, is_dir: bool, selection: Selection) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            is_dir,
            icon: "file",
            detail: SharedString::default(),
            thumbnail: None,
            peek: Vec::new(),
            badge: None,
            image: EntryIcon::Glyph,
            tint: None,
            emblem: None,
            selected: false,
            active: false,
            cut: false,
            drop_target: false,
            metrics: GridMetrics::new(1),
            selection,
            checkbox: None,
            decorate: None,
            editor: None,
        }
    }

    /// Draws `editor` in place of the name, for a rename in place.
    pub fn editor(mut self, editor: impl IntoElement) -> Self {
        self.editor = Some(editor.into_any_element());
        self
    }

    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = icon;
        self
    }

    /// The line under the name: the size for files, empty for folders.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = detail.into();
        self
    }

    /// A small app icon at the thumbnail's bottom left, as Windows marks a preview with its app.
    pub fn badge(mut self, badge: Option<Arc<RenderImage>>) -> Self {
        self.badge = badge;
        self
    }

    pub fn peek(mut self, peek: Vec<PeekCard>) -> Self {
        self.peek = peek;
        self
    }

    pub fn thumbnail(mut self, thumbnail: Option<Arc<RenderImage>>) -> Self {
        self.thumbnail = thumbnail;
        self
    }

    /// BoxBox's own glyph: its kind's colour and a special folder's emblem.
    pub fn tint(mut self, tint: Option<u32>, emblem: Option<super::Emblem>) -> Self {
        self.tint = tint;
        self.emblem = emblem;
        self
    }

    pub fn image(mut self, image: EntryIcon) -> Self {
        self.image = image;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn cut(mut self, cut: bool) -> Self {
        self.cut = cut;
        self
    }

    /// A dragged item is over this folder and would be dropped into it.
    pub fn drop_target(mut self, drop_target: bool) -> Self {
        self.drop_target = drop_target;
        self
    }

    pub fn zoom(mut self, zoom: u8) -> Self {
        self.metrics = GridMetrics::new(zoom);
        self
    }

    /// Shows the selection checkbox on hover, and keeps it while selected.
    pub fn checkbox(mut self, on_check: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.checkbox = Some(Box::new(on_check));
        self
    }

    /// Lets the view attach behaviour (clicks, drag, context menu) to the tile.
    pub fn decorate(mut self, decorate: impl FnOnce(Stateful<Div>) -> Stateful<Div> + 'static) -> Self {
        self.decorate = Some(Box::new(decorate));
        self
    }
}

impl RenderOnce for FileTile {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Selection { background, accent } = self.selection;
        let metrics = self.metrics;
        let group = SharedString::from(format!("file-tile-{}", self.id));
        let tone = c(self.tint.unwrap_or(if self.is_dir { FOLDER } else { TEXT_SECONDARY }));
        // As tall as the zoom step says. A thumbnail sits in thumbnailClass's frame
        // (rounded border bg-surface-primary); an icon stands on its own, as in Thunar.
        let preview = div()
            .h(px(metrics.preview_height))
            .w_full()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .map(|slot| match (self.thumbnail, self.image) {
                (Some(image), _) => {
                    // A quarter of the icon's size, kept legible on the smallest tiles.
                    let badge = (metrics.theme_icon() * 0.28).round().clamp(16., 32.);
                    slot.relative()
                        .rounded(px(ROUNDED))
                        .border_1()
                        .border_color(c(BORDER_PRIMARY))
                        .bg(c(SURFACE_PRIMARY))
                        .child(img(image).size_full().object_fit(ObjectFit::Cover))
                        .when_some(self.badge, |slot, app| {
                            slot.child(div().absolute().left(px(4.)).bottom(px(4.)).size(px(badge)).child(img(app).size_full()))
                        })
                }
                (None, EntryIcon::Theme(image)) if !self.peek.is_empty() => slot.child(peeking(img(image).size(px(metrics.theme_icon())), self.peek, metrics.theme_icon(), tone)),
                (None, EntryIcon::Theme(image)) => slot.child(img(image).size(px(metrics.theme_icon()))),
                (None, EntryIcon::Waiting) => slot,
                (None, EntryIcon::Glyph) if !self.peek.is_empty() => slot.child(peeking(thin_icon(self.icon, metrics.theme_icon(), tone), self.peek, metrics.theme_icon(), tone)),
                (None, EntryIcon::Glyph) => slot.child(super::file_list::emblemed(thin_icon(self.icon, metrics.theme_icon(), tone), self.emblem, metrics.theme_icon(), tone)),
            });
        let label = div()
            .w_full()
            .mt(px(6.))
            .min_w_0()
            .overflow_hidden()
            .map(|label| match self.editor {
                Some(editor) => label.child(div().text_size(px(13.)).child(editor)),
                // text-[13px] leading-4, two lines at most, and always two lines tall, so
                // the size under it lines up across a row (issue #186).
                None => {
                    let font = cx.try_global::<super::UiFont>().map_or_else(|| window.text_style().font(), |ui| gpui::font(ui.0.clone()));
                    // The tile's border and padding.
                    let width = metrics.tile_width - 18.;
                    let (first, second) = super::tile_label::lines(&self.name, width, &font, 13., window);
                    label.child(
                        div()
                            .w_full()
                            .h(px(32.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .text_size(px(13.))
                            .line_height(px(16.))
                            .text_color(c(TEXT_PRIMARY))
                            .child(div().whitespace_nowrap().child(first))
                            .children(second.map(|second| div().whitespace_nowrap().child(second))),
                    )
                }
            })
            .child(
                // mt-0.5 truncate text-[11px] leading-3.5. GPUI's ellipsis collapses centred
                // text to "…", so this clips instead.
                div()
                    .w_full()
                    .mt(px(2.))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_center()
                    .text_size(px(11.))
                    .line_height(px(14.))
                    .text_color(c(if self.selected { TEXT_PRIMARY } else { TEXT_MUTED }))
                    .child(self.detail),
            );
        let feedback_id = self.id.clone();
        // The cursor tile and a drop target keep their own fill; the others ease in a
        // hover wash, over the selection when selected.
        let washed = !(self.active && !self.selected) && !self.drop_target;
        // tileClass: relative flex flex-col rounded border p-2, the row less its padding
        let tile = div()
            .id(self.id)
            .group(group.clone())
            .relative()
            .w(px(metrics.tile_width))
            .h(px(metrics.tile_height()))
            .flex_none()
            .flex()
            .flex_col()
            .min_w_0()
            .rounded(px(ROUNDED))
            .border_1()
            .p(px(8.))
            .cursor_default()
            .when(self.cut, |tile| tile.opacity(0.5))
            .map(|tile| match (self.selected, self.active) {
                (true, _) => tile.border_color(alpha_of(accent, 0.7)),
                (false, true) => tile.border_color(gpui::transparent_black()).bg(c(SURFACE_SECONDARY)),
                (false, false) => tile.border_color(gpui::transparent_black()),
            })
            .when(self.drop_target, |tile| tile.border_color(accent).bg(background))
            .child(preview)
            .child(label)
            .when_some(self.checkbox, |tile, on_check| {
                // absolute top-2 left-2 h-4 w-4, shown on tile hover
                tile.child(
                    // Grows with the zoom, from 16px at the smallest tiles to 28px.
                    checkbox(self.selected, accent, (metrics.theme_icon() * 0.2).round().clamp(16., 28.))
                        .absolute()
                        .top(px(8.))
                        .left(px(8.))
                        .when(!self.selected, |el| el.invisible().group_hover(group, |style| style.visible()))
                        .on_click(move |event, window, cx| {
                            cx.stop_propagation();
                            on_check(event, window, cx);
                        }),
                )
            });
        let tile = match self.decorate {
            Some(decorate) => decorate(tile).into_any_element(),
            None => tile.into_any_element(),
        };
        let feedback = super::motion::interact(feedback_id, tile).rounded(ROUNDED).without_press().selected(self.selected.then_some(background));
        match washed {
            true => feedback.fill(super::motion::hover_fill()).into_any_element(),
            false => feedback.into_any_element(),
        }
    }
}

fn alpha_of(color: gpui::Rgba, a: f32) -> gpui::Rgba {
    gpui::Rgba { a: color.a * a, ..color }
}
