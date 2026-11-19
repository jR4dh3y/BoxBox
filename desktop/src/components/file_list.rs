//! `lib/components/FileList.svelte`: the sortable header and one row. The
//! view owns virtualisation, selection and events; these own the look.
//!
//! The columns, their widths and their order are `ColumnLayout`'s, as in
//! Thunar's details view; the header sorts, resizes and offers the columns.

use super::Ellipsis as _;
use std::rc::Rc;

use gpui::{
    div, img, prelude::*, px, relative, AnyElement, App, ClickEvent, Div, ElementId, Entity, FontWeight, MouseButton, Pixels, Point,
    RenderImage, Rgba, SharedString, Stateful, Window,
};

use super::columns::{Column, ColumnLayout, ColumnSpec, Table, Width};
use super::TextField;

use super::Type;
use crate::sort::SortDir;
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

/// A border between header cells being dragged; `pane` keeps a split view's
/// other header still.
struct ColumnDrag {
    pane: usize,
    column: Column,
}

/// A border drag draws nothing under the pointer.
struct NoPreview;

impl Render for NoPreview {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

/// How wide the strip that grabs a border is.
const GRIP: f32 = 8.;

/// A `<td>`/`<th>`: px-3, its column's width or the room left.
fn cell(width: Width) -> Div {
    let cell = div().flex().items_center().h_full().px(px(12.)).min_w_0();
    match width {
        Width::Fixed(width) => cell.w(px(width)).flex_none(),
        Width::Fill(min) => cell.flex_1().min_w(px(min)),
    }
}

/// The table's own box: as wide as the pane, or as its columns when they are
/// wider, so the header and rows scroll sideways together.
pub fn table_frame(layout: &ColumnLayout, table: Table) -> Div {
    div().flex().flex_col().h_full().min_w_full().w(px(layout.min_width(table)))
}

/// `table` in a box that scrolls sideways when its columns are wider than the
/// pane; the rows inside scroll up and down on their own. Only a sideways
/// gesture or Shift+wheel moves it, not a wheel over the header.
pub fn sideways(id: impl Into<ElementId>, table: impl IntoElement) -> impl IntoElement {
    let mut frame = div().id(id).size_full().overflow_x_scroll();
    frame.style().restrict_scroll_to_axis = Some(true);
    frame.child(table)
}

type SortHandler = Rc<dyn Fn(Column, &mut Window, &mut App) + 'static>;
type ResizeHandler = Rc<dyn Fn(ColumnLayout, &mut Window, &mut App) + 'static>;
type MenuHandler = Rc<dyn Fn(Point<Pixels>, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct FileListHeader {
    table: Table,
    layout: ColumnLayout,
    sorted: Option<(Column, SortDir)>,
    pane: usize,
    on_sort: Option<SortHandler>,
    on_resize: Option<ResizeHandler>,
    on_menu: Option<MenuHandler>,
    background: Rgba,
}

impl FileListHeader {
    /// `pane` tells this header's border drags from another pane's.
    pub fn new(table: Table, layout: ColumnLayout, pane: usize) -> Self {
        Self { table, layout, sorted: None, pane, on_sort: None, on_resize: None, on_menu: None, background: c(SURFACE_SECONDARY) }
    }

    /// The window's chrome colour, which is translucent over a wallpaper or the desktop.
    pub fn background(mut self, background: Rgba) -> Self {
        self.background = background;
        self
    }

    /// The column the rows are sorted by, which shows the arrow; `None` when no
    /// column describes the order, such as search results by relevance.
    pub fn sorted(mut self, sorted: Option<(Column, SortDir)>) -> Self {
        self.sorted = sorted;
        self
    }

    pub fn on_sort(mut self, handler: impl Fn(Column, &mut Window, &mut App) + 'static) -> Self {
        self.on_sort = Some(Rc::new(handler));
        self
    }

    /// What a drag of a border, or a double click on one, sets.
    pub fn on_resize(mut self, handler: impl Fn(ColumnLayout, &mut Window, &mut App) + 'static) -> Self {
        self.on_resize = Some(Rc::new(handler));
        self
    }

    /// A right click on the header, which offers the columns to show.
    pub fn on_menu(mut self, handler: impl Fn(Point<Pixels>, &mut Window, &mut App) + 'static) -> Self {
        self.on_menu = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for FileListHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self { table, layout, sorted, pane, on_sort, on_resize, on_menu, background } = self;
        let specs = layout.specs(table);
        let last = specs.len() - 1;
        let grip = |column: Column, on_resize: ResizeHandler, layout: ColumnLayout| {
            div()
                .id(("column-border", column as usize))
                .group("column-border")
                .absolute()
                .top_0()
                .bottom_0()
                .right_0()
                .w(px(GRIP))
                .flex()
                .justify_end()
                .cursor(gpui::CursorStyle::ResizeColumn)
                .child(div().w(px(1.)).h(px(16.)).my_auto().bg(c(BORDER_PRIMARY)).group_hover("column-border", |style| style.h_full().bg(c(BORDER_FOCUS))))
                .on_drag(ColumnDrag { pane, column }, |_, _, _, cx| cx.new(|_| NoPreview))
                // A double click puts the column back at its own width.
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    if event.click_count() == 2 {
                        on_resize(layout.reset_width(column), window, cx);
                    }
                })
        };
        let drag_layout = layout.clone();
        let drag_resize = on_resize.clone();
        div()
            .id(("table-header", pane))
            .flex()
            .flex_none()
            .w_full()
            .h(px(HEADER_HEIGHT))
            .bg(background)
            .border_b_1()
            .border_color(c(BORDER_PRIMARY))
            .text_size(px(13.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .on_drag_move::<ColumnDrag>(move |event, window, cx| {
                let drag = event.drag(cx);
                if drag.pane != pane {
                    return;
                }
                let column = drag.column;
                if let Some(on_resize) = &drag_resize {
                    let x = f32::from(event.event.position.x - event.bounds.left());
                    on_resize(drag_layout.resized(table, column, x, f32::from(event.bounds.size.width)), window, cx);
                }
            })
            .when_some(on_menu, |header, on_menu| {
                header.on_mouse_down(MouseButton::Right, move |event, window, cx| {
                    cx.stop_propagation();
                    on_menu(event.position, window, cx);
                })
            })
            .children(specs.into_iter().enumerate().map(|(index, ColumnSpec { column, width })| {
                let arrow = sorted.filter(|(sorted, _)| *sorted == column).map(|(_, dir)| dir);
                // The border is inside GPUI's box but half outside the web's collapsed one;
                // pb-px puts the label on the same line.
                cell(width)
                    .id(column.label())
                    .relative()
                    .pb(px(1.))
                    .when(column.right_aligned(), |cell| cell.justify_end())
                    // `thSortedClass` loses to `thClass`'s text colour in the built CSS, so the web
                    // draws every header, sorted or not, in text-secondary.
                    .text_color(c(TEXT_SECONDARY))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .hover(|style| style.bg(c(SURFACE_TERTIARY)).text_color(c(TEXT_PRIMARY)))
                    .child(
                        // flex items-center gap-1; the chevron sits in a w-3 slot
                        div().flex().min_w_0().overflow_hidden().items_center().gap(px(4.)).child(div().truncate().child(column.label())).child(
                            div()
                                .flex()
                                .flex_none()
                                .w(px(12.))
                                .justify_center()
                                // The arrow turns over when the order flips.
                                .when_some(arrow, |slot, dir| {
                                    let (from, to) = match dir {
                                        SortDir::Asc => (180., 0.),
                                        SortDir::Desc => (0., 180.),
                                    };
                                    let id = ElementId::Name(format!("sort-arrow-{}-{}", column.label(), to).into());
                                    slot.child(super::motion::turn(id, icon("chevron-up", 12., c(TEXT_SECONDARY)), from, to, cx))
                                }),
                        ),
                    )
                    .when_some(on_sort.clone(), |cell, handler| cell.on_click(move |_, window, cx| handler(column, window, cx)))
                    // The last column's right border is the table's edge.
                    .when_some(on_resize.clone().filter(|_| index < last), |cell, on_resize| cell.child(grip(column, on_resize, layout.clone())))
            }))
    }
}

type Decorate = Box<dyn FnOnce(Stateful<Div>) -> Stateful<Div>>;
type CheckHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The look of a selected row: the view derives it from the accent.
#[derive(Clone, Copy)]
pub struct Selection {
    pub background: Rgba,
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
    /// The cells after Name, by column.
    values: Vec<(Column, SharedString)>,
    selected: bool,
    active: bool,
    cut: bool,
    drop_target: bool,
    metrics: ListMetrics,
    specs: Vec<ColumnSpec>,
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
            values: Vec::new(),
            selected: false,
            active: false,
            cut: false,
            drop_target: false,
            metrics: ListMetrics::new(1),
            specs: ColumnLayout::new(Table::Files).specs(Table::Files),
            selection,
            checkbox: None,
            decorate: None,
            editor: None,
        }
    }

