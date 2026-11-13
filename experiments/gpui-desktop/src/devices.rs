//! Devices from UDisks2 over the system D-Bus: drives' filesystems to list,
//! mount, unmount and eject. Everything here blocks, so it runs on a watcher
//! thread or the background executor, never on the UI thread.
//!
//! Visibility follows GIO's udisks2 volume monitor, which the web app shows
//! through the Go server: no ignored or driveless blocks, filesystems only,
//! and nothing mounted at a system location.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context as _, Result};
use zbus::blocking::{fdo::ObjectManagerProxy, Connection, MessageIterator, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const SERVICE: &str = "org.freedesktop.UDisks2";
const ROOT: &str = "/org/freedesktop/UDisks2";
const BLOCK: &str = "org.freedesktop.UDisks2.Block";
const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
const DRIVE: &str = "org.freedesktop.UDisks2.Drive";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    /// The UDisks block object; stable while the device is present.
    pub id: String,
    pub name: String,
    pub mount_point: Option<PathBuf>,
    /// Removable media or a drive that can be ejected; otherwise it can only be unmounted.
    pub ejectable: bool,
}

type Properties = HashMap<String, OwnedValue>;

fn property<'a, T>(properties: &'a Properties, name: &str) -> Option<T>
where
    T: TryFrom<&'a Value<'a>>,
    <T as TryFrom<&'a Value<'a>>>::Error: Into<zbus::zvariant::Error>,
{
    properties.get(name).and_then(|value| value.downcast_ref::<T>().ok())
}

/// `Filesystem.MountPoints` is an array of NUL-terminated byte strings.
fn mount_points(properties: &Properties) -> Vec<PathBuf> {
    let Some(points) = properties
        .get("MountPoints")
        .and_then(|value| value.try_clone().ok())
        .and_then(|value| <Vec<Vec<u8>>>::try_from(value).ok())
    else {
        return Vec::new();
    };
    points
        .into_iter()
        .map(|mut bytes| {
            if bytes.last() == Some(&0) {
                bytes.pop();
            }
            PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
        })
        .collect()
}

/// Mounted under a user location, as GIO requires before it shows a mount.
fn user_mount(path: &Path) -> bool {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    ["/media", "/run/media", "/mnt"].iter().any(|root| path.starts_with(root))
        || home.is_some_and(|home| path.starts_with(&home) && path != home)
}

/// `udisks_client_get_size_for_display`: decimal units, whole numbers from 10 up.
fn size_label(bytes: u64) -> String {
    let units = ["bytes", "kB", "MB", "GB", "TB", "PB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1000. && unit + 1 < units.len() {
        size /= 1000.;
        unit += 1;
    }
    if size < 10. && unit > 0 { format!("{size:.1} {}", units[unit]) } else { format!("{size:.0} {}", units[unit]) }
}

/// The user-visible filesystems, sorted by name.
pub fn list(connection: &Connection) -> Result<Vec<Volume>> {
    let manager = ObjectManagerProxy::builder(connection).destination(SERVICE)?.path(ROOT)?.build()?;
    let objects = manager.get_managed_objects().context("UDisks did not list its devices")?;
    let interfaces = |path: &OwnedObjectPath, name: &str| -> Option<Properties> {
        let object = objects.get(path)?;
        object.iter().find(|(interface, _)| interface.as_str() == name).map(|(_, properties)| {
            properties.iter().filter_map(|(key, value)| Some((key.to_string(), value.try_clone().ok()?))).collect()
        })
    };
    let mut volumes = Vec::new();
    for path in objects.keys() {
        let (Some(block), Some(filesystem)) = (interfaces(path, BLOCK), interfaces(path, FILESYSTEM)) else { continue };
        if property::<bool>(&block, "HintIgnore").unwrap_or(false) {
            continue;
        }
        // Loop devices and other blocks without a drive are not devices.
        let Some(drive_path) = property::<zbus::zvariant::ObjectPath>(&block, "Drive").filter(|drive| drive.as_str() != "/") else {
            continue;
        };
        let points = mount_points(&filesystem);
        if !points.is_empty() && !points.iter().all(|point| user_mount(point)) {
            continue;
        }
        let drive = interfaces(&OwnedObjectPath::from(drive_path.to_owned()), DRIVE).unwrap_or_default();
        let removable = property::<bool>(&drive, "Removable").unwrap_or(false)
            || property::<bool>(&drive, "MediaRemovable").unwrap_or(false);
        let ejectable = removable || property::<bool>(&drive, "Ejectable").unwrap_or(false);
        let label = property::<&str>(&block, "IdLabel").filter(|label| !label.is_empty());
        let hint = property::<&str>(&block, "HintName").filter(|name| !name.is_empty());
        let size = property::<u64>(&block, "Size").unwrap_or(0);
        let name = label.or(hint).map(str::to_string).unwrap_or_else(|| {
            if removable { "Removable volume".to_string() } else { format!("{} Volume", size_label(size)) }
        });
        volumes.push(Volume { id: path.to_string(), name, mount_point: points.into_iter().next(), ejectable });
    }
    volumes.sort_by(|left, right| crate::sort::natural_cmp(&left.name, &right.name));
    Ok(volumes)
}

