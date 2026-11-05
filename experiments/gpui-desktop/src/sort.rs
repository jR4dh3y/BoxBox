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

/// Hidden entries last; folders first for name/type; then value or name, in `dir`.
pub fn sort_files(items: &mut [FileInfo], sort: Sort) {
    let key = |item: &FileInfo| -> f64 {
        match sort.field {
            SortField::Size => item.size as f64,
            SortField::Modified => DateTime::parse_from_rfc3339(&item.mod_time)
                .map(|date| date.timestamp_millis() as f64)
                .unwrap_or(0.0),
            SortField::Name | SortField::Type => f64::from(u8::from(!item.is_dir)),
        }
    };
    let by_value = matches!(sort.field, SortField::Size | SortField::Modified);
    items.sort_by(|left, right| {
        let (left_hidden, right_hidden) = (left.name.starts_with('.'), right.name.starts_with('.'));
        if left_hidden != right_hidden {
            return if left_hidden {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        let (lv, rv) = (key(left), key(right));
        if !by_value && lv != rv {
            return lv.total_cmp(&rv);
        }
        let comparison = if by_value && lv != rv {
            lv.total_cmp(&rv)
        } else {
            natural_cmp(&left.name, &right.name)
        };
        match sort.dir {
            SortDir::Asc => comparison,
            SortDir::Desc => comparison.reverse(),
        }
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

    #[test]
    fn folders_first_hidden_last_numeric_names() {
        let mut items = vec![
            file("b10.txt", false, 1),
            file(".hidden", true, 0),
            file("b2.txt", false, 5),
            file("Zeta", true, 0),
            file("alpha", true, 0),
        ];
        sort_files(&mut items, Sort::default());
        let names: Vec<_> = items.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, ["alpha", "Zeta", "b2.txt", "b10.txt", ".hidden"]);
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
