//! Custom actions: commands that apps and the user add to the item menu, read
//! from KDE service menus (`kio/servicemenus/*.desktop`, which Dolphin shows)
//! and Thunar's `uca.xml`, and offered for the items they declare. Both are
//! small files read when a menu opens, so an edit shows on the next right-click.
//! Reading and running touch the filesystem, so they run off the UI thread.

use std::collections::{HashMap, HashSet};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context as _, Result};

use crate::local::mime;

/// One selected item, as the matching rules see it.
pub struct Target {
    pub path: PathBuf,
    pub is_dir: bool,
    mime: String,
}

impl Target {
    /// The type comes from the name, as for the item's icon.
    pub fn new(path: PathBuf, is_dir: bool) -> Self {
        let mime = match is_dir {
            true => "inode/directory".into(),
            false => mime::from_name(&path).unwrap_or_else(|| "application/octet-stream".into()),
        };
        Self { path, is_dir, mime }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub label: String,
    /// A theme icon name or an absolute path.
    pub icon: Option<String>,
    /// The submenu it is listed in; `None` lists it in the menu itself.
    pub submenu: Option<String>,
    exec: Exec,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Exec {
    /// A KDE action's Exec line, split by the Desktop Entry rules.
    Desktop { line: String, file: PathBuf },
    /// A Thunar command: codes become shell-quoted text, then the line is split
    /// as a shell would, without running a shell.
    Thunar(String),
}

impl Action {
    /// Whether the command only opens a terminal, which BoxBox's own Open in
    /// Terminal row already does. Thunar ships such an action by default.
    pub fn opens_terminal(&self) -> bool {
        let program = match &self.exec {
            Exec::Desktop { line, .. } => crate::launch::exec_words(line).and_then(|words| words.into_iter().next()),
            Exec::Thunar(command) => shlex::split(command).and_then(|words| words.into_iter().next()),
        };
        program.is_some_and(|program| crate::launch::is_terminal(&program))
    }

    /// Starts the command for `paths`, in the folder that holds the first.
    pub fn run(&self, paths: &[PathBuf]) -> Result<()> {
        let first = paths.first().context("no items")?;
        let folder = first.parent().unwrap_or(Path::new("/"));
        for argv in self.argv(paths)? {
            let (program, args) = argv.split_first().context("the command is empty")?;
            let mut command = Command::new(program);
            command.args(args).current_dir(folder);
            crate::launch::detach(command).with_context(|| format!("could not start {program}"))?;
        }
        Ok(())
    }

    /// The command lines to start: one, or one per item when a KDE line takes a single file.
    fn argv(&self, paths: &[PathBuf]) -> Result<Vec<Vec<String>>> {
        match &self.exec {
            Exec::Desktop { line, file } => {
                let words = crate::launch::exec_words(line).context("the action's Exec line cannot be read")?;
                let single = words.iter().any(|word| word.contains("%f") || word.contains("%u"));
                Ok(match single {
                    true => paths.iter().map(|path| desktop_argv(&words, std::slice::from_ref(path), self, file)).collect(),
                    false => vec![desktop_argv(&words, paths, self, file)],
                })
            }
            Exec::Thunar(command) => {
                let line = thunar_line(command, paths)?;
                Ok(vec![shlex::split(&line).context("the action's command cannot be read")?])
            }
        }
    }
}

fn names(paths: &[PathBuf]) -> impl Iterator<Item = String> + '_ {
    paths.iter().map(|path| path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned()))
}

fn folders(paths: &[PathBuf]) -> impl Iterator<Item = String> + '_ {
    paths.iter().map(|path| path.parent().map_or_else(String::new, |folder| folder.to_string_lossy().into_owned()))
}

/// Expands Desktop Entry field codes. A code alone in a word that takes a list
/// becomes one argument per item; elsewhere it is replaced in place.
fn desktop_argv(words: &[String], paths: &[PathBuf], action: &Action, file: &Path) -> Vec<String> {
    let mut argv = Vec::new();
    for word in words {
        match word.as_str() {
            "%F" => argv.extend(paths.iter().map(|path| path.to_string_lossy().into_owned())),
            "%U" => argv.extend(paths.iter().map(|path| crate::os_clipboard::file_uri(path))),
            "%D" => argv.extend(folders(paths)),
            "%N" => argv.extend(names(paths)),
            "%i" => {
                if let Some(icon) = &action.icon {
                    argv.extend(["--icon".to_string(), icon.clone()]);
                }
            }
            _ => argv.push(expand(word, |code| match code {
                'f' => paths.first().map(|path| path.to_string_lossy().into_owned()),
                'u' => paths.first().map(|path| crate::os_clipboard::file_uri(path)),
                'd' => folders(paths).next(),
                'n' => names(paths).next(),
                'c' => Some(action.label.clone()),
                'k' => Some(file.to_string_lossy().into_owned()),
                _ => None,
            })),
        }
    }
    argv
}

