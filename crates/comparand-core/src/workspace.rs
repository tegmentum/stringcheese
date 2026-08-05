//! Reusable workspaces for allocation-free repeated comparisons.
//!
//! Many Comparand algorithms allocate one or more scratch buffers during a
//! single comparison. For workloads that perform many comparisons against
//! a fixed query — record linkage, deduplication, top-k queries — repeatedly
//! allocating and freeing those buffers becomes a large fraction of total
//! runtime.
//!
//! A [`Workspace`] is a caller-owned handle to those scratch buffers.
//! Passing the same workspace to every call in a batch reuses the same
//! allocation, growing it only when a larger comparison demands more space.
//!
//! # Design
//!
//! The trait exposes only the common capacity-management operations so that
//! generic infrastructure (thread-local pools, batch runners, arena
//! providers) can manage workspaces without knowing their internal layout.
//! Concrete workspaces — a Levenshtein rolling-row workspace, a
//! Needleman–Wunsch full-matrix workspace — live alongside the algorithms
//! that use them.

/// A caller-owned scratch buffer whose capacity can be grown to at least a
/// requested size.
///
/// The unit of `capacity` is workspace-specific — it may be "cells", "rows",
/// or "bytes". Callers pass a `Workspace` back into successive calls of the
/// same algorithm; the workspace's implementation is responsible for its
/// internal layout.
pub trait Workspace {
    /// Ensures the workspace can hold at least `required` units of its
    /// internal cell type without further allocation.
    ///
    /// If the workspace is already large enough, this is a no-op.
    fn ensure_capacity(&mut self, required: usize);

    /// The current capacity in the workspace's native unit.
    ///
    /// Intended for diagnostic and reporting use. Callers should not rely on
    /// this value being equal to the last argument passed to
    /// [`ensure_capacity`], as implementations may over-allocate to reduce
    /// future reallocation.
    ///
    /// [`ensure_capacity`]: Workspace::ensure_capacity
    fn capacity(&self) -> usize;

    /// Releases any memory the workspace holds beyond the base cost.
    ///
    /// Implementations should do their best to return heap memory to the
    /// allocator; the default is a no-op for implementations that hold no
    /// heap memory.
    #[inline]
    fn shrink(&mut self) {}
}
