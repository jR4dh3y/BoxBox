//! Single instance (like `desktop/src/instance.ts`): a later launch hands its path to the
//! running window over a private Unix socket and exits; the window opens it in a new tab.
//! The socket is this client's own, so it never talks to the Electrobun app.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use futures::channel::mpsc;
use serde::{Deserialize, Serialize};

const MAX_MESSAGE: u64 = 8 * 1024;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Launch {
    pub version: u32,
    pub path: String,
}

pub enum Instance {
    /// This process owns the socket; launches arrive on the channel.
    Primary(mpsc::UnboundedReceiver<String>),
    /// Another window took the launch.
    Forwarded,
}

fn socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let uid = std::fs::metadata("/proc/self")
        .map(|meta| std::os::unix::fs::MetadataExt::uid(&meta))
        .unwrap_or(0);
    runtime
        .join(format!("boxbox-gpui-{uid}"))
        .join("instance.sock")
}

pub fn claim(path: &str) -> Result<Instance> {
    let socket = socket_path();
    if let Ok(mut stream) = UnixStream::connect(&socket) {
        let message = serde_json::to_string(&Launch {
            version: 1,
            path: path.to_string(),
        })?;
        writeln!(stream, "{message}")?;
        let mut reply = String::new();
        BufReader::new(stream).read_line(&mut reply)?;
        if reply.trim() == "ok" {
            return Ok(Instance::Forwarded);
        }
    }
    let dir = socket.parent().context("socket directory")?;
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    // Nothing answered, so any socket file left behind belongs to a crashed instance.
    std::fs::remove_file(&socket).ok();
    let listener = UnixListener::bind(&socket)
        .with_context(|| format!("cannot listen on {}", socket.display()))?;
    let (send, receive) = mpsc::unbounded();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut line = String::new();
            let mut reader = BufReader::new(std::io::Read::take(&stream, MAX_MESSAGE));
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            let Ok(launch) = serde_json::from_str::<Launch>(line.trim()) else {
                continue;
            };
            if launch.version != 1 || send.unbounded_send(launch.path).is_err() {
                continue;
            }
            writeln!(&stream, "ok").ok();
        }
    });
    Ok(Instance::Primary(receive))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_claim_forwards_to_the_first() {
        let runtime =
            std::env::temp_dir().join(format!("boxbox-instance-test-{}", std::process::id()));
        std::fs::create_dir_all(&runtime).unwrap();
        // SAFETY: tests in this binary do not read XDG_RUNTIME_DIR concurrently.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", &runtime) };
        let Instance::Primary(mut launches) = claim("").unwrap() else {
            panic!("first claim must own the socket")
        };
        assert!(matches!(
            claim("home/Documents").unwrap(),
            Instance::Forwarded
        ));
        assert_eq!(
            futures::executor::block_on(futures::StreamExt::next(&mut launches)).as_deref(),
            Some("home/Documents")
        );
        std::fs::remove_dir_all(runtime).ok();
    }
}
