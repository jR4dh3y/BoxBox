//! Search query policy shared by the pane and the toolbar field.
//! Mirrors `location.svelte.ts` (`isSearchActive`) and `config.ts` (`searchDebounceMs`).

use std::time::Duration;

/// The web UI only searches once the trimmed query has two characters.
const MIN_QUERY_CHARS: usize = 2;

pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// The trimmed query to send, or None while the input is too short to search.
pub fn active_query(input: &str) -> Option<&str> {
    let query = input.trim();
    (query.chars().count() >= MIN_QUERY_CHARS).then_some(query)
}

/// Appends typed text, dropping control characters (Enter, Tab, ...).
pub fn insert_text(input: &mut String, text: &str) -> bool {
    let before = input.len();
    input.extend(text.chars().filter(|ch| !ch.is_control()));
    input.len() != before
}

pub fn backspace(input: &mut String) -> bool {
    input.pop().is_some()
}

/// The end of `input` that fits `max_chars`, so the caret stays visible in a narrow field.
pub fn visible_tail(input: &str, max_chars: usize) -> String {
    let count = input.chars().count();
    if count <= max_chars {
        return input.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let tail: String = input.chars().skip(count - keep).collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn typing_skips_control_characters() {
        let mut input = String::new();
        assert!(insert_text(&mut input, "a"));
        assert!(insert_text(&mut input, " "));
        assert!(!insert_text(&mut input, "\n"));
        assert!(!insert_text(&mut input, "\u{7f}"));
        assert_eq!(input, "a ");
    }

    #[test]
    fn backspace_removes_one_character() {
        let mut input = "né".to_string();
        assert!(backspace(&mut input));
        assert_eq!(input, "n");
        assert!(backspace(&mut input));
        assert!(!backspace(&mut input));
    }

    #[test]
    fn tail_keeps_the_caret_end_visible() {
        assert_eq!(visible_tail("short", 10), "short");
        assert_eq!(visible_tail("abcdefghij", 5), "…ghij");
        assert_eq!(visible_tail("ééééé", 3), "…éé");
    }
}
