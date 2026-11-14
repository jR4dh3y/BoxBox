//! Searching by name: when the field's text becomes a query, how a query
//! matches a file name, and how matches are ordered. The providers walk or
//! ask the server; this file decides what counts and what comes first.

use std::cmp::Ordering;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::api::FileInfo;
use crate::sort::natural_cmp;

/// The field only searches once the trimmed text has two characters.
const MIN_QUERY_CHARS: usize = 2;

pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// The most matches a search keeps: the best ones, not the first ones found.
pub const LIMIT: usize = 1000;

/// The trimmed query to send, or None while the input is too short to search.
pub fn active_query(input: &str) -> Option<&str> {
    let query = input.trim();
    (query.chars().count() >= MIN_QUERY_CHARS).then_some(query)
}

/// How well a name matches, best first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rank {
    /// The name, or the name without its extension, is the query.
    Exact,
    /// The name starts with it.
    Prefix,
    /// A word inside the name starts with it, as `notes` in `my-notes.txt`.
    Word,
    /// It appears somewhere in the name, or a wildcard pattern matched.
    Inside,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Term {
    /// Lower-case text to find in the name.
    Text(String),
    /// Lower-case pattern with `*` and `?`, matched against the whole name.
    Glob(Vec<char>),
}

/// The words of a search. Every word must match the name; case is ignored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    terms: Vec<Term>,
}

impl Query {
    /// Splits on whitespace. A word with `*` or `?` is a pattern for the whole name.
    pub fn parse(text: &str) -> Self {
        let terms = text
            .split_whitespace()
            .map(str::to_lowercase)
            .map(|word| match word.contains(['*', '?']) {
                true => Term::Glob(word.chars().collect()),
                false => Term::Text(word),
            })
            .collect();
        Self { terms }
    }

    /// One plain word a server can pre-filter by: the longest, as it narrows most.
    pub fn broadest_filter(&self) -> Option<&str> {
        self.terms
            .iter()
            .filter_map(|term| match term {
                Term::Text(text) => Some(text.as_str()),
                Term::Glob(_) => None,
            })
            .max_by_key(|text| text.chars().count())
    }

    /// The name's rank, or None when a word does not match. A name is as good as
    /// its weakest word.
    pub fn rank(&self, name: &str) -> Option<Rank> {
        if self.terms.is_empty() {
            return None;
        }
        let name = name.to_lowercase();
        let mut weakest = Rank::Exact;
        for term in &self.terms {
            let rank = match term {
                Term::Text(text) => rank_text(&name, text)?,
                Term::Glob(pattern) => glob(pattern, &name.chars().collect::<Vec<_>>()).then_some(Rank::Inside)?,
            };
            weakest = weakest.max(rank);
        }
        Some(weakest)
    }
}

fn rank_text(name: &str, text: &str) -> Option<Rank> {
    let at = name.find(text)?;
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    if name == text || (!stem.is_empty() && stem == text) {
        return Some(Rank::Exact);
    }
    if at == 0 {
        return Some(Rank::Prefix);
    }
    // Any occurrence that begins a word counts, not only the first.
    let starts_word = name.match_indices(text).any(|(index, _)| name[..index].chars().next_back().is_some_and(|before| !before.is_alphanumeric()));
    Some(if starts_word { Rank::Word } else { Rank::Inside })
}

/// `*` matches any run of characters and `?` exactly one.
fn glob(pattern: &[char], name: &[char]) -> bool {
    let (mut p, mut n) = (0, 0);
    // Where the last `*` was, and how much of the name it has swallowed so far.
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, n));
            p += 1;
        } else if let Some((star_p, star_n)) = star {
            p = star_p + 1;
            n = star_n + 1;
            star = Some((star_p, star_n + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|ch| *ch == '*')
}

/// A match with what orders it.
#[derive(Clone, Debug)]
pub struct Hit {
    pub item: FileInfo,
    pub rank: Rank,
    /// Folders below the searched one; nearer results come first.
    pub depth: usize,
}

/// Best match first, then nearest, then by name; paths settle the rest so
/// repeated searches agree.
pub fn compare(left: &Hit, right: &Hit) -> Ordering {
    left.rank
        .cmp(&right.rank)
        .then(left.depth.cmp(&right.depth))
        .then_with(|| natural_cmp(&left.item.name, &right.item.name))
        .then_with(|| left.item.path.cmp(&right.item.path))
}

/// Orders the hits and keeps the best `LIMIT`.
pub fn best(hits: &mut Vec<Hit>) {
    hits.sort_by(compare);
    hits.truncate(LIMIT);
}

/// One search, as a provider receives it.
pub struct Request<'a> {
    /// The folder to search below.
    pub path: &'a str,
    pub query: &'a Query,
    /// Whether hidden files, and what hidden folders hold, are searched.
    pub hidden: bool,
    /// Set when the search is no longer wanted; a walk stops at the next folder.
    pub cancel: &'a AtomicBool,
}

/// Ranks what a server returned for `broadest_filter` against the whole query.
pub fn rank_listed(items: Vec<FileInfo>, request: &Request) -> Vec<FileInfo> {
    let below = format!("{}/", request.path.trim_end_matches('/'));
    let mut hits: Vec<Hit> = items
        .into_iter()
        .filter_map(|item| {
            let inside = item.path.strip_prefix(&below).unwrap_or(&item.path);
            if !request.hidden && inside.split('/').any(|part| part.starts_with('.')) {
                return None;
            }
            let (rank, depth) = (request.query.rank(&item.name)?, inside.matches('/').count());
            Some(Hit { item, rank, depth })
        })
        .collect();
    best(&mut hits);
    hits.into_iter().map(|hit| hit.item).collect()
}

