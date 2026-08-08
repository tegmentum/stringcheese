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
use core::mem;

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
// Metaspace pre-tokenizer (`SentencePiece`).
// ---------------------------------------------------------------------

/// `SentencePiece`-style prepending policy for the [`Metaspace`]
/// pre-tokenizer.
///
/// Mirrors Hugging Face's on-disk shape: the JSON string
/// `"always"` / `"never"` / `"first"` maps to the corresponding
/// variant. The default is [`Self::Always`] — HF's own default and the
/// shape used by Llama, Mistral, T5, and XLM-RoBERTa checkpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrependScheme {
    /// Always prepend the replacement character to the input, even if
    /// the input already starts with it.
    #[default]
    Always,
    /// Never prepend.
    Never,
    /// Prepend the replacement character only when the input does not
    /// already start with it. Used by tokenizers that pre-mark
    /// caller-visible strings with a leading `▁`.
    First,
}

/// `SentencePiece` "Metaspace" pre-tokenizer — Kudo & Richardson (2018).
///
/// Every ASCII space in the input is replaced with a single
/// [`Self::replacement`] character (canonically `▁`, U+2581 LOWER ONE
/// EIGHTH BLOCK, informally the "space bar" mark). Depending on
/// [`Self::prepend_scheme`] the replacement character may also be
/// prepended to the string. When [`Self::split`] is `true` — HF's
/// default — the transformed string is split on the replacement
/// character with `SentencePiece`'s `MergedWithNext` semantics: each
/// output piece starts with the replacement character (except for the
/// first piece when no prepend happened and the input did not start
/// with a space).
///
/// This pre-tokenizer is what makes Llama, Mistral, T5, and
/// XLM-RoBERTa treat token-initial position uniformly — a word that
/// appears at the start of the input is scored the same as a word that
/// follows a space, because both surface strings begin with `▁`.
///
/// # Example
///
/// ```
/// use stringcheese_tokenizer_bpe::Metaspace;
///
/// let ms = Metaspace::new();
/// assert_eq!(
///     ms.apply("hello world"),
///     vec!["\u{2581}hello".to_string(), "\u{2581}world".to_string()],
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metaspace {
    /// The character substituted for ASCII space. HF's default is
    /// `▁` (U+2581) — see [`Self::DEFAULT_REPLACEMENT`].
    pub replacement: char,
    /// The prepend policy — see [`PrependScheme`].
    pub prepend_scheme: PrependScheme,
    /// Whether to split the transformed string on the replacement
    /// character so each piece starts with [`Self::replacement`]
    /// (`SentencePiece`'s `MergedWithNext` split behaviour). When
    /// `false`, [`Self::apply`] returns a single-element `Vec`
    /// containing the transformed string verbatim.
    pub split: bool,
}

impl Metaspace {
    /// The canonical replacement character (`▁`, U+2581 LOWER ONE
    /// EIGHTH BLOCK) — HF's own default and the shape used by every
    /// `SentencePiece`-descended checkpoint on the Hub.
    pub const DEFAULT_REPLACEMENT: char = '\u{2581}';

    /// Construct a Metaspace with Hugging Face's default configuration:
    /// [`Self::DEFAULT_REPLACEMENT`], [`PrependScheme::Always`], and
    /// `split = true`. Matches the shape a bare
    /// `{"type": "Metaspace"}` block deserialises to.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            replacement: Self::DEFAULT_REPLACEMENT,
            prepend_scheme: PrependScheme::Always,
            split: true,
        }
    }

    /// Apply the pre-tokenizer to `text`, returning the split pieces.
    ///
    /// Semantics, in order:
    ///
    /// 1. Every ASCII space in `text` is replaced with
    ///    [`Self::replacement`].
    /// 2. Depending on [`Self::prepend_scheme`], the replacement
    ///    character may be prepended:
    ///    * [`PrependScheme::Always`] — always prepend.
    ///    * [`PrependScheme::First`] — prepend only if the (already
    ///      transformed) string does not start with the replacement.
    ///    * [`PrependScheme::Never`] — never prepend.
    /// 3. If [`Self::split`] is `true`, the transformed string is
    ///    split on the replacement character with `SentencePiece`'s
    ///    `MergedWithNext` semantics: the delimiter stays glued to the
    ///    following piece. If `false`, the returned vec contains
    ///    exactly one element (the transformed string).
    ///
    /// An empty input returns an empty `Vec` regardless of
    /// [`Self::prepend_scheme`] or [`Self::split`] — matching HF's own
    /// behaviour.
    #[must_use]
    pub fn apply(&self, text: &str) -> Vec<String> {
        if text.is_empty() {
            return Vec::new();
        }

        // Step 1: replace ASCII spaces with the replacement character.
        // The output is at most `text.chars().count() * repl_utf8_len`
        // bytes; a good enough starting capacity is `text.len()`.
        let mut transformed = String::with_capacity(text.len());
        for c in text.chars() {
            if c == ' ' {
                transformed.push(self.replacement);
            } else {
                transformed.push(c);
            }
        }

        // Step 2: optional prepend, per the scheme.
        let should_prepend = match self.prepend_scheme {
            PrependScheme::Always => true,
            PrependScheme::First => !transformed.starts_with(self.replacement),
            PrependScheme::Never => false,
        };
        if should_prepend {
            let mut prefixed =
                String::with_capacity(self.replacement.len_utf8() + transformed.len());
            prefixed.push(self.replacement);
            prefixed.push_str(&transformed);
            transformed = prefixed;
        }

        // Step 3: optional split. Without splitting we return the whole
        // transformed string as a single piece.
        if !self.split {
            return alloc::vec![transformed];
        }

        // `MergedWithNext`: the delimiter attaches to the piece that
        // follows it. We walk the string once, breaking off a new
        // piece whenever a replacement char appears mid-buffer. A
        // leading replacement is *not* treated as a boundary — it
        // becomes the first character of the first piece.
        let mut pieces: Vec<String> = Vec::new();
        let mut current = String::new();
        for c in transformed.chars() {
            if c == self.replacement && !current.is_empty() {
                pieces.push(mem::take(&mut current));
            }
            current.push(c);
        }
        if !current.is_empty() {
            pieces.push(current);
        }
        pieces
    }
}

