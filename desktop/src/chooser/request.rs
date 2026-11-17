//! One portal file chooser request: what the app asked for, and how a choice
//! becomes the answer. Parsing trusts nothing; acceptance checks the disk again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context as _, Result};
use zbus::zvariant::{OwnedValue, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Open,
    Save,
    /// Several named files into one chosen folder.
    SaveFiles,
}

/// A rule of a filter: a shell glob, or a MIME type such as `image/*`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rule {
    Glob(String),
    Mime(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub name: String,
    pub rules: Vec<Rule>,
}

/// A choice the app adds to the dialog: a list of options, or a check box
/// when it has none (its value is then `"true"` or `"false"`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub id: String,
    pub label: String,
    pub options: Vec<(String, String)>,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct Request {
    pub kind: Kind,
    pub app_id: String,
    pub title: String,
    pub accept_label: String,
    pub multiple: bool,
    /// Folders are chosen, not files.
    pub directory: bool,
    /// Where to start; a hint, never a grant.
    pub folder: Option<PathBuf>,
    pub name: String,
    /// The names a SaveFiles request writes.
    pub files: Vec<String>,
    pub filters: Vec<Filter>,
    pub filter: Option<usize>,
    pub choices: Vec<Choice>,
}

/// The portal's reply: 0 chose, 1 cancelled, 2 failed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Response {
    pub code: u32,
    pub uris: Vec<String>,
    pub filter: Option<Filter>,
    pub choices: Vec<(String, String)>,
}

impl Response {
    pub fn cancelled() -> Self {
        Self { code: 1, ..Self::default() }
    }

    /// The `results` vardict of the reply.
    pub fn results(&self) -> HashMap<String, OwnedValue> {
        let mut results = HashMap::new();
        if self.code != 0 {
            return results;
        }
        let mut put = |key: &str, value: Value<'_>| {
            if let Ok(value) = value.try_to_owned() {
                results.insert(key.to_string(), value);
            }
        };
        put("uris", Value::from(self.uris.clone()));
        put("choices", Value::from(self.choices.clone()));
        if let Some(filter) = &self.filter {
            put("current_filter", Value::from(wire_filter(filter)));
        }
        results
    }
}

type WireFilter = (String, Vec<(u32, String)>);
type WireChoice = (String, String, Vec<(String, String)>, String);

fn wire_filter(filter: &Filter) -> WireFilter {
    let rules = filter
        .rules
        .iter()
        .map(|rule| match rule {
            Rule::Glob(pattern) => (0, pattern.clone()),
            Rule::Mime(mime) => (1, mime.clone()),
        })
        .collect();
    (filter.name.clone(), rules)
}

fn filter_from_wire((name, rules): WireFilter) -> Result<Filter> {
    ensure!(rules.len() <= 100, "too many filter rules");
    let rules = rules
        .into_iter()
        .map(|(kind, value)| {
            ensure!(value.len() <= 1024, "filter rule too long");
            match kind {
                0 => Ok(Rule::Glob(value)),
                1 => Ok(Rule::Mime(value)),
                _ => bail!("invalid filter rule"),
            }
        })
        .collect::<Result<_>>()?;
    Ok(Filter { name, rules })
}

fn option<T: TryFrom<OwnedValue>>(options: &HashMap<String, OwnedValue>, key: &str) -> Result<Option<T>> {
    match options.get(key) {
        None => Ok(None),
        Some(value) => {
            let value = value.try_clone().with_context(|| format!("invalid {key}"))?;
            T::try_from(value).map(Some).map_err(|_| anyhow::anyhow!("invalid {key}"))
        }
    }
}

/// A byte string with its trailing NUL, as portals send paths.
fn nul_terminated(bytes: Vec<u8>, what: &str) -> Result<Option<String>> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let Some((&0, text)) = bytes.split_last() else { bail!("{what} must end in NUL") };
    ensure!(!text.contains(&0), "{what} holds a NUL");
    let text = String::from_utf8(text.to_vec()).with_context(|| format!("{what} is not UTF-8"))?;
    Ok((!text.is_empty()).then_some(text))
}

