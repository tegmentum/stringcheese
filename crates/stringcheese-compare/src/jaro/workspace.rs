//! Reusable scratch buffer for the Jaro matching kernel.
//!
//! The Jaro similarity kernel allocates two boolean bitmaps per call —
//! `a_matched` and `b_matched` — tracking which positions in each input
//! have been consumed by a match. On workloads that compare a fixed query
//! against a corpus of candidates (record linkage, deduplication) those
//! per-call allocations become a large fraction of runtime.
//!
//! A [`JaroWorkspace`] holds those two [`Vec<bool>`] buffers and is passed
//! back into successive calls to
//! [`Jaro::similarity_with_workspace`](crate::jaro::jaro::Jaro::similarity_with_workspace)
//! or
//! [`JaroWinkler::similarity_with_workspace`](crate::jaro::jaro_winkler::JaroWinkler::similarity_with_workspace).
//! The workspace implements [`Workspace`] from `stringcheese-core`, so
//! generic batch infrastructure can manage it without knowing its internal
//! layout.
//!
//! # Sizing
//!
//! The Jaro kernel needs `|a|` cells in `a_matched` and `|b|` cells in
//! `b_matched`. Each call grows both buffers as needed via
//! [`Workspace::ensure_capacity`] with the sum `|a| + |b|` — the kernel
//! then split-borrows both halves out of the workspace in one shot.

use alloc::vec::Vec;

use stringcheese_core::Workspace;

/// Scratch buffer used by the Jaro matching kernel.
///
/// Holds two [`Vec<bool>`] buffers concatenated end-to-end into a single
/// allocation. Callers do not need to size it themselves — the kernel
/// grows it as needed via [`Workspace::ensure_capacity`].
///
/// # Sizing convention
///
/// The workspace's capacity unit is *total cells across both bitmaps*; a
/// call that compares an `m`-symbol input against an `n`-symbol input asks
/// for `m + n` cells. The kernel split-borrows the first `m` cells as
/// `a_matched` and the remaining `n` as `b_matched`.
#[derive(Debug, Default, Clone)]
pub struct JaroWorkspace {
    /// The concatenated matched-position bitmaps. Length is grown to at
    /// least the largest total capacity any kernel has requested; the two
    /// halves are split at `|a|` at the top of every call.
    ///
    /// The bytes are treated as scratch — every cell is reset to `false`
    /// at the start of the kernel call before any match bookkeeping runs.
    data: Vec<bool>,
}

impl JaroWorkspace {
    /// Constructs an empty workspace with no allocated cells.
    ///
    /// The first kernel call will grow the workspace to fit its needs.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Constructs a workspace with at least `cells` total cells of
    /// allocated capacity.
    ///
    /// Useful when the caller knows the maximum comparison size up front
    /// (for example, a batch runner comparing a fixed-length query against
    /// a corpus) and wants to guarantee a single allocation. Sizing here
    /// counts *both* bitmaps: a run of `m`-vs-`n` comparisons should pass
    /// `m + n`.
    #[inline]
    #[must_use]
    pub fn with_capacity(cells: usize) -> Self {
        Self {
            data: alloc::vec![false; cells],
        }
    }

    /// Split-borrow accessor for the Jaro kernel.
    ///
    /// Returns two disjoint mutable slices of length `len_a` and `len_b`
    /// respectively, both zeroed to `false` regardless of the previous
    /// call's contents. The buffer is grown to `len_a + len_b` cells if
    /// needed.
    #[inline]
    pub(crate) fn split_bitmaps_mut(
        &mut self,
        len_a: usize,
        len_b: usize,
    ) -> (&mut [bool], &mut [bool]) {
        let required = len_a + len_b;
        if self.data.len() < required {
            self.data.resize(required, false);
        }
        // Reset the cells we're about to hand out; leave anything past the
        // request alone (the extra capacity may be re-used by a larger
        // later call).
        for cell in &mut self.data[..required] {
            *cell = false;
        }
        let (a, b) = self.data[..required].split_at_mut(len_a);
        (a, b)
    }
}

impl Workspace for JaroWorkspace {
    #[inline]
    fn ensure_capacity(&mut self, required: usize) {
        if self.data.len() < required {
            self.data.resize(required, false);
        }
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.data.capacity()
    }

    #[inline]
    fn shrink(&mut self) {
        self.data.shrink_to_fit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_empty() {
        let ws = JaroWorkspace::new();
        assert_eq!(ws.capacity(), 0);
    }

    #[test]
    fn with_capacity_preallocates() {
        let ws = JaroWorkspace::with_capacity(64);
        assert!(ws.capacity() >= 64);
    }

    #[test]
    fn ensure_capacity_grows_but_does_not_shrink() {
        let mut ws = JaroWorkspace::new();
        ws.ensure_capacity(16);
        let after = ws.capacity();
        assert!(after >= 16);
        ws.ensure_capacity(4);
        assert_eq!(
            ws.capacity(),
            after,
            "shrinking on smaller request would be a bug"
        );
    }

    #[test]
    fn split_bitmaps_returns_disjoint_halves_at_requested_lengths() {
        let mut ws = JaroWorkspace::new();
        let (a, b) = ws.split_bitmaps_mut(3, 5);
        assert_eq!(a.len(), 3);
        assert_eq!(b.len(), 5);
        for cell in a.iter().chain(b.iter()) {
            assert!(!*cell, "cells should be zeroed on split");
        }
    }

    #[test]
    fn split_bitmaps_resets_dirty_cells_between_calls() {
        let mut ws = JaroWorkspace::new();
        {
            let (a, b) = ws.split_bitmaps_mut(2, 2);
            a[0] = true;
            b[1] = true;
        }
        let (a, b) = ws.split_bitmaps_mut(2, 2);
        assert_eq!(a, &[false, false]);
        assert_eq!(b, &[false, false]);
    }

    #[test]
    fn shrink_releases_capacity() {
        let mut ws = JaroWorkspace::with_capacity(128);
        assert!(ws.capacity() >= 128);
        ws.ensure_capacity(4);
        ws.shrink();
        assert!(ws.capacity() <= 128);
    }
}
