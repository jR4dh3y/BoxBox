//! The Details panel at the window's right, as Dolphin's Information panel
//! (F11): the active pane's selection, or the folder when nothing is selected.

use gpui::{div, img, prelude::*, px, AnyElement, Context, ObjectFit, SharedString};

use super::{icon, thin_icon, TextScale};
use crate::api::FileInfo;
use crate::components::{emblemed, EntryIcon};
use crate::format::{file_size, full_date, type_description};
use crate::theme::*;
use crate::workspace::{ToggleDetails, Workspace};

pub(crate) const DETAILS_WIDTH: f32 = 280.;
const PREVIEW_HEIGHT: f32 = 160.;
const ICON_SIZE: f32 = 96.;

/// A label over its value.
fn field(label: &'static str, value: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(div().text_color(c(TEXT_SECONDARY)).child(label))
        .child(div().text_color(c(TEXT_PRIMARY)).child(value.into()))
}

fn parent(path: &str) -> String {
    path.rsplit_once('/').map(|(parent, _)| parent.to_string()).unwrap_or_default()
}

impl Workspace {
    /// What the panel will draw for a single item: its thumbnail, or its icon at the panel's size.
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

    pub(crate) fn render_details(&self, cx: &mut Context<Self>) -> AnyElement {
        let pane = self.pane();
        let selected = if self.trash_open { Vec::new() } else { pane.selected_items() };
        let body = match selected.as_slice() {
            [item] => self.single_details(item).into_any_element(),
            [] => self.folder_details().into_any_element(),
            items => many_details(items).into_any_element(),
        };
        div()
            .id("details-panel")
            .debug_selector(|| "details-panel".into())
            .w(px(DETAILS_WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(c(BORDER_SECONDARY))
            .bg(self.shell_background())
            .child(
                div()
                    .h(px(41.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(16.))
                    .border_b_1()
                    .border_color(c(BORDER_SECONDARY))
                    .text_13()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child("Details")
                    .child(
                        div()
                            .id("details-close")
                            .rounded(px(4.))
                            .p(px(4.))
                            .cursor_pointer()
                            .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                            .child(icon("x", 16., c(TEXT_SECONDARY)))
                            .on_click(cx.listener(|this, _, window, cx| this.toggle_details(&ToggleDetails, window, cx))),
                    ),
            )
            .child(div().id("details-body").flex_1().min_h_0().overflow_y_scroll().p(px(16.)).text_xs_tw().child(body))
            .into_any_element()
    }

    fn single_details(&self, item: &FileInfo) -> gpui::Div {
        let pane = self.pane();
        let preview = div().h(px(PREVIEW_HEIGHT)).w_full().flex().items_center().justify_center().overflow_hidden().rounded(px(8.));
        let preview = match self.thumbnail(&pane.api, item) {
            Some(image) => preview.bg(c(SURFACE_SECONDARY)).child(img(image).size_full().object_fit(ObjectFit::Contain)),
            None => match self.item_icon(&pane.api, item, ICON_SIZE) {
                EntryIcon::Theme(image) => preview.child(img(image).size(px(ICON_SIZE))),
                EntryIcon::Waiting => preview,
                EntryIcon::Glyph => {
                    let style = self.glyph_style(&pane.api, item);
                    let tone = c(style.tint.unwrap_or(if item.is_dir { FOLDER } else { TEXT_SECONDARY }));
                    preview.child(emblemed(thin_icon(style.glyph, ICON_SIZE, tone), style.emblem, ICON_SIZE, tone))
                }
            },
        };
        let kind = if item.is_dir { "Folder".to_string() } else { type_description(&item.name) };
        div()
            .flex()
            .flex_col()
            .child(preview)
            .child(div().mt(px(16.)).text_sm_tw().font_weight(gpui::FontWeight::MEDIUM).text_color(c(TEXT_PRIMARY)).child(item.name.clone()))
            .child(div().mt(px(2.)).text_color(c(TEXT_SECONDARY)).child(kind))
            .child(
                div()
                    .mt(px(20.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .when(!item.is_dir, |rows| rows.child(field("Size", file_size(item.size))))
                    .child(field("Modified", full_date(item.mod_time)))
                    .child(field("Location", self.location_label(&parent(&item.path)))),
            )
    }

    fn folder_details(&self) -> gpui::Div {
        let pane = self.pane();
        let (name, count) = if self.trash_open {
            ("Trash".to_string(), None)
        } else if pane.is_at_root() {
            ("Overview".to_string(), None)
        } else {
            (self.location_label(&pane.path), Some(pane.items.len()))
        };
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(div().text_sm_tw().font_weight(gpui::FontWeight::MEDIUM).text_color(c(TEXT_PRIMARY)).child(name))
            .when_some(count, |folder, count| folder.child(div().text_color(c(TEXT_SECONDARY)).child(items_label(count))))
            .child(div().mt(px(16.)).text_color(c(TEXT_MUTED)).child("Select an item to see its details."))
    }

    /// A virtual path as the path bar names it: the drive or place, then the folders under it.
    fn location_label(&self, path: &str) -> String {
        let crumbs = self.breadcrumbs(self.active_pane);
        let root = crumbs.first().map(|crumb| (crumb.label.clone(), crumb.path.clone()));
        match root {
            Some((label, root)) if path == root || path.starts_with(&format!("{root}/")) => format!("{label}{}", &path[root.len()..]),
            _ => path.to_string(),
        }
    }
}

fn many_details(items: &[FileInfo]) -> gpui::Div {
    let folders = items.iter().filter(|item| item.is_dir).count();
    let files = items.len() - folders;
    let bytes: u64 = items.iter().filter(|item| !item.is_dir).map(|item| item.size).sum();
    div()
        .flex()
        .flex_col()
        .child(div().text_sm_tw().font_weight(gpui::FontWeight::MEDIUM).text_color(c(TEXT_PRIMARY)).child(format!("{} items selected", items.len())))
        .child(div().mt(px(2.)).text_color(c(TEXT_SECONDARY)).child(format!("{} · {}", counted(files, "file"), counted(folders, "folder"))))
        .child(div().mt(px(20.)).child(field(if folders > 0 { "Size of the files" } else { "Size" }, file_size(bytes))))
        .child(
            div()
                .mt(px(20.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .text_color(c(TEXT_SECONDARY))
                .children(items.iter().take(50).map(|item| div().truncate().child(item.name.clone())))
                .when(items.len() > 50, |list| list.child(div().text_color(c(TEXT_MUTED)).child(format!("and {} more", items.len() - 50)))),
        )
}

fn counted(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

fn items_label(count: usize) -> String {
    counted(count, "item")
}
