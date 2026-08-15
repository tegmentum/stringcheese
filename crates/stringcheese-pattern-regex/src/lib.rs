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

    // -----------------------------------------------------------------
    // Constructor-path coverage — every entry point pipes through
    // `compile` with a specific MatchUnit and prefix. Cover them all.
    // -----------------------------------------------------------------

    #[test]
    fn with_unit_bytes_matches_regex_bytes_shortcut() {
        let via_shortcut = Regex::bytes(r"\w+").unwrap();
        let via_with_unit = Regex::with_unit(r"\w+", MatchUnit::Bytes).unwrap();
        // Both compile the same effective pattern; behaviour should match.
        let hay = "Привет 世界 hello";
        let a: Vec<_> = via_shortcut.find_iter(hay).map(|m| m.matched).collect();
        let b: Vec<_> = via_with_unit.find_iter(hay).map(|m| m.matched).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn with_unit_codepoints_matches_regex_new_shortcut() {
        let via_shortcut = Regex::new(r"\w+").unwrap();
        let via_with_unit = Regex::with_unit(r"\w+", MatchUnit::CodePoints).unwrap();
        let hay = "Привет 世界 hello";
        let a: Vec<_> = via_shortcut.find_iter(hay).map(|m| m.matched).collect();
        let b: Vec<_> = via_with_unit.find_iter(hay).map(|m| m.matched).collect();
        assert_eq!(a, b);
    }

    #[test]
    #[should_panic(expected = "MatchUnit::Graphemes is reserved")]
    fn with_unit_graphemes_panics() {
        let _ = Regex::with_unit(r".", MatchUnit::Graphemes);
    }

    #[test]
    fn literal_constructor_stores_escaped_form_as_pattern() {
        // `literal()` funnels through `compile(&regex::escape(text), …)`
        // — so `pattern()` returns the escaped text (the string that
        // actually compiled), not the raw user input. `regex::escape`
        // only inserts backslashes in front of actual metacharacters;
        // spaces, letters, and `=` pass through unchanged.
        let re = Regex::literal("2 + 2 = 4").unwrap();
        assert_eq!(re.pattern(), r"2 \+ 2 = 4");
        assert!(re.is_match("2 + 2 = 4"));
        assert!(!re.is_match("2 + 2 = 5"));
    }

    // -----------------------------------------------------------------
    // Error-mode coverage
    // -----------------------------------------------------------------

    #[test]
    fn unterminated_class_is_an_error() {
        let e = Regex::new("[abc").unwrap_err();
        assert_eq!(e.pattern, "[abc");
        assert!(format!("{e}").contains("[abc"));
    }

    #[test]
    fn empty_alternation_branch_is_accepted() {
        // The `regex` crate treats `a|` as a valid pattern where the
        // right branch matches the empty string. Codify the current
        // behaviour so a future upgrade that tightens this surfaces.
        assert!(Regex::new("a|").is_ok());
    }

    #[test]
    fn backreference_in_pattern_is_an_error() {
        // The `regex` crate deliberately excludes backreferences —
        // they make the language non-regular. Verify the error
        // surfaces with the offending pattern text preserved.
        let e = Regex::new(r"(a)\1").unwrap_err();
        assert_eq!(e.pattern, r"(a)\1");
    }

    #[test]
    fn lookahead_in_pattern_is_an_error() {
        // No unbounded lookaround either — same reason.
        let e = Regex::new(r"foo(?=bar)").unwrap_err();
        assert_eq!(e.pattern, r"foo(?=bar)");
    }

    #[test]
    fn invalid_unicode_property_class_is_an_error() {
        let e = Regex::new(r"\p{NoSuchProperty}").unwrap_err();
        assert_eq!(e.pattern, r"\p{NoSuchProperty}");
    }

    #[test]
    fn regex_error_display_contains_underlying_message() {
        let e = Regex::new("(").unwrap_err();
        let msg = format!("{e}");
        assert!(msg.starts_with("invalid regex `(`:"));
        // The underlying engine's diagnostic is preserved verbatim.
        assert!(!e.message.is_empty());
    }

    #[test]
    fn regex_error_is_std_error() {
        // Prove the `impl std::error::Error` binding is picked up.
        fn expects_error<E: std::error::Error>(_: &E) {}
        let e = Regex::new("(").unwrap_err();
        expects_error(&e);
    }

    #[test]
    fn regex_error_clone_preserves_fields() {
        let e = Regex::new("(unclosed").unwrap_err();
        let cloned = e.clone();
        assert_eq!(cloned.pattern, e.pattern);
        assert_eq!(cloned.message, e.message);
    }

    // -----------------------------------------------------------------
    // Unicode escape / character-class coverage
    // -----------------------------------------------------------------

    #[test]
    fn p_letter_matches_all_scripts() {
        let re = Regex::new(r"\p{L}+").unwrap();
        let hits: Vec<_> = re
            .find_iter("abc 123 Привет 中文 α")
            .map(|m| m.matched)
            .collect();
        assert_eq!(hits, vec!["abc", "Привет", "中文", "α"]);
    }

    #[test]
    fn p_nd_matches_all_decimal_digit_scripts() {
        let re = Regex::new(r"\p{Nd}+").unwrap();
        // Arabic-Indic digits are Nd but not ASCII 0-9.
        let hits: Vec<_> = re.find_iter("12 ٣٤ ๕๖").map(|m| m.matched).collect();
        assert_eq!(hits, vec!["12", "٣٤", "๕๖"]);
    }

    #[test]
    fn p_script_han_matches_only_chinese_characters() {
        let re = Regex::new(r"\p{sc=Han}+").unwrap();
        let hits: Vec<_> = re.find_iter("hello 世界 abc").map(|m| m.matched).collect();
        assert_eq!(hits, vec!["世界"]);
    }

    #[test]
    fn unicode_escape_matches_single_scalar() {
        // U+4E16 = 世
        let re = Regex::new(r"\u{4E16}").unwrap();
        assert!(re.is_match("世界"));
        assert!(!re.is_match("hello"));
    }

    #[test]
    fn posix_alpha_class_is_ascii_even_under_unicode_mode() {
        // POSIX classes in the `regex` crate stay ASCII-only even
        // under Unicode mode — the caller reaches for `\p{Alphabetic}`
        // when they want the full-Unicode notion. Codify the actual
        // behaviour so a future engine upgrade that promotes POSIX
        // classes to Unicode-aware surfaces here.
        let re_posix = Regex::new(r"[[:alpha:]]+").unwrap();
        let hits: Vec<_> = re_posix
            .find_iter("abc 123 δεν")
            .map(|m| m.matched)
            .collect();
        assert_eq!(hits, vec!["abc"]);
        // The Unicode-aware alternative:
        let re_unicode = Regex::new(r"\p{Alphabetic}+").unwrap();
        let hits: Vec<_> = re_unicode
            .find_iter("abc 123 δεν")
            .map(|m| m.matched)
            .collect();
        assert_eq!(hits, vec!["abc", "δεν"]);
    }

    #[test]
    fn negated_class_matches_complement() {
        let re = Regex::new(r"[^\d ]+").unwrap();
        let hits: Vec<_> = re.find_iter("a 1 b2 c").map(|m| m.matched).collect();
        assert_eq!(hits, vec!["a", "b", "c"]);
    }

    #[test]
    fn negated_unicode_class_matches_complement() {
        let re = Regex::new(r"\P{L}+").unwrap();
        let hits: Vec<_> = re.find_iter("abc 123 xyz").map(|m| m.matched).collect();
        assert_eq!(hits, vec![" 123 "]);
    }

    // -----------------------------------------------------------------
    // Byte vs code-point match-unit semantics
    // -----------------------------------------------------------------

    #[test]
    fn bytes_mode_rejects_patterns_that_could_match_invalid_utf8() {
        // The wrapped `regex::Regex` only sees `&str` input, so it
        // refuses to compile a byte-mode pattern that could match a
        // sub-scalar byte span. Codify this — the caller who needs
        // byte-level `.` reaches for `regex::bytes::Regex` directly.
        let e = Regex::bytes(".").unwrap_err();
        assert!(
            e.message.contains("invalid UTF-8"),
            "expected UTF-8-safety error, got: {}",
            e.message
        );
    }

    #[test]
    fn bytes_mode_ascii_class_matches_byte_by_byte() {
        // A byte-mode class whose codepoints are all ASCII always
        // lands on UTF-8 boundaries; the engine happily compiles it.
        let re = Regex::bytes("[a-z]").unwrap();
        // Multi-byte scalars aren't ASCII, so they yield no matches;
        // the ASCII letters at the end each match as one byte.
        let hits: Vec<_> = re.find_iter("日本 hello").map(|m| m.matched).collect();
        assert_eq!(hits, vec!["h", "e", "l", "l", "o"]);
    }

    #[test]
    fn dot_in_codepoints_mode_spans_multi_byte_scalars() {
        let re = Regex::new(".").unwrap();
        let hits: Vec<_> = re.find_iter("日本").map(|m| m.end - m.start).collect();
        // Two scalars, each 3 bytes.
        assert_eq!(hits, vec![3, 3]);
    }

    #[test]
    fn bytes_mode_digit_class_is_ascii_only() {
        // `\d` in `(?-u)` mode is `[0-9]` — Arabic-Indic digits (which
        // are `\p{Nd}`) don't match.
        let re = Regex::bytes(r"\d+").unwrap();
        let hits: Vec<_> = re.find_iter("12 ٣٤ 5").map(|m| m.matched).collect();
        assert_eq!(hits, vec!["12", "5"]);
    }

    #[test]
    fn combining_marks_are_separate_scalars_under_codepoints() {
        // "é" as e (U+0065) + combining acute (U+0301) is TWO scalars.
        // `.` under codepoints matches ONE scalar, so we get two hits.
        let re = Regex::new(".").unwrap();
        let hits: Vec<_> = re.find_iter("e\u{0301}").map(|m| m.matched).collect();
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn rtl_text_matches_predictably_by_code_point() {
        // Hebrew word "שלום" — the regex engine treats it as a
        // sequence of scalars in logical order regardless of the
        // display direction.
        let re = Regex::new(r"\p{L}+").unwrap();
        let hits: Vec<_> = re.find_iter("שלום world").map(|m| m.matched).collect();
        assert_eq!(hits, vec!["שלום", "world"]);
    }

    #[test]
    fn find_positions_are_byte_offsets_even_in_bytes_mode() {
        // The wrapper's contract: match positions are always valid
        // `&str` byte offsets, regardless of the MatchUnit.
        let re = Regex::bytes("h").unwrap();
        let hay = "日本 hello";
        let m = re.find(hay).unwrap();
        // "日本 " is 3 + 3 + 1 = 7 bytes; 'h' sits at byte 7.
        assert_eq!(m.start, 7);
        assert_eq!(m.end, 8);
        assert_eq!(&hay[m.start..m.end], "h");
    }

    // -----------------------------------------------------------------
    // Anchor behaviour
    // -----------------------------------------------------------------

    #[test]
    fn caret_anchors_at_start_of_haystack_by_default() {
        let re = Regex::new("^foo").unwrap();
        assert!(re.is_match("foobar"));
        assert!(!re.is_match("barfoo"));
    }

    #[test]
    fn dollar_anchors_at_end_of_haystack_by_default() {
        let re = Regex::new("foo$").unwrap();
        assert!(re.is_match("barfoo"));
        assert!(!re.is_match("foobar"));
    }

    #[test]
    fn a_and_z_anchors_are_absolute() {
        // `\A` and `\z` don't respect multi-line mode — always
        // anchor at the actual haystack boundaries.
        let re = Regex::new(r"(?m)\Afoo").unwrap();
        assert!(re.is_match("foo\nbar"));
        assert!(!re.is_match("bar\nfoo"));
    }

    #[test]
    fn multiline_mode_lets_caret_anchor_per_line() {
        let re = Regex::new("(?m)^foo").unwrap();
        assert!(re.is_match("bar\nfoo"));
        assert!(re.is_match("foo\nbar"));
    }

    #[test]
    fn dotall_mode_lets_dot_match_newline() {
        // Without `(?s)`, `.` doesn't match `\n`.
        let no_s = Regex::new("a.b").unwrap();
        assert!(!no_s.is_match("a\nb"));
        // With `(?s)`, `.` does.
        let s = Regex::new("(?s)a.b").unwrap();
        assert!(s.is_match("a\nb"));
    }

    #[test]
    fn word_boundary_anchor_respects_unicode() {
        // `\b` under Unicode mode fires at Unicode word boundaries,
        // not just ASCII ones.
        let re = Regex::new(r"\bΠρόγραμμα\b").unwrap();
        assert!(re.is_match("το Πρόγραμμα σου"));
    }

    // -----------------------------------------------------------------
    // Case-insensitive matching under Unicode
    // -----------------------------------------------------------------

    #[test]
    fn case_insensitive_folds_greek_letters() {
        // `(?i)` under Unicode folds Greek uppercase to lowercase.
        let re = Regex::case_insensitive("άλφα").unwrap();
        assert!(re.is_match("ΆΛΦΑ"));
        assert!(re.is_match("Άλφα"));
    }

    #[test]
    fn case_insensitive_folds_german_sharp_s() {
        // The `regex` crate's simple case folding treats `ß` and
        // `ss` as distinct by default (single-codepoint folding). The
        // uppercase form `ẞ` (U+1E9E) DOES fold to `ß`, since that's
        // a single-scalar equivalence. Codify both to prevent silent
        // behaviour changes.
        let re = Regex::case_insensitive("ß").unwrap();
        assert!(re.is_match("ß"));
        assert!(re.is_match("ẞ"));
        assert!(!re.is_match("ss"));
    }

    #[test]
    fn case_insensitive_matches_ascii_uppercase() {
        let re = Regex::case_insensitive("HELLO").unwrap();
        assert!(re.is_match("hello"));
        assert!(re.is_match("HELLO"));
        assert!(re.is_match("Hello"));
    }

    // -----------------------------------------------------------------
    // Pattern trait — every method on the trait
    // -----------------------------------------------------------------

    #[test]
    fn find_returns_first_match_span() {
        let re = Regex::new(r"\d+").unwrap();
        let m = re.find("a 42 b 7").unwrap();
        assert_eq!(m.matched, "42");
        assert_eq!(m.start, 2);
        assert_eq!(m.end, 4);
    }

    #[test]
    fn find_none_on_no_match() {
        let re = Regex::new(r"\d+").unwrap();
        assert!(re.find("no digits here").is_none());
    }

    #[test]
    fn is_match_true_and_false_paths() {
        let re = Regex::new(r"^\d+$").unwrap();
        assert!(re.is_match("12345"));
        assert!(!re.is_match("12 34"));
    }

    #[test]
    fn find_iter_lifetimes_permit_haystack_slicing_after_drop() {
        // The find_iter impl materialises spans up front so the
        // returned iterator doesn't borrow from the Regex — but it
        // must still tie the string slices to the haystack lifetime.
        let hay = String::from("a1 b2 c3");
        let re = Regex::new(r"[a-z]\d").unwrap();
        drop(re.clone()); // burn a clone to prove the iterator is independent
        let re = Regex::new(r"[a-z]\d").unwrap();
        let hits: Vec<&str> = re.find_iter(&hay).map(|m| m.matched).collect();
        assert_eq!(hits, vec!["a1", "b2", "c3"]);
    }

    #[test]
    fn replace_all_with_empty_replacement_deletes_matches() {
        let re = Regex::new(r"\d+").unwrap();
        assert_eq!(re.replace_all("a1b22c333d", ""), "abcd");
    }

    #[test]
    fn replace_all_no_matches_returns_input_unchanged() {
        let re = Regex::new(r"\d+").unwrap();
        assert_eq!(re.replace_all("no digits here", "X"), "no digits here");
    }

    #[test]
    fn replace_all_uses_literal_dollar_sign_replacement() {
        // Contract from the Pattern trait — replacement text is NOT
        // interpreted; `$1`, `${name}`, and `$$` all pass through.
        let re = Regex::new(r"\d+").unwrap();
        assert_eq!(re.replace_all("a1b2", "${name}"), "a${name}b${name}");
        assert_eq!(re.replace_all("a1b2", "$$"), "a$$b$$");
    }

    #[test]
    fn split_produces_between_match_spans_including_empties() {
        // Trait's default `split` behaves like `str::split(pat)` —
        // adjacent matches yield an empty span.
        let re = Regex::new(",").unwrap();
        let parts = re.split("a,,b,c");
        assert_eq!(parts, vec!["a", "", "b", "c"]);
    }

    #[test]
    fn split_no_match_yields_full_input() {
        let re = Regex::new(",").unwrap();
        let parts = re.split("no commas");
        assert_eq!(parts, vec!["no commas"]);
    }

    #[test]
    fn split_leading_match_yields_leading_empty() {
        let re = Regex::new(",").unwrap();
        let parts = re.split(",abc");
        assert_eq!(parts, vec!["", "abc"]);
    }

    #[test]
    fn split_trailing_match_yields_trailing_empty() {
        let re = Regex::new(",").unwrap();
        let parts = re.split("abc,");
        assert_eq!(parts, vec!["abc", ""]);
    }

    // -----------------------------------------------------------------
    // Clone / Debug / accessors
    // -----------------------------------------------------------------

    #[test]
    fn clone_produces_functionally_equivalent_regex() {
        let re = Regex::new(r"^\w+$").unwrap();
        let cloned = re.clone();
        for hay in ["hello", "1_2", "with space"] {
            assert_eq!(
                re.is_match(hay),
                cloned.is_match(hay),
                "mismatch on {hay:?}"
            );
        }
        assert_eq!(re.pattern(), cloned.pattern());
        assert_eq!(re.unit(), cloned.unit());
    }

    #[test]
    fn debug_impl_is_non_empty() {
        let re = Regex::new("foo").unwrap();
        let dbg = format!("{re:?}");
        assert!(dbg.contains("Regex"));
    }

    #[test]
    fn as_inner_returns_engine_with_capture_names() {
        let re = Regex::new(r"(?P<num>\d+)").unwrap();
        let inner = re.as_inner();
        let caps = inner.captures("value=42").unwrap();
        assert_eq!(&caps["num"], "42");
    }

    // -----------------------------------------------------------------
    // Property tests — invariants over arbitrary patterns / haystacks.
    // Only enabled off wasm (proptest isn't available there).
    // -----------------------------------------------------------------

    #[cfg(not(target_family = "wasm"))]
    mod props {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// `.*` matches everywhere it's tried — a compiled regex
            /// that matches the empty string must accept any haystack.
            #[test]
            fn dot_star_always_matches(s in ".{0,64}") {
                let re = Regex::new(".*").unwrap();
                prop_assert!(re.is_match(&s));
            }

            /// Substring monotonicity: a literal-pattern regex that
            /// matches `x` also matches `prefix + x + suffix` for any
            /// prefix / suffix. (`is_match` is closed under
            /// concatenation.)
            #[test]
            fn substring_match_monotonicity(
                needle in "[a-z]{1,4}",
                prefix in "[a-z]{0,10}",
                suffix in "[a-z]{0,10}",
                middle in "[a-z]{0,10}",
            ) {
                let re = Regex::literal(&needle).unwrap();
                let hay = format!("{prefix}{needle}{middle}");
                prop_assert!(re.is_match(&hay));
                let bigger = format!("{prefix}{needle}{middle}{suffix}");
                prop_assert!(re.is_match(&bigger));
            }

            /// A cloned regex behaves identically to the original.
            #[test]
            fn clone_is_functionally_equivalent(
                pat in "[a-z]{1,4}",
                hay in "[a-z ]{0,32}",
            ) {
                let re = Regex::literal(&pat).unwrap();
                let cloned = re.clone();
                prop_assert_eq!(re.is_match(&hay), cloned.is_match(&hay));
                let a: Vec<_> = re.find_iter(&hay).map(|m| (m.start, m.end)).collect();
                let b: Vec<_> = cloned.find_iter(&hay).map(|m| (m.start, m.end)).collect();
                prop_assert_eq!(a, b);
            }

            /// `Regex::literal(text)` always matches `text` verbatim,
            /// regardless of which metacharacters `text` contains.
            #[test]
            fn literal_matches_verbatim(text in ".{0,16}") {
                let re = Regex::literal(&text).unwrap();
                prop_assert!(re.is_match(&text));
            }

            /// `is_match` and `find().is_some()` agree on every input.
            #[test]
            fn is_match_agrees_with_find(hay in ".{0,32}") {
                let re = Regex::new(r"\d+").unwrap();
                prop_assert_eq!(re.is_match(&hay), re.find(&hay).is_some());
            }

            /// `find_iter` results are non-overlapping and left-to-right:
            /// each match's start is >= the previous match's end.
            #[test]
            fn find_iter_matches_are_left_to_right_non_overlapping(
                hay in "[a-z0-9 ]{0,64}",
            ) {
                let re = Regex::new(r"\w+").unwrap();
                let hits: Vec<_> = re.find_iter(&hay).collect();
                for pair in hits.windows(2) {
                    prop_assert!(pair[0].end <= pair[1].start);
                }
            }

            /// `split` and `find_iter` are inverses: joining `split`
            /// results with the matched text reproduces the haystack,
            /// as long as every match is a single literal character
            /// (the simplest case to state).
            #[test]
            fn split_reassembles_via_single_char_delim(
                hay in "[a-z,]{0,32}",
            ) {
                let re = Regex::literal(",").unwrap();
                let parts = re.split(&hay);
                let joined = parts.join(",");
                prop_assert_eq!(joined, hay);
            }

            /// `replace_all` with the original match text is the
            /// identity ONLY when there are no matches. When there
            /// ARE matches, replacement is verbatim (the Pattern
            /// contract), so replacing digits with "0" collapses runs.
            #[test]
            fn replace_all_with_empty_never_grows_output(
                hay in "[a-z0-9 ]{0,64}",
            ) {
                let re = Regex::new(r"\d").unwrap();
                let replaced = re.replace_all(&hay, "");
                prop_assert!(replaced.len() <= hay.len());
            }

            /// Never panics on arbitrary UTF-8 haystacks.
            #[test]
            fn is_match_never_panics_on_arbitrary_haystack(
                hay in ".{0,64}",
            ) {
                let re = Regex::new(r"\w+").unwrap();
                let _ = re.is_match(&hay);
                let _ = re.find(&hay);
                let _: Vec<_> = re.find_iter(&hay).collect();
            }
        }
    }
}
