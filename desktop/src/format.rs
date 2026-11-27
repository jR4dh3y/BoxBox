//! Ports of `frontend/src/lib/utils/format.ts` and `fileTypes.ts`.

use std::borrow::Cow;

use chrono::{DateTime, Datelike, Local, NaiveDate};

const SIZE_UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

/// `formatFileSize`: two decimals with trailing zeros trimmed, e.g. "1.5 MB".
pub fn file_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".into();
    }
    let unit = (((63 - bytes.leading_zeros()) / 10) as usize).min(SIZE_UNITS.len() - 1);
    if unit == 0 {
        return format!("{bytes} B");
    }
    let divisor = (1u64 << (unit * 10)) as f64;
    let value = bytes as f64 / divisor;
    let rounded = (value * 100.0).round() / 100.0;
    format!("{rounded} {}", SIZE_UNITS[unit])
}

/// `formatFileDate`: "Today 3:04 PM", "Nov 5", or "Nov 5, 2024".
pub fn file_date(mod_time: Option<i64>) -> String {
    let now = Local::now();
    file_date_at(mod_time, now.date_naive(), now.year())
}

/// `file_date` reusing a pre-fetched local date and year across a frame of rows.
pub fn file_date_at(mod_time: Option<i64>, today: NaiveDate, year: i32) -> String {
    let Some(date) = mod_time.and_then(DateTime::from_timestamp_millis) else {
        return "-".into();
    };
    let date = date.with_timezone(&Local);
    if date.date_naive() == today {
        format!("Today {}", date.format("%-I:%M %p"))
    } else if date.year() == year {
        date.format("%b %-d").to_string()
    } else {
        date.format("%b %-d, %Y").to_string()
    }
}

fn extension_slice(name: &str) -> &str {
    match name.rfind('.') {
        Some(index) if index > 0 => &name[index + 1..],
        _ => "",
    }
}

/// Lowercases short ASCII strings onto the stack to avoid heap allocation on hot paths.
fn lower_ascii_stack<'a>(input: &str, buf: &'a mut [u8; 32]) -> Option<&'a str> {
    if input.len() > buf.len() || !input.is_ascii() {
        return None;
    }
    for (dst, &byte) in buf.iter_mut().zip(input.as_bytes()) {
        *dst = byte.to_ascii_lowercase();
    }
    // SAFETY: ASCII bytes lowercased remain valid UTF-8.
    Some(unsafe { std::str::from_utf8_unchecked(&buf[..input.len()]) })
}

pub fn extension(name: &str) -> String {
    extension_slice(name).to_lowercase()
}

fn is_single_dotfile(name: &str) -> bool {
    name.starts_with('.') && name.len() > 1 && !name[1..].contains('.')
}

fn is_shell_dotfile(lower: &str) -> bool {
    matches!(
        lower,
        ".bashrc"
            | ".bash_profile"
            | ".bash_login"
            | ".bash_logout"
            | ".profile"
            | ".zshrc"
            | ".zprofile"
            | ".zshenv"
            | ".zlogin"
            | ".zlogout"
            | ".kshrc"
            | ".cshrc"
            | ".tcshrc"
    )
}

fn is_config_dotfile(lower: &str) -> bool {
    matches!(
        lower,
        ".env"
            | ".envrc"
            | ".gitconfig"
            | ".gitignore"
            | ".dockerignore"
            | ".editorconfig"
            | ".npmrc"
            | ".yarnrc"
            | ".curlrc"
            | ".wgetrc"
            | ".vimrc"
    )
}

fn is_special_code_filename(lower: &str) -> bool {
    matches!(
        lower,
        "dockerfile" | "makefile" | "gnumakefile" | "cmakelists.txt" | "gemfile" | "rakefile"
    )
}

