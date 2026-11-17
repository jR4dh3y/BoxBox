//! Opting in to BoxBox as the Open and Save dialog. xdg-desktop-portal finds
//! backends by their `.portal` file, starts them through a D-Bus service file,
//! and picks one per interface from `portals.conf`. All three live under the
//! user's own folders, so no root is needed; only the FileChooser line changes,
//! and the line it replaced is kept to restore.

use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context as _, Result};

use super::portal::BUS_NAME;

const INTERFACE: &str = "org.freedesktop.impl.portal.FileChooser";
const MARKER: &str = "# BoxBox file chooser override";
const PREVIOUS: &str = "# Previous: ";
const MANAGED: &str = "# BoxBox managed";

fn absolute_env(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).map(PathBuf::from).filter(|path| path.is_absolute())
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn config_home() -> PathBuf {
    absolute_env("XDG_CONFIG_HOME").unwrap_or_else(|| home().join(".config"))
}

fn data_home() -> PathBuf {
    absolute_env("XDG_DATA_HOME").unwrap_or_else(|| home().join(".local/share"))
}

fn portal_file() -> PathBuf {
    data_home().join("xdg-desktop-portal/portals/boxbox.portal")
}

fn service_file() -> PathBuf {
    data_home().join(format!("dbus-1/services/{BUS_NAME}.service"))
}

/// `niri-portals.conf` and the like for each current desktop, then `portals.conf`.
fn config_names() -> Vec<String> {
    let desktops = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    desktops
        .split(':')
        .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .map(|name| format!("{}-portals.conf", name.to_lowercase()))
        .chain(["portals.conf".to_string()])
        .collect()
}

/// The directories xdg-desktop-portal reads its configuration from, in its order.
fn config_roots() -> Vec<PathBuf> {
    let dirs = |key: &str, fallback: &str| -> Vec<PathBuf> {
        std::env::var(key).ok().filter(|dirs| !dirs.is_empty()).unwrap_or_else(|| fallback.into()).split(':').map(PathBuf::from).filter(|path| path.is_absolute()).collect()
    };
    let mut roots = vec![config_home()];
    roots.extend(dirs("XDG_CONFIG_DIRS", "/etc/xdg"));
    roots.push("/etc".into());
    roots.push(data_home());
    roots.extend(dirs("XDG_DATA_DIRS", "/usr/local/share:/usr/share"));
    roots
}

/// The configuration file in effect and its text; none is an empty one.
fn effective_config() -> Result<Option<(PathBuf, String)>> {
    let names = config_names();
    for root in config_roots() {
        for name in &names {
            let path = root.join("xdg-desktop-portal").join(name);
            match std::fs::read_to_string(&path) {
                Ok(text) => return Ok(Some((path, text))),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).with_context(|| format!("cannot read {}", path.display())),
            }
        }
    }
    Ok(None)
}

/// Whether the configuration in effect sends the file chooser to BoxBox.
pub fn enabled() -> bool {
    effective_config().ok().flatten().is_some_and(|(_, text)| text.lines().any(|line| line == MARKER))
}

/// `source` with the FileChooser line routed to BoxBox (`enable`), or with the
/// line BoxBox replaced put back. BoxBox goes first, before what was there, so
/// that remains the fallback while BoxBox is not installed.
pub fn route(source: &str, enable: bool) -> Result<String> {
    let mut lines: Vec<String> = source.lines().map(str::to_string).collect();
    if let Some(at) = lines.iter().position(|line| line == MARKER) {
        let previous = lines.get(at + 1).and_then(|line| line.strip_prefix(PREVIOUS)).map(str::to_string);
        let ours = lines.get(at + 2).is_some_and(|line| line.starts_with(&format!("{INTERFACE}=boxbox")));
        let Some(previous) = previous.filter(|_| ours) else {
            bail!("The BoxBox lines in the portal configuration were edited; fix them by hand first");
        };
        lines.splice(at..at + 3, (!previous.is_empty()).then_some(previous));
    }
    if enable {
        let mut group = false;
        let mut insert_at = None;
        let mut previous: Option<String> = None;
        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                if group && insert_at.is_none() {
                    insert_at = Some(index);
                }
                group = trimmed == "[preferred]";
            }
            let key = trimmed.split_once('=').map(|(key, _)| key.trim());
            if group && key == Some(INTERFACE) {
                ensure!(previous.is_none(), "The portal configuration names the file chooser twice");
                previous = Some(line.clone());
                insert_at = Some(index);
            }
        }
        let fallback = previous.as_deref().and_then(|line| line.split_once('=')).map_or(String::new(), |(_, value)| value.trim().to_string());
        let routed = format!("{INTERFACE}=boxbox;{fallback}");
        let block = [MARKER.to_string(), format!("{PREVIOUS}{}", previous.clone().unwrap_or_default()), routed];
        let at = match insert_at {
            Some(at) => at,
            None if group => lines.len(),
            None => {
                if !lines.iter().any(|line| line.trim() == "[preferred]") {
                    lines.push("[preferred]".into());
                }
                lines.len()
            }
        };
        let removed = usize::from(previous.is_some());
        lines.splice(at..at + removed, block);
    }
    let mut text = lines.join("\n");
    text.push('\n');
    Ok(text)
}

