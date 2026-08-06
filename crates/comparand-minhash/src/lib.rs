//! `MinHash` sketches and `LSH` banding for approximate Jaccard-similarity
//! search at scale.
//!
//! This crate is Comparand's probabilistic layer of the record-linkage
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
//! * [`WeightedMinHashSketch`] — the Ioffe 2010 Consistent Weighted
//!   Sampling variant. Estimates the generalized ("weighted") Jaccard
//!   similarity `Σ min(w_a, w_b) / Σ max(w_a, w_b)` on non-negatively
//!   weighted multisets. Reduces to regular `MinHash` when every weight is
//!   `1`.
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
//! This crate ships **k-permutation** as its v0.1 approach. Rationale:
//!
//! * The estimator is unambiguously unbiased; there is no densification
//!   subroutine that could ship with a subtle correctness defect.
//! * The property tests can hold the estimator to `E[jhat] ≈ J` at every
//!   `k` without disentangling densification-induced biases.
//! * The extra hash mixings are cheap — `splitmix64` is a handful of
//!   scalar operations — and `MinHash`'s practical bottleneck at large
//!   scale is I/O, not hash CPU.
//!
//! A one-permutation variant remains a plausible future sibling; if it
//! lands, its variant slug will be distinct (`"minhash-one-permutation-*"`)
//! so a golden case for one variant cannot be silently validated against
//! the other.
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
//! output. In exchange, the crate has zero non-Comparand runtime
//! dependencies.
//!
//! [`property_tests`]: https://github.com/tegmentum/comparand
//!
//! # Empty-vs-empty convention
//!
//! Two empty sets are treated as **identical**: [`MinHashSketch::from_iter`]
//! over an empty input produces an all-sentinel sketch, and two
//! all-sentinel sketches compare identical, yielding
//! [`MinHashSketch::estimated_jaccard`] `= 1.0` bit-exactly. This matches
//! the convention `comparand-set-similarity` adopts for its `Jaccard`,
//! `Dice`, `Overlap`, and `Cosine` families, and gives golden and
//! property tests a clean starting point. The sentinel used is
//! [`u64::MAX`] — a value that is exceedingly unlikely to be produced by
//! `splitmix64` on any realistic gram hash (probability ~2^-64 per
//! permutation).
//!
//! # `AlgorithmDescriptor` scope
//!
//! Only the *comparison* the crate performs carries a descriptor: the
//! Jaccard estimator on [`MinHashSketch`] (see
//! [`sketch::MINHASH_JACCARD_DESCRIPTOR`]) and its weighted counterpart
//! on [`WeightedMinHashSketch`] (see
//! [`weighted::WEIGHTED_MINHASH_JACCARD_DESCRIPTOR`]).
//!
//! The sketch types and [`LshIndex`] themselves are *representation and
//! infrastructure* — they organize inputs and outputs for the estimator
//! but do not implement a distinct comparison. This mirrors the same rule
//! `comparand-index` follows for BK-tree / VP-tree / q-gram-index: index
//! structures wrap descriptor-carrying algorithms but do not themselves
//! carry a descriptor.
//!
//! # Sequence type
//!
//! Every sketch is generic over the gram type `G: Hash`. Callers pass any
//! hashable type — `u8`, `char`, `&[u8]`, an owned `Vec<char>` gram from
//! `comparand-ngram`, or a domain-specific token. The crate never makes a
//! representation choice on the caller's behalf.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. Every type in it requires heap
//! allocation (a `Vec` of signatures, a `BTreeMap` of band postings), so
//! **the entire public surface is behind the `alloc` feature.** Under
//! `--no-default-features` the crate compiles to an empty module, matching
//! the pattern established by `comparand-ngram`, `comparand-set-similarity`,
//! and `comparand-index`.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
#[allow(unused_extern_crates)]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod hash;
#[cfg(feature = "alloc")]
pub mod lsh;
#[cfg(feature = "alloc")]
pub mod sketch;
// Weighted `MinHash` uses `f64::ln`, `f64::exp`, and `f64::floor` — `std`
// operations, not `core`. Gated on `std` for the same reason
// `comparand-set-similarity`'s `Cosine` is `std`-gated.
#[cfg(all(feature = "alloc", feature = "std"))]
pub mod weighted;

#[cfg(all(test, feature = "alloc"))]
mod golden;

#[cfg(all(test, feature = "alloc"))]
mod property_tests;

#[cfg(feature = "alloc")]
pub use lsh::LshIndex;
#[cfg(feature = "alloc")]
pub use sketch::{MINHASH_JACCARD_DESCRIPTOR, MinHashSketch};
#[cfg(all(feature = "alloc", feature = "std"))]
pub use weighted::{WEIGHTED_MINHASH_JACCARD_DESCRIPTOR, WeightedMinHashSketch};
