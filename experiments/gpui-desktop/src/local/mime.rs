//! A file's MIME type from its name, by the shared-mime-info glob tables
//! (`mime/globs2` and `mime/aliases` in each XDG data dir), the rules GIO
//! applies before it sniffs content.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

struct Glob {
    weight: u32,
    pattern: String,
    mime: String,
    case_sensitive: bool,
}

#[derive(Default)]
struct Database {
    globs: Vec<Glob>,
    aliases: HashMap<String, String>,
    /// `mime/icons` and `mime/generic-icons`: a type's own icon name and its family's.
    icons: HashMap<String, String>,
    generic_icons: HashMap<String, String>,
}

/// `$XDG_DATA_HOME`, then `$XDG_DATA_DIRS`: the user's files win.
pub fn data_dirs() -> Vec<PathBuf> {
    let absolute = |path: &str| Some(PathBuf::from(path)).filter(|path| path.is_absolute());
    let home = std::env::var("XDG_DATA_HOME")
        .ok()
        .and_then(|path| absolute(&path))
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/share")));
    let system = std::env::var("XDG_DATA_DIRS").ok().filter(|dirs| !dirs.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    home.into_iter().chain(system.split(':').filter_map(absolute)).collect()
}

fn database() -> &'static Database {
    static DATABASE: OnceLock<Database> = OnceLock::new();
    DATABASE.get_or_init(|| {
        let mut database = Database::default();
        for dir in data_dirs() {
            if let Ok(text) = std::fs::read_to_string(dir.join("mime/globs2")) {
                database.globs.extend(parse_globs(&text));
            }
            if let Ok(text) = std::fs::read_to_string(dir.join("mime/aliases")) {
                for (alias, canonical) in text.lines().filter_map(|line| line.split_once(' ')) {
                    database.aliases.entry(alias.to_string()).or_insert_with(|| canonical.trim().to_string());
                }
            }
            for (file, table) in [("mime/icons", &mut database.icons), ("mime/generic-icons", &mut database.generic_icons)] {
                if let Ok(text) = std::fs::read_to_string(dir.join(file)) {
                    for (mime, icon) in text.lines().filter_map(|line| line.split_once(':')) {
                        table.entry(mime.to_string()).or_insert_with(|| icon.trim().to_string());
                    }
                }
            }
        }
        database
    })
}

/// `weight:type:glob[:flags]` lines.
fn parse_globs(text: &str) -> impl Iterator<Item = Glob> + '_ {
    text.lines().filter(|line| !line.starts_with('#')).filter_map(|line| {
        let mut fields = line.splitn(4, ':');
        let weight = fields.next()?.parse().ok()?;
        let mime = fields.next()?;
        let pattern = fields.next()?;
        let case_sensitive = fields.next().is_some_and(|flags| flags.split(',').any(|flag| flag == "cs"));
        (pattern != "__NOGLOBS__").then(|| Glob {
            weight,
            pattern: if case_sensitive { pattern.to_string() } else { pattern.to_lowercase() },
            mime: mime.to_string(),
            case_sensitive,
        })
    })
}

/// `*` and `?` only; the tables use brackets rarely, and such names fall through.
fn matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern.len() == name.len() && pattern.chars().zip(name.chars()).all(|(p, n)| p == '?' || p == n),
        Some((head, tail)) if !tail.contains('*') => {
            // `get` refuses a cut inside a multi-byte character.
            name.len() >= head.len() + tail.len()
                && name.get(..head.len()).is_some_and(|start| matches(head, start))
                && name.get(name.len() - tail.len()..).is_some_and(|end| matches(tail, end))
        }
        Some(_) => false,
    }
}

fn lookup(database: &Database, name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    database
        .globs
        .iter()
        .filter(|glob| matches(&glob.pattern, if glob.case_sensitive { name } else { &lower }))
        // The highest weight, then an exact-case match, then the longest pattern.
        .max_by_key(|glob| (glob.weight, glob.case_sensitive, glob.pattern.len()))
        .map(|glob| canonical_in(database, &glob.mime).to_string())
}

fn canonical_in<'a>(database: &'a Database, mime: &'a str) -> &'a str {
    database.aliases.get(mime).map_or(mime, String::as_str)
}

/// The type a file's name gives it, if any glob matches.
pub fn from_name(path: &Path) -> Option<String> {
    lookup(database(), &path.file_name()?.to_string_lossy())
}

/// The icon names for a type, best first, as GIO lists them: the type's own
/// (`text-x-python`), its family's (`text-x-script`), then the media's generic one.
pub fn icon_names(mime: &str) -> Vec<String> {
    icon_names_in(database(), mime)
}

fn icon_names_in(database: &Database, mime: &str) -> Vec<String> {
    let mime = canonical_in(database, mime);
    let media = mime.split('/').next().unwrap_or("application");
    let mut names = vec![database.icons.get(mime).cloned().unwrap_or_else(|| mime.replace('/', "-"))];
    names.extend(database.generic_icons.get(mime).cloned());
    names.push(format!("{media}-x-generic"));
    names.dedup();
    names
}

/// The canonical name of a type that may be an alias, such as `video/x-matroska`.
pub fn canonical(mime: &str) -> &str {
    canonical_in(database(), mime)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Database {
        let globs = "# comment\n50:video/mp4:*.mp4\n50:video/matroska:*.mkv\n10:text/x-makefile:makefile\n50:text/x-c++src:*.C:cs\n50:text/x-csrc:*.c\n60:application/x-compressed-tar:*.tar.gz\n50:application/gzip:*.gz\n0:model/x:__NOGLOBS__\n";
        Database {
            globs: parse_globs(globs).collect(),
            aliases: [("video/x-matroska".to_string(), "video/matroska".to_string())].into_iter().collect(),
            icons: HashMap::new(),
            generic_icons: [("text/x-python".to_string(), "text-x-script".to_string())].into_iter().collect(),
        }
    }

    #[test]
    fn names_take_the_heaviest_longest_glob() {
        let database = database();
        assert_eq!(lookup(&database, "Clip.MP4").as_deref(), Some("video/mp4"));
        assert_eq!(lookup(&database, "a.tar.gz").as_deref(), Some("application/x-compressed-tar"));
        assert_eq!(lookup(&database, "b.gz").as_deref(), Some("application/gzip"));
        assert_eq!(lookup(&database, "Makefile").as_deref(), Some("text/x-makefile"));
        assert_eq!(lookup(&database, "notes").as_deref(), None);
        assert_eq!(lookup(&database, "vidéo.mp4").as_deref(), Some("video/mp4"));
        assert_eq!(lookup(&database, "ééé").as_deref(), None);
    }

    #[test]
    fn icon_names_run_from_the_type_to_its_family() {
        let database = database();
        assert_eq!(icon_names_in(&database, "text/x-python"), ["text-x-python", "text-x-script", "text-x-generic"]);
        assert_eq!(icon_names_in(&database, "video/x-matroska"), ["video-matroska", "video-x-generic"]);
    }

    #[test]
    fn case_sensitive_globs_keep_their_case() {
        let database = database();
        assert_eq!(lookup(&database, "main.C").as_deref(), Some("text/x-c++src"));
        assert_eq!(lookup(&database, "main.c").as_deref(), Some("text/x-csrc"));
    }

    #[test]
    fn aliases_resolve_to_their_canonical_type() {
        let database = database();
        assert_eq!(canonical_in(&database, "video/x-matroska"), "video/matroska");
        assert_eq!(canonical_in(&database, "video/mp4"), "video/mp4");
    }
}
