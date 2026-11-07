//! `lib/components/Sidebar.svelte`: Drives, Places, Trash and Favorites.

use gpui::{div, prelude::*, px, AnyElement, Context, FontWeight, SharedString};

use super::{icon, TextScale, SIDEBAR_WIDTH};
use crate::api::RootKind;
use crate::theme::*;
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

fn section_header(title: &'static str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(12.))
        .py(px(10.))
        .text_11()
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(TEXT_SECONDARY))
        .hover(|style| style.text_color(c(TEXT_PRIMARY)))
        .child(icon("chevron-down", 14., c(TEXT_SECONDARY)))
        .child(title.to_uppercase())
}

impl Workspace {
    fn nav_item(
        &self,
        id: SharedString,
        item: NavItem,
        active: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = item.path;
        div()
            .id(id)
            .w_full()
            .flex()
            .items_center()
            .gap(px(10.))
            .py(px(6.))
            .pl(px(20.))
            .pr(px(12.))
            .text_13()
            .cursor_pointer()
            // `navItemActiveClass` adds bg-selection, but the built CSS lets the base
            // bg-transparent win, so only the white text shows. Match what renders.
            .text_color(c(if active { WHITE } else { TEXT_PRIMARY }))
            .hover(|style| style.bg(c(SURFACE_SECONDARY)))
            .child(icon(
                item.icon,
                16.,
                alpha(if active { WHITE } else { TEXT_PRIMARY }, 0.8),
            ))
            .child(div().flex_1().min_w_0().truncate().child(item.name))
            .when(!path.is_empty(), |item| {
                let (dest, external) = (path.clone(), path.clone());
                item.drag_over::<crate::drag_out::DraggedItem>(|style, _, _, _| {
                    style.bg(c(SURFACE_SECONDARY))
                })
                .on_drop(cx.listener(
                    move |this, dragged: &crate::drag_out::DraggedItem, window, cx| {
                        this.drop_dragged(dragged, dest.clone(), window, cx)
                    },
                ))
                .on_drop(cx.listener(
                    move |this, paths: &gpui::ExternalPaths, _, cx| {
                        this.drop_external(paths, external.clone(), cx)
                    },
                ))
            })
            .on_click(cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx)))
            .into_any_element()
    }

    pub(crate) fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.pane().path.clone();
        let drives = std::iter::once(NavItem {
            name: "This PC".into(),
            path: String::new(),
            icon: "server",
        })
        .chain(
            self.roots
                .iter()
                .filter(|root| root.kind != RootKind::Place)
                .map(|root| NavItem {
                    name: root.display_name().to_string().into(),
                    path: root.name.clone(),
                    icon: "hard-drive",
                }),
        )
        .collect::<Vec<_>>();
        let places = self
            .roots
            .iter()
            .filter(|root| root.kind == RootKind::Place)
            .map(|root| NavItem {
                name: capitalize(root.display_name()).into(),
                path: root.name.clone(),
                icon: place_icon(&root.name),
            })
            .collect::<Vec<_>>();

        let section = || div().border_b_1().border_color(c(BORDER_SECONDARY));
        let mut drive_rows = Vec::new();
        for (index, item) in drives.into_iter().enumerate() {
            let active = is_active(&current, &item.path);
            drive_rows.push(self.nav_item(format!("drive-{index}").into(), item, active, cx));
        }
        let mut place_rows = Vec::new();
        for (index, item) in places.into_iter().enumerate() {
            let active = is_active(&current, &item.path);
            place_rows.push(self.nav_item(format!("place-{index}").into(), item, active, cx));
        }

        let favorites_empty = self.preferences.favorite_folders.is_empty();
        let mut favorite_rows = Vec::new();
        for (index, favorite) in self.preferences.favorite_folders.iter().enumerate() {
            let active = is_active(&current, &favorite.path);
            let path = favorite.path.clone();
            let item = NavItem {
                name: favorite.name.clone().into(),
                path: favorite.path.clone(),
                icon: "star",
            };
            favorite_rows.push(
                div()
                    .id(("favorite", index))
                    .relative()
                    .group("favorite")
                    .child(self.nav_item(format!("favorite-item-{index}").into(), item, active, cx))
                    .child(
                        div()
                            .id(("unpin", index))
                            .absolute()
                            .top(px(4.))
                            .right(px(8.))
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .invisible()
                            .group_hover("favorite", |style| style.visible())
                            .hover(|style| style.bg(c(SURFACE_SECONDARY)))
                            .child(icon("x", 14., c(TEXT_MUTED)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.unpin(&path, cx);
                            })),
                    )
                    .into_any_element(),
            );
        }

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
            .bg(c(SURFACE_PRIMARY))
            .child(
                section()
                    .child(section_header("Drives"))
                    .child(div().pb(px(8.)).children(drive_rows)),
            )
            .when(!place_rows.is_empty(), |sidebar| {
                sidebar.child(
                    section()
                        .child(section_header("Places"))
                        .child(div().pb(px(8.)).children(place_rows)),
                )
            })
            .children(self.render_devices(cx))
            // The desktop shell passes onTrash, so the Trash entry is always shown.
            .child(
                div()
                    .id("trash")
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .py(px(6.))
                    .pl(px(20.))
                    .pr(px(12.))
                    .text_13()
                    .cursor_pointer()
                    .hover(|style| style.bg(c(SURFACE_SECONDARY)))
                    .when(current == crate::trash::TRASH_PATH, |row| {
                        row.text_color(c(WHITE))
                    })
                    .child(icon("trash-2", 16., c(TEXT_PRIMARY)))
                    .child("Trash")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.navigate(crate::trash::TRASH_PATH.to_string(), cx)
                    })),
            )
            .child(
                section().child(section_header("Favorites")).child(
                    div()
                        .pb(px(8.))
                        .children(favorite_rows)
                        .when(favorites_empty, |list| {
                            list.child(
                                div()
                                    .px(px(20.))
                                    .py(px(8.))
                                    .text_xs_tw()
                                    .text_color(c(TEXT_MUTED))
                                    .child("No favorites yet"),
                            )
                        }),
                ),
            )
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
