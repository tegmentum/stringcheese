//! # StringCheese regex — the ergonomic wrapper
//!
//! Regex support plugged into `stringcheese-pattern`'s [`Pattern`]
//! trait, so callers dispatch through the same interface as
//! literal / wildcard / glob. Backed by the [`regex`] crate — a
//! finite-automata-oriented engine that matches our design
//! commitments (linear time, no backreferences, Unicode-aware,
//! WASM-compatible, safe on untrusted patterns). See
//! `docs/design/scope-and-decomposition.md` for the regex-subsystem
//! philosophy this crate implements.
//!
//! ## What this crate contributes
//!
//! Not the algorithm — the ecosystem already has an excellent one.
//! What StringCheese adds is the **developer experience**:
//!
//! - **One trait across every pattern kind.** `Regex`, [`Literal`],
//!   [`Wildcard`], [`Glob`] all implement [`Pattern`]. Swap between
//!   them without touching call sites.
//! - **Explicit Unicode semantic units.** [`MatchUnit`] on the
//!   constructor names whether `.` matches a byte or a code point.
//!   No guessing.
//! - **Ergonomic shortcuts** for the shapes callers actually reach
//!   for: [`Regex::new`] (Unicode default), [`Regex::bytes`] (ASCII
//!   byte-oriented), [`Regex::case_insensitive`], [`Regex::literal`]
//!   (auto-escapes untrusted user input into a fixed-string regex).
//! - **Error type that carries the pattern text and byte position**
//!   through [`RegexError`] — no need to cross-reference the
//!   underlying engine's error messages.
//!
//! [`Pattern`]: stringcheese_pattern::Pattern
//! [`Literal`]: stringcheese_pattern::Literal
//! [`Wildcard`]: stringcheese_pattern::Wildcard
//! [`Glob`]: stringcheese_pattern::Glob
//! [`MatchUnit`]: stringcheese_pattern::MatchUnit
//!
//! ## Design commitments — inherited from `regex`
//!
//! - **Finite-automata engine.** Guaranteed linear time in
//!   `|haystack| × |NFA|` — no backtracking pathologies.
//! - **No backreferences.** Backreferences make the language
//!   non-regular; the `regex` crate deliberately excludes them and
//!   StringCheese endorses that boundary.
//! - **No unbounded lookaround.** Same reason.
//! - **Unicode-aware by default.** `\w`, `\d`, `\s`, `\p{Letter}`,
//!   `\p{Script=Greek}`, boundary anchors — all supported through
//!   the wrapped engine.
//!
//! ## Example
//!
//! ```
//! use stringcheese_pattern::Pattern;
//! use stringcheese_pattern_regex::Regex;
//!
//! // Default constructor is Unicode-aware — `\w` matches Cyrillic
//! // and CJK letters too, not just ASCII.
//! let re = Regex::new(r"\w+")?;
//! let hits: Vec<_> = re.find_iter("Привет 世界 hello").collect();
//! assert_eq!(hits.len(), 3);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Interop with the raw `regex` crate
//!
//! Callers who need the underlying engine's non-trait surface
//! (capture groups, replace-with-callback, `Regex::split_n`) reach
//! for [`Regex::as_inner`] to get an `&regex::Regex`. This keeps
//! the wrapper thin — nothing here obscures what `regex` provides.

#![deny(unsafe_code)]

use stringcheese_pattern::{Match, MatchUnit, Pattern};

// ---------------------------------------------------------------------
// Public type
// ---------------------------------------------------------------------

/// A compiled regex.
///
/// Construct once via [`Regex::new`] (or the shortcut constructors)
/// and reuse across haystacks. Cheap to `Clone` — the underlying
/// `regex::Regex` uses an `Arc` internally.
#[derive(Debug, Clone)]
pub struct Regex {
    inner: regex::Regex,
    unit: MatchUnit,
    pattern: String,
}

impl Regex {
    /// Compile a Unicode-aware regex.
    ///
    /// `\w` / `\d` / `\s` and character classes match under the full
    /// Unicode definitions; `.` matches one Unicode scalar. This is
    /// what most callers want.
    ///
    /// # Errors
    ///
    /// Returns [`RegexError`] on syntactic problems, with the
    /// original pattern text preserved for diagnostics.
    pub fn new(pattern: &str) -> Result<Self, RegexError> {
        Self::compile(pattern, MatchUnit::CodePoints)
    }

    /// Compile a byte-oriented regex — `.` matches one byte, `\w` /
    /// `\d` / `\s` and character classes match only their ASCII
    /// members.
    ///
    /// Match positions returned by [`Pattern::find`] and
    /// [`Pattern::find_iter`] are still valid `str` byte offsets
    /// (the wrapped engine only sees `&str` input); the difference
    /// is purely in what each metacharacter matches.
    ///
    /// # Errors
    ///
    /// Returns [`RegexError`] on syntactic problems.
    pub fn bytes(pattern: &str) -> Result<Self, RegexError> {
        Self::compile(pattern, MatchUnit::Bytes)
    }