/// Thunar's codes: each path is quoted for the shell, and a list is joined by spaces.
fn thunar_line(command: &str, paths: &[PathBuf]) -> Result<String> {
    let quote = |text: String| shlex::try_quote(&text).map(|quoted| quoted.into_owned()).ok();
    let all = |items: Vec<String>| items.into_iter().map(quote).collect::<Option<Vec<_>>>().map(|items| items.join(" "));
    let strings = |paths: &[PathBuf]| paths.iter().map(|path| path.to_string_lossy().into_owned()).collect::<Vec<_>>();
    let mut failed = false;
    let line = expand(command, |code| {
        let value = match code {
            'f' => paths.first().and_then(|path| quote(path.to_string_lossy().into_owned())),
            'F' => all(strings(paths)),
            'u' => paths.first().and_then(|path| quote(crate::os_clipboard::file_uri(path))),
            'U' => all(paths.iter().map(|path| crate::os_clipboard::file_uri(path)).collect()),
            'd' => folders(paths).next().and_then(quote),
            'D' => all(folders(paths).collect()),
            'n' => names(paths).next().and_then(quote),
            'N' => all(names(paths).collect()),
            // Not a code, such as the `%` in `50%`: kept as written.
            other => return Some(format!("%{other}")),
        };
        // A NUL in a path cannot be quoted; refuse instead of passing it whole.
        failed |= value.is_none();
        value
    });
    if failed {
        bail!("a file name cannot be passed to the command");
    }
    Ok(line)
}

/// Replaces `%x` codes by `value`; `%%` is a percent sign and `None` drops the code.
fn expand(text: &str, mut value: impl FnMut(char) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some(code) => out.extend(value(code)),
            None => {}
        }
    }
    out
}

/// The actions that apply to every one of `targets`, KDE's first, in file order.
pub fn for_targets(targets: &[Target]) -> Vec<Action> {
    let _span = crate::profile::span("custom actions");
    if targets.is_empty() {
        return Vec::new();
    }
    let mut actions = kde_actions(targets);
    actions.extend(thunar_actions(targets));
    let mut seen = HashSet::new();
    actions.retain(|action| seen.insert((action.label.clone(), action.exec.clone())));
    actions
}

// KDE service menus.

/// User files first, so they shadow the system's; then the KF5 location.
fn kde_dirs() -> Vec<(PathBuf, bool)> {
    let data = mime::data_dirs();
    let user = data.first().cloned();
    let roots = ["kio/servicemenus", "kservices5/ServiceMenus"];
    let user = user.as_ref();
    roots.iter().flat_map(|root| data.iter().map(move |dir| (dir.join(root), Some(dir) == user))).collect()
}

fn kde_actions(targets: &[Target]) -> Vec<Action> {
    let mut seen = HashSet::new();
    let mut actions = Vec::new();
    for (dir, user) in kde_dirs() {
        let Ok(children) = std::fs::read_dir(&dir) else { continue };
        let mut files: Vec<PathBuf> = children.flatten().map(|child| child.path()).filter(|path| path.extension().is_some_and(|extension| extension == "desktop")).collect();
        files.sort();
        for file in files {
            let Some(name) = file.file_name().map(|name| name.to_os_string()) else { continue };
            if !seen.insert(name) {
                continue;
            }
            // KDE runs a service menu from the user's own folder only when it is
            // executable, so a downloaded file cannot add commands by itself.
            if user && std::fs::metadata(&file).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 == 0) {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&file) {
                actions.extend(parse_service_menu(&text, &file, targets));
            }
        }
    }
    actions
}

/// `[group] key=value` sections, keeping unlocalised keys only.
fn groups(text: &str) -> HashMap<String, HashMap<String, String>> {
    let mut groups: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current = None;
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|line| line.strip_suffix(']')) {
            current = Some(name.to_string());
            continue;
        }
        let (Some(group), Some((key, value))) = (&current, line.split_once('=')) else { continue };
        if line.starts_with('#') || key.contains('[') {
            continue;
        }
        groups.entry(group.clone()).or_default().insert(key.trim().to_string(), crate::open_with::unescape(value.trim()));
    }
    groups
}

