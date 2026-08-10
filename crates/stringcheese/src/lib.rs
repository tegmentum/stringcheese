//! StringCheese — rigorous sequence comparison for Rust and WebAssembly.
//!
//! This is the top-level facade. It re-exports StringCheese's public API from
//! the underlying implementation crates so library consumers need only one
//! dependency and one `use` path.
//!
//! For an overview of the project's design, algorithm coverage, and
//! validation strategy, see the `DESIGN.md` document in the repository.
//!
//! # Status
//!
//! Version 0.1 is under initial development. The current release covers the
//! type-system substrate — result types, metric traits, algorithm-variant
//! descriptors, workspace and sequence abstractions, and the golden-case
//! validation schema. Concrete algorithm implementations arrive in
//! subsequent milestones.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
// Doc comments here reference technical acronyms (MinHash /
// SimHash / LSH / NFKC / UCA / Moss / ISO 9) and non-code proper
// nouns liberally. Wrapping each in backticks harms readability
// of a facade doc whose whole purpose is orientation.
#![allow(clippy::doc_markdown)]

pub use stringcheese_core::*;

/// The consolidated comparison-kernel crate: edit-distance metrics
/// (Levenshtein, Hamming, Jaro/Jaro-Winkler, Damerau/OSA, LCS), n-gram
/// representations, set-similarity coefficients (Dice, Jaccard, Overlap,
/// Cosine), substring-search algorithms (Rabin-Karp, KMP, Boyer-Moore,
/// Aho-Corasick, Horspool, Two-way), and `MinHash` sketches with LSH
/// banding. Re-exported unchanged from the `stringcheese-compare` crate.
pub use stringcheese_compare as compare;

// Per-family module aliases preserved so downstream code that used to
// import the algorithm crates individually keeps its short paths working
// (`stringcheese::levenshtein::Levenshtein`, `stringcheese::ngram::GramSet`,
// …). Every module here is exactly `stringcheese_compare::<module>`.
pub use stringcheese_compare::{
    damerau, hamming, jaro, lcs, levenshtein, minhash, ngram, search, set_similarity,
};

/// The content-defined chunking and rolling-hash fingerprint subsystem:
/// Rabin (GF(2) polynomial), polynomial (mod Mersenne-61), and Gear
/// (from the `FastCDC` paper) rolling hashes behind a shared
/// `RollingHash` trait, plus `FastCDC` content-defined chunking as a
/// streaming state machine, re-exported unchanged from the
/// `stringcheese-cdc` crate.
pub use stringcheese_cdc as cdc;

/// The index-structures subsystem: BK-tree and VP-tree for metric-space
/// range and k-nearest queries (both enforce `is_metric()` at
/// construction), plus a q-gram inverted index for set-similarity
/// candidate generation, re-exported unchanged from the
/// `stringcheese-index` crate.
pub use stringcheese_index as index;

/// The alignment subsystem: Needleman-Wunsch (global) and Smith-Waterman
/// (local) pairwise sequence alignment with linear and affine gap
/// penalties (full Gotoh 1982 three-matrix formulation for affine),
/// plus edit-script reconstruction, re-exported unchanged from the
/// `stringcheese-align` crate.
pub use stringcheese_align as align;

/// The Unicode preprocessing subsystem: NFC/NFD/NFKC/NFKD normalization,
/// full Unicode case folding, grapheme-cluster segmentation with an
/// `IndexableSequence` bridge, diacritic stripping, and a composable
/// `PreprocessingPipeline` builder, re-exported unchanged from the
/// `stringcheese-unicode` crate.
pub use stringcheese_unicode as unicode;

/// The phonetic subsystem: Soundex (NARA 1918), NYSIIS (Taft 1970), and
/// Double Metaphone (Philips 1999, primary-only) encoders plus the
/// `PhoneticEncoder` trait and `PhoneticMatcher` composer, re-exported
/// unchanged from the `stringcheese-phonetic` crate.
pub use stringcheese_phonetic as phonetic;

/// The string-manipulation subsystem — inspect, trim, case, split, join,
/// replace, normalize, pad, slice, find, escape, quote, lines, template,
/// plus the `TextPipeline` transformation IR. Scaffold only in v0.1;
/// modules populate in subsequent releases. Re-exported unchanged from
/// the `stringcheese-manip` crate.
pub use stringcheese_manip as manip;

/// The language-pack infrastructure: the `Language` trait, the
/// `LanguageProvider` discovery trait, the `Stemmer` / `Collator` /
/// `LanguagePhoneticEncoder` plugin points, and the `Stopwords` and
/// `SimpleTokenizer` helper types. Re-exported unchanged from the
/// `stringcheese-lang` crate.
///
/// # Language packs are opt-in
///
/// The umbrella facade re-exports `stringcheese-lang` because *every*
/// language pack builds against its trait surface — but it does **not**
/// re-export any specific `stringcheese-<lang>` crate. Language packs
/// (`stringcheese-en`, `stringcheese-de`, `stringcheese-fr`, …) are
/// per-language, opt-in dependencies: callers who need English pull in
/// `stringcheese-en` explicitly, callers who don't pay nothing (not a
/// byte of stopword list, not an entry in the stemmer's rule tables)
/// at compile or runtime.
pub use stringcheese_lang as lang;

