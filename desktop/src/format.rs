//! Ports of `frontend/src/lib/utils/format.ts` and `fileTypes.ts`.

use chrono::{DateTime, Datelike, Local};

const SIZE_UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

/// `formatFileSize`: two decimals with trailing zeros trimmed, e.g. "1.5 MB".
pub fn file_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".into();
    }
    let unit = ((bytes as f64).ln() / 1024f64.ln()).floor() as usize;
    let unit = unit.min(SIZE_UNITS.len() - 1);
    let value = bytes as f64 / 1024f64.powi(unit as i32);
    let rounded = (value * 100.0).round() / 100.0;
    format!("{rounded} {}", SIZE_UNITS[unit])
}

/// `formatFileDate`: "Today 3:04 PM", "Nov 5", or "Nov 5, 2024".
pub fn file_date(mod_time: Option<i64>) -> String {
    let Some(date) = mod_time.and_then(DateTime::from_timestamp_millis) else {
        return "-".into();
    };
    let date = date.with_timezone(&Local);
    let now = Local::now();
    if date.date_naive() == now.date_naive() {
        format!("Today {}", date.format("%-I:%M %p"))
    } else if date.year() == now.year() {
        date.format("%b %-d").to_string()
    } else {
        date.format("%b %-d, %Y").to_string()
    }
}

const VIDEO: &[&str] = &[
    "mp4", "webm", "mkv", "avi", "mov", "wmv", "flv", "m4v", "ogv",
];
const AUDIO: &[&str] = &["mp3", "wav", "flac", "aac", "ogg", "m4a", "wma", "opus"];
const IMAGE: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "ico", "avif", "tiff", "tif", "raw", "qoi",
];
const CODE: &[&str] = &[
    "js",
    "jsx",
    "ts",
    "tsx",
    "mjs",
    "cjs",
    "py",
    "go",
    "rs",
    "java",
    "kt",
    "scala",
    "rb",
    "php",
    "cs",
    "fs",
    "c",
    "cpp",
    "cc",
    "cxx",
    "h",
    "hpp",
    "hxx",
    "sh",
    "bash",
    "zsh",
    "fish",
    "ps1",
    "bat",
    "cmd",
    "ini",
    "conf",
    "cfg",
    "env",
    "md",
    "mdx",
    "rst",
    "tex",
    "sql",
    "graphql",
    "gql",
    "dockerfile",
    "makefile",
    "cmake",
    "gradle",
    "swift",
    "r",
    "lua",
    "vim",
    "asm",
];
const TEXT: &[&str] = &["txt", "log", "csv", "tsv"];
const ARCHIVE: &[&str] = &["zip", "rar", "7z", "tar", "gz", "bz2", "xz"];
const SPREADSHEET: &[&str] = &[
    "xls", "xlsx", "xlsm", "xlsb", "xlt", "xltx", "xltm", "ods", "ots",
];
const DOCUMENT: &[&str] = &[
    "doc", "docx", "docm", "dot", "dotx", "dotm", "odt", "ott", "rtf",
];
const PRESENTATION: &[&str] = &[
    "ppt", "pptx", "pptm", "pot", "potx", "potm", "pps", "ppsx", "ppsm", "odp", "otp",
];
const WEB: &[&str] = &["html", "htm", "xml"];
const STYLE: &[&str] = &["css", "scss", "sass", "less"];
const DATA: &[&str] = &["json", "yaml", "yml", "toml"];
const SHELL_DOTFILES: &[&str] = &[
    ".bashrc",
    ".bash_profile",
    ".bash_login",
    ".bash_logout",
    ".profile",
    ".zshrc",
    ".zprofile",
    ".zshenv",
    ".zlogin",
    ".zlogout",
    ".kshrc",
    ".cshrc",
    ".tcshrc",
];
/// Keys of `DOTFILE_LANGUAGE_MAP`.
const CONFIG_DOTFILES: &[&str] = &[
    ".env",
    ".envrc",
    ".gitconfig",
    ".gitignore",
    ".dockerignore",
    ".editorconfig",
    ".npmrc",
    ".yarnrc",
    ".curlrc",
    ".wgetrc",
    ".vimrc",
];
const SPECIAL_CODE_FILENAMES: &[&str] = &[
    "dockerfile",
    "makefile",
    "gnumakefile",
    "cmakelists.txt",
    "gemfile",
    "rakefile",
];

