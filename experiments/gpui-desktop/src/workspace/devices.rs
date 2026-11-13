//! The sidebar's Devices: the watcher's list, and mount, unmount and eject.
//! UDisks calls block, so each runs on the background executor.

use gpui::Context;

use super::Workspace;
use crate::devices::{self, Volume};

#[derive(Default)]
pub(crate) struct Devices {
    pub volumes: Vec<Volume>,
    /// A volume being mounted, unmounted or ejected.
    pub busy: bool,
}

impl Workspace {
    pub(crate) fn start_device_watch(&mut self, cx: &mut Context<Self>) {
        let (sender, receiver) = async_channel::unbounded();
        devices::watch(sender);
        cx.spawn(async move |this, cx| {
            while let Ok(update) = receiver.recv().await {
                let alive = this
                    .update(cx, |this, cx| {
                        match update {
                            Ok(volumes) => this.devices.volumes = volumes,
                            Err(error) => log::warn!("Devices are unavailable: {error}"),
                        }
                        cx.notify();
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        })
        .detach();
    }

    /// Opens a volume, mounting it first if needed.
    pub(crate) fn open_volume(&mut self, volume: Volume, cx: &mut Context<Self>) {
        if let Some(point) = volume.mount_point.clone() {
            self.open_mounted(&volume.name, &point, cx);
            return;
        }
        if self.devices.busy {
            return;
        }
        self.devices.busy = true;
        let id = volume.id.clone();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { devices::mount(&id) }).await;
            this.update(cx, |this, cx| {
                this.devices.busy = false;
                match result {
                    Ok(point) => this.open_mounted(&volume.name, &point, cx),
                    Err(error) => this.report(Err(format!("Could not mount {}: {error:#}", volume.name)), cx),
                }
            })
            .ok();
        })
        .detach();
    }

    fn open_mounted(&mut self, name: &str, point: &std::path::Path, cx: &mut Context<Self>) {
        match self.pane().api.virtual_path(point) {
            Some(path) => self.navigate(path, cx),
            None => self.report(Err(format!("{name} is mounted at {}, outside the folders this window can open.", point.display())), cx),
        }
    }

    /// Ejects a removable volume's drive, or unmounts a fixed one.
    pub(crate) fn eject_volume(&mut self, volume: Volume, cx: &mut Context<Self>) {
        if self.devices.busy {
            return;
        }
        self.devices.busy = true;
        let (id, ejectable) = (volume.id.clone(), volume.ejectable);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { if ejectable { devices::eject(&id) } else { devices::unmount(&id) } })
                .await;
            this.update(cx, |this, cx| {
                this.devices.busy = false;
                let verb = if ejectable { "eject" } else { "unmount" };
                this.report(
                    result
                        .map(|()| match ejectable {
                            true => format!("{} can be removed", volume.name),
                            false => format!("Unmounted {}", volume.name),
                        })
                        .map_err(|error| format!("Could not {verb} {}: {error:#}", volume.name)),
                    cx,
                );
            })
            .ok();
        })
        .detach();
    }
}
