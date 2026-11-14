//! The sidebar's Devices: the watchers' lists; mounting, unlocking and removing a
//! volume; opening a typed network location; and the questions a mount asks.
//! The D-Bus calls block, so each runs on the background executor.

use std::path::{Path, PathBuf};

use gpui::{Context, Window};

use super::confirm::Confirm;
use super::operations::PaneTarget;
use super::Workspace;
use crate::components::InputEvent;
use crate::devices::{self, Answer, Prompt, Prompts, State, Update, Volume};

#[derive(Default)]
pub(crate) struct Devices {
    disks: Vec<Volume>,
    gvfs: Vec<Volume>,
    /// A volume being mounted, unlocked or removed.
    pub busy: bool,
    pub prompt: Option<Credentials>,
    /// The user closed the sign-in dialog, so the mount's failure is not news.
    cancelled: bool,
}

impl Devices {
    /// Disks first, then phones and network shares.
    pub fn volumes(&self) -> impl Iterator<Item = &Volume> {
        self.disks.iter().chain(&self.gvfs)
    }
}

/// The dialog that asks for a passphrase or a login.
pub(crate) struct Credentials {
    pub title: String,
    pub message: String,
    pub user: bool,
    pub domain: bool,
    then: Then,
}

impl Credentials {
    /// A volume's passphrase rather than a login.
    pub fn unlocks(&self) -> bool {
        matches!(self.then, Then::Unlock(_))
    }
}

enum Then {
    /// Unlock this volume with the passphrase.
    Unlock(Volume),
    /// Hand the answer to the backend that asked. Dropping it cancels the mount.
    Reply(async_channel::Sender<Answer>),
}

/// The dialog's fields, in `Workspace::device_fields` order.
#[derive(Clone, Copy)]
pub(crate) enum CredentialField {
    User,
    Domain,
    Password,
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
                            Update::Disks(Ok(volumes)) => this.devices.disks = volumes,
                            Update::Gvfs(Ok(volumes)) => this.devices.gvfs = volumes,
                            Update::Disks(Err(error)) => log::warn!("Disks are unavailable: {error}"),
                            Update::Gvfs(Err(error)) => log::warn!("Phones and network shares are unavailable: {error}"),
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

    pub(crate) fn credential_field(&self, field: CredentialField) -> &gpui::Entity<crate::components::TextField> {
        &self.device_fields[field as usize]
    }

    /// Opens a volume, unlocking or mounting it first if needed.
    pub(crate) fn open_volume(&mut self, volume: Volume, window: &mut Window, cx: &mut Context<Self>) {
        match &volume.state {
            State::Mounted { point, .. } => self.open_mounted(self.active_pane, &volume.name, &point.clone(), cx),
            State::Unmounted => self.mount_volume(volume, None, cx),
            State::Locked => {
                let credentials = Credentials {
                    title: format!("Unlock {}", volume.name),
                    message: "This volume is encrypted. Enter its passphrase to open it.".into(),
                    user: false,
                    domain: false,
                    then: Then::Unlock(volume),
                };
                self.show_credentials(credentials, Answer::default(), window, cx);
            }
        }
    }

    fn mount_volume(&mut self, volume: Volume, passphrase: Option<String>, cx: &mut Context<Self>) {
        let name = volume.name.clone();
        self.run_mount(self.active_pane, name, cx, move |prompts| devices::mount(&volume, passphrase.as_deref(), prompts));
    }

    /// Mounts a typed location such as `smb://nas/media` and opens it in `target`.
    pub(crate) fn connect_location(&mut self, target: PaneTarget, uri: String, cx: &mut Context<Self>) {
        self.run_mount(target, uri.clone(), cx, move |prompts| devices::gvfs::mount(&uri, prompts));
    }

