//! Comparison kernels for the StringCheese toolkit.
//!
//! One crate for every algorithm that computes a distance, similarity,
//! score, or match between two sequences. Organised by algorithm family:
//!
//! * [`levenshtein`] — unit-cost Levenshtein edit distance with
//!   full-matrix, rolling-rows, and Ukkonen-style banded kernels.
//! * [`learned`] — Ristad-Yianilos (1998) learned string-edit distance:
//!   a memoryless stochastic transducer with per-character insert /
//!   delete / substitute costs trained from labeled pairs via
//!   Expectation-Maximization. Distance-side surface (`LearnedEdit`,
//!   `LearnedEditModel`) is available under `alloc`; the training
//!   estimator (`RistadYianilosEstimator`) requires `std` for `f64::ln`
//!   and `f64::exp`.
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
//!
//! # Benchmarks
//!
//! A criterion bench harness lives at `benches/compare.rs`; run it with
//!
//! ```text
//! cargo bench -p stringcheese-compare
//! ```
//!
//! Eight groups drive every distance / similarity family the crate ships
//! (Levenshtein, OSA Damerau, full Damerau, Hamming, Jaro, Jaro-Winkler,
//! LCS, and Jaccard-over-bigram-set). Each group runs three input sizes
//! (32 / 256 / 2048 elements) crossed with two flavors (`ascii` byte
//! slices and `utf8` `char` slices drawn from Latin, Cyrillic, Greek,
//! Georgian, and CJK blocks).
//!
//! # Baseline (aarch64 Apple M-series, macOS 15, rustc 1.97.1, release + LTO)
//!
//! Numbers below are median throughput of one representative run
//! (`--warm-up-time 1 --measurement-time 3 --sample-size 15`). Wall-clock
//! samples vary ±10–15 % on a laptop under load; treat the ratios as
//! informative and the absolutes as illustrative. Throughput reported as
//! MiB/s (or MiB/s-equivalents; the O(n²) DPs are reported over the input
//! length, so a "3 MiB/s" number at n = 128 actually corresponds to
//! `128² / 3µs` cell updates). Higher is better.
//!
//! ```text
//! group                             32              256            2048
//! ------------------------------------------------------------------------
//! levenshtein         / ascii     13.2 MiB/s      1.33 MiB/s     160 KiB/s
//! levenshtein         / utf8      14.3 MiB/s      1.32 MiB/s      84 KiB/s
//! damerau_osa         / ascii      3.9 MiB/s       588 KiB/s      70 KiB/s
//! damerau_osa         / utf8       5.0 MiB/s       655 KiB/s      65 KiB/s
//! damerau_full        / ascii      1.8 MiB/s       458 KiB/s      43 KiB/s
//! damerau_full        / utf8       2.4 MiB/s       369 KiB/s      45 KiB/s
//! hamming             / ascii     3.10 GiB/s      4.24 GiB/s    3.07 GiB/s
//! hamming             / utf8      3.26 GiB/s      4.46 GiB/s    4.93 GiB/s
//! jaro                / ascii      135 MiB/s      15.7 MiB/s    1.85 MiB/s
//! jaro                / utf8       128 MiB/s       6.4 MiB/s    1.78 MiB/s
//! jaro_winkler        / ascii      120 MiB/s      11.3 MiB/s    2.77 MiB/s
//! jaro_winkler        / utf8       118 MiB/s      13.6 MiB/s    2.04 MiB/s
//! lcs                 / ascii      23  MiB/s      2.54 MiB/s     307 KiB/s
//! lcs                 / utf8       23  MiB/s      2.09 MiB/s     250 KiB/s
//! jaccard_set_bigrams / ascii      8.8 MiB/s      5.03 MiB/s    4.25 MiB/s
//! jaccard_set_bigrams / utf8      12.2 MiB/s      9.28 MiB/s    4.58 MiB/s
//! ```
//!
//! Read:
//!
//! * **Hamming** is dispatched to a block-wise byte compare via
//!   `Hamming::distance_bytes` and reports GiB/s; the O(n) linear scan is
//!   memory-bandwidth-bound, not compute-bound. Every other kernel is
//!   O(m·n) DP and the throughput collapses roughly quadratically as the
//!   input grows — the `256 → 2048` step is where that becomes obvious.
//! * **`damerau_full` vs `damerau_osa`**: the full-Damerau kernel is
//!   `HashMap`-backed (`std`-only) and pays a per-cell hash-probe cost
//!   OSA does not; the `~2×` gap on ASCII inputs at every size is the
//!   expected constant-factor overhead of the "last position of symbol"
//!   auxiliary map.
//! * **Jaro / Jaro-Winkler** at n = 32 sit ~10× the throughput of the DP
//!   kernels because their inner loop is a single O(m + n) matched-pos
//!   sweep, not a full DP fill; the boost applied by Jaro-Winkler is
//!   `O(prefix_limit)` and costs ~10 % on top.
//! * **`jaccard_set_bigrams`** includes both `GramSet` constructions
//!   inside the bench iter — the set-similarity kernel itself is
//!   trivially cheap, so measuring the pipeline the crate's docs
//!   recommend (build both sets, take Jaccard) gives numbers that track
//!   what a real caller pays.
//! * **Regression trip-wire**: this table is the reference the bench
//!   suite is expected to hold to within ±15 %. A number outside that
//!   band on a subsequent run is either a genuine regression or a
//!   measurement environment change (thermal throttling, background
//!   load); rerun with `--sample-size 30` and a quiet host before
//!   filing a fix.

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
pub mod learned;
pub mod levenshtein;
pub mod minhash;
pub mod ngram;
pub mod search;
pub mod set_similarity;
/// Shared helpers used by every byte-slice SIMD sub-tree (`levenshtein::simd`,
/// `jaro::simd`, `damerau::osa::simd`). Compiled only under `--features simd`;
/// see the module docs for the shape of the helper and the reasoning for
/// keeping the per-arch dispatchers per-module.
#[cfg(feature = "simd")]
pub mod simd_dispatch;

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

// learned (Ristad-Yianilos)
#[cfg(feature = "std")]
pub use crate::learned::RistadYianilosEstimator;
#[cfg(feature = "alloc")]
pub use crate::learned::{LearnedEdit, LearnedEditModel};

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