/// A name for one file in a folder: no slash, not `.` or `..`.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\0'])
}

impl Request {
    /// Reads an `OpenFile`, `SaveFile` or `SaveFiles` call.
    pub fn parse(kind: Kind, app_id: &str, title: &str, options: &HashMap<String, OwnedValue>) -> Result<Self> {
        let accept: Option<String> = option(options, "accept_label")?;
        // GTK marks a mnemonic with `_`; this dialog has none.
        let accept_label = accept.filter(|label| !label.is_empty()).map(|label| label.replace('_', "")).unwrap_or_else(|| {
            match kind {
                Kind::Open => "Open",
                Kind::Save | Kind::SaveFiles => "Save",
            }
            .to_string()
        });
        let multiple = kind == Kind::Open && option::<bool>(options, "multiple")?.unwrap_or(false);
        let directory = match kind {
            Kind::Open => option::<bool>(options, "directory")?.unwrap_or(false),
            Kind::Save => false,
            Kind::SaveFiles => true,
        };
        let mut filters = option::<Vec<WireFilter>>(options, "filters")?.unwrap_or_default().into_iter().map(filter_from_wire).collect::<Result<Vec<_>>>()?;
        ensure!(filters.len() <= 100, "too many filters");
        let filter = match option::<WireFilter>(options, "current_filter")? {
            None => None,
            Some(current) => {
                let current = filter_from_wire(current)?;
                Some(match filters.iter().position(|filter| *filter == current) {
                    Some(index) => index,
                    None => {
                        filters.push(current);
                        filters.len() - 1
                    }
                })
            }
        };
        let filter = filter.or((!filters.is_empty()).then_some(0));
        let mut folder = option::<Vec<u8>>(options, "current_folder")?.map(|bytes| nul_terminated(bytes, "current_folder")).transpose()?.flatten().map(PathBuf::from);
        let mut name = String::new();
        if kind == Kind::Save {
            if let Some(file) = option::<Vec<u8>>(options, "current_file")?.map(|bytes| nul_terminated(bytes, "current_file")).transpose()?.flatten() {
                let file = PathBuf::from(file);
                folder = file.parent().map(Path::to_path_buf);
                name = file.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            }
        }
        if let Some(current) = option::<String>(options, "current_name")? {
            name = current;
        }
        ensure!(name.is_empty() || valid_name(&name), "invalid suggested file name");
        let folder = folder.filter(|folder| folder.is_absolute() && folder.is_dir());
        let mut files = Vec::new();
        if kind == Kind::SaveFiles {
            let names: Vec<Vec<u8>> = option(options, "files")?.unwrap_or_default();
            ensure!(!names.is_empty() && names.len() <= 1024, "invalid number of files to save");
            for bytes in names {
                let file = nul_terminated(bytes, "file name")?.unwrap_or_default();
                ensure!(valid_name(&file) && !files.contains(&file), "invalid or repeated file name");
                files.push(file);
            }
        }
        let wire_choices: Vec<WireChoice> = option(options, "choices")?.unwrap_or_default();
        ensure!(wire_choices.len() <= 100, "too many choices");
        let choices = wire_choices
            .into_iter()
            .map(|(id, label, options, value)| {
                let value = match value {
                    value if !value.is_empty() => value,
                    _ => options.first().map_or_else(|| "false".to_string(), |(id, _)| id.clone()),
                };
                Choice { id, label, options, value }
            })
            .collect();
        Ok(Self {
            kind,
            app_id: app_id.to_string(),
            title: title.to_string(),
            accept_label,
            multiple,
            directory,
            folder,
            name,
            files,
            filters,
            filter,
            choices,
        })
    }

    /// Whether a file shows under the current filter. Folders always show.
    pub fn shows(&self, name: &str, is_dir: bool, filter: Option<usize>) -> bool {
        if is_dir {
            return true;
        }
        if self.directory {
            return false;
        }
        match filter.and_then(|index| self.filters.get(index)) {
            Some(filter) => matches(filter, name),
            None => true,
        }
    }
}

