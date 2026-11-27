//! Single instance: a later launch hands its
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

/// An exclusive `flock` beside the socket, held from the connect attempt to the
/// bind, so two launches at once cannot both find no window and both start one.
/// Released when dropped; by then a listener is bound and later launches find it.
struct ClaimLock {
    _held: std::fs::File,
}

impl ClaimLock {
    fn take(socket: &std::path::Path) -> Result<Self> {
        use std::os::fd::AsRawFd as _;
        let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(socket.with_extension("lock"))?;
        // SAFETY: the descriptor is open for as long as `file` lives.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(std::io::Error::last_os_error()).context("cannot lock the instance socket");
        }
        Ok(Self { _held: file })
    }
}

fn claim_at(socket: &std::path::Path, argument: &str) -> Result<Instance> {
    let dir = socket.parent().context("socket directory")?;
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    let _lock = ClaimLock::take(socket)?;
    if let Ok(mut stream) = UnixStream::connect(socket) {
        writeln!(stream, "{}", serde_json::to_string(argument)?)?;
        let mut reply = String::new();
        BufReader::new(stream.take(16)).read_line(&mut reply)?;
        if reply.trim() == "ok" {
            return Ok(Instance::Forwarded);
        }
    }
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

    #[test]
    fn launches_at_once_start_one_window() {
        let dir = std::env::temp_dir().join(format!("boxbox-instance-race-{}", std::process::id()));
        let socket = dir.join("instance.sock");
        for round in 0..20 {
            std::fs::remove_dir_all(&dir).ok();
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
            let claims: Vec<_> = (0..8)
                .map(|_| {
                    let (socket, barrier) = (socket.clone(), barrier.clone());
                    std::thread::spawn(move || {
                        barrier.wait();
                        claim_at(&socket, "x").unwrap()
                    })
                })
                .collect();
            let claims: Vec<_> = claims.into_iter().map(|claim| claim.join().unwrap()).collect();
            let primaries = claims.iter().filter(|claim| matches!(claim, Instance::Primary(..))).count();
            assert_eq!(primaries, 1, "round {round}: one window, the rest handed over");
        }
        std::fs::remove_dir_all(dir).ok();
    }
}
