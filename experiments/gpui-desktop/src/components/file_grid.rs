//! `lib/components/FileGrid.svelte`: grid metrics and one tile. The view owns
//! virtualisation, selection and events.

use std::sync::Arc;

use gpui::{div, img, prelude::*, px, App, ClickEvent, Div, ElementId, Image, ObjectFit, SharedString, Stateful, Window};

use super::file_list::{checkbox, Selection};
use super::ROUNDED;
use crate::theme::*;
use crate::ui::thin_icon;

/// Tile width, row height, column gap and outer padding.
#[derive(Clone, Copy)]
pub struct GridMetrics {
    pub tile_width: f32,
    pub row_height: f32,
    pub gap: f32,
    pub padding: f32,
}

impl GridMetrics {
    /// `grid h-[164px] gap-x-3` in `p-3`, or `h-[136px] gap-x-2` in `p-2` when compact.
    pub fn new(compact: bool) -> Self {
        if compact {
            Self { tile_width: 96., row_height: 136., gap: 8., padding: 8. }
        } else {
            Self { tile_width: 112., row_height: 164., gap: 12., padding: 12. }
        }
    }

    /// `columnsFor` in `virtualWindow.ts`, for the pane's full width.
    pub fn columns(&self, width: f32) -> usize {
        let inner = width - self.padding * 2.;
        (((inner + self.gap) / (self.tile_width + self.gap)).floor() as usize).max(1)
    }
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
    thumbnail: Option<Arc<Image>>,
    selected: bool,
    active: bool,
    cut: bool,
    compact: bool,
    selection: Selection,
    checkbox: Option<CheckHandler>,
    decorate: Option<Decorate>,
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
            selected: false,
            active: false,
            cut: false,
            compact: false,
            selection,
            checkbox: None,
            decorate: None,
        }
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

    pub fn thumbnail(mut self, thumbnail: Option<Arc<Image>>) -> Self {
        self.thumbnail = thumbnail;
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

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
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
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Selection { background, hover, accent } = self.selection;
        let metrics = GridMetrics::new(self.compact);
        let group = SharedString::from(format!("file-tile-{}", self.id));
        let tone = if self.is_dir { FOLDER } else { TEXT_SECONDARY };
        // thumbnailClass: rounded border bg-surface-primary, h-20 (h-14 compact)
        let preview = div()
            .h(px(if self.compact { 56. } else { 80. }))
            .w_full()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .rounded(px(ROUNDED))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_PRIMARY))
            .map(|slot| match self.thumbnail {
                Some(image) => slot.child(img(image).size_full().object_fit(ObjectFit::Cover)),
                None => slot.child(thin_icon(self.icon, if self.compact { 24. } else { 30. }, c(tone))),
            });
        let label = div()
            .w_full()
            .mt(px(6.))
            .min_w_0()
            .overflow_hidden()
            .child(
                // text-[13px] leading-4, two lines at most
                div()
                    .w_full()
                    .text_center()
                    .text_size(px(13.))
                    .line_height(px(16.))
                    .line_clamp(2)
                    .text_color(c(if self.is_dir { FOLDER } else { TEXT_PRIMARY }))
                    .child(self.name),
            )
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
        // tileClass: relative flex flex-col rounded border p-2, 152px (128px compact) tall
        let tile = div()
            .id(self.id)
            .group(group.clone())
            .relative()
            .w(px(metrics.tile_width))
            .h(px(if self.compact { 128. } else { 152. }))
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
                (true, _) => tile.border_color(alpha_of(accent, 0.7)).bg(background).hover(move |style| style.bg(hover)),
                (false, true) => tile.border_color(gpui::transparent_black()).bg(c(SURFACE_SECONDARY)),
                (false, false) => {
                    tile.border_color(gpui::transparent_black()).hover(|style| style.bg(c(SURFACE_SECONDARY)))
                }
            })
            .child(preview)
            .child(label)
            .when_some(self.checkbox, |tile, on_check| {
                // absolute top-2 left-2 h-4 w-4, shown on tile hover
                tile.child(
                    checkbox(self.selected, accent)
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
        match self.decorate {
            Some(decorate) => decorate(tile).into_any_element(),
            None => tile.into_any_element(),
        }
    }
}

fn alpha_of(color: gpui::Rgba, a: f32) -> gpui::Rgba {
    gpui::Rgba { a: color.a * a, ..color }
}
