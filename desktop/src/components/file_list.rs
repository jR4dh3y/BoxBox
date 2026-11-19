//! `lib/components/FileList.svelte`: the sortable header and one row. The
//! view owns virtualisation, selection and events; these own the look.
//!
//! The table is `table-fixed` with Type 18%, Size 15% and Modified 25%, and
//! Name takes the rest, so every column scales with the pane.

use super::Ellipsis as _;
use gpui::{
    div, img, prelude::*, px, relative, AnyElement, App, ClickEvent, Div, ElementId, Entity, FontWeight, MouseButton, RenderImage, Rgba,
    SharedString, Stateful, Window,
};

use super::TextField;

use super::Type;
use crate::sort::{Sort, SortDir, SortField};
use crate::theme::*;
use crate::ui::icon;

/// `LIST_HEADER_HEIGHT`; compact mode only shrinks rows.
pub const HEADER_HEIGHT: f32 = 32.;

/// A list zoom step: the row height and icon size. Text stays 13px.
#[derive(Clone, Copy)]
pub struct ListMetrics {
    pub row_height: f32,
    pub icon: f32,
}

impl ListMetrics {
    /// Step 1 is the web app's `h-8` row; step 0 its compact one.
    pub const STEPS: [Self; 4] = [
        Self { row_height: 28., icon: 16. },
        Self { row_height: 32., icon: 16. },
        Self { row_height: 40., icon: 24. },
        Self { row_height: 48., icon: 32. },
    ];

    pub fn new(zoom: u8) -> Self {
        Self::STEPS[usize::from(zoom).min(Self::STEPS.len() - 1)]
    }
}

const COLUMNS: [(SortField, &str, Option<f32>, bool); 4] = [
    (SortField::Name, "Name", None, false),
    (SortField::Type, "Type", Some(0.18), false),
    (SortField::Size, "Size", Some(0.15), true),
    (SortField::Modified, "Modified", Some(0.25), false),
];

/// A `<td>`/`<th>`: px-3, clipped, the given share of the row or the rest.
fn cell(share: Option<f32>) -> Div {
    let cell = div().flex().items_center().h_full().px(px(12.)).min_w_0().overflow_hidden();
    match share {
        Some(share) => cell.w(relative(share)).flex_none(),
        None => cell.flex_1(),
    }
}

