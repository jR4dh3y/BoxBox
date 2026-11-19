//! `lib/components/Sidebar.svelte`: Drives (the Overview, the system and every device), Places, Trash and Favorites.

use gpui::{div, prelude::*, px, Context, SharedString};

use super::{TextScale, SIDEBAR_WIDTH};
use crate::api::RootKind;
use crate::components::{self, SidebarSection};
use crate::theme::*;
use crate::workspace::drag_drop::{drop_zone, DropTarget};
use crate::workspace::overview::Storage;
use crate::drag_out::DragPreview;
use crate::workspace::bookmarks::FavoriteDrag;
use crate::workspace::drive_names::DriveKey;
use crate::workspace::place_menu::Place;
use crate::workspace::Workspace;

struct NavItem {
    name: SharedString,
    path: String,
    icon: &'static str,
    /// A drive's own name, which Reset Name returns to; places have none.
    own: Option<String>,
    /// A drive's size and use, for its hover card; places have none.
    storage: Option<Storage>,
}

fn place_icon(name: &str) -> &'static str {
    match name.to_lowercase().as_str() {
        "home" => "home",
        "desktop" => "monitor",
        "downloads" => "download",
        "documents" => "file-text",
        "music" => "music",
        "pictures" => "image",
        "videos" => "video",
        "templates" => "layout-template",
        "recent" => "clock",
        "network" => "network",
        _ => "folder",
    }
}

/// "media" owns "media/movies" but not "media2".
fn is_active(current: &str, path: &str) -> bool {
    if path.is_empty() {
        return current.is_empty();
    }
    current == path || current.starts_with(&format!("{path}/"))
}

impl Workspace {
    /// A collapsible section whose fold state lives on the workspace.
    fn section(&self, title: &'static str, cx: &mut Context<Self>) -> SidebarSection {
        SidebarSection::new(SharedString::from(format!("section-{title}")), title).collapsible(
            self.collapsed_sections.contains(title),
            self.section_toggles.get(title).copied(),
            cx.listener(move |this, _, _, cx| {
                if !this.collapsed_sections.remove(title) {
                    this.collapsed_sections.insert(title);
                }
                this.section_toggles.insert(title, std::time::Instant::now());
                cx.notify();
            }),
        )
    }

    /// A row for a drive or place of the window's own provider. While the pane is
    /// on a server it still opens here, but takes no drops and has no menu, which
    /// act on the pane's provider.
    fn nav_item(&self, id: String, item: NavItem, current: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let path = item.path;
        let home = std::sync::Arc::ptr_eq(&self.pane().api, &self.home.api);
        let card = item.storage.as_ref().and_then(|storage| self.drive_hover(storage));
        let row = components::NavItem::new(SharedString::from(id.clone()), item.name, item.icon).active(home && is_active(current, &path));
        let row = match card {
            Some(card) => row.hover_card(card),
            None => row,
        };
        let row = row.on_click({
            let path = path.clone();
            cx.listener(move |this, _, _, cx| {
                let api = this.home.api.clone();
                this.navigate_to(this.active_pane, api, path.clone(), cx);
            })
        });
        let selector = move || format!("sidebar-{id}");
        if !home {
            return div().child(row).debug_selector(selector);
        }
        let menu = match item.own {
            Some(own) => Place::Drive { key: DriveKey::root(&self.home.api, &path), own, folder: Some(path.clone()) },
            None => Place::Folder { pane: self.active_pane, path: path.clone() },
        };
        // Recent and Network are listings: nothing is dropped into them.
        let folder = (path != crate::local::RECENT && path != crate::local::NETWORK).then(|| path.clone());
        self.sidebar_row(row, folder, menu, cx).debug_selector(selector)
    }

