//! `FastCDC` content-defined chunking.
//!
//! `FastCDC` (Xia et al., 2016) selects chunk boundaries by running the
//! [`GearHash`] rolling hash across the input and cutting at positions
//! where the hash matches a mask. Two masks are used — strict and loose —
//! so short chunks are pushed toward the target average and long chunks
//! are aggressively cut before hitting the maximum. This is the
//! *normalized-level-2* (NLC = 2) variant from the paper; the mask
//! definitions below encode that choice.
//!
//! # Semantics
//!
//! For every chunk in the interior of the input,
//! `min_size <= size <= max_size`. The **final** chunk may be shorter
//! than `min_size` if the input runs out before enough bytes have
//! accumulated to reach `min_size` — this is unavoidable without
//! padding, and the property tests treat the last chunk as a special
//! case.
//!
//! The byte that triggers a cut is included as the **last** byte of the
//! current chunk; the next chunk starts one byte later. This is one of
//! two conventional choices; the [`ChunkBoundary::offset`] field
//! together with the file-length invariant guarantees the choice is
//! unambiguous even when reading the returned boundaries in isolation.
//!
//! # Streaming
//!
//! [`FastCdcStream`] is the underlying state machine — a byte-at-a-time
//! feed that emits boundaries as they arise. It requires no allocation
//! and is available in every feature configuration.
//!
//! [`FastCdcIter`] wraps a stream around a contiguous `&[u8]` and drives
//! it to yield [`ChunkBoundary`] values. It is the API most callers will
//! reach for.
//!
//! The streaming and contiguous forms are equivalent by construction:
//! `FastCdcStream` sees the same byte sequence regardless of how the
//! input is split at the byte level, so a caller who feeds the input in
//! ten pieces gets the same boundary set as a caller who feeds it in
//! one. This is asserted as a property test.
//!
//! # `alloc` gate
//!
//! The state machine itself is `no_std`- and no-alloc-compatible. The
//! `Vec`-returning helper [`FastCdc::chunk_boundaries_vec`] is gated on
//! the `alloc` feature.
//!
//! [`GearHash`]: crate::fingerprint::GearHash
//! [`ChunkBoundary`]: crate::cdc::ChunkBoundary
//!
//! # References
//!
//! * Xia, W., Jiang, H., Feng, D., Douglis, F., Shilane, P., Hua, Y., Fu,
//!   M., Zhang, Y., & Zhou, Y. (2016). "`FastCDC`: a fast and efficient
//!   content-defined chunking approach for data deduplication."
//!   *2016 USENIX Annual Technical Conference (USENIX ATC 16)*, 101-114.
//!   <https://www.usenix.org/conference/atc16/technical-sessions/presentation/xia>
//! * Muthitacharoen, A., Chen, B., & Mazières, D. (2001). "A low-bandwidth
//!   network file system." *Proceedings of the eighteenth ACM symposium on
//!   Operating systems principles (SOSP '01)*, 174-187.
//!   <https://doi.org/10.1145/502034.502052> — background on the CDC
//!   deduplication model this algorithm targets.

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::cdc::ChunkBoundary;
use crate::fingerprint::gear::GEAR_TABLE;

/// Configuration for a `FastCDC` chunker.
///
/// The three size fields set the acceptable chunk-length range; the two
/// mask fields set the "cut here" trigger. See the module docs for the
/// scheme.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FastCdcConfig {
    /// Minimum chunk size in bytes.
    ///
    /// Below this size the algorithm skips both the hash update and the
    /// mask check — chunks cannot end earlier. The interior-chunk
    /// invariant `size >= min_size` follows from this.
    pub min_size: usize,
    /// Target average chunk size in bytes.
    ///
    /// The boundary between the `mask_small` region (chunks below the
    /// average) and the `mask_large` region (chunks above).
    pub avg_size: usize,
    /// Maximum chunk size in bytes.
    ///
    /// A chunk that reaches this size is force-cut regardless of the
    /// hash. The interior-chunk invariant `size <= max_size` follows
    /// from this.
    pub max_size: usize,
    /// Strict cut mask, applied while the current chunk is at or below
    /// the target average size.
    ///
    /// Higher [`u64::count_ones`] on this mask reduces the probability
    /// of a cut per byte and pushes chunk sizes toward the average from
    /// below.
    pub mask_small: u64,
    /// Loose cut mask, applied while the current chunk is above the
    /// target average size.
    ///
    /// Lower [`u64::count_ones`] on this mask increases the probability
    /// of a cut per byte and pulls chunk sizes back toward the average
    /// from above, well before the max is reached.
    pub mask_large: u64,
}