/// Replaces a file's contents at once; through a link, the file it names.
fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let parent = path.parent().context("no parent folder")?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".{}.boxbox-{}", path.file_name().unwrap_or_default().to_string_lossy(), std::process::id()));
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temporary);
    })?;
    Ok(())
}

/// D-Bus service files split `Exec` like a shell: quote and escape the path.
fn quoted(path: &Path) -> String {
    let text = path.display().to_string();
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The executable to register: an AppImage runs from a mount that goes away,
/// so its own file is the one that stays.
fn program() -> Result<String> {
    if let Some(image) = absolute_env("APPIMAGE").filter(|path| path.is_file()) {
        return Ok(appimage_command(&image, absolute_env("APPDIR").as_deref()));
    }
    Ok(quoted(&std::env::current_exe().context("cannot find the BoxBox program")?))
}

/// The AppImage runtime unpacks to `appimage_extracted_*` when it runs without
/// FUSE (`--appimage-extract-and-run`); D-Bus has to start it the same way.
fn appimage_command(image: &Path, appdir: Option<&Path>) -> String {
    let extracted = appdir.and_then(Path::file_name).is_some_and(|name| name.to_string_lossy().starts_with("appimage_extracted_"));
    match extracted {
        true => format!("{} --appimage-extract-and-run", quoted(image)),
        false => quoted(image),
    }
}

/// The first xdg-desktop-portal that reads backends from the user's data folder.
const USER_BACKENDS_SINCE: (u32, u32, u32) = (1, 20, 1);

/// `1.18.4` from `xdg-desktop-portal 1.18.4`.
fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let word = text.split_whitespace().find(|word| word.starts_with(|c: char| c.is_ascii_digit()))?;
    let mut parts = word.split('.').map(|part| part.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<u32>().ok());
    Some((parts.next()??, parts.next().flatten().unwrap_or(0), parts.next().flatten().unwrap_or(0)))
}

/// The portal frontend's program, from its systemd unit or where distributions put it.
fn frontend_program() -> Option<PathBuf> {
    type ExecStart = Vec<(String, Vec<String>, bool, u64, u64, u64, u64, u32, i32, i32)>;
    let from_unit = || -> Result<PathBuf> {
        let connection = zbus::blocking::Connection::session()?;
        let manager = zbus::blocking::Proxy::new(&connection, "org.freedesktop.systemd1", "/org/freedesktop/systemd1", "org.freedesktop.systemd1.Manager")?;
        let unit: zbus::zvariant::OwnedObjectPath = manager.call("LoadUnit", &("xdg-desktop-portal.service",))?;
        let service = zbus::blocking::Proxy::new(&connection, "org.freedesktop.systemd1", unit, "org.freedesktop.systemd1.Service")?;
        let exec: ExecStart = service.get_property("ExecStart")?;
        exec.into_iter().next().map(|(path, ..)| PathBuf::from(path)).context("no ExecStart")
    };
    from_unit()
        .ok()
        .into_iter()
        .chain(["/usr/libexec/xdg-desktop-portal", "/usr/lib/xdg-desktop-portal", "/usr/lib/xdg-desktop-portal/xdg-desktop-portal"].map(PathBuf::from))
        .find(|path| path.is_file())
}

/// Fails unless the portal frontend will find a backend in the user's data
/// folder; routing to one it cannot find leaves apps without a dialog.
fn ensure_discoverable() -> Result<()> {
    // The frontend does not report its version on the bus; the program does.
    let program = frontend_program().context("xdg-desktop-portal is not installed")?;
    let output = std::process::Command::new(&program).arg("--version").output().with_context(|| format!("cannot run {}", program.display()))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let version = parse_version(&text).with_context(|| format!("cannot tell which xdg-desktop-portal {} is", program.display()))?;
    let (major, minor, patch) = USER_BACKENDS_SINCE;
    ensure!(
        version >= USER_BACKENDS_SINCE,
        "xdg-desktop-portal {}.{}.{} only finds file choosers installed for every user; BoxBox needs {major}.{minor}.{patch} or newer",
        version.0,
        version.1,
        version.2
    );
    ensure!(std::env::var_os("XDG_DESKTOP_PORTAL_DIR").is_none(), "XDG_DESKTOP_PORTAL_DIR is set, so xdg-desktop-portal reads no other backends");
    Ok(())
}

