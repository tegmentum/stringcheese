//! Gear hash — the rolling hash underlying `FastCDC`.
//!
//! Gear is a byte-indexed rolling hash: a fixed table `G[0..256]` of
//! 64-bit values, and the per-byte update
//!
//! ```text
//! hash = (hash << 1).wrapping_add(G[byte])
//! ```
//!
//! Because the shift is one bit per byte, a byte's contribution decays
//! naturally after at most 64 rolls without any explicit removal — the
//! defining virtue that lets Gear be `O(1)` per byte with a single load,
//! shift, and add, and no per-instance window buffer.
//!
//! # The table
//!
//! The `FastCDC` paper (Xia et al. 2016) prescribes a table of pseudo-random
//! 64-bit values, without pinning any single canonical set. This
//! implementation generates its table deterministically at compile time
//! from a fixed `SplitMix64` seed
//! `0xC0FFEE_C0FFEE_C0FFEE_C0FFEE_u64.wrapping_mul(1)` — encoded in the
//! variant slug as `"splitmix64-seed-coffee"` so a golden case tied to
//! this table cannot silently be re-run against a differently-seeded
//! implementation.
//!
//! The choice of *deterministic* generation matters: the design commits
//! to identical hash output across native and Wasm targets and across
//! debug and release builds. An ASLR-influenced or wall-clock-seeded
//! table would violate that invariant.
//!
//! # `no_std`
//!
//! Gear does not need a per-instance window buffer, so it is available in
//! every feature configuration — including the pure no-alloc build.
//!
//! # References
//!
//! * Xia, W., Jiang, H., Feng, D., Douglis, F., Shilane, P., Hua, Y., Fu,
//!   M., Zhang, Y., & Zhou, Y. (2016). "`FastCDC`: a fast and efficient
//!   content-defined chunking approach for data deduplication."
//!   *2016 USENIX Annual Technical Conference (USENIX ATC 16)*, 101-114.
//!   <https://www.usenix.org/conference/atc16/technical-sessions/presentation/xia>
//!   — introduces the Gear rolling-hash construction this module
//!   implements as the primitive underlying `FastCDC`.

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use super::RollingHash;

/// The 256-entry Gear table.
///
/// Generated at compile time via `SplitMix64` seeded with a fixed constant.
/// See the [module-level documentation][crate::fingerprint::gear] for the
/// seed and the reason it is exposed as part of the variant slug rather
/// than baked in silently.
pub const GEAR_TABLE: [u64; 256] = build_gear_table();

/// Gear hash from `FastCDC` (Xia et al. 2016).
///
/// Maintains a single `u64` state; each byte fed shifts the state left by
/// one bit and XORs (via wrapping add) the table entry indexed by the
/// byte. Old bytes fall out of the state naturally as they shift past
/// bit 63.
///
/// See the [module-level documentation][crate::fingerprint::gear] for the
/// table's provenance.
#[derive(Copy, Clone, Debug)]
pub struct GearHash {
    /// The current hash state. The full 64 bits carry signal, but
    /// [`FastCdc`][crate::cdc::fastcdc::FastCdc] only tests specific
    /// mask bits.
    state: u64,
    /// Nominal window size. Gear's effective window is 64 bytes (one bit
    /// per byte), so this field is retained for parity with the
    /// [`RollingHash`] trait rather than because it changes the update
    /// rule. It is used only to advertise the configured window and to
    /// support the `reset` contract.
    window: usize,
}

impl GearHash {
    /// The algorithm descriptor for this variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::GearHash,
        variant: VariantId("splitmix64-seed-coffee"),
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

    /// Constructs a Gear hash with the given nominal window size.
    ///
    /// Gear's effective window is fixed at 64 bytes; the `window`
    /// parameter is stored for parity with the [`RollingHash`] trait but
    /// does not change the update rule.
    #[inline]
    #[must_use]
    pub const fn with_window(window: usize) -> Self {
        Self { state: 0, window }
    }

    /// Returns the raw state.
    ///
    /// Exposed so [`FastCdc`][crate::cdc::fastcdc::FastCdc] can test bits
    /// against its mask without going through the abstract [`digest`]
    /// return type.
    ///
    /// [`digest`]: RollingHash::digest
    #[inline]
    #[must_use]
    pub const fn state(&self) -> u64 {
        self.state
    }

