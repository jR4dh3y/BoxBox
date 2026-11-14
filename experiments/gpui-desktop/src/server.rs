//! The private loopback server, started like `desktop/src/server.ts`: a fresh config with the
//! desktop mounts, a random 64-hex token, and readiness announced as one JSON line on stdout.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{bail, Context as _, Result};
use serde::Deserialize;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const HOME_FOLDERS: [&str; 6] = [
    "Desktop",
    "Downloads",
    "Documents",
    "Music",
    "Pictures",
    "Videos",
];

pub struct LocalServer {
    pub origin: String,
    pub token: String,
    child: Child,
    /// The server exits on stdin EOF, which also covers this process crashing.
    _stdin: ChildStdin,
    work_dir: PathBuf,
}

impl LocalServer {
    pub fn start(binary: &Path, data_dir: &Path) -> Result<Self> {
        let work_dir = std::env::temp_dir().join(format!("boxbox-gpui-{}", std::process::id()));
        std::fs::create_dir_all(&work_dir)?;
        create_private_dir(data_dir)?;
        let token = random_token()?;
        let config = work_dir.join("config.json");
        write_private(&config, &desktop_config(data_dir, &home_dir())?)?;

        let mut command = Command::new(binary);
        command
            .args(["--desktop", "--config"])
            .arg(&config)
            .current_dir(&work_dir)
            .env("BOXBOX_DESKTOP_TOKEN", &token)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        // Deployment overrides must not change desktop mounts, origins or credentials.
        for (key, _) in std::env::vars_os() {
            let key = key.to_string_lossy();
            if key.starts_with("BOXBOX_") && key != "BOXBOX_DESKTOP_TOKEN"
                || key.starts_with("FM_")
                || key == "CONFIG_PATH"
            {
                command.env_remove(key.as_ref());
            }
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("cannot start {}", binary.display()))?;
        let stdin = child.stdin.take().context("server stdin")?;
        let stdout = child.stdout.take().context("server stdout")?;

        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let read = BufReader::new(stdout.take(4096)).read_line(&mut line);
            send.send(read.map(|_| line)).ok();
        });
        let origin = match receive.recv_timeout(STARTUP_TIMEOUT) {
            Ok(Ok(line)) if !line.is_empty() => parse_readiness(&line),
            Ok(Ok(_)) => Err(anyhow::anyhow!("the server exited before becoming ready")),
            Ok(Err(error)) => Err(error.into()),
            Err(_) => Err(anyhow::anyhow!("server startup timed out")),
        };
        match origin {
            Ok(origin) => Ok(Self {
                origin,
                token,
                child,
                _stdin: stdin,
                work_dir,
            }),
            Err(error) => {
                child.kill().ok();
                child.wait().ok();
                std::fs::remove_dir_all(&work_dir).ok();
                Err(error)
            }
        }
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        // SIGTERM lets the server finish jobs; it also exits once stdin closes.
        unsafe_terminate(self.child.id());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        self.child.kill().ok();
        self.child.wait().ok();
        std::fs::remove_dir_all(&self.work_dir).ok();
    }
}

fn unsafe_terminate(pid: u32) {
    Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .ok();
}

/// The bundled server sits next to this executable; `BOXBOX_SERVER_BIN` overrides it for development.
pub fn find_binary() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("BOXBOX_SERVER_BIN") {
        return Ok(PathBuf::from(path));
    }
    let exe = std::env::current_exe()?;
    let candidates = exe
        .parent()
        .into_iter()
        .flat_map(|dir| [dir.join("boxbox"), dir.join("../lib/boxbox/boxbox")]);
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .context("cannot find the bundled boxbox server; set BOXBOX_SERVER_BIN or BOXBOX_URL")
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home_dir().join(".local/share"))
        .join("boxbox-gpui")
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn desktop_config(data_dir: &Path, home: &Path) -> Result<String> {
    let mut mounts = vec![
        serde_json::json!({ "name": "root", "path": "/", "read_only": false }),
        serde_json::json!({ "name": "home", "path": home, "read_only": false }),
    ];
    for folder in HOME_FOLDERS {
        let path = home.join(folder);
        if path.is_dir() {
            mounts.push(serde_json::json!({
                "name": folder.to_lowercase(), "path": path, "read_only": false,
            }));
        }
    }
    Ok(serde_json::to_string(&serde_json::json!({
        "port": 8080,
        "host": "127.0.0.1",
        "data_dir": data_dir,
        "allow_root_mount": true,
        "mount_points": mounts,
    }))?)
}

/// Accepts only `http://127.0.0.1:<port>/`, like `serverURL` in `desktop/src/target.ts`.
fn parse_readiness(line: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Ready {
        url: String,
    }
    let ready: Ready = serde_json::from_str(line.trim()).context("invalid readiness message")?;
    let rest = ready.url.strip_prefix("http://127.0.0.1:").unwrap_or("");
    let port = rest.strip_suffix('/').unwrap_or(rest);
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("desktop server must listen on loopback, got {}", ready.url);
    }
    Ok(format!("http://127.0.0.1:{port}"))
}

fn random_token() -> Result<String> {
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn create_private_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_private(path: &Path, contents: &str) -> Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(contents.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_must_be_loopback() {
        assert_eq!(
            parse_readiness("{\"url\":\"http://127.0.0.1:4100\"}\n").unwrap(),
            "http://127.0.0.1:4100"
        );
        for bad in [
            "{\"url\":\"http://0.0.0.0:1\"}",
            "{\"url\":\"http://127.0.0.1:\"}",
            "{\"url\":\"https://127.0.0.1:2\"}",
            "nope",
        ] {
            assert!(parse_readiness(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn config_mounts_root_home_and_existing_folders() {
        let home = std::env::temp_dir().join(format!("boxbox-home-{}", std::process::id()));
        std::fs::create_dir_all(home.join("Music")).unwrap();
        let config: serde_json::Value =
            serde_json::from_str(&desktop_config(Path::new("/data"), &home).unwrap()).unwrap();
        let names: Vec<_> = config["mount_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(names, ["root", "home", "music"]);
        assert_eq!(config["allow_root_mount"], true);
        std::fs::remove_dir_all(home).ok();
    }
}
