//! Benchmark suite and comparative-reporting harness for the Comparand
//! toolkit.
//!
//! # Status
//!
//! v0.1 scope: criterion-based wall-clock latency benchmarks over the five
//! algorithm crates that currently exist in the workspace
//! (`comparand-levenshtein`, `comparand-hamming`, `comparand-jaro`,
//! `comparand-damerau`, `comparand-ngram`). The bench binaries live under
//! `benches/`; the library itself only exposes shared input-generation
//! helpers under [`inputs`] so that no benchmark has to duplicate corpus
//! construction.
//!
//! # Not measured here
//!
//! Allocation counts, peak resident memory, and Wasm linear-memory growth
//! are explicitly deferred: measuring them properly needs either a
//! `#[global_allocator]` shim (which changes the whole binary's allocator
//! and interferes with criterion's timing) or a specialized harness
//! (dhat-rs, heaptrack). Both are v0.2 work.
//!
//! # Not compared to external libraries
//!
//! Comparative benchmarks against `strsim`, `rapidfuzz`, and the Python /
//! Java / JavaScript / C++ / Go ecosystems are also v0.2 work; each
//! adapter is its own build and language-runtime story, as sketched in
//! `docs/DESIGN.md` ("Comparative Library Benchmarking").
//!
//! # Determinism
//!
//! Every helper in [`inputs`] is seeded from a `u64` and threads that seed
//! through a small deterministic RNG (see the module docs). Benchmarks
//! should never depend on process-time or OS entropy so that criterion's
//! sample-to-sample variance is dominated by the machine and not the
//! corpus.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "std")]
pub mod inputs;