/// `getFileTypeDescription`.
pub fn type_description(name: &str) -> Cow<'static, str> {
    if name.starts_with('.') {
        let mut buf = [0u8; 32];
        if let Some(lower) = lower_ascii_stack(name, &mut buf) {
            if is_shell_dotfile(lower) {
                return Cow::Borrowed("Shell Config");
            }
            if is_config_dotfile(lower) {
                return Cow::Borrowed("Config File");
            }
        }
        if is_single_dotfile(name) {
            return Cow::Borrowed("Dotfile");
        }
    }
    let raw_ext = extension_slice(name);
    if raw_ext.is_empty() {
        let mut buf = [0u8; 32];
        let is_special = lower_ascii_stack(name, &mut buf).is_some_and(is_special_code_filename);
        return Cow::Borrowed(if is_special { "Code File" } else { "File" });
    }
    let mut buf = [0u8; 32];
    if let Some(ext) = lower_ascii_stack(raw_ext, &mut buf) {
        if let Some(known) = known_type_description(ext) {
            return Cow::Borrowed(known);
        }
    }
    Cow::Owned(format!("{} File", raw_ext.to_uppercase()))
}

fn known_type_description(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "pdf" => "PDF Document",
        "doc" | "docx" | "docm" => "Word Document",
        "dot" | "dotx" | "dotm" => "Word Template",
        "xls" | "xlsx" | "xlsm" | "xlsb" => "Excel Spreadsheet",
        "xlt" | "xltx" | "xltm" => "Excel Template",
        "ppt" | "pptx" | "pptm" => "PowerPoint",
        "pot" | "potx" | "potm" => "PowerPoint Template",
        "pps" | "ppsx" | "ppsm" => "PowerPoint Slideshow",
        "txt" => "Text File",
        "rtf" => "Rich Text",
        "odt" => "OpenDocument Text",
        "ott" => "OpenDocument Template",
        "ods" => "OpenDocument Spreadsheet",
        "ots" => "OpenDocument Spreadsheet Template",
        "odp" => "OpenDocument Presentation",
        "otp" => "OpenDocument Presentation Template",
        "ipynb" => "Jupyter Notebook",
        "jpg" | "jpeg" => "JPEG Image",
        "png" => "PNG Image",
        "gif" => "GIF Image",
        "bmp" => "Bitmap Image",
        "svg" => "SVG Image",
        "webp" => "WebP Image",
        "ico" => "Icon File",
        "tiff" | "tif" => "TIFF Image",
        "raw" => "RAW Image",
        "avif" => "AVIF Image",
        "mp4" => "MP4 Video",
        "mkv" => "MKV Video",
        "avi" => "AVI Video",
        "mov" => "QuickTime Video",
        "wmv" => "WMV Video",
        "flv" => "Flash Video",
        "webm" => "WebM Video",
        "m4v" => "M4V Video",
        "ogv" => "OGV Video",
        "mp3" => "MP3 Audio",
        "wav" => "WAV Audio",
        "flac" => "FLAC Audio",
        "aac" => "AAC Audio",
        "ogg" => "OGG Audio",
        "wma" => "WMA Audio",
        "m4a" => "M4A Audio",
        "opus" => "Opus Audio",
        "zip" => "ZIP Archive",
        "rar" => "RAR Archive",
        "7z" => "7-Zip Archive",
        "tar" => "TAR Archive",
        "gz" => "GZip Archive",
        "bz2" => "BZip2 Archive",
        "xz" => "XZ Archive",
        "js" => "JavaScript",
        "ts" => "TypeScript",
        "jsx" => "React JSX",
        "tsx" => "React TSX",
        "py" => "Python",
        "java" => "Java",
        "c" => "C Source",
        "cpp" => "C++ Source",
        "h" => "C Header",
        "hpp" => "C++ Header",
        "cs" => "C#",
        "go" => "Go",
        "rs" => "Rust",
        "rb" => "Ruby",
        "php" => "PHP",
        "swift" => "Swift",
        "kt" => "Kotlin",
        "scala" => "Scala",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "scss" => "SCSS",
        "sass" => "Sass",
        "less" => "Less",
        "json" => "JSON",
        "xml" => "XML",
        "yaml" | "yml" => "YAML",
        "csv" => "CSV",
        "sql" => "SQL",
        "toml" => "TOML",
        "ini" | "conf" | "cfg" => "Config File",
        "env" => "Environment File",
        "exe" => "Windows Executable",
        "msi" => "Windows Installer",
        "dmg" => "macOS Disk Image",
        "deb" => "Debian Package",
        "rpm" => "RPM Package",
        "sh" => "Shell Script",
        "bat" => "Batch File",
        "ps1" => "PowerShell",
        "iso" | "img" => "Disk Image",
        "log" => "Log File",
        "md" => "Markdown",
        "lock" => "Lock File",
        _ => return None,
    })
}

