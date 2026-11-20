//! The Details panel under the toolbar, as Dolphin's Information panel (F11):
//! the active pane's selection, or the folder when nothing is selected. From
//! the top: the preview, the name, then the details as label and value columns.

use gpui::{div, img, prelude::*, px, AnyElement, Context, ObjectFit, SharedString};

use super::{thin_icon, TextScale};
use crate::api::FileInfo;
use crate::components::{emblemed, EntryIcon};
use crate::format::{file_size, full_date, type_description};
use crate::theme::*;
use crate::workspace::Workspace;

pub(crate) const DETAILS_WIDTH: f32 = 260.;
const PREVIEW_HEIGHT: f32 = 128.;
const ICON_SIZE: f32 = 64.;

/// Wide enough for "Modified", so every value starts on one line.
const LABEL_WIDTH: f32 = 72.;

/// The label in its column, the value beside it, wrapping under itself.
fn row(label: &'static str, value: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .items_start()
        .gap(px(8.))
        .py(px(4.))
        .child(div().w(px(LABEL_WIDTH)).flex_none().text_color(c(TEXT_SECONDARY)).child(label))
        .child(div().flex_1().min_w_0().text_color(c(TEXT_PRIMARY)).child(value.into()))
}

/// "Details" over its rows.
fn rows(children: Vec<gpui::Div>) -> gpui::Div {
    div()
        .mt(px(18.))
        .flex()
        .flex_col()
        .child(div().mb(px(4.)).text_13().font_weight(gpui::FontWeight::MEDIUM).text_color(c(TEXT_PRIMARY)).child("Details"))
        .children(children)
}

/// The name, and a line under it when there is one.
fn title(name: impl Into<SharedString>, under: impl Into<SharedString>) -> gpui::Div {
    let under = under.into();
    div()
        .flex()
        .flex_col()
        .child(div().w_full().text_sm_tw().font_weight(gpui::FontWeight::SEMIBOLD).text_color(c(TEXT_PRIMARY)).child(name.into()))
        .when(!under.is_empty(), |title| title.child(div().mt(px(2.)).text_color(c(TEXT_SECONDARY)).child(under)))
}

fn parent(path: &str) -> String {
    path.rsplit_once('/').map(|(parent, _)| parent.to_string()).unwrap_or_default()
}

impl Workspace {
    /// Starts making what the panel draws for a single item: its thumbnail and its icon.
    pub(crate) fn request_details_preview(&mut self, cx: &mut Context<Self>) {
        if !self.preferences.details_panel || self.trash_open {
            return;
        }
        let target = self.active_pane;
        let pane = self.pane();
        let items = pane.selected_items();
        let [item] = items.as_slice() else { return };
        let (item, api) = (item.clone(), pane.api.clone());
        self.request_thumbnail(target, item.clone(), cx);
        self.request_item_icon(&api, &item, ICON_SIZE, cx);
    }

    /// The panel while it is open, opening or closing.
    pub(crate) fn render_details(&self, window: &gpui::Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        // Asks for frames while it moves; the panel and the panes read one share.
        crate::components::motion::progress(self.details_toggled, crate::components::motion::PANEL, window, cx);
        let open = self.details_share();
        if open <= 0. {
            return None;
        }
        let pane = self.pane();
        let selected = if self.trash_open { Vec::new() } else { pane.selected_items() };
        let body = match selected.as_slice() {
            [item] => self.single_details(item),
            [] => self.folder_details(),
            items => many_details(items),
        };
        let panel = div()
            .id("details-panel")
            .debug_selector(|| "details-panel".into())
            .w(px(DETAILS_WIDTH))
            .flex_none()
            .h_full()
            .border_l_1()
            .border_color(c(BORDER_SECONDARY))
            .bg(self.chrome_background())
            .overflow_y_scroll()
            .text_xs_tw()
            .p(px(16.))
            .child(body);
        // It opens by its width, so the contents beside it make room as it comes
        // and take it back as it goes; the panel itself is drawn whole and slides
        // out from the right edge.
        Some(
            div()
                .flex()
                .justify_end()
                .flex_none()
                .h_full()
                .w(px(DETAILS_WIDTH * open))
                .overflow_hidden()
                .opacity(open)
                .child(panel)
                .into_any_element(),
        )
    }

