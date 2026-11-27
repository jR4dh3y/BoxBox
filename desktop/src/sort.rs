//! Port of `frontend/src/lib/browse/sortFiles.ts`.

use std::cmp::Ordering;

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

impl SortField {
    /// The table column that shows this order.
    pub fn column(self) -> crate::components::Column {
        use crate::components::Column;
        match self {
            Self::Name => Column::Name,
            Self::Type => Column::Type,
            Self::Size => Column::Size,
            Self::Modified => Column::Modified,
        }
    }

    /// The order a folder column stands for; Trash's own columns have none.
    pub fn of_column(column: crate::components::Column) -> Option<Self> {
        use crate::components::Column;
        match column {
            Column::Name => Some(Self::Name),
            Column::Type => Some(Self::Type),
            Column::Size => Some(Self::Size),
            Column::Modified => Some(Self::Modified),
            Column::Location | Column::Deleted => None,
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Folders lead every sort. Size, date and type sort by the displayed column,
/// breaking ties by ascending natural name. Paths break remaining ties.
pub fn sort_files(items: &mut Vec<FileInfo>, sort: Sort) {
    let directed = |ordering: Ordering| match sort.dir {
        SortDir::Asc => ordering,
        SortDir::Desc => ordering.reverse(),
    };
    let mut keyed: Vec<_> = items
        .drain(..)
        .map(|item| {
            let kind = (sort.field == SortField::Type && !item.is_dir)
                .then(|| crate::format::type_description(&item.name));
            (kind, item)
        })
        .collect();
    keyed.sort_by(|(left_kind, left), (right_kind, right)| {
        if left.is_dir != right.is_dir {
            return if left.is_dir { Ordering::Less } else { Ordering::Greater };
        }
        let order = match sort.field {
            SortField::Name => natural_cmp(&left.name, &right.name),
            SortField::Size => left.size.cmp(&right.size),
            SortField::Modified => left.mod_time.unwrap_or(0).cmp(&right.mod_time.unwrap_or(0)),
            SortField::Type => natural_cmp(
                left_kind.as_deref().unwrap_or_default(),
                right_kind.as_deref().unwrap_or_default(),
            ),
        };
        directed(order)
            .then_with(|| natural_cmp(&left.name, &right.name))
            .then_with(|| natural_cmp(&left.path, &right.path))
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
            hidden: false,
            mod_time: None,
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
    fn type_groups_displayed_kinds_and_keeps_folders_first() {
        let mut items = vec![
            file("d.pdf", false, 0),
            file("c.txt", false, 0),
            file("b.pdf", false, 0),
            file("a.TXT", false, 0),
            file("z.txt", true, 0),
        ];
        sort_files(&mut items, Sort { field: SortField::Type, dir: SortDir::Asc });
        assert_eq!(names(&items), ["z.txt", "b.pdf", "d.pdf", "a.TXT", "c.txt"]);
        sort_files(&mut items, Sort { field: SortField::Type, dir: SortDir::Desc });
        assert_eq!(names(&items), ["z.txt", "a.TXT", "c.txt", "b.pdf", "d.pdf"]);
    }
}

