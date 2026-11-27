//! Running what a folder holds: executable scripts and programs, and `.desktop`
//! launchers, with the checks Thunar and Nautilus make. A file runs only when
//! it has the execute bit; a launcher is trusted when it is executable or
//! installed among the system's applications. Everything here touches the
//! filesystem or starts a process, so it runs off the UI thread.

use std::collections::HashMap;
use std::io::Read as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context as _, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Runnable {
    /// A desktop entry for an application.
    Launcher { name: String, trusted: bool },
    /// An executable file: a script with `#!`, or a compiled program.
    Program { script: bool },
    /// Anything else, which opens in its app.
    Document,
}

fn executable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// What activating `path` should do.
pub fn classify(path: &Path) -> Runnable {
    if path.extension().is_some_and(|extension| extension == "desktop") {
        if let Some(entry) = DesktopEntry::read(path) {
            if entry.value("Type") == "Application" && !entry.value("Exec").is_empty() {
                let installed = crate::open_with::application_dirs().iter().any(|dir| path.starts_with(dir));
                let name = match entry.value("Name") {
                    "" => path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default(),
                    name => name.to_string(),
                };
                return Runnable::Launcher { name, trusted: installed || executable(path) };
            }
        }
        return Runnable::Document;
    }
    if !executable(path) {
        return Runnable::Document;
    }
    let mut head = [0u8; 4];
    let read = std::fs::File::open(path).and_then(|mut file| file.read(&mut head)).unwrap_or(0);
    match &head[..read] {
        [b'#', b'!', ..] => Runnable::Program { script: true },
        [0x7f, b'E', b'L', b'F'] => Runnable::Program { script: false },
        _ => Runnable::Document,
    }
}

/// Starts the program in its own folder, without waiting for it.
pub fn run(path: &Path) -> Result<()> {
    if !executable(path) {
        bail!("{} is not marked as executable", path.display());
    }
    let mut command = Command::new(path);
    if let Some(folder) = path.parent() {
        command.current_dir(folder);
    }
    detach(command)
}

/// Marks a launcher as trusted the way other file managers do: executable by its owner.
pub fn trust(path: &Path) -> Result<()> {
    let mode = std::fs::metadata(path)?.permissions().mode();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode | 0o100)).with_context(|| format!("cannot mark {} as trusted", path.display()))
}

/// Starts the application a launcher describes. Untrusted launchers are refused.
pub fn launch(path: &Path) -> Result<()> {
    if classify(path) != (Runnable::Launcher { name: name_of(path), trusted: true }) {
        bail!("{} is not a trusted launcher", path.display());
    }
    let entry = DesktopEntry::read(path).context("cannot read the launcher")?;
    let argv = exec_argv(entry.value("Exec"), entry.value("Name"), entry.value("Icon"), path).context("the launcher's Exec line cannot be read")?;
    anyhow::ensure!(!argv.is_empty(), "the launcher's Exec line is empty");
    let mut command = match entry.value("Terminal") == "true" {
        true => in_terminal(&argv).context("this launcher runs in a terminal, and no terminal was found; set $TERMINAL or install xdg-terminal-exec")?,
        false => {
            let mut command = Command::new(&argv[0]);
            command.args(&argv[1..]);
            command
        }
    };
    match entry.value("Path") {
        "" => {}
        folder => {
            command.current_dir(folder);
        }
    }
    detach(command)
}

fn name_of(path: &Path) -> String {
    match classify(path) {
        Runnable::Launcher { name, .. } => name,
        _ => String::new(),
    }
}

/// Terminals by the option that makes them run a command, most common first.
/// Each takes the command as the rest of its arguments.
const TERMINALS: &[(&str, &[&str])] = &[
    ("kgx", &["--"]),
    ("gnome-terminal", &["--"]),
    ("konsole", &["-e"]),
    ("xfce4-terminal", &["-x"]),
    ("kitty", &[]),
    ("foot", &[]),
    ("alacritty", &["-e"]),
    ("wezterm", &["start", "--"]),
    ("ghostty", &["-e"]),
    ("xterm", &["-e"]),
];