    fn preview(&self, item: &FileInfo) -> gpui::Div {
        let pane = self.pane();
        let frame = div().h(px(PREVIEW_HEIGHT)).w_full().flex().items_center().justify_center().overflow_hidden().rounded(px(6.));
        match self.thumbnail(&pane.api, item) {
            Some(image) => frame.child(img(image).size_full().object_fit(ObjectFit::Contain)),
            None => match self.item_icon(&pane.api, item, ICON_SIZE) {
                EntryIcon::Theme(image) => frame.child(img(image).size(px(ICON_SIZE))),
                EntryIcon::Waiting => frame,
                EntryIcon::Glyph => {
                    let style = self.glyph_style(&pane.api, item);
                    let tone = c(style.tint.unwrap_or(if item.is_dir { FOLDER } else { TEXT_SECONDARY }));
                    frame.child(emblemed(thin_icon(style.glyph, ICON_SIZE, tone), style.emblem, ICON_SIZE, tone))
                }
            },
        }
    }

    fn single_details(&self, item: &FileInfo) -> gpui::Div {
        let kind = if item.is_dir { "Folder".to_string() } else { type_description(&item.name) };
        let mut details = vec![row("Type", kind)];
        if !item.is_dir {
            details.push(row("Size", file_size(item.size)));
        }
        details.push(row("Location", self.location_label(&parent(&item.path))));
        details.push(row("Modified", full_date(item.mod_time)));
        div().flex().flex_col().child(self.preview(item)).child(div().mt(px(12.)).child(title(item.name.clone(), ""))).child(rows(details))
    }

    fn folder_details(&self) -> gpui::Div {
        let pane = self.pane();
        let (name, under) = if self.trash_open {
            ("Trash".to_string(), String::new())
        } else if pane.is_at_root() {
            ("Overview".to_string(), String::new())
        } else {
            (self.location_label(&pane.path), counted(pane.items.len(), "item"))
        };
        div()
            .flex()
            .flex_col()
            .child(title(name, under))
            .child(div().mt(px(14.)).text_color(c(TEXT_MUTED)).child("Select an item to see its details."))
    }

    /// A virtual path as the path bar names it: the drive or place, then the folders under it.
    fn location_label(&self, path: &str) -> String {
        let crumbs = self.breadcrumbs(self.active_pane);
        match crumbs.first() {
            Some(root) if path == root.path || path.starts_with(&format!("{}/", root.path)) => format!("{}{}", root.label, &path[root.path.len()..]),
            _ => path.to_string(),
        }
    }
}

fn many_details(items: &[FileInfo]) -> gpui::Div {
    const LISTED: usize = 50;
    let folders = items.iter().filter(|item| item.is_dir).count();
    let files = items.len() - folders;
    let bytes: u64 = items.iter().filter(|item| !item.is_dir).map(|item| item.size).sum();
    let mut details = vec![row("Files", files.to_string()), row("Folders", folders.to_string())];
    // Folders are not walked here; Properties counts what is inside them.
    details.push(row("Size", if folders > 0 { format!("{}, files only", file_size(bytes)) } else { file_size(bytes) }));
    div()
        .flex()
        .flex_col()
        .child(title(format!("{} items selected", items.len()), ""))
        .child(rows(details))
        .child(
            div()
                .mt(px(18.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .text_color(c(TEXT_SECONDARY))
                .child(div().mb(px(2.)).text_13().font_weight(gpui::FontWeight::MEDIUM).text_color(c(TEXT_PRIMARY)).child("Selected"))
                .children(items.iter().take(LISTED).map(|item| div().truncate().child(item.name.clone())))
                .when(items.len() > LISTED, |list| list.child(div().text_color(c(TEXT_MUTED)).child(format!("and {} more", items.len() - LISTED)))),
        )
}

fn counted(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}
