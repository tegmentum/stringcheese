//! Reusable scratch buffers for alignment DPs.
//!
//! The workspace stores up to three DP matrices, sized lazily based on the
//! largest allocation seen so far. Two operating modes are supported:
//!
//! * **Score mode** — rolling two-row buffers, `O(min(m, n))` memory. Used
//!   by `score_with_workspace`. Cannot support edit-script reconstruction
//!   because prior rows are overwritten as the DP marches forward.
//! * **Full mode** — three full `(m + 1) * (n + 1)` matrices, `O(m * n)`
//!   memory. Used by `align`. Supports Gotoh-style affine backtrace through
//!   the M / X / Y three-matrix DP.
//!
//! In both modes, capacity only ever grows — reusing a workspace across
//! many alignments amortizes the allocation cost. Call
//! [`AlignmentWorkspace::shrink`] to release excess capacity.

use alloc::vec::Vec;

use stringcheese_core::Workspace;

/// Reusable scratch state for the [`crate::NeedlemanWunsch`] and
/// [`crate::SmithWaterman`] alignment kernels.
///
/// A workspace holds up to three `Vec<i32>` buffers, one per DP matrix
/// (`M`, `X`, `Y`) required by the Gotoh 1982 three-matrix formulation.
/// Score-only kernels use only the first buffer; edit-script backtrace
/// uses all three when the scheme is affine and only the first when linear.
#[derive(Debug, Default, Clone)]
pub struct AlignmentWorkspace {
    /// Primary buffer; the `M` (match) matrix in Full mode, and the sole
    /// scratch buffer in Score mode.
    pub(crate) m: Vec<i32>,
    /// The `X` matrix — "gap in b" — for affine-gap Full mode.
    pub(crate) x: Vec<i32>,
    /// The `Y` matrix — "gap in a" — for affine-gap Full mode.
    pub(crate) y: Vec<i32>,
}

impl AlignmentWorkspace {
    /// Create an empty workspace. No allocation until the first use.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            m: Vec::new(),
            x: Vec::new(),
            y: Vec::new(),
        }
    }

    /// Create a workspace with the given per-matrix capacity, pre-zeroed.
    ///
    /// `vec![0; n]` compiles to a single `alloc_zeroed`, so this is a
    /// single-syscall preallocation.
    #[must_use]
    pub fn with_capacity(cells: usize) -> Self {
        Self {
            m: alloc::vec![0; cells],
            x: alloc::vec![0; cells],
            y: alloc::vec![0; cells],
        }
    }

    /// Return a mutable slice of the primary buffer, resizing if needed.
    ///
    /// Used by score-only kernels that only need one buffer.
    pub(crate) fn score_buffer(&mut self, required: usize) -> &mut [i32] {
        if self.m.len() < required {
            self.m.resize(required, 0);
        }
        &mut self.m[..required]
    }

    /// Return a mutable slice of the primary buffer, resizing to exactly
    /// `cells`.
    ///
    /// Used by the linear-gap Full-mode DP (edit-script reconstruction that
    /// only needs one matrix).
    pub(crate) fn m_matrix(&mut self, cells: usize) -> &mut [i32] {
        if self.m.len() < cells {
            self.m.resize(cells, 0);
        }
        &mut self.m[..cells]
    }

    /// Return mutable slices of all three matrices, resizing each to
    /// exactly `cells`.
    ///
    /// Used by the affine-gap Full-mode DP.
    pub(crate) fn full_matrices(&mut self, cells: usize) -> (&mut [i32], &mut [i32], &mut [i32]) {
        if self.m.len() < cells {
            self.m.resize(cells, 0);
        }
        if self.x.len() < cells {
            self.x.resize(cells, 0);
        }
        if self.y.len() < cells {
            self.y.resize(cells, 0);
        }
        (
            &mut self.m[..cells],
            &mut self.x[..cells],
            &mut self.y[..cells],
        )
    }
}

impl Workspace for AlignmentWorkspace {
    fn ensure_capacity(&mut self, required: usize) {
        if self.m.len() < required {
            self.m.resize(required, 0);
        }
        if self.x.len() < required {
            self.x.resize(required, 0);
        }
        if self.y.len() < required {
            self.y.resize(required, 0);
        }
    }

    fn capacity(&self) -> usize {
        // The advertised capacity is the smallest per-matrix capacity — the
        // largest `required` an `ensure_capacity` call could accept without
        // any resize.
        self.m.len().min(self.x.len()).min(self.y.len())
    }

    fn shrink(&mut self) {
        self.m.shrink_to_fit();
        self.x.shrink_to_fit();
        self.y.shrink_to_fit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_empty() {
        let ws = AlignmentWorkspace::new();
        assert_eq!(ws.capacity(), 0);
    }

    #[test]
    fn with_capacity_preallocates_all_three_matrices() {
        let ws = AlignmentWorkspace::with_capacity(16);
        assert_eq!(ws.capacity(), 16);
    }

    #[test]
    fn ensure_capacity_grows_but_does_not_shrink() {
        let mut ws = AlignmentWorkspace::new();
        ws.ensure_capacity(8);
        assert!(ws.capacity() >= 8);
        ws.ensure_capacity(4);
        assert!(ws.capacity() >= 8);
    }

    #[test]
    fn score_buffer_returns_requested_length() {
        let mut ws = AlignmentWorkspace::new();
        let buf = ws.score_buffer(5);
        assert_eq!(buf.len(), 5);
        buf[0] = 42;
        // Second call still returns 5 cells, memory reused.
        let buf2 = ws.score_buffer(3);
        assert_eq!(buf2.len(), 3);
        assert_eq!(buf2[0], 42);
    }

    #[test]
    fn full_matrices_returns_three_slices_of_requested_length() {
        let mut ws = AlignmentWorkspace::new();
        let (m, x, y) = ws.full_matrices(7);
        assert_eq!(m.len(), 7);
        assert_eq!(x.len(), 7);
        assert_eq!(y.len(), 7);
    }

    #[test]
    fn shrink_releases_capacity() {
        let mut ws = AlignmentWorkspace::with_capacity(1024);
        assert!(ws.capacity() >= 1024);
        ws.m.clear();
        ws.x.clear();
        ws.y.clear();
        ws.shrink();
        assert_eq!(ws.capacity(), 0);
    }
}
