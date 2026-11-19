//! What the item menu takes from the system: the apps that open a file, with
//! their icons, and custom actions (`custom_actions.rs`). They load off the UI
//! thread when the menu opens. The menu stays hidden until they arrive, for a
//! short time at most, so it opens once and complete rather than growing rows
//! under the pointer.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use gpui::{Context, RenderImage, SharedString, Window};

use super::Workspace;
use crate::api::{Api, FileInfo};
use crate::components::MenuItem;
use crate::custom_actions::{Action, Target};
use crate::open_with::App;

/// How long a right-click waits for apps and actions before the menu opens
/// without them. Warm, they take a few milliseconds; a stalled disk can take far longer.
const LIMIT: Duration = Duration::from_millis(150);

/// Menu icons drawn at 16 logical pixels.
const ICON_SIZE: f32 = 16.;

/// Distinct menu icons kept on the GPU. Menus reuse a few dozen.
const ICON_LIMIT: usize = 256;

#[derive(Clone, Default)]
pub(crate) struct MenuExtras {
    /// Tells this menu's results from those of a menu opened before it.
    request: Rc<()>,
    pending: bool,
    /// The recommended apps for a single file, the default first.
    apps: Vec<Row<App>>,
    actions: Vec<Row<Action>>,
    /// The items' (or the folder's) real paths, which apps and actions open.
    paths: Vec<PathBuf>,
}

#[derive(Clone)]
struct Row<T> {
    value: T,
    image: Option<Arc<RenderImage>>,
}

impl MenuExtras {
    /// The menu stays hidden while pending, so it opens once and complete.
    pub(crate) fn pending(&self) -> bool {
        self.pending
    }
}

struct Loaded {
    apps: Vec<Row<App>>,
    actions: Vec<Row<Action>>,
    paths: Vec<PathBuf>,
}

/// Reads apps and actions for the items, or for the folder when there are none.
fn load(api: &Api, items: &[FileInfo], folder: Option<PathBuf>, pixels: u32) -> anyhow::Result<Loaded> {
    let _span = crate::profile::span("menu extras");
    let targets: Vec<Target> = match items {
        [] => vec![Target::new(folder.context("the folder is not on this computer")?, true)],
        items => items
            .iter()
            .map(|item| Ok(Target::new(api.local_path(&item.path)?.context("not a local file")?, item.is_dir)))
            .collect::<anyhow::Result<_>>()?,
    };
    let apps = match targets.as_slice() {
        [file] if !file.is_dir => crate::open_with::recommended(&file.path).unwrap_or_else(|error| {
            log::debug!("no apps for {}: {error:#}", file.path.display());
            Vec::new()
        }),
        _ => Vec::new(),
    };
    let apps = apps.into_iter().map(|app| Row { image: app.icon.as_deref().and_then(|path| image(path, pixels)), value: app }).collect();
    let theme = crate::icon_theme::IconTheme::shared();
    let actions = crate::custom_actions::for_targets(&targets)
        .into_iter()
        .map(|action| Row { image: action.icon.as_deref().and_then(|icon| theme.find(icon)).and_then(|path| image(&path, pixels)), value: action })
        .collect();
    Ok(Loaded { apps, actions, paths: targets.into_iter().map(|target| target.path).collect() })
}

/// An icon file as a GPU image, made once per file and size and then shared.
fn image(path: &Path, pixels: u32) -> Option<Arc<RenderImage>> {
    type Images = HashMap<(PathBuf, u32), Arc<RenderImage>>;
    static IMAGES: std::sync::Mutex<Option<Images>> = std::sync::Mutex::new(None);
    let key = (path.to_path_buf(), pixels);
    if let Some(image) = IMAGES.lock().ok()?.get_or_insert_default().get(&key) {
        return Some(image.clone());
    }
    let image = crate::file_icons::rasterize(path, pixels).map_err(|error| log::debug!("{}: {error:#}", path.display())).ok()?;
    let mut images = IMAGES.lock().ok()?;
    let images = images.get_or_insert_default();
    if images.len() < ICON_LIMIT {
        images.insert(key, image.clone());
    }
    Some(image)
}

/// Long enough for the first folder to list before the warm-up competes with it.
const WARM_DELAY: Duration = Duration::from_secs(1);

impl Workspace {
    /// Reads the app list and the icon theme once, soon after launch: cold, they
    /// take over a second, far past `LIMIT`, and the first menu would lack its apps.
    pub(super) fn warm_menu_apps(&self, cx: &mut Context<Self>) {
        let executor = cx.background_executor().clone();
        executor
            .spawn({
                let executor = executor.clone();
                async move {
                    executor.timer(WARM_DELAY).await;
                    crate::open_with::warm();
                }
            })
            .detach();
    }