fn list(value: Option<&String>, separator: char) -> Vec<&str> {
    value.map(|value| value.split(separator).map(str::trim).filter(|item| !item.is_empty()).collect()).unwrap_or_default()
}

fn parse_service_menu(text: &str, file: &Path, targets: &[Target]) -> Vec<Action> {
    let groups = groups(text);
    let Some(entry) = groups.get("Desktop Entry") else { return Vec::new() };
    let value = |key: &str| entry.get(key).map(String::as_str).unwrap_or("");
    // Conditions that need a running app or a D-Bus call are not checked here, so
    // such menus are left out rather than shown where they do not apply.
    if value("Hidden") == "true" || !value("X-KDE-ShowIfRunning").is_empty() || !value("X-KDE-ShowIfDBusCall").is_empty() {
        return Vec::new();
    }
    let protocols = list(entry.get("X-KDE-Protocols").or(entry.get("X-KDE-Protocol")), ',');
    if !protocols.is_empty() && !protocols.contains(&"file") {
        return Vec::new();
    }
    let count = targets.len();
    let number = |key: &str| value(key).parse::<usize>().ok();
    if number("X-KDE-MinNumberOfUrls").is_some_and(|min| count < min) || number("X-KDE-MaxNumberOfUrls").is_some_and(|max| count > max) {
        return Vec::new();
    }
    let required: Vec<usize> = list(entry.get("X-KDE-RequiredNumberOfUrls"), ',').iter().filter_map(|number| number.parse().ok()).collect();
    if !required.is_empty() && !required.contains(&count) {
        return Vec::new();
    }
    // KF6 lists types in MimeType; KF5 also put them among the service types.
    let mut types = list(entry.get("MimeType"), ';');
    for key in ["ServiceTypes", "X-KDE-ServiceTypes"] {
        types.extend(list(entry.get(key), ',').into_iter().filter(|kind| kind.contains('/') && !kind.starts_with("KonqPopupMenu")));
    }
    let excluded = list(entry.get("ExcludeServiceTypes"), ',');
    let applies = |target: &Target| types.iter().any(|kind| kde_type_matches(kind, target)) && !excluded.iter().any(|kind| kde_type_matches(kind, target));
    if !targets.iter().all(applies) {
        return Vec::new();
    }
    let submenu = match value("X-KDE-Submenu") {
        "" if value("X-KDE-Priority") == "TopLevel" => None,
        // Dolphin groups the rest under Actions.
        "" => Some("Actions".to_string()),
        name => Some(name.to_string()),
    };
    list(entry.get("Actions"), ';')
        .into_iter()
        .filter(|name| *name != "_SEPARATOR_")
        .filter_map(|name| {
            let action = groups.get(&format!("Desktop Action {name}"))?;
            let line = action.get("Exec").filter(|line| !line.is_empty())?;
            if action.get("NoDisplay").is_some_and(|value| value == "true") {
                return None;
            }
            Some(Action {
                label: action.get("Name").cloned().unwrap_or_else(|| name.to_string()),
                icon: action.get("Icon").filter(|icon| !icon.is_empty()).cloned(),
                submenu: submenu.clone(),
                exec: Exec::Desktop { line: line.clone(), file: file.to_path_buf() },
            })
        })
        .collect()
}

fn kde_type_matches(kind: &str, target: &Target) -> bool {
    match kind {
        "all/all" | "*" | "*/*" => true,
        "all/allfiles" => !target.is_dir,
        "inode/directory" | "inode/*" => target.is_dir,
        _ => !target.is_dir && mime::is_a(&target.mime, kind),
    }
}

// Thunar custom actions.

/// The first `Thunar/uca.xml` among the config folders, as Thunar reads it.
fn thunar_file() -> Option<PathBuf> {
    let home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")));
    let system = std::env::var("XDG_CONFIG_DIRS").ok().filter(|dirs| !dirs.is_empty()).unwrap_or_else(|| "/etc/xdg".into());
    home.into_iter()
        .chain(system.split(':').map(PathBuf::from).filter(|path| path.is_absolute()))
        .map(|dir| dir.join("Thunar/uca.xml"))
        .find(|file| file.is_file())
}

fn thunar_actions(targets: &[Target]) -> Vec<Action> {
    let Some(file) = thunar_file() else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(&file) else { return Vec::new() };
    parse_uca(&text, targets).unwrap_or_else(|error| {
        log::warn!("{}: {error}", file.display());
        Vec::new()
    })
}

