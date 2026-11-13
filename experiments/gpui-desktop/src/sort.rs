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

/// `DEFAULT_SORT_BY` and `DEFAULT_SORT_DIR`: newest first.
impl Default for Sort {
    fn default() -> Self {
        Self {
            field: SortField::Modified,
            dir: SortDir::Desc,
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

/// Folders lead every sort. Size and time sort by value, and their ties by
/// ascending natural name; name and type sort by name in `dir`. Paths break
/// remaining ties, so repeated reads keep one order.
pub fn sort_files(items: &mut [FileInfo], sort: Sort) {
    let by_value = matches!(sort.field, SortField::Size | SortField::Modified);
    let value = |item: &FileInfo| -> f64 {
        match sort.field {
            SortField::Size => item.size as f64,
            SortField::Modified => DateTime::parse_from_rfc3339(&item.mod_time)
                .map(|date| date.timestamp_millis() as f64)
                .unwrap_or(0.0),
            SortField::Name | SortField::Type => 0.0,
        }
    };
    let directed = |ordering: Ordering| match sort.dir {
        SortDir::Asc => ordering,
        SortDir::Desc => ordering.reverse(),
    };
    items.sort_by(|left, right| {
        if left.is_dir != right.is_dir {
            return if left.is_dir { Ordering::Less } else { Ordering::Greater };
        }
        if by_value {
            let ordering = value(left).total_cmp(&value(right));
            if ordering != Ordering::Equal {
                return directed(ordering);
            }
        }
        let names = natural_cmp(&left.name, &right.name);
        let names = if by_value { names } else { directed(names) };
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
        }
    }

    fn name_asc() -> Sort {
        Sort { field: SortField::Name, dir: SortDir::Asc }
    }

    fn names(items: &[FileInfo]) -> Vec<&str> {
        items.iter().map(|item| item.name.as_str()).collect()
    }

    #[test]
    fn folders_lead_and_numbers_compare_by_value() {
        let mut items = vec![
            file("b10.txt", false, 1),
            file(".hidden", true, 0),
            file("b2.txt", false, 5),
            file("Zeta", true, 0),
            file("alpha", true, 0),
        ];
        sort_files(&mut items, name_asc());
        assert_eq!(names(&items), [".hidden", "alpha", "Zeta", "b2.txt", "b10.txt"]);
    }

    #[test]
    fn folders_lead_descending_sorts_and_ties_stay_ascending() {
        let mut items = vec![file("b", false, 5), file("a", false, 5), file("dir", true, 0), file("c", false, 9)];
        sort_files(&mut items, Sort { field: SortField::Size, dir: SortDir::Desc });
        assert_eq!(names(&items), ["dir", "c", "a", "b"]);
    }

    #[test]
    fn the_default_is_newest_first() {
        assert_eq!(Sort::default(), Sort { field: SortField::Modified, dir: SortDir::Desc });
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
