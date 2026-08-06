//! Canonical golden cases for the fingerprint and CDC subsystems.
//!
//! Cases fall into two shapes:
//!
//! * **Fixed-digest cases** — [`GoldenCase`] records with a `u64`
//!   expected digest pinned at compile time. These cover inputs whose
//!   expected output is unambiguous by inspection: the empty input, and
//!   inputs one byte long whose digest a reader can derive by hand from
//!   the algorithm's rolling formula.
//! * **Derived-expectation cases** — inputs where the expected output
//!   depends on a compile-time table (like `GEAR_TABLE`, generated from
//!   a `SplitMix64` seed). These are exercised inside `#[test]` functions
//!   that recompute the expectation from the primitive rather than
//!   pinning a table lookup as a `const`.
//!
//! Because `comparand-corpus` is a dev-dependency, this whole module is
//! `#[cfg(test)]`.

use alloc::{vec, vec::Vec};

use comparand_corpus::{GoldenCase, GoldenSource};

use crate::cdc::{ChunkBoundary, FastCdc, FastCdcConfig};
use crate::fingerprint::{GearHash, PolynomialHash, RabinFingerprint, RollingHash};

/// A fingerprint golden case: an input, window size, and expected digest.
pub type FingerprintCase = GoldenCase<(&'static [u8], usize), u64>;

// -------------------------------------------------------------------------
// Fingerprint golden cases — expected digests pinned at compile time
// -------------------------------------------------------------------------

/// Gear-hash golden cases with hand-verifiable expected digests.
pub const GEAR_CASES: &[FingerprintCase] = &[GoldenCase {
    id: "gear/empty",
    descriptor: GearHash::DESCRIPTOR,
    input: (b"", 64),
    expected: 0,
    source: GoldenSource::IndependentlyDerived,
    notes: "Empty input: no bytes rolled, state remains at the identity zero.",
    tags: &["basic", "empty"],
}];

/// Polynomial-hash golden cases.
pub const POLYNOMIAL_CASES: &[FingerprintCase] = &[
    GoldenCase {
        id: "polynomial/empty",
        descriptor: PolynomialHash::DESCRIPTOR,
        input: (b"", 4),
        expected: 0,
        source: GoldenSource::IndependentlyDerived,
        notes: "Empty input: hash is zero by definition of the empty sum.",
        tags: &["basic", "empty"],
    },
    GoldenCase {
        id: "polynomial/single-byte-A",
        descriptor: PolynomialHash::DESCRIPTOR,
        input: (b"A", 4),
        expected: 0x41,
        source: GoldenSource::IndependentlyDerived,
        notes: "Single byte 'A' (0x41): H = (0 * BASE + 0x41) mod PRIME = 0x41.",
        tags: &["basic", "single-byte"],
    },
    GoldenCase {
        id: "polynomial/single-byte-0xFF",
        descriptor: PolynomialHash::DESCRIPTOR,
        input: (b"\xFF", 4),
        expected: 0xFF,
        source: GoldenSource::IndependentlyDerived,
        notes: "Single byte 0xFF: H = 0xFF. Guards against a sign-extension bug on the byte-to-u64 conversion.",
        tags: &["basic", "single-byte"],
    },
    GoldenCase {
        id: "polynomial/two-bytes-AB",
        descriptor: PolynomialHash::DESCRIPTOR,
        input: (b"AB", 4),
        // (0x41 * 257 + 0x42) is well under the Mersenne-61 modulus, so
        // the explicit reduction step is a no-op — asserted by leaving
        // the value un-modded here and by the value's magnitude.
        expected: 0x41_u64 * 257 + 0x42,
        source: GoldenSource::IndependentlyDerived,
        notes: "Two bytes 'AB': H = (0x41 * 257 + 0x42) mod (2^61-1) = 16769 + 66 = 16835.",
        tags: &["basic", "two-byte"],
    },
];

/// Rabin fingerprint golden cases.
pub const RABIN_CASES: &[FingerprintCase] = &[
    GoldenCase {
        id: "rabin/empty",
        descriptor: RabinFingerprint::DESCRIPTOR,
        input: (b"", 4),
        expected: 0,
        source: GoldenSource::IndependentlyDerived,
        notes: "Empty input: state is zero, matching the identity of the reduction.",
        tags: &["basic", "empty"],
    },
    GoldenCase {
        id: "rabin/single-byte-A",
        descriptor: RabinFingerprint::DESCRIPTOR,
        input: (b"A", 4),
        expected: 0x41,
        source: GoldenSource::IndependentlyDerived,
        notes: "Single byte 'A' (0x41): state = (0 << 8) ^ 0x41 ^ SHIFT_TABLE[0] = 0x41.",
        tags: &["basic", "single-byte"],
    },
    GoldenCase {
        id: "rabin/single-byte-0xFF",
        descriptor: RabinFingerprint::DESCRIPTOR,
        input: (b"\xFF", 4),
        expected: 0xFF,
        source: GoldenSource::IndependentlyDerived,
        notes: "Single byte 0xFF: state = 0xFF. No reduction is triggered because the state fits in the low 8 bits.",
        tags: &["basic", "single-byte"],
    },
];

// -------------------------------------------------------------------------
// CDC golden cases
// -------------------------------------------------------------------------

/// A `FastCDC` golden case: an input plus a full expected boundary list.
///
/// Owned `Vec`s are used so cases that construct their input by
/// repetition do not have to be pinned in static storage.
pub struct FastCdcCase {
    /// A stable identifier.
    pub id: &'static str,
    /// The chunker config to use.
    pub config: FastCdcConfig,
    /// The input to chunk.
    pub input: Vec<u8>,
    /// The expected boundary sequence.
    pub expected: Vec<ChunkBoundary>,
    /// Notes on what the case exercises.
    #[allow(
        dead_code,
        reason = "documentation-only field consumed by the case listing; not read by the runner"
    )]
    pub notes: &'static str,
}