/// Sends the device list now and after every UDisks change, until the receiver is dropped.
pub fn watch(sender: async_channel::Sender<Result<Vec<Volume>, String>>) {
    std::thread::Builder::new()
        .name("devices".into())
        .spawn(move || {
            let run = || -> Result<()> {
                let connection = Connection::system().context("cannot reach the system bus")?;
                if sender.send_blocking(list(&connection).map_err(|error| format!("{error:#}"))).is_err() {
                    return Ok(());
                }
                // Devices come and go as objects; mounts change as properties.
                let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).sender(SERVICE)?.build();
                let mut signals = MessageIterator::for_match_rule(rule, &connection, Some(64))?;
                while signals.next().is_some() {
                    // A plug-in sends a burst of signals; list once it settles.
                    std::thread::sleep(Duration::from_millis(250));
                    let volumes = list(&connection).map_err(|error| format!("{error:#}"));
                    if sender.send_blocking(volumes).is_err() {
                        return Ok(());
                    }
                }
                Ok(())
            };
            if let Err(error) = run() {
                let _ = sender.send_blocking(Err(format!("{error:#}")));
            }
        })
        .ok();
}

fn filesystem(connection: &Connection, id: &str) -> Result<Proxy<'static>> {
    Ok(Proxy::new(connection, SERVICE, id.to_string(), FILESYSTEM)?)
}

fn no_options() -> HashMap<&'static str, Value<'static>> {
    HashMap::new()
}

/// Mounts the volume (UDisks asks polkit for permission) and returns where.
pub fn mount(id: &str) -> Result<PathBuf> {
    let connection = Connection::system()?;
    let path: String = filesystem(&connection, id)?.call("Mount", &(no_options(),)).context("could not mount")?;
    Ok(PathBuf::from(path))
}

pub fn unmount(id: &str) -> Result<()> {
    let connection = Connection::system()?;
    filesystem(&connection, id)?.call::<_, _, ()>("Unmount", &(no_options(),)).context("could not unmount")?;
    Ok(())
}

/// Unmounts every filesystem on the volume's drive, then ejects or powers it off.
pub fn eject(id: &str) -> Result<()> {
    let connection = Connection::system()?;
    let block = Proxy::new(&connection, SERVICE, id.to_string(), BLOCK)?;
    let drive: OwnedObjectPath = block.get_property("Drive")?;
    if drive.as_str() == "/" {
        bail!("this volume has no drive to eject");
    }
    let manager = ObjectManagerProxy::builder(&connection).destination(SERVICE)?.path(ROOT)?.build()?;
    for (path, interfaces) in manager.get_managed_objects()? {
        let on_drive = interfaces.iter().any(|(name, properties)| {
            name.as_str() == BLOCK
                && properties.get("Drive").and_then(|value| value.downcast_ref::<zbus::zvariant::ObjectPath>().ok()).is_some_and(|value| value.as_str() == drive.as_str())
        });
        let mounted = interfaces.iter().any(|(name, properties)| {
            name.as_str() == FILESYSTEM && !mount_points(&properties.iter().filter_map(|(key, value)| Some((key.to_string(), value.try_clone().ok()?))).collect()).is_empty()
        });
        if on_drive && mounted {
            filesystem(&connection, path.as_str())?.call::<_, _, ()>("Unmount", &(no_options(),)).context("could not unmount before ejecting")?;
        }
    }
    let drive = Proxy::new(&connection, SERVICE, drive.as_str().to_string(), DRIVE)?;
    let can_power_off: bool = drive.get_property("CanPowerOff").unwrap_or(false);
    let ejectable: bool = drive.get_property("Ejectable").unwrap_or(false);
    if ejectable {
        drive.call::<_, _, ()>("Eject", &(no_options(),)).context("could not eject")?;
    } else if can_power_off {
        drive.call::<_, _, ()>("PowerOff", &(no_options(),)).context("could not power off")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_like_gio() {
        assert_eq!(size_label(482_168_000_000), "482 GB");
        assert_eq!(size_label(7_800_000_000), "7.8 GB");
        assert_eq!(size_label(512), "512 bytes");
    }

    #[test]
    fn only_user_locations_count_as_user_mounts() {
        assert!(user_mount(Path::new("/run/media/me/USB")));
        assert!(user_mount(Path::new("/mnt/data")));
        assert!(!user_mount(Path::new("/")));
        assert!(!user_mount(Path::new("/boot")));
    }
}