/// What the user picked, before it is checked against the disk.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub paths: Vec<PathBuf>,
    pub filter: Option<usize>,
    pub choices: Vec<(String, String)>,
    /// The user saw that the target exists and chose Replace.
    pub replace: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Accept(Response),
    /// The file exists; the user must agree to replace it first.
    AskReplace(String),
}

/// Checks a choice against the request and the disk, and builds the reply.
pub fn accept(request: &Request, selection: &Selection) -> Result<Verdict> {
    ensure!(!selection.paths.is_empty() && selection.paths.len() <= 1024, "Nothing is selected");
    ensure!(request.multiple || selection.paths.len() == 1, "Choose one item");
    let filter = match selection.filter {
        Some(index) => Some(request.filters.get(index).context("Unknown file type filter")?.clone()),
        None => None,
    };
    let mut choices = Vec::with_capacity(request.choices.len());
    for choice in &request.choices {
        let value = selection.choices.iter().find(|(id, _)| *id == choice.id).map_or(choice.value.as_str(), |(_, value)| value.as_str());
        let valid = if choice.options.is_empty() { value == "true" || value == "false" } else { choice.options.iter().any(|(id, _)| id == value) };
        ensure!(valid, "Invalid value for “{}”", choice.label);
        choices.push((choice.id.clone(), value.to_string()));
    }
    let targets: Vec<PathBuf> = match request.kind {
        Kind::SaveFiles => {
            let [folder] = selection.paths.as_slice() else { bail!("Choose one folder") };
            ensure!(folder.is_dir(), "{} is not a folder", folder.display());
            request.files.iter().map(|name| folder.join(name)).collect()
        }
        _ => selection.paths.clone(),
    };
    let mut uris = Vec::with_capacity(targets.len());
    for path in &targets {
        ensure!(path.is_absolute(), "Choose a full path");
        match request.kind {
            Kind::Open => {
                let metadata = std::fs::metadata(path).with_context(|| format!("Cannot open {}", path.display()))?;
                ensure!(metadata.is_dir() == request.directory, "{}", if request.directory { "Choose a folder" } else { "Choose a file, not a folder" });
                if !metadata.is_dir() {
                    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                    if let Some(filter) = &filter {
                        ensure!(matches(filter, &name), "{name} is not a {}", filter.name);
                    }
                }
            }
            Kind::Save | Kind::SaveFiles => {
                let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                ensure!(valid_name(&name), "Enter a file name without a slash");
                let parent = path.parent().context("Choose a folder")?;
                ensure!(parent.is_dir(), "{} is not a folder", parent.display());
                match std::fs::symlink_metadata(path) {
                    Ok(metadata) if metadata.is_dir() => bail!("{name} is a folder"),
                    Ok(_) if !selection.replace => {
                        return Ok(Verdict::AskReplace(if targets.len() == 1 {
                            format!("“{name}” already exists. Replace it?")
                        } else {
                            format!("“{name}” and perhaps others already exist here. Replace them?")
                        }))
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error).with_context(|| format!("Cannot check {name}")),
                }
            }
        }
        uris.push(crate::os_clipboard::file_uri(path));
    }
    Ok(Verdict::Accept(Response { code: 0, uris, filter, choices }))
}

/// Whether a file name passes a filter. A type rule uses the name's type and
/// the types it is a kind of; a name with no known type passes, as GTK shows it.
pub fn matches(filter: &Filter, name: &str) -> bool {
    if filter.rules.is_empty() {
        return true;
    }
    let mime = std::sync::OnceLock::new();
    filter.rules.iter().any(|rule| match rule {
        Rule::Glob(pattern) => glob(pattern, name),
        Rule::Mime(wanted) => match mime.get_or_init(|| crate::local::mime::from_name(Path::new(name))) {
            None => true,
            Some(mime) => crate::local::mime::is_a(mime, wanted),
        },
    })
}

/// Shell globbing as GTK does it: `*`, `?`, `[...]` with `!` or `^`, and `\`
/// escapes. Case-insensitive, as GTK's filters match.
pub fn glob(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let name: Vec<char> = name.to_lowercase().chars().collect();
    glob_at(&pattern, &name)
}