    /// Compile a regex with case-insensitive matching turned on.
    /// Equivalent to prefixing the pattern with `(?i)`.
    ///
    /// # Errors
    ///
    /// Returns [`RegexError`] on syntactic problems.
    pub fn case_insensitive(pattern: &str) -> Result<Self, RegexError> {
        Self::compile(&format!("(?i){pattern}"), MatchUnit::CodePoints)
    }

    /// Compile an already-fixed string as a regex — every regex
    /// metacharacter in `text` is escaped, so the resulting pattern
    /// matches `text` verbatim.
    ///
    /// Use this when the "pattern" comes from user input that you
    /// don't want to interpret as regex syntax (search boxes,
    /// filter strings, etc.). For an even lighter path when only
    /// substring semantics are needed, reach for
    /// `stringcheese_pattern::Literal` directly — it skips regex
    /// compilation entirely.
    ///
    /// # Errors
    ///
    /// Returns [`RegexError`] only in the pathological case where
    /// the escaped pattern still fails to compile (in practice this
    /// doesn't happen for arbitrary strings).
    pub fn literal(text: &str) -> Result<Self, RegexError> {
        Self::compile(&regex::escape(text), MatchUnit::CodePoints)
    }

    /// Compile a regex with an explicit [`MatchUnit`].
    ///
    /// # Errors
    ///
    /// Returns [`RegexError`] on syntactic problems.
    ///
    /// # Panics
    ///
    /// Panics on [`MatchUnit::Graphemes`] — grapheme-level regex
    /// needs the segmenter integration that hasn't landed yet.
    pub fn with_unit(pattern: &str, unit: MatchUnit) -> Result<Self, RegexError> {
        Self::compile(pattern, unit)
    }

    /// The pattern text this regex was compiled from.
    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// The [`MatchUnit`] this regex was compiled with.
    #[must_use]
    pub fn unit(&self) -> MatchUnit {
        self.unit
    }

    /// Access the underlying `regex::Regex` for capture-group
    /// extraction, replace-with-callback, and other features not
    /// exposed through the [`Pattern`] trait.
    #[must_use]
    pub fn as_inner(&self) -> &regex::Regex {
        &self.inner
    }

    // -----------------------------------------------------------------
    // Internal — one compile pipeline every constructor funnels into.
    // -----------------------------------------------------------------

    fn compile(pattern: &str, unit: MatchUnit) -> Result<Self, RegexError> {
        assert!(
            !matches!(unit, MatchUnit::Graphemes),
            "MatchUnit::Graphemes is reserved for the segmenter integration; not implemented",
        );
        // Byte mode: prepend `(?-u)` to disable the Unicode flag in
        // the wrapped engine. This makes `.` match one byte, `\d`
        // match `[0-9]` only, etc. Positions are still `str` byte
        // offsets — `regex` only sees `&str` input.
        let effective = match unit {
            MatchUnit::Bytes => format!("(?-u){pattern}"),
            MatchUnit::CodePoints => pattern.to_owned(),
            MatchUnit::Graphemes => unreachable!(),
        };
        let inner =
            regex::Regex::new(&effective).map_err(|e| RegexError::from_regex(pattern, &e))?;
        Ok(Self {
            inner,
            unit,
            pattern: pattern.to_owned(),
        })
    }
}

impl Pattern for Regex {
    fn is_match(&self, haystack: &str) -> bool {
        self.inner.is_match(haystack)
    }

    fn find<'h>(&self, haystack: &'h str) -> Option<Match<'h>> {
        self.inner.find(haystack).map(|m| Match {
            start: m.start(),
            end: m.end(),
            matched: m.as_str(),
        })
    }

    fn find_iter<'h>(&self, haystack: &'h str) -> Box<dyn Iterator<Item = Match<'h>> + 'h> {
        // Collect spans upfront so the returned iterator doesn't
        // borrow from `self.inner` (the underlying `regex::Regex`
        // find_iter borrows from the compiled program, which would
        // force `'self: 'h` on this method — a constraint the
        // Pattern trait doesn't grant). The materialised Vec is
        // small; each entry is two `usize`s.
        let spans: Vec<(usize, usize)> = self
            .inner
            .find_iter(haystack)
            .map(|m| (m.start(), m.end()))
            .collect();
        Box::new(spans.into_iter().map(move |(start, end)| Match {
            start,
            end,
            matched: &haystack[start..end],
        }))
    }

    fn replace_all(&self, haystack: &str, replacement: &str) -> String {
        // Escape `$` in replacement so it's treated as a literal
        // rather than a backreference — matches the shape of the
        // other Pattern impls (`Literal::replace_all` etc. don't
        // interpret replacement strings).
        self.inner
            .replace_all(haystack, regex::NoExpand(replacement))
            .into_owned()
    }
}

// ---------------------------------------------------------------------
// Error type — carries the pattern text through so diagnostics don't
// require the caller to reason in isolation from the source string.
// ---------------------------------------------------------------------

/// Regex compilation error.
///
/// Carries the pattern text alongside the underlying `regex` engine's
/// error so `Display` gives a self-contained diagnostic — no need to
/// remember which pattern threw or cross-reference the engine's error
/// messages.
#[derive(Debug, Clone)]
pub struct RegexError {
    /// The pattern text that failed to compile.
    pub pattern: String,
    /// The underlying `regex` engine's error message.
    pub message: String,
}

