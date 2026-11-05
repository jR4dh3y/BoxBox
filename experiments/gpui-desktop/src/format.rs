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
pub fn file_date(raw: &str) -> String {
    let Ok(date) = DateTime::parse_from_rfc3339(raw) else {
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

/// `getFileIcon`, as an icon asset name.
pub fn file_icon(name: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "folder";
    }
    let ext = extension(name);
    if ext.is_empty() {
        let lower = name.to_lowercase();
        let code = SPECIAL_CODE_FILENAMES.contains(&lower.as_str()) || is_single_dotfile(&lower);
        return if code { "file-code" } else { "file" };
    }
    let ext = ext.as_str();
    let is = |list: &[&str]| list.contains(&ext);
    if is(IMAGE) {
        "file-image"
    } else if is(VIDEO) {
        "file-video"
    } else if is(AUDIO) {
        "file-audio"
    } else if is(CODE) {
        "file-code"
    } else if is(ARCHIVE) {
        "file-archive"
    } else if is(SPREADSHEET) {
        "file-spreadsheet"
    } else if is(DOCUMENT) || is(PRESENTATION) || ext == "ipynb" || ext == "pdf" || is(TEXT) {
        "file-text"
    } else if is(WEB) {
        "globe"
    } else if is(STYLE) {
        "palette"
    } else if is(DATA) {
        "file-json"
    } else {
        "file"
    }
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
        assert_eq!(file_icon("docs", true), "folder");
    }
}
