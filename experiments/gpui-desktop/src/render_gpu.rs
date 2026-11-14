//! Picks the GPU the app draws with on Wayland: the one the compositor uses.
//!
//! GPUI takes the first Vulkan device that can present, which on a laptop with
//! two GPUs is often the discrete one while the compositor runs on the
//! integrated one. Every frame then crosses GPUs, and on NVIDIA that shows as
//! stale frames. The compositor names its GPU in the `linux-dmabuf` feedback;
//! GPUI is pointed at it through `ZED_DEVICE_ID`, which it reads at start.
//!
//! The Vulkan loader also starts every installed driver, and starting NVIDIA's
//! took 58 ms of a 134 ms launch on a laptop drawing with its AMD GPU, and can
//! wake a sleeping discrete GPU. Only the drawing GPU's driver is loaded.

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

/// The sysfs directory of a DRM device's hardware.
fn sysfs_device(device: u64) -> std::path::PathBuf {
    let (major, minor) = (libc::major(device), libc::minor(device));
    format!("/sys/dev/char/{major}:{minor}/device").into()
}

/// The PCI device ID of a DRM device, as sysfs gives it: `0x1638`.
fn pci_device_id(sysfs: &std::path::Path) -> Result<u32> {
    let path = sysfs.join("device");
    let text = std::fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    parse_id(&text).with_context(|| format!("{} holds no device ID", path.display()))
}

/// Variables through which a user already chose Vulkan drivers.
const DRIVER_CHOICES: [&str; 4] = ["VK_DRIVER_FILES", "VK_ICD_FILENAMES", "VK_LOADER_DRIVERS_SELECT", "VK_LOADER_DRIVERS_DISABLE"];
/// Where the loader finds driver manifests on Linux, below each data and config directory.
const MANIFEST_DIRECTORY: &str = "vulkan/icd.d";

/// Manifest name fragments of the Vulkan drivers for a kernel GPU driver.
fn vulkan_drivers(kernel_driver: &str) -> &'static [&'static str] {
    match kernel_driver {
        // RADV ships radeon_icd.json; AMDVLK, amd_icd64.json.
        "amdgpu" | "radeon" => &["radeon", "amd"],
        "i915" | "xe" => &["intel"],
        "nvidia" => &["nvidia"],
        "nouveau" => &["nouveau"],
        _ => &[],
    }
}

/// The installed driver manifests' file names, from the loader's search path.
fn installed_manifests() -> Vec<String> {
    let data = std::env::var("XDG_DATA_DIRS").ok().filter(|dirs| !dirs.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let config = std::env::var("XDG_CONFIG_DIRS").ok().filter(|dirs| !dirs.is_empty()).unwrap_or_else(|| "/etc/xdg".into());
    let home = std::env::var_os("HOME").map(|home| std::path::Path::new(&home).join(".local/share"));
    data.split(':')
        .chain(config.split(':'))
        .map(std::path::PathBuf::from)
        .chain(["/etc".into()])
        .chain(home)
        .filter_map(|directory| std::fs::read_dir(directory.join(MANIFEST_DIRECTORY)).ok())
        .flatten()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect()
}

/// The `VK_LOADER_DRIVERS_SELECT` globs for the drawing GPU, when one of its
/// drivers is installed. Without a match nothing is filtered, or GPUI would have
/// no driver at all.
fn driver_select(kernel_driver: &str, manifests: &[String]) -> Option<String> {
    let fragments = vulkan_drivers(kernel_driver);
    let installed = manifests.iter().any(|name| fragments.iter().any(|fragment| name.contains(fragment)));
    installed.then(|| fragments.iter().map(|fragment| format!("*{fragment}*")).collect::<Vec<_>>().join(","))
}

fn load_only_drivers_for(sysfs: &std::path::Path) {
    if DRIVER_CHOICES.iter().any(|name| std::env::var_os(name).is_some()) {
        return;
    }
    let Some(kernel_driver) = std::fs::read_link(sysfs.join("driver")).ok().and_then(|link| Some(link.file_name()?.to_str()?.to_string())) else {
        return;
    };
    if let Some(select) = driver_select(&kernel_driver, &installed_manifests()) {
        log::info!("Loading only the {kernel_driver} Vulkan drivers ({select})");
        std::env::set_var("VK_LOADER_DRIVERS_SELECT", select);
    }
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
    let found = compositor_device().and_then(|device| device.map(|device| {
        let sysfs = sysfs_device(device);
        pci_device_id(&sysfs).map(|id| (sysfs, id))
    }).transpose());
    match found {
        Ok(Some((sysfs, id))) => {
            log::info!("Drawing with the compositor's GPU, PCI device {id:04x}");
            std::env::set_var(OVERRIDE, format!("{id:04x}"));
            load_only_drivers_for(&sysfs);
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

    #[test]
    fn only_the_drawing_gpus_drivers_load_and_only_when_installed() {
        let manifests = ["nvidia_icd.json".to_string(), "radeon_icd.json".to_string()];
        assert_eq!(driver_select("amdgpu", &manifests).as_deref(), Some("*radeon*,*amd*"));
        assert_eq!(driver_select("nvidia", &manifests).as_deref(), Some("*nvidia*"));
        // Filtering to a driver that is not installed would leave none.
        assert_eq!(driver_select("i915", &manifests), None);
        assert_eq!(driver_select("virtio_gpu", &manifests), None);
    }
}
