//! Regex-based pre-tokenizer for BPE (Phase 2b).
//!
//! # Rationale
//!
//! `docs/design/tokenizers.md` § 5.1 calls out pre-tokenization as a
//! critical detail: tiktoken's `cl100k_base` / `o200k_base` variants
//! split input into "words" via a specific regex *before* the BPE
//! merge loop runs, and this pre-split is what keeps merges from
//! crossing letter/number/punctuation/whitespace boundaries. Without
//! it, `BpeTokenizer` cannot produce bit-identical tiktoken ids.
//!
//! Phase 2a shipped a literal-string separator only (see
//! [`PreTokenizerRegex::Literal`](crate::PreTokenizerRegex)). Phase 2b
//! (this module) delivers a real compiled-regex pre-tokenizer that
//! supports the canonical tiktoken pattern verbatim.
//!
//! # Regex backend choice
//!
//! The design doc § 12 discusses two candidates:
//!
//! * [`regex`] — the standard Rust regex crate. Small and fast, but
//!   does not support look-around. tiktoken's canonical pattern
//!   contains `\s+(?!\S)` (negative lookahead used to peel trailing
//!   whitespace before end-of-input separately from interior runs).
//! * [`regex-lite`] — no third-party deps, tiny code size, but no
//!   Unicode property classes: `\p{L}` and `\p{N}` are unavailable,
//!   which alone rules it out for the tiktoken pattern.
//! * [`fancy-regex`] — layers a backtracking VM on top of the
//!   `regex-automata` engine. Supports look-around and Unicode
//!   classes. This is the crate the upstream tiktoken Rust
//!   implementation uses.
//!
//! We pick [`fancy-regex`] because it is the only option that can
//! compile the canonical pattern verbatim. Callers who want the
//! smaller footprint of the plain `regex` crate can still supply
//! their own pre-tokenizer implementation via
//! [`PreTokenizerRegex::Literal`](crate::PreTokenizerRegex) or by
//! rebuilding this module against a different backend; the API here
//! is intentionally a thin wrapper.
//!
//! [`regex`]: https://docs.rs/regex/
//! [`regex-lite`]: https://docs.rs/regex-lite/
//! [`fancy-regex`]: https://docs.rs/fancy-regex/
//!
//! # `no_std` note
//!
//! `fancy-regex` requires the standard library. The entire module is
//! therefore gated behind `#[cfg(feature = "std")]`; the
//! `alloc`-only build of `stringcheese-tokenizer-bpe` retains the
//! literal-separator fallback and the whitespace default.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use fancy_regex::Regex;

/// The canonical tiktoken pre-tokenizer pattern used by both
/// `cl100k_base` (GPT-3.5 / GPT-4) and `o200k_base` (GPT-4o / o1) —
/// the two workhorse variants shipped by
/// `stringcheese-tokenizer-tiktoken`.
///
/// Verbatim from the upstream tiktoken repository:
///
/// * Contractions (`'s`, `'t`, `'re`, `'ve`, `'m`, `'ll`, `'d`) —
///   case-insensitive.
/// * A run of Unicode letters, optionally preceded by a single
///   non-letter / non-number / non-newline character (typically a
///   space).
/// * A run of one to three Unicode digits.
/// * A run of non-whitespace non-letter non-number (punctuation),
///   optionally preceded by a single space, optionally followed by
///   any number of newlines.
/// * A run of whitespace ending in one or more `[\r\n]`.
/// * A run of whitespace not followed by non-whitespace (the
///   "trailing whitespace at end-of-input" special case; the negative
///   lookahead is the reason we need `fancy-regex`).
/// * Any other run of whitespace.
///
/// Callers who need a different variant (e.g. `p50k_base` /
/// `r50k_base`, which use a slightly different GPT-2-shape pattern)
/// can construct their own via [`RegexPreTokenizer::new`].
pub const TIKTOKEN_CANONICAL_PATTERN: &str = concat!(
    r"(?i:'s|'t|'re|'ve|'m|'ll|'d)",
    r"|[^\r\n\p{L}\p{N}]?\p{L}+",
    r"|\p{N}{1,3}",
    r"| ?[^\s\p{L}\p{N}]+[\r\n]*",
    r"|\s*[\r\n]+",
    r"|\s+(?!\S)",
    r"|\s+",
);

