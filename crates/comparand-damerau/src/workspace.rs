//! Reusable scratch buffers for the Damerau family of edit-distance kernels.
//!
//! Two workspaces live here — one per algorithm — because the two variants
//! have different DP shapes and it would be misleading to conflate their
//! capacities. Both hold a single flat `Vec<u32>`; the internal layout is
//! private to each kernel.
//!
//! # [`OsaWorkspace`]
//!
//! Holds the three rolling rows the OSA DP needs to check the transposition
//! candidate at `d[i-2][j-2]`. Capacity is measured in `u32` cells; the
//! rolling-rows and banded kernels grow it to `3 · (min(m, n) + 1)` cells
//! and leave it at that size on return.
//!
//! # [`DamerauWorkspace`]
//!
//! Holds the full Lowrance-Wagner DP matrix. Capacity is measured in `u32`
//! cells; the production kernel grows it to `(m + 1) · (n + 1)` cells and
//! leaves it at that size on return. The auxiliary "last position of
//! symbol" `HashMap<&T, usize>` is *not* held here — its keys borrow from
//! the input slice for that specific call, so it cannot outlive the call
//! frame. The production kernel allocates a fresh one per call; the DP
//! matrix is the dominant allocation and reuse is where the win lies.

use alloc::vec::Vec;

use comparand_core::Workspace;

/// Scratch buffer used by the [`crate::osa`] kernels.
///
/// The buffer holds three rolling rows of the OSA dynamic-programming
/// matrix concatenated end-to-end. Callers do not need to size it
/// themselves — the kernels grow it as needed via
/// [`Workspace::ensure_capacity`].
#[derive(Debug, Default, Clone)]
pub struct OsaWorkspace {
    /// The scratch cells. Length is grown to at least the largest capacity
    /// any kernel has requested; contents are meaningless between calls.
    data: Vec<u32>,
}

impl OsaWorkspace {
    /// Constructs an empty workspace with no allocated cells.
    ///
    /// The first kernel call will grow the workspace to fit its needs.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Constructs a workspace with at least `cells` cells of allocated
    /// capacity.
    ///
    /// Useful when the caller knows the maximum comparison size up front
    /// (for example, a batch runner comparing a fixed-length query against a
    /// corpus) and wants to guarantee a single allocation.
    #[inline]
    #[must_use]
    pub fn with_capacity(cells: usize) -> Self {
        // `vec![0; cells]` compiles to `alloc_zeroed`, which is materially
        // faster than reserve-then-resize on any allocator that hands back
        // pre-zeroed pages.
        Self {
            data: alloc::vec![0; cells],
        }
    }

    /// Returns a mutable slice covering at least `required` cells,
    /// resizing the underlying buffer if needed.
    ///
    /// The returned slice's contents are treated as scratch by every caller
    /// — no cell is assumed to hold any particular value on entry.
    #[inline]
    pub(crate) fn buffer_mut(&mut self, required: usize) -> &mut [u32] {
        self.ensure_capacity(required);
        &mut self.data[..required]
    }
}