impl RegexError {
    fn from_regex(pattern: &str, err: &regex::Error) -> Self {
        Self {
            pattern: pattern.to_owned(),
            message: format!("{err}"),
        }
    }
}

impl core::fmt::Display for RegexError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "invalid regex `{}`: {}", self.pattern, self.message)
    }
}

impl std::error::Error for RegexError {}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Correctness ---------------------------------------------------

    #[test]
    fn unicode_word_matches_non_ascii() {
        let re = Regex::new(r"\w+").unwrap();
        let hits: Vec<_> = re.find_iter("Привет 世界 hello").collect();
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].matched, "Привет");
        assert_eq!(hits[1].matched, "世界");
        assert_eq!(hits[2].matched, "hello");
    }

    #[test]
    fn bytes_mode_ascii_only_word() {
        let re = Regex::bytes(r"\w+").unwrap();
        // \w in byte mode is `[A-Za-z0-9_]` only — Cyrillic and
        // CJK letters aren't in that class.
        let hits: Vec<_> = re.find_iter("Привет 世界 hello").collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].matched, "hello");
    }

    #[test]
    fn case_insensitive_matches_mixed_case() {
        let re = Regex::case_insensitive("hello").unwrap();
        assert!(re.is_match("HELLO"));
        assert!(re.is_match("HeLLo"));
        assert!(re.is_match("hello"));
    }

    #[test]
    fn literal_auto_escapes_metacharacters() {
        // Would be a class in raw regex — literal() escapes it.
        let re = Regex::literal("[a-z]").unwrap();
        assert!(re.is_match("prefix [a-z] suffix"));
        assert!(!re.is_match("prefix aaa suffix"));
    }

    #[test]
    fn dot_matches_one_atom_unicode() {
        let re = Regex::new(".本").unwrap();
        // `.` under Unicode mode matches one scalar — the multi-byte
        // 日 counts as one atom.
        assert!(re.is_match("日本"));
    }

    #[test]
    fn find_iter_multiple_hits() {
        let re = Regex::new(r"\d+").unwrap();
        let ns: Vec<&str> = re.find_iter("a12 b34 c56 d").map(|m| m.matched).collect();
        assert_eq!(ns, vec!["12", "34", "56"]);
    }

    #[test]
    fn replace_all_treats_replacement_as_literal() {
        // A raw `regex` `replace_all` would interpret `$1` as a
        // backreference. The Pattern trait's `replace_all` contract
        // says the replacement is literal — we honour that.
        let re = Regex::new(r"\d").unwrap();
        assert_eq!(re.replace_all("a1b2c3", "$1"), "a$1b$1c$1");
    }

    #[test]
    fn split_via_pattern_trait() {
        let re = Regex::new(r"\s+").unwrap();
        let parts = re.split("one   two three\tfour");
        assert_eq!(parts, vec!["one", "two", "three", "four"]);
    }

    // --- Errors --------------------------------------------------------

    #[test]
    fn error_carries_pattern_text() {
        let e = Regex::new("(unclosed").unwrap_err();
        assert_eq!(e.pattern, "(unclosed");
        // The Display includes both the pattern and the engine's message.
        let msg = format!("{e}");
        assert!(msg.contains("(unclosed"));
    }

    // --- Interop -------------------------------------------------------

    #[test]
    fn as_inner_exposes_underlying_regex() {
        let re = Regex::new(r"(\w+)\s+(\w+)").unwrap();
        // The `Pattern` trait doesn't do captures; drop down to the
        // underlying engine for that.
        let caps = re.as_inner().captures("hello world").unwrap();
        assert_eq!(&caps[1], "hello");
        assert_eq!(&caps[2], "world");
    }

    #[test]
    fn pattern_and_unit_are_preserved() {
        let re = Regex::new("foo").unwrap();
        assert_eq!(re.pattern(), "foo");
        assert_eq!(re.unit(), MatchUnit::CodePoints);

        let re = Regex::bytes("bar").unwrap();
        assert_eq!(re.pattern(), "bar");
        assert_eq!(re.unit(), MatchUnit::Bytes);
    }

    // --- Pattern trait consistency ------------------------------------

    #[test]
    fn regex_is_usable_via_pattern_trait_object() {
        // The whole point of the wrapper — any Pattern-typed API
        // accepts a Regex.
        let re: Box<dyn Pattern> = Box::new(Regex::new(r"\d+").unwrap());
        assert!(re.is_match("abc 42"));
    }

    // --- Non-goal: pathological input runs in bounded time ------------

    #[test]
    fn no_backtracking_pathology() {
        // The classic "regex DoS" pattern — the wrapped engine
        // matches this in linear time.
        let re = Regex::new("a?a?a?a?a?a?a?a?a?a?a?a?a?a?a?aaaaaaaaaaaaaaa").unwrap();
        assert!(re.is_match("aaaaaaaaaaaaaaa"));
    }
}
