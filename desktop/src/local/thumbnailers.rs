//! The system's thumbnailers: `thumbnailers/*.thumbnailer` entries in the XDG
//! data dirs (the freedesktop thumbnailer spec), which GNOME, Cinnamon and MATE
//! file managers run for videos, documents and other formats.

use std::os::unix::fs::DirBuilderExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context as _};

/// External thumbnailers at once; each may decode a whole video frame.
const MAX_RUNNING: usize = 3;
const TIME_LIMIT: Duration = Duration::from_secs(10);
const CPU_SECONDS: libc::rlim_t = 10;

pub enum Failure {
    /// The thumbnailer could not make one; the file is marked so it is not retried.
    Failed(anyhow::Error),
    /// Out of time, perhaps only because the system was busy; tried again later.
    TimedOut,
}

impl From<anyhow::Error> for Failure {
    fn from(error: anyhow::Error) -> Self {
        Failure::Failed(error)
    }
}

pub struct Thumbnailer {
    exec: Vec<String>,
    mime_types: Vec<String>,
}

/// The installed thumbnailers. An entry in an earlier data dir hides one of the
/// same file name in a later one.
fn installed() -> &'static [Thumbnailer] {
    static INSTALLED: OnceLock<Vec<Thumbnailer>> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let mut seen = std::collections::HashSet::new();
        let mut found = Vec::new();
        for dir in super::mime::data_dirs() {
            let Ok(entries) = std::fs::read_dir(dir.join("thumbnailers")) else { continue };
            let mut paths: Vec<PathBuf> = entries.filter_map(|entry| Some(entry.ok()?.path())).filter(|path| path.extension().is_some_and(|ext| ext == "thumbnailer")).collect();
            paths.sort();
            for path in paths {
                let Some(name) = path.file_name().map(|name| name.to_os_string()) else { continue };
                if !seen.insert(name) {
                    continue;
                }
                match std::fs::read_to_string(&path).map_err(anyhow::Error::from).and_then(|text| parse(&text)) {
                    Ok(Some(thumbnailer)) => found.push(thumbnailer),
                    Ok(None) => {}
                    Err(error) => log::info!("Skipping thumbnailer {}: {error:#}", path.display()),
                }
            }
        }
        found
    })
}

/// The `[Thumbnailer Entry]` group, or None when its `TryExec` program is missing.
fn parse(text: &str) -> anyhow::Result<Option<Thumbnailer>> {
    let mut in_entry = false;
    let (mut exec, mut try_exec, mut mime_types) = (None, None, Vec::new());
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Thumbnailer Entry]";
            continue;
        }
        let Some((key, value)) = line.split_once('=').filter(|_| in_entry) else { continue };
        match key.trim() {
            "Exec" => exec = Some(value.trim().to_string()),
            "TryExec" => try_exec = Some(value.trim().to_string()),
            "MimeType" => mime_types = value.split(';').map(str::trim).filter(|mime| !mime.is_empty()).map(|mime| super::mime::canonical(mime).to_string()).collect(),
            _ => {}
        }
    }
    if try_exec.is_some_and(|program| !on_path(&program)) {
        return Ok(None);
    }
    let exec = split_exec(&exec.context("no Exec line")?)?;
    anyhow::ensure!(!exec.is_empty(), "an empty Exec line");
    Ok(Some(Thumbnailer { exec, mime_types }))
}

fn on_path(program: &str) -> bool {
    let executable = |path: &Path| std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o111 != 0);
    if program.contains('/') {
        return executable(Path::new(program));
    }
    std::env::var_os("PATH").is_some_and(|paths| std::env::split_paths(&paths).any(|dir| executable(&dir.join(program))))
}

/// Desktop Entry `Exec` quoting: arguments split on spaces, double quotes group
/// one, and inside them a backslash escapes `"`, `` ` ``, `$` and `\`.
fn split_exec(line: &str) -> anyhow::Result<Vec<String>> {
    let mut args = Vec::new();
    let mut current: Option<String> = None;
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\t' => {
                if let Some(arg) = current.take() {
                    args.push(arg);
                }
            }
            '"' => {
                let arg = current.get_or_insert_with(String::new);
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => arg.push(chars.next().ok_or_else(|| anyhow!("a quote ends with a backslash"))?),
                        Some(ch) => arg.push(ch),
                        None => anyhow::bail!("an unclosed quote"),
                    }
                }
            }
            ch => current.get_or_insert_with(String::new).push(ch),
        }
    }
    args.extend(current);
    Ok(args)
}

/// The field codes, inside any argument: `%i` path, `%u` URI, `%o` output, `%s` size.
fn expand(arg: &str, input: &Path, uri: &str, output: &Path, size: u32) -> String {
    let mut expanded = String::new();
    let mut chars = arg.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            expanded.push(ch);
            continue;
        }
        match chars.next() {
            Some('i') => expanded.push_str(&input.to_string_lossy()),
            Some('u') => expanded.push_str(uri),
            Some('o') => expanded.push_str(&output.to_string_lossy()),
            Some('s') => expanded.push_str(&size.to_string()),
            Some('%') => expanded.push('%'),
            _ => {}
        }
    }
    expanded
}

