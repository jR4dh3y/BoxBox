use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use gpui::{Context, RenderImage, Window};

use crate::components::MenuItem;
use crate::windows::{
    context_menu::{Branch, Entry, Icon, ShellMenu, Target},
    window_id,
};

use super::Workspace;

/// How long a right-click waits for Windows commands before the menu opens without them.
/// A warm Shell menu takes about 60 ms; a dead network drive can take seconds.
const SHELL_MENU_LIMIT: Duration = Duration::from_millis(200);

/// Distinct Shell menu icons kept on the GPU. Menus reuse a few dozen.
const ICON_LIMIT: usize = 256;

#[derive(Default)]
pub(crate) struct WindowsMenu {
    request: Rc<()>,
    pending: bool,
    shell: Option<Rc<ShellMenu>>,
    pages: Vec<Vec<Row>>,
}

struct Row {
    entry: Entry,
    image: Option<Arc<RenderImage>>,
}

/// Makes each row's icon a GPU image once, as the page arrives, not every frame.
fn page(entries: Vec<Entry>) -> Vec<Row> {
    entries
        .into_iter()
        .map(|mut entry| {
            let icon = match &mut entry {
                Entry::Action { icon, .. } | Entry::Submenu { icon, .. } => icon.take(),
                Entry::Separator => None,
            };
            Row {
                entry,
                image: icon.and_then(image),
            }
        })
        .collect()
}

fn image(icon: Icon) -> Option<Arc<RenderImage>> {
    thread_local! {
        // Keyed by content, so every menu that shows 7-Zip shares one texture.
        static IMAGES: RefCell<HashMap<(u32, u32, u32), Arc<RenderImage>>> = RefCell::default();
    }
    let key = (icon.width, icon.height, crc32fast::hash(&icon.bgra));
    IMAGES.with_borrow_mut(|images| {
        if let Some(image) = images.get(&key) {
            return Some(image.clone());
        }
        if images.len() >= ICON_LIMIT {
            return None;
        }
        let pixels = image::RgbaImage::from_raw(icon.width, icon.height, icon.bgra)?;
        let image = Arc::new(RenderImage::new(vec![image::Frame::new(pixels)]));
        images.insert(key, image.clone());
        Some(image)
    })
}

impl WindowsMenu {
    pub(crate) fn depth(&self) -> usize {
        self.pages.len().saturating_sub(1)
    }

    /// The menu stays hidden while pending, so it opens once and complete.
    pub(crate) fn pending(&self) -> bool {
        self.pending
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
        let limit = request.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SHELL_MENU_LIMIT).await;
            this.update(cx, |this, cx| {
                let Some(menu) = &mut this.context_menu else {
                    return;
                };
                if menu.windows.pending && Rc::ptr_eq(&menu.windows.request, &limit) {
                    // Open without Windows commands and ignore the late result.
                    menu.windows = WindowsMenu::default();
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let target = cx
                .background_executor()
                .spawn(async move {
                    if items.is_empty() {
                        return Ok(Target::Folder(
                            folder.context("this folder has no Windows path")?,
                        ));
                    }
                    let paths = items
                        .iter()
                        .map(|item| {
                            api.entry_path(item)?
                                .context("this item has no Windows path")
                        })
                        .collect::<anyhow::Result<Vec<_>>>()?;
                    anyhow::Ok(Target::Items(paths))
                })
                .await;
            let result = match target {
                Ok(target) => ShellMenu::open(owner, target, shift).await,
                Err(error) => Err(error),
            };
            this.update(cx, |this, cx| {
                let Some(menu) = &mut this.context_menu else {
                    return;
                };
                if !Rc::ptr_eq(&menu.windows.request, &request) {
                    return;
                }
                menu.windows.pending = false;
                match result {
                    Ok((shell, entries)) => {
                        menu.windows.pages.push(page(entries));
                        menu.windows.shell = Some(Rc::new(shell));
                    }
                    Err(error) => log::warn!("Windows commands unavailable: {error:#}"),
                }
                cx.notify();
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
        for (index, Row { entry, image }) in page.iter().enumerate() {
            let item = match entry {
                Entry::Action {
                    id,
                    label,
                    shortcut,
                    disabled,
                    ..
                } => {
                    let id = *id;
                    let mut item = MenuItem::action(("windows-command", index), label.clone())
                        .disabled(*disabled)
                        .on_select(cx.listener(move |this, _, window, cx| {
                            this.run_windows_menu_command(window, id, cx)
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
                    ..
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
            };
            items.push(match image {
                Some(image) => item.image(image.clone()),
                None => item,
            });
        }
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
            let entries = shell.expand(branch).await;
            this.update(cx, |this, cx| {
                let Some(menu) = &mut this.context_menu else {
                    return;
                };
                let same_page = menu.windows.depth() == depth
                    && menu
                        .windows
                        .shell
                        .as_ref()
                        .is_some_and(|open| Rc::ptr_eq(open, &shell));
                match entries {
                    Ok(entries) if same_page => {
                        menu.windows.pages.push(page(entries));
                        cx.notify();
                    }
                    Ok(_) => {}
                    Err(error) => this.report(Err(format!("Windows Shell menu failed: {error:#}")), cx),
                }
            })
            .ok();
        })
        .detach();
    }

    fn run_windows_menu_command(&mut self, window: &Window, command: u32, cx: &mut Context<Self>) {
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
            if let Err(error) = shell.invoke(command, anchor).await {
                this.update(cx, |this, cx| {
                    this.report(Err(format!("Windows Shell command failed: {error:#}")), cx)
                })
                .ok();
            }
        })
        .detach();
    }
}
