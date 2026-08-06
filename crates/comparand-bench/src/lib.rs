//! Benchmark suite and comparative-reporting harness for the Comparand
//! toolkit.
//!
//! # Status
//!
//! v0.1 scope has two harnesses under this crate:
//!
//! - **criterion wall-clock benchmarks** under `benches/`, covering the
//!   five existing algorithm crates (`comparand-levenshtein`,
//!   `comparand-hamming`, `comparand-jaro`, `comparand-damerau`,
//!   `comparand-ngram`). Runs by default with `cargo bench`.
//! - **dhat-rs allocation counting** under `src/bin/alloc_report_*.rs`,
//!   opt-in behind the `alloc-tracking` feature. Reports blocks and bytes
//!   allocated per comparison as TSV to stdout — the raw evidence for the
//!   design doc's workspace-reuse story. Off by default because dhat
//!   installs a global allocator that would skew criterion timings.
//!
//! # Determinism
//!
//! Every helper in [`inputs`] is seeded from a `u64` and threads that seed
//! through a small deterministic RNG (see the module docs). Benchmarks
//! should never depend on process-time or OS entropy so that criterion's
//! sample-to-sample variance is dominated by the machine and not the
//! corpus.
//!
//! # Running the allocation reports
//!
//! ```text
//! cargo run -p comparand-bench --features alloc-tracking \
//!   --bin alloc_report_levenshtein
//! ```
//!
//! Each `alloc_report_*` binary prints a tab-separated table:
//!
//! ```text
//! algorithm  variant  len  regime  blocks  bytes  max_blocks  max_bytes
//! ```
//!
//! Pipe through `column -t` for readability or redirect into a spreadsheet.
//!
//! # Not compared to external libraries
//!
//! Head-to-head benchmarks against `strsim`, `rapidfuzz`, and the Python /
//! Java / JavaScript / C++ / Go ecosystems live under the sibling
//! `bench-adapters/` directory (a standalone workspace), not here — mixing
//! external Rust libraries into the main workspace's Cargo.lock is
//! avoided so a `cargo build --workspace` for library consumers stays
//! minimal.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "std")]
pub mod inputs;

/// Ad-hoc allocation-measurement helpers backed by dhat-rs. Gated on the
/// `alloc-tracking` feature so that a default build never links the heap
/// profiler and the criterion benches never inherit its global allocator.
#[cfg(feature = "alloc-tracking")]
pub mod alloc_harness;

#[cfg(feature = "alloc-tracking")]
pub use alloc_harness::{AllocMeasurement, measure};
