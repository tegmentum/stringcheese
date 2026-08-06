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

/// The Levenshtein subsystem: unit-cost edit distance with full-matrix,
/// rolling-rows, and Ukkonen-style banded kernels, re-exported unchanged
/// from the `stringcheese-levenshtein` crate.
pub use stringcheese_levenshtein as levenshtein;

/// The Hamming subsystem: equal-length Hamming distance with a fallible
/// entry point for callers who cannot statically establish equal length,
/// re-exported unchanged from the `stringcheese-hamming` crate.
pub use stringcheese_hamming as hamming;

/// The Jaro subsystem: base Jaro (1989) similarity and the Jaro-Winkler
/// variant family, re-exported unchanged from the `stringcheese-jaro` crate.
pub use stringcheese_jaro as jaro;

/// The Damerau subsystem: Optimal String Alignment (semimetric, restricted
/// Damerau-Levenshtein) and the full unrestricted Damerau-Levenshtein
/// (true metric per Damerau 1964), re-exported unchanged from the
/// `stringcheese-damerau` crate.
pub use stringcheese_damerau as damerau;

/// The Longest Common Subsequence subsystem: LCS length (as `Score<u32>`)
/// and the derived LCS distance metric (`|a| + |b| - 2 · lcs(a, b)`),
/// re-exported unchanged from the `stringcheese-lcs` crate.
pub use stringcheese_lcs as lcs;

/// The substring-search subsystem: Rabin-Karp, KMP, Boyer-Moore
/// (bad-character variant), and Aho-Corasick multi-pattern matching,
/// re-exported unchanged from the `stringcheese-search` crate.
pub use stringcheese_search as search;

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

/// The `MinHash` subsystem: probabilistic Jaccard-similarity sketches
/// and LSH banding for approximate-nearest-neighbor search at scale.
/// K-permutation `MinHash` (Broder 1997) + weighted `MinHash`
/// (Ioffe 2010 CWS) + banded LSH (Gionis-Indyk-Motwani 1999),
/// re-exported unchanged from the `stringcheese-minhash` crate.
pub use stringcheese_minhash as minhash;

/// The n-gram representation layer: character, byte, and token n-gram
/// generators plus set / multiset / weighted-vector representations,
/// re-exported unchanged from the `stringcheese-ngram` crate.
pub use stringcheese_ngram as ngram;

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

/// The set-similarity subsystem: Dice, Jaccard, Overlap, and Cosine over
/// the n-gram representations from `stringcheese_ngram`, re-exported
/// unchanged from the `stringcheese-set-similarity` crate.
pub use stringcheese_set_similarity as set_similarity;

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
