//! Properties: the item with its type and size, its dates and place, and for a
//! local item a permissions grid that changes the mode when a box is clicked.
//! Several items show their counts and total size.

use gpui::{div, prelude::*, px, AnyElement, Context, FontWeight, MouseButton};

use super::thin_icon;
use crate::components::check::checkbox;
use crate::components::{Button, ButtonVariant, Modal, Type};
use crate::format::{file_icon, file_size, full_date, type_description};
use crate::theme::*;
use crate::workspace::properties::{permission_string, Ownership, Totals};
use crate::workspace::{Load, Workspace};

/// Owner, group and others, with the read, write and execute bits of each.
const CLASSES: [(&str, [u32; 3]); 3] = [("owner", [0o400, 0o200, 0o100]), ("group", [0o040, 0o020, 0o010]), ("others", [0o004, 0o002, 0o001])];

impl Workspace {
    pub(crate) fn render_properties(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(properties) = &self.properties else { return div().into_any_element() };
        let parent = |path: &str| path.rsplit_once('/').map(|(parent, _)| parent.to_string()).unwrap_or_else(|| "/".into());
        let size = match &properties.totals {
            Load::Loading => "Calculating…".to_string(),
            Load::Failed(error) => error.clone(),
            Load::Loaded(totals) => file_size(totals.bytes),
        };
        let ownership = match &properties.ownership {
            Some(Load::Loaded(ownership)) => Some(ownership),
            _ => None,
        };
        let remote = !properties.provider_label.is_empty() && properties.provider_label != "Local storage";
        let mut body = div().flex().flex_col().type_13();
        let rows: Vec<(&str, String)> = match properties.items.as_slice() {
            [item] => {
                let kind = if item.is_dir { "Folder".to_string() } else { type_description(&item.name).to_string() };
                let summary = match (item.is_dir, &properties.totals) {
                    // The walk counts the folder itself.
                    (true, Load::Loaded(Totals { files, folders, .. })) => format!("{kind} · {size} · {}", counted(*files, "file", folders.saturating_sub(1), "folder")),
                    _ => format!("{kind} · {size}"),
                };
                body = body.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .pb(px(14.))
                        .child(
                            div()
                                .size(px(40.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .bg(c(SURFACE_SECONDARY))
                                .child(thin_icon(file_icon(&item.name, item.is_dir), 22., c(TEXT_SECONDARY))),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT_PRIMARY)).child(item.name.clone()))
                                .child(div().text_size(px(12.)).text_color(c(TEXT_SECONDARY)).child(summary)),
                        ),
                );
                let location = match ownership.and_then(|ownership| ownership.path.parent()) {
                    Some(folder) => home_relative(folder),
                    None => parent(&item.path),
                };
                let mut rows = vec![("Location", location)];
                if let Some(target) = ownership.and_then(|ownership| ownership.link_target.clone()) {
                    rows.push(("Link to", target));
                }
                rows.push(("Modified", full_date(item.mod_time)));
                if let Some(ownership) = ownership {
                    rows.push(("Accessed", full_date(ownership.accessed)));
                    rows.push(("Created", full_date(ownership.created)));
                }
                if let Some(Load::Failed(error)) = &properties.ownership {
                    rows.push(("Permissions", error.clone()));
                }
                rows
            }
            items => {
                let folders = items.iter().filter(|item| item.is_dir).count();
                let mut rows = vec![("Selected", format!("{} items", items.len())), ("Folders", folders.to_string()), ("Size", size)];
                if let Load::Loaded(totals) = &properties.totals {
                    rows.push(("Contents", counted(totals.files, "file", totals.folders, "folder")));
                }
                let location = parent(&items[0].path);
                if items.iter().all(|item| parent(&item.path) == location) {
                    rows.push(("Location", location));
                }
                rows
            }
        };
        let mut rows = rows;
        if remote {
            rows.push(("Server", properties.provider_label.clone()));
        }
        body = body.child(div().flex().flex_col().gap(px(10.)).children(rows.into_iter().map(|(label, value)| {
            div()
                .flex()
                .gap(px(16.))
                .child(div().w(px(88.)).flex_none().text_color(c(TEXT_SECONDARY)).child(label))
                .child(div().flex_1().min_w_0().text_color(c(TEXT_PRIMARY)).child(value))
        })));
        if let Some(ownership) = ownership.filter(|ownership| ownership.link_target.is_none()) {
            body = body.child(self.permissions_grid(ownership, cx));
        }
        div()
            .id("properties-overlay")
            .debug_selector(|| "properties-overlay".into())
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .title("Properties")
                    .on_close(cx.listener(|this, _, _, cx| {
                        this.properties = None;
                        cx.notify();
                    }))
                    .body(body),
            )
            .into_any_element()
    }

    /// Read, Write and Execute (Access for a folder) for the owner, the group
    /// and everyone else, with the mode in octal. Only the owner can change it.
    fn permissions_grid(&self, ownership: &Ownership, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = c(self.accent_hex());
        let folder = ownership.is_dir();
        let busy = self.operation_status.is_busy();
        let column = |label: &'static str| div().w(px(56.)).flex().justify_center().text_size(px(12.)).text_color(c(TEXT_SECONDARY)).child(label);
        let header = div()
            .flex()
            .items_center()
            .h(px(24.))
            .child(div().flex_1().font_weight(FontWeight::MEDIUM).text_color(c(TEXT_PRIMARY)).child("Permissions"))
            .child(column("Read"))
            .child(column("Write"))
            .child(column(if folder { "Access" } else { "Execute" }));
        let rows = CLASSES.iter().map(|(class, bits)| {
            let who = match *class {
                "owner" => Some(ownership.owner.clone()),
                "group" => Some(ownership.group.clone()),
                _ => None,
            };
            let label = div()
                .flex_1()
                .min_w_0()
                .flex()
                .gap(px(6.))
                .truncate()
                .child(div().text_color(c(TEXT_PRIMARY)).child(who.clone().unwrap_or_else(|| "Others".into())))
                .when(who.is_some(), |label| label.child(div().text_color(c(TEXT_MUTED)).child(*class)));
            let boxes = bits.iter().map(|bit| {
                let (bit, checked) = (*bit, ownership.mode & *bit != 0);
                let mark = checkbox(checked, accent, 15.).when(!ownership.editable, |mark| mark.opacity(0.5));
                div()
                    .id(gpui::SharedString::from(format!("permission-{class}-{bit:o}")))
                    .debug_selector(move || format!("permission-{bit:o}"))
                    .w(px(56.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(ownership.editable, |cell| cell.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| this.toggle_permission(bit, cx))))
                    .child(mark)
            });
            div().flex().items_center().child(label).children(boxes)
        });
        div()
            .mt(px(14.))
            .pt(px(12.))
            .border_t_1()
            .border_color(c(BORDER_SECONDARY))
            .child(header)
            .children(rows)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt(px(8.))
                    .child(div().font_family(self.fonts.mono.clone()).text_size(px(12.)).text_color(c(TEXT_MUTED)).child(format!("{}  {:03o}", permission_string(ownership.mode), ownership.mode & 0o7777)))
                    .when(folder && ownership.editable, |row| {
                        row.child(
                            Button::new("permissions-apply-inside")
                                .label("Apply to contents")
                                .variant(ButtonVariant::Secondary)
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| this.apply_permissions_to_contents(cx))),
                        )
                    }),
            )
    }
}

/// `~/code/BoxBox` for a folder under the home folder.
fn home_relative(folder: &std::path::Path) -> String {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    match home.as_deref().and_then(|home| folder.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => folder.display().to_string(),
    }
}

/// "3 files, 1 folder".
fn counted(first: u64, first_noun: &str, second: u64, second_noun: &str) -> String {
    let plural = |count: u64, noun: &str| format!("{count} {noun}{}", if count == 1 { "" } else { "s" });
    format!("{}, {}", plural(first, first_noun), plural(second, second_noun))
}