/// `argv` in the user's terminal: the default from `xdg-terminal-exec` (the
/// proposed freedesktop default-terminal spec), else `$TERMINAL`, else the
/// first installed terminal known here.
fn in_terminal(argv: &[String]) -> Option<Command> {
    in_terminal_with(argv, std::env::var("TERMINAL").ok(), &installed)
}

fn installed(name: &str) -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).any(|folder| executable(&folder.join(name)))
}

/// The user's terminal and the options that make it run a command.
fn user_terminal(variable: Option<String>, installed: &dyn Fn(&str) -> bool) -> Option<(String, &'static [&'static str])> {
    if installed("xdg-terminal-exec") {
        return Some(("xdg-terminal-exec".into(), &[]));
    }
    if let Some(terminal) = variable.filter(|terminal| !terminal.trim().is_empty()) {
        // A known terminal keeps its own option; anything else gets the common `-e`.
        let options = TERMINALS.iter().find(|(name, _)| Path::new(&terminal).file_name().is_some_and(|file| file == *name)).map_or(&["-e"][..], |(_, options)| options);
        return Some((terminal, options));
    }
    TERMINALS.iter().find(|(name, _)| installed(name)).map(|(name, options)| ((*name).into(), *options))
}

fn in_terminal_with(argv: &[String], variable: Option<String>, installed: &dyn Fn(&str) -> bool) -> Option<Command> {
    let (program, before) = user_terminal(variable, installed)?;
    let mut command = Command::new(program);
    command.args(before).args(argv);
    Some(command)
}

/// Opens the user's terminal in `folder`. Every terminal starts its shell in
/// its working directory, so no terminal-specific option is needed.
pub fn terminal_in(folder: &Path) -> Result<()> {
    let (program, _) = user_terminal(std::env::var("TERMINAL").ok(), &installed).context("no terminal was found; set $TERMINAL or install xdg-terminal-exec")?;
    let mut command = Command::new(program);
    command.current_dir(folder);
    detach(command)
}

/// Whether `program` (a name or a path) is a terminal, so a custom action that
/// only opens one repeats Open in Terminal.
pub fn is_terminal(program: &str) -> bool {
    let Some(name) = Path::new(program).file_name().and_then(|name| name.to_str()) else { return false };
    let variable = std::env::var("TERMINAL").ok();
    name == "xdg-terminal-exec"
        || TERMINALS.iter().any(|(known, _)| *known == name)
        || variable.as_deref().and_then(|terminal| Path::new(terminal).file_name()).is_some_and(|terminal| terminal == name)
}

/// Spawns `command` and reaps it when it exits, so it never lingers as a zombie.
pub(crate) fn detach(mut command: Command) -> Result<()> {
    let mut child = command.spawn().context("could not start it")?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

struct DesktopEntry {
    values: HashMap<String, String>,
}

impl DesktopEntry {
    fn read(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        let mut values = HashMap::new();
        let mut in_entry = false;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') && line.ends_with(']') {
                in_entry = line == "[Desktop Entry]";
                continue;
            }
            if in_entry && !line.starts_with('#') {
                if let Some((key, value)) = line.split_once('=') {
                    values.insert(key.trim().to_string(), crate::open_with::unescape(value.trim()));
                }
            }
        }
        (!values.is_empty()).then_some(Self { values })
    }

    fn value(&self, key: &str) -> &str {
        self.values.get(key).map(String::as_str).unwrap_or("")
    }
}

/// An Exec line as arguments, by the Desktop Entry rules: double quotes group,
/// a backslash escapes inside them, and field codes expand. No file is passed,
/// so the file codes drop out.
fn exec_argv(exec: &str, name: &str, icon: &str, path: &Path) -> Option<Vec<String>> {
    let mut argv = Vec::new();
    for word in exec_words(exec)? {
        match word.as_str() {
            "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => {}
            "%i" if !icon.is_empty() => argv.extend(["--icon".to_string(), icon.to_string()]),
            "%i" => {}
            _ => argv.push(expand(&word, name, path)),
        }
    }
    (!argv.is_empty()).then_some(argv)
}

