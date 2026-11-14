//! Disks from UDisks2 on the system bus: filesystems to mount, unmount and
//! eject, and encrypted volumes to unlock and lock.
//!
//! Visibility follows GIO's udisks2 volume monitor, as other file managers
//! do: no ignored blocks, nothing mounted at a system location, and only
//! blocks on a drive or on a loop device this user set up.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context as _, Result};
use zbus::blocking::{fdo::ObjectManagerProxy, Connection, MessageIterator, Proxy};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use super::{Kind, Source, State, Update, Volume};

const SERVICE: &str = "org.freedesktop.UDisks2";
const ROOT: &str = "/org/freedesktop/UDisks2";
const BLOCK: &str = "org.freedesktop.UDisks2.Block";
const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
const DRIVE: &str = "org.freedesktop.UDisks2.Drive";
const ENCRYPTED: &str = "org.freedesktop.UDisks2.Encrypted";
const LOOP: &str = "org.freedesktop.UDisks2.Loop";

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

/// A block's own path, or None for UDisks' "no object" path.
fn object(properties: &Properties, name: &str) -> Option<OwnedObjectPath> {
    property::<ObjectPath>(properties, name).filter(|path| path.as_str() != "/").map(|path| path.to_owned().into())
}

/// The user-visible filesystems and locked volumes, sorted by name.
pub fn list(connection: &Connection) -> Result<Vec<Volume>> {
    let manager = ObjectManagerProxy::builder(connection).destination(SERVICE)?.path(ROOT)?.build()?;
    let objects = manager.get_managed_objects().context("UDisks did not list its devices")?;
    let interfaces = |path: &OwnedObjectPath, name: &str| -> Option<Properties> {
        let object = objects.get(path)?;
        object.iter().find(|(interface, _)| interface.as_str() == name).map(|(_, properties)| {
            properties.iter().filter_map(|(key, value)| Some((key.to_string(), value.try_clone().ok()?))).collect()
        })
    };
    // SAFETY: getuid cannot fail.
    let uid = unsafe { libc::getuid() };
    let mut volumes = Vec::new();
    for path in objects.keys() {
        let Some(block) = interfaces(path, BLOCK) else { continue };
        if property::<bool>(&block, "HintIgnore").unwrap_or(false) {
            continue;
        }
        // An unlocked volume is a device-mapper block; its drive is the encrypted block's.
        let crypto = object(&block, "CryptoBackingDevice");
        let backing = crypto.as_ref().and_then(|crypto| interfaces(crypto, BLOCK));
        let physical = backing.as_ref().unwrap_or(&block);
        let drive = object(physical, "Drive").and_then(|drive| interfaces(&drive, DRIVE));
        let own_loop = crypto.as_ref().unwrap_or(path);
        let user_loop = interfaces(own_loop, LOOP).is_some_and(|properties| property::<u32>(&properties, "SetupByUID") == Some(uid));
        if drive.is_none() && !user_loop {
            continue;
        }
        let drive = drive.unwrap_or_default();
        let removable = property::<bool>(&drive, "Removable").unwrap_or(false)
            || property::<bool>(&drive, "MediaRemovable").unwrap_or(false);
        let ejectable = removable || property::<bool>(&drive, "Ejectable").unwrap_or(false);
        let label = property::<&str>(&block, "IdLabel").filter(|label| !label.is_empty());
        let hint = property::<&str>(&block, "HintName").filter(|name| !name.is_empty());
        let size = size_label(property::<u64>(&block, "Size").unwrap_or(0));
        let read_only = property::<bool>(&block, "ReadOnly").unwrap_or(false);

        let (state, fallback) = if let Some(filesystem) = interfaces(path, FILESYSTEM) {
            let points = mount_points(&filesystem);
            if !points.is_empty() && !points.iter().all(|point| user_mount(point)) {
                continue;
            }
            let state = points.into_iter().next().map_or(State::Unmounted, |point| super::mounted(point, read_only));
            (state, if removable { "Removable volume".to_string() } else { format!("{size} Volume") })
        } else if interfaces(path, ENCRYPTED).is_some_and(|encrypted| object(&encrypted, "CleartextDevice").is_none()) {
            (State::Locked, format!("{size} Encrypted"))
        } else {
            continue;
        };
        volumes.push(Volume {
            id: path.to_string(),
            name: label.or(hint).map(str::to_string).unwrap_or(fallback),
            kind: Kind::Disk,
            state,
            source: Source::Block { object: path.to_string(), crypto: crypto.map(|crypto| crypto.to_string()) },
            ejectable,
        });
    }
    volumes.sort_by(|left, right| crate::sort::natural_cmp(&left.name, &right.name));
    Ok(volumes)
}

