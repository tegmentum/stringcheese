//! Comparand — rigorous sequence comparison for Rust and WebAssembly.
//!
//! This is the top-level facade. It re-exports Comparand's public API from
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

pub use comparand_core::*;

/// The Levenshtein subsystem: unit-cost edit distance with full-matrix,
/// rolling-rows, and Ukkonen-style banded kernels, re-exported unchanged
/// from the `comparand-levenshtein` crate.
pub use comparand_levenshtein as levenshtein;

/// The Hamming subsystem: equal-length Hamming distance with a fallible
/// entry point for callers who cannot statically establish equal length,
/// re-exported unchanged from the `comparand-hamming` crate.
pub use comparand_hamming as hamming;

/// The Jaro subsystem: base Jaro (1989) similarity and the Jaro-Winkler
/// variant family, re-exported unchanged from the `comparand-jaro` crate.
pub use comparand_jaro as jaro;

/// The Damerau subsystem: Optimal String Alignment (semimetric, restricted
/// Damerau-Levenshtein) and the full unrestricted Damerau-Levenshtein
/// (true metric per Damerau 1964), re-exported unchanged from the
/// `comparand-damerau` crate.
pub use comparand_damerau as damerau;

/// The n-gram representation layer: character, byte, and token n-gram
/// generators plus set / multiset / weighted-vector representations,
/// re-exported unchanged from the `comparand-ngram` crate.
pub use comparand_ngram as ngram;

/// The Unicode preprocessing subsystem: NFC/NFD/NFKC/NFKD normalization,
/// full Unicode case folding, grapheme-cluster segmentation with an
/// `IndexableSequence` bridge, diacritic stripping, and a composable
/// `PreprocessingPipeline` builder, re-exported unchanged from the
/// `comparand-unicode` crate.
pub use comparand_unicode as unicode;

/// The phonetic subsystem: Soundex (NARA 1918), NYSIIS (Taft 1970), and
/// Double Metaphone (Philips 1999, primary-only) encoders plus the
/// `PhoneticEncoder` trait and `PhoneticMatcher` composer, re-exported
/// unchanged from the `comparand-phonetic` crate.
pub use comparand_phonetic as phonetic;

/// Metadata about this release.
pub mod meta {
    /// The `comparand` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