/// An Exec line split into words with its field codes kept; `None` when a quote is left open.
pub(crate) fn exec_words(exec: &str) -> Option<Vec<String>> {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    let (mut quoted, mut in_word, mut chars) = (false, false, exec.chars());
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                quoted = !quoted;
                in_word = true;
            }
            '\\' if quoted => current.push(chars.next()?),
            ch if ch.is_whitespace() && !quoted => {
                if in_word {
                    words.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            ch => {
                current.push(ch);
                in_word = true;
            }
        }
    }
    if quoted {
        return None;
    }
    if in_word {
        words.push(current);
    }
    Some(words)
}

/// `%c` is the name, `%k` the launcher's own path, `%%` a percent sign.
fn expand(word: &str, name: &str, path: &Path) -> String {
    let mut out = String::new();
    let mut chars = word.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('c') => out.push_str(name),
            Some('k') => out.push_str(&path.to_string_lossy()),
            _ => {}
        }
    }
    out
}

/// A folder a test can make launchers in.
#[cfg(test)]
pub fn scratch(name: &str) -> std::path::PathBuf {
    let folder = std::env::temp_dir().join(format!("boxbox-launch-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_terminal_launcher_opens_in_the_users_terminal() {
        let argv = ["htop".to_string(), "-d".into(), "10".into()];
        let run = |variable: Option<&str>, installed: &[&str]| {
            let command = in_terminal_with(&argv, variable.map(str::to_string), &|name| installed.contains(&name))?;
            let mut line = vec![command.get_program().to_string_lossy().into_owned()];
            line.extend(command.get_args().map(|arg| arg.to_string_lossy().into_owned()));
            Some(line.join(" "))
        };
        assert_eq!(run(Some("kitty"), &["xdg-terminal-exec", "xterm"]).as_deref(), Some("xdg-terminal-exec htop -d 10"), "the default terminal first");
        assert_eq!(run(Some("/usr/bin/wezterm"), &["xterm"]).as_deref(), Some("/usr/bin/wezterm start -- htop -d 10"), "$TERMINAL with its own option");
        assert_eq!(run(Some("myterm"), &[]).as_deref(), Some("myterm -e htop -d 10"));
        assert_eq!(run(None, &["xterm", "kitty"]).as_deref(), Some("kitty htop -d 10"));
        assert_eq!(run(Some(" "), &[]), None, "nothing to run it in");
    }

    #[test]
    fn exec_lines_split_and_expand_like_the_spec() {
        let path = Path::new("/apps/x.desktop");
        assert_eq!(exec_argv("app --name=%c %U", "My App", "", path).unwrap(), ["app", "--name=My App"]);
        assert_eq!(exec_argv(r#""/opt/my app/run" "say \"hi\"" %i %k"#, "", "logo", path).unwrap(), ["/opt/my app/run", "say \"hi\"", "--icon", "logo", "/apps/x.desktop"]);
        assert_eq!(exec_argv("app 100%%", "", "", path).unwrap(), ["app", "100%"]);
        assert!(exec_argv("app \"open", "", "", path).is_none());
    }

    #[test]
    fn only_executable_files_run_and_launchers_need_trust() {
        let folder = scratch("classify");
        let write = |name: &str, text: &str, mode: u32| {
            let path = folder.join(name);
            std::fs::write(&path, text).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            path
        };
        assert_eq!(classify(&write("go.sh", "#!/bin/sh\n", 0o755)), Runnable::Program { script: true });
        assert_eq!(classify(&write("read.sh", "#!/bin/sh\n", 0o644)), Runnable::Document);
        assert_eq!(classify(&write("notes.txt", "hello", 0o755)), Runnable::Document);
        let launcher = write("tool.desktop", "[Desktop Entry]\nType=Application\nName=Tool\nExec=true\n", 0o644);
        assert_eq!(classify(&launcher), Runnable::Launcher { name: "Tool".into(), trusted: false });
        assert!(launch(&launcher).is_err(), "an untrusted launcher does not start");
        trust(&launcher).unwrap();
        assert_eq!(classify(&launcher), Runnable::Launcher { name: "Tool".into(), trusted: true });
        launch(&launcher).unwrap();
        std::fs::remove_dir_all(folder).unwrap();
    }
}