/// Sends the device list now and after every UDisks change, until the receiver is dropped.
pub fn watch(sender: async_channel::Sender<Update>) {
    std::thread::Builder::new()
        .name("devices".into())
        .spawn(move || {
            let run = || -> Result<()> {
                let connection = Connection::system().context("cannot reach the system bus")?;
                if sender.send_blocking(Update::Disks(list(&connection).map_err(|error| format!("{error:#}")))).is_err() {
                    return Ok(());
                }
                // Devices come and go as objects; mounts change as properties.
                let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).sender(SERVICE)?.build();
                let mut signals = MessageIterator::for_match_rule(rule, &connection, Some(64))?;
                while signals.next().is_some() {
                    // A plug-in sends a burst of signals; list once it settles.
                    std::thread::sleep(Duration::from_millis(250));
                    let volumes = list(&connection).map_err(|error| format!("{error:#}"));
                    if sender.send_blocking(Update::Disks(volumes)).is_err() {
                        return Ok(());
                    }
                }
                Ok(())
            };
            if let Err(error) = run() {
                let _ = sender.send_blocking(Update::Disks(Err(format!("{error:#}"))));
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
    mount_on(&Connection::system()?, id)
}

fn mount_on(connection: &Connection, id: &str) -> Result<PathBuf> {
    let path: String = filesystem(connection, id)?.call("Mount", &(no_options(),)).context("could not mount")?;
    Ok(PathBuf::from(path))
}

/// Unlocks an encrypted volume and mounts the filesystem inside it.
pub fn unlock(id: &str, passphrase: &str) -> Result<PathBuf> {
    let connection = Connection::system()?;
    let encrypted = Proxy::new(&connection, SERVICE, id.to_string(), ENCRYPTED)?;
    let cleartext: OwnedObjectPath = encrypted.call("Unlock", &(passphrase, no_options())).map_err(|error| {
        let error = super::reason(error);
        // cryptsetup's own words differ by version; both mean the same to the user.
        let text = error.to_string();
        if text.contains("Incorrect passphrase") || text.contains("Operation not permitted") {
            anyhow::anyhow!("the passphrase is wrong")
        } else {
            error
        }
    })?;
    // The filesystem inside appears on the bus a moment after the unlock returns.
    for _ in 0..40 {
        let ready = Proxy::new(&connection, SERVICE, cleartext.as_str().to_string(), BLOCK)?.get_property::<String>("IdUsage").is_ok_and(|usage| usage == "filesystem");
        if ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    mount_on(&connection, cleartext.as_str())
}

/// Unmounts, and locks the encrypted volume underneath when there is one.
pub fn unmount(id: &str, crypto: Option<&str>) -> Result<()> {
    let connection = Connection::system()?;
    unmount_on(&connection, id, crypto)
}

fn unmount_on(connection: &Connection, id: &str, crypto: Option<&str>) -> Result<()> {
    let filesystem = filesystem(connection, id)?;
    let mounted = filesystem.get_property::<Vec<Vec<u8>>>("MountPoints").is_ok_and(|points| !points.is_empty());
    if mounted {
        filesystem.call::<_, _, ()>("Unmount", &(no_options(),)).context("could not unmount")?;
    }
    if let Some(crypto) = crypto {
        Proxy::new(connection, SERVICE, crypto.to_string(), ENCRYPTED)?.call::<_, _, ()>("Lock", &(no_options(),)).context("could not lock")?;
    }
    Ok(())
}

/// Unmounts every filesystem on the volume's drive, then ejects or powers it off.
pub fn eject(id: &str, crypto: Option<&str>) -> Result<()> {
    let connection = Connection::system()?;
    if crypto.is_some() {
        unmount_on(&connection, id, crypto)?;
    }
    let block = Proxy::new(&connection, SERVICE, crypto.unwrap_or(id).to_string(), BLOCK)?;
    let drive: OwnedObjectPath = block.get_property("Drive")?;
    if drive.as_str() == "/" {
        bail!("this volume has no drive to eject");
    }
    let manager = ObjectManagerProxy::builder(&connection).destination(SERVICE)?.path(ROOT)?.build()?;
    for (path, interfaces) in manager.get_managed_objects()? {
        let on_drive = interfaces.iter().any(|(name, properties)| {
            name.as_str() == BLOCK
                && properties.get("Drive").and_then(|value| value.downcast_ref::<ObjectPath>().ok()).is_some_and(|value| value.as_str() == drive.as_str())
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