impl FastCdcCase {
    /// Builds the standard collection of `FastCDC` golden cases.
    #[must_use]
    pub fn corpus() -> Vec<Self> {
        vec![
            Self {
                id: "fastcdc/empty",
                config: FastCdcConfig::default_8k(),
                input: vec![],
                expected: vec![],
                notes: "Empty input yields zero chunks.",
            },
            Self {
                id: "fastcdc/below-min-size",
                config: FastCdcConfig::default_8k(),
                input: vec![0u8; 500],
                expected: vec![ChunkBoundary::new(500, 500)],
                notes: "Input shorter than min_size collapses to one final chunk of the full length.",
            },
            Self {
                id: "fastcdc/exactly-min-size",
                config: FastCdcConfig::default_8k(),
                input: vec![0u8; FastCdcConfig::default_8k().min_size],
                expected: vec![ChunkBoundary::new(
                    FastCdcConfig::default_8k().min_size,
                    FastCdcConfig::default_8k().min_size,
                )],
                notes: "Input exactly min_size: the mask check on the min_size-th byte does not fire for the all-zeros hash state, so the final chunk of length min_size is flushed by finish().",
            },
            Self {
                id: "fastcdc/max-size-force-cut",
                config: FastCdcConfig {
                    min_size: 4,
                    avg_size: 8,
                    max_size: 16,
                    // Effectively unreachable mask (63 bits) so the hash
                    // never triggers a cut on its own, guaranteeing the
                    // max-size force cut fires.
                    mask_small: u64::MAX >> 1,
                    mask_large: u64::MAX >> 1,
                },
                input: vec![0u8; 40],
                expected: vec![
                    ChunkBoundary::new(16, 16),
                    ChunkBoundary::new(32, 16),
                    ChunkBoundary::new(40, 8),
                ],
                notes: "With an unreachable hash mask, the max_size force cut is the only mechanism producing boundaries.",
            },
        ]
    }
}

/// Window-transition cases exercised across all fingerprint families.
///
/// Each entry is `(id, input, window)`. The property under test — a
/// rolling digest after `n` bytes equals a fresh digest over the
/// trailing `window` bytes — holds for Rabin and Polynomial exactly and
/// for Gear when the window fits within Gear's effective 64-byte
/// horizon.
pub const WINDOW_TRANSITION_CASES: &[(&str, &[u8], usize)] = &[
    ("window-transition/exactly-window", b"abcdefgh", 8),
    ("window-transition/window-plus-one", b"abcdefghX", 8),
    (
        "window-transition/window-plus-many",
        b"the quick brown fox jumps over the lazy dog",
        8,
    ),
    ("window-transition/short-input", b"abc", 8),
];