impl FastCdcConfig {
    /// Returns a paper-derived 8 KB configuration.
    ///
    /// * `min_size = 2 KB`, `avg_size = 8 KB`, `max_size = 64 KB`.
    /// * `mask_small` has 15 bits set; `mask_large` has 11 bits. Both
    ///   are taken verbatim from the reference `FastCDC` implementation
    ///   accompanying the paper.
    #[must_use]
    pub const fn default_8k() -> Self {
        Self {
            min_size: 2 * 1024,
            avg_size: 8 * 1024,
            max_size: 64 * 1024,
            // The two masks below are the "normalized level 2" (NLC=2)
            // masks for `avg_size = 8 KB` published in Xia et al. 2016.
            mask_small: 0x0003_5907_0353_0000,
            mask_large: 0x0000_D900_0353_0000,
        }
    }

    /// Returns a paper-consistent 16 KB configuration.
    ///
    /// * `min_size = 4 KB`, `avg_size = 16 KB`, `max_size = 128 KB`.
    /// * `mask_small` has 16 bits set; `mask_large` has 12 bits.
    ///   These extend the 8 KB masks by one extra bit each, preserving
    ///   the `NLC = 2` normalization on a doubled target average.
    #[must_use]
    pub const fn default_16k() -> Self {
        Self {
            min_size: 4 * 1024,
            avg_size: 16 * 1024,
            max_size: 128 * 1024,
            mask_small: 0x0007_5907_0353_0000,
            mask_large: 0x0001_D900_0353_0000,
        }
    }

    /// Validates the size relationships between the three size fields.
    ///
    /// Returns `true` when `0 < min_size <= avg_size <= max_size`. The
    /// constructors on this type always produce valid configs; the
    /// method is exposed for callers who construct one by hand.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.min_size > 0 && self.min_size <= self.avg_size && self.avg_size <= self.max_size
    }
}

/// The `FastCDC` content-defined chunker.
///
/// See the [module-level documentation][crate::cdc::fastcdc] for the
/// algorithm and the streaming/iterator options.
#[derive(Copy, Clone, Debug)]
pub struct FastCdc {
    /// The size and mask configuration in force.
    config: FastCdcConfig,
}

impl FastCdc {
    /// The algorithm descriptor for this variant.
    ///
    /// The variant slug `"normalized-lc-2"` pins the NLC-2 mask scheme
    /// from the paper. A future NLC-1 or NLC-3 sibling would carry a
    /// different slug so a golden case could not silently be re-run
    /// against it.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::FastCdc,
        variant: VariantId("normalized-lc-2"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "FastCDC: a fast and efficient content-defined chunking approach for data deduplication",
            authors: "W. Xia et al.",
            year: 2016,
        },
    };

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Constructs a chunker with the given configuration.
    ///
    /// # Panics
    ///
    /// Panics if the config's size fields are not in the order
    /// `0 < min_size <= avg_size <= max_size` — see
    /// [`FastCdcConfig::is_valid`].
    #[must_use]
    pub const fn new(config: FastCdcConfig) -> Self {
        assert!(
            config.is_valid(),
            "FastCdcConfig sizes must satisfy 0 < min_size <= avg_size <= max_size",
        );
        Self { config }
    }

    /// Returns the configuration in force.
    #[inline]
    #[must_use]
    pub const fn config(&self) -> &FastCdcConfig {
        &self.config
    }

    /// Returns an iterator over the chunk boundaries in `input`.
    ///
    /// The iterator is lazy: it holds only the input reference, a
    /// position cursor, and the streaming state. No boundary vector is
    /// built up front.
    ///
    /// The returned boundaries partition `input` — the last boundary's
    /// [`ChunkBoundary::offset`] always equals `input.len()`.
    #[must_use]
    pub fn chunk_boundaries<'a>(&self, input: &'a [u8]) -> FastCdcIter<'a> {
        FastCdcIter {
            stream: FastCdcStream::new(self.config),
            input,
            pos: 0,
            finished: false,
        }
    }

    /// Materialises every chunk boundary into a `Vec`.
    ///
    /// Convenience over [`chunk_boundaries`] for callers that want the
    /// full boundary list up front. Available under the `alloc` feature.
    ///
    /// [`chunk_boundaries`]: FastCdc::chunk_boundaries
    #[cfg(feature = "alloc")]
    #[must_use]
    pub fn chunk_boundaries_vec(&self, input: &[u8]) -> alloc::vec::Vec<ChunkBoundary> {
        self.chunk_boundaries(input).collect()
    }
}

