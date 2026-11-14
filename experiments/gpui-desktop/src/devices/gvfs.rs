//! Phones and network shares from GVfs on the session bus, the service GTK
//! and Qt file managers share. Its mount tracker lists and makes mounts, each
//! reachable as a folder under `$XDG_RUNTIME_DIR/gvfs`; its volume monitors
//! report devices that are plugged in but not mounted yet, such as a phone.

mod prompt;
pub mod spec;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Context as _, Result};
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Structure, Value};

pub use prompt::{Answer, Prompt, Prompts};

use super::{reason, Kind, Source, State, Update, Volume};

const DAEMON: &str = "org.gtk.vfs.Daemon";
const TRACKER_PATH: &str = "/org/gtk/vfs/mounttracker";
const TRACKER: &str = "org.gtk.vfs.MountTracker";
const MOUNT: &str = "org.gtk.vfs.Mount";
const MONITOR_PATH: &str = "/org/gtk/Private/RemoteVolumeMonitor";
const MONITOR: &str = "org.gtk.Private.RemoteVolumeMonitor";
const MONITOR_DIR: &str = "/usr/share/gvfs/remote-volume-monitors";
const OPERATION: &str = "/org/boxbox/MountOperation";

/// `(sossssssbay(aya{sv})ay)`: the daemon's bus name and object, display and stable
/// names, content types, icons, filename encoding, user-visible, the FUSE folder,
/// the mount spec and the default location.
type MountInfo = (
    String,
    OwnedObjectPath,
    String,
    String,
    String,
    String,
    String,
    String,
    bool,
    Vec<u8>,
    (Vec<u8>, HashMap<String, OwnedValue>),
    Vec<u8>,
);

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes.strip_suffix(&[0]).unwrap_or(bytes)).into_owned()
}

fn tracker(connection: &Connection) -> Result<Proxy<'static>> {
    Ok(Proxy::new(connection, DAEMON, TRACKER_PATH, TRACKER)?)
}

fn volume(mount: MountInfo) -> Option<Volume> {
    let (daemon, object, name, _, _, _, _, _, _, fuse, (_, spec), _) = mount;
    let point = PathBuf::from(text(&fuse));
    // A mount without a folder cannot be browsed by path.
    if point.as_os_str().is_empty() {
        return None;
    }
    let kind = spec.get("type").and_then(|value| <Vec<u8>>::try_from(value.try_clone().ok()?).ok()).map(|bytes| text(&bytes)).unwrap_or_default();
    Some(Volume {
        id: format!("gvfs:{daemon}{object}"),
        name,
        kind: if matches!(kind.as_str(), "mtp" | "gphoto2" | "afc") { Kind::Phone } else { Kind::Network },
        state: super::mounted(point, false),
        source: Source::GvfsMount { daemon, object: object.to_string() },
        ejectable: false,
    })
}

/// The bus names of the installed volume monitors, except the disk one that
/// `udisks` replaces.
fn monitors() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(MONITOR_DIR) else { return Vec::new() };
    entries
        .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path()).ok())
        .filter_map(|file| file.lines().find_map(|line| line.strip_prefix("DBusName=").map(|name| name.trim().to_string())))
        .filter(|name| !name.contains("UDisks2"))
        .collect()
}

/// The monitor's volumes that have no mount: `(ssssssbbssa{ss}sa{sv})` is id, name,
/// two icons, UUID, the location to mount, can-mount, automount, drive and mount ids.
fn unmounted(connection: &Connection, monitor: &str) -> Result<Vec<Volume>> {
    let reply = Proxy::new(connection, monitor.to_string(), MONITOR_PATH, MONITOR)?.call_method("List", &())?;
    let body = reply.body();
    let lists: Structure = body.deserialize()?;
    let Some(Value::Array(volumes)) = lists.fields().get(1) else { return Ok(Vec::new()) };
    let field = |fields: &[Value], index: usize| match fields.get(index) {
        Some(Value::Str(text)) => text.to_string(),
        _ => String::new(),
    };
    Ok(volumes
        .iter()
        .filter_map(|volume| {
            let Value::Structure(volume) = volume else { return None };
            let fields = volume.fields();
            let (id, name, uri, mount) = (field(fields, 0), field(fields, 1), field(fields, 5), field(fields, 9));
            let can_mount = matches!(fields.get(6), Some(Value::Bool(true)));
            (mount.is_empty() && can_mount && !uri.is_empty()).then(|| Volume {
                id: format!("gvfs:{monitor}/{id}"),
                name,
                kind: Kind::Phone,
                state: State::Unmounted,
                source: Source::GvfsVolume { uri },
                ejectable: false,
            })
        })
        .collect())
}