/// The GPT-2 / `r50k_base` / `p50k_base` pre-tokenizer pattern.
///
/// Structurally the same as [`TIKTOKEN_CANONICAL_PATTERN`] but
/// omitting the `?i:` on the contractions alternative and using
/// slightly different whitespace handling — matching the original
/// GPT-2 byte-level BPE regex. Kept as a convenience constant so the
/// downstream tiktoken pack can select by variant.
pub const GPT2_PATTERN: &str = concat!(
    r"'s|'t|'re|'ve|'m|'ll|'d",
    r"| ?\p{L}+",
    r"| ?\p{N}+",
    r"| ?[^\s\p{L}\p{N}]+",
    r"|\s+(?!\S)",
    r"|\s+",
);

/// Error returned when a pre-tokenizer pattern fails to compile.
///
/// The wrapped string is the underlying `fancy-regex` diagnostic;
/// its exact format is not part of the stability contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreTokenizerCompileError(String);

impl PreTokenizerCompileError {
    /// The diagnostic message from the underlying regex engine.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PreTokenizerCompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid pre-tokenizer pattern: {}", self.0)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PreTokenizerCompileError {}

/// A compiled regex pre-tokenizer.
///
/// Constructed once via [`new`](Self::new) (or one of the
/// [`tiktoken_canonical`](Self::tiktoken_canonical) /
/// [`gpt2`](Self::gpt2) shortcuts), then reused to split every input
/// text via [`split`](Self::split) / [`split_ranges`](Self::split_ranges).
/// Compilation is not free (the fancy-regex VM has to build its state
/// tables) so callers should share one instance across encodes; the
/// type is `Send + Sync + Clone`.
///
/// # Semantics
///
/// [`split`](Self::split) returns every non-overlapping match of the
/// pattern in left-to-right order. Any input region *between*
/// matches — which for the tiktoken canonical pattern is only
/// possible if the pattern is misconfigured, since that pattern
/// covers all of Unicode — is silently dropped. This matches
/// upstream tiktoken's `re.findall(...)` semantics.
///
/// # Examples
///
/// ```
/// use stringcheese_tokenizer_bpe::RegexPreTokenizer;
///
/// let pre = RegexPreTokenizer::tiktoken_canonical();
/// let chunks: Vec<&str> = pre
///     .split("Hello, world!")
///     .into_iter()
///     .map(|(_off, s)| s)
///     .collect();
/// assert_eq!(chunks, ["Hello", ",", " world", "!"]);
/// ```
#[derive(Debug, Clone)]
pub struct RegexPreTokenizer {
    pattern: String,
    regex: Regex,
}

impl RegexPreTokenizer {
    /// Compile a pre-tokenizer from an arbitrary regex pattern.
    ///
    /// # Errors
    ///
    /// Returns [`PreTokenizerCompileError`] if the pattern is not a
    /// valid `fancy-regex` regular expression. The diagnostic is
    /// forwarded verbatim from the underlying engine.
    pub fn new(pattern: impl Into<String>) -> Result<Self, PreTokenizerCompileError> {
        let pattern = pattern.into();
        let regex = Regex::new(&pattern).map_err(|e| PreTokenizerCompileError(e.to_string()))?;
        Ok(Self { pattern, regex })
    }

    /// Convenience: compile [`TIKTOKEN_CANONICAL_PATTERN`].
    ///
    /// # Panics
    ///
    /// Panics if the hard-coded canonical pattern fails to compile,
    /// which would indicate a regression in the crate. A test
    /// (`tiktoken_canonical_compiles`) guards against this.
    #[must_use]
    pub fn tiktoken_canonical() -> Self {
        Self::new(TIKTOKEN_CANONICAL_PATTERN)
            .expect("the hard-coded canonical tiktoken pattern must compile")
    }

    /// Convenience: compile [`GPT2_PATTERN`].
    ///
    /// # Panics
    ///
    /// Panics if the hard-coded GPT-2 pattern fails to compile,
    /// which would indicate a regression in the crate.
    #[must_use]
    pub fn gpt2() -> Self {
        Self::new(GPT2_PATTERN).expect("the hard-coded GPT-2 pattern must compile")
    }