/// What a file is, for BoxBox's own icons: a glyph each, and a colour per group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Folder,
    Code,
    Script,
    Config,
    Web,
    Data,
    Database,
    Diff,
    Image,
    Video,
    Audio,
    Pdf,
    Document,
    Text,
    Sheet,
    Slides,
    Archive,
    Package,
    Disc,
    Font,
    Key,
    Other,
}

impl Kind {
    pub fn of(name: &str, is_dir: bool) -> Self {
        if is_dir {
            return Self::Folder;
        }
        let raw_ext = extension_slice(name);
        if raw_ext.is_empty() {
            let mut buf = [0u8; 32];
            if let Some(lower) = lower_ascii_stack(name, &mut buf) {
                return match lower {
                    "makefile" | "gnumakefile" | "dockerfile" | "cmakelists.txt" => Self::Config,
                    _ if lower.starts_with("id_") => Self::Key,
                    _ if is_shell_dotfile(lower) => Self::Script,
                    _ if is_config_dotfile(lower) || is_single_dotfile(lower) => Self::Config,
                    _ if is_special_code_filename(lower) => Self::Code,
                    _ => Self::Other,
                };
            }
            if is_single_dotfile(name) {
                return Self::Config;
            }
            if name.get(..3).is_some_and(|prefix| prefix.eq_ignore_ascii_case("id_")) {
                return Self::Key;
            }
            return Self::Other;
        }
        let mut buf = [0u8; 32];
        let Some(ext) = lower_ascii_stack(raw_ext, &mut buf) else {
            return Self::Other;
        };
        match ext {
            "pdf" => Self::Pdf,
            "diff" | "patch" => Self::Diff,
            "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd" => Self::Script,
            "ini" | "conf" | "cfg" | "env" | "cmake" | "gradle" => Self::Config,
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "bmp" | "ico" | "avif" | "tiff"
            | "tif" | "raw" | "qoi" => Self::Image,
            "mp4" | "webm" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "m4v" | "ogv" => Self::Video,
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus" => Self::Audio,
            "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" | "py" | "go" | "rs" | "java" | "kt"
            | "scala" | "rb" | "php" | "cs" | "fs" | "c" | "cpp" | "cc" | "cxx" | "h" | "hpp"
            | "hxx" | "md" | "mdx" | "rst" | "tex" | "sql" | "graphql" | "gql" | "dockerfile"
            | "makefile" | "swift" | "r" | "lua" | "vim" | "asm" => Self::Code,
            "deb" | "rpm" | "appimage" | "flatpak" | "snap" | "apk" | "msi" | "jar" => {
                Self::Package
            }
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" => Self::Archive,
            "iso" | "img" | "dmg" => Self::Disc,
            "ttf" | "otf" | "woff" | "woff2" => Self::Font,
            "pem" | "key" | "crt" | "cer" | "pub" | "asc" | "gpg" | "p12" => Self::Key,
            "db" | "sqlite" | "sqlite3" => Self::Database,
            "xls" | "xlsx" | "xlsm" | "xlsb" | "xlt" | "xltx" | "xltm" | "ods" | "ots" => {
                Self::Sheet
            }
            "ppt" | "pptx" | "pptm" | "pot" | "potx" | "potm" | "pps" | "ppsx" | "ppsm" | "odp"
            | "otp" => Self::Slides,
            "doc" | "docx" | "docm" | "dot" | "dotx" | "dotm" | "odt" | "ott" | "rtf"
            | "ipynb" => Self::Document,
            "txt" | "log" | "csv" | "tsv" => Self::Text,
            "html" | "htm" | "xml" | "css" | "scss" | "sass" | "less" => Self::Web,
            "json" | "yaml" | "yml" | "toml" => Self::Data,
            _ => Self::Other,
        }
    }

