//! Servers on the local network, found through Avahi (mDNS/DNS-SD) over D-Bus,
//! as GVfs's network backend finds them. Each is offered by the address GVfs
//! mounts, such as `smb://nas.local/`.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

const AVAHI: &str = "org.freedesktop.Avahi";
/// Service types and the URI scheme GVfs mounts them with.
const SERVICES: [(&str, &str, &str); 5] = [
    ("_smb._tcp", "smb", "Windows share"),
    ("_sftp-ssh._tcp", "sftp", "SSH"),
    ("_ftp._tcp", "ftp", "FTP"),
    ("_webdav._tcp", "dav", "WebDAV"),
    ("_webdavs._tcp", "davs", "WebDAV"),
];
/// How long to wait for answers; Avahi says "all for now" sooner on a quiet network.
const WAIT: Duration = Duration::from_millis(1500);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Server {
    /// The name the server announces, such as "NAS".
    pub name: String,
    /// What it serves: "Windows share", "SSH", …
    pub kind: &'static str,
    pub uri: String,
}

/// Browses for servers until every service type has answered or `WAIT` passes.
pub fn browse() -> Result<Vec<Server>> {
    let connection = Connection::system().context("cannot reach the system bus")?;
    // Fail plainly when Avahi is not running, rather than wait and show nothing.
    Proxy::new(&connection, AVAHI, "/", "org.freedesktop.Avahi.Server")?
        .call::<_, _, String>("GetVersionString", &())
        .context("Network discovery needs the Avahi service, which is not running")?;
    let (found, results) = mpsc::channel();
    for (service, scheme, kind) in SERVICES {
        let (connection, found) = (connection.clone(), found.clone());
        std::thread::spawn(move || {
            if let Err(error) = browse_type(&connection, service, scheme, kind, &found) {
                log::debug!("network browse of {service} failed: {error:#}");
            }
            let _ = found.send(None);
        });
    }
    drop(found);
    let deadline = Instant::now() + WAIT;
    let (mut servers, mut finished) = (Vec::<Server>::new(), 0);
    while finished < SERVICES.len() {
        match results.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Some(server)) if !servers.contains(&server) => servers.push(server),
            Ok(Some(_)) => {}
            Ok(None) => finished += 1,
            Err(_) => break,
        }
    }
    servers.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()).then(left.kind.cmp(right.kind)));
    Ok(servers)
}

fn browse_type(connection: &Connection, service: &str, scheme: &str, kind: &'static str, found: &mpsc::Sender<Option<Server>>) -> Result<()> {
    let server = Proxy::new(connection, AVAHI, "/", "org.freedesktop.Avahi.Server2")?;
    let resolver = Proxy::new(connection, AVAHI, "/", "org.freedesktop.Avahi.Server")?;
    // Prepare, subscribe, then Start, so no answer is missed before the subscription.
    let path: OwnedObjectPath = server.call("ServiceBrowserPrepare", &(-1i32, -1i32, service, "", 0u32))?;
    let browser = Proxy::new(connection, AVAHI, path.as_str(), "org.freedesktop.Avahi.ServiceBrowser")?;
    let signals = browser.receive_all_signals()?;
    browser.call_method("Start", &())?;
    for message in signals {
        let header = message.header();
        match header.member().map(|member| member.as_str()) {
            Some("ItemNew") => {
                let (interface, protocol, name, kind_name, domain, _flags): (i32, i32, String, String, String, u32) = message.body().deserialize()?;
                let resolved: zbus::Result<(i32, i32, String, String, String, String, i32, String, u16, Vec<Vec<u8>>, u32)> =
                    resolver.call("ResolveService", &(interface, protocol, name.as_str(), kind_name.as_str(), domain.as_str(), -1i32, 0u32));
                if let Ok((_, _, _, _, _, host, _, _, _, _, _)) = resolved {
                    let _ = found.send(Some(Server { name, kind, uri: format!("{scheme}://{host}/") }));
                }
            }
            Some("AllForNow") | Some("Failed") => break,
            _ => {}
        }
    }
    let _ = browser.call_method("Free", &());
    Ok(())
}
