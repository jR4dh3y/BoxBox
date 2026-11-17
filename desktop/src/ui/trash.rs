//! The Trash page: a details table of what was deleted, from where and when,
//! with Restore and Delete on each row, as Nautilus and Thunar show it.

use gpui::{div, prelude::*, px, relative, AnyElement, Context, Div, FontWeight, SharedString};

use super::{icon, TextScale};
use crate::api::TrashItem;
use crate::components::{Button, ButtonSize, ButtonVariant, Tooltip, HEADER_HEIGHT};
use crate::format::{deletion_millis, file_date, full_date, Kind};
use crate::theme::*;
use crate::workspace::{Load, Workspace};

const ROW_HEIGHT: f32 = 36.;
const LOCATION_SHARE: f32 = 0.38;
const DELETED_SHARE: f32 = 0.16;
const ACTIONS_WIDTH: f32 = 128.;

/// A table cell: clipped, the given share of the row or the rest.
fn cell(share: Option<f32>) -> Div {
    let cell = div().flex().items_center().h_full().px(px(12.)).min_w_0().overflow_hidden();
    match share {
        Some(share) => cell.w(relative(share)).flex_none(),
        None => cell.flex_1(),
    }
}

/// The folder an item came from, with the home folder as `~`.
fn location(original: &str) -> String {
    let parent = std::path::Path::new(original).parent().map(|parent| parent.display().to_string()).unwrap_or_default();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && parent == home => "~".into(),
        Ok(home) if !home.is_empty() && parent.starts_with(&format!("{home}/")) => format!("~{}", &parent[home.len()..]),
        _ => parent,
    }
}

impl Workspace {
    pub(crate) fn empty_trash_button(&self, cx: &mut Context<Self>) -> Button {
        Button::new("empty-trash")
            .label("Empty Trash")
            .variant(ButtonVariant::Danger)
            .size(ButtonSize::Sm)
            .disabled(!matches!(&self.trash, Load::Loaded(items) if !items.is_empty()) || self.operation_status.is_busy())
            .on_click(cx.listener(|this, _, _, cx| this.empty_trash(cx)))
    }

    pub(crate) fn render_trash(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let message = |text: SharedString, glyph: Option<&str>| {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .text_sm_tw()
                .text_color(c(TEXT_SECONDARY))
                .when_some(glyph, |el, glyph| el.child(icon(glyph, 32., alpha(TEXT_MUTED, 0.5))))
                .child(text)
                .into_any_element()
        };
        let body: AnyElement = match &self.trash {
            Load::Loading => message("Loading Trash…".into(), None),
            Load::Failed(error) => message(error.clone().into(), Some("alert-triangle")),
            Load::Loaded(items) if items.is_empty() => message("Trash is empty".into(), Some("trash-2")),
            Load::Loaded(items) => div()
                .id("trash-list")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .children(items.iter().map(|item| self.render_trash_row(item, cx)))
                .into_any_element(),
        };
        let has_rows = matches!(&self.trash, Load::Loaded(items) if !items.is_empty());
        div()
            .id("trash-page")
            .size_full()
            .flex()
            .flex_col()
            .when(has_rows, |page| page.child(trash_header()))
            .child(body)
    }

    fn render_trash_row(&self, item: &TrashItem, cx: &mut Context<Self>) -> impl IntoElement {
        let kind = Kind::of(&item.name, item.is_dir);
        let tint = self.kind_tint(kind).map_or_else(|| c(TEXT_SECONDARY), c);
        let deleted = deletion_millis(&item.deleted_at);
        let busy = self.operation_status.is_busy();
        let (restored, removed) = (item.clone(), item.clone());
        let (restore_name, delete_name) = (item.name.clone(), item.name.clone());
        div()
            .id(SharedString::from(format!("trash-{}", item.id)))
            .flex()
            .flex_none()
            .w_full()
            .h(px(ROW_HEIGHT))
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .text_13()
            .hover(|style| style.bg(c(SURFACE_SECONDARY)))
            .child(
                cell(None)
                    .gap(px(10.))
                    .child(icon(kind.glyph(), 16., tint))
                    .child(div().min_w_0().truncate().text_color(c(TEXT_PRIMARY)).child(item.name.clone())),
            )
            .child(
                cell(Some(LOCATION_SHARE))
                    .id(SharedString::from(format!("trash-from-{}", item.id)))
                    .text_color(c(TEXT_SECONDARY))
                    .tooltip(Tooltip::text(item.original_path.clone()))
                    .child(div().min_w_0().truncate().child(location(&item.original_path))),
            )
            .child(
                cell(Some(DELETED_SHARE))
                    .id(SharedString::from(format!("trash-when-{}", item.id)))
                    .text_color(c(TEXT_SECONDARY))
                    .tooltip(Tooltip::text(full_date(deleted)))
                    .child(div().truncate().child(file_date(deleted))),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .w(px(ACTIONS_WIDTH))
                    .h_full()
                    .items_center()
                    .justify_end()
                    .gap(px(4.))
                    .pr(px(8.))
                    .child(
                        div()
                            .id(SharedString::from(format!("restore-wrap-{}", item.id)))
                            .debug_selector(move || format!("restore-{restore_name}"))
                            .when(!item.can_restore, |el| {
                                el.tooltip(Tooltip::text("Cannot restore: something already exists at the original location, or it is unavailable"))
                            })
                            .child(
                                Button::new(SharedString::from(format!("restore-{}", item.id)))
                                    .label("Restore")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .disabled(!item.can_restore || busy)
                                    .on_click(cx.listener(move |this, _, _, cx| this.restore_trash(restored.clone(), cx))),
                            ),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("delete-trash-{}", item.id)))
                            .debug_selector(move || format!("delete-trash-{delete_name}"))
                            .group("delete-trash")
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(28.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|style| style.bg(alpha(DANGER, 0.15)))
                            .active(crate::components::pressed)
                            .tooltip(Tooltip::text("Delete permanently"))
                            .child(icon("trash-2", 16., c(TEXT_MUTED)).group_hover("delete-trash", |style| style.text_color(c(DANGER))))
                            .on_click(cx.listener(move |this, _, _, cx| this.delete_trash_item(removed.clone(), cx))),
                    ),
            )
    }
}

fn trash_header() -> impl IntoElement {
    let label = |text: &'static str, share: Option<f32>| cell(share).pb(px(1.)).text_color(c(TEXT_SECONDARY)).child(div().truncate().child(text));
    div()
        .flex()
        .flex_none()
        .w_full()
        .h(px(HEADER_HEIGHT))
        .border_b_1()
        .border_color(c(BORDER_PRIMARY))
        .text_size(px(13.))
        .line_height(px(20.))
        .font_weight(FontWeight::MEDIUM)
        .child(label("Name", None))
        .child(label("Original location", Some(LOCATION_SHARE)))
        .child(label("Deleted", Some(DELETED_SHARE)))
        .child(div().flex_none().w(px(ACTIONS_WIDTH)))
}

#[cfg(test)]
mod tests {
    use super::location;

    #[test]
    fn locations_show_the_folder_with_home_as_tilde() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(location(&format!("{home}/Desktop/a.pdf")), "~/Desktop");
        assert_eq!(location(&format!("{home}/a.pdf")), "~");
        assert_eq!(location("/srv/media/a.mkv"), "/srv/media");
    }
}