fn parse_uca(text: &str, targets: &[Target]) -> Result<Vec<Action>, roxmltree::Error> {
    let document = roxmltree::Document::parse(text)?;
    let actions = document.root_element().children().filter(|node| node.has_tag_name("action"));
    Ok(actions
        .filter_map(|action| {
            let text = |tag: &str| action.children().find(|node| node.has_tag_name(tag)).and_then(|node| node.text()).map(str::trim).unwrap_or("");
            let flag = |tag: &str| action.children().any(|node| node.has_tag_name(tag));
            let (name, command) = (text("name"), text("command"));
            if name.is_empty() || command.is_empty() || !in_range(text("range"), targets.len()) {
                return None;
            }
            let patterns: Vec<String> = text("patterns").split(';').map(|pattern| pattern.trim().to_lowercase()).filter(|pattern| !pattern.is_empty()).collect();
            let applies = |target: &Target| {
                let name = target.path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
                let kind = match target.is_dir {
                    true => "directories",
                    false => file_kind(&target.mime),
                };
                patterns.iter().any(|pattern| glob(pattern, &name)) && (flag(kind) || (!target.is_dir && flag("other-files")))
            };
            targets.iter().all(applies).then(|| Action {
                label: name.to_string(),
                icon: Some(text("icon")).filter(|icon| !icon.is_empty()).map(str::to_string),
                submenu: Some(text("submenu")).filter(|submenu| !submenu.is_empty()).map(str::to_string),
                exec: Exec::Thunar(command.to_string()),
            })
        })
        .collect())
}

/// Thunar's file classes, by the type's family.
fn file_kind(mime_type: &str) -> &'static str {
    match mime_type.split('/').next() {
        Some("audio") => "audio-files",
        Some("image") => "image-files",
        Some("video") => "video-files",
        _ if mime::is_a(mime_type, "text/plain") => "text-files",
        _ => "other-files",
    }
}

/// `<range>`: empty or `*` for any count, `n`, or `min-max` with `*` for no bound.
fn in_range(range: &str, count: usize) -> bool {
    let bound = |text: &str, unbounded: usize| match text.trim() {
        "" | "*" => Some(unbounded),
        number => number.parse().ok(),
    };
    let (min, max) = match range.split_once('-') {
        Some((min, max)) => (bound(min, 0), bound(max, usize::MAX)),
        None if range.trim().is_empty() || range.trim() == "*" => (Some(0), Some(usize::MAX)),
        None => (bound(range, 0), bound(range, usize::MAX)),
    };
    matches!((min, max), (Some(min), Some(max)) if (min..=max).contains(&count))
}

