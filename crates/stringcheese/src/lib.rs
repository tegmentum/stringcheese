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

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