type SortHandler = Box<dyn Fn(SortField, &ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct FileListHeader {
    sort: Sort,
    unsorted: bool,
    on_sort: Option<SortHandler>,
    background: Rgba,
}

impl FileListHeader {
    pub fn new(sort: Sort) -> Self {
        Self { sort, unsorted: false, on_sort: None, background: c(SURFACE_SECONDARY) }
    }

    /// The window's chrome colour, which is translucent over a wallpaper or the desktop.
    pub fn background(mut self, background: Rgba) -> Self {
        self.background = background;
        self
    }

    /// The rows are in an order no column describes, such as search results by
    /// relevance, so no column shows a sort arrow.
    pub fn unsorted(mut self, unsorted: bool) -> Self {
        self.unsorted = unsorted;
        self
    }

    pub fn on_sort(mut self, handler: impl Fn(SortField, &ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_sort = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for FileListHeader {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let on_sort = self.on_sort.map(std::rc::Rc::new);
        let chevron = match self.sort.dir {
            SortDir::Asc => "chevron-up",
            SortDir::Desc => "chevron-down",
        };
        div()
            .flex()
            .flex_none()
            .w_full()
            .h(px(HEADER_HEIGHT))
            .bg(self.background)
            .border_b_1()
            .border_color(c(BORDER_PRIMARY))
            .text_size(px(13.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .children(COLUMNS.into_iter().map(|(field, label, share, right)| {
                // `thSortedClass` loses to `thClass`'s text colour in the built CSS, so the web
                // draws every header, sorted or not, in text-secondary.
                let sorted = !self.unsorted && self.sort.field == field;
                // The border is inside GPUI's box but half outside the web's collapsed one;
                // pb-px puts the label on the same line.
                cell(share)
                    .id(label)
                    .pb(px(1.))
                    .when(right, |cell| cell.justify_end())
                    .text_color(c(TEXT_SECONDARY))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .hover(|style| style.bg(c(SURFACE_TERTIARY)).text_color(c(TEXT_PRIMARY)))
                    .child(
                        // flex items-center gap-1; the chevron sits in a w-3 slot
                        div().flex().min_w_0().items_center().gap(px(4.)).child(div().truncate().child(label)).child(
                            div()
                                .flex()
                                .flex_none()
                                .w(px(12.))
                                .justify_center()
                                .when(sorted, |slot| slot.child(icon(chevron, 12., c(TEXT_SECONDARY)))),
                        ),
                    )
                    .when_some(on_sort.clone(), |cell, handler| {
                        cell.on_click(move |event, window, cx| handler(field, event, window, cx))
                    })
            }))
    }
}

type Decorate = Box<dyn FnOnce(Stateful<Div>) -> Stateful<Div>>;
type CheckHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The look of a selected row: the view derives it from the accent.
#[derive(Clone, Copy)]
pub struct Selection {
    pub background: Rgba,
    pub hover: Rgba,
    pub accent: Rgba,
}

/// What stands in an entry's icon slot.
#[derive(Clone)]
pub enum EntryIcon {
    /// BoxBox's own glyph.
    Glyph,
    /// A theme icon is being made; the slot stays empty so BoxBox's glyph never flashes first.
    Waiting,
    Theme(std::sync::Arc<RenderImage>),
}

#[derive(IntoElement)]
pub struct FileRow {
    id: ElementId,
    name: SharedString,
    location: SharedString,
    is_dir: bool,
    icon: &'static str,
    image: EntryIcon,
    tint: Option<u32>,
    emblem: Option<&'static str>,
    kind: SharedString,
    size: SharedString,
    modified: SharedString,
    selected: bool,
    active: bool,
    cut: bool,
    drop_target: bool,
    metrics: ListMetrics,
    selection: Selection,
    checkbox: Option<CheckHandler>,
    decorate: Option<Decorate>,
    editor: Option<AnyElement>,
}

/// The name field of a rename in place: a thin focused box that keeps clicks and
/// drags to itself, so they edit the text instead of selecting or moving the item.
pub fn rename_box(field: Entity<TextField>) -> Div {
    div()
        .flex()
        .items_center()
        .min_w_0()
        .h(px(22.))
        .px(px(4.))
        .rounded(px(super::ROUNDED))
        .border_1()
        .border_color(c(BORDER_FOCUS))
        .bg(c(SURFACE_PRIMARY))
        .text_color(c(TEXT_PRIMARY))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .child(field)
}

impl FileRow {
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>, is_dir: bool, selection: Selection) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            location: SharedString::default(),
            is_dir,
            icon: "file",
            image: EntryIcon::Glyph,
            tint: None,
            emblem: None,
            kind: SharedString::default(),
            size: SharedString::default(),
            modified: SharedString::default(),
            selected: false,
            active: false,
            cut: false,
            drop_target: false,
            metrics: ListMetrics::new(1),
            selection,
            checkbox: None,
            decorate: None,
            editor: None,
        }
    }

    /// BoxBox's own glyph: its kind's colour and a special folder's emblem.
    pub fn tint(mut self, tint: Option<u32>, emblem: Option<&'static str>) -> Self {
        self.tint = tint;
        self.emblem = emblem;
        self
    }

    pub fn image(mut self, image: EntryIcon) -> Self {
        self.image = image;
        self
    }

    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = icon;
        self
    }

    /// Draws `editor` in place of the name, for a rename in place.
    pub fn editor(mut self, editor: impl IntoElement) -> Self {
        self.editor = Some(editor.into_any_element());
        self
    }

    /// The folder a search result is in, shown muted after its name.
    pub fn location(mut self, location: impl Into<SharedString>) -> Self {
        self.location = location.into();
        self
    }

    /// The Type, Size and Modified cells, already formatted.
    pub fn details(
        mut self,
        kind: impl Into<SharedString>,
        size: impl Into<SharedString>,
        modified: impl Into<SharedString>,
    ) -> Self {
        self.kind = kind.into();
        self.size = size.into();
        self.modified = modified.into();
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The keyboard cursor: an accent stripe on the left edge.
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
        self.metrics = ListMetrics::new(zoom);
        self
    }

    /// Shows the selection checkbox over the icon on hover, and keeps it while selected.
    pub fn checkbox(mut self, on_check: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.checkbox = Some(Box::new(on_check));
        self
    }

    /// Lets the view attach behaviour (clicks, drag, context menu) to the row element.
    pub fn decorate(mut self, decorate: impl FnOnce(Stateful<Div>) -> Stateful<Div> + 'static) -> Self {
        self.decorate = Some(Box::new(decorate));
        self
    }
}

impl RenderOnce for FileRow {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Selection { background, hover, accent } = self.selection;
        let secondary = c(if self.selected { TEXT_PRIMARY } else { TEXT_SECONDARY });
        let tone = c(self.tint.unwrap_or(if self.is_dir { FOLDER } else { TEXT_SECONDARY }));
        let group = SharedString::from(format!("file-icon-{}", self.id));
        let has_checkbox = self.checkbox.is_some();
        let ListMetrics { row_height, icon: glyph_size } = self.metrics;
        let slot_size = glyph_size + 4.;

        // group/icon relative h-5 w-5; the checkbox is absolute inset-0.5 over the icon
        let glyph = div()
            .flex()
            .items_center()
            .justify_center()
            .size_full()
            .when(has_checkbox && self.selected, |el| el.invisible())
            .when(has_checkbox && !self.selected, |el| el.group_hover(group.clone(), |style| style.invisible()))
            .map(|glyph| match self.image {
                // Theme icons are full pictures and fill the slot; glyphs keep their margin.
                EntryIcon::Theme(image) => glyph.child(img(image).size(px(slot_size))),
                EntryIcon::Waiting => glyph,
                EntryIcon::Glyph => glyph.child(emblemed(icon(self.icon, glyph_size, tone), self.emblem, glyph_size, tone)),
            });
        let slot = div()
            .id("icon")
            .group(group.clone())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(slot_size))
            .child(glyph)
            .when_some(self.checkbox, |slot, on_check| {
                slot.child(
                    checkbox(self.selected, accent, list_checkbox(slot_size))
                        .absolute()
                        .top(px((slot_size - list_checkbox(slot_size)) / 2.))
                        .left(px((slot_size - list_checkbox(slot_size)) / 2.))
                        .when(!self.selected, |el| el.invisible().group_hover(group, |style| style.visible()))
                        .on_click(move |event, window, cx| {
                            cx.stop_propagation();
                            on_check(event, window, cx);
                        }),
                )
            });

        let text = |value: SharedString| div().min_w_0().ellipsis().child(value);
        let feedback_id = self.id.clone();
        // Only a plain row eases in a hover fill; selected and active rows keep their own.
        let plain = !self.selected && !self.active && !self.drop_target;
        let row = div()
            .id(self.id)
            .relative()
            .flex()
            .w_full()
            .h(px(row_height))
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .type_13()
            .line_height(px(20.))
            .text_color(c(TEXT_PRIMARY))
            .cursor_default()
            .when(self.cut, |row| row.opacity(0.5))
            .map(|row| match (self.selected, self.active) {
                (true, _) => row.bg(background).hover(move |style| style.bg(hover)),
                (false, true) => row.bg(c(SURFACE_SECONDARY)),
                (false, false) => row,
            })
            // shadow-[inset_2px_0_0_var(--color-accent)]
            .when(self.active, |row| row.child(div().absolute().left_0().top_0().bottom_0().w(px(2.)).bg(accent)))
            .when(self.drop_target, |row| row.bg(background).child(div().absolute().inset_0().border_1().border_color(accent)))
            .child(
                cell(None)
                    .gap(px(8.))
                    .child(slot)
                    .map(|cell| match self.editor {
                        Some(editor) => cell.child(div().flex_1().min_w_0().child(editor)),
                        None => cell
                            .child(text(self.name).flex_shrink_0().max_w(relative(0.7)))
                            .child(text(self.location).flex_1().text_color(c(TEXT_MUTED))),
                    }),
            )
            .child(cell(Some(0.18)).text_color(secondary).child(text(self.kind)))
            .child(cell(Some(0.15)).justify_end().text_color(secondary).child(text(self.size)))
            .child(cell(Some(0.25)).text_color(secondary).child(text(self.modified)));
        let row = match self.decorate {
            Some(decorate) => decorate(row).into_any_element(),
            None => row.into_any_element(),
        };
        let feedback = super::motion::interact(feedback_id, row).without_press();
        match plain {
            true => feedback.fill(super::motion::hover_fill()).into_any_element(),
            false => feedback.into_any_element(),
        }
    }
}

/// The selection checkbox of rows and tiles, `size` pixels square.
pub(super) fn checkbox(checked: bool, accent: Rgba, size: f32) -> Stateful<Div> {
    super::check::checkbox(checked, accent, size).id("check").cursor_pointer()
}

/// A row's checkbox, which covers its icon, grows with the zoom from 16px.
fn list_checkbox(slot: f32) -> f32 {
    (slot * 0.65).round().max(16.)
}

/// A folder glyph with a small one inside its body, as a special folder's mark.
pub fn emblemed(glyph: gpui::Svg, emblem: Option<&'static str>, size: f32, color: Rgba) -> Div {
    let frame = div().relative().size(px(size)).flex_none().child(glyph);
    match emblem {
        Some(emblem) => frame.child(div().absolute().left(px(size * 0.3)).top(px(size * 0.36)).child(icon(emblem, size * 0.4, color))),
        None => frame,
    }
}
