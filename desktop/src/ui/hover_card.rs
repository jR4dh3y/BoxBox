//! The card a row or tile shows when the pointer rests on it, as File Pilot
//! does: what the item is, how big it is, and the details a glance needs,
//! without selecting it or opening Properties. GPUI's tooltip shows it after
//! the pointer rests and hides it when the pointer leaves or presses; what
//! takes a disk read (a folder's contents, an image's size, the app that opens
//! a file) loads off the UI thread and stops when the card closes.

use crate::components::Ellipsis as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use gpui::{div, img, prelude::*, px, AnyView, App, Context, ObjectFit, RenderImage, SharedString, WeakEntity, Window};

use super::{thin_icon, TextScale};
use crate::api::{Api, FileInfo};
use crate::components::{emblemed, motion, EntryIcon, UiFont, ROUNDED_LG};
use crate::format::{file_size, full_date, type_description};
use crate::theme::*;
use crate::workspace::properties::Totals;
use crate::workspace::{Load, Workspace};

const WIDTH: f32 = 280.;
/// Added to GPUI's fixed 500 ms tooltip delay, so the card waits for a real rest
/// and not a pass across the grid.
const EXTRA_DELAY: std::time::Duration = std::time::Duration::from_millis(250);
const PICTURE: f32 = 40.;
/// Wide enough for "Opens with", so every value starts in one column.
const LABEL_WIDTH: f32 = 76.;

enum Picture {
    Image { image: Arc<RenderImage>, thumbnail: bool, badge: Option<Arc<RenderImage>> },
    Glyph { glyph: &'static str, emblem: Option<&'static str>, tone: u32 },
}

/// What only a disk read tells.
#[derive(Default)]
struct Extra {
    /// A folder's files, folders and bytes, all the way down on this computer.
    contents: Option<Load<Totals>>,
    /// An image's width and height in pixels.
    dimensions: Option<(u32, u32)>,
    /// The app a file opens in.
    opens_with: Option<String>,
}

pub(crate) struct HoverCard {
    item: FileInfo,
    kind: String,
    location: Option<String>,
    picture: Picture,
    extra: Extra,
    /// Set when the card closes, so a folder walk stops.
    cancel: Arc<AtomicBool>,
    workspace: WeakEntity<Workspace>,
    /// A menu or drag covered the card; it stays away until the pointer leaves the item.
    dismissed: bool,
    /// Whether `EXTRA_DELAY` has passed.
    due: bool,
}

impl Drop for HoverCard {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// The `.tooltip()` builder for `item` in a pane of `workspace`.
fn builder(
    workspace: WeakEntity<Workspace>,
    api: Arc<Api>,
    item: FileInfo,
    location: Option<String>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |_, cx| {
        // Read what the views already made when the card opens, not when the row was drawn.
        let picture = workspace.upgrade().map(|workspace| picture(workspace.read(cx), &api, &item));
        let (api, item, location) = (api.clone(), item.clone(), location.clone());
        let workspace = workspace.clone();
        cx.new(|cx| HoverCard::new(workspace, api, item, location, picture, cx)).into()
    }
}

impl Workspace {
    /// The card for a pane's row or tile, unless Settings turn it off or the item is being renamed.
    pub(crate) fn hover_card(
        &self,
        target: crate::workspace::operations::PaneTarget,
        item: &FileInfo,
        cx: &mut Context<Self>,
    ) -> Option<impl Fn(&mut Window, &mut App) -> AnyView + 'static> {
        let pane = self.pane_by_id(target.0)?;
        if !self.preferences.hover_details || self.renaming(target, &item.path) {
            return None;
        }
        let location = pane.shows_locations().then(|| crate::search::location(&pane.path, &item.path).to_string());
        Some(builder(cx.entity().downgrade(), pane.api.clone(), item.clone(), location))
    }
}

