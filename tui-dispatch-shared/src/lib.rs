//! Helpers for parsing action variant names.
//!
//! These utilities are shared by derive-time and runtime category inference so
//! action filtering stays consistent across crates.

/// Action verbs used by category inference.
///
/// Category inference treats these as boundaries between a domain prefix and
/// the action verb (for example `SearchStart` -> `search`).
pub const ACTION_VERBS: &[&str] = &[
    "Start", "End", "Open", "Close", "Submit", "Confirm", "Cancel", "Next", "Prev", "Up", "Down",
    "Left", "Right", "Enter", "Exit", "Escape", "Add", "Remove", "Clear", "Update", "Set", "Get",
    "Load", "Save", "Delete", "Create", "Fetch", "Change", "Resize", "Error", "Show", "Hide",
    "Enable", "Disable", "Toggle", "Focus", "Blur", "Select", "Move", "Copy", "Cycle", "Reset",
    "Scroll",
];

/// Split PascalCase text into parts.
///
/// Handles acronym boundaries: `APIFetchStart` -> `["API", "Fetch", "Start"]`.
pub fn split_pascal_case(value: &str) -> Vec<String> {
    let chars: Vec<char> = value.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }

    let mut parts = Vec::new();
    let mut start = 0usize;
    for idx in 1..chars.len() {
        let prev = chars[idx - 1];
        let curr = chars[idx];
        let next = chars.get(idx + 1).copied();

        let lower_to_upper = prev.is_ascii_lowercase() && curr.is_ascii_uppercase();
        let acronym_to_word = prev.is_ascii_uppercase()
            && curr.is_ascii_uppercase()
            && next.is_some_and(|ch| ch.is_ascii_lowercase());

        if lower_to_upper || acronym_to_word {
            parts.push(chars[start..idx].iter().collect());
            start = idx;
        }
    }
    parts.push(chars[start..].iter().collect());
    parts
}

/// Convert PascalCase text to snake_case.
pub fn pascal_to_snake_case(value: &str) -> String {
    split_pascal_case(value)
        .into_iter()
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

/// Infer action category from an action variant name.
///
/// Returns:
/// - `Some("async_result")` for `Did*`
/// - `Some(<domain>)` for `DomainVerb*` patterns
/// - `None` when no category can be inferred
pub fn infer_action_category(action_name: &str) -> Option<String> {
    let parts = split_pascal_case(action_name);
    if parts.is_empty() {
        return None;
    }

    if parts[0] == "Did" {
        return Some("async_result".to_string());
    }

    if parts.len() < 2 {
        return None;
    }

    // VerbNoun actions (e.g. OpenFile) intentionally do not infer categories.
    if ACTION_VERBS.contains(&parts[0].as_str()) {
        return None;
    }

    let mut prefix_end = parts.len();
    let mut found_verb = false;
    for (idx, part) in parts.iter().enumerate().skip(1) {
        if part == "Did" || ACTION_VERBS.contains(&part.as_str()) {
            prefix_end = idx;
            found_verb = true;
            break;
        }
    }

    if !found_verb || prefix_end == 0 {
        return None;
    }

    Some(
        parts[..prefix_end]
            .iter()
            .map(|part| part.to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join("_"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_pascal_case_handles_acronyms() {
        assert_eq!(
            split_pascal_case("APIFetchStart"),
            vec!["API".to_string(), "Fetch".to_string(), "Start".to_string()]
        );
        assert_eq!(
            split_pascal_case("SearchHTTPResult"),
            vec![
                "Search".to_string(),
                "HTTP".to_string(),
                "Result".to_string()
            ]
        );
    }

    #[test]
    fn test_pascal_to_snake_case_handles_acronyms() {
        assert_eq!(pascal_to_snake_case("APIFetch"), "api_fetch");
        assert_eq!(pascal_to_snake_case("HTTPResult"), "http_result");
    }

    #[test]
    fn test_infer_action_category() {
        assert_eq!(
            infer_action_category("SearchStart"),
            Some("search".to_string())
        );
        assert_eq!(
            infer_action_category("SearchQuerySubmit"),
            Some("search_query".to_string())
        );
        assert_eq!(
            infer_action_category("WeatherDidLoad"),
            Some("weather".to_string())
        );
        assert_eq!(
            infer_action_category("DidLoad"),
            Some("async_result".to_string())
        );
        assert_eq!(infer_action_category("Tick"), None);
        assert_eq!(infer_action_category("OpenConnectionForm"), None);
        assert_eq!(
            infer_action_category("APIFetchStart"),
            Some("api".to_string())
        );
    }
}
