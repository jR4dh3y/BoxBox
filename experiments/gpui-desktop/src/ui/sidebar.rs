//! `lib/components/Sidebar.svelte`: Drives, Places, Trash and Favorites.

use gpui::{div, prelude::*, px, Context, SharedString};

use super::{TextScale, SIDEBAR_WIDTH};
use crate::api::RootKind;
use crate::components::{self, SidebarSection};
use crate::theme::*;
use crate::workspace::drag_drop::{drop_zone, DropTarget};
use crate::workspace::Workspace;

struct NavItem {
    name: SharedString,
    path: String,
    icon: &'static str,
}

fn place_icon(name: &str) -> &'static str {
    match name.to_lowercase().as_str() {
        "desktop" => "monitor",
        "downloads" => "download",
        "documents" => "file-text",
        "music" => "music",
        "pictures" => "image",
        "videos" => "video",
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
            cx.listener(move |this, _, _, cx| {
                if !this.collapsed_sections.remove(title) {
                    this.collapsed_sections.insert(title);
                }
                cx.notify();
            }),
        )
    }

    fn nav_item(&self, id: String, item: NavItem, current: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let path = item.path;
        let row = components::NavItem::new(SharedString::from(id), item.name, item.icon).active(is_active(current, &path)).on_click({
            let path = path.clone();
            cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx))
        });
        self.drop_place(row, path, cx)
    }

    /// Lets files be dropped onto a sidebar row to put them in its folder.
    pub(crate) fn drop_place(&self, row: components::NavItem, path: String, cx: &mut Context<Self>) -> impl IntoElement {
        let target = DropTarget::Place { path };
        let row = row.drop_target(self.drop_hover(cx) == Some(&target));
        drop_zone(div(), target, cx.entity().downgrade()).child(row)
    }

    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane();
        let current = pane.path.clone();
        let provider = crate::workspace::favorites::FavoriteProvider::for_api(&pane.api);
        let roots: &[crate::api::MountPoint] = match &pane.roots {
            crate::pane::Load::Loaded(roots) => roots,
            _ => &[],
        };
        let drives = std::iter::once(NavItem { name: "This PC".into(), path: String::new(), icon: "server" })
            .chain(roots.iter().filter(|root| root.kind != RootKind::Place).map(|root| NavItem {
                name: root.display_name().to_string().into(),
                path: root.name.clone(),
                icon: "hard-drive",
            }))
            .enumerate()
            .map(|(index, item)| self.nav_item(format!("drive-{index}"), item, &current, cx))
            .collect::<Vec<_>>();
        let places = roots
            .iter()
            .filter(|root| root.kind == RootKind::Place)
            .map(|root| NavItem {
                name: capitalize(root.display_name()).into(),
                path: root.name.clone(),
                icon: place_icon(&root.name),
            })
            .enumerate()
            .map(|(index, item)| self.nav_item(format!("place-{index}"), item, &current, cx))
            .collect::<Vec<_>>();
        let favorites = self
            .favorites
            .entries
            .iter()
            .enumerate()
            .map(|(index, favorite)| {
                let row = components::NavItem::new(SharedString::from(format!("favorite-{index}")), favorite.name.clone(), "star")
                    .trailing("x", format!("Unpin {}", favorite.name), cx.listener(move |this, _, _, cx| this.remove_favorite(index, cx)))
                    .on_click(cx.listener(move |this, _, _, cx| this.open_favorite(index, cx)));
                // A favorite on another provider is not a folder this pane can write to.
                match favorite.provider == provider {
                    true => self.drop_place(row, favorite.path.clone(), cx).into_any_element(),
                    false => row.into_any_element(),
                }
            })
            .collect::<Vec<_>>();
        let no_favorites = favorites.is_empty();

        div()
            .id("sidebar")
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
            .when(pane.api.is_local() && self.devices.volumes().next().is_some(), |sidebar| sidebar.child(self.render_devices(cx)))
            .when(pane.api.is_local(), |sidebar| {
                sidebar.child(
                    div().debug_selector(|| "sidebar-trash".into()).child(
                        components::NavItem::new("trash", "Trash", "trash-2")
                            .dim_icon(false)
                            .active(self.trash_open)
                            .on_click(cx.listener(|this, _, _, cx| this.open_trash(cx))),
                    ),
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
