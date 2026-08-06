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
//! Holds the full Lowrance-Wagner DP matrix *and* the "last position of
//! symbol in `a`" `HashMap` the production kernel uses to lift the
//! transposition-source lookup to amortized `O(1)`. Capacity for the DP
//! matrix is measured in `u32` cells; the production kernel grows it to
//! `(m + 1) · (n + 1)` cells and leaves it at that size on return. The
//! `HashMap` is cleared (retaining its capacity) at the top of every call so
//! the previous call's keys are gone before the new call inserts.
//!
//! ## Why the workspace is generic in `T`
//!
//! The auxiliary table maps *symbol values from `a`* to the row where each
//! was last seen. Holding it in the workspace across calls therefore forces
//! the workspace to name the symbol type, because the table stores owned
//! `T` keys (we cannot store `&T` — the borrow's lifetime would be shorter
//! than the workspace's). The generic parameter defaults to `u8`, which
//! matches the byte-slice callers StringCheese ships (batch benches, the
//! alloc-report harness, the golden runners); callers over `&[char]` or
//! any other `T: Eq + Hash + Clone` construct
//! `DamerauWorkspace::<T>::new()` explicitly.
//!
//! ## Why `T: Clone`
//!
//! The workspace's `HashMap` owns its keys so the workspace can outlive the
//! call that inserted them (and be cleared and reused on the next call).
//! Owning keys means cloning them on insert. For byte slices and `char`
//! slices — the typical Damerau inputs — `Clone` is `Copy` and free. For a
//! non-`Copy` `T` the clone cost is dominated by the DP matrix work and
//! remains negligible.

use alloc::vec::Vec;

use stringcheese_core::Workspace;

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

// ---------------------------------------------------------------------------
// DamerauWorkspace
// ---------------------------------------------------------------------------

#[cfg(feature = "std")]
use core::hash::Hash;
#[cfg(feature = "std")]
use std::collections::HashMap;

/// Scratch buffer used by the [`crate::damerau`] production kernel.
///
/// Holds the full `(m + 1) · (n + 1)` DP matrix in row-major layout and the
/// auxiliary "last position of each symbol of `a`" `HashMap` the
/// production kernel needs to keep the transposition-source lookup at
/// amortized `O(1)`. Callers do not need to size either — the production
/// kernel grows the DP matrix and reuses the pre-allocated `HashMap`
/// automatically.
///
/// # Type parameter
///
/// `T` is the *symbol type* — the element type of the sequences the
/// workspace will be compared against. It defaults to `u8` so that
/// byte-slice callers can construct the workspace with the unadorned
/// `DamerauWorkspace::new()`. Callers over other symbol types write
/// `DamerauWorkspace::<char>::new()` (or similar).
///
/// The bound is `T: Eq + Hash + Clone`. `Eq + Hash` are required by
/// `HashMap`. `Clone` is required because the workspace owns its keys —
/// the `HashMap` outlives any single kernel call, so it cannot hold
/// borrowed keys.
///
/// # Availability
///
/// The workspace is only useful with the production kernel, which is gated
/// on the crate's `std` feature. Under `--no-default-features --features
/// alloc` this type is therefore not available; use the Damerau oracle
/// (which allocates its own buffer per call) instead.
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct DamerauWorkspace<T: Eq + Hash + Clone = u8> {
    /// The DP-matrix scratch cells. Length is grown to at least the
    /// largest capacity any kernel has requested; contents are meaningless
    /// between calls.
    data: Vec<u32>,
    /// The "last position of each symbol of `a`" table. Cleared (retaining
    /// its capacity) at the top of every call, so previously-inserted keys
    /// never leak into a subsequent comparison.
    last_positions: HashMap<T, usize>,
}