/// The `FastCDC` streaming state machine.
///
/// Consumes bytes one at a time; emits boundaries as they arise. The
/// state machine has no dependence on the input's total length, so
/// splitting the input at any byte-level boundary and feeding the pieces
/// sequentially produces the identical boundary sequence to a
/// contiguous feed. This is asserted as a property test.
#[derive(Copy, Clone, Debug)]
pub struct FastCdcStream {
    /// The size and mask configuration in force.
    config: FastCdcConfig,
    /// The current rolling hash state.
    ///
    /// Reset to zero every time a cut is emitted.
    hash: u64,
    /// Bytes accumulated in the current chunk so far, including any
    /// bytes still inside the `min_size` skip window.
    bytes_in_chunk: usize,
    /// Total bytes fed since construction. Used to compute the
    /// [`ChunkBoundary::offset`] of each emitted boundary.
    total_bytes: usize,
}

impl FastCdcStream {
    /// Constructs a streaming chunker with the given configuration.
    ///
    /// # Panics
    ///
    /// Panics if the config's size fields are not in the order
    /// `0 < min_size <= avg_size <= max_size`.
    #[must_use]
    pub const fn new(config: FastCdcConfig) -> Self {
        assert!(
            config.is_valid(),
            "FastCdcConfig sizes must satisfy 0 < min_size <= avg_size <= max_size",
        );
        Self {
            config,
            hash: 0,
            bytes_in_chunk: 0,
            total_bytes: 0,
        }
    }

    /// Feeds a single byte into the stream, possibly emitting a
    /// [`ChunkBoundary`].
    ///
    /// Bytes below `min_size` within the current chunk are skipped
    /// entirely — neither hashed nor checked against the cut mask. From
    /// `min_size` onward the byte is hashed and the mask is tested.
    /// When the chunk reaches `max_size` a cut is forced regardless of
    /// the hash.
    pub fn feed(&mut self, byte: u8) -> Option<ChunkBoundary> {
        self.bytes_in_chunk += 1;
        self.total_bytes += 1;

        // The first `min_size - 1` bytes of a chunk are skipped entirely.
        // The `min_size`-th byte is the first to be hashed and checked;
        // this makes the smallest possible chunk exactly `min_size`
        // bytes long.
        if self.bytes_in_chunk < self.config.min_size {
            return None;
        }

        // Roll the hash forward by one bit and mix in this byte's table
        // entry. The wrapping-add is intentional — the shift is how old
        // bytes fall out of the hash's contribution.
        self.hash = (self.hash << 1).wrapping_add(GEAR_TABLE[byte as usize]);

        // Force a cut at the maximum. This guarantees the interior
        // `size <= max_size` invariant even on inputs that never
        // organically satisfy either mask.
        if self.bytes_in_chunk >= self.config.max_size {
            return Some(self.emit_cut());
        }

        // Below the target average: apply the strict mask, so we cut
        // less often than at random and let small chunks grow toward
        // the target.
        //
        // Above the target average: apply the loose mask, so we cut
        // more often than at random and prevent chunks from drifting
        // toward the max.
        let mask = if self.bytes_in_chunk <= self.config.avg_size {
            self.config.mask_small
        } else {
            self.config.mask_large
        };

        if (self.hash & mask) == 0 {
            Some(self.emit_cut())
        } else {
            None
        }
    }

    /// Feeds a slice of bytes, invoking the callback for every emitted
    /// boundary in order.
    ///
    /// A closure-based interface avoids requiring `alloc` here — the
    /// caller decides where to store the boundaries. Under `alloc`,
    /// callers can pass `|b| boundaries.push(b)` for the natural
    /// `Vec`-collecting form.
    pub fn feed_slice(&mut self, bytes: &[u8], mut on_boundary: impl FnMut(ChunkBoundary)) {
        for &byte in bytes {
            if let Some(cb) = self.feed(byte) {
                on_boundary(cb);
            }
        }
    }

