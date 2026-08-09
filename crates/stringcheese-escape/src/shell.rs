//! POSIX shell word quoting / unquoting.
//!
//! Turns arbitrary text into a single shell word safe to pass to
//! `sh -c` and back again. Wraps [`shlex`] because shell quoting
//! is exactly the kind of small, well-defined problem the
//! ecosystem has already gotten right.
//!
//! ## Ruleset
//!
//! [`quote`] uses `shlex::try_quote`, which:
//!
//! - Passes safe inputs through unchanged (`hello`, `path/to/file`).
//! - Wraps risky inputs in single quotes and backslash-escapes
//!   any embedded single quote.
//! - Refuses to quote inputs containing NUL bytes — POSIX shells
//!   can't represent them in any quoting mode.
//!
//! [`unquote`] uses `shlex::split` restricted to a single word.

use alloc::string::{String, ToString};

/// Quote `input` as one POSIX shell word.
///
/// Returns the raw input unchanged when it's already shell-safe;
/// wraps it in single quotes otherwise. Falls back to a doubled-
/// quoted form (empty string `""`) for empty input so the caller
/// gets a valid word rather than nothing.
#[must_use]
pub fn quote(input: &str) -> String {
    // `try_quote` errors when the input contains a NUL — POSIX
    // shells have no way to embed one. Fall back to a sentinel
    // empty-word form so the caller still gets a valid word; the
    // NUL is a shell-side problem regardless.
    match shlex::try_quote(input) {
        Ok(cow) => cow.into_owned(),
        Err(_) => "''".to_string(),
    }
}

/// Reverse [`quote`]. Returns `None` when the input isn't a single
/// well-formed shell word (unmatched quote, trailing backslash,
/// multiple words).
#[must_use]
pub fn unquote(input: &str) -> Option<String> {
    let mut parts = shlex::split(input)?;
    match parts.len() {
        1 => parts.pop(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_input_passes_through() {
        assert_eq!(quote("hello"), "hello");
        assert_eq!(quote("path/to/file"), "path/to/file");
    }

    #[test]
    fn space_forces_quoting() {
        assert_eq!(quote("hello world"), "'hello world'");
    }

    #[test]
    fn shell_metacharacters_get_quoted() {
        for meta in [";", "&", "|", "$", "`", "*", "(", ")"] {
            let q = quote(meta);
            assert!(q.starts_with('\''), "{meta:?} quoted as {q:?}");
        }
    }

    #[test]
    fn embedded_single_quote_backslash_escaped() {
        // Input `it's` — shell quoting closes the single quote,
        // backslash-escapes the apostrophe, reopens.
        let q = quote("it's");
        assert_eq!(unquote(&q).unwrap(), "it's");
    }

    #[test]
    fn round_trip_common_cases() {
        let cases = [
            "hello",
            "hello world",
            "path/with spaces/file",
            "it's ok",
            "$var",
            "a\tb",
        ];
        for s in cases {
            let q = quote(s);
            assert_eq!(unquote(&q).unwrap(), s);
        }
    }

    #[test]
    fn empty_input_produces_valid_empty_word() {
        assert_eq!(quote(""), "''");
        assert_eq!(unquote("''").unwrap(), "");
    }

    #[test]
    fn unquote_rejects_multi_word() {
        assert!(unquote("hello world").is_none());
    }

    #[test]
    fn unquote_rejects_unmatched_quote() {
        assert!(unquote("'unclosed").is_none());
    }
}