pub fn extension(name: &str) -> String {
    match name.rfind('.') {
        Some(index) if index > 0 => name[index + 1..].to_lowercase(),
        _ => String::new(),
    }
}

fn is_single_dotfile(lower: &str) -> bool {
    lower.starts_with('.') && lower.len() > 1 && !lower[1..].contains('.')
}

/// `getFileTypeDescription`.
pub fn type_description(name: &str) -> String {
    let lower = name.to_lowercase();
    if SHELL_DOTFILES.contains(&lower.as_str()) {
        return "Shell Config".into();
    }
    if CONFIG_DOTFILES.contains(&lower.as_str()) {
        return "Config File".into();
    }
    if is_single_dotfile(&lower) {
        return "Dotfile".into();
    }
    let ext = extension(name);
    if ext.is_empty() {
        return if SPECIAL_CODE_FILENAMES.contains(&lower.as_str()) {
            "Code File"
        } else {
            "File"
        }
        .into();
    }
    let known = match ext.as_str() {
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
        _ => return format!("{} File", ext.to_uppercase()),
    };
    known.into()
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

const SCRIPT: &[&str] = &["sh", "bash", "zsh", "fish", "ps1", "bat", "cmd"];
const CONFIG: &[&str] = &["ini", "conf", "cfg", "env", "cmake", "gradle"];
const PACKAGE: &[&str] = &["deb", "rpm", "appimage", "flatpak", "snap", "apk", "msi", "jar"];
const DISC: &[&str] = &["iso", "img", "dmg"];
const FONT: &[&str] = &["ttf", "otf", "woff", "woff2"];
const KEY: &[&str] = &["pem", "key", "crt", "cer", "pub", "asc", "gpg", "p12"];
const DATABASE: &[&str] = &["db", "sqlite", "sqlite3"];

impl Kind {
    pub fn of(name: &str, is_dir: bool) -> Self {
        if is_dir {
            return Self::Folder;
        }
        let lower = name.to_lowercase();
        let ext = extension(name).to_lowercase();
        if ext.is_empty() {
            return match lower.as_str() {
                "makefile" | "gnumakefile" | "dockerfile" | "cmakelists.txt" => Self::Config,
                _ if lower.starts_with("id_") => Self::Key,
                _ if SHELL_DOTFILES.contains(&lower.as_str()) => Self::Script,
                _ if CONFIG_DOTFILES.contains(&lower.as_str()) || is_single_dotfile(&lower) => Self::Config,
                _ if SPECIAL_CODE_FILENAMES.contains(&lower.as_str()) => Self::Code,
                _ => Self::Other,
            };
        }
        let is = |list: &[&str]| list.contains(&ext.as_str());
        match () {
            _ if ext == "pdf" => Self::Pdf,
            _ if ext == "diff" || ext == "patch" => Self::Diff,
            _ if is(SCRIPT) => Self::Script,
            _ if is(CONFIG) => Self::Config,
            _ if is(IMAGE) => Self::Image,
            _ if is(VIDEO) => Self::Video,
            _ if is(AUDIO) => Self::Audio,
            _ if is(CODE) => Self::Code,
            _ if is(PACKAGE) => Self::Package,
            _ if is(ARCHIVE) => Self::Archive,
            _ if is(DISC) => Self::Disc,
            _ if is(FONT) => Self::Font,
            _ if is(KEY) => Self::Key,
            _ if is(DATABASE) => Self::Database,
            _ if is(SPREADSHEET) => Self::Sheet,
            _ if is(PRESENTATION) => Self::Slides,
            _ if is(DOCUMENT) || ext == "ipynb" => Self::Document,
            _ if is(TEXT) => Self::Text,
            _ if is(WEB) || is(STYLE) => Self::Web,
            _ if is(DATA) => Self::Data,
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

/// "13 Nov 2026, 15:31": the whole date, for Properties.
pub fn full_date(millis: Option<i64>) -> String {
    match millis.and_then(DateTime::from_timestamp_millis) {
        Some(date) => date.with_timezone(&Local).format("%-d %b %Y, %H:%M").to_string(),
        None => "—".into(),
    }
}
