use std::rc::Rc;

use anyhow::Context as _;
use gpui::{Context, Window};

use crate::components::MenuItem;
use crate::windows::{
    context_menu::{Branch, Entry, ShellMenu, Target},
    window_id,
};

use super::Workspace;

#[derive(Default)]
pub(crate) struct WindowsMenu {
    request: Rc<()>,
    pending: bool,
    shell: Option<Rc<ShellMenu>>,
    pages: Vec<Vec<Entry>>,
}

impl WindowsMenu {
    pub(crate) fn depth(&self) -> usize {
        self.pages.len().saturating_sub(1)
    }
}

impl Workspace {
    pub(super) fn load_windows_context_menu(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(menu) = &self.context_menu else {
            return;
        };
        let Some(pane) = self.pane_by_id(menu.target_id) else {
            return;
        };
        let Some(owner) = window_id(window) else {
            return;
        };
        if !pane.api.is_local() || (menu.items.is_empty() && pane.real_path.is_none()) {
            return;
        }
        let api = pane.api.clone();
        let folder = pane.real_path.clone();
        let items: Vec<_> = menu.items.iter().map(|item| item.path.clone()).collect();
        let request = menu.windows.request.clone();
        let shift = window.modifiers().shift;
        if let Some(menu) = &mut self.context_menu {
            menu.windows.pending = true;
        }
        cx.spawn(async move |this, cx| {
            let target = cx
                .background_executor()
                .spawn(async move {
                    if items.is_empty() {
                        return Target::folder(&folder.context("this folder has no Windows path")?);
                    }
                    let paths = items
                        .iter()
                        .map(|item| {
                            api.entry_path(item)?
                                .context("this item has no Windows path")
                        })
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    Target::items(&paths)
                })
                .await;
            let current = this
                .update(cx, |this, _| {
                    this.context_menu
                        .as_ref()
                        .is_some_and(|menu| Rc::ptr_eq(&menu.windows.request, &request))
                })
                .unwrap_or(false);
            if !current {
                return;
            }
            // Shell callbacks can pump Win32 messages. Run them outside a GPUI entity update.
            let shell = target.and_then(|target| ShellMenu::new(owner, target, shift));
            let result = shell.map(|shell| (shell.entries(), Rc::new(shell)));
            this.update(cx, |this, cx| {
                let Some(menu) = &mut this.context_menu else {
                    return;
                };
                if Rc::ptr_eq(&menu.windows.request, &request) {
                    menu.windows.pending = false;
                    if let Ok((entries, shell)) = result {
                        menu.windows.pages.push(entries);
                        menu.windows.shell = Some(shell);
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn windows_context_entries(
        &self,
        menu: &WindowsMenu,
        cx: &mut Context<Self>,
    ) -> Vec<MenuItem> {
        if menu.pending {
            return vec![
                MenuItem::action("windows-loading", "Loading Windows commands…").disabled(true),
            ];
        }
        let Some(page) = menu.pages.last() else {
            return Vec::new();
        };
        let mut items = Vec::new();
        if menu.depth() > 0 {
            items.push(
                MenuItem::action("windows-back", "Back")
                    .icon("chevron-left")
                    .on_select(cx.listener(|this, _, _, cx| {
                        if let Some(menu) = &mut this.context_menu {
                            menu.windows.pages.pop();
                            cx.notify();
                        }
                    })),
            );
            items.push(MenuItem::Separator);
        }
        for (index, entry) in page.iter().enumerate() {
            items.push(match entry {
                Entry::Action {
                    id,
                    label,
                    shortcut,
                    disabled,
                } => {
                    let id = *id;
                    let mut item = MenuItem::action(("windows-command", index), label.clone())
                        .disabled(*disabled)
                        .on_select(cx.listener(move |this, _, window, cx| {
                            this.run_windows_menu_action(window, Some(id), cx)
                        }));
                    if let Some(keys) = shortcut {
                        item = item.shortcut(keys.clone());
                    }
                    item
                }
                Entry::Submenu {
                    branch,
                    label,
                    disabled,
                } => {
                    let branch = *branch;
                    MenuItem::action(("windows-submenu", index), label.clone())
                        .submenu_link()
                        .disabled(*disabled)
                        .on_select(
                            cx.listener(move |this, _, _, cx| this.expand_windows_menu(branch, cx)),
                        )
                }
                Entry::Separator => MenuItem::Separator,
            });
        }
        if !page.is_empty() {
            items.push(MenuItem::Separator);
        }
        items.push(
            MenuItem::action("windows-classic", "Open classic Windows menu…")
                .icon("app-window")
                .on_select(cx.listener(|this, _, window, cx| {
                    this.run_windows_menu_action(window, None, cx)
                })),
        );
        items
    }

    fn expand_windows_menu(&mut self, branch: Branch, cx: &mut Context<Self>) {
        let Some(menu) = &self.context_menu else {
            return;
        };
        let Some(shell) = menu.windows.shell.clone() else {
            return;
        };
        let depth = menu.windows.depth();
        cx.spawn(async move |this, cx| {
            let entries = shell.expand(branch);
            this.update(cx, |this, cx| {
                let Some(menu) = &mut this.context_menu else {
                    return;
                };
                if menu.windows.depth() == depth
                    && menu
                        .windows
                        .shell
                        .as_ref()
                        .is_some_and(|open| Rc::ptr_eq(open, &shell))
                {
                    menu.windows.pages.push(entries);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn run_windows_menu_action(
        &mut self,
        window: &Window,
        command: Option<u32>,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.context_menu.take() else {
            return;
        };
        let Some(shell) = menu.windows.shell else {
            return;
        };
        let scale = window.scale_factor();
        let anchor = (
            (f32::from(menu.position.x) * scale).round() as i32,
            (f32::from(menu.position.y) * scale).round() as i32,
        );
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match command {
                Some(id) => shell.invoke(id, anchor),
                None => shell.popup(anchor),
            };
            if let Err(error) = result {
                this.update(cx, |this, cx| {
                    this.report(Err(format!("Windows Shell command failed: {error:#}")), cx)
                })
                .ok();
            }
        })
        .detach();
    }
}