    /// Flushes any bytes still accumulated in the current chunk as a
    /// final boundary.
    ///
    /// The final chunk may be shorter than `min_size` if the input ran
    /// out before enough bytes accumulated — this is unavoidable
    /// without padding. Returns `None` when no bytes are pending, e.g.
    /// on an empty input or immediately after a cut aligned with the
    /// end of the stream.
    pub fn finish(&mut self) -> Option<ChunkBoundary> {
        if self.bytes_in_chunk > 0 {
            Some(self.emit_cut())
        } else {
            None
        }
    }

    /// Total bytes fed since construction.
    #[inline]
    #[must_use]
    pub const fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Builds a boundary that ends at the current position with the
    /// current chunk size, and resets the per-chunk state.
    fn emit_cut(&mut self) -> ChunkBoundary {
        let cb = ChunkBoundary {
            offset: self.total_bytes,
            size: self.bytes_in_chunk,
        };
        self.hash = 0;
        self.bytes_in_chunk = 0;
        cb
    }
}

/// Iterator over the chunk boundaries in a contiguous input slice.
///
/// Returned by [`FastCdc::chunk_boundaries`]. Iterates lazily: each
/// [`next`][Iterator::next] call feeds bytes into the underlying
/// [`FastCdcStream`] until a boundary is emitted, then returns that
/// boundary. When the input is exhausted, the iterator drains the
/// stream's pending bytes as one final boundary if any remain, and
/// yields `None` on subsequent calls.
#[derive(Clone, Debug)]
pub struct FastCdcIter<'a> {
    /// The underlying streaming state machine.
    stream: FastCdcStream,
    /// The full input slice being iterated over.
    input: &'a [u8],
    /// The next byte to be fed to the stream.
    pos: usize,
    /// Whether the terminal `finish()` call has already emitted a
    /// boundary (or confirmed none was pending). Iteration returns
    /// `None` for every subsequent call once this is `true`.
    finished: bool,
}

impl Iterator for FastCdcIter<'_> {
    type Item = ChunkBoundary;

    fn next(&mut self) -> Option<ChunkBoundary> {
        while self.pos < self.input.len() {
            let byte = self.input[self.pos];
            self.pos += 1;
            if let Some(cb) = self.stream.feed(byte) {
                return Some(cb);
            }
        }
        if !self.finished {
            self.finished = true;
            return self.stream.finish();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_pins_family_variant_and_year() {
        let d = FastCdc::descriptor();
        assert_eq!(d.family, AlgorithmFamily::FastCdc);
        assert_eq!(d.variant, VariantId("normalized-lc-2"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 2016, .. }
        ));
    }

    #[test]
    fn default_configs_are_valid() {
        assert!(FastCdcConfig::default_8k().is_valid());
        assert!(FastCdcConfig::default_16k().is_valid());
    }

    #[test]
    fn default_8k_masks_have_paper_popcounts() {
        // The NLC=2 masks for an 8 KB average have 15 and 11 bits set.
        // This assertion documents that the hard-coded values were not
        // altered by accident.
        let c = FastCdcConfig::default_8k();
        assert_eq!(c.mask_small.count_ones(), 15);
        assert_eq!(c.mask_large.count_ones(), 11);
    }

    #[test]
    fn default_16k_masks_have_paper_popcounts() {
        let c = FastCdcConfig::default_16k();
        assert_eq!(c.mask_small.count_ones(), 16);
        assert_eq!(c.mask_large.count_ones(), 12);
    }

    #[test]
    fn empty_input_yields_no_chunks() {
        let cdc = FastCdc::new(FastCdcConfig::default_8k());
        let boundaries: alloc::vec::Vec<ChunkBoundary> = cdc.chunk_boundaries(&[]).collect();
        assert!(boundaries.is_empty());
    }

    #[test]
    fn input_below_min_size_yields_one_final_chunk() {
        let cdc = FastCdc::new(FastCdcConfig::default_8k());
        let input = alloc::vec![0u8; 500];
        let boundaries: alloc::vec::Vec<ChunkBoundary> = cdc.chunk_boundaries(&input).collect();
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].size, 500);
        assert_eq!(boundaries[0].offset, 500);
    }
}
