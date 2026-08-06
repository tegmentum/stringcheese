//! Comparison kernels for the StringCheese toolkit.
//!
//! One crate for every algorithm that computes a distance, similarity,
//! score, or match between two sequences. Organised by algorithm family:
//!
//! * [`levenshtein`] — unit-cost Levenshtein edit distance with
//!   full-matrix, rolling-rows, and Ukkonen-style banded kernels.
//! * [`hamming`] — equal-length Hamming distance with a fallible entry
//!   point for callers who cannot statically prove equal length.
//! * [`jaro`] — the base Jaro similarity and the Jaro-Winkler variant
//!   family (both boundable to `[0.0, 1.0]`).
//! * [`damerau`] — Optimal String Alignment (semimetric, restricted
//!   Damerau-Levenshtein) and full unrestricted Damerau-Levenshtein
//!   (true metric under unit costs).
//! * [`lcs`] — Longest Common Subsequence length and the derived
//!   `|a| + |b| - 2·lcs(a, b)` distance metric.
//! * [`ngram`] — character, byte, and token n-gram generators plus
//!   set / multiset / weighted-vector representations. Substrate for
//!   the set-similarity, `MinHash`, and (downstream) index families.
//! * [`search`] — Rabin-Karp, KMP, Boyer-Moore, Aho-Corasick, Horspool,
//!   and Crochemore-Perrin Two-Way single- and multi-pattern search.
//! * [`set_similarity`] — Dice, Jaccard, Overlap, and Cosine over the
//!   representations exposed by the [`ngram`] module.
//! * [`minhash`] — k-permutation `MinHash` sketches, Ioffe 2010 weighted
//!   `MinHash` (Consistent Weighted Sampling), and banded LSH for
//!   approximate Jaccard-nearest-neighbour search at scale.
//!
//! Every algorithm crate that used to live at its own top-level slug
//! (`stringcheese-levenshtein`, `stringcheese-hamming`, ...) is now a
//! module here; the load-bearing types they used to re-export at their
//! crate roots are also re-exported at *this* crate's root so the
//! previous flat-import shape (`use stringcheese_compare::Levenshtein`)
//! keeps working alongside the new namespaced form
//! (`use stringcheese_compare::levenshtein::Levenshtein`).
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. Every kernel that requires heap
//! allocation is gated on the `alloc` feature; a build with neither
//! `std` nor `alloc` compiles as a near-empty surface (only the
//! trait shells from [`search::api`] survive). Under `--no-default-features`
//! this is what makes the crate safe to add as a dependency in embedded
//! configurations that only need to link against the substrate.
//!
//! # Features
//!
//! * `default = ["std"]`.
//! * `std` — the standard library. Enables the following behaviors that
//!   `alloc` alone cannot cover: Damerau's `HashMap`-backed production
//!   kernel and [`damerau::DamerauWorkspace`]; set-similarity's
//!   [`set_similarity::Cosine`] (uses `f64::sqrt`); n-gram's
//!   `l2_norm` / `normalize_l2` on [`ngram::GramVector`]; and `MinHash`'s
//!   weighted CWS sketch. Implies `alloc`.
//! * `alloc` — heap-allocating types from `alloc`. Every dynamic-programming
//!   kernel and every representation type here needs this.

#![cfg_attr(not(feature = "std"), no_std)]
// The crate is `deny(unsafe_code)` rather than `forbid(unsafe_code)` so
// that the optional SIMD backend under `levenshtein::simd` (only compiled
// with `--features simd`) can carry a documented module-scoped
// `#[allow(unsafe_code)]`. `deny` is enforced everywhere else — every
// module outside `levenshtein::simd` is expected to be safe Rust; the
// allow attribute must be added deliberately and comes with a `reason`
// explaining the exception. See `levenshtein/simd/mod.rs` for the full
// unsafe policy.
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
#[allow(unused_extern_crates)]
extern crate alloc;

pub mod damerau;
pub mod hamming;
pub mod jaro;
pub mod lcs;
pub mod levenshtein;
pub mod minhash;
pub mod ngram;
pub mod search;
pub mod set_similarity;

// -----------------------------------------------------------------------
// Convenience re-exports — matches what each source crate's `lib.rs`
// used to re-export at its own root. Downstream code that had
// `use stringcheese_<crate>::<X>` becomes `use stringcheese_compare::<X>`
// without a per-module hop. The namespaced path
// `stringcheese_compare::<module>::<X>` also works for consumers that
// prefer the explicit form.
// -----------------------------------------------------------------------

// levenshtein
#[cfg(feature = "alloc")]
pub use crate::levenshtein::{
    Levenshtein, LevenshteinWorkspace, distance_banded_with_workspace, distance_full_matrix,
    distance_rolling_rows_with_workspace,
};

// hamming
pub use crate::hamming::{Hamming, LengthMismatch, hamming_distance, hamming_distance_within};

// jaro
#[cfg(feature = "alloc")]
pub use crate::jaro::{Jaro, JaroWinkler, JaroWinklerError, JaroWorkspace};

// damerau
#[cfg(feature = "std")]
pub use crate::damerau::DamerauWorkspace;
#[cfg(feature = "alloc")]
pub use crate::damerau::{Damerau, Osa, OsaWorkspace};

// lcs
#[cfg(feature = "alloc")]
pub use crate::lcs::{
    Lcs, LcsDistance, LcsWorkspace, lcs_distance_full_matrix,
    lcs_distance_rolling_rows_with_workspace, lcs_length_full_matrix,
    lcs_length_rolling_rows_with_workspace,
};

// ngram
#[cfg(feature = "alloc")]
pub use crate::ngram::{
    CharacterGramSlices, CharacterGrams, GramMultiSet, GramSet, GramVector, InvalidN,
    NGramGenerator, PaddingPolicy, TokenGrams, count_grams,
};

// search
#[cfg(feature = "alloc")]
pub use crate::search::{
    AhoCorasick, BoyerMoore, BoyerMooreFull, Horspool, Kmp, RabinKarp, SearchStream,
    SinglePatternSearch, StreamingSearch, TwoWay,
};
pub use crate::search::{Match, SearchAlgorithm};

// set_similarity
#[cfg(all(feature = "std", feature = "alloc"))]
pub use crate::set_similarity::Cosine;
#[cfg(feature = "alloc")]
pub use crate::set_similarity::{
    DiceOverMultiSet, DiceOverSet, JaccardOverMultiSet, JaccardOverSet, Overlap,
};

// minhash
#[cfg(feature = "alloc")]
pub use crate::minhash::{
    LshIndex, MINHASH_JACCARD_DESCRIPTOR, MinHashSketch,
    ONE_PERMUTATION_MINHASH_JACCARD_DESCRIPTOR, OnePermutationMinHashSketch,
    SIMHASH_COSINE_DESCRIPTOR, SimHashSketch,
};
#[cfg(all(feature = "std", feature = "alloc"))]
pub use crate::minhash::{
    P_STABLE_LSH_L1_DESCRIPTOR, P_STABLE_LSH_L2_DESCRIPTOR, PStableFamily, PStableLshSketch,
    WEIGHTED_MINHASH_JACCARD_DESCRIPTOR, WeightedMinHashSketch,
};
