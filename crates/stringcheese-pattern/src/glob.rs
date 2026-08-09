//! Glob pattern — wildcard + character classes.
//!
//! [`Glob`] extends [`crate::wildcard::Wildcard`] with POSIX-style
//! character classes:
//!
//! - `[abc]` — matches any one of `a`, `b`, `c`.
//! - `[a-z]` — matches any code point in the inclusive range.
//! - `[!abc]` or `[^abc]` — negated class; matches anything but.
//! - `[a-zA-Z0-9_]` — arbitrary combinations of ranges and singles.
//! - `\[`, `\]`, `\?`, `\*`, `\\` — escapes.
//!
//! Otherwise identical to `Wildcard`: `?` = one atom, `*` = zero
//! or more, everything else is literal.
//!
//! Whole-string matching by default; wrap with [`Glob::anywhere`]
//! for find-anywhere semantics.
//!
//! ## Implementation
//!
//! Wraps [`globset`] for pattern parsing and the [`regex`] crate for
//! matching, via a shared crate-private glob-engine pipeline (see
//! `src/glob_engine.rs`).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;

use regex::bytes::Regex;

use crate::glob_engine;
use crate::{Match, MatchUnit, Pattern};

/// A POSIX-style glob pattern.
#[derive(Clone, Debug)]
pub struct Glob<'p> {
    _pattern: &'p str,
    unit: MatchUnit,
    anchored: bool,
    regex: Arc<Regex>,
}

impl<'p> Glob<'p> {
    /// Whole-string glob: the pattern must match the entire haystack.
    ///
    /// # Panics
    ///
    /// Panics on [`MatchUnit::Graphemes`] — grapheme-level matching
    /// is deferred until the segmenter integration lands. Also
    /// panics on malformed glob syntax (unclosed class, bad
    /// backslash escape).
    #[must_use]
    pub fn new(pattern: &'p str, unit: MatchUnit) -> Self {
        Self::build(pattern, unit, true)
    }

    /// Find-anywhere glob: matches at any position in the haystack.
    #[must_use]
    pub fn anywhere(pattern: &'p str, unit: MatchUnit) -> Self {
        Self::build(pattern, unit, false)
    }

    fn build(pattern: &'p str, unit: MatchUnit, anchored: bool) -> Self {
        assert!(
            !matches!(unit, MatchUnit::Graphemes),
            "MatchUnit::Graphemes is reserved for the segmenter integration; not implemented",
        );
        let regex = glob_engine::compile(pattern, unit, anchored)
            .expect("glob pattern rejected by globset");
        Self {
            _pattern: pattern,
            unit,
            anchored,
            regex: Arc::new(regex),
        }
    }

    /// The [`MatchUnit`] this glob was constructed with.
    #[must_use]
    pub fn unit(&self) -> MatchUnit {
        self.unit
    }
}

impl Pattern for Glob<'_> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_matches_any_listed() {
        let pat = Glob::new("[abc]xy", MatchUnit::Bytes);
        assert!(pat.is_match("axy"));
        assert!(pat.is_match("bxy"));
        assert!(pat.is_match("cxy"));
        assert!(!pat.is_match("dxy"));
    }

    #[test]
    fn class_range() {
        let pat = Glob::new("[a-c]xy", MatchUnit::Bytes);
        assert!(pat.is_match("axy"));
        assert!(pat.is_match("bxy"));
        assert!(pat.is_match("cxy"));
        assert!(!pat.is_match("dxy"));
    }

    #[test]
    fn class_negated() {
        let pat = Glob::new("[!abc]xy", MatchUnit::Bytes);
        assert!(!pat.is_match("axy"));
        assert!(pat.is_match("dxy"));
    }

    #[test]
    fn combined_ranges_and_singles() {
        let pat = Glob::new("[a-zA-Z_][a-zA-Z0-9_]*", MatchUnit::Bytes);
        assert!(pat.is_match("hello"));
        assert!(pat.is_match("_private"));
        assert!(pat.is_match("Foo42"));
        assert!(!pat.is_match("42foo")); // starts with digit
    }

    #[test]
    fn shell_style_extension_match() {
        let pat = Glob::new("*.[ch]", MatchUnit::Bytes);
        assert!(pat.is_match("foo.c"));
        assert!(pat.is_match("foo.h"));
        assert!(!pat.is_match("foo.rs"));
    }

    #[test]
    fn find_anywhere_yields_matches() {
        let pat = Glob::anywhere("[0-9]+", MatchUnit::Bytes);
        // Glob has no `+` metachar — `+` is literal, so this
        // matches the literal string "0+" through "9+", not a
        // regex-style repetition. Sanity-check literal semantics.
        assert!(pat.find("value 3+ hidden").is_some());
    }
}