impl Workspace for OsaWorkspace {
    #[inline]
    fn ensure_capacity(&mut self, required: usize) {
        if self.data.len() < required {
            self.data.resize(required, 0);
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

/// Scratch buffer used by the [`crate::damerau`] production kernel.
///
/// The buffer holds the full `(m + 1) · (n + 1)` DP matrix in row-major
/// layout. Callers do not need to size it themselves — the production
/// kernel grows it as needed via [`Workspace::ensure_capacity`].
///
/// The auxiliary "last position of symbol in `a`" table is not stored here:
/// its keys are `&T` borrows into the input slices, whose lifetime is
/// bounded by the call. Keeping the DP matrix reusable is where the win
/// lies; the auxiliary table is small relative to the matrix.
///
/// # Availability
///
/// The workspace is only useful with the production kernel, which is gated
/// on the crate's `std` feature. Under `--no-default-features --features
/// alloc` this type is therefore not available; use the Damerau oracle
/// (which allocates its own buffer per call) instead.
#[cfg(feature = "std")]
#[derive(Debug, Default, Clone)]
pub struct DamerauWorkspace {
    /// The scratch cells. Length is grown to at least the largest capacity
    /// any kernel has requested; contents are meaningless between calls.
    data: Vec<u32>,
}

#[cfg(feature = "std")]
impl DamerauWorkspace {
    /// Constructs an empty workspace with no allocated cells.
    ///
    /// The first kernel call will grow the workspace to fit its needs.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Constructs a workspace with at least `cells` cells of allocated
    /// capacity.
    #[inline]
    #[must_use]
    pub fn with_capacity(cells: usize) -> Self {
        Self {
            data: alloc::vec![0; cells],
        }
    }

    /// Returns a mutable slice covering at least `required` cells,
    /// resizing the underlying buffer if needed.
    #[inline]
    pub(crate) fn buffer_mut(&mut self, required: usize) -> &mut [u32] {
        self.ensure_capacity(required);
        &mut self.data[..required]
    }
}

#[cfg(feature = "std")]
impl Workspace for DamerauWorkspace {
    #[inline]
    fn ensure_capacity(&mut self, required: usize) {
        if self.data.len() < required {
            self.data.resize(required, 0);
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
    fn osa_new_starts_empty() {
        let ws = OsaWorkspace::new();
        assert_eq!(ws.capacity(), 0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn damerau_new_starts_empty() {
        let ws = DamerauWorkspace::new();
        assert_eq!(ws.capacity(), 0);
    }

    #[test]
    fn osa_with_capacity_preallocates() {
        let ws = OsaWorkspace::with_capacity(32);
        assert!(ws.capacity() >= 32);
    }

    #[cfg(feature = "std")]
    #[test]
    fn damerau_with_capacity_preallocates() {
        let ws = DamerauWorkspace::with_capacity(64);
        assert!(ws.capacity() >= 64);
    }

    #[test]
    fn osa_ensure_capacity_grows_but_does_not_shrink() {
        let mut ws = OsaWorkspace::new();
        ws.ensure_capacity(16);
        let after_grow = ws.capacity();
        assert!(after_grow >= 16);
        ws.ensure_capacity(4);
        assert_eq!(
            ws.capacity(),
            after_grow,
            "shrinking on smaller request is a bug"
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn damerau_ensure_capacity_grows_but_does_not_shrink() {
        let mut ws = DamerauWorkspace::new();
        ws.ensure_capacity(16);
        let after_grow = ws.capacity();
        assert!(after_grow >= 16);
        ws.ensure_capacity(4);
        assert_eq!(ws.capacity(), after_grow);
    }

    #[test]
    fn osa_buffer_mut_returns_requested_length() {
        let mut ws = OsaWorkspace::new();
        assert_eq!(ws.buffer_mut(7).len(), 7);
        assert_eq!(ws.buffer_mut(3).len(), 3);
        assert_eq!(ws.buffer_mut(11).len(), 11);
    }

    #[cfg(feature = "std")]
    #[test]
    fn damerau_buffer_mut_returns_requested_length() {
        let mut ws = DamerauWorkspace::new();
        assert_eq!(ws.buffer_mut(9).len(), 9);
        assert_eq!(ws.buffer_mut(1).len(), 1);
        assert_eq!(ws.buffer_mut(21).len(), 21);
    }

    #[test]
    fn osa_shrink_releases_capacity() {
        let mut ws = OsaWorkspace::with_capacity(128);
        assert!(ws.capacity() >= 128);
        ws.ensure_capacity(4);
        ws.shrink();
        assert!(ws.capacity() <= 128);
    }

    #[cfg(feature = "std")]
    #[test]
    fn damerau_shrink_releases_capacity() {
        let mut ws = DamerauWorkspace::with_capacity(256);
        assert!(ws.capacity() >= 256);
        ws.ensure_capacity(4);
        ws.shrink();
        assert!(ws.capacity() <= 256);
    }
}
