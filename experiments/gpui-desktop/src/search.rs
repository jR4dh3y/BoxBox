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
}