/// The tokenizer/segmenter trait crate + built-in tokenizers
/// (whitespace, delimiter, identifier, grapheme, n-gram). Re-exported
/// unchanged from the `stringcheese-tokenizer` crate.
///
/// # Downstream algorithm crates are opt-in
///
/// The facade re-exports the trait crate here so a caller who only
/// needs the built-in segmenters (whitespace, delimiter, identifier,
/// grapheme, n-gram) never has to add a second dependency. Data-heavy
/// downstream algorithm crates are per-algorithm, opt-in dependencies:
/// `stringcheese-tokenizer-hf`, and the planned `-wordpiece`,
/// `-sentencepiece`, `-tiktoken`, and `-huggingface` model packs, are
/// pulled in by callers who need them. Callers who don't pay nothing —
/// not a byte of vocabulary data, not an entry in a merge table — at
/// compile or runtime.
pub use stringcheese_tokenizer as tokenizer;

/// Unicode-aware segmentation: bytes, code points, graphemes,
/// words, sentences, lines. `SegmentUnit` names the boundary
/// explicitly at every call site. Re-exported from the
/// `stringcheese-segment` crate.
pub use stringcheese_segment as segment;

/// String statistics and characterisation: Shannon entropy,
/// Unicode general-category histograms, printable / control /
/// whitespace / digit / alphabetic / punctuation ratios, and the
/// byte / code-point / grapheme length triple. Re-exported from
/// the `stringcheese-stats` crate.
pub use stringcheese_stats as stats;

/// Identifier utilities: case conversion (`Case::{Snake, Camel,
/// Pascal, Kebab, ScreamingSnake, ScreamingKebab, Train}`),
/// `Case::detect` best-effort classification, `slugify` /
/// `Slugger` for URL/filename slugs, and `Sanitizer` for
/// identifier coercion. Re-exported from the
/// `stringcheese-ident` crate.
pub use stringcheese_ident as ident;

/// Escape / quote / encode utilities: URI percent-encoding, HTML
/// entity escaping (text and attribute contexts), JSON string
/// body escape, POSIX shell word quoting — one `Escape` target
/// enum names the grammar explicitly. Re-exported from the
/// `stringcheese-escape` crate.
pub use stringcheese_escape as escape;

/// SimHash fingerprints (64-bit / 128-bit) with weighted-feature
/// support, Hamming-distance similarity, and permutation-band
/// LSH. Re-exported from the `stringcheese-simhash` crate.
///
/// See [`compare::minhash`] for the older set-Jaccard MinHash
/// sketches — SimHash is for weighted feature BAGS, MinHash for
/// SETS.
pub use stringcheese_simhash as simhash;

/// Winnowing document fingerprints (Schleimer-Wilkerson-Aiken
/// 2003) — the local-algorithm sketch used by Moss and other
/// plagiarism detectors. Re-exported from the
/// `stringcheese-winnowing` crate.
pub use stringcheese_winnowing as winnowing;

/// Named normalization pipelines: `identifier` (NFKC + case-fold +
/// strip-diacritics), `display_safe` (strip-controls + NFC +
/// collapse-whitespace), `search_key`, `punctuation_canonical`,
/// plus the primitives that back them. Re-exported from the
/// `stringcheese-normalize` crate.
pub use stringcheese_normalize as normalize;

/// Text splitters and semantic chunkers for LLM RAG pipelines:
/// a `TextSplitter` trait, LangChain-style `RecursiveSplitter`,
/// `ParagraphSplitter`, `SentenceSplitter`. Complements the
/// byte-oriented content-defined chunking in [`cdc`].
/// Re-exported from the `stringcheese-textsplit` crate.
pub use stringcheese_textsplit as textsplit;

/// Locale-aware collation: `UcaCollator` (Unicode Collation
/// Algorithm via feruca), `NaturalCollator` (`file2 < file10`),
/// `AsciiCiCollator` (ASCII case-insensitive fast path), all
/// behind a common `Collator` trait. Re-exported from the
/// `stringcheese-collate` crate.
pub use stringcheese_collate as collate;

/// Script-to-script transliteration: `DeunicodeTransliterator`
/// for the general any-script → ASCII path, `TableTransliterator`
/// for classic char-to-string lookup, a `Chained` combinator,
/// and a built-in ISO 9 Cyrillic → Latin table. Re-exported from
/// the `stringcheese-translit` crate.
pub use stringcheese_translit as translit;

/// Diff and sequence alignment: Myers (1986) and Patience diff
/// over any `T: Eq` sequence, edit-script + unified-diff-format
/// output, hunks, and `patch::apply`. Re-exported from the
/// `stringcheese-diff` crate.
pub use stringcheese_diff as diff;

/// Pattern-matching subsystem: `Literal`, `Wildcard`, `Glob`
/// (Wildcard + Glob wrap `globset` under the same `Pattern`
/// trait) with explicit `MatchUnit` (Bytes / CodePoints /
/// Graphemes) at construction. The finite-automata regex engine
/// lives in `pattern_regex` (the sibling
/// `stringcheese-pattern-regex` crate) behind the `pattern-regex`
/// feature.
/// Re-exported from the `stringcheese-pattern` crate.
pub use stringcheese_pattern as pattern;

/// Finite-automata regex engine — thin adapter over the `regex`
/// crate that plugs into [`pattern::Pattern`]. Available when
/// the `pattern-regex` feature is on; off by default because the
/// underlying regex-engine data isn't negligible.
///
/// Callers who prefer a direct dep on `stringcheese-pattern-regex`
/// get the same crate through that path.
#[cfg(feature = "pattern-regex")]
pub use stringcheese_pattern_regex as pattern_regex;

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