    /// The pattern this pre-tokenizer was compiled from.
    #[must_use]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Split `text` into pre-tokenizer chunks.
    ///
    /// Each returned tuple is `(byte_offset_in_text, chunk)`. Chunks
    /// are yielded in left-to-right order and never overlap. If the
    /// regex engine reports a runtime error mid-iteration the
    /// iterator stops early; results up to that point are still
    /// returned.
    #[must_use]
    pub fn split<'t>(&self, text: &'t str) -> Vec<(usize, &'t str)> {
        let mut out = Vec::new();
        for m in self.regex.find_iter(text) {
            let Ok(m) = m else { break };
            out.push((m.start(), &text[m.range()]));
        }
        out
    }

    /// Same as [`split`](Self::split) but returns byte ranges instead
    /// of borrowed substrings — useful when the caller wants to
    /// forward offsets into an owning encoder without re-slicing.
    #[must_use]
    pub fn split_ranges(&self, text: &str) -> Vec<core::ops::Range<usize>> {
        let mut out = Vec::new();
        for m in self.regex.find_iter(text) {
            let Ok(m) = m else { break };
            out.push(m.range());
        }
        out
    }
}

impl PartialEq for RegexPreTokenizer {
    /// Two pre-tokenizers are equal iff they were compiled from the
    /// same source pattern. This is a weaker invariant than "produce
    /// the same matches on every input" (regex compilers may
    /// normalise), but is enough for the deriving purpose of
    /// checking a downstream configuration against a stored one.
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl Eq for RegexPreTokenizer {}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn split_strs<'t>(pre: &RegexPreTokenizer, text: &'t str) -> Vec<&'t str> {
        pre.split(text).into_iter().map(|(_, s)| s).collect()
    }

    #[test]
    fn tiktoken_canonical_compiles() {
        // If this ever regresses, `tiktoken_canonical()` will panic
        // in downstream code, which we would rather catch here.
        let _ = RegexPreTokenizer::tiktoken_canonical();
    }

    #[test]
    fn gpt2_pattern_compiles() {
        let _ = RegexPreTokenizer::gpt2();
    }

    #[test]
    fn new_reports_invalid_pattern() {
        let err = RegexPreTokenizer::new("(unclosed").unwrap_err();
        // Message content is engine-defined; just check we got the
        // wrapper and the message is non-empty.
        assert!(!err.message().is_empty());
        assert!(err.to_string().contains("invalid pre-tokenizer pattern"));
    }

    #[test]
    fn pattern_accessor_returns_source() {
        let pre = RegexPreTokenizer::new(r"\s+").unwrap();
        assert_eq!(pre.pattern(), r"\s+");
    }

    #[test]
    fn eq_by_pattern_string() {
        let a = RegexPreTokenizer::new(r"\s+").unwrap();
        let b = RegexPreTokenizer::new(r"\s+").unwrap();
        let c = RegexPreTokenizer::new(r"\d+").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn split_empty_input_is_empty() {
        let pre = RegexPreTokenizer::tiktoken_canonical();
        assert!(pre.split("").is_empty());
        assert!(pre.split_ranges("").is_empty());
    }

    #[test]
    fn split_hello_world() {
        let pre = RegexPreTokenizer::tiktoken_canonical();
        assert_eq!(split_strs(&pre, "hello world"), vec!["hello", " world"]);
    }

    #[test]
    fn split_hello_comma_world_bang() {
        // tiktoken's canonical breaks punctuation into its own chunk;
        // leading spaces attach to the *following* word.
        let pre = RegexPreTokenizer::tiktoken_canonical();
        assert_eq!(
            split_strs(&pre, "Hello, world!"),
            vec!["Hello", ",", " world", "!"]
        );
    }

    #[test]
    fn split_contractions() {
        let pre = RegexPreTokenizer::tiktoken_canonical();
        // Case-insensitive alt catches both cases.
        assert_eq!(split_strs(&pre, "it's"), vec!["it", "'s"]);
        assert_eq!(split_strs(&pre, "IT'S"), vec!["IT", "'S"]);
        assert_eq!(split_strs(&pre, "they're"), vec!["they", "'re"]);
    }

    #[test]
    fn split_digit_groups_of_three() {
        // \p{N}{1,3} splits digit runs into groups of at most three.
        let pre = RegexPreTokenizer::tiktoken_canonical();
        assert_eq!(split_strs(&pre, "1000"), vec!["100", "0"]);
        assert_eq!(split_strs(&pre, "1234567"), vec!["123", "456", "7"]);
    }

    #[test]
    fn split_unicode_letters() {
        let pre = RegexPreTokenizer::tiktoken_canonical();
        // Latin letters with accents.
        assert_eq!(split_strs(&pre, "café"), vec!["café"]);
        // Cyrillic.
        assert_eq!(split_strs(&pre, "привет"), vec!["привет"]);
        // Greek word with a preceding space.
        assert_eq!(split_strs(&pre, " κόσμος"), vec![" κόσμος"]);
    }

    #[test]
    fn split_interior_whitespace_run() {
        // Three interior spaces before a word: the last space attaches
        // to the following word, the first two form their own chunk
        // (matched by `\s+(?!\S)` after backtracking to a length that
        // leaves whitespace ahead).
        let pre = RegexPreTokenizer::tiktoken_canonical();
        assert_eq!(
            split_strs(&pre, "hello   world"),
            vec!["hello", "  ", " world"]
        );
    }

    #[test]
    fn split_trailing_whitespace_at_end_of_input() {
        // The negative-lookahead alternative — the reason we need
        // fancy-regex — is what makes the trailing run land in one
        // chunk instead of being sliced by the "space + word" alt.
        let pre = RegexPreTokenizer::tiktoken_canonical();
        assert_eq!(split_strs(&pre, "hello   "), vec!["hello", "   "]);
    }

    #[test]
    fn split_newlines_are_their_own_chunk() {
        let pre = RegexPreTokenizer::tiktoken_canonical();
        // `\s*[\r\n]+` swallows the run of newline whitespace.
        assert_eq!(split_strs(&pre, "a\nb"), vec!["a", "\n", "b"]);
        assert_eq!(split_strs(&pre, "a\r\nb"), vec!["a", "\r\n", "b"]);
        assert_eq!(split_strs(&pre, "a\n\nb"), vec!["a", "\n\n", "b"]);
    }

    #[test]
    fn split_offsets_are_byte_indices_into_input() {
        // Explicitly verify byte offsets (multi-byte characters
        // exercise the "byte, not char" contract).
        let pre = RegexPreTokenizer::tiktoken_canonical();
        let text = "café world";
        // "café" is c(1) a(1) f(1) é(2) = 5 bytes, then " world" begins
        // at byte offset 5.
        let chunks = pre.split(text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].0, 0);
        assert_eq!(chunks[0].1, "café");
        assert_eq!(chunks[1].0, 5);
        assert_eq!(chunks[1].1, " world");
    }

    #[test]
    fn split_ranges_agrees_with_split() {
        let pre = RegexPreTokenizer::tiktoken_canonical();
        let text = "Hello, world!\n";
        let strs = pre.split(text);
        let ranges = pre.split_ranges(text);
        assert_eq!(strs.len(), ranges.len());
        for ((off, s), r) in strs.into_iter().zip(ranges) {
            assert_eq!(off, r.start);
            assert_eq!(s, &text[r.clone()]);
            assert_eq!(s.len(), r.end - r.start);
        }
    }

    #[test]
    fn split_concatenation_covers_input_for_canonical_pattern() {
        // The canonical pattern's alternatives together cover every
        // byte of every valid UTF-8 input. Verify that on a
        // representative mix.
        let pre = RegexPreTokenizer::tiktoken_canonical();
        for text in [
            "hello world",
            "Hello, world!",
            "it's a test.",
            "1234567890",
            "café résumé",
            "line1\nline2",
            "  leading space",
            "trailing space   ",
        ] {
            let mut concat = String::new();
            for (_, s) in pre.split(text) {
                concat.push_str(s);
            }
            assert_eq!(concat, text, "canonical pattern did not cover {text:?}");
        }
    }

    #[test]
    fn split_user_supplied_pattern_word_only() {
        // A custom pattern that only matches word runs demonstrates
        // the "gaps get dropped" contract.
        let pre = RegexPreTokenizer::new(r"\p{L}+").unwrap();
        assert_eq!(split_strs(&pre, "abc, def"), vec!["abc", "def"]);
    }

    #[test]
    fn gpt2_pattern_splits_basic_input() {
        let pre = RegexPreTokenizer::gpt2();
        assert_eq!(
            split_strs(&pre, "Hello, world!"),
            vec!["Hello", ",", " world", "!"]
        );
        // GPT-2 groups digits without the {1,3} cap.
        assert_eq!(split_strs(&pre, "1234567"), vec!["1234567"]);
    }
}
