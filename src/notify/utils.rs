/// Wrap a string in single quotes, escaping any interior single quotes.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Escape a string for use inside an AppleScript double-quoted string.
pub fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sh_quote_plain() {
        assert_eq!(sh_quote("hello world"), "'hello world'");
    }

    #[test]
    fn sh_quote_with_single_quotes() {
        assert_eq!(sh_quote("it's fine"), r"'it'\''s fine'");
    }

    #[test]
    fn sh_quote_empty() {
        assert_eq!(sh_quote(""), "''");
    }

    #[test]
    fn sh_quote_only_single_quote() {
        assert_eq!(sh_quote("'"), r"''\'''");
    }

    #[test]
    fn applescript_escape_quotes() {
        assert_eq!(applescript_escape(r#"say "hi""#), r#"say \"hi\""#);
    }

    #[test]
    fn applescript_escape_backslash() {
        assert_eq!(applescript_escape(r"foo\bar"), r"foo\\bar");
    }

    #[test]
    fn applescript_escape_no_special_chars() {
        assert_eq!(applescript_escape("hello"), "hello");
    }
}