/// Routes Open and Save dialogs to this program's `--portal`. Returns the file changed.
pub fn enable() -> Result<PathBuf> {
    ensure_discoverable()?;
    let program = program()?;
    let service = service_file();
    if let Ok(existing) = std::fs::read_to_string(&service) {
        ensure!(existing.starts_with(MANAGED), "{} was not written by BoxBox; remove it first", service.display());
    }
    write_atomic(&portal_file(), &format!("[portal]\nDBusName={BUS_NAME}\nInterfaces={INTERFACE};\n"))?;
    write_atomic(&service, &format!("{MANAGED}\n[D-BUS Service]\nName={BUS_NAME}\nExec={program} --portal\n"))?;
    let destination = set_route(true)?;
    restart_portal();
    Ok(destination)
}

/// Puts back the dialog that was in use before, and removes BoxBox's backend files.
pub fn disable() -> Result<PathBuf> {
    let destination = set_route(false)?;
    for file in [portal_file(), service_file()] {
        if std::fs::read_to_string(&file).is_ok_and(|text| text.starts_with(MANAGED) || text.contains(BUS_NAME)) {
            let _ = std::fs::remove_file(&file);
        }
    }
    restart_portal();
    Ok(destination)
}

fn set_route(enable: bool) -> Result<PathBuf> {
    let effective = effective_config()?;
    let ours = config_home();
    let (destination, source) = match effective {
        // Edited where it is when it is the user's own; otherwise the user's copy overrides it.
        Some((path, text)) if path.starts_with(&ours) => (path, text),
        Some((_, text)) => (ours.join("xdg-desktop-portal").join(&config_names()[0]), text),
        None => (ours.join("xdg-desktop-portal").join(&config_names()[0]), String::new()),
    };
    if !enable && !source.lines().any(|line| line == MARKER) {
        return Ok(destination);
    }
    write_atomic(&destination, &route(&source, enable)?)?;
    Ok(destination)
}

/// The frontend reads its configuration and backends only when it starts.
fn restart_portal() {
    let restart = || -> Result<()> {
        let connection = zbus::blocking::Connection::session()?;
        connection.call_method(
            Some("org.freedesktop.systemd1"),
            "/org/freedesktop/systemd1",
            Some("org.freedesktop.systemd1.Manager"),
            "TryRestartUnit",
            &("xdg-desktop-portal.service", "replace"),
        )?;
        Ok(())
    };
    if let Err(error) = restart() {
        log::warn!("Could not restart xdg-desktop-portal; log out and in to apply: {error:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::{appimage_command, parse_version, route};
    use std::path::Path;

    #[test]
    fn an_appimage_without_fuse_is_started_the_same_way() {
        let image = Path::new("/opt/BoxBox.AppImage");
        assert_eq!(appimage_command(image, Some(Path::new("/tmp/.mount_BoxBoxa1b2c3"))), "\"/opt/BoxBox.AppImage\"");
        assert_eq!(appimage_command(image, Some(Path::new("/tmp/appimage_extracted_0123abcd"))), "\"/opt/BoxBox.AppImage\" --appimage-extract-and-run");
        assert_eq!(appimage_command(image, None), "\"/opt/BoxBox.AppImage\"");
    }

    #[test]
    fn portal_versions_parse() {
        assert_eq!(parse_version("xdg-desktop-portal 1.22.1\n"), Some((1, 22, 1)));
        assert_eq!(parse_version("xdg-desktop-portal 1.18.4-1ubuntu2"), Some((1, 18, 4)));
        assert_eq!(parse_version("xdg-desktop-portal 1.20"), Some((1, 20, 0)));
        assert!(parse_version("nothing").is_none());
    }

    #[test]
    fn routing_keeps_the_old_line_and_restores_it() {
        let source = "[preferred]\ndefault=gtk;\norg.freedesktop.impl.portal.FileChooser=gtk;\norg.freedesktop.impl.portal.Secret=gnome-keyring;\n";
        let routed = route(source, true).unwrap();
        assert!(routed.contains("org.freedesktop.impl.portal.FileChooser=boxbox;gtk;\n"));
        assert!(!routed.contains("\norg.freedesktop.impl.portal.FileChooser=gtk;"), "the old line is replaced");
        assert!(routed.contains("Secret=gnome-keyring"));
        assert_eq!(route(&routed, true).unwrap(), routed, "enabling twice changes nothing");
        assert_eq!(route(&routed, false).unwrap(), source);
    }

    #[test]
    fn routing_adds_a_group_when_there_is_none() {
        let routed = route("", true).unwrap();
        assert!(routed.starts_with("[preferred]\n# BoxBox file chooser override\n# Previous: \norg.freedesktop.impl.portal.FileChooser=boxbox;\n"));
        assert_eq!(route(&routed, false).unwrap(), "[preferred]\n");
        // The line goes inside [preferred], before the next group.
        let routed = route("[preferred]\ndefault=gtk\n[other]\nx=y\n", true).unwrap();
        assert!(routed.find("FileChooser=boxbox").unwrap() < routed.find("[other]").unwrap());
    }

    #[test]
    fn edited_lines_are_left_alone() {
        let edited = "[preferred]\n# BoxBox file chooser override\norg.freedesktop.impl.portal.FileChooser=kde\n";
        assert!(route(edited, false).is_err());
    }
}
