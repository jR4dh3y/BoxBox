//! `ui/ContextMenu.svelte` with the items of `utils/fileContextMenu.ts`.

use gpui::{div, prelude::*, px, Context, MouseButton, Pixels, Point, SharedString, Window};

use crate::clipboard::ClipOp;
use crate::theme::*;
use crate::ui::{icon, TextScale};
use crate::workspace::Workspace;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    Open,
    OpenWith,
    Preview,
    OpenTerminal,
    NewFile,
    NewFolder,
    Copy,
    Cut,
    Paste,
    Pin,
    Unpin,
    Rename,
    Delete,
    Properties,
    OpenDrive,
    OpenInNewTab,
}

pub enum MenuEntry {
    Item {
        action: MenuAction,
        label: &'static str,
        icon: Option<&'static str>,
        shortcut: Option<&'static str>,
        disabled: bool,
    },
    Separator,
}

fn item(
    action: MenuAction,
    label: &'static str,
    icon: &'static str,
    shortcut: Option<&'static str>,
    disabled: bool,
) -> MenuEntry {
    MenuEntry::Item {
        action,
        label,
        icon: Some(icon),
        shortcut,
        disabled,
    }
}

pub struct ContextMenu {
    pub position: Point<Pixels>,
    pub entries: Vec<MenuEntry>,
    /// Drive menus act on this root instead of the selection.
    pub target: Option<String>,
}

pub struct MenuContext {
    pub selected: usize,
    pub single_dir: Option<(bool, bool)>, // (is_dir, is_favorite) for a single selection
    pub can_create: bool,
    pub read_only: bool,
}

/// `getFileContextMenuItems` for native paths (no download/share; those need a web server).
pub fn file_menu(context: &MenuContext) -> Vec<MenuEntry> {
    let create = [
        item(
            MenuAction::NewFile,
            "New File",
            "file-plus",
            None,
            context.read_only || !context.can_create,
        ),
        item(
            MenuAction::NewFolder,
            "New Folder",
            "folder-plus",
            None,
            context.read_only || !context.can_create,
        ),
        MenuEntry::Separator,
    ];
    let paste = item(
        MenuAction::Paste,
        "Paste",
        "clipboard-paste",
        Some("Ctrl+V"),
        context.read_only || !context.can_create,
    );
    if context.selected == 0 {
        return create.into_iter().chain([paste]).collect();
    }
    let mut entries: Vec<MenuEntry> = create.into_iter().collect();
    match context.single_dir {
        Some((true, _)) => entries.push(item(
            MenuAction::OpenTerminal,
            "Open in Terminal",
            "terminal",
            None,
            false,
        )),
        Some((false, _)) => {
            entries.push(item(
                MenuAction::Open,
                "Open",
                "external-link",
                Some("Enter"),
                false,
            ));
            entries.push(item(
                MenuAction::OpenWith,
                "Open with…",
                "app-window",
                None,
                false,
            ));
            entries.push(item(
                MenuAction::Preview,
                "Preview",
                "eye",
                Some("Space"),
                false,
            ));
        }
        None => {}
    }
    entries.push(item(
        MenuAction::Copy,
        "Copy",
        "copy",
        Some("Ctrl+C"),
        false,
    ));
    entries.push(item(
        MenuAction::Cut,
        "Cut",
        "scissors",
        Some("Ctrl+X"),
        context.read_only,
    ));
    entries.push(paste);
    if let Some((true, favorite)) = context.single_dir {
        entries.push(if favorite {
            item(
                MenuAction::Unpin,
                "Unpin from Favorites",
                "star-off",
                None,
                false,
            )
        } else {
            item(MenuAction::Pin, "Pin to Favorites", "star", None, false)
        });
    }
    entries.push(MenuEntry::Separator);
    entries.push(item(
        MenuAction::Rename,
        "Rename",
        "pencil",
        Some("F2"),
        context.read_only || context.selected > 1,
    ));
    entries.push(item(
        MenuAction::Delete,
        "Move to Trash",
        "trash-2",
        Some("Del"),
        context.read_only,
    ));
    entries.push(MenuEntry::Separator);
    entries.push(item(
        MenuAction::Properties,
        "Properties",
        "info",
        Some("Alt+Enter"),
        context.selected > 1,
    ));
    entries
}

