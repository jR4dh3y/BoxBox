//! Properties: the item's name and icon in the title, then rows with the label
//! on the left and the value on the right, in groups: what it is, when it
//! changed, and for a local item who may use it, changed from the row itself.

use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton, SharedString};

use super::thin_icon;
use crate::components::{Button, ButtonSize, ButtonVariant, Modal, Type};
use crate::format::{file_icon, file_size, full_date, type_description};
use crate::theme::*;
use crate::workspace::properties::{permission_string, Ownership, Totals, CLASSES};
use crate::workspace::{Load, Workspace};

/// A row: the label left, the value right.
fn row(label: impl IntoElement, value: impl IntoElement) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .min_h(px(30.))
        .child(div().flex_none().flex().gap(px(6.)).text_color(c(TEXT_SECONDARY)).child(label))
        .child(div().min_w_0().flex().justify_end().text_color(c(TEXT_PRIMARY)).child(value))
}

fn text(value: String) -> gpui::Div {
    div().min_w_0().truncate().child(value)
}

/// Rows under a divider, except the first group.
fn group(first: bool, rows: Vec<gpui::Div>) -> gpui::Div {
    div().flex().flex_col().px(px(16.)).py(px(6.)).when(!first, |group| group.border_t_1().border_color(c(BORDER_SECONDARY))).children(rows)
}

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
        let remote = properties.provider_label != "Local storage";
        let (title, glyph, groups) = match properties.items.as_slice() {
            [item] => {
                let kind = if item.is_dir { "Folder".to_string() } else { type_description(&item.name).to_string() };
                let mut about = vec![row("Type", text(kind)), row("Size", text(size))];
                if let (true, Load::Loaded(Totals { files, folders, .. })) = (item.is_dir, &properties.totals) {
                    // The walk counts the folder itself.
                    about.push(row("Contents", text(counted(*files, "file", folders.saturating_sub(1), "folder"))));
                }
                let location = match ownership.and_then(|ownership| ownership.path.parent()) {
                    Some(folder) => home_relative(folder),
                    None => parent(&item.path),
                };
                about.push(row("Location", text(location)));
                if let Some(target) = ownership.and_then(|ownership| ownership.link_target.clone()) {
                    about.push(row("Link to", text(target)));
                }
                if remote {
                    about.push(row("Server", text(properties.provider_label.clone())));
                }
                let mut dates = vec![row("Modified", text(full_date(item.mod_time)))];
                if let Some(ownership) = ownership {
                    dates.push(row("Accessed", text(full_date(ownership.accessed))));
                    dates.push(row("Created", text(full_date(ownership.created))));
                }
                let mut groups = vec![about, dates];
                match (&properties.ownership, ownership) {
                    (_, Some(ownership)) if ownership.link_target.is_none() => groups.push(self.permission_rows(ownership, cx)),
                    (Some(Load::Failed(error)), _) => groups.push(vec![row("Permissions", text(error.clone()))]),
                    _ => {}
                }
                (item.name.clone(), file_icon(&item.name, item.is_dir), groups)
            }
            items => {
                let folders = items.iter().filter(|item| item.is_dir).count();
                let mut about = vec![row("Folders", text(folders.to_string())), row("Files", text((items.len() - folders).to_string())), row("Size", text(size))];
                if let Load::Loaded(totals) = &properties.totals {
                    about.push(row("Contents", text(counted(totals.files, "file", totals.folders, "folder"))));
                }
                let location = parent(&items[0].path);
                if items.iter().all(|item| parent(&item.path) == location) {
                    about.push(row("Location", text(location)));
                }
                if remote {
                    about.push(row("Server", text(properties.provider_label.clone())));
                }
                (format!("{} items", items.len()), "copy", vec![about])
            }
        };
        let body = div().flex().flex_col().py(px(4.)).type_13().children(groups.into_iter().enumerate().map(|(index, rows)| group(index == 0, rows)));
        div()
            .id("properties-overlay")
            .debug_selector(|| "properties-overlay".into())
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .title(title)
                    .leading(thin_icon(glyph, 18., c(TEXT_SECONDARY)))
                    .padded(false)
                    .on_close(cx.listener(|this, _, _, cx| {
                        this.properties = None;
                        cx.notify();
                    }))
                    .body(body),
            )
            .into_any_element()
    }

    /// Owner, group and others, each with Read, Write and Run (Open for a
    /// folder) as one control; Run only where something can run. Then "Apply
    /// to contents" for a folder, and the mode as `ls` shows it. Only the owner
    /// can change them; for anyone else the control is dimmed.
    fn permission_rows(&self, ownership: &Ownership, cx: &mut Context<Self>) -> Vec<gpui::Div> {
        let folder = ownership.is_dir();
        let names = ["Read", "Write", if folder { "Open" } else { "Run" }];
        let parts = if ownership.runnable { 3 } else { 2 };
        let accent = self.accent_hex();
        let mut rows: Vec<gpui::Div> = CLASSES
            .iter()
            .map(|(class, bits)| {
                let who = match *class {
                    "owner" => Some(ownership.owner.clone()),
                    "group" => Some(ownership.group.clone()),
                    _ => None,
                };
                let label = div()
                    .flex()
                    .gap(px(6.))
                    .child(match *class { "owner" => "Owner", "group" => "Group", _ => "Others" })
                    .when_some(who, |label, who| label.child(div().text_color(c(TEXT_PRIMARY)).child(who)));
                let segments = bits.iter().zip(names).take(parts).map(|(bit, name)| {
                    let (bit, on) = (*bit, ownership.mode & *bit != 0);
                    div()
                        .id(SharedString::from(format!("permission-{class}-{bit:o}")))
                        .debug_selector(move || format!("permission-{bit:o}"))
                        .w(px(52.))
                        .h(px(22.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .text_size(px(12.))
                        .map(|part| match on {
                            true => part.bg(alpha(accent, 0.22)).border_1().border_color(alpha(accent, 0.45)).text_color(c(TEXT_PRIMARY)),
                            false => part.text_color(c(TEXT_MUTED)),
                        })
                        .when(ownership.editable, |part| {
                            part.cursor_pointer()
                                .when(!on, |part| part.hover(|style| style.bg(c(SURFACE_TERTIARY)).text_color(c(TEXT_SECONDARY))).active(crate::components::pressed))
                                .on_click(cx.listener(move |this, _, _, cx| this.toggle_permission(bit, cx)))
                        })
                        .child(name)
                });
                let control = div()
                    .flex()
                    .gap(px(2.))
                    .p(px(2.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(BORDER_SECONDARY))
                    .bg(c(SURFACE_PRIMARY))
                    .when(!ownership.editable, |control| control.opacity(0.6))
                    .children(segments);
                row(label, control)
            })
            .collect();
        if ownership.editable && folder {
            rows.push(row(
                "Everything inside",
                Button::new("permissions-apply-inside")
                    .label("Apply to contents")
                    .variant(ButtonVariant::Secondary)
                    // As tall as the Read · Write · Open control above it.
                    .size(ButtonSize::Sm)
                    .disabled(self.operation_status.is_busy())
                    .on_click(cx.listener(|this, _, _, cx| this.apply_permissions_to_contents(cx))),
            ));
        }
        rows.push(row(
            "Mode",
            div()
                .font_family(self.fonts.mono.clone())
                .text_size(px(12.))
                .text_color(c(TEXT_SECONDARY))
                .child(SharedString::from(format!("{}  {:03o}", permission_string(ownership.mode), ownership.mode & 0o7777))),
        ));
        rows
    }
}

/// `~/code/BoxBox` for a folder under the home folder.
fn home_relative(folder: &std::path::Path) -> String {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let path = match home.as_deref().and_then(|home| folder.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => folder.display().to_string(),
    };
    keep_end(path, 44)
}

/// A long path loses its start, at a folder boundary, so the folder it names shows.
fn keep_end(path: String, limit: usize) -> String {
    let count = path.chars().count();
    if count <= limit {
        return path;
    }
    let tail: String = path.chars().skip(count - limit).collect();
    let tail = tail.find('/').map_or(tail.as_str(), |slash| &tail[slash..]);
    format!("…{tail}")
}

/// "3 files, 1 folder".
fn counted(first: u64, first_noun: &str, second: u64, second_noun: &str) -> String {
    let plural = |count: u64, noun: &str| format!("{count} {noun}{}", if count == 1 { "" } else { "s" });
    format!("{}, {}", plural(first, first_noun), plural(second, second_noun))
}
