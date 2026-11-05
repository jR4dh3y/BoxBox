//! Port of `frontend/src/lib/browse/sortFiles.ts`.

use std::cmp::Ordering;

use chrono::DateTime;

use crate::api::FileInfo;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortField {
    Name,
    Type,
    Size,
    Modified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub field: SortField,
    pub dir: SortDir,
}

impl Default for Sort {
    fn default() -> Self {
        Self {
            field: SortField::Name,
            dir: SortDir::Asc,
        }
    }
}

impl Sort {
    /// Clicking the sorted column flips it; another column starts ascending.
    pub fn toggled(self, field: SortField) -> Self {
        match (self.field == field, self.dir) {
            (true, SortDir::Asc) => Self {
                field,
                dir: SortDir::Desc,
            },
            _ => Self {
                field,
                dir: SortDir::Asc,
            },
        }
    }
}

/// PR #211's `sortFiles`: folders lead every sort. Size and time ties use ascending natural
/// names, then paths, so search results and repeated reads have a deterministic order.
pub fn sort_files(items: &mut [FileInfo], sort: Sort) {
    let key = |item: &FileInfo| -> i64 {
        match sort.field {
            SortField::Size => item.size as i64,
            SortField::Modified => DateTime::parse_from_rfc3339(&item.mod_time)
                .map(|date| date.timestamp_millis())
                .unwrap_or(0),
            SortField::Name | SortField::Type => 0,
        }
    };
    let by_value = matches!(sort.field, SortField::Size | SortField::Modified);
    let direction = |ordering: Ordering| match sort.dir {
        SortDir::Asc => ordering,
        SortDir::Desc => ordering.reverse(),
    };
    items.sort_by(|left, right| {
        if left.is_dir != right.is_dir {
            return if left.is_dir {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        if by_value {
            let value = key(left).cmp(&key(right));
            if value != Ordering::Equal {
                return direction(value);
            }
        }
        let names = natural_cmp(&left.name, &right.name).then_with(|| left.name.cmp(&right.name));
        let names = if by_value { names } else { direction(names) };
        names.then_with(|| natural_cmp(&left.path, &right.path))
    });
}

/// Approximates `Intl.Collator(undefined, { numeric: true })`: case-insensitive, digit runs by value.
pub fn natural_cmp(left: &str, right: &str) -> Ordering {
    let (mut a, mut b) = (left.chars().peekable(), right.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return left.cmp(right),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut digits = String::new();
                    while let Some(d) = it.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(d);
                        it.next();
                    }
                    digits
                };
                let (da, db) = (take(&mut a), take(&mut b));
                let (ta, tb) = (da.trim_start_matches('0'), db.trim_start_matches('0'));
                let ordering = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            (Some(x), Some(y)) => {
                let ordering = x.to_lowercase().cmp(y.to_lowercase());
                if ordering != Ordering::Equal {
                    return ordering;
                }
                a.next();
                b.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, is_dir: bool, size: u64) -> FileInfo {
        FileInfo {
            name: name.into(),
            path: name.into(),
            size,
            is_dir,
            mod_time: String::new(),
            permissions: String::new(),
        }
    }

    fn dated(name: &str, is_dir: bool, time: &str) -> FileInfo {
        FileInfo {
            mod_time: time.into(),
            ..file(name, is_dir, 0)
        }
    }

    fn names(items: &[FileInfo]) -> Vec<&str> {
        items.iter().map(|item| item.name.as_str()).collect()
    }

    #[test]
    fn folders_lead_and_names_are_natural() {
        let mut items = vec![
            file("b10.txt", false, 1),
            file(".hidden", true, 0),
            file("b2.txt", false, 5),
            file("Zeta", true, 0),
            file("alpha", true, 0),
        ];
        sort_files(
            &mut items,
            Sort {
                field: SortField::Name,
                dir: SortDir::Asc,
            },
        );
        assert_eq!(
            names(&items),
            [".hidden", "alpha", "Zeta", "b2.txt", "b10.txt"]
        );
        sort_files(
            &mut items,
            Sort {
                field: SortField::Name,
                dir: SortDir::Desc,
            },
        );
        assert_eq!(
            names(&items),
            ["Zeta", "alpha", ".hidden", "b10.txt", "b2.txt"]
        );
    }

    #[test]
    fn default_sort_is_newest_first_with_name_ties() {
        let mut items = vec![
            dated("old.txt", false, "2026-01-01T00:00:00Z"),
            dated("b.txt", false, "2026-03-01T00:00:00Z"),
            dated("a.txt", false, "2026-03-01T00:00:00Z"),
            dated("dir", true, "2020-01-01T00:00:00Z"),
        ];
        sort_files(&mut items, Sort::default());
        assert_eq!(names(&items), ["dir", "a.txt", "b.txt", "old.txt"]);
    }

    #[test]
    fn size_desc() {
        let mut items = vec![file("a", false, 1), file("b", false, 9)];
        sort_files(
            &mut items,
            Sort {
                field: SortField::Size,
                dir: SortDir::Desc,
            },
        );
        assert_eq!(items[0].name, "b");
    }
}
