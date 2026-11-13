//! Port of `frontend/src/lib/browse/sortFiles.ts`.

use std::cmp::Ordering;

use chrono::DateTime;

use crate::api::FileInfo;

/// Serialised with the settings file's labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SortField {
    Name,
    Type,
    Size,
    #[serde(rename = "Date modified")]
    Modified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SortDir {
    #[serde(rename = "Ascending")]
    Asc,
    #[serde(rename = "Descending")]
    Desc,
}

impl SortField {
    /// `SORT_BY_OPTIONS` in `settings/options.ts`.
    pub const OPTIONS: [(Self, &'static str); 4] =
        [(Self::Name, "Name"), (Self::Size, "Size"), (Self::Modified, "Date modified"), (Self::Type, "Type")];
}

impl SortDir {
    pub const OPTIONS: [(Self, &'static str); 2] = [(Self::Asc, "Ascending"), (Self::Desc, "Descending")];
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
///
/// Keys are read once per item rather than once per comparison: parsing the
/// timestamp inside the comparator made 100k files take 240 ms.
pub fn sort_files(items: &mut Vec<FileInfo>, sort: Sort) {
    let by_value = matches!(sort.field, SortField::Size | SortField::Modified);
    let value = |item: &FileInfo| -> i64 {
        match sort.field {
            SortField::Size => i64::try_from(item.size).unwrap_or(i64::MAX),
            SortField::Modified => DateTime::parse_from_rfc3339(&item.mod_time)
                .map(|date| date.timestamp_millis())
                .unwrap_or(0),
            SortField::Name | SortField::Type => 0,
        }
    };
    let directed = |ordering: Ordering| match sort.dir {
        SortDir::Asc => ordering,
        SortDir::Desc => ordering.reverse(),
    };
    let mut keyed: Vec<(i64, FileInfo)> = items.drain(..).map(|item| (value(&item), item)).collect();
    keyed.sort_by(|(left_value, left), (right_value, right)| {
        if left.is_dir != right.is_dir {
            return if left.is_dir { Ordering::Less } else { Ordering::Greater };
        }
        if by_value && left_value != right_value {
            return directed(left_value.cmp(right_value));
        }
        let names = natural_cmp(&left.name, &right.name);
        let names = if by_value { names } else { directed(names) };
        names.then_with(|| natural_cmp(&left.path, &right.path))
    });
    items.extend(keyed.into_iter().map(|(_, item)| item));
}

/// Approximates `Intl.Collator(undefined, { numeric: true })`: case-insensitive, digit runs by value.
pub fn natural_cmp(left: &str, right: &str) -> Ordering {
    let (a, b) = (left.as_bytes(), right.as_bytes());
    let (mut i, mut j) = (0, 0);
    loop {
        match (a.get(i).copied(), b.get(j).copied()) {
            (None, None) => return left.cmp(right),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let run = |bytes: &[u8], start: usize| {
                    let end = start + bytes[start..].iter().take_while(|byte| byte.is_ascii_digit()).count();
                    let zeros = bytes[start..end].iter().take_while(|byte| **byte == b'0').count();
                    (start + zeros, end)
                };
                let ((sa, ea), (sb, eb)) = (run(a, i), run(b, j));
                let ordering = (ea - sa).cmp(&(eb - sb)).then_with(|| a[sa..ea].cmp(&b[sb..eb]));
                if ordering != Ordering::Equal {
                    return ordering;
                }
                (i, j) = (ea, eb);
            }
            (Some(x), Some(y)) if x.is_ascii() && y.is_ascii() => {
                let ordering = x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase());
                if ordering != Ordering::Equal {
                    return ordering;
                }
                (i, j) = (i + 1, j + 1);
            }
            _ => {
                // Both indices sit on char boundaries: digits and ASCII advance one byte.
                let (x, y) = (left[i..].chars().next(), right[j..].chars().next());
                let (Some(x), Some(y)) = (x, y) else { return left.cmp(right) };
                let ordering = x.to_lowercase().cmp(y.to_lowercase());
                if ordering != Ordering::Equal {
                    return ordering;
                }
                (i, j) = (i + x.len_utf8(), j + y.len_utf8());
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