fn picture(workspace: &Workspace, api: &Arc<Api>, item: &FileInfo) -> Picture {
    if let Some(image) = workspace.thumbnail(api, item) {
        // The app icon the grid or the list already made for the item's own slot.
        let sizes = [
            crate::components::GridMetrics::new(workspace.preferences.grid_zoom).theme_icon(),
            crate::components::ListMetrics::new(workspace.preferences.list_zoom).icon + 4.,
        ];
        let badge = sizes.into_iter().find_map(|size| workspace.app_badge(api, item, size));
        return Picture::Image { image, thumbnail: true, badge };
    }
    if let EntryIcon::Theme(image) = workspace.item_icon(api, item, PICTURE) {
        return Picture::Image { image, thumbnail: false, badge: None };
    }
    let style = workspace.glyph_style(api, item);
    Picture::Glyph { glyph: style.glyph, emblem: style.emblem, tone: style.tint.unwrap_or(if item.is_dir { FOLDER } else { TEXT_SECONDARY }) }
}

impl HoverCard {
    fn new(workspace: WeakEntity<Workspace>, api: Arc<Api>, item: FileInfo, location: Option<String>, picture: Option<Picture>, cx: &mut Context<Self>) -> Self {
        let kind = if item.is_dir { "Folder".to_string() } else { type_description(&item.name) };
        let picture = picture.unwrap_or(Picture::Glyph { glyph: "file", emblem: None, tone: TEXT_SECONDARY });
        let cancel = Arc::new(AtomicBool::new(false));
        // GPUI keeps a tooltip through a right-click, so the card watches for menus itself.
        if let Some(workspace) = workspace.upgrade() {
            cx.observe(&workspace, |_, _, cx| cx.notify()).detach();
        }
        let mut card = Self { item, kind, location, picture, extra: Extra::default(), cancel: cancel.clone(), workspace, dismissed: false, due: false };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(EXTRA_DELAY).await;
            this.update(cx, |card, cx| {
                card.due = true;
                cx.notify();
            })
            .ok();
        })
        .detach();
        if card.item.is_dir {
            card.extra.contents = Some(Load::Loading);
        }
        let item = card.item.clone();
        cx.spawn(async move |this, cx| {
            let extra = cx.background_executor().spawn(async move { read_extra(&api, &item, &cancel) }).await;
            this.update(cx, |card, cx| {
                card.extra = extra;
                cx.notify();
            })
            .ok();
        })
        .detach();
        card
    }
}

fn read_extra(api: &Api, item: &FileInfo, cancel: &AtomicBool) -> Extra {
    let real = api.local_path(&item.path).ok().flatten();
    if item.is_dir {
        let totals = match &real {
            Some(real) => crate::workspace::properties::local_totals(std::slice::from_ref(real), cancel),
            // A server's folder: its own listing only, one request.
            None => api.list(&item.path).ok().map(|items| Totals {
                files: items.iter().filter(|item| !item.is_dir).count() as u64,
                folders: items.iter().filter(|item| item.is_dir).count() as u64,
                bytes: items.iter().map(|item| item.size).sum(),
            }),
        };
        // The walk counts the folder itself.
        let totals = totals.map(|totals| Totals { folders: totals.folders.saturating_sub(u64::from(real.is_some())), ..totals });
        return Extra { contents: Some(totals.map_or_else(|| Load::Failed("Could not read it".into()), Load::Loaded)), ..Extra::default() };
    }
    let Some(real) = real else { return Extra::default() };
    let dimensions = crate::local::mime::from_name(&real)
        .filter(|mime| mime.starts_with("image/"))
        .and_then(|_| image::image_dimensions(&real).ok());
    #[cfg(target_os = "linux")]
    let opens_with = crate::open_with::recommended(&real).ok().and_then(|apps| apps.into_iter().find(|app| app.is_default)).map(|app| app.name);
    #[cfg(windows)]
    let opens_with = crate::windows::default_app_name(&real);
    Extra { contents: None, dimensions, opens_with }
}

/// The label in its column and the value beside it, wrapping under itself.
fn row(label: &'static str, value: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .items_start()
        .gap(px(8.))
        .child(div().w(px(LABEL_WIDTH)).flex_none().text_color(c(TEXT_MUTED)).child(label))
        .child(div().flex_1().min_w_0().text_color(c(TEXT_PRIMARY)).child(value.into()))
}

