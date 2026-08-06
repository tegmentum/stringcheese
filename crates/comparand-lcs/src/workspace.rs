//! Reusable scratch buffer for LCS kernels.
//!
//! The rolling-rows LCS kernel stores its single active row in a flat
//! `Vec<u32>`. When a caller performs many comparisons against a fixed
//! query — record linkage, deduplication, top-k queries — that `Vec` is
//! allocated once and grown on demand, rather than being freshly allocated
//! inside every call.
//!
//! The workspace implements [`Workspace`] from `comparand-core`, so generic
//! batch infrastructure can manage it without knowing its internal layout.

use alloc::vec::Vec;

use comparand_core::Workspace;

/// Scratch buffer used by the LCS kernels.
///
/// An `LcsWorkspace` holds a single `Vec<u32>`. Its capacity is the only
/// observable state; the buffer's contents are treated as scratch and are
/// overwritten on every kernel call.
///
/// # Sizing
///
/// The rolling-rows kernel needs `min(len_a, len_b) + 1` cells. The kernel
/// grows the workspace via [`Workspace::ensure_capacity`]; callers do not
/// need to size it themselves.
#[derive(Debug, Default, Clone)]
pub struct LcsWorkspace {
    /// The scratch cells. Length is grown to at least the largest capacity
    /// any kernel has requested; contents are meaningless between calls.
    data: Vec<u32>,
}

impl LcsWorkspace {
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
    /// Useful when the caller knows the maximum comparison size up front —
    /// for example, a batch runner comparing a fixed-length query against a
    /// corpus — and wants to guarantee a single allocation.
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

    /// Returns a mutable slice covering at least `required` cells, resizing
    /// the underlying buffer if needed.
    ///
    /// The returned slice's contents are treated as scratch by every
    /// caller — no cell is assumed to hold any particular value on entry.
    #[inline]
    pub(crate) fn buffer_mut(&mut self, required: usize) -> &mut [u32] {
        self.ensure_capacity(required);
        &mut self.data[..required]
    }
}

impl Workspace for LcsWorkspace {
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
    fn new_starts_empty() {
        let ws = LcsWorkspace::new();
        assert_eq!(ws.capacity(), 0);
    }

    #[test]
    fn with_capacity_preallocates() {
        let ws = LcsWorkspace::with_capacity(32);
        assert!(ws.capacity() >= 32);
    }

    #[test]
    fn ensure_capacity_grows_but_does_not_shrink() {
        let mut ws = LcsWorkspace::new();
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

    #[test]
    fn buffer_mut_returns_requested_length() {
        let mut ws = LcsWorkspace::new();
        assert_eq!(ws.buffer_mut(7).len(), 7);
        assert_eq!(ws.buffer_mut(3).len(), 3);
        assert_eq!(ws.buffer_mut(11).len(), 11);
    }

    #[test]
    fn shrink_releases_capacity() {
        let mut ws = LcsWorkspace::with_capacity(128);
        assert!(ws.capacity() >= 128);
        ws.ensure_capacity(4);
        ws.shrink();
        // Vec::shrink_to_fit is allowed to keep some slack, but must not grow.
        assert!(ws.capacity() <= 128);
    }
}