/// Mounted locations, then plugged-in devices that are not mounted, each sorted by name.
pub fn list(connection: &Connection) -> Result<Vec<Volume>> {
    let mounts: Vec<MountInfo> = tracker(connection)?.call("ListMounts2", &(true,)).context("GVfs did not list its mounts")?;
    let mut volumes: Vec<Volume> = mounts.into_iter().filter_map(volume).collect();
    for monitor in monitors() {
        match unmounted(connection, &monitor) {
            Ok(found) => volumes.extend(found),
            Err(error) => log::info!("{monitor} did not list its devices: {error:#}"),
        }
    }
    volumes.sort_by(|left, right| crate::sort::natural_cmp(&left.name, &right.name));
    Ok(volumes)
}

pub fn watch(sender: async_channel::Sender<Update>) {
    std::thread::Builder::new()
        .name("gvfs".into())
        .spawn(move || {
            let send = |connection: &Connection| sender.send_blocking(Update::Gvfs(list(connection).map_err(|error| format!("{error:#}")))).is_ok();
            let run = || -> Result<()> {
                let connection = Connection::session().context("cannot reach the session bus")?;
                if !send(&connection) {
                    return Ok(());
                }
                // The tracker and every volume monitor signal from under /org/gtk.
                let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).path_namespace("/org/gtk")?.build();
                let mut signals = MessageIterator::for_match_rule(rule, &connection, Some(64))?;
                while signals.next().is_some() {
                    // A phone reports its storage in a burst; list once it settles.
                    std::thread::sleep(Duration::from_millis(250));
                    if !send(&connection) {
                        return Ok(());
                    }
                }
                Ok(())
            };
            if let Err(error) = run() {
                let _ = sender.send_blocking(Update::Gvfs(Err(format!("{error:#}"))));
            }
        })
        .ok();
}

fn lookup(tracker: &Proxy, location: &spec::Location) -> Option<PathBuf> {
    let mount: MountInfo = tracker.call("LookupMount", &(location.wire(),)).ok()?;
    let root = PathBuf::from(text(&mount.9));
    (!root.as_os_str().is_empty()).then_some(root)
}

/// Mounts a location such as `smb://nas/media` unless it already is, and returns
/// its folder. The backend asks for credentials through `prompts`.
pub fn mount(uri: &str, prompts: Prompts) -> Result<PathBuf> {
    let location = spec::parse(uri)?;
    let connection = zbus::blocking::connection::Builder::session()?
        .serve_at(OPERATION, prompt::MountOperation { prompts })?
        .build()
        .context("cannot reach the session bus")?;
    let tracker = tracker(&connection)?;
    let folder = |root: PathBuf| if location.path.is_empty() { root } else { root.join(&location.path) };
    if let Some(root) = lookup(&tracker, &location) {
        return Ok(folder(root));
    }
    let name = connection.unique_name().context("the session bus gave this app no name")?.to_string();
    tracker
        .call::<_, _, ()>("MountLocation", &(location.wire(), (name, ObjectPath::try_from(OPERATION)?)))
        .map_err(reason)?;
    // The FUSE folder appears a moment after the mount is registered.
    for _ in 0..40 {
        if let Some(root) = lookup(&tracker, &location).filter(|root| root.exists()) {
            return Ok(folder(root));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(anyhow!("the location was mounted, but GVfs gave it no folder; is gvfsd-fuse running?"))
}

pub fn unmount(daemon: &str, object: &str) -> Result<()> {
    let connection = Connection::session()?;
    // No mount operation: an unmount has nothing to ask.
    Proxy::new(&connection, daemon.to_string(), object.to_string(), MOUNT)?
        .call::<_, _, ()>("Unmount", &("", ObjectPath::try_from("/")?, 0u32))
        .map_err(reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_strings_lose_their_terminator() {
        assert_eq!(text(b"/run/user/1000/gvfs/x\0"), "/run/user/1000/gvfs/x");
        assert_eq!(text(b"plain"), "plain");
    }
}