/// `DriveContextMenu.svelte`.
pub fn drive_menu() -> Vec<MenuEntry> {
    vec![
        item(MenuAction::OpenDrive, "Open", "folder-open", None, false),
        item(
            MenuAction::OpenInNewTab,
            "Open in New Tab",
            "plus",
            None,
            false,
        ),
        item(
            MenuAction::OpenTerminal,
            "Open in Terminal",
            "terminal",
            None,
            false,
        ),
    ]
}

impl Workspace {
    /// Right-click on an item selects it alone unless it is already selected.
    pub(crate) fn open_item_menu(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let pane = self.pane_mut();
        if let Some(item) = pane.items.get(index) {
            if !pane.is_selected(&item.path.clone()) {
                let path = item.path.clone();
                pane.select_path(&path);
            }
        }
        self.open_menu(position, None, cx);
    }

    pub(crate) fn open_background_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.pane_mut().clear_selection();
        self.open_menu(position, None, cx);
    }

    pub(crate) fn open_drive_menu(
        &mut self,
        root: String,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.context_menu = Some(ContextMenu {
            position,
            entries: drive_menu(),
            target: Some(root),
        });
        cx.notify();
    }

    fn open_menu(
        &mut self,
        position: Point<Pixels>,
        target: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let selected = self.pane().selected_items();
        let single_dir = match selected.as_slice() {
            [item] => Some((item.is_dir, self.preferences.is_favorite(&item.path))),
            _ => None,
        };
        let context = MenuContext {
            selected: selected.len(),
            single_dir,
            can_create: self.can_write_here(),
            read_only: !self.can_write_here(),
        };
        self.context_menu = Some(ContextMenu {
            position,
            entries: file_menu(&context),
            target,
        });
        cx.notify();
    }

    fn run_menu_action(&mut self, action: MenuAction, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.context_menu.take().and_then(|menu| menu.target);
        let selected = self.pane().selected_items();
        match action {
            MenuAction::Open => {
                if let Some(item) = selected.first() {
                    self.open_item(item.clone(), cx);
                }
            }
            MenuAction::Preview => {
                if let Some(item) = selected.first() {
                    self.open_preview(item.clone(), window, cx);
                }
            }
            MenuAction::OpenWith => {
                if let Some(item) = selected.first() {
                    self.open_with_dialog(item.clone(), window, cx);
                }
            }
            MenuAction::OpenTerminal => {
                let path = target.or_else(|| selected.first().map(|item| item.path.clone()));
                if let Some(path) = path {
                    self.open_terminal(path, cx);
                }
            }
            MenuAction::NewFile => self.open_create(false, window, cx),
            MenuAction::NewFolder => self.open_create(true, window, cx),
            MenuAction::Copy => self.copy_selection(ClipOp::Copy, cx),
            MenuAction::Cut => self.copy_selection(ClipOp::Cut, cx),
            MenuAction::Paste => self.paste(cx),
            MenuAction::Pin => {
                if let Some(item) = selected.first() {
                    self.pin(item.name.clone(), item.path.clone(), cx);
                }
            }
            MenuAction::Unpin => {
                if let Some(item) = selected.first() {
                    self.unpin(&item.path.clone(), cx);
                }
            }
            MenuAction::Rename => self.open_rename(window, cx),
            MenuAction::Delete => self.open_delete(true, cx),
            MenuAction::Properties => self.open_properties(cx),
            MenuAction::OpenDrive => {
                if let Some(root) = target {
                    self.navigate(root, cx);
                }
            }
            MenuAction::OpenInNewTab => {
                if let Some(root) = target {
                    self.open_tab(root, window, cx);
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn render_context_menu(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let menu = self.context_menu.as_ref()?;
        // Keep the menu inside the window, like `adjustedPosition`.
        let viewport = window.viewport_size();
        let rows = menu
            .entries
            .iter()
            .map(|entry| {
                if matches!(entry, MenuEntry::Separator) {
                    9.
                } else {
                    36.
                }
            })
            .sum::<f32>()
            + 8.;
        let x = f32::from(menu.position.x)
            .min(f32::from(viewport.width) - 248.)
            .max(0.);
        let y = f32::from(menu.position.y)
            .min(f32::from(viewport.height) - rows - 8.)
            .max(0.);
        let entries = menu
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                MenuEntry::Separator => div()
                    .mx(px(8.))
                    .my(px(4.))
                    .h(px(1.))
                    .bg(c(BORDER_SECONDARY))
                    .into_any_element(),
                MenuEntry::Item {
                    action,
                    label,
                    icon: glyph,
                    shortcut,
                    disabled,
                } => {
                    let (action, disabled) = (*action, *disabled);
                    div()
                        .id(("menu-item", index))
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .px(px(12.))
                        .py(px(8.))
                        .text_13()
                        .text_color(c(if disabled {
                            TEXT_DISABLED
                        } else {
                            TEXT_PRIMARY
                        }))
                        .when(!disabled, |row| {
                            row.cursor_pointer()
                                .hover(|style| style.bg(c(SURFACE_SECONDARY)))
                        })
                        .child(
                            div()
                                .size(px(16.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .children(glyph.map(|name| {
                                    icon(
                                        name,
                                        14.,
                                        c(if disabled {
                                            TEXT_DISABLED
                                        } else {
                                            TEXT_SECONDARY
                                        }),
                                    )
                                })),
                        )
                        .child(div().flex_1().child(SharedString::from(*label)))
                        .children(shortcut.map(|text| {
                            div()
                                .text_11()
                                .text_color(c(TEXT_MUTED))
                                .child(SharedString::from(text))
                        }))
                        .when(!disabled, |row| {
                            row.on_click(cx.listener(move |this, _, window, cx| {
                                this.run_menu_action(action, window, cx)
                            }))
                        })
                        .into_any_element()
                }
            });
        Some(
            div()
                .id("menu-backdrop")
                .absolute()
                .inset_0()
                .occlude()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.context_menu = None;
                    cx.notify();
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, _, _, cx| {
                        this.context_menu = None;
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .id("menu")
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .min_w(px(180.))
                        .max_w(px(280.))
                        .py(px(4.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(c(BORDER_PRIMARY))
                        .bg(c(SURFACE_PRIMARY))
                        .shadow_xl()
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .children(entries),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions(entries: &[MenuEntry]) -> Vec<(MenuAction, bool)> {
        entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Item {
                    action, disabled, ..
                } => Some((*action, *disabled)),
                MenuEntry::Separator => None,
            })
            .collect()
    }

    #[test]
    fn folder_menu_offers_terminal_and_pin() {
        let menu = file_menu(&MenuContext {
            selected: 1,
            single_dir: Some((true, false)),
            can_create: true,
            read_only: false,
        });
        let names: Vec<_> = actions(&menu)
            .into_iter()
            .map(|(action, _)| action)
            .collect();
        assert!(names.contains(&MenuAction::OpenTerminal) && names.contains(&MenuAction::Pin));
        assert!(!names.contains(&MenuAction::OpenWith));
    }

    #[test]
    fn multiple_selection_disables_rename_and_properties() {
        let menu = actions(&file_menu(&MenuContext {
            selected: 2,
            single_dir: None,
            can_create: true,
            read_only: false,
        }));
        assert!(
            menu.contains(&(MenuAction::Rename, true))
                && menu.contains(&(MenuAction::Properties, true))
        );
        let background = actions(&file_menu(&MenuContext {
            selected: 0,
            single_dir: None,
            can_create: false,
            read_only: true,
        }));
        assert_eq!(background.last(), Some(&(MenuAction::Paste, true)));
    }
}
