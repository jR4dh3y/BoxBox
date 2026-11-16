//! Recent files from the freedesktop desktop-bookmark list
//! (`$XDG_DATA_HOME/recently-used.xbel`), which GTK apps write.

use std::path::{Path, PathBuf};

/// The local files in the list that still exist, most recently used first.
pub(super) fn files(data_home: &Path, limit: usize) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(data_home.join("recently-used.xbel")) else { return Vec::new() };
    let mut entries: Vec<(String, PathBuf)> = bookmarks(&text)
        .filter_map(|(href, used)| Some((used, crate::os_clipboard::path_from_uri(&xml_unescape(&href))?)))
        .filter(|(_, path)| path.is_file())
        .collect();
    // RFC 3339 times in UTC sort as text.
    entries.sort_by(|left, right| right.0.cmp(&left.0));
    entries.dedup_by(|left, right| left.1 == right.1);
    entries.into_iter().take(limit).map(|(_, path)| path).collect()
}

/// Each `<bookmark href=… modified=… visited=…>`: its href and when it was last used.
fn bookmarks(text: &str) -> impl Iterator<Item = (String, String)> + '_ {
    text.split("<bookmark ").skip(1).filter_map(|element| {
        // Put back the space the split took, so every attribute follows one.
        let tag = format!(" {}", &element[..element.find('>')?]);
        let tag = tag.as_str();
        let href = attribute(tag, "href")?;
        let used = [attribute(tag, "visited"), attribute(tag, "modified"), attribute(tag, "added")].into_iter().flatten().max().unwrap_or_default();
        Some((href, used))
    })
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let end = start + tag[start..].find('"')?;
    Some(tag[start..end].to_string())
}

fn xml_unescape(text: &str) -> String {
    text.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_newest_existing_files_first() {
        let root = std::env::temp_dir().join(format!("boxbox-recent-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("old.txt"), "").unwrap();
        std::fs::write(root.join("new file.txt"), "").unwrap();
        let uri = |name: &str| format!("file://{}/{}", root.display(), name.replace(' ', "%20"));
        let xbel = format!(
            r#"<?xml version="1.0"?><xbel version="1.0">
  <bookmark href="{}" added="2026-01-01T00:00:00Z" modified="2026-01-01T00:00:00Z" visited="2026-01-02T00:00:00Z"></bookmark>
  <bookmark href="{}" added="2026-03-01T00:00:00Z" modified="2026-03-01T00:00:00Z" visited="2026-03-01T00:00:00Z"></bookmark>
  <bookmark href="{}" added="2026-04-01T00:00:00Z" modified="2026-04-01T00:00:00Z" visited="2026-04-01T00:00:00Z"></bookmark>
  <bookmark href="https://example.com/" added="2026-05-01T00:00:00Z" modified="2026-05-01T00:00:00Z" visited="2026-05-01T00:00:00Z"></bookmark>
</xbel>"#,
            uri("old.txt"),
            uri("new file.txt"),
            uri("gone.txt"),
        );
        std::fs::write(root.join("recently-used.xbel"), xbel).unwrap();
        assert_eq!(files(&root, 10), [root.join("new file.txt"), root.join("old.txt")]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