impl Default for Metaspace {
    fn default() -> Self {
        Self::new()
    }
}

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

    // ---------------------------------------------------------------
    // Metaspace
    // ---------------------------------------------------------------

    #[test]
    fn metaspace_default_matches_hf_shape() {
        let ms = Metaspace::default();
        assert_eq!(ms.replacement, '\u{2581}');
        assert_eq!(ms.prepend_scheme, PrependScheme::Always);
        assert!(ms.split);
    }

    #[test]
    fn metaspace_hello_world_with_defaults_splits_into_two_marked_pieces() {
        // HF Python reference:
        //   >>> from tokenizers.pre_tokenizers import Metaspace
        //   >>> Metaspace().pre_tokenize_str("hello world")
        //   [('▁hello', (0, 5)), ('▁world', (5, 11))]
        let ms = Metaspace::new();
        assert_eq!(
            ms.apply("hello world"),
            vec!["\u{2581}hello".to_string(), "\u{2581}world".to_string()]
        );
    }

    #[test]
    fn metaspace_prepend_never_leaves_first_piece_unmarked() {
        // Reference:
        //   >>> Metaspace(prepend_scheme="never").pre_tokenize_str("hello world")
        //   [('hello', ...), ('▁world', ...)]
        let ms = Metaspace {
            replacement: Metaspace::DEFAULT_REPLACEMENT,
            prepend_scheme: PrependScheme::Never,
            split: true,
        };
        assert_eq!(
            ms.apply("hello world"),
            vec!["hello".to_string(), "\u{2581}world".to_string()]
        );
    }

    #[test]
    fn metaspace_prepend_first_does_not_double_prepend() {
        // Input already starts with `▁` — First should be a no-op.
        let ms = Metaspace {
            replacement: Metaspace::DEFAULT_REPLACEMENT,
            prepend_scheme: PrependScheme::First,
            split: true,
        };
        assert_eq!(
            ms.apply("\u{2581}already prefixed"),
            vec![
                "\u{2581}already".to_string(),
                "\u{2581}prefixed".to_string()
            ]
        );
    }

    #[test]
    fn metaspace_prepend_first_prepends_when_missing() {
        // Input does not start with `▁` — First behaves like Always.
        let ms = Metaspace {
            replacement: Metaspace::DEFAULT_REPLACEMENT,
            prepend_scheme: PrependScheme::First,
            split: true,
        };
        assert_eq!(
            ms.apply("hello world"),
            vec!["\u{2581}hello".to_string(), "\u{2581}world".to_string()]
        );
    }

    #[test]
    fn metaspace_split_false_returns_single_transformed_piece() {
        let ms = Metaspace {
            replacement: Metaspace::DEFAULT_REPLACEMENT,
            prepend_scheme: PrependScheme::Always,
            split: false,
        };
        assert_eq!(
            ms.apply("hello world"),
            vec!["\u{2581}hello\u{2581}world".to_string()]
        );
    }

    #[test]
    fn metaspace_empty_input_is_empty_output() {
        let ms = Metaspace::new();
        assert!(ms.apply("").is_empty());
    }

    #[test]
    fn metaspace_no_spaces_still_prepends_when_always() {
        let ms = Metaspace::new();
        assert_eq!(ms.apply("hello"), vec!["\u{2581}hello".to_string()]);
    }

    #[test]
    fn metaspace_custom_replacement_char_is_honoured() {
        let ms = Metaspace {
            replacement: '_',
            prepend_scheme: PrependScheme::Always,
            split: true,
        };
        assert_eq!(
            ms.apply("hello world"),
            vec!["_hello".to_string(), "_world".to_string()]
        );
    }
}