/// The folder a result sits in, relative to the searched folder; empty when it is
/// directly inside.
pub fn location<'a>(searched: &str, result: &'a str) -> &'a str {
    let inside = result.strip_prefix(searched).map_or(result, |rest| rest.trim_start_matches('/'));
    inside.rsplit_once('/').map_or("", |(folder, _)| folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rank(query: &str, name: &str) -> Option<Rank> {
        Query::parse(query).rank(name)
    }

    #[test]
    fn needs_two_trimmed_characters() {
        assert_eq!(active_query(""), None);
        assert_eq!(active_query("a"), None);
        assert_eq!(active_query("  a  "), None);
        assert_eq!(active_query("  ab  "), Some("ab"));
        // Characters, not bytes.
        assert_eq!(active_query("éé"), Some("éé"));
        assert_eq!(active_query("é"), None);
    }

    #[test]
    fn a_name_ranks_by_where_the_text_sits() {
        assert_eq!(rank("notes", "notes"), Some(Rank::Exact));
        assert_eq!(rank("notes", "Notes.txt"), Some(Rank::Exact));
        assert_eq!(rank("note", "notes.txt"), Some(Rank::Prefix));
        assert_eq!(rank("notes", "my-notes.txt"), Some(Rank::Word));
        assert_eq!(rank("otes", "notes.txt"), Some(Rank::Inside));
        assert_eq!(rank("notes", "report.pdf"), None);
        // A later occurrence may start a word when the first does not.
        assert_eq!(rank("an", "banana an.txt"), Some(Rank::Word));
        assert_eq!(rank(".txt", "a.txt"), Some(Rank::Inside));
    }

    #[test]
    fn every_word_must_match_and_the_weakest_decides() {
        assert_eq!(rank("tax 2024", "2024 tax return.pdf"), Some(Rank::Word));
        assert_eq!(rank("tax 2024", "tax return.pdf"), None);
        assert_eq!(rank("re tax", "tax return.pdf"), Some(Rank::Word));
        assert_eq!(rank("   ", "anything"), None);
    }

    #[test]
    fn wildcards_match_the_whole_name() {
        assert_eq!(rank("*.pdf", "Report.PDF"), Some(Rank::Inside));
        assert_eq!(rank("*.pdf", "report.pdf.bak"), None);
        assert_eq!(rank("img_????.jpg", "IMG_0042.jpg"), Some(Rank::Inside));
        assert_eq!(rank("img_????.jpg", "IMG_42.jpg"), None);
        assert_eq!(rank("a*b*c", "aXbYbZc"), Some(Rank::Inside));
        assert_eq!(rank("report *.pdf", "report-2024.pdf"), Some(Rank::Inside));
    }

    fn hit(path: &str, rank: Rank) -> Hit {
        let name = path.rsplit('/').next().unwrap().to_string();
        Hit { item: FileInfo { name, path: path.into(), size: 0, is_dir: false, mod_time: String::new() }, rank, depth: path.matches('/').count() }
    }

    #[test]
    fn better_then_nearer_then_by_name() {
        let mut hits = vec![hit("a/b/c/notes", Rank::Exact), hit("a/my notes 10", Rank::Word), hit("a/my notes 9", Rank::Word), hit("a/notes", Rank::Exact), hit("a/notes2", Rank::Prefix)];
        best(&mut hits);
        let paths: Vec<_> = hits.iter().map(|hit| hit.item.path.as_str()).collect();
        assert_eq!(paths, ["a/notes", "a/b/c/notes", "a/notes2", "a/my notes 9", "a/my notes 10"]);
    }

    #[test]
    fn only_the_best_are_kept() {
        let mut hits: Vec<Hit> = (0..LIMIT + 50).map(|index| hit(&format!("a/inside{index}"), Rank::Inside)).collect();
        hits.push(hit("a/z/exact", Rank::Exact));
        best(&mut hits);
        assert_eq!(hits.len(), LIMIT);
        assert_eq!(hits[0].item.path, "a/z/exact");
    }

    #[test]
    fn a_server_list_is_ranked_and_loses_hidden_folders() {
        let cancel = AtomicBool::new(false);
        let query = Query::parse("tax 2024");
        assert_eq!(query.broadest_filter(), Some("2024"));
        let item = |path: &str| hit(path, Rank::Inside).item;
        let listed = vec![item("docs/old/2024 tax.pdf"), item("docs/2024 holiday.jpg"), item("docs/.cache/tax 2024.tmp"), item("docs/tax 2024.pdf")];
        let visible = Request { path: "docs", query: &query, hidden: false, cancel: &cancel };
        let paths: Vec<_> = rank_listed(listed.clone(), &visible).into_iter().map(|item| item.path).collect();
        assert_eq!(paths, ["docs/tax 2024.pdf", "docs/old/2024 tax.pdf"]);
        let all = Request { hidden: true, ..visible };
        assert_eq!(rank_listed(listed, &all).len(), 3);
    }

    #[test]
    fn a_result_names_its_folder_below_the_search() {
        assert_eq!(location("home/docs", "home/docs/a.txt"), "");
        assert_eq!(location("home/docs", "home/docs/tax/2024/a.txt"), "tax/2024");
    }
}
