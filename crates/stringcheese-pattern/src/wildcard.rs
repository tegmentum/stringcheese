//! Wildcard pattern — `?` for one atom, `*` for zero-or-more atoms.
//!
//! Shell-style, no character classes (those live in [`crate::glob`]).
//! Whole-string matching: the pattern must consume the entire
//! haystack. For "find anywhere" semantics, wrap with implicit
//! leading and trailing `*` via [`Wildcard::anywhere`].
//!
//! ## Semantics
//!
//! - `?` — matches exactly one atom (byte or code point,
//!   per [`MatchUnit`]).
//! - `*` — matches zero or more atoms, greedy (regex-style).
//! - Every other character matches itself literally.
//! - Escape wildcards with `\?` and `\*`; `\\` is a literal
//!   backslash.
//!
//! ## Implementation
//!
//! Wraps [`globset`] for parsing + the [`regex`] crate for matching.
//! `[` and `]` in the input are escaped to literals before handing
//! the pattern to globset so wildcard-only semantics survive — the
//! character-class grammar lives in [`crate::glob`].

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;

use regex::bytes::Regex;

use crate::glob_engine;
use crate::{Match, MatchUnit, Pattern};

/// A wildcard pattern.
///
/// Compiles the pattern to a regex at construction time via
/// `globset` and reuses that compilation across all match calls.
#[derive(Clone, Debug)]
pub struct Wildcard<'p> {
    _pattern: &'p str,
    unit: MatchUnit,
    anchored: bool,
    regex: Arc<Regex>,
}

impl<'p> Wildcard<'p> {
    /// Whole-string wildcard: the pattern must match the entire
    /// haystack.
    ///
    /// # Panics
    ///
    /// Panics on [`MatchUnit::Graphemes`] (segmenter integration
    /// hasn't landed yet), or when the pattern is malformed in a
    /// way globset rejects (unterminated backslash escape, etc.).
    #[must_use]
    pub fn new(pattern: &'p str, unit: MatchUnit) -> Self {
        Self::build(pattern, unit, true)
    }

    /// Find-anywhere wildcard: matches at any position in the
    /// haystack. Equivalent to wrapping the pattern in leading and
    /// trailing `*`.
    #[must_use]
    pub fn anywhere(pattern: &'p str, unit: MatchUnit) -> Self {
        Self::build(pattern, unit, false)
    }

    fn build(pattern: &'p str, unit: MatchUnit, anchored: bool) -> Self {
        assert!(
            !matches!(unit, MatchUnit::Graphemes),
            "MatchUnit::Graphemes is reserved for the segmenter integration; not implemented",
        );
        let escaped = escape_brackets(pattern);
        let regex = glob_engine::compile(&escaped, unit, anchored)
            .expect("wildcard pattern rejected by globset");
        Self {
            _pattern: pattern,
            unit,
            anchored,
            regex: Arc::new(regex),
        }
    }
}

impl Pattern for Wildcard<'_> {
    fn is_match(&self, haystack: &str) -> bool {
        glob_engine::is_match(&self.regex, haystack)
    }

    fn find_iter<'h>(&self, haystack: &'h str) -> Box<dyn Iterator<Item = Match<'h>> + 'h> {
        glob_engine::find_iter(&self.regex, haystack, self.anchored)
    }

    fn replace_all(&self, haystack: &str, replacement: &str) -> String {
        let mut out = String::with_capacity(haystack.len());
        let mut cursor = 0usize;
        for m in self.find_iter(haystack) {
            out.push_str(&haystack[cursor..m.start]);
            out.push_str(replacement);
            cursor = m.end;
        }
        out.push_str(&haystack[cursor..]);
        out
    }
}

impl Wildcard<'_> {
    /// The [`MatchUnit`] this wildcard was constructed with.
    #[must_use]
    pub fn unit(&self) -> MatchUnit {
        self.unit
    }
}

/// Escape `[` and `]` in a wildcard input so globset treats them as
/// literals (globset would otherwise open a character class — that's
/// `Glob`'s job, not `Wildcard`'s). Bare backslashes already-escaping
/// something else pass through untouched.
fn escape_brackets(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let bytes = pattern.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\' && i + 1 < bytes.len() {
            // Pass escape sequences through unchanged.
            out.push('\\');
            let step = utf8_char_width(bytes[i + 1]).max(1);
            out.push_str(&pattern[i + 1..i + 1 + step]);
            i += 1 + step;
        } else if b == b'[' || b == b']' {
            out.push('\\');
            out.push(b as char);
            i += 1;
        } else {
            let step = utf8_char_width(b).max(1);
            out.push_str(&pattern[i..i + step]);
            i += step;
        }
    }
    out
}

const fn utf8_char_width(b: u8) -> usize {
    match b {
        0x00..=0x7F => 1,
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchored_whole_string() {
        let pat = Wildcard::new("h?llo", MatchUnit::Bytes);
        assert!(pat.is_match("hello"));
        assert!(pat.is_match("hallo"));
        assert!(!pat.is_match("helloo"));
        assert!(!pat.is_match("heello"));
    }

    #[test]
    fn star_matches_any_length() {
        let pat = Wildcard::new("h*o", MatchUnit::Bytes);
        assert!(pat.is_match("ho"));
        assert!(pat.is_match("hello"));
        assert!(pat.is_match("hoooooo"));
        assert!(!pat.is_match("hey"));
    }

    #[test]
    fn multiple_stars() {
        let pat = Wildcard::new("*.rs", MatchUnit::Bytes);
        assert!(pat.is_match("lib.rs"));
        assert!(pat.is_match("path/to/file.rs"));
        assert!(!pat.is_match("Cargo.toml"));
    }

    #[test]
    fn escape_wildcards() {
        let pat = Wildcard::new(r"a\?b", MatchUnit::Bytes);
        assert!(pat.is_match("a?b"));
        assert!(!pat.is_match("axb"));
    }

    #[test]
    fn find_anywhere_yields_matches() {
        let pat = Wildcard::anywhere("f?o", MatchUnit::Bytes);
        let hits: alloc::vec::Vec<_> = pat.find_iter("fooXbarXfyo").collect();
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn codepoints_unit_matches_scalar_width() {
        // ? matches one code point, so a 3-byte scalar counts as
        // one "?" — not three.
        let pat = Wildcard::new("?本語", MatchUnit::CodePoints);
        assert!(pat.is_match("日本語"));
        assert!(pat.is_match("中本語"));
        assert!(!pat.is_match("日日本語")); // extra scalar
    }

    #[test]
    fn brackets_in_wildcard_are_literal() {
        // Wildcard has no character classes — `[` and `]` are just
        // characters. Glob handles class semantics.
        let pat = Wildcard::new("[abc]", MatchUnit::Bytes);
        assert!(pat.is_match("[abc]"));
        assert!(!pat.is_match("a"));
    }
}
