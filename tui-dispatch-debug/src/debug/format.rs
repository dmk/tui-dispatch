use std::fmt::Debug;

pub fn debug_string<T>(value: &T) -> String
where
    T: Debug,
{
    debug_string_compact(value)
}

pub fn debug_string_compact<T>(value: &T) -> String
where
    T: Debug,
{
    format!("{:?}", value)
}

pub fn debug_string_pretty<T>(value: &T) -> String
where
    T: Debug,
{
    format!("{:#?}", value)
}

/// Reformat a compact Debug-style string with indentation and newlines.
///
/// Takes a single-line string like `{\"a\", \"b\", \"c\"}` and produces:
/// ```text
/// {
///     "a",
///     "b",
///     "c",
/// }
/// ```
///
/// Handles nested `{}`, `[]`, `()` and preserves quoted strings.
pub fn pretty_reformat(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    let mut indent: usize = 0;
    let mut chars = input.chars().peekable();
    let indent_str = "    ";

    while let Some(ch) = chars.next() {
        match ch {
            // String literal — copy verbatim
            '"' => {
                out.push('"');
                loop {
                    match chars.next() {
                        Some('\\') => {
                            out.push('\\');
                            if let Some(esc) = chars.next() {
                                out.push(esc);
                            }
                        }
                        Some('"') => {
                            out.push('"');
                            break;
                        }
                        Some(c) => out.push(c),
                        None => break,
                    }
                }
            }
            // Char literal — copy verbatim. Debug chars look like 'x', '\n', '\'', '\u{7f}'.
            '\'' => {
                out.push('\'');
                loop {
                    match chars.next() {
                        Some('\\') => {
                            out.push('\\');
                            if let Some(esc) = chars.next() {
                                out.push(esc);
                            }
                        }
                        Some('\'') => {
                            out.push('\'');
                            break;
                        }
                        Some(c) => out.push(c),
                        None => break,
                    }
                }
            }
            '{' | '[' | '(' => {
                // Peek: if the matching close is next (empty container), keep inline
                let close = match ch {
                    '{' => '}',
                    '[' => ']',
                    _ => ')',
                };
                if chars.peek() == Some(&close) {
                    out.push(ch);
                    out.push(chars.next().unwrap());
                } else {
                    indent += 1;
                    out.push(ch);
                    out.push('\n');
                    for _ in 0..indent {
                        out.push_str(indent_str);
                    }
                }
            }
            '}' | ']' | ')' => {
                indent = indent.saturating_sub(1);
                // Trim trailing whitespace on current line
                let trimmed = out.trim_end_matches(' ');
                out.truncate(trimmed.len());
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                for _ in 0..indent {
                    out.push_str(indent_str);
                }
                out.push(ch);
            }
            ',' => {
                out.push(',');
                // If we're at indent > 0, break the line
                if indent > 0 {
                    out.push('\n');
                    for _ in 0..indent {
                        out.push_str(indent_str);
                    }
                } else {
                    out.push(' ');
                }
            }
            // Skip spaces after commas / braces (we handle our own whitespace)
            ' ' if out.ends_with('\n') || out.ends_with(indent_str) => {}
            _ => {
                out.push(ch);
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_reformat_simple_set() {
        let input = r#"{"a", "b", "c"}"#;
        let result = pretty_reformat(input);
        assert!(result.contains('\n'));
        assert!(result.contains("    \"a\","));
        assert!(result.contains("    \"b\","));
    }

    #[test]
    fn pretty_reformat_nested() {
        let input = "Foo{x: [1, 2], y: Bar{z: 3}}";
        let result = pretty_reformat(input);
        assert!(result.contains('\n'));
        // Check nesting increases indent
        assert!(result.contains("        z: 3"));
    }

    #[test]
    fn pretty_reformat_empty_containers() {
        assert_eq!(pretty_reformat("{}"), "{}");
        assert_eq!(pretty_reformat("[]"), "[]");
        assert_eq!(pretty_reformat("()"), "()");
    }

    #[test]
    fn pretty_reformat_preserves_strings() {
        let input = r#"{"hello, world", "foo{bar}"}"#;
        let result = pretty_reformat(input);
        assert!(result.contains(r#""hello, world""#));
        assert!(result.contains(r#""foo{bar}""#));
    }

    #[test]
    fn pretty_reformat_plain_value() {
        assert_eq!(pretty_reformat("42"), "42");
        assert_eq!(pretty_reformat("true"), "true");
    }

    #[test]
    fn pretty_reformat_preserves_char_literals() {
        // Char literals with structural chars inside must not be parsed as structure.
        assert_eq!(pretty_reformat("Some('}')"), "Some(\n    '}'\n)");

        let input = "Foo { x: Some('}'), y: 2 }";
        let result = pretty_reformat(input);
        assert!(result.contains("        '}'"));
        assert!(result.contains("    y: 2"));
    }

    #[test]
    fn pretty_reformat_preserves_escaped_char() {
        let input = r#"Foo { c: '\'', n: '\n' }"#;
        let result = pretty_reformat(input);
        assert!(result.contains(r#"'\''"#));
        assert!(result.contains(r#"'\n'"#));
    }
}
