//! `MinHash` sketches and `LSH` banding for approximate Jaccard-similarity
//! search at scale.
//!
//! This crate is StringCheese's probabilistic layer of the record-linkage
//! story: exact-index structures like `QgramIndex` and `BkTree` scale to
//! low millions of items, and beyond that the memory and query costs
//! demand a *sketch* — a fixed-size, lossy summary of a set from which
//! Jaccard similarity between two sets can be estimated to a controllable
//! variance without materializing either set at query time.
//!
//! # What lives here
//!
//! * [`MinHashSketch`] — a k-permutation `MinHash` sketch (Broder 1997). A
//!   sketch of size `k` estimates Jaccard between two sets with standard
//!   error `~1/sqrt(k)`. Bigger `k` costs proportionally more memory and
//!   more hash computations at construction time and estimator time.
//! * [`OnePermutationMinHashSketch`] — the Li-Owen-Zhang 2012
//!   one-permutation variant with rotation-based densification for empty
//!   bins. A single hash pass over the input rather than `k`, at
//!   comparable large-`k` accuracy to the k-permutation sketch.
//! * [`WeightedMinHashSketch`] — the Ioffe 2010 Consistent Weighted
//!   Sampling variant. Estimates the generalized ("weighted") Jaccard
//!   similarity `Σ min(w_a, w_b) / Σ max(w_a, w_b)` on non-negatively
//!   weighted multisets. Reduces to regular `MinHash` when every weight is
//!   `1`.
//! * [`SimHashSketch`] — Charikar 2002 `SimHash` for cosine similarity: a
//!   64-bit signature of signed random projections whose Hamming distance
//!   approximates the angle between the two multiset-implied vectors.
//! * [`PStableLshSketch`] — Datar-Immorlica-Indyk-Mirrokni 2004
//!   `p`-stable LSH for `L_p` distances (`p ∈ (0, 2]`). Cauchy
//!   projections for `L_1`, Gaussian for `L_2`. Returns an integer
//!   bucket per input; two buckets collide iff the vectors are close in
//!   the target `L_p` metric with probability decreasing in their
//!   distance.
//! * [`LshIndex`] — banded locality-sensitive hashing (Gionis, Indyk,
//!   Motwani 1999) over [`MinHashSketch`]es. Answers approximate
//!   Jaccard-nearest-neighbor queries: hands back candidate item ids whose
//!   sketches band-collide with the query, without ever computing a full
//!   similarity.
//! * [`hash`] — the hash primitives the k-permutation approach is built
//!   on: a deterministic portable [`hash::PortableHasher`] of any
//!   [`core::hash::Hash`] value, and the [`hash::splitmix64`] finalizer
//!   used to seed independent permutations from a single base seed. Kept
//!   public so tests, benchmarks, and downstream sketches can reuse the
//!   exact same primitives.
//!
//! # Design decision: k-permutation vs one-permutation
//!
//! `MinHash` has two schools:
//!
//! * **K independent hash functions ("k-permutation")** — for a sketch of
//!   size `k`, hash each gram `k` times with `k` independent hash
//!   functions; `sketch[i] = min_g h_i(g)`. Simple to state and to
//!   analyze; the estimator is unambiguously unbiased for every finite `k`;
//!   costs `k · m` hash mixings for `m` grams.
//! * **One-permutation with rotation / densification** — one hash function
//!   plus `k` buckets. `m` hash mixings total, but empty buckets need
//!   densification (Shrivastava 2014's optimal densification, or Li's
//!   original rotation trick) to preserve unbiasedness, and the analysis
//!   of the estimator's variance is subtly different from the
//!   k-permutation case.
//!
//! This crate ships **both** variants — [`MinHashSketch`] (k-permutation)
//! and [`OnePermutationMinHashSketch`] (one-permutation with rotation
//! densification). Their variant slugs are distinct
//! (`"minhash-k-permutation-*"` vs `"minhash-one-permutation-*"`) so a
//! golden case for one cannot silently be validated against the other.
//! Callers who want unambiguous unbiasedness at every `k` should prefer
//! the k-permutation variant; callers whose input-side hash-CPU
//! dominates (very large corpora, large `k`) should prefer the
//! one-permutation variant.
//!
//! # Design decision: hash function
//!
//! `MinHash`'s statistical guarantees depend on the hash function being
//! effectively pairwise independent — collisions between distinct grams
//! should be as rare as chance would predict for a uniform random hash.
//! In practice this means avoiding a hash whose collisions correlate with
//! the input's structural properties (a poorly-mixed hash of ASCII
//! trigrams that clusters at power-of-two moduli, for example).
//!
//! This crate hand-rolls two primitives rather than pulling a dep:
//!
//! * [`hash::PortableHasher`] — a stable `FNV-1a`-derived byte hasher. Used as
//!   the *base* gram hash. `FNV`'s ostensibly weak mixing is not a problem
//!   here because its output is immediately fed to `splitmix64` per
//!   permutation. `FNV` alone would fail `MinHash`'s statistical assumptions;
//!   `FNV` plus `splitmix64` per permutation empirically does not (this is
//!   exactly the trick standard hash-map benchmarks use to make
//!   `FxHash`-like schemes acceptable for `MinHash`).
//! * [`hash::splitmix64`] — `SplitMix64`'s finalizer (from Sebastiano
//!   Vigna's `xoshiro` family), applied as `splitmix64(base_hash XOR
//!   permutation_seed_i)`. `splitmix64` has excellent avalanche and
//!   passes `SmallCrush` and Crush without failure, which is what we need
//!   for the per-permutation independence assumption to hold in practice.
//!
//! The tradeoff is that we do not use a benchmark-verified hash like
//! `xxh3` or `wyhash`. The [`property_tests`][] module's statistical
//! unbiasedness assertion is the safety net: any real regression in the
//! hash's mixing would fail there long before it corrupted production
//! output. In exchange, the crate has zero non-StringCheese runtime
//! dependencies.
//!
//! [`property_tests`]: https://github.com/tegmentum/stringcheese
//!
//! # Empty-vs-empty convention
//!
//! Two empty sets are treated as **identical**: [`MinHashSketch::from_iter`]
//! over an empty input produces an all-sentinel sketch, and two
//! all-sentinel sketches compare identical, yielding
//! [`MinHashSketch::estimated_jaccard`] `= 1.0` bit-exactly. This matches
//! the convention `stringcheese-set-similarity` adopts for its `Jaccard`,
//! `Dice`, `Overlap`, and `Cosine` families, and gives golden and
//! property tests a clean starting point. The sentinel used is
//! [`u64::MAX`] — a value that is exceedingly unlikely to be produced by
//! `splitmix64` on any realistic gram hash (probability ~2^-64 per
//! permutation).
//!
//! # `AlgorithmDescriptor` scope
//!
//! Only the *comparison* the crate performs carries a descriptor:
//!
//! * The Jaccard estimator on [`MinHashSketch`]
//!   ([`sketch::MINHASH_JACCARD_DESCRIPTOR`]).
//! * The Jaccard estimator on [`OnePermutationMinHashSketch`]
//!   ([`one_permutation::ONE_PERMUTATION_MINHASH_JACCARD_DESCRIPTOR`]).
//! * The weighted-Jaccard estimator on [`WeightedMinHashSketch`]
//!   ([`weighted::WEIGHTED_MINHASH_JACCARD_DESCRIPTOR`]).
//! * The cosine estimator on [`SimHashSketch`]
//!   ([`simhash::SIMHASH_COSINE_DESCRIPTOR`]).
//! * The `p`-stable LSH hash function on [`PStableLshSketch`]
//!   ([`p_stable::P_STABLE_LSH_L1_DESCRIPTOR`] for `L_1`,
//!   [`p_stable::P_STABLE_LSH_L2_DESCRIPTOR`] for `L_2`).
//!
//! [`LshIndex`] itself is *representation and infrastructure* — it
//! organizes inputs and outputs for the estimator but does not implement
//! a distinct comparison. This mirrors the same rule
//! `stringcheese-index` follows for BK-tree / VP-tree / q-gram-index: index
//! structures wrap descriptor-carrying algorithms but do not themselves
//! carry a descriptor.
//!
//! # Sequence type
//!
//! Every sketch is generic over the gram type `G: Hash`. Callers pass any
//! hashable type — `u8`, `char`, `&[u8]`, an owned `Vec<char>` gram from
//! `stringcheese-ngram`, or a domain-specific token. The crate never makes a
//! representation choice on the caller's behalf.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. Every type in it requires heap
//! allocation (a `Vec` of signatures, a `BTreeMap` of band postings), so
//! **the entire public surface is behind the `alloc` feature.** Under
//! `--no-default-features` the crate compiles to an empty module, matching
//! the pattern established by `stringcheese-ngram`, `stringcheese-set-similarity`,
//! and `stringcheese-index`.

