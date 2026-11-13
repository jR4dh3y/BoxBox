//! Single instance, like `desktop/src/instance.ts`: a later launch hands its
//! argument to the running window over a private Unix socket and exits, and
//! the window opens it in a new tab. Adapted from PR #226.

use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use anyhow::{Context as _, Result};

const MAX_MESSAGE: u64 = 8 * 1024;

pub enum Instance {
    /// This process owns the socket; later launches arrive on the channel.
    Primary(async_channel::Receiver<String>, async_channel::Sender<String>),
    /// The running window took the launch.
    Forwarded,
}

/// One socket per user and provider setting, so a window on a server or a test
/// root never takes launches meant for the local one.
fn socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let uid = std::fs::metadata("/proc/self").map(|meta| std::os::unix::fs::MetadataExt::uid(&meta)).unwrap_or(0);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (std::env::var_os("BOXBOX_URL"), std::env::var_os("BOXBOX_ROOT")).hash(&mut hasher);
    runtime.join(format!("boxbox-gpui-{uid}")).join(format!("instance-{:x}.sock", hasher.finish()))
}

pub fn claim(argument: &str) -> Result<Instance> {
    claim_at(&socket_path(), argument)
}

fn claim_at(socket: &std::path::Path, argument: &str) -> Result<Instance> {
    if let Ok(mut stream) = UnixStream::connect(socket) {
        writeln!(stream, "{}", serde_json::to_string(argument)?)?;
        let mut reply = String::new();
        BufReader::new(stream.take(16)).read_line(&mut reply)?;
        if reply.trim() == "ok" {
            return Ok(Instance::Forwarded);
        }
    }
    let dir = socket.parent().context("socket directory")?;
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    // Nothing answered, so a socket file left behind belongs to a crashed window.
    std::fs::remove_file(socket).ok();
    let listener = UnixListener::bind(socket).with_context(|| format!("cannot listen on {}", socket.display()))?;
    let (sender, receiver) = async_channel::unbounded();
    let launches = sender.clone();
    std::thread::Builder::new().name("instance".into()).spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut line = String::new();
            if BufReader::new((&stream).take(MAX_MESSAGE)).read_line(&mut line).is_err() {
                continue;
            }
            let Ok(argument) = serde_json::from_str::<String>(line.trim()) else { continue };
            if sender.send_blocking(argument).is_err() {
                break;
            }
            writeln!(&stream, "ok").ok();
        }
    })?;
    Ok(Instance::Primary(receiver, launches))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_claim_forwards_to_the_first() {
        let dir = std::env::temp_dir().join(format!("boxbox-instance-{}", std::process::id()));
        let socket = dir.join("instance.sock");
        let Instance::Primary(launches, _) = claim_at(&socket, "").unwrap() else { panic!("the first claim owns the socket") };
        assert!(matches!(claim_at(&socket, "/home/me/Documents").unwrap(), Instance::Forwarded));
        assert_eq!(launches.recv_blocking().unwrap(), "/home/me/Documents");
        std::fs::remove_dir_all(dir).ok();
    }
}