    /// Each paired server's drives, or one row for a server that has none to show yet.
    fn server_rows(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let pane = self.pane();
        let storage = self.storage().unwrap_or_default();
        storage
            .into_iter()
            .filter(|entry| matches!(entry, Storage::ServerDrive { .. } | Storage::Server { .. } | Storage::Remembered { .. }))
            .enumerate()
            .map(|(index, entry)| {
                let info = self.drive_info(&entry);
                let (detail, waiting, tooltip) = match &entry {
                    Storage::ServerDrive { server, .. } => (server.name.clone(), false, server.url.clone()),
                    Storage::Server { server, status, reason } => ((*status).to_string(), true, reason.clone().unwrap_or_else(|| server.url.clone())),
                    Storage::Remembered { server, status, reason, .. } => (server.name.clone(), true, reason.clone().unwrap_or_else(|| (*status).to_string())),
                    _ => unreachable!("filtered above"),
                };
                let active = entry.holds(self, &pane.api, &pane.path);
                let row = components::NavItem::new(SharedString::from(format!("server-{index}")), info.name, "server")
                    .detail(detail)
                    .muted(waiting)
                    .active(active);
                // The drive's card when hover details are on, else its address or status.
                let row = match self.drive_hover(&entry) {
                    Some(card) => row.hover_card(card),
                    None => row.tooltip(tooltip),
                };
                let row = match info.state.wait() {
                    Some(wait) => row.waiting(wait),
                    None => row,
                };
                let menu = self.drive_key(&entry).map(|(key, own)| Place::Drive { key, own, folder: None });
                let row = row.on_click(cx.listener(move |this, _, window, cx| this.open_storage(entry.clone(), window, cx)));
                match menu {
                    Some(menu) => div()
                        .child(row)
                        .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.open_place_menu(menu.clone(), event.position, cx);
                        }))
                        .into_any_element(),
                    None => row.into_any_element(),
                }
            })
            .collect()
    }

    /// A sidebar row with its right-click menu, taking files dropped into `folder`.
    pub(crate) fn sidebar_row(&self, row: components::NavItem, folder: Option<String>, menu: Place, cx: &mut Context<Self>) -> gpui::Div {
        let element = match folder {
            Some(path) => {
                let target = DropTarget::Place { path };
                let row = row.drop_target(self.drop_hover(cx) == Some(&target));
                drop_zone(div(), target, cx.entity().downgrade()).child(row)
            }
            None => div().child(row),
        };
        element.on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
            cx.stop_propagation();
            this.open_place_menu(menu.clone(), event.position, cx);
        }))
    }

    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane();
        let current = pane.path.clone();
        let provider = crate::workspace::favorites::FavoriteProvider::for_api(&pane.api);
        let home_provider = crate::workspace::favorites::FavoriteProvider::for_api(&self.home.api);
        // The window's own drives and places, whatever provider the pane is on.
        let roots: &[crate::api::MountPoint] = match &self.home.roots {
            crate::pane::Load::Loaded(roots) => roots,
            _ => &[],
        };
        // Read once for every drive row's card.
        let storage = self.storage().unwrap_or_default();
        let stats = |name: &str| storage.iter().find(|entry| matches!(entry, Storage::Drive(drive) if drive.name == name)).cloned();
        let mut drives = std::iter::once(NavItem { name: "Overview".into(), path: String::new(), icon: "layout-dashboard", own: None, storage: None })
            .chain(roots.iter().filter(|root| root.kind != RootKind::Place).map(|root| {
                let own = root.display_name().to_string();
                let key = DriveKey::root(&self.home.api, &root.name);
                NavItem { name: self.drive_names.name(&key, &own).to_string().into(), path: root.name.clone(), icon: "hard-drive", own: Some(own), storage: stats(&root.name) }
            }))
            .enumerate()
            .map(|(index, item)| self.nav_item(format!("drive-{index}"), item, &current, cx).into_any_element())
            .collect::<Vec<_>>();
        if self.home.api.is_local() {
            drives.extend(self.device_rows(cx).into_iter().map(IntoElement::into_any_element));
        }
        drives.extend(self.server_rows(cx));
        let places = roots
            .iter()
            .filter(|root| root.kind == RootKind::Place)
            .map(|root| NavItem {
                name: capitalize(root.display_name()).into(),
                path: root.name.clone(),
                icon: place_icon(&root.name),
                own: None,
                storage: None,
            })
            .enumerate()
            .map(|(index, item)| self.nav_item(format!("place-{index}"), item, &current, cx))
            .collect::<Vec<_>>();
        let favorites = self
            .favorites
            .entries
            .iter()
            .enumerate()
            // A bookmark of a drive or place, such as GTK's default Documents or Music, is
            // already in the sidebar; it stays in the file for other apps.
            .filter(|(_, favorite)| favorite.provider != home_provider || !roots.iter().any(|root| root.name == favorite.path))
            .map(|(index, favorite)| {
                let row = components::NavItem::new(SharedString::from(format!("favorite-{index}")), favorite.name.clone(), "star")
                    .trailing("x", format!("Unpin {}", favorite.name), cx.listener(move |this, _, _, cx| this.remove_favorite(index, cx)))
                    .on_click(cx.listener(move |this, _, _, cx| this.open_favorite(index, cx)));
                // A favorite on another provider is not a folder this pane can write to.
                let folder = (favorite.provider == provider).then(|| favorite.path.clone());
                let row = self.sidebar_row(row, folder, Place::Favorite(index), cx);
                // Dragged onto another favorite, it takes that one's place.
                let name: SharedString = favorite.name.clone().into();
                div()
                    .id(("favorite-drag", index))
                    .debug_selector(move || format!("favorite-row-{index}"))
                    .on_drag(FavoriteDrag(index), move |_, grip, _, cx| cx.new(|_| DragPreview { label: name.clone(), grip }))
                    .drag_over::<FavoriteDrag>(|style, _, _, _| style.border_t_2().border_color(c(ACCENT)))
                    .on_drop(cx.listener(move |this, dragged: &FavoriteDrag, _, cx| this.drop_favorite(dragged.0, index, cx)))
                    .child(row)
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let no_favorites = favorites.is_empty();

        div()
            .id("sidebar")
            .debug_selector(|| "sidebar".into())
            .w(px(SIDEBAR_WIDTH))
            .min_w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .flex_none()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(c(BORDER_SECONDARY))
            .bg(self.shell_background())
            .child(self.section("Drives", cx).children(drives))
            .when(!places.is_empty(), |sidebar| sidebar.child(self.section("Places", cx).children(places)))
            .when(pane.api.is_local(), |sidebar| {
                // trash-2 has the lines of a full bin; trash is the empty one.
                let row = components::NavItem::new("trash", "Trash", if self.trash_full { "trash-2" } else { "trash" })
                    .dim_icon(false)
                    .active(self.trash_open)
                    .drop_target(self.drop_hover(cx) == Some(&DropTarget::Trash))
                    .on_click(cx.listener(|this, _, _, cx| this.open_trash(cx)));
                sidebar.child(
                    drop_zone(div(), DropTarget::Trash, cx.entity().downgrade())
                        .debug_selector(|| "sidebar-trash".into())
                        .on_mouse_down(gpui::MouseButton::Right, cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                            this.open_place_menu(Place::Trash, event.position, cx);
                        }))
                        .child(row),
                )
            })
            .child(self.section("Favorites", cx).children(favorites).when(no_favorites, |section| {
                // px-5 py-2 text-xs text-text-muted
                section.child(div().px(px(20.)).py(px(8.)).text_xs_tw().text_color(c(TEXT_MUTED)).child("No favorites yet"))
            }))
    }
}

/// Places use CSS `capitalize`: the first letter of each word.
fn capitalize(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}