/// The first thumbnailer that takes this type.
pub fn find(mime: &str) -> Option<&'static Thumbnailer> {
    installed().iter().find(|thumbnailer| thumbnailer.mime_types.iter().any(|candidate| candidate == mime))
}

/// Holds one of the `MAX_RUNNING` places until dropped.
struct Slot;

static RUNNING: Mutex<usize> = Mutex::new(0);
static FREED: Condvar = Condvar::new();

impl Slot {
    fn take() -> Self {
        let mut running = RUNNING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        while *running >= MAX_RUNNING {
            running = FREED.wait(running).unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        *running += 1;
        Slot
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        *RUNNING.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) -= 1;
        FREED.notify_one();
    }
}

/// A private folder for the output, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> anyhow::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("boxbox-thumbnail-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::DirBuilder::new().mode(0o700).create(&path).with_context(|| format!("cannot create {}", path.display()))?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl Thumbnailer {
    /// The image the thumbnailer writes for `input`, in whatever format it chose.
    pub fn run(&self, input: &Path, uri: &str, size: u32) -> Result<Vec<u8>, Failure> {
        let _slot = Slot::take();
        let scratch = Scratch::new()?;
        let output = scratch.0.join("thumbnail.png");
        let args: Vec<String> = self.exec.iter().map(|arg| expand(arg, input, uri, &output, size)).collect();
        let mut command = Command::new(&args[0]);
        command.args(&args[1..]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0);
        // SAFETY: setrlimit and setpriority are async-signal-safe and touch only the child.
        unsafe {
            command.pre_exec(|| {
                let limit = libc::rlimit { rlim_cur: CPU_SECONDS, rlim_max: CPU_SECONDS };
                libc::setrlimit(libc::RLIMIT_CPU, &limit);
                libc::setpriority(libc::PRIO_PROCESS, 0, 10);
                Ok(())
            });
        }
        let mut child = command.spawn().with_context(|| format!("cannot start {}", args[0]))?;
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().context("cannot wait for the thumbnailer")? {
                break status;
            }
            if started.elapsed() > TIME_LIMIT {
                // The whole group, so a wrapper script's children go too.
                // SAFETY: the child leads its own process group.
                unsafe { libc::killpg(child.id() as libc::pid_t, libc::SIGKILL) };
                let _ = child.wait();
                return Err(Failure::TimedOut);
            }
            std::thread::sleep(Duration::from_millis(15));
        };
        if !status.success() {
            return Err(anyhow!("{} exited with {status}", args[0]).into());
        }
        let bytes = std::fs::read(&output).with_context(|| format!("{} wrote no thumbnail", args[0]))?;
        if bytes.is_empty() {
            return Err(anyhow!("{} wrote an empty thumbnail", args[0]).into());
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_lines_split_like_desktop_entries() {
        assert_eq!(split_exec("ffmpegthumbnailer -i %i -o %o -s %s -f").unwrap(), ["ffmpegthumbnailer", "-i", "%i", "-o", "%o", "-s", "%s", "-f"]);
        assert_eq!(split_exec(r#"sh -c "echo \"a b\" \$x" --size=%s"#).unwrap(), ["sh", "-c", r#"echo "a b" $x"#, "--size=%s"]);
        assert!(split_exec(r#"broken "quote"#).is_err());
    }

    #[test]
    fn field_codes_expand_inside_arguments() {
        let (input, output) = (Path::new("/v/a b.mp4"), Path::new("/t/out.png"));
        assert_eq!(expand("--size=%s", input, "file:///v/a%20b.mp4", output, 256), "--size=256");
        assert_eq!(expand("%i", input, "", output, 0), "/v/a b.mp4");
        assert_eq!(expand("%u", input, "file:///v/a%20b.mp4", output, 0), "file:///v/a%20b.mp4");
        assert_eq!(expand("100%%", input, "", output, 0), "100%");
    }

    #[test]
    fn entries_without_their_program_are_skipped() {
        let entry = "[Thumbnailer Entry]\nTryExec=/nonexistent/thumbnailer\nExec=x %i %o\nMimeType=video/mp4;\n";
        assert!(parse(entry).unwrap().is_none());
        let entry = "[Other]\nExec=ignored\n[Thumbnailer Entry]\nExec=sh %i %o\nMimeType=video/mp4;video/webm;\n";
        let thumbnailer = parse(entry).unwrap().unwrap();
        assert_eq!(thumbnailer.exec, ["sh", "%i", "%o"]);
        assert_eq!(thumbnailer.mime_types, ["video/mp4", "video/webm"]);
    }

    #[test]
    fn a_thumbnailer_runs_with_its_field_codes() {
        let thumbnailer = Thumbnailer { exec: split_exec(r#"sh -c "printf %s > \"$1\"" sh %o"#).unwrap(), mime_types: Vec::new() };
        let bytes = thumbnailer.run(Path::new("/dev/null"), "file:///dev/null", 128).ok().unwrap();
        assert_eq!(bytes, b"128");
        let failing = Thumbnailer { exec: vec!["false".into()], mime_types: Vec::new() };
        assert!(matches!(failing.run(Path::new("/dev/null"), "", 128), Err(Failure::Failed(_))));
    }
}