fn glob_at(pattern: &[char], name: &[char]) -> bool {
    let Some((&first, rest)) = pattern.split_first() else { return name.is_empty() };
    match first {
        '*' => (0..=name.len()).any(|skip| glob_at(rest, &name[skip..])),
        '?' => !name.is_empty() && glob_at(rest, &name[1..]),
        '[' => {
            let Some(&c) = name.first() else { return false };
            let negated = matches!(rest.first(), Some('!' | '^'));
            let start = usize::from(negated);
            // A `]` first in the class is a member, not its end.
            let Some(end) = rest.iter().enumerate().skip(start + 1).find(|(_, &c)| c == ']').map(|(index, _)| index) else {
                return c == '[' && glob_at(rest, &name[1..]);
            };
            let class = &rest[start..end];
            let mut found = false;
            let mut index = 0;
            while index < class.len() {
                if index + 2 < class.len() && class[index + 1] == '-' {
                    found |= class[index] <= c && c <= class[index + 2];
                    index += 3;
                } else {
                    found |= class[index] == c;
                    index += 1;
                }
            }
            found != negated && glob_at(&rest[end + 1..], &name[1..])
        }
        '\\' if !rest.is_empty() => name.first() == Some(&rest[0]) && glob_at(&rest[1..], &name[1..]),
        literal => name.first() == Some(&literal) && glob_at(rest, &name[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(pairs: Vec<(&str, Value<'_>)>) -> HashMap<String, OwnedValue> {
        pairs.into_iter().map(|(key, value)| (key.to_string(), value.try_to_owned().unwrap())).collect()
    }

    fn nul(text: &str) -> Vec<u8> {
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        bytes
    }

    #[test]
    fn globs_match_as_gtk_does() {
        assert!(glob("*.png", "Photo.PNG"));
        assert!(glob("report-??.txt", "report-01.txt"));
        assert!(!glob("report-??.txt", "report-1.txt"));
        assert!(glob("[a-c]*", "beta"));
        assert!(!glob("[!a-c]*", "beta"));
        assert!(glob("\\*", "*"));
        assert!(!glob("\\*", "a"));
        assert!(glob("[]]", "]"));
    }

    #[test]
    fn a_save_request_reads_its_folder_name_and_filters() {
        let dir = std::env::temp_dir();
        let images: WireFilter = ("Images".into(), vec![(1, "image/*".into())]);
        let text: WireFilter = ("Text".into(), vec![(0, "*.txt".into())]);
        let request = Request::parse(
            Kind::Save,
            "org.example.App",
            "Save As",
            &options(vec![
                ("accept_label", Value::from("_Save")),
                ("current_name", Value::from("photo.png")),
                ("current_folder", Value::from(nul(&dir.display().to_string()))),
                ("filters", Value::from(vec![images.clone(), text.clone()])),
                ("current_filter", Value::from(text)),
                ("choices", Value::from(vec![("q".to_string(), "Quality".to_string(), vec![("hi".to_string(), "High".to_string())], String::new())])),
            ]),
        )
        .unwrap();
        assert_eq!(request.accept_label, "Save");
        assert_eq!(request.name, "photo.png");
        assert_eq!(request.folder.as_deref(), Some(dir.as_path()));
        assert_eq!(request.filter, Some(1), "the current filter is one of the list");
        assert_eq!(request.filters.len(), 2);
        assert_eq!(request.choices[0].value, "hi", "a choice without a value takes its first option");
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(Request::parse(Kind::Save, "", "", &options(vec![("current_name", Value::from("a/b"))])).is_err());
        assert!(Request::parse(Kind::Open, "", "", &options(vec![("current_folder", Value::from(b"/tmp".to_vec()))])).is_err(), "paths end in NUL");
        assert!(Request::parse(Kind::SaveFiles, "", "", &options(vec![])).is_err(), "SaveFiles names its files");
        assert!(Request::parse(Kind::SaveFiles, "", "", &options(vec![("files", Value::from(vec![nul("a"), nul("a")]))])).is_err());
        assert!(Request::parse(Kind::Open, "", "", &options(vec![("multiple", Value::from("yes"))])).is_err());
    }

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn dir(files: &[&str]) -> Dir {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!("boxbox-chooser-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        for file in files {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            if file.ends_with('/') {
                std::fs::create_dir_all(&path).unwrap();
            } else {
                std::fs::write(&path, "x").unwrap();
            }
        }
        std::fs::create_dir_all(&root).unwrap();
        Dir(root)
    }

    fn open_request(directory: bool, multiple: bool) -> Request {
        let mut request = Request::parse(Kind::Open, "", "", &HashMap::new()).unwrap();
        request.directory = directory;
        request.multiple = multiple;
        request
    }

    #[test]
    fn open_returns_uris_and_checks_kind_and_filter() {
        let root = dir(&["a b.txt", "c.png", "sub/"]);
        let mut request = open_request(false, true);
        request.filters = vec![Filter { name: "Text".into(), rules: vec![Rule::Glob("*.txt".into())] }];
        let pick = |paths: &[&str], filter| Selection { paths: paths.iter().map(|name| root.0.join(name)).collect(), filter, ..Selection::default() };
        let Verdict::Accept(response) = accept(&request, &pick(&["a b.txt"], Some(0))).unwrap() else { panic!() };
        assert_eq!(response.uris, vec![crate::os_clipboard::file_uri(&root.0.join("a b.txt"))]);
        assert!(response.uris[0].ends_with("a%20b.txt"));
        assert_eq!(response.filter.unwrap().name, "Text");
        assert!(accept(&request, &pick(&["c.png"], Some(0))).is_err(), "outside the filter");
        assert!(accept(&request, &pick(&["sub"], None)).is_err(), "a folder where a file was asked");
        assert!(accept(&open_request(true, false), &pick(&["sub"], None)).is_ok());
        assert!(accept(&open_request(false, false), &pick(&["a b.txt", "c.png"], None)).is_err(), "one only");
    }

    #[test]
    fn save_asks_before_replacing() {
        let root = dir(&["taken.txt", "sub/"]);
        let request = Request::parse(Kind::Save, "", "", &HashMap::new()).unwrap();
        let pick = |name: &str, replace| Selection { paths: vec![root.0.join(name)], replace, ..Selection::default() };
        assert!(matches!(accept(&request, &pick("new.txt", false)).unwrap(), Verdict::Accept(_)));
        assert!(matches!(accept(&request, &pick("taken.txt", false)).unwrap(), Verdict::AskReplace(_)));
        assert!(matches!(accept(&request, &pick("taken.txt", true)).unwrap(), Verdict::Accept(_)));
        assert!(accept(&request, &pick("sub", true)).is_err(), "a folder is never replaced");
    }

    #[test]
    fn save_files_names_each_file_in_the_folder() {
        let root = dir(&["a.txt"]);
        let mut request = Request::parse(Kind::SaveFiles, "", "", &options(vec![("files", Value::from(vec![nul("b.txt"), nul("c.txt")]))])).unwrap();
        let pick = Selection { paths: vec![root.0.clone()], ..Selection::default() };
        let Verdict::Accept(response) = accept(&request, &pick).unwrap() else { panic!() };
        assert_eq!(response.uris.len(), 2);
        request.files.push("a.txt".into());
        assert!(matches!(accept(&request, &pick).unwrap(), Verdict::AskReplace(_)));
    }

    #[test]
    fn choices_are_checked_and_returned() {
        let mut request = open_request(false, false);
        request.choices = vec![Choice { id: "ro".into(), label: "Read only".into(), options: vec![], value: "false".into() }];
        let root = dir(&["a.txt"]);
        let pick = |value: &str| Selection { paths: vec![root.0.join("a.txt")], choices: vec![("ro".into(), value.into())], ..Selection::default() };
        let Verdict::Accept(response) = accept(&request, &pick("true")).unwrap() else { panic!() };
        assert_eq!(response.choices, vec![("ro".to_string(), "true".to_string())]);
        assert!(accept(&request, &pick("maybe")).is_err());
        assert!(response.results().contains_key("uris"));
        assert!(Response::cancelled().results().is_empty());
    }
}
