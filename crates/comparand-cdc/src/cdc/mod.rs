//! Content-defined chunking algorithms.
//!
//! A CDC algorithm consumes a byte stream and emits boundary offsets whose
//! positions depend on the *content* of the stream rather than on absolute
//! byte counts. The property that motivates CDC — insensitivity to
//! shift — is what makes deduplication survive an insertion or deletion:
//! only the affected chunks change; the rest are re-identified by
//! content-hash and shared with the pre-edit stream.
//!
//! # This crate
//!
//! Version 0.1 ships one CDC algorithm, [`FastCdc`], built on top of the
//! [`GearHash`] primitive. The `RabinCdc` sibling is reserved in the
//! algorithm-family enum ([`AlgorithmFamily::RabinCdc`]) but not yet
//! implemented — it is superseded in practice by `FastCDC` on modern CPUs
//! and deferred to a future release.
//!
//! [`GearHash`]: crate::fingerprint::GearHash
//! [`AlgorithmFamily::RabinCdc`]: comparand_core::AlgorithmFamily::RabinCdc
//!
//! # References
//!
//! * Muthitacharoen, A., Chen, B., & Mazières, D. (2001). "A low-bandwidth
//!   network file system." *Proceedings of the eighteenth ACM symposium on
//!   Operating systems principles (SOSP '01)*, 174-187.
//!   <https://doi.org/10.1145/502034.502052> — the LBFS paper that
//!   popularized content-defined chunking as a deduplication primitive.

pub mod fastcdc;

pub use fastcdc::{FastCdc, FastCdcConfig, FastCdcIter, FastCdcStream};

/// A chunk boundary emitted by a CDC algorithm.
///
/// Boundaries are half-open at the end: a boundary with `offset = 10` and
/// `size = 4` describes the chunk covering bytes `[6, 10)`, and the next
/// chunk begins at byte 10.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChunkBoundary {
    /// Byte offset of the boundary — the end of the chunk that ended
    /// here (exclusive), and equivalently the start of the next chunk.
    pub offset: usize,
    /// Size of the chunk that ended at this boundary in bytes.
    pub size: usize,
}

impl ChunkBoundary {
    /// Constructs a chunk boundary. The chunk covers
    /// `[offset - size, offset)`.
    #[inline]
    #[must_use]
    pub const fn new(offset: usize, size: usize) -> Self {
        Self { offset, size }
    }

    /// Returns the start offset of the chunk (inclusive).
    #[inline]
    #[must_use]
    pub const fn start(&self) -> usize {
        // `offset` is always `>= size` by construction, so this cannot
        // wrap. The static `const` context prevents adding `saturating_sub`
        // ceremony that would only obscure the invariant.
        self.offset - self.size
    }

    /// Returns the end offset of the chunk (exclusive). Alias for
    /// [`offset`](Self::offset).
    #[inline]
    #[must_use]
    pub const fn end(&self) -> usize {
        self.offset
    }
}