    /// The columns the header shows, Name first, as `ColumnLayout::specs` gives them.
    pub fn columns(mut self, specs: Vec<ColumnSpec>) -> Self {
        self.specs = specs;
        self
    }

    /// A cell after Name, already formatted.
    pub fn value(mut self, column: Column, value: impl Into<SharedString>) -> Self {
        self.values.push((column, value.into()));
        self
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
        let Selection { background, accent } = self.selection;
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
        let name_width = self.specs.first().map_or(Width::Fill(0.), |spec| spec.width);
        let values = self.values;
        let cells = self.specs.into_iter().skip(1).map(move |ColumnSpec { column, width }| {
            let value = values.iter().find(|(key, _)| *key == column).map(|(_, value)| value.clone()).unwrap_or_default();
            cell(width).overflow_hidden().when(column.right_aligned(), |cell| cell.justify_end()).text_color(secondary).child(text(value))
        });
        let feedback_id = self.id.clone();
        // The cursor row and a drop target keep their own fill; the others ease in a
        // hover wash, over the selection when selected.
        let washed = !(self.active && !self.selected) && !self.drop_target;
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
            .when(self.active && !self.selected, |row| row.bg(c(SURFACE_SECONDARY)))
            // shadow-[inset_2px_0_0_var(--color-accent)]
            .when(self.active, |row| row.child(div().absolute().left_0().top_0().bottom_0().w(px(2.)).bg(accent)))
            .when(self.drop_target, |row| row.bg(background).child(div().absolute().inset_0().border_1().border_color(accent)))
            .child(
                cell(name_width)
                    .overflow_hidden()
                    .gap(px(8.))
                    .child(slot)
                    .map(|cell| match self.editor {
                        Some(editor) => cell.child(div().flex_1().min_w_0().child(editor)),
                        None => cell
                            .child(text(self.name).flex_shrink_0().max_w(relative(0.7)))
                            .child(text(self.location).flex_1().text_color(c(TEXT_MUTED))),
                    }),
            )
            .children(cells);
        let row = match self.decorate {
            Some(decorate) => decorate(row).into_any_element(),
            None => row.into_any_element(),
        };
        let feedback = super::motion::interact(feedback_id, row).without_press().selected(self.selected.then_some(background));
        match washed {
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
