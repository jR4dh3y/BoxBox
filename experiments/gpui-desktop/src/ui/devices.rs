//! The sidebar's Devices rows, and the dialog a mount opens to ask for a
//! passphrase or a login.

use gpui::{div, prelude::*, px, Context, SharedString, Window};

use super::TextScale;
use crate::components::{self, Button, Input, Modal, SidebarSection};
use crate::devices::{Kind, State, Volume};
use crate::format::file_size;
use crate::theme::*;
use crate::workspace::devices::CredentialField;
use crate::workspace::Workspace;

/// What the row says on hover: where the volume is and how full.
fn summary(volume: &Volume) -> String {
    match &volume.state {
        State::Mounted { point, space, read_only } => {
            let mut text = point.display().to_string();
            if let Some(space) = space {
                text += &format!("\n{} free of {}", file_size(space.free), file_size(space.total));
            }
            if *read_only {
                text += "\nRead-only";
            }
            text
        }
        State::Unmounted => "Not mounted. Click to mount and open.".into(),
        State::Locked => "Encrypted. Click to unlock.".into(),
    }
}

impl Workspace {
    /// A plain label, then each volume: a usage bar when mounted, dimmed when not,
    /// and a button to eject or unmount.
    pub(crate) fn render_devices(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.devices.busy;
        SidebarSection::new("devices", "Devices").children(self.devices.volumes().enumerate().map(|(index, volume)| {
            let (open, eject) = (volume.clone(), volume.clone());
            let glyph = match (&volume.state, volume.kind) {
                (State::Locked, _) => "lock",
                (_, Kind::Disk) => "hard-drive",
                (_, Kind::Phone) => "smartphone",
                (_, Kind::Network) => "globe",
            };
            let mut row = components::NavItem::new(SharedString::from(format!("volume-{index}")), volume.name.clone(), glyph)
                .dim_icon(false)
                .disabled(busy)
                .tooltip(summary(volume))
                .on_click(cx.listener(move |this, _, window, cx| this.open_volume(open.clone(), window, cx)));
            match &volume.state {
                State::Mounted { space, read_only, .. } => {
                    if let Some(space) = space {
                        row = row.usage((space.total - space.free) as f64 * 100. / space.total as f64);
                    }
                    if *read_only {
                        row = row.badge("pencil-off");
                    }
                }
                State::Unmounted | State::Locked => row = row.muted(true),
            }
            if volume.removable() {
                let label = format!("{} {}", if volume.ejectable { "Eject" } else { "Unmount" }, volume.name);
                row = row.trailing("arrow-up-from-line", label, cx.listener(move |this, _, _, cx| this.eject_volume(eject.clone(), cx)));
            }
            div().debug_selector(move || format!("volume-{index}")).child(row)
        }))
    }

    fn credential_input(&self, label: &'static str, field: CredentialField, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let entity = self.credential_field(field).clone();
        let focused = entity.read(cx).is_focused(window);
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().text_xs_tw().text_color(c(TEXT_SECONDARY)).child(label))
            .child(Input::new("").focused(focused).editor(entity))
    }

    pub(crate) fn render_credentials(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(credentials) = &self.devices.prompt else {
            return div().into_any_element();
        };
        let body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().text_13().text_color(c(TEXT_SECONDARY)).child(credentials.message.clone()))
            .when(credentials.user, |body| body.child(self.credential_input("User name", CredentialField::User, window, cx)))
            .when(credentials.domain, |body| body.child(self.credential_input("Domain", CredentialField::Domain, window, cx)))
            .child(self.credential_input(if credentials.unlocks() { "Passphrase" } else { "Password" }, CredentialField::Password, window, cx));
        div()
            .id("credentials-overlay")
            .absolute()
            .inset_0()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .title(credentials.title.clone())
                    .close_selector("credentials-cancel")
                    .on_close(cx.listener(|this, _, window, cx| this.cancel_credentials(window, cx)))
                    .body(body)
                    .footer(
                        Button::new("credentials-submit")
                            .label(if credentials.unlocks() { "Unlock" } else { "Sign in" })
                            .on_click(cx.listener(|this, _, window, cx| this.submit_credentials(window, cx))),
                    ),
            )
            .into_any_element()
    }
}
