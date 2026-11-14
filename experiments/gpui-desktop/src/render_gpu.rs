//! Picks the GPU the app draws with on Wayland: the one the compositor uses.
//!
//! GPUI takes the first Vulkan device that can present, which on a laptop with
//! two GPUs is often the discrete one while the compositor runs on the
//! integrated one. Every frame then crosses GPUs, and on NVIDIA that shows as
//! stale frames. The compositor names its GPU in the `linux-dmabuf` feedback;
//! GPUI is pointed at it through `ZED_DEVICE_ID`, which it reads at start.

use anyhow::{bail, Context as _, Result};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{delegate_noop, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_dmabuf_feedback_v1::{self, ZwpLinuxDmabufFeedbackV1};
use wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1;

/// GPUI's variable for the PCI device ID to draw with.
const OVERRIDE: &str = "ZED_DEVICE_ID";
/// Feedback, and with it the main device, came with version 4.
const FEEDBACK_VERSION: u32 = 4;

#[derive(Default)]
struct State {
    main_device: Option<u64>,
    done: bool,
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(_: &mut Self, _: &WlRegistry, _: <WlRegistry as Proxy>::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwpLinuxDmabufFeedbackV1, ()> for State {
    fn event(state: &mut Self, _: &ZwpLinuxDmabufFeedbackV1, event: zwp_linux_dmabuf_feedback_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            // A `dev_t` in the machine's byte order.
            zwp_linux_dmabuf_feedback_v1::Event::MainDevice { device } => {
                state.main_device = device.as_slice().try_into().ok().map(u64::from_ne_bytes);
            }
            zwp_linux_dmabuf_feedback_v1::Event::Done => state.done = true,
            _ => {}
        }
    }
}

delegate_noop!(State: ignore ZwpLinuxDmabufV1);

/// The device number of the compositor's GPU, or None when it does not say.
fn compositor_device() -> Result<Option<u64>> {
    let connection = Connection::connect_to_env().context("cannot reach the compositor")?;
    let (globals, mut queue) = registry_queue_init::<State>(&connection)?;
    let handle = queue.handle();
    let Ok(dmabuf) = globals.bind::<ZwpLinuxDmabufV1, _, _>(&handle, FEEDBACK_VERSION..=FEEDBACK_VERSION, ()) else {
        return Ok(None);
    };
    dmabuf.get_default_feedback(&handle, ());
    let mut state = State::default();
    // The compositor answers at once; a second round covers one that splits its reply.
    for _ in 0..2 {
        queue.roundtrip(&mut state)?;
        if state.done {
            break;
        }
    }
    Ok(state.main_device)
}

/// The PCI device ID of a DRM device, as sysfs gives it: `0x1638`.
fn pci_device_id(device: u64) -> Result<u32> {
    let (major, minor) = (libc::major(device), libc::minor(device));
    let path = format!("/sys/dev/char/{major}:{minor}/device/device");
    let text = std::fs::read_to_string(&path).with_context(|| format!("cannot read {path}"))?;
    parse_id(&text).with_context(|| format!("{path} holds no device ID"))
}

fn parse_id(text: &str) -> Result<u32> {
    let digits = text.trim().trim_start_matches("0x");
    if digits.is_empty() {
        bail!("empty");
    }
    Ok(u32::from_str_radix(digits, 16)?)
}

/// Call before GPUI starts and before any thread does. A GPU the user chose stays chosen.
pub fn follow_compositor() {
    if std::env::var_os(OVERRIDE).is_some() || std::env::var_os("WAYLAND_DISPLAY").is_none_or(|display| display.is_empty()) {
        return;
    }
    match compositor_device().and_then(|device| device.map(pci_device_id).transpose()) {
        Ok(Some(id)) => {
            log::info!("Drawing with the compositor's GPU, PCI device {id:04x}");
            std::env::set_var(OVERRIDE, format!("{id:04x}"));
        }
        Ok(None) => log::info!("The compositor does not name its GPU; GPUI picks one"),
        Err(error) => log::warn!("Could not find the compositor's GPU: {error:#}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sysfs_ids_are_hex() {
        assert_eq!(parse_id("0x1638\n").unwrap(), 0x1638);
        assert_eq!(parse_id("25a2").unwrap(), 0x25a2);
        assert!(parse_id("\n").is_err());
        assert!(parse_id("amd").is_err());
    }
}