/// `*` and `?` over characters, as GLib's simple patterns match.
fn glob(pattern: &str, name: &str) -> bool {
    let (pattern, name): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    let (mut p, mut n, mut star, mut mark) = (0, 0, None, 0);
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            mark = n;
            p += 1;
        } else if let Some(position) = star {
            p = position + 1;
            mark += 1;
            n = mark;
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|ch| *ch == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> Target {
        Target { path: path.into(), is_dir: false, mime: mime::from_name(Path::new(path)).unwrap_or_else(|| "application/octet-stream".into()) }
    }

    fn folder(path: &str) -> Target {
        Target::new(path.into(), true)
    }

    const SERVICE_MENU: &str = "[Desktop Entry]
Type=Service
MimeType=image/*;
Actions=rotate;_SEPARATOR_;hidden
X-KDE-Submenu=Image
X-KDE-MaxNumberOfUrls=2

[Desktop Action rotate]
Name=Rotate
Name[de]=Drehen
Icon=object-rotate-right
Exec=convert -rotate 90 %f %f

[Desktop Action hidden]
Name=Hidden
Exec=true
NoDisplay=true
";

    #[test]
    fn service_menus_apply_by_type_and_count() {
        let path = Path::new("/menus/image.desktop");
        let actions = parse_service_menu(SERVICE_MENU, path, &[file("/p/a.png")]);
        assert_eq!(actions.len(), 1);
        assert_eq!((actions[0].label.as_str(), actions[0].submenu.as_deref()), ("Rotate", Some("Image")));
        assert!(parse_service_menu(SERVICE_MENU, path, &[file("/p/a.png"), file("/p/b.txt")]).is_empty(), "every item must match");
        assert!(parse_service_menu(SERVICE_MENU, path, &[file("/a.png"), file("/b.png"), file("/c.png")]).is_empty(), "over the maximum");
        assert!(parse_service_menu(SERVICE_MENU, path, &[folder("/p")]).is_empty());
        let folders = "[Desktop Entry]\nMimeType=inode/directory;\nActions=t\nX-KDE-Priority=TopLevel\n[Desktop Action t]\nName=Konsole Here\nExec=konsole --workdir %f\n";
        let actions = parse_service_menu(folders, path, &[folder("/p")]);
        assert_eq!(actions[0].submenu, None);
        assert!(actions[0].opens_terminal());
        assert!(parse_service_menu(folders, path, &[file("/p/a.png")]).is_empty());
        let any = "[Desktop Entry]\nServiceTypes=KonqPopupMenu/Plugin,all/allfiles\nActions=x\n[Desktop Action x]\nName=X\nExec=x %F\n";
        assert_eq!(parse_service_menu(any, path, &[file("/a.png")])[0].submenu.as_deref(), Some("Actions"));
        assert!(parse_service_menu(any, path, &[folder("/p")]).is_empty(), "all/allfiles leaves folders out");
        let busy = "[Desktop Entry]\nMimeType=all/all;\nX-KDE-ShowIfRunning=org.kde.x\nActions=x\n[Desktop Action x]\nName=X\nExec=x\n";
        assert!(parse_service_menu(busy, path, &[file("/a.png")]).is_empty());
    }

    #[test]
    fn kde_codes_expand_per_file_or_as_a_list() {
        let action = parse_service_menu(SERVICE_MENU, Path::new("/m.desktop"), &[file("/p/a.png")]).remove(0);
        let paths = [PathBuf::from("/p/a b.png"), PathBuf::from("/p/c.png")];
        assert_eq!(action.argv(&paths).unwrap(), [
            ["convert", "-rotate", "90", "/p/a b.png", "/p/a b.png"],
            ["convert", "-rotate", "90", "/p/c.png", "/p/c.png"],
        ]);
        let list = Action { exec: Exec::Desktop { line: "app --name=%c %U %N".into(), file: "/m.desktop".into() }, ..action };
        assert_eq!(list.argv(&paths).unwrap(), [["app", "--name=Rotate", "file:///p/a%20b.png", "file:///p/c.png", "a b.png", "c.png"]]);
    }

    const UCA: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<actions>
<action><icon>utilities-terminal</icon><name>Open Terminal Here</name><unique-id>1</unique-id>
<command>kitty --directory %f</command><patterns>*</patterns><directories/></action>
<action><name>Shrink</name><submenu>Images</submenu><command>magick %F -resize 50% out.png</command>
<range>1-*</range><patterns>*.png;*.JPG</patterns><image-files/></action>
<action><name>Count words</name><command>wc -w %N</command><range>1</range><patterns>*</patterns><text-files/><other-files/></action>
</actions>"#;

    #[test]
    fn thunar_actions_apply_by_pattern_kind_and_range() {
        let labels = |targets: &[Target]| parse_uca(UCA, targets).unwrap().into_iter().map(|action| action.label).collect::<Vec<_>>();
        assert_eq!(labels(&[folder("/p")]), ["Open Terminal Here"]);
        assert_eq!(labels(&[file("/p/a.png"), file("/p/B.jpg")]), ["Shrink"]);
        assert_eq!(labels(&[file("/p/a.png")]), ["Shrink", "Count words"], "other-files takes any file");
        assert_eq!(labels(&[file("/p/notes.txt"), file("/p/b.txt")]), Vec::<String>::new(), "out of range");
        let actions = parse_uca(UCA, &[folder("/p")]).unwrap();
        assert!(actions[0].opens_terminal());
        assert_eq!(actions[0].argv(&[PathBuf::from("/p/it's")]).unwrap(), [["kitty", "--directory", "/p/it's"]]);
        let shrink = parse_uca(UCA, &[file("/a.png")]).unwrap().remove(0);
        assert_eq!(shrink.submenu.as_deref(), Some("Images"));
        assert_eq!(shrink.argv(&[PathBuf::from("/a b.png"), PathBuf::from("/c.png")]).unwrap(), [["magick", "/a b.png", "/c.png", "-resize", "50%", "out.png"]]);
    }

    #[test]
    fn ranges_and_globs_follow_thunar() {
        assert!(in_range("", 3) && in_range("*", 3) && in_range("1", 1) && !in_range("1", 2));
        assert!(in_range("2-*", 5) && !in_range("2-*", 1) && in_range("1-3", 3) && !in_range("x", 1));
        assert!(glob("*.tar.*", "a.tar.gz") && glob("a?c", "abc") && !glob("*.png", "a.jpg") && glob("*", ""));
    }
}
