//! Substring search algorithms for the StringCheese toolkit.
//!
//! This crate ships four canonical single- and multi-pattern search
//! algorithms behind a small, common interface. Every algorithm is
//! independently derived, byte-oriented, and validated against the others
//! via a cross-algorithm differential property suite so a bug in any single
//! implementation is likely to surface as a disagreement rather than as a
//! silent wrong answer.
//!
//! # Algorithms
//!
//! * [`rabin_karp`] — polynomial rolling-hash search (`Rabin`, `Karp` 1987).
//!   Uses a Mersenne prime modulus (`2^61 − 1`) with base `257`; hash matches
//!   are always verified byte-by-byte, so a hash collision cannot produce a
//!   false positive.
//! * [`kmp`] — Knuth-Morris-Pratt (`Knuth`, `Morris`, `Pratt` 1977). Builds
//!   the classical failure function and scans the haystack in a single pass
//!   that never re-reads any byte.
//! * [`boyer_moore`] — Boyer-Moore search shipped as two related
//!   handles: [`BoyerMoore`] with the bad-character heuristic only
//!   (variant slug `"bad-character-only"`) and [`BoyerMooreFull`] with
//!   both the bad-character and the good-suffix heuristics (variant
//!   slug `"full-with-good-suffix"`). Both produce the same match set;
//!   the two variants exist so a golden case pinned to one shape
//!   cannot be silently validated against the other, and so callers
//!   can pick a performance profile.
//! * [`horspool`] — Horspool 1980, a simpler bad-character-only
//!   Boyer-Moore variant that always shifts based on the byte aligned
//!   with the pattern's rightmost position.
//! * [`two_way`] — Crochemore-Perrin 1991 two-way string matching,
//!   `O(1)` extra space at preprocessing and `O(n)` worst-case scan
//!   (for [`SinglePatternSearch::find`]).
//! * [`aho_corasick`] — Aho-Corasick multi-pattern automaton (`Aho`,
//!   `Corasick` 1975). Streams a haystack against an arbitrary set of
//!   patterns in a single pass and reports every match — including
//!   overlapping matches from different patterns.
//! * [`stream`] — streaming state-machine wrappers for the algorithms
//!   that admit them (KMP, Rabin-Karp, Aho-Corasick). Boyer-Moore and
//!   its variants are deliberately omitted; see the module
//!   documentation for the rationale.
//!
//! # Common surface
//!
//! The single-pattern algorithms share the [`SearchAlgorithm`] and
//! [`SinglePatternSearch`] traits from [`api`]. Every search returns
//! [`Match`] values carrying a byte offset and a `pattern_index` (always
//! `0` for single-pattern algorithms; the pattern's position in the input
//! set for Aho-Corasick). Every `find_all` result is returned in ascending
//! `position` order and includes overlapping matches — the algorithms never
//! silently deduplicate.
//!
//! # Byte orientation
//!
//! All algorithms operate on `&[u8]`; there is no `str`-typed entry point.
//! This is a deliberate consequence of StringCheese's rule against silent
//! representation choices: searching in a UTF-8 string for a UTF-8 pattern
//! works correctly at the byte level because UTF-8 is prefix-free, but the
//! caller is responsible for that choice. Cases exercising Unicode inputs
//! in the crate-internal `golden` test module make this explicit.
//!
//! # `no_std`
//!
//! The trait surface in [`api`] is pure `core`. Every algorithm module
//! requires heap allocation (for `Vec`-backed match results and
//! preprocessing tables) and is therefore gated on the `alloc` feature.
//! A build with neither `std` nor `alloc` compiles only the trait surface;
//! this is what makes the crate safe to add as a dependency in embedded
//! configurations that only need the API types.
//!
//! # Descriptors
//!
//! Every algorithm exposes an [`AlgorithmDescriptor`] pinning its variant
//! and source paper. Golden test cases in the crate-internal `golden` test
//! module reference algorithms by descriptor rather than by common name so
//! a "Boyer-Moore" case cannot silently be validated against a future
//! full-good-suffix variant.
//!
//! [`AlgorithmDescriptor`]: stringcheese_core::AlgorithmDescriptor
//!
//! # References
//!
//! * Charras, C., & Lecroq, T. (2004). *Handbook of Exact String-Matching
//!   Algorithms*. King's College Publications. ISBN 0-9543006-2-X. — a
//!   uniform presentation of every algorithm in this crate, useful as
//!   secondary background alongside the primary sources cited on each
//!   module.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod api;

#[cfg(feature = "alloc")]
pub mod aho_corasick;
#[cfg(feature = "alloc")]
pub mod boyer_moore;
#[cfg(feature = "alloc")]
pub mod horspool;
#[cfg(feature = "alloc")]
pub mod kmp;
#[cfg(feature = "alloc")]
pub mod rabin_karp;
#[cfg(feature = "alloc")]
pub mod stream;
#[cfg(feature = "alloc")]
pub mod two_way;

#[cfg(all(test, feature = "alloc"))]
mod golden;

#[cfg(all(test, feature = "alloc"))]
mod property_tests;

#[cfg(feature = "alloc")]
pub use api::SinglePatternSearch;
pub use api::{Match, SearchAlgorithm};

#[cfg(feature = "alloc")]
pub use aho_corasick::AhoCorasick;
#[cfg(feature = "alloc")]
pub use boyer_moore::{BoyerMoore, BoyerMooreFull};
#[cfg(feature = "alloc")]
pub use horspool::Horspool;
#[cfg(feature = "alloc")]
pub use kmp::Kmp;
#[cfg(feature = "alloc")]
pub use rabin_karp::RabinKarp;
#[cfg(feature = "alloc")]
pub use stream::{SearchStream, StreamingSearch};
#[cfg(feature = "alloc")]
pub use two_way::TwoWay;
