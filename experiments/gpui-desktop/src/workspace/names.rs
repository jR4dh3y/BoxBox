//! What a new or changed name must be, and the names the create dialog suggests.

/// Why `name` cannot name an entry among `siblings`, or `None` when it can.
/// `name` is already trimmed.
pub(crate) fn name_problem<'a>(name: &str, mut siblings: impl Iterator<Item = &'a str>) -> Option<String> {
    if name.is_empty() {
        return Some("Enter a name.".into());
    }
    if name == "." || name == ".." {
        return Some(format!("“{name}” is not a valid name."));
    }
    if name.contains('/') {
        return Some("A name cannot contain “/”.".into());
    }
    if name.contains('\0') {
        return Some("A name cannot contain a null character.".into());
    }
    if siblings.any(|sibling| sibling == name) {
        return Some(format!("“{name}” already exists here."));
    }
    None
}

/// Where the part of a name before its extension ends: the whole name for a
/// folder or a dotfile, else up to the last dot.
pub(crate) fn basename_end(name: &str, is_dir: bool) -> usize {
    match name.rfind('.') {
        Some(dot) if dot > 0 && !is_dir => dot,
        _ => name.len(),
    }
}

/// `name`, or the first of `name 2`, `name 3`, … (before the extension) not taken.
pub(crate) fn unused_name(name: &str, is_dir: bool, taken: &dyn Fn(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_string();
    }
    let (stem, extension) = name.split_at(basename_end(name, is_dir));
    (2..).map(|number| format!("{stem} {number}{extension}")).find(|candidate| !taken(candidate)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_refused_with_a_reason() {
        let none = std::iter::empty();
        assert_eq!(name_problem("", none.clone()).unwrap(), "Enter a name.");
        assert_eq!(name_problem("a/b", none.clone()).unwrap(), "A name cannot contain “/”.");
        assert_eq!(name_problem("..", none.clone()).unwrap(), "“..” is not a valid name.");
        assert_eq!(name_problem("a.txt", ["a.txt"].into_iter()).unwrap(), "“a.txt” already exists here.");
        // A backslash is an ordinary character on Linux.
        assert!(name_problem("a\\b", none).is_none());
    }

    #[test]
    fn suggestions_skip_taken_names_before_the_extension() {
        let taken = |name: &str| ["untitled.txt", "untitled 2.txt", "New folder"].contains(&name);
        assert_eq!(unused_name("untitled.txt", false, &taken), "untitled 3.txt");
        assert_eq!(unused_name("New folder", true, &taken), "New folder 2");
        assert_eq!(unused_name("notes", false, &taken), "notes");
        assert_eq!(basename_end(".bashrc", false), 7);
        assert_eq!(basename_end("my.folder", true), 9);
    }
}