    /// The Lucide glyph.
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Code => "file-code",
            Self::Script => "file-terminal",
            Self::Config => "file-cog",
            Self::Web => "globe",
            Self::Data => "file-json",
            Self::Database => "database",
            Self::Diff => "file-diff",
            Self::Image => "file-image",
            Self::Video => "file-play",
            Self::Audio => "file-music",
            Self::Pdf | Self::Document | Self::Text => "file-text",
            Self::Sheet => "file-spreadsheet",
            Self::Slides => "presentation",
            Self::Archive => "file-archive",
            Self::Package => "file-box",
            Self::Disc => "disc-3",
            Self::Font => "file-type",
            Self::Key => "file-key",
            Self::Other => "file",
        }
    }

    /// The colour of its group in the tinted set; `None` keeps the plain grey.
    pub fn tint(self) -> Option<u32> {
        match self {
            Self::Code | Self::Script | Self::Web | Self::Data | Self::Database => Some(0x60a5fa),
            Self::Image => Some(0x2dd4bf),
            Self::Video => Some(0xa78bfa),
            Self::Audio => Some(0xfb923c),
            Self::Pdf => Some(0xf87171),
            Self::Sheet => Some(0x4ade80),
            Self::Slides => Some(0xfb7185),
            Self::Archive | Self::Package => Some(0xfbbf24),
            Self::Folder | Self::Config | Self::Diff | Self::Document | Self::Text | Self::Disc | Self::Font | Self::Key | Self::Other => None,
        }
    }
}

/// `getFileIcon`, as an icon asset name.
pub fn file_icon(name: &str, is_dir: bool) -> &'static str {
    Kind::of(name, is_dir).glyph()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletion_dates_read_both_forms() {
        assert_eq!(deletion_millis("2026-11-15T11:20:01Z"), Some(1_794_741_601_000));
        let local = deletion_millis("2026-11-15T11:20:01").unwrap();
        assert_eq!(full_date(Some(local)), "15 Nov 2026, 11:20");
        assert_eq!(deletion_millis("yesterday"), None);
    }

    #[test]
    fn sizes_match_format_ts() {
        assert_eq!(file_size(0), "0 B");
        assert_eq!(file_size(1023), "1023 B");
        assert_eq!(file_size(1536), "1.5 KB");
        assert_eq!(file_size(1024 * 1024), "1 MB");
        assert_eq!(file_size(1_234_567_890), "1.15 GB");
    }

    #[test]
    fn types_and_icons() {
        assert_eq!(type_description("a.tar.gz"), "GZip Archive");
        assert_eq!(type_description("Makefile"), "Code File");
        assert_eq!(type_description("README"), "File");
        assert_eq!(type_description("x.weird"), "WEIRD File");
        assert_eq!(file_icon("photo.JPG", false), "file-image");
        assert_eq!(file_icon("data.yml", false), "file-json");
        assert_eq!(file_icon("run.sh", false), "file-terminal");
        assert_eq!(file_icon("clip.MP4", false), "file-play");
        assert_eq!(file_icon("ubuntu.iso", false), "disc-3");
        assert_eq!(file_icon("id_ed25519", false), "file-key");
        assert_eq!(file_icon("ID_é", false), "file-key");
        assert_eq!(type_description("éé"), "File");
        assert_eq!(type_description("😀"), "File");
        assert_eq!(Kind::of("report.pdf", false).tint(), Some(0xf87171));
        assert_eq!(file_icon("docs", true), "folder");
    }
}

/// `getDisplayName`: drops the extension unless extensions are shown. Folders keep their name.
pub fn display_name(name: &str, is_dir: bool, show_extensions: bool) -> String {
    if is_dir || show_extensions {
        return name.to_string();
    }
    name.rsplit_once('.')
        .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        .map(|(stem, _)| stem.to_string())
        .unwrap_or_else(|| name.to_string())
}

/// A Trash deletion date as milliseconds: RFC 3339 from a server, or the
/// trash spec's zone-less local time.
pub fn deletion_millis(text: &str) -> Option<i64> {
    if let Ok(date) = DateTime::parse_from_rfc3339(text) {
        return Some(date.timestamp_millis());
    }
    let naive = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S").ok()?;
    naive.and_local_timezone(Local).earliest().map(|date| date.timestamp_millis())
}

/// "13 Nov 2026, 15:31": the whole date, for Properties.
pub fn full_date(millis: Option<i64>) -> String {
    match millis.and_then(DateTime::from_timestamp_millis) {
        Some(date) => date.with_timezone(&Local).format("%-d %b %Y, %H:%M").to_string(),
        None => "—".into(),
    }
}
