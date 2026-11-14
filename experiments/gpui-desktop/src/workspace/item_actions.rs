//! Context-menu actions on selected items: Open in New Tab, Create Link,
//! Compress and Send To (a removable drive, the Desktop, or an email).

use std::collections::{HashMap, HashSet};
use std::os::fd::AsFd as _;
use std::sync::Arc;

use anyhow::Context as _;
use gpui::{Context, SharedString};

use super::operations::{ClipboardEntry, ClipboardMode, FileClipboard, PaneTarget};
use super::Workspace;
use crate::api::{Api, FileInfo};
use crate::components::MenuItem;

/// Where Send To can copy items: a label, an icon and a folder on the local provider.
pub(crate) struct Destination {
    pub label: String,
    pub icon: &'static str,
    pub folder: String,
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

impl Workspace {
    /// Each selected folder in a tab of its own, the last one active.
    pub(crate) fn open_folders_in_tabs(&mut self, target: PaneTarget, items: Vec<FileInfo>, cx: &mut Context<Self>) {
        for item in items.into_iter().filter(|item| item.is_dir) {
            self.open_in_new_tab(target, item.path, cx);
        }
    }

    /// A link to each item beside it, named "Link to …".
    pub(crate) fn create_links(&mut self, target: PaneTarget, items: Vec<FileInfo>, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_by_id(target.0) else { return };
        let api = pane.api.clone();
        let mut taken: HashSet<String> = pane.items.iter().map(|item| item.name.clone()).collect();
        let links: Vec<(String, String)> = items
            .iter()
            .map(|item| {
                let name = super::names::unused_name(&format!("Link to {}", item.name), item.is_dir, &|name| taken.contains(name));
                taken.insert(name.clone());
                let folder = parent(&item.path);
                (item.path.clone(), if folder.is_empty() { name } else { format!("{folder}/{name}") })
            })
            .collect();
        let count = links.len();
        let generation = self.begin_operation("Creating links…");
        let request_api = api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    for (item, link) in &links {
                        request_api.create_link(item, link)?;
                    }
                    Ok(links.into_iter().map(|(_, link)| link).collect::<Vec<_>>())
                })
                .await;
            this.update(cx, |this, cx| {
                let result = result.map(|links: Vec<String>| {
                    if let Some(pane) = this.pane_mut_by_id(target.0) {
                        pane.selected = links.into_iter().collect();
                    }
                    if count == 1 { "Created a link".to_string() } else { format!("Created {count} links") }
                });
                this.refresh_provider(api, cx);
                this.finish_operation(generation, result, cx);
            })
            .ok();
        })
        .detach();
    }

    /// A ZIP of the items beside them: "name.zip" for one, "Archive.zip" for several.
    /// It then opens for renaming.
    pub(crate) fn compress(&mut self, target: PaneTarget, items: Vec<FileInfo>, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_by_id(target.0) else { return };
        let Some(first) = items.first() else { return };
        // Search results may come from several folders; the archive goes in the first's.
        let folder = parent(&first.path).to_string();
        let stem = match items.as_slice() {
            [item] => item.name[..super::names::basename_end(&item.name, item.is_dir)].to_string(),
            _ => "Archive".to_string(),
        };
        let taken: HashSet<String> = pane.items.iter().filter(|item| parent(&item.path) == folder).map(|item| item.name.clone()).collect();
        let name = super::names::unused_name(&format!("{stem}.zip"), false, &|name| taken.contains(name));
        let destination = if folder.is_empty() { name.clone() } else { format!("{folder}/{name}") };
        let api = pane.api.clone();
        let generation = self.begin_operation(format!("Compressing into {name}…"));
        let (request_api, archive) = (api.clone(), destination.clone());
        let paths: Vec<String> = items.into_iter().map(|item| item.path).collect();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { request_api.compress(&paths, &archive) }).await;
            this.update(cx, |this, cx| {
                if result.is_ok() {
                    this.pending_rename = Some((target, destination));
                }
                this.refresh_provider(api, cx);
                this.finish_operation(generation, result.map(|()| format!("Created “{name}”")), cx);
            })
            .ok();
        })
        .detach();
    }

    /// The Desktop and every mounted removable volume, on the local provider.
    pub(crate) fn send_destinations(&self, api: &Api) -> Vec<Destination> {
        let mut destinations = Vec::new();
        if let Some(crate::pane::Load::Loaded(roots)) = self.tabs.iter().find(|pane| pane.api.is_local()).map(|pane| &pane.roots) {
            if roots.iter().any(|root| root.name == "desktop") {
                destinations.push(Destination { label: "Desktop".into(), icon: "monitor", folder: "desktop".into() });
            }
        }
        for volume in self.devices.volumes().filter(|volume| volume.removable()) {
            if let Some(folder) = volume.mount_point().and_then(|point| api.virtual_path(point)) {
                let icon = match volume.kind {
                    crate::devices::Kind::Phone => "smartphone",
                    crate::devices::Kind::Network => "globe",
                    crate::devices::Kind::Disk => "hard-drive",
                };
                destinations.push(Destination { label: volume.name.clone(), icon, folder });
            }
        }
        destinations
    }

    /// The "Send To" submenu for local items.
    pub(crate) fn send_to_menu(&self, target: PaneTarget, items: &[FileInfo], cx: &mut Context<Self>) -> Option<MenuItem> {
        let api = self.pane_by_id(target.0)?.api.clone();
        if !api.is_local() || items.is_empty() {
            return None;
        }
        let mut entries: Vec<MenuItem> = self
            .send_destinations(&api)
            .into_iter()
            .enumerate()
            .map(|(index, destination)| {
                let (items, folder, api) = (items.to_vec(), destination.folder.clone(), api.clone());
                MenuItem::action(("send-to", index), destination.label)
                    .icon(destination.icon)
                    .on_select(cx.listener(move |this, _, _, cx| {
                        this.close_menus();
                        let entries = items.iter().map(|item| ClipboardEntry { api: api.clone(), item: item.clone() }).collect();
                        this.start_transfer(target, FileClipboard { mode: ClipboardMode::Copy, entries }, api.clone(), folder.clone(), cx);
                    }))
            })
            .collect();
        let files = items.iter().all(|item| !item.is_dir);
        let mail_items = items.to_vec();
        entries.push(
            MenuItem::action("send-mail", "Mail Recipient…")
                .icon("send")
                .disabled(!files)
                .on_select(cx.listener(move |this, _, _, cx| {
                    this.close_menus();
                    this.send_by_mail(api.clone(), mail_items.clone(), cx);
                })),
        );
        let id: SharedString = "send-to".into();
        let open = self.open_submenu.as_ref() == Some(&id);
        Some(MenuItem::submenu(id, "Send To", entries, open).icon("send"))
    }

    /// Opens a new email with the files attached, through the desktop's Email
    /// portal, which hands them to the user's mail app.
    pub(crate) fn send_by_mail(&mut self, api: Arc<Api>, items: Vec<FileInfo>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { compose_email(&api, &items) }).await;
            if let Err(error) = result {
                this.update(cx, |this, cx| this.report(Err(format!("Could not start an email: {error:#}")), cx)).ok();
            }
        })
        .detach();
    }

    pub(crate) fn close_menus(&mut self) {
        self.context_menu = None;
        self.open_submenu = None;
    }
}

fn compose_email(api: &Api, items: &[FileInfo]) -> anyhow::Result<()> {
    let files = items
        .iter()
        .map(|item| {
            let path = api.local_path(&item.path)?.context("not a local file")?;
            std::fs::File::open(&path).with_context(|| format!("cannot open {}", path.display()))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let fds: Vec<zbus::zvariant::Fd> = files.iter().map(|file| zbus::zvariant::Fd::from(file.as_fd())).collect();
    let mut options: HashMap<&str, zbus::zvariant::Value> = HashMap::new();
    options.insert("attachment_fds", zbus::zvariant::Value::new(fds));
    let connection = zbus::blocking::Connection::session()?;
    let portal = zbus::blocking::Proxy::new(&connection, "org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop", "org.freedesktop.portal.Email")?;
    portal.call::<_, _, zbus::zvariant::OwnedObjectPath>("ComposeEmail", &("", options))?;
    Ok(())
}