    /// Runs a mount in the background, shows what it asks, and opens the folder it returns.
    fn run_mount(
        &mut self,
        target: PaneTarget,
        name: String,
        cx: &mut Context<Self>,
        mount: impl FnOnce(Prompts) -> anyhow::Result<PathBuf> + Send + 'static,
    ) {
        if self.devices.busy {
            return;
        }
        self.devices.busy = true;
        self.devices.cancelled = false;
        cx.notify();
        let (prompts, questions) = async_channel::unbounded();
        // Ends when the mount returns and drops its end of the channel.
        cx.spawn(async move |this, cx| {
            while let Ok(prompt) = questions.recv().await {
                if this.update(cx, |this, cx| this.show_prompt(prompt, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { mount(prompts) }).await;
            this.update(cx, |this, cx| {
                this.devices.busy = false;
                this.devices.prompt = None;
                match result {
                    Ok(point) => this.open_mounted(target, &name, &point, cx),
                    Err(_) if this.devices.cancelled => {}
                    Err(error) => this.report(Err(format!("Could not open {name}: {error:#}")), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_mounted(&mut self, target: PaneTarget, name: &str, point: &Path, cx: &mut Context<Self>) {
        let path = self.pane_by_id(target.0).and_then(|pane| pane.api.virtual_path(point));
        match path {
            Some(path) => {
                self.location_editor = None;
                self.navigate_in(target, path, cx);
            }
            None => self.report(Err(format!("{name} is mounted at {}, outside the folders this window can open.", point.display())), cx),
        }
    }

    /// A backend's question. The window cannot be focused from here, so the dialog's
    /// field takes focus on the next render.
    fn show_prompt(&mut self, prompt: Prompt, cx: &mut Context<Self>) {
        match prompt {
            Prompt::Password { message, user, domain, reply } => {
                let answer = Answer { user: user.clone().unwrap_or_default(), domain: domain.clone().unwrap_or_default(), password: String::new() };
                let credentials = Credentials {
                    title: "Sign in".into(),
                    message,
                    user: user.is_some(),
                    domain: domain.is_some(),
                    then: Then::Reply(reply),
                };
                self.fill_credentials(answer, cx);
                self.pending_credential_focus = true;
                self.devices.prompt = Some(credentials);
                cx.notify();
            }
            Prompt::Question { message, choices, reply } => {
                // GVfs puts the question's title on the first line.
                let (title, body) = message.split_once('\n').unwrap_or((&message, ""));
                let accept = choices.first().cloned().unwrap_or_else(|| "Continue".into());
                let mut confirm = Confirm::new(title.to_string(), body.trim().to_string(), accept, move |_, _| {
                    let _ = reply.try_send(0);
                });
                confirm.danger = false;
                self.ask(confirm, cx);
            }
        }
    }

    fn fill_credentials(&mut self, answer: Answer, cx: &mut Context<Self>) {
        for (field, text) in [(CredentialField::User, answer.user), (CredentialField::Domain, answer.domain), (CredentialField::Password, String::new())] {
            let end = text.len();
            self.credential_field(field).clone().update(cx, |field, cx| field.set_text(text, 0..end, cx));
        }
    }

    fn show_credentials(&mut self, credentials: Credentials, answer: Answer, window: &mut Window, cx: &mut Context<Self>) {
        self.fill_credentials(answer, cx);
        self.devices.prompt = Some(credentials);
        self.focus_credentials(window, cx);
        cx.notify();
    }

    /// Focuses the first field the dialog shows.
    pub(crate) fn focus_credentials(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(credentials) = &self.devices.prompt else { return };
        let first = if credentials.user && self.credential_field(CredentialField::User).read(cx).text().is_empty() {
            CredentialField::User
        } else {
            CredentialField::Password
        };
        self.credential_field(first).read(cx).focus(window);
    }

    /// Enter and Escape in the dialog's fields.
    pub(crate) fn on_credential_field(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Submit => self.submit_credentials(window, cx),
            InputEvent::Cancel => self.cancel_credentials(window, cx),
            InputEvent::Changed => {}
        }
    }

    pub(crate) fn submit_credentials(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(credentials) = self.devices.prompt.take() else { return };
        let read = |field| self.credential_field(field).read(cx).text().to_string();
        let answer = Answer { user: read(CredentialField::User), domain: read(CredentialField::Domain), password: read(CredentialField::Password) };
        // The secret does not stay in the field after it is used.
        self.credential_field(CredentialField::Password).clone().update(cx, |field, cx| field.set_text("", 0..0, cx));
        match credentials.then {
            Then::Unlock(volume) => self.mount_volume(volume, Some(answer.password), cx),
            Then::Reply(reply) => {
                let _ = reply.try_send(answer);
            }
        }
        self.focus_handle.focus(window);
        cx.notify();
    }

    /// Closing the dialog drops the reply, which cancels the mount that asked.
    pub(crate) fn cancel_credentials(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.devices.prompt.take().is_some() {
            self.devices.cancelled = true;
            self.credential_field(CredentialField::Password).clone().update(cx, |field, cx| field.set_text("", 0..0, cx));
            self.focus_handle.focus(window);
            cx.notify();
        }
    }

    /// Ejects a removable volume's drive, or unmounts a fixed one or a network share.
    pub(crate) fn eject_volume(&mut self, volume: Volume, cx: &mut Context<Self>) {
        if self.devices.busy {
            return;
        }
        self.devices.busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let removed = volume.clone();
            let result = cx.background_executor().spawn(async move { devices::remove(&removed) }).await;
            this.update(cx, |this, cx| {
                this.devices.busy = false;
                if result.is_ok() {
                    this.leave_volume(&volume, cx);
                }
                let verb = if volume.ejectable { "eject" } else { "unmount" };
                this.report(
                    result
                        .map(|()| match volume.ejectable {
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

    /// Sends every pane that was inside the removed volume back to This PC.
    fn leave_volume(&mut self, volume: &Volume, cx: &mut Context<Self>) {
        let Some(point) = volume.mount_point() else { return };
        let inside: Vec<PaneTarget> = self
            .tabs
            .iter()
            .chain(self.split_panes.values())
            .filter(|pane| {
                pane.api.virtual_path(point).is_some_and(|root| pane.path == root || pane.path.starts_with(&format!("{root}/")))
            })
            .map(|pane| PaneTarget(pane.id))
            .collect();
        for target in inside {
            self.navigate_in(target, String::new(), cx);
        }
    }
}
