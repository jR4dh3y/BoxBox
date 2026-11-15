//! `lib/components/FileGrid.svelte`: grid metrics and one tile. The view owns
//! virtualisation, selection and events.

use std::sync::Arc;

use gpui::{div, img, prelude::*, px, AnyElement, App, ClickEvent, Div, ElementId, ObjectFit, RenderImage, SharedString, Stateful, Window};

use super::file_list::{checkbox, Selection};
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
    pub icon: f32,
    /// The longest edge of a thumbnail drawn at this size.
    pub thumbnail: u32,
}

impl GridMetrics {
    /// Step 1 is the web app's `grid h-[164px] gap-x-3` in `p-3`; step 0 its compact one.
    pub const STEPS: [Self; 5] = [
        Self { tile_width: 96., row_height: 136., gap: 8., padding: 8., preview_height: 56., icon: 24., thumbnail: 160 },
        Self { tile_width: 112., row_height: 164., gap: 12., padding: 12., preview_height: 80., icon: 30., thumbnail: 160 },
        Self { tile_width: 144., row_height: 196., gap: 12., padding: 12., preview_height: 112., icon: 42., thumbnail: 160 },
        Self { tile_width: 192., row_height: 244., gap: 12., padding: 12., preview_height: 160., icon: 60., thumbnail: 256 },
        Self { tile_width: 256., row_height: 308., gap: 12., padding: 12., preview_height: 224., icon: 84., thumbnail: 256 },
    ];

    pub fn new(zoom: u8) -> Self {
        Self::STEPS[usize::from(zoom).min(Self::STEPS.len() - 1)]
    }

    /// The edge of a theme icon: the preview box, less a margin, never wider than the tile.
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
    /// The icon theme's image, drawn without a frame when there is no thumbnail.
    image: Option<Arc<RenderImage>>,
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
            image: None,
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

    pub fn thumbnail(mut self, thumbnail: Option<Arc<RenderImage>>) -> Self {
        self.thumbnail = thumbnail;
        self
    }

    pub fn image(mut self, image: Option<Arc<RenderImage>>) -> Self {
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
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Selection { background, hover, accent } = self.selection;
        let metrics = self.metrics;
        let group = SharedString::from(format!("file-tile-{}", self.id));
        let tone = if self.is_dir { FOLDER } else { TEXT_SECONDARY };
        // A theme icon already says "folder"; the name keeps the text colour, as in Thunar.
        let folder_name = self.is_dir && self.image.is_none();
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
                (Some(image), _) => slot
                    .rounded(px(ROUNDED))
                    .border_1()
                    .border_color(c(BORDER_PRIMARY))
                    .bg(c(SURFACE_PRIMARY))
                    .child(img(image).size_full().object_fit(ObjectFit::Cover)),
                (None, Some(image)) => slot.child(img(image).size(px(metrics.theme_icon()))),
                (None, None) => slot.child(thin_icon(self.icon, metrics.icon, c(tone))),
            });
        let label = div()
            .w_full()
            .mt(px(6.))
            .min_w_0()
            .overflow_hidden()
            .map(|label| match self.editor {
                Some(editor) => label.child(div().text_size(px(13.)).child(editor)),
                // text-[13px] leading-4, two lines at most
                None => label.child(
                    div()
                        .w_full()
                        .text_center()
                        .text_size(px(13.))
                        .line_height(px(16.))
                        .line_clamp(2)
                        .text_color(c(if folder_name { FOLDER } else { TEXT_PRIMARY }))
                        .child(self.name),
                ),
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
                (true, _) => tile.border_color(alpha_of(accent, 0.7)).bg(background).hover(move |style| style.bg(hover)),
                (false, true) => tile.border_color(gpui::transparent_black()).bg(c(SURFACE_SECONDARY)),
                (false, false) => {
                    tile.border_color(gpui::transparent_black()).hover(|style| style.bg(c(SURFACE_SECONDARY)))
                }
            })
            .when(self.drop_target, |tile| tile.border_color(accent).bg(background))
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
