//! Allocation-counting harness for Comparand algorithms.
//!
//! Wraps `dhat-rs`'s `HeapStats::get()` in a before/after delta helper. The
//! result is an exact heap-allocation count and byte total for the closure
//! that ran between the two snapshots — deterministic (unlike criterion's
//! sampling) and independent of wall-clock timing.
//!
//! # Requirements
//!
//! Every process that calls [`measure`] must have a live [`dhat::Profiler`]
//! guard active for the entire duration of the measurement, and must install
//! [`dhat::Alloc`] as the `#[global_allocator]`. Otherwise `HeapStats::get()`
//! panics. The `src/bin/alloc_report_*.rs` binaries in this crate show the
//! canonical wiring.
//!
//! # A note on the `max_*` deltas
//!
//! `HeapStats::max_blocks` and `HeapStats::max_bytes` are the *global* peaks
//! observed since the profiler started, so
//! `after.max_bytes.saturating_sub(before.max_bytes)` is only informative
//! when this call actually pushed the peak past its previous value — a call
//! that peaks below an earlier high-water mark contributes zero. Prefer
//! `total_blocks` / `total_bytes` for reporting per-call allocation volume;
//! the `max_*` deltas are provided so a caller who cares about the "did this
//! call ever grow the live footprint" question can still ask it, but the
//! answer is only meaningful when the outer test orders calls carefully or
//! resets the process for each measurement.

use dhat::HeapStats;

/// The delta between two [`dhat::HeapStats`] snapshots taken around a
/// user closure.
///
/// All fields are widened to `u64` even though dhat exposes `max_*` as
/// `usize` — a uniform width lets callers print, aggregate, and diff the
/// values without per-field casting.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct AllocMeasurement {
    /// Number of heap allocations performed inside the measured closure.
    pub total_blocks: u64,
    /// Total number of bytes allocated inside the measured closure.
    pub total_bytes: u64,
    /// Increase in the process-wide peak live-allocation count. See the
    /// module-level "note on the `max_*` deltas" for what this measures
    /// and what it does not.
    pub max_blocks: u64,
    /// Increase in the process-wide peak live-byte count. See the
    /// module-level "note on the `max_*` deltas" for what this measures
    /// and what it does not.
    pub max_bytes: u64,
}

/// Runs `f` between two [`dhat::HeapStats`] snapshots and returns both the
/// result of `f` and the allocation delta observed across the call.
///
/// # Panics
///
/// Panics if no [`dhat::Profiler`] is running in heap-profiling mode, per
/// [`HeapStats::get`]'s contract. Callers must ensure a `Profiler` guard is
/// live for the entire duration of the measurement.
pub fn measure<T>(f: impl FnOnce() -> T) -> (T, AllocMeasurement) {
    let before = HeapStats::get();
    let result = f();
    let after = HeapStats::get();
    let measurement = AllocMeasurement {
        total_blocks: after.total_blocks - before.total_blocks,
        total_bytes: after.total_bytes - before.total_bytes,
        // `max_*` are `usize` in `dhat` 0.3.x, widen to `u64` so the whole
        // struct has one integer width and downstream printers do not need
        // per-field casting.
        max_blocks: (after.max_blocks as u64).saturating_sub(before.max_blocks as u64),
        max_bytes: (after.max_bytes as u64).saturating_sub(before.max_bytes as u64),
    };
    (result, measurement)
}