fn plural(count: u64, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

impl Render for HoverCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let covered = cx.has_active_drag() || self.workspace.upgrade().is_none_or(|workspace| workspace.read(cx).covers_hover_cards());
        if covered && !self.dismissed {
            self.dismissed = true;
            self.cancel.store(true, Ordering::Relaxed);
        }
        if self.dismissed || !self.due {
            return div().into_any_element();
        }
        let frame = div().size(px(PICTURE)).flex_none().relative().flex().items_center().justify_center().overflow_hidden().rounded(px(4.));
        let picture = match &self.picture {
            Picture::Image { image, thumbnail: true, badge } => frame
                .child(img(image.clone()).size_full().object_fit(ObjectFit::Cover))
                .when_some(badge.clone(), |frame, app| frame.child(div().absolute().left(px(2.)).bottom(px(2.)).size(px(16.)).child(img(app).size_full()))),
            Picture::Image { image, thumbnail: false, .. } => frame.child(img(image.clone()).size(px(PICTURE))),
            Picture::Glyph { glyph, emblem, tone } => frame.child(emblemed(thin_icon(glyph, PICTURE - 4., c(*tone)), *emblem, PICTURE - 4., c(*tone))),
        };
        let mut rows = Vec::new();
        match &self.extra.contents {
            Some(Load::Loaded(totals)) => {
                let mut parts = Vec::new();
                if totals.folders > 0 {
                    parts.push(plural(totals.folders, "folder", "folders"));
                }
                parts.push(plural(totals.files, "file", "files"));
                rows.push(row("Contains", parts.join(", ")));
                rows.push(row("Size", file_size(totals.bytes)));
            }
            Some(Load::Loading) => rows.push(row("Contains", "Counting…")),
            Some(Load::Failed(error)) => rows.push(row("Contains", error.clone())),
            None => rows.push(row("Size", file_size(self.item.size))),
        }
        if let Some((width, height)) = self.extra.dimensions {
            rows.push(row("Dimensions", format!("{width} × {height}")));
        }
        if let Some(app) = &self.extra.opens_with {
            rows.push(row("Opens with", app.clone()));
        }
        rows.push(row("Modified", full_date(self.item.mod_time)));
        if let Some(location) = &self.location {
            rows.push(row("Location", location.clone()));
        }
        let card = div()
            .when_some(cx.try_global::<UiFont>(), |card, font| card.font_family(font.0.clone()))
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(12.))
            .rounded(px(ROUNDED_LG))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_ELEVATED))
            .shadow_lg()
            .text_xs_tw()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(picture)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(div().truncate().text_sm_tw().text_color(c(TEXT_PRIMARY)).child(self.item.name.clone()))
                            .child(div().truncate().text_color(c(TEXT_SECONDARY)).child(self.kind.clone())),
                    ),
            )
            .child(div().h(px(1.)).bg(c(BORDER_SECONDARY)))
            .child(div().flex().flex_col().gap(px(4.)).children(rows));
        motion::enter("hover-card-enter", card, motion::POPOVER, cx)
    }
}

impl Workspace {
    /// Whether a menu or dialog is open, which a hover card would cover.
    fn covers_hover_cards(&self) -> bool {
        self.context_menu.is_some()
            || self.place_menu.is_some()
            || self.history_menu.is_some()
            || self.drop_menu.is_some()
            || self.add_menu.is_some()
            || self.confirm.is_some()
            || self.properties.is_some()
            || self.settings_open
    }
}