    /// Starts reading the open menu's apps and actions, for a local pane.
    pub(super) fn load_menu_extras(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(menu) = &self.context_menu else { return };
        let Some(pane) = self.pane_by_id(menu.target_id) else { return };
        if !pane.api.is_local() || (menu.items.is_empty() && pane.real_path.is_none()) {
            return;
        }
        let (api, folder, items) = (pane.api.clone(), pane.real_path.clone(), menu.items.clone());
        let pixels = crate::file_icons::bucket(ICON_SIZE, window.scale_factor());
        let request = menu.extras.request.clone();
        if let Some(menu) = &mut self.context_menu {
            menu.extras.pending = true;
        }
        let limit = request.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(LIMIT).await;
            this.update(cx, |this, cx| {
                let Some(menu) = &mut this.context_menu else { return };
                if menu.extras.pending && Rc::ptr_eq(&menu.extras.request, &limit) {
                    // Open without them; a new request makes the late result stale.
                    menu.extras = MenuExtras::default();
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let loaded = cx.background_executor().spawn(async move { load(&api, &items, folder, pixels) }).await;
            this.update(cx, |this, cx| {
                let Some(menu) = &mut this.context_menu else { return };
                if !Rc::ptr_eq(&menu.extras.request, &request) {
                    return;
                }
                menu.extras.pending = false;
                match loaded {
                    Ok(loaded) => {
                        menu.extras.apps = loaded.apps;
                        menu.extras.actions = loaded.actions;
                        menu.extras.paths = loaded.paths;
                    }
                    Err(error) => log::debug!("menu apps and actions unavailable: {error:#}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// "Open With <default>" and the other recommended apps in a submenu, as
    /// Thunar and Nautilus list them; the dialog stays for any other app.
    pub(crate) fn open_with_entries(&self, extras: &MenuExtras, cx: &mut Context<Self>) -> Vec<MenuItem> {
        let dialog = |label: &'static str, cx: &mut Context<Self>| {
            MenuItem::action("context-open-with", label).icon("app-window").on_select(cx.listener(|this, _, window, cx| {
                this.context_open_with(window, cx);
                this.close_menus();
                cx.notify();
            }))
        };
        let (default, others): (Vec<_>, Vec<_>) = extras.apps.iter().partition(|row| row.value.is_default);
        let mut entries: Vec<MenuItem> = default.into_iter().map(|row| self.app_entry(row, extras, format!("Open With {}", row.value.name), cx)).collect();
        if others.is_empty() {
            entries.push(dialog("Open With…", cx));
            return entries;
        }
        let mut apps: Vec<MenuItem> = others.into_iter().map(|row| self.app_entry(row, extras, row.value.name.clone(), cx)).collect();
        apps.push(MenuItem::Separator);
        apps.push(dialog("Other Application…", cx));
        let id: SharedString = "context-open-with-apps".into();
        let open = self.open_submenu.as_ref() == Some(&id);
        entries.push(MenuItem::submenu(id, "Open With", apps, open).icon("app-window"));
        entries
    }

    fn app_entry(&self, row: &Row<App>, extras: &MenuExtras, label: String, cx: &mut Context<Self>) -> MenuItem {
        let (id, name) = (row.value.id.clone(), row.value.name.clone());
        let path = extras.paths.first().cloned();
        let item = MenuItem::action(SharedString::from(format!("context-app-{id}")), label).on_select(cx.listener(move |this, _, _, cx| {
            this.close_menus();
            cx.notify();
            let Some(path) = path.clone() else { return };
            let (id, name) = (id.clone(), name.clone());
            cx.spawn(async move |this, cx| {
                let result = cx.background_executor().spawn(async move { crate::open_with::launch(&path, &id, false) }).await;
                if let Err(error) = result {
                    this.update(cx, |this, cx| this.report(Err(format!("Could not open it in {name}: {error:#}")), cx)).ok();
                }
            })
            .detach();
        }));
        match &row.image {
            Some(image) => item.image(image.clone()),
            None => item.icon("app-window"),
        }
    }

    /// Custom actions, those in a submenu grouped under it. `terminal` leaves
    /// out actions that only open a terminal, when the menu has its own row.
    pub(crate) fn custom_action_entries(&self, extras: &MenuExtras, terminal: bool, cx: &mut Context<Self>) -> Vec<MenuItem> {
        let mut entries = Vec::new();
        let mut submenus: Vec<(String, Vec<MenuItem>)> = Vec::new();
        for (index, row) in extras.actions.iter().enumerate() {
            if terminal && row.value.opens_terminal() {
                continue;
            }
            let item = self.action_entry(index, row, extras, cx);
            match &row.value.submenu {
                None => entries.push(item),
                Some(name) => match submenus.iter_mut().find(|(existing, _)| existing == name) {
                    Some((_, items)) => items.push(item),
                    None => submenus.push((name.clone(), vec![item])),
                },
            }
        }
        for (name, items) in submenus {
            let id = SharedString::from(format!("context-actions-{name}"));
            let open = self.open_submenu.as_ref() == Some(&id);
            entries.push(MenuItem::submenu(id, name, items, open).icon("list"));
        }
        entries
    }

    fn action_entry(&self, index: usize, row: &Row<Action>, extras: &MenuExtras, cx: &mut Context<Self>) -> MenuItem {
        let (action, paths) = (row.value.clone(), extras.paths.clone());
        let item = MenuItem::action(SharedString::from(format!("context-action-{index}")), row.value.label.clone()).on_select(cx.listener(move |this, _, _, cx| {
            this.close_menus();
            cx.notify();
            let (action, paths) = (action.clone(), paths.clone());
            cx.spawn(async move |this, cx| {
                let label = action.label.clone();
                let result = cx.background_executor().spawn(async move { action.run(&paths) }).await;
                if let Err(error) = result {
                    this.update(cx, |this, cx| this.report(Err(format!("Could not run “{label}”: {error:#}")), cx)).ok();
                }
            })
            .detach();
        }));
        match &row.image {
            Some(image) => item.image(image.clone()),
            None => item,
        }
    }
}