#[cfg(feature = "std")]
impl<T: Eq + Hash + Clone> Default for DamerauWorkspace<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl<T: Eq + Hash + Clone> DamerauWorkspace<T> {
    /// Constructs an empty workspace with no allocated cells.
    ///
    /// The first kernel call will grow the workspace to fit its needs.
    ///
    /// Not a `const fn` because [`HashMap::new`] is not `const` on the
    /// crate's MSRV. Callers that need a `const` initializer can wrap this
    /// in a `once_cell::sync::OnceCell` or `LazyLock`.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            last_positions: HashMap::new(),
        }
    }

    /// Constructs a workspace with at least `cells` cells of DP-matrix
    /// capacity and `map_cap` entries of `HashMap` capacity.
    ///
    /// The two-arg form lets a caller with prior knowledge of both the
    /// DP-matrix size and the alphabet cardinality guarantee a single
    /// allocation each.
    #[inline]
    #[must_use]
    pub fn with_full_capacity(cells: usize, map_cap: usize) -> Self {
        Self {
            data: alloc::vec![0; cells],
            last_positions: HashMap::with_capacity(map_cap),
        }
    }

    /// Constructs a workspace with at least `cells` cells of DP-matrix
    /// capacity and a `HashMap` sized for the input's alphabet.
    ///
    /// Convenience wrapper around [`with_full_capacity`](Self::with_full_capacity)
    /// that picks a `HashMap` capacity proportional to `cells` — enough for
    /// most inputs to avoid a rehash on first fill.
    #[inline]
    #[must_use]
    pub fn with_capacity(cells: usize) -> Self {
        // A reasonable heuristic for the auxiliary map: it is bounded above
        // by the alphabet of `a`, which is bounded above by `m`. The DP
        // matrix is `(m + 1) · (n + 1)` cells, so `sqrt(cells)` roughly
        // approximates `m` for square inputs. Undershoot rehashes; a
        // slightly-generous pick avoids that at negligible cost.
        let map_cap = {
            // Integer sqrt via a couple of Newton iterations is plenty here.
            // For `cells = 1089` (33·33) we get ~33 — matching the typical
            // small-alphabet Damerau input.
            let mut x = cells;
            let mut y = 1_usize;
            while x > y {
                x = usize::midpoint(x, y);
                y = cells / x.max(1);
            }
            x.max(1)
        };
        Self::with_full_capacity(cells, map_cap)
    }

    /// Split-borrow accessor for the production kernel.
    ///
    /// Returns disjoint mutable references to (a) a DP-matrix slice sized
    /// to `required` cells and (b) the auxiliary `HashMap`, cleared so the
    /// previous call's keys are gone but its capacity is retained. Both
    /// borrows are held simultaneously by the kernel; a two-call sequence
    /// of separate `data`- and `last_positions`-focused accessors would
    /// fail to compile under the borrow checker.
    #[inline]
    pub(crate) fn split_mut(&mut self, required: usize) -> (&mut [u32], &mut HashMap<T, usize>) {
        if self.data.len() < required {
            self.data.resize(required, 0);
        }
        self.last_positions.clear();
        (&mut self.data[..required], &mut self.last_positions)
    }

    /// Returns the current `HashMap` capacity in entries.
    ///
    /// Intended for tests and diagnostics that want to observe whether the
    /// per-call clear-and-reuse cycle is preserving the pre-allocated
    /// capacity.
    #[doc(hidden)]
    #[must_use]
    pub fn map_capacity(&self) -> usize {
        self.last_positions.capacity()
    }
}

#[cfg(feature = "std")]
impl<T: Eq + Hash + Clone> Workspace for DamerauWorkspace<T> {
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
        self.last_positions.shrink_to_fit();
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
        let ws: DamerauWorkspace = DamerauWorkspace::new();
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
        let ws: DamerauWorkspace = DamerauWorkspace::with_capacity(64);
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
        let mut ws: DamerauWorkspace = DamerauWorkspace::new();
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
    fn damerau_split_mut_returns_requested_length() {
        let mut ws: DamerauWorkspace = DamerauWorkspace::new();
        assert_eq!(ws.split_mut(9).0.len(), 9);
        assert_eq!(ws.split_mut(1).0.len(), 1);
        assert_eq!(ws.split_mut(21).0.len(), 21);
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
        let mut ws: DamerauWorkspace = DamerauWorkspace::with_capacity(256);
        assert!(ws.capacity() >= 256);
        ws.ensure_capacity(4);
        ws.shrink();
        assert!(ws.capacity() <= 256);
    }

    #[cfg(feature = "std")]
    #[test]
    fn damerau_split_mut_clears_and_keeps_capacity() {
        let mut ws: DamerauWorkspace<u8> = DamerauWorkspace::with_full_capacity(8, 32);
        // First call: pretend to fill the map.
        {
            let (_d, map) = ws.split_mut(8);
            map.insert(b'a', 1);
            map.insert(b'b', 2);
            assert_eq!(map.len(), 2);
        }
        let cap_after_first = ws.map_capacity();
        assert!(cap_after_first >= 32, "pre-allocated map capacity lost");
        // Second call: split_mut clears before returning.
        {
            let (_d, map) = ws.split_mut(8);
            assert_eq!(map.len(), 0, "map should be cleared before reuse");
            assert!(
                map.capacity() >= cap_after_first,
                "capacity should be retained across clear()"
            );
        }
    }
}
