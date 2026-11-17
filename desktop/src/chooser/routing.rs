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

/// Routes Open and Save dialogs to `program --portal`. Returns the file changed.
pub fn enable(program: &Path) -> Result<PathBuf> {
    ensure!(program.is_absolute(), "the program path must be absolute");
    let service = service_file();
    if let Ok(existing) = std::fs::read_to_string(&service) {
        ensure!(existing.starts_with(MANAGED), "{} was not written by BoxBox; remove it first", service.display());
    }
    write_atomic(&portal_file(), &format!("[portal]\nDBusName={BUS_NAME}\nInterfaces={INTERFACE};\n"))?;
    write_atomic(&service, &format!("{MANAGED}\n[D-BUS Service]\nName={BUS_NAME}\nExec={} --portal\n", quoted(program)))?;
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
    use super::route;

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
