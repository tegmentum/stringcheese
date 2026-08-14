//! Index structures for large-scale sequence matching with the StringCheese
//! toolkit.
//!
//! This crate is the record-linkage / candidate-generation layer of
//! StringCheese: the code that scales pairwise comparison to millions of records
//! by pruning the search space before an expensive distance kernel ever runs.
//!
//! # What lives here
//!
//! Three complementary index structures, each covering a different retrieval
//! model:
//!
//! * [`BkTree`] — the Burkhard-Keller tree. A metric-space index that groups
//!   items by their distance to a fixed pivot at each level. Answers *range*
//!   queries ("all items within distance `r` of `query`") efficiently by
//!   pruning subtrees whose contents cannot possibly fall within the radius.
//!   Requires a *true metric* (triangle inequality is what makes the pruning
//!   sound).
//! * [`VpTree`] — the Vantage-Point tree. Also a metric-space index, but with
//!   a different partitioning strategy: each node chooses a vantage point and
//!   splits its remaining items by whether they lie closer to or farther from
//!   the vantage than a threshold. Answers both range and *k-nearest*
//!   queries. Also requires a true metric. Supports both incremental
//!   construction (via [`VpTree::insert`]) and balanced bulk construction
//!   from a full corpus (via [`VpTree::from_corpus`]) with median-threshold
//!   partitioning.
//! * [`QgramIndex`] — a q-gram inverted index. A set-similarity candidate
//!   generator, not a metric-space index: for a query with gram set `G`, it
//!   returns items whose gram overlap with `G` could conceivably meet a
//!   threshold without computing full similarities.
//!
//! Together with the small [`length_filter`] helper (a q-gram-friendly length
//! prune based on the Jaccard length bound), these are the pieces most
//! entity-resolution pipelines assemble.
//!
//! [`length_filter`]: crate::prefix_filter::length_filter
//!
//! # Why no `AlgorithmDescriptor`s live here
//!
//! Index structures are *infrastructure*: they wrap descriptor-carrying
//! algorithms (a BK-tree wraps a metric that has a descriptor; a q-gram index
//! is driven by grams from `stringcheese-ngram`) but do not implement a
//! comparison themselves. The [`AlgorithmDescriptor`] scheme in
//! `stringcheese-core` identifies algorithm variants, not the containers that
//! organize their inputs. Golden cases for BK-tree and VP-tree are keyed to
//! the *wrapped metric*'s descriptor — they exercise index-tree correctness
//! against the metric's known outputs, not a new "BK-tree algorithm."
//!
//! This mirrors `stringcheese-ngram`: n-gram *generation* is a representation
//! layer, not a comparison algorithm, and so does not carry an
//! `AlgorithmDescriptor` either.
//!
//! [`AlgorithmDescriptor`]: stringcheese_core::AlgorithmDescriptor
//!
//! # Metric-vs-semimetric enforcement
//!
//! [`BkTree`] and [`VpTree`] traversal is sound only when the wrapped
//! distance function is a true metric — the triangle inequality is exactly
//! what lets the tree prune whole subtrees without visiting them. A
//! semimetric (e.g. OSA per `stringcheese-damerau`) would produce wrong answers
//! silently: pruning would drop items whose true distance is within the
//! requested radius.
//!
//! Both trees therefore refuse to construct over a non-metric input:
//!
//! * [`BkTree::new`] and [`VpTree::new`] panic on non-metric input. This is
//!   the fail-loud default — passing OSA to a BK-tree is a category error and
//!   is almost certainly a bug at the call site.
//! * [`BkTree::try_new`] and [`VpTree::try_new`] return a
//!   [`NotAMetricError`] instead of panicking. Use these when the metric is
//!   assembled dynamically and rejection is a normal control-flow outcome.
//!
//! This mirrors the fallible-vs-panicking split already established in
//! `stringcheese-hamming` for equal-length inputs.
//!
//! # Related structures elsewhere
//!
//! * `MinHash` and locality-sensitive hashing live in the sibling
//!   `stringcheese-minhash` crate — both are probabilistic and warrant their
//!   own statistical tests.
//! * **Sorted-neighborhood blocking** — extracted to the separate
//!   `record-linkage` library. It is unambiguously an entity-resolution
//!   technique (Hernández & Stolfo 1995 is a record-linkage paper); the
//!   generic metric-space and set-similarity index structures here stay,
//!   but candidate generation for record linkage lives with the
//!   record-linkage decision layer.
//!
//! # Sequence type
//!
//! Every index in this crate is generic. [`BkTree`] and [`VpTree`] are
//! generic over both the symbol type `T` and the wrapped metric `M`, which
//! means they operate on any `&[T]` a metric can compare — bytes, `char`s,
//! grapheme clusters, tokens, phonemes. [`QgramIndex`] is generic over its
//! gram type `G`, so it works with grams from `stringcheese-ngram`'s
//! [`GramSet`] as easily as with any other `Ord + Clone` type.
//!
//! [`GramSet`]: https://docs.rs/stringcheese-ngram
//!
//! # Bench baselines
//!
//! `benches/index.rs` is a criterion binary that measures
//! candidate-generation throughput across all three index families at
//! two corpus scales (1 000 and 10 000 records). Run with:
//!
//! ```text
//! cargo bench -p stringcheese-index --bench index
//! ```
//!
//! Baseline numbers (aarch64 Apple M-series, macOS 15, rustc 1.97.1,
//! release + LTO; `--quick` sample budget, ±30 % ballpark):
//!
//! ```text
//! group                                    1 000            10 000
//! ------------------------------------------------------------------
//! index/bk_tree/build                      1.68 M rec/s     1.16 M rec/s
//! index/bk_tree/find_within r=1             48.9 K qps       6.8 K qps
//! index/vp_tree/from_corpus                1.25 M rec/s      907 K rec/s
//! index/vp_tree/find_within r=1             24.9 K qps       2.6 K qps
//! index/vp_tree/find_k_nearest k=5           9.5 K qps        785 elem/s
//! index/qgram/build                        1.21 M rec/s     1.38 M rec/s
//! index/qgram/overlap_candidates o=2         588 K qps        51 K qps
//! index/qgram/length_filter_candidates      1.07 M qps         40 K qps
//! ```
//!
//! See the bench file's module doc for candidate-fanout counts:
//! metric-space trees surface ~6 hits per query at r=1 on the
//! 1 000-record corpus and ~91 on the 10 000-record corpus;
//! `overlap_candidates` surfaces ~193 candidates at 1k and ~1 883 at
//! 10k; `length_filter_candidates` at θ=0.6 surfaces the entire
//! valid length window, ~55 000 at 1k and ~549 000 at 10k.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. Every index in it requires heap
//! allocation for its backing storage, so **the entire public surface is
//! behind the `alloc` feature.** Under `--no-default-features` the crate
//! compiles to an empty module, which is what makes the crate safe to add
//! as a dependency in embedded configurations that only need to link
//! against the substrate crates.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod bk_tree;
#[cfg(feature = "alloc")]
pub mod error;
#[cfg(feature = "alloc")]
pub mod prefix_filter;
#[cfg(feature = "alloc")]
pub mod qgram_index;
#[cfg(feature = "alloc")]
pub mod vp_tree;

#[cfg(all(test, feature = "alloc"))]
mod golden;

#[cfg(all(test, feature = "alloc"))]
#[cfg(not(target_family = "wasm"))]
mod property_tests;

#[cfg(feature = "alloc")]
pub use bk_tree::BkTree;
#[cfg(feature = "alloc")]
pub use error::NotAMetricError;
#[cfg(feature = "alloc")]
pub use prefix_filter::length_filter;
#[cfg(feature = "alloc")]
pub use qgram_index::QgramIndex;
#[cfg(feature = "alloc")]
pub use vp_tree::{VantageStrategy, VpTree};
