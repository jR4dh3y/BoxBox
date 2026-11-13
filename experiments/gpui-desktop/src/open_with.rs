//! Open With: the apps that can open a file, from XDG desktop entries and
//! `mimeapps.list`, as `backend/internal/service/desktop_apps.go` lists them
//! for the web app. Recommended apps come first (the default, then added
//! associations, then entries registering the type); every other visible app
//! that takes a file follows. Launching and defaults go through GIO, as the
//! desktop's own file manager does. Everything here blocks on the filesystem
//! or a process, so it runs off the UI thread.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context as _, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct App {
    /// The desktop file ID, such as `org.gnome.TextEditor.desktop`.
    pub id: String,
    pub name: String,
    pub icon: Option<PathBuf>,
    pub is_default: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Apps {
    pub mime_type: String,
    pub recommended: Vec<App>,
    pub other: Vec<App>,
}

#[derive(Clone)]
struct Entry {
    app: App,
    path: PathBuf,
    mime_types: Vec<String>,
    /// Handler-only entries are offered when associated, never in the full list.
    no_display: bool,
    accepts_files: bool,
}

fn home_dir(variable: &str, fallback: &str) -> PathBuf {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/")).join(fallback))
}

fn search_dirs(variable: &str, fallback: &str) -> Vec<PathBuf> {
    let value = std::env::var(variable).unwrap_or_else(|_| fallback.to_string());
    value.split(':').map(PathBuf::from).filter(|path| path.is_absolute()).collect()
}

fn unique(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths.into_iter().filter(|path| seen.insert(path.clone())).collect()
}

fn application_dirs() -> Vec<PathBuf> {
    let data_home = home_dir("XDG_DATA_HOME", ".local/share");
    unique(
        std::iter::once(data_home.join("applications"))
            .chain(search_dirs("XDG_DATA_DIRS", "/usr/local/share:/usr/share").into_iter().map(|dir| dir.join("applications"))),
    )
}

fn mimeapps_files() -> Vec<PathBuf> {
    let desktops: Vec<String> = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .map(str::to_lowercase)
        .filter(|desktop| !desktop.is_empty() && !desktop.contains(['/', '\\', '\0']))
        .collect();
    let dirs = std::iter::once(home_dir("XDG_CONFIG_HOME", ".config"))
        .chain(search_dirs("XDG_CONFIG_DIRS", "/etc/xdg"))
        .chain(application_dirs());
    unique(dirs.flat_map(|dir| {
        desktops.iter().map(|desktop| dir.join(format!("{desktop}-mimeapps.list"))).chain(std::iter::once(dir.join("mimeapps.list"))).collect::<Vec<_>>()
    }))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && !id.contains(['/', '\\', '\0', '\r', '\n', ';']) && id.ends_with(".desktop")
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// GIO refuses an Exec line it cannot split: an open quote or a trailing escape.
fn valid_exec(exec: &str) -> bool {
    let bytes = exec.as_bytes();
    let mut quote = 0u8;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if quote == b'\'' {
            if byte == b'\'' {
                quote = 0;
            }
        } else if byte == b'\\' {
            if index + 1 >= bytes.len() {
                return false;
            }
            index += 1;
        } else if byte == b'"' || byte == b'\'' {
            if quote == byte {
                quote = 0;
            } else if quote == 0 {
                quote = byte;
            }
        }
        index += 1;
    }
    quote == 0 && !exec.trim().is_empty()
}

/// Whether GIO passes the opened file to the command.
fn exec_accepts_files(exec: &str) -> bool {
    let exec = exec.replace("%%", "");
    ["%f", "%F", "%u", "%U"].iter().any(|code| exec.contains(code))
}

fn in_path(program: &str) -> bool {
    if program.contains('/') {
        return Path::new(program).is_file();
    }
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

/// Parses one entry. `Err` hides it from lower-priority directories too, as `Hidden=true` does.
fn read_entry(path: &Path, id: &str) -> Option<Result<Entry, ()>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut values = HashMap::new();
    let mut in_entry = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key.trim().to_string(), unescape(value.trim()));
        }
    }
    let value = |key: &str| values.get(key).map(String::as_str).unwrap_or("");
    if value("Hidden") == "true" {
        return Some(Err(()));
    }
    if value("Type") != "Application" {
        return None;
    }
    let exec = value("Exec");
    let dbus = value("DBusActivatable") == "true";
    if (exec.is_empty() && !dbus) || (!exec.is_empty() && !valid_exec(exec)) {
        return None;
    }
    if !value("TryExec").is_empty() && !in_path(value("TryExec")) {
        return None;
    }
    let name = if value("Name").is_empty() { id.trim_end_matches(".desktop").to_string() } else { value("Name").to_string() };
    Some(Ok(Entry {
        app: App { id: id.to_string(), name, icon: icon_path(value("Icon")), is_default: false },
        path: path.to_path_buf(),
        mime_types: value("MimeType").split(';').map(str::trim).filter(|mime| !mime.is_empty()).map(str::to_string).collect(),
        no_display: value("NoDisplay") == "true",
        accepts_files: dbus || exec_accepts_files(exec),
    }))
}