    /// Returns the nominal window size configured at construction.
    ///
    /// Gear's effective window is fixed at 64 bytes regardless of this
    /// value — the field is retained for parity with the
    /// [`RollingHash::new`] trait method rather than because it changes
    /// the update rule.
    #[inline]
    #[must_use]
    pub const fn nominal_window(&self) -> usize {
        self.window
    }
}

impl RollingHash for GearHash {
    type Output = u64;

    #[inline]
    fn new(window: usize) -> Self {
        Self::with_window(window)
    }

    #[inline]
    fn roll(&mut self, byte: u8) {
        // Wrapping shift-and-add: overflow is expected and desired — it
        // *is* the mechanism by which old bytes fall out of the state.
        self.state = (self.state << 1).wrapping_add(GEAR_TABLE[byte as usize]);
    }

    #[inline]
    fn digest(&self) -> Self::Output {
        self.state
    }

    #[inline]
    fn reset(&mut self) {
        self.state = 0;
        // `window` is preserved across `reset`; only the state is
        // cleared.
    }
}

/// A single `SplitMix64` step.
///
/// The classical `SplitMix64` output function due to Steele, Lea, and Flood
/// (2014), used verbatim as documented in the reference. Constants are
/// intentionally kept as-published so a reader can cross-check against
/// the reference.
#[allow(
    clippy::unreadable_literal,
    reason = "these are the published `SplitMix64` mixing constants — separating with underscores obscures the fact that they match the reference"
)]
const fn splitmix64_next(state: u64) -> (u64, u64) {
    let next_state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = next_state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D49BB133111EB1);
    z ^= z >> 31;
    (next_state, z)
}

/// The fixed `SplitMix64` seed that produces `GEAR_TABLE`.
///
/// `0xC0FFEE_C0FFEE_C0FFEE`-ish, encoded so the variant slug
/// `"splitmix64-seed-coffee"` names it mnemonically.
const GEAR_SEED: u64 = 0x00C0_FFEE_C0FF_EE00;

/// Builds `GEAR_TABLE` at compile time.
const fn build_gear_table() -> [u64; 256] {
    let mut table = [0u64; 256];
    let mut state = GEAR_SEED;
    let mut i = 0;
    while i < 256 {
        let (next_state, z) = splitmix64_next(state);
        state = next_state;
        table[i] = z;
        i += 1;
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_pins_family_variant_and_year() {
        let d = GearHash::descriptor();
        assert_eq!(d.family, AlgorithmFamily::GearHash);
        assert_eq!(d.variant, VariantId("splitmix64-seed-coffee"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 2016, .. }
        ));
    }

    #[test]
    fn empty_hash_is_zero() {
        let h = GearHash::new(64);
        assert_eq!(h.digest(), 0);
    }

    #[test]
    fn single_byte_matches_table_entry() {
        // Feeding a single byte with the state at zero yields
        // (0 << 1) + GEAR_TABLE[byte] = GEAR_TABLE[byte].
        for b in [0u8, 1, 42, 0xFF] {
            let mut h = GearHash::new(64);
            h.roll(b);
            assert_eq!(h.digest(), GEAR_TABLE[b as usize]);
        }
    }

    #[test]
    fn reset_returns_to_zero_state() {
        let mut h = GearHash::new(64);
        for &b in b"content for the hash to churn on" {
            h.roll(b);
        }
        assert_ne!(h.digest(), 0);
        h.reset();
        assert_eq!(h.digest(), 0);
    }

    #[test]
    fn table_is_diverse() {
        // A trivial sanity check that the `SplitMix64` seeding did not
        // produce a degenerate all-zero or all-equal table.
        assert_ne!(GEAR_TABLE[0], GEAR_TABLE[1]);
        assert_ne!(GEAR_TABLE[0], 0);
        // Assert every entry is unique — `SplitMix64` does not guarantee
        // collision-freedom, but for 256 draws from a 64-bit space this
        // is overwhelmingly likely and is easy to check.
        for (i, &a) in GEAR_TABLE.iter().enumerate() {
            for &b in &GEAR_TABLE[i + 1..] {
                assert_ne!(a, b, "table collision detected at index {i}");
            }
        }
    }

    #[test]
    fn deterministic_across_calls() {
        // Two GearHash instances fed the same bytes must produce
        // identical digests, always. This is the property that keeps
        // `FastCDC` output reproducible.
        let mut h1 = GearHash::new(64);
        let mut h2 = GearHash::new(64);
        for &b in b"reproducibility matters here" {
            h1.roll(b);
            h2.roll(b);
        }
        assert_eq!(h1.digest(), h2.digest());
    }
}
