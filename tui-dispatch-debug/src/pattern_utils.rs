/// Split a comma-separated pattern list into trimmed non-empty values.
pub(crate) fn split_patterns_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_patterns_csv() {
        let parsed = split_patterns_csv("  Search* , , Tick ,name:Foo  ");
        assert_eq!(
            parsed,
            vec![
                "Search*".to_string(),
                "Tick".to_string(),
                "name:Foo".to_string()
            ]
        );
    }
}