fn installed() -> HashMap<String, Entry> {
    let mut entries = HashMap::new();
    let mut seen = HashSet::new();
    for root in application_dirs() {
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(children) = std::fs::read_dir(&dir) else { continue };
            for child in children.flatten() {
                let path = child.path();
                let Ok(kind) = child.file_type() else { continue };
                if kind.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.extension().is_none_or(|extension| extension != "desktop") {
                    continue;
                }
                let Ok(relative) = path.strip_prefix(&root) else { continue };
                let id = relative.to_string_lossy().replace('/', "-");
                if !valid_id(&id) || !seen.insert(id.clone()) {
                    continue;
                }
                if let Some(Ok(entry)) = read_entry(&path, &id) {
                    entries.insert(id, entry);
                }
            }
        }
    }
    entries
}

#[derive(Default)]
struct Associations {
    defaults: Vec<String>,
    added: Vec<String>,
    removed: HashSet<String>,
}

fn associations(mime_type: &str, files: &[PathBuf]) -> Associations {
    let mut found = Associations::default();
    for path in files {
        let Ok(text) = std::fs::read_to_string(path) else { continue };
        let mut section = "";
        for line in text.lines().map(str::trim) {
            if let Some(name) = line.strip_prefix('[').and_then(|line| line.strip_suffix(']')) {
                section = name;
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            if key.trim() != mime_type {
                continue;
            }
            let ids: Vec<String> = value.split(';').map(str::trim).filter(|id| valid_id(id)).map(str::to_string).collect();
            match section {
                "Default Applications" if found.defaults.is_empty() => found.defaults = ids,
                "Added Associations" => found.added.extend(ids),
                "Removed Associations" => found.removed.extend(ids),
                _ => {}
            }
        }
    }
    found
}

fn registers(mime_types: &[String], mime_type: &str) -> bool {
    mime_types.iter().any(|candidate| {
        candidate == mime_type || candidate.strip_suffix('*').is_some_and(|prefix| prefix.ends_with('/') && mime_type.starts_with(prefix))
    })
}

fn by_name(entries: &HashMap<String, Entry>, ids: &mut [String]) {
    ids.sort_by(|left, right| {
        let names = |id: &String| entries[id].app.name.to_lowercase();
        names(left).cmp(&names(right)).then_with(|| left.cmp(right))
    });
}

fn content_type(path: &Path) -> Result<String> {
    let output = Command::new("gio")
        .args(["info", "--attributes=standard::content-type"])
        .arg(path)
        .output()
        .context("GIO is needed to tell the file's type")?;
    if !output.status.success() {
        bail!("GIO could not inspect {}", path.display());
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.trim().strip_prefix("standard::content-type:"))
        .map(|value| value.trim().split(';').next().unwrap_or("").trim().to_string())
        .filter(|mime| mime.contains('/'))
        .context("GIO returned no content type")
}

fn list_for(mime_type: &str, entries: &HashMap<String, Entry>) -> Apps {
    let found = associations(mime_type, &mimeapps_files());
    let usable = |id: &String| entries.get(id).is_some_and(|entry| entry.accepts_files);
    let mut ordered: Vec<String> = Vec::new();
    let push = |id: &String, ordered: &mut Vec<String>| {
        if usable(id) && !ordered.contains(id) {
            ordered.push(id.clone());
        }
    };
    for id in &found.defaults {
        push(id, &mut ordered);
    }
    for id in found.added.iter().filter(|id| !found.removed.contains(*id)) {
        push(id, &mut ordered);
    }
    let mut matching: Vec<String> = entries
        .iter()
        .filter(|(id, entry)| !found.removed.contains(*id) && registers(&entry.mime_types, mime_type))
        .map(|(id, _)| id.clone())
        .collect();
    by_name(entries, &mut matching);
    for id in &matching {
        push(id, &mut ordered);
    }
    let default = found.defaults.iter().find(|id| usable(id)).cloned();
    let recommended: Vec<App> = ordered
        .iter()
        .map(|id| App { is_default: Some(id) == default.as_ref(), ..entries[id].app.clone() })
        .collect();
    let mut other: Vec<String> = entries
        .iter()
        .filter(|(id, entry)| !ordered.contains(*id) && !entry.no_display && entry.accepts_files)
        .map(|(id, _)| id.clone())
        .collect();
    by_name(entries, &mut other);
    Apps { mime_type: mime_type.to_string(), recommended, other: other.iter().map(|id| entries[id].app.clone()).collect() }
}

/// The apps that can open `path`.
pub fn list(path: &Path) -> Result<Apps> {
    let mime_type = content_type(path)?;
    Ok(list_for(&mime_type, &installed()))
}

/// Opens `path` in the app, which must be one `list` offers for it.
pub fn launch(path: &Path, app_id: &str, make_default: bool) -> Result<()> {
    let mime_type = content_type(path)?;
    let entries = installed();
    let apps = list_for(&mime_type, &entries);
    if !apps.recommended.iter().chain(&apps.other).any(|app| app.id == app_id) {
        bail!("that app cannot open this file");
    }
    let entry = &entries[app_id];
    let status = Command::new("gio").arg("launch").arg(&entry.path).arg(path).status().context("could not start GIO")?;
    if !status.success() {
        bail!("{} did not start", entry.app.name);
    }
    if make_default {
        let status = Command::new("gio").args(["mime", &mime_type, app_id]).status().context("could not start GIO")?;
        if !status.success() {
            bail!("could not make {} the default for {mime_type}", entry.app.name);
        }
    }
    Ok(())
}

/// An icon name or path as a file: the hicolor theme and pixmaps, PNG or SVG.
fn icon_path(icon: &str) -> Option<PathBuf> {
    if icon.is_empty() {
        return None;
    }
    let as_path = Path::new(icon);
    if as_path.is_absolute() {
        return as_path.is_file().then(|| as_path.to_path_buf());
    }
    let data_dirs = std::iter::once(home_dir("XDG_DATA_HOME", ".local/share")).chain(search_dirs("XDG_DATA_DIRS", "/usr/local/share:/usr/share"));
    for dir in data_dirs {
        for size in ["scalable", "48x48", "64x64", "32x32", "128x128", "256x256"] {
            for extension in ["svg", "png"] {
                let candidate = dir.join(format!("icons/hicolor/{size}/apps/{icon}.{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        for extension in ["png", "svg"] {
            let candidate = dir.join(format!("pixmaps/{icon}.{extension}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// `filterOpenWithChoices`: search covers both groups, by name or ID.
pub fn filter(apps: &Apps, search: &str) -> (Vec<App>, Vec<App>) {
    let query = search.trim().to_lowercase();
    let matches = |app: &&App| query.is_empty() || app.name.to_lowercase().contains(&query) || app.id.to_lowercase().contains(&query);
    (apps.recommended.iter().filter(matches).cloned().collect(), apps.other.iter().filter(matches).cloned().collect())
}

/// `initialOpenWithChoice`: the default, else the first recommended app.
pub fn initial_choice(apps: &Apps) -> Option<String> {
    apps.recommended.iter().find(|app| app.is_default).or(apps.recommended.first()).map(|app| app.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, name: &str, mime: &[&str], accepts: bool, no_display: bool) -> (String, Entry) {
        (
            id.to_string(),
            Entry {
                app: App { id: id.into(), name: name.into(), icon: None, is_default: false },
                path: PathBuf::from(format!("/apps/{id}")),
                mime_types: mime.iter().map(|mime| mime.to_string()).collect(),
                no_display,
                accepts_files: accepts,
            },
        )
    }

    #[test]
    fn registered_apps_lead_and_the_rest_follow_by_name() {
        let entries: HashMap<_, _> = [
            entry("b.desktop", "Beta", &["text/plain"], true, false),
            entry("a.desktop", "Alpha", &["text/*"], true, false),
            entry("z.desktop", "Zed", &[], true, false),
            entry("h.desktop", "Handler", &[], true, true),
            entry("n.desktop", "No files", &["text/plain"], false, false),
        ]
        .into_iter()
        .collect();
        let apps = list_for("text/plain", &entries);
        let names = |apps: &[App]| apps.iter().map(|app| app.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&apps.recommended), ["Alpha", "Beta"]);
        assert_eq!(names(&apps.other), ["Zed"]);
    }

    #[test]
    fn exec_lines_follow_gio() {
        assert!(exec_accepts_files("gedit %U"));
        assert!(!exec_accepts_files("app %%f"));
        assert!(valid_exec("sh -c 'echo \"hi\"' %f"));
        assert!(!valid_exec("app \"open"));
        assert!(!valid_exec("app \\"));
        assert_eq!(unescape("a\\sb\\\\c"), "a b\\c");
    }
}