#[cfg(feature = "alloc")]
pub mod hash;
#[cfg(feature = "alloc")]
pub mod lsh;
#[cfg(feature = "alloc")]
pub mod one_permutation;
// `p`-stable LSH uses `f64::tan`, `f64::cos`, `f64::ln`, and `f64::sqrt`
// (via inversion-sampling Cauchy and Box-Muller Gaussian) — all `std`
// operations. Gated on `std` for the same reason
// `stringcheese-set-similarity`'s `Cosine` and `minhash::weighted` are.
#[cfg(all(feature = "alloc", feature = "std"))]
pub mod p_stable;
#[cfg(feature = "alloc")]
pub mod simhash;
#[cfg(feature = "alloc")]
pub mod sketch;
// Weighted `MinHash` uses `f64::ln`, `f64::exp`, and `f64::floor` — `std`
// operations, not `core`. Gated on `std` for the same reason
// `stringcheese-set-similarity`'s `Cosine` is `std`-gated.
#[cfg(all(feature = "alloc", feature = "std"))]
pub mod weighted;

#[cfg(all(test, feature = "alloc"))]
mod golden;

#[cfg(all(test, feature = "alloc"))]
mod property_tests;

#[cfg(feature = "alloc")]
pub use lsh::LshIndex;
#[cfg(feature = "alloc")]
pub use one_permutation::{
    ONE_PERMUTATION_MINHASH_JACCARD_DESCRIPTOR, OnePermutationMinHashSketch,
};
#[cfg(all(feature = "alloc", feature = "std"))]
pub use p_stable::{
    P_STABLE_LSH_L1_DESCRIPTOR, P_STABLE_LSH_L2_DESCRIPTOR, PStableFamily, PStableLshSketch,
};
#[cfg(feature = "alloc")]
pub use simhash::{SIMHASH_COSINE_DESCRIPTOR, SimHashSketch};
#[cfg(feature = "alloc")]
pub use sketch::{MINHASH_JACCARD_DESCRIPTOR, MinHashSketch};
#[cfg(all(feature = "alloc", feature = "std"))]
pub use weighted::{WEIGHTED_MINHASH_JACCARD_DESCRIPTOR, WeightedMinHashSketch};