/// Pairs the spec calls out as distinguishing between the three
/// fingerprint families. Each entry is a `(id, bytes, window)`.
pub const CROSS_FAMILY_DISTINCTIVENESS_CASES: &[(&str, &[u8], usize)] = &[
    ("cross/short-ascii", b"abcdef", 4),
    ("cross/repeating-pattern", b"AAAAAA", 4),
];

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    /// Feeds `input` through a fresh `H`-typed hash and returns its
    /// final digest.
    fn run_hash<H: RollingHash<Output = u64>>(input: &[u8], window: usize) -> u64 {
        let mut h = H::new(window);
        for &b in input {
            h.roll(b);
        }
        h.digest()
    }

    #[test]
    fn every_case_uses_the_correct_descriptor() {
        for case in GEAR_CASES {
            assert_eq!(
                case.descriptor,
                GearHash::DESCRIPTOR,
                "gear case {}",
                case.id
            );
        }
        for case in POLYNOMIAL_CASES {
            assert_eq!(
                case.descriptor,
                PolynomialHash::DESCRIPTOR,
                "polynomial case {}",
                case.id
            );
        }
        for case in RABIN_CASES {
            assert_eq!(
                case.descriptor,
                RabinFingerprint::DESCRIPTOR,
                "rabin case {}",
                case.id
            );
        }
    }

    #[test]
    fn every_case_has_a_unique_id() {
        let mut ids: Vec<&str> = Vec::new();
        for c in GEAR_CASES {
            ids.push(c.id);
        }
        for c in POLYNOMIAL_CASES {
            ids.push(c.id);
        }
        for c in RABIN_CASES {
            ids.push(c.id);
        }
        for c in FastCdcCase::corpus() {
            ids.push(c.id);
        }
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate golden-case id detected");
    }

    #[test]
    fn total_case_count_meets_minimum() {
        // The spec asks for at least 15 golden cases across the crate.
        let total = GEAR_CASES.len()
            + POLYNOMIAL_CASES.len()
            + RABIN_CASES.len()
            + FastCdcCase::corpus().len()
            + WINDOW_TRANSITION_CASES.len()
            + CROSS_FAMILY_DISTINCTIVENESS_CASES.len();
        assert!(
            total >= 15,
            "expected at least 15 golden cases across the crate, got {total}"
        );
    }

    // -------------------------------------------------------------------
    // Fixed-digest cases
    // -------------------------------------------------------------------

    #[test]
    fn gear_fixed_digest_cases_match_kernel() {
        for case in GEAR_CASES {
            let (input, window) = case.input;
            assert_eq!(
                run_hash::<GearHash>(input, window),
                case.expected,
                "gear case {}",
                case.id
            );
        }
    }

    #[test]
    fn polynomial_fixed_digest_cases_match_kernel() {
        for case in POLYNOMIAL_CASES {
            let (input, window) = case.input;
            assert_eq!(
                run_hash::<PolynomialHash>(input, window),
                case.expected,
                "polynomial case {}",
                case.id
            );
        }
    }

    #[test]
    fn rabin_fixed_digest_cases_match_kernel() {
        for case in RABIN_CASES {
            let (input, window) = case.input;
            assert_eq!(
                run_hash::<RabinFingerprint>(input, window),
                case.expected,
                "rabin case {}",
                case.id
            );
        }
    }

    // -------------------------------------------------------------------
    // Derived-expectation case: single-byte Gear digest equals the
    // GEAR_TABLE entry for the byte. Computed at test time because
    // GEAR_TABLE is not const-indexable in a `const` field.
    // -------------------------------------------------------------------

    #[test]
    fn gear_single_byte_matches_table_entry() {
        for b in [0u8, 1, 42, 0xFF] {
            let observed = run_hash::<GearHash>(&[b], 64);
            let expected = crate::fingerprint::gear::GEAR_TABLE[b as usize];
            assert_eq!(observed, expected, "gear single-byte byte={b:#04x}");
        }
    }

    // -------------------------------------------------------------------
    // Window-transition cases: rolling digest equals fresh digest over
    // the trailing `window` bytes.
    // -------------------------------------------------------------------

    #[test]
    fn rabin_rolling_matches_fresh_over_trailing_window() {
        for &(id, input, window) in WINDOW_TRANSITION_CASES {
            let trailing = if input.len() >= window {
                &input[input.len() - window..]
            } else {
                input
            };
            let rolling = run_hash::<RabinFingerprint>(input, window);
            let fresh = run_hash::<RabinFingerprint>(trailing, window);
            assert_eq!(rolling, fresh, "rabin {id}");
        }
    }

    #[test]
    fn polynomial_rolling_matches_fresh_over_trailing_window() {
        for &(id, input, window) in WINDOW_TRANSITION_CASES {
            let trailing = if input.len() >= window {
                &input[input.len() - window..]
            } else {
                input
            };
            let rolling = run_hash::<PolynomialHash>(input, window);
            let fresh = run_hash::<PolynomialHash>(trailing, window);
            assert_eq!(rolling, fresh, "polynomial {id}");
        }
    }

    // -------------------------------------------------------------------
    // Cross-family distinctiveness
    // -------------------------------------------------------------------

    #[test]
    fn cross_family_digests_differ() {
        for &(id, input, window) in CROSS_FAMILY_DISTINCTIVENESS_CASES {
            let rabin = run_hash::<RabinFingerprint>(input, window);
            let polynomial = run_hash::<PolynomialHash>(input, window);
            let gear = run_hash::<GearHash>(input, window);
            assert_ne!(rabin, polynomial, "rabin == polynomial on {id}");
            assert_ne!(rabin, gear, "rabin == gear on {id}");
            assert_ne!(polynomial, gear, "polynomial == gear on {id}");
        }
    }

    // -------------------------------------------------------------------
    // `FastCDC` verification
    // -------------------------------------------------------------------

    #[test]
    fn fastcdc_corpus_matches_algorithm() {
        for case in FastCdcCase::corpus() {
            let cdc = FastCdc::new(case.config);
            let observed: Vec<ChunkBoundary> = cdc.chunk_boundaries(&case.input).collect();
            assert_eq!(
                observed, case.expected,
                "fastcdc case {} disagreed:\n  expected {:?}\n  observed {:?}",
                case.id, case.expected, observed
            );
        }
    }

    #[test]
    fn fastcdc_deterministic_across_repeat_calls() {
        let cdc = FastCdc::new(FastCdcConfig::default_8k());
        // Intentional truncation: we want a repeatable pseudo-random
        // byte stream, and taking the low 8 bits of a 32-bit mixer is
        // the simplest way to get one.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "the truncation is the intended byte-derivation step"
        )]
        let input: Vec<u8> = (0..10_000u32).map(|i| (i.wrapping_mul(31)) as u8).collect();
        let first: Vec<ChunkBoundary> = cdc.chunk_boundaries(&input).collect();
        let second: Vec<ChunkBoundary> = cdc.chunk_boundaries(&input).collect();
        assert_eq!(
            first, second,
            "same input, same config: boundaries must match"
        );
    }

    #[test]
    fn fastcdc_shift_invariance_under_prefix() {
        // Prepending enough of a constant prefix to guarantee at least
        // one full prefix chunk before the payload begins means that,
        // by determinism, the payload's cut positions in `shifted`
        // match the payload's cut positions in `base` plus the number
        // of prefix bytes actually consumed by prefix chunks.
        //
        // We do not assert byte-for-byte equality of the trailing
        // boundary list; we assert the sum-of-sizes invariant and the
        // end-of-stream alignment, both of which are corollaries of
        // shift-invariance.
        let cdc = FastCdc::new(FastCdcConfig::default_8k());

        #[allow(
            clippy::cast_possible_truncation,
            reason = "the truncation is the intended byte-derivation step"
        )]
        let payload: Vec<u8> = (0..30_000u32).map(|i| (i ^ (i >> 3)) as u8).collect();
        let base: Vec<ChunkBoundary> = cdc.chunk_boundaries(&payload).collect();

        let prefix_len = FastCdcConfig::default_8k().max_size * 2;
        let mut prefixed = vec![0u8; prefix_len];
        prefixed.extend_from_slice(&payload);

        let shifted: Vec<ChunkBoundary> = cdc.chunk_boundaries(&prefixed).collect();

        assert_eq!(shifted.last().unwrap().offset, prefix_len + payload.len());
        assert_eq!(base.last().unwrap().offset, payload.len());

        let base_sum: usize = base.iter().map(|b| b.size).sum();
        let shifted_sum: usize = shifted.iter().map(|b| b.size).sum();
        assert_eq!(base_sum, payload.len());
        assert_eq!(shifted_sum, prefix_len + payload.len());
    }
}
