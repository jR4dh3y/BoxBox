//! Searching local folders by name: a breadth-first walk, so near results come
//! first, that reports its best matches while it runs and stops when told to.

use std::collections::VecDeque;
use std::fs;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};

use super::{skip_search_directory, LocalFiles};
use crate::api::FileInfo;
use crate::search::{best, Hit, Request, LIMIT};

/// How often the matches so far are shown while the walk goes on.
const REPORT_EVERY: Duration = Duration::from_millis(150);

fn items(hits: &[Hit]) -> Vec<FileInfo> {
    hits.iter().map(|hit| hit.item.clone()).collect()
}

impl LocalFiles {
    /// The best matches below `request.path`, best first. `progress` receives the
    /// matches so far, in the same order, while the walk runs.
    ///
    /// Names are matched from the directory entry alone; only a match is inspected.
    /// Linked folders are not entered, which also keeps the walk free of loops, and
    /// hidden folders are entered only when hidden files are shown.
    pub fn search(&self, request: &Request, progress: &mut dyn FnMut(Vec<FileInfo>)) -> Result<Vec<FileInfo>> {
        let (mount, start) = self.resolve_directory(request.path)?;
        let mut hits: Vec<Hit> = Vec::new();
        let mut pending = VecDeque::from([(start.clone(), request.path.to_owned(), 0usize)]);
        let (mut reported, mut unreported) = (Instant::now(), false);
        while let Some((directory, virtual_directory, depth)) = pending.pop_front() {
            if request.cancel.load(Ordering::Relaxed) {
                break;
            }
            let entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if directory == start => {
                    return Err(error).with_context(|| format!("cannot search {}", request.path));
                }
                // A folder this user cannot read hides its contents, not the search.
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(name) = name.to_str() else { continue };
                if !request.hidden && name.starts_with('.') {
                    continue;
                }
                let path = entry.path();
                if let Some(rank) = request.query.rank(name) {
                    if let Ok(item) = self.info_for_path(mount, &virtual_directory, &path) {
                        hits.push(Hit { item, rank, depth });
                        unreported = true;
                    }
                }
                let is_folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
                if is_folder && !skip_search_directory(mount, &path) {
                    pending.push_back((path, format!("{virtual_directory}/{name}"), depth + 1));
                }
            }
            // Past the limit only better matches matter; keep memory bounded.
            if hits.len() > LIMIT * 2 {
                best(&mut hits);
            }
            if unreported && reported.elapsed() >= REPORT_EVERY {
                best(&mut hits);
                progress(items(&hits));
                (reported, unreported) = (Instant::now(), false);
            }
        }
        best(&mut hits);
        Ok(items(&hits))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    use super::*;
    use crate::search::Query;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(name: &str, files: &[&str]) -> (Self, LocalFiles) {
            let root = std::env::temp_dir().join(format!("boxbox-search-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            for file in files {
                let path = root.join(file);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, "x").unwrap();
            }
            let local = LocalFiles::with_root(root.clone()).unwrap();
            (Self(root), local)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn find(local: &LocalFiles, path: &str, query: &str, hidden: bool) -> Vec<String> {
        let (query, cancel) = (Query::parse(query), AtomicBool::new(false));
        let request = Request { path, query: &query, hidden, cancel: &cancel };
        local.search(&request, &mut |_| {}).unwrap().into_iter().map(|item| item.path).collect()
    }

    #[test]
    fn nested_matches_come_best_then_nearest() {
        let (_fixture, local) = Fixture::new("rank", &["deep/a/b/notes.txt", "my notes.md", "notes/readme.md", "other/keynotes.pdf", "plain.txt"]);
        assert_eq!(find(&local, "home", "notes", false), ["home/notes", "home/deep/a/b/notes.txt", "home/my notes.md", "home/other/keynotes.pdf"]);
        assert_eq!(find(&local, "home/deep", "NOTES", false), ["home/deep/a/b/notes.txt"]);
        assert_eq!(find(&local, "home", "*.md", false), ["home/my notes.md", "home/notes/readme.md"]);
        assert!(find(&local, "home", "missing", false).is_empty());
    }

    #[test]
    fn hidden_folders_are_searched_only_when_shown() {
        let (_fixture, local) = Fixture::new("hidden", &[".cache/report.tmp", ".report", "docs/report.pdf"]);
        assert_eq!(find(&local, "home", "report", false), ["home/docs/report.pdf"]);
        // Exact names lead, by name when equally deep; `.report` only holds the word.
        assert_eq!(find(&local, "home", "report", true), ["home/docs/report.pdf", "home/.cache/report.tmp", "home/.report"]);
    }

    #[test]
    fn a_linked_folder_is_found_but_not_entered() {
        let (fixture, local) = Fixture::new("links", &["real/target.txt"]);
        std::os::unix::fs::symlink(fixture.0.join("real"), fixture.0.join("real-link")).unwrap();
        std::os::unix::fs::symlink(&fixture.0, fixture.0.join("real/loop")).unwrap();
        assert_eq!(find(&local, "home", "target", false), ["home/real/target.txt"]);
        assert_eq!(find(&local, "home", "real", false), ["home/real", "home/real-link"]);
    }

    #[test]
    fn a_cancelled_search_stops_and_a_missing_folder_fails() {
        let (_fixture, local) = Fixture::new("cancel", &["a/one.txt", "a/b/one.txt"]);
        let (query, cancel) = (Query::parse("one"), AtomicBool::new(true));
        let request = Request { path: "home", query: &query, hidden: false, cancel: &cancel };
        assert!(local.search(&request, &mut |_| {}).unwrap().is_empty());
        let request = Request { path: "home/nowhere", cancel: &AtomicBool::new(false), ..request };
        assert!(local.search(&request, &mut |_| {}).is_err());
    }
}