/// A drive's card in the sidebar: what it is, how full, and where it lives,
/// as the Overview's drive card shows it, for the sidebar's narrow rows.
pub(crate) struct DriveHover {
    info: crate::components::DriveInfo,
    /// Where the drive is: this computer, a disk, or the server it belongs to.
    origin: SharedString,
    details: Vec<(&'static str, String)>,
}

impl Workspace {
    /// The card for a sidebar drive, unless Settings turn hover details off.
    pub(crate) fn drive_hover(&self, storage: &crate::workspace::overview::Storage) -> Option<impl Fn(&mut Window, &mut App) -> AnyView + 'static> {
        use crate::workspace::overview::Storage;
        if !self.preferences.hover_details {
            return None;
        }
        let info = self.drive_info(storage);
        let mut details = Vec::new();
        let origin: SharedString = match storage {
            Storage::Drive(stats) => {
                if let Ok(Some(real)) = self.home.api.local_path(&stats.name) {
                    details.push(("Location", real.display().to_string()));
                }
                "This computer".into()
            }
            Storage::Volume(volume) => {
                if let Some(point) = volume.mount_point() {
                    details.push(("Location", point.display().to_string()));
                }
                match volume.kind {
                    crate::devices::Kind::Disk => "Disk",
                    crate::devices::Kind::Phone => "Phone",
                    crate::devices::Kind::Network => "Network share",
                }
                .into()
            }
            Storage::ServerDrive { server, .. } | Storage::Remembered { server, .. } => server.name.clone().into(),
            Storage::Server { reason, .. } => {
                if let Some(reason) = reason {
                    details.push(("Reason", reason.clone()));
                }
                "Server".into()
            }
        };
        // A whole server's device is its address.
        let device_label = if matches!(storage, Storage::Server { .. }) { "Address" } else { "Device" };
        if let Some(device) = info.device.clone().filter(|device| !device.is_empty()) {
            details.push((device_label, device));
        }
        if let Some(fs) = info.fs_type.clone().filter(|fs| !fs.is_empty()) {
            details.push(("Filesystem", fs));
        }
        if info.read_only {
            details.push(("Access", "Read-only".into()));
        }
        Some(move |_: &mut Window, cx: &mut App| -> AnyView {
            cx.new(|_| DriveHover { info: info.clone(), origin: origin.clone(), details: details.clone() }).into()
        })
    }
}

/// A detail kept to one line and cut short with an ellipsis; a reason is a
/// sentence and wraps.
fn drive_row(label: &'static str, value: impl Into<SharedString>) -> gpui::Div {
    let value = div().flex_1().min_w_0().text_color(c(TEXT_PRIMARY)).child(value.into());
    div()
        .flex()
        .items_start()
        .gap(px(8.))
        .child(div().w(px(LABEL_WIDTH)).flex_none().text_color(c(TEXT_MUTED)).child(label))
        .child(if label == "Reason" { value } else { value.ellipsis() })
}

impl Render for DriveHover {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::components::{card_usage_bar, DriveState};
        let usage: gpui::AnyElement = match &self.info.state {
            DriveState::Usage(usage) => div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(card_usage_bar(usage.pct))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .gap(px(8.))
                        .child(div().min_w_0().truncate().text_color(c(TEXT_PRIMARY)).child(format!("{} / {}", file_size(usage.used), file_size(usage.total))))
                        .child(div().flex_none().text_color(c(TEXT_SECONDARY)).child(format!("{:.0}% used", usage.pct))),
                )
                .into_any_element(),
            DriveState::Status { status, size } => div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(drive_row("Status", status.clone()))
                .when_some(*size, |rows, size| rows.child(drive_row("Size", file_size(size))))
                .into_any_element(),
            DriveState::Pending { status, .. } => drive_row("Status", status.clone()).into_any_element(),
        };
        let card = div()
            .when_some(cx.try_global::<UiFont>(), |card, font| card.font_family(font.0.clone()))
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(12.))
            .rounded(px(ROUNDED_LG))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_ELEVATED))
            .shadow_lg()
            .text_xs_tw()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(8.))
                    .child(div().flex_1().min_w_0().ellipsis().text_sm_tw().text_color(c(TEXT_PRIMARY)).child(self.info.name.clone()))
                    .child(div().flex_none().max_w(px(WIDTH / 2.)).truncate().text_color(c(TEXT_MUTED)).child(self.origin.clone())),
            )
            .child(usage)
            .when(!self.details.is_empty(), |card| {
                card.child(div().h(px(1.)).bg(c(BORDER_SECONDARY)))
                    .child(div().flex().flex_col().gap(px(4.)).children(self.details.iter().map(|(label, value)| drive_row(label, value.clone()))))
            });
        motion::enter("drive-hover-enter", card, motion::POPOVER, cx)
    }
}
