//! Rabin-Karp substring search over `&[u8]`.
//!
//! # Algorithm
//!
//! A polynomial rolling hash is computed over the pattern once at
//! [`prepare`](RabinKarp::prepare) time. The scan then computes the same
//! polynomial hash over successive windows of the haystack — each window's
//! hash is derived from the previous one in constant time by subtracting
//! the leaving byte's contribution and adding the entering byte's
//! contribution.
//!
//! On every window where the rolling hash equals the pattern hash, the
//! window is verified byte-by-byte against the pattern. The verification
//! step is **not optional** — a hash collision would otherwise cause a
//! false-positive match — and the sibling verification cost is what makes
//! Rabin-Karp's average case competitive with the more sophisticated
//! algorithms in this crate while remaining conceptually simple enough to
//! serve as a differential oracle for them.
//!
//! # Modulus and base
//!
//! The hash is taken modulo the Mersenne prime `2^61 − 1`. That prime fits
//! in a `u64`, which lets the intermediate multiplication `hash * base +
//! byte` be computed as a `u128` in one machine word on 64-bit targets
//! without saturation. The base is `257` — the smallest prime greater than
//! `u8::MAX`, which ensures no two single-byte values hash to the same
//! residue class modulo the base. Together these give:
//!
//! * a very low false-positive rate on random input (`< 2^-61` per window);
//! * deterministic output — no random seed;
//! * `no_std` + `alloc`-only implementation, no floating-point involved.
//!
//! # Fingerprint sharing note
//!
//! `comparand-cdc` also needs a polynomial rolling hash for content-defined
//! chunking. Both crates are landing in parallel; the two rolling-hash
//! implementations should be consolidated into a shared
//! `comparand-fingerprint` crate once both are settled. For now the
//! implementation is inlined here to keep the crate self-contained.
//!
//! TODO(comparand-cdc): consolidate rolling-hash into `comparand-fingerprint`.
//!
//! # Descriptor
//!
//! The variant slug is `"polynomial-mersenne-61-base-257"`. Golden test
//! cases reference this variant rather than the common name so that a
//! future implementation with a different modulus, base, or hash
//! definition cannot be silently validated against these cases.

use alloc::vec::Vec;

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::api::{Match, SearchAlgorithm, SinglePatternSearch};

/// The Mersenne prime modulus used for the rolling hash: `2^61 − 1`.
///
/// Values reduced modulo this prime fit in `u64` with room to spare, so
/// `u128` intermediate arithmetic never overflows on the range Rabin-Karp
/// actually exercises.
const MODULUS: u64 = (1u64 << 61) - 1;

/// The polynomial base used by the rolling hash.
///
/// `257` is the smallest prime greater than `u8::MAX`; no two single-byte
/// values collide modulo the base, and the value keeps intermediate
/// arithmetic well inside the range covered by the `u128` multiply below.
const BASE: u64 = 257;

/// Rabin-Karp substring search (Karp & Rabin, 1987).
///
/// A zero-sized unit struct that carries the algorithm's descriptor and the
/// `prepare` / `find` / `find_all` methods. See the module documentation
/// for the algorithm and its guarantees.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RabinKarp;

/// Preprocessed state produced by [`RabinKarp::prepare`].
///
/// Holds the pattern bytes (cloned so the state can outlive the caller's
/// slice), the pattern's polynomial hash, and the precomputed
/// `BASE^(pattern.len() − 1) mod MODULUS` factor used to remove the
/// leaving byte's contribution during the rolling update.
#[derive(Clone, Debug)]
pub struct RabinKarpPrepared {
    /// The pattern, cloned into an owned buffer.
    pattern: Vec<u8>,
    /// The pattern's polynomial hash modulo [`MODULUS`].
    pattern_hash: u64,
    /// `BASE^(pattern.len() − 1) mod MODULUS`, or `0` when the pattern is
    /// empty. Used to subtract the leaving byte's contribution during the
    /// rolling update.
    leading_factor: u64,
}

impl RabinKarpPrepared {
    /// Returns the pattern used to build this state.
    #[inline]
    #[must_use]
    pub fn pattern(&self) -> &[u8] {
        &self.pattern
    }

    /// Returns the pattern's precomputed polynomial hash.
    ///
    /// Exposed for cross-crate verification; not required by ordinary
    /// callers.
    #[inline]
    #[must_use]
    pub fn pattern_hash(&self) -> u64 {
        self.pattern_hash
    }
}

impl RabinKarp {
    /// The algorithm descriptor for this variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::RabinKarp,
        variant: VariantId("polynomial-mersenne-61-base-257"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Efficient randomized pattern-matching algorithms",
            authors: "R. M. Karp, M. O. Rabin",
            year: 1987,
        },
    };

    /// Returns the algorithm descriptor for this variant.
    ///
    /// A `const` accessor for use where the trait method is not available
    /// (for example, pinning the descriptor into a `const GOLDEN_CASE`).
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }
}

impl SearchAlgorithm for RabinKarp {
    type Prepared = RabinKarpPrepared;

    fn prepare(pattern: &[u8]) -> Self::Prepared {
        let mut hash: u64 = 0;
        for &b in pattern {
            hash = mul_mod(hash, BASE) + u64::from(b);
            hash = reduce_mod(hash);
        }
        let leading_factor = if pattern.is_empty() {
            0
        } else {
            pow_mod(BASE, (pattern.len() - 1) as u64)
        };
        RabinKarpPrepared {
            pattern: pattern.to_vec(),
            pattern_hash: hash,
            leading_factor,
        }
    }

    fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }
}

impl SinglePatternSearch for RabinKarp {
    fn find(prepared: &Self::Prepared, haystack: &[u8]) -> Option<Match> {
        let m = prepared.pattern.len();
        // Empty pattern matches at position 0.
        if m == 0 {
            return Some(Match::new(0));
        }
        if haystack.len() < m {
            return None;
        }

        let mut window_hash = initial_window_hash(&haystack[..m]);
        if window_hash == prepared.pattern_hash && haystack[..m] == prepared.pattern[..] {
            return Some(Match::new(0));
        }

        let n = haystack.len();
        for i in 1..=n - m {
            // Remove the leaving byte's contribution, then multiply by BASE
            // to shift the window, then add the entering byte.
            let leaving = u64::from(haystack[i - 1]);
            let entering = u64::from(haystack[i + m - 1]);
            // Subtracting inside modular arithmetic: add MODULUS to keep
            // the intermediate non-negative under u64.
            let leaving_contribution = mul_mod(leaving, prepared.leading_factor);
            window_hash = window_hash + MODULUS - leaving_contribution;
            window_hash = reduce_mod(window_hash);
            window_hash = mul_mod(window_hash, BASE) + entering;
            window_hash = reduce_mod(window_hash);

            if window_hash == prepared.pattern_hash && haystack[i..i + m] == prepared.pattern[..] {
                return Some(Match::new(i));
            }
        }
        None
    }

    fn find_all(prepared: &Self::Prepared, haystack: &[u8]) -> Vec<Match> {
        let m = prepared.pattern.len();
        // Empty pattern matches at position 0 exactly once — same policy
        // as strstr / memmem.
        if m == 0 {
            return alloc::vec![Match::new(0)];
        }
        let mut out = Vec::new();
        if haystack.len() < m {
            return out;
        }

        let mut window_hash = initial_window_hash(&haystack[..m]);
        if window_hash == prepared.pattern_hash && haystack[..m] == prepared.pattern[..] {
            out.push(Match::new(0));
        }

        let n = haystack.len();
        for i in 1..=n - m {
            let leaving = u64::from(haystack[i - 1]);
            let entering = u64::from(haystack[i + m - 1]);
            let leaving_contribution = mul_mod(leaving, prepared.leading_factor);
            window_hash = window_hash + MODULUS - leaving_contribution;
            window_hash = reduce_mod(window_hash);
            window_hash = mul_mod(window_hash, BASE) + entering;
            window_hash = reduce_mod(window_hash);

            if window_hash == prepared.pattern_hash && haystack[i..i + m] == prepared.pattern[..] {
                out.push(Match::new(i));
            }
        }
        out
    }
}

/// Computes the polynomial hash of a single window of length `m`.
#[inline]
fn initial_window_hash(window: &[u8]) -> u64 {
    let mut h: u64 = 0;
    for &b in window {
        h = mul_mod(h, BASE) + u64::from(b);
        h = reduce_mod(h);
    }
    h
}

/// `(a * b) mod MODULUS`, computed via a `u128` intermediate that cannot
/// overflow given inputs in `[0, MODULUS)`.
#[inline]
fn mul_mod(a: u64, b: u64) -> u64 {
    let product = (u128::from(a)) * (u128::from(b));
    // MODULUS fits in 61 bits, so the product fits in 122 bits — well
    // within u128's 128-bit range.
    let modulus_128 = u128::from(MODULUS);
    let reduced = product % modulus_128;
    // `reduced` is strictly less than MODULUS (61 bits) and therefore
    // fits in u64 without loss.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "reduced < MODULUS (61 bits); fits in u64 without loss"
    )]
    {
        reduced as u64
    }
}

/// `base^exp mod MODULUS`, computed by exponentiation-by-squaring.
fn pow_mod(base: u64, mut exp: u64) -> u64 {
    let mut result: u64 = 1;
    let mut b = base % MODULUS;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod(result, b);
        }
        exp >>= 1;
        b = mul_mod(b, b);
    }
    result
}

/// Reduces a value that may be up to slightly above `MODULUS` back into
/// `[0, MODULUS)`.
///
/// After the arithmetic `mul_mod(x, y) + byte` the input is bounded by
/// `MODULUS + u8::MAX`, so a single subtraction suffices.
#[inline]
fn reduce_mod(x: u64) -> u64 {
    if x >= MODULUS { x - MODULUS } else { x }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_pins_variant_and_source() {
        let d = RabinKarp::descriptor();
        assert_eq!(d.family, AlgorithmFamily::RabinKarp);
        assert_eq!(d.variant, VariantId("polynomial-mersenne-61-base-257"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1987, .. }
        ));
    }

    #[test]
    fn descriptor_is_const() {
        const D: AlgorithmDescriptor = RabinKarp::DESCRIPTOR;
        assert_eq!(D.variant.0, "polynomial-mersenne-61-base-257");
    }

    #[test]
    fn find_returns_first_match() {
        let p = RabinKarp::prepare(b"abc");
        assert_eq!(RabinKarp::find(&p, b"xxabcxxabc"), Some(Match::new(2)));
    }

    #[test]
    fn find_returns_none_when_absent() {
        let p = RabinKarp::prepare(b"xyz");
        assert_eq!(RabinKarp::find(&p, b"abcabcabc"), None);
    }

    #[test]
    fn find_at_start_and_end() {
        let p = RabinKarp::prepare(b"abc");
        assert_eq!(RabinKarp::find(&p, b"abcxx"), Some(Match::new(0)));
        assert_eq!(RabinKarp::find(&p, b"xxabc"), Some(Match::new(2)));
    }

    #[test]
    fn find_all_overlapping() {
        let p = RabinKarp::prepare(b"aa");
        let matches = RabinKarp::find_all(&p, b"aaaa");
        assert_eq!(
            matches,
            alloc::vec![Match::new(0), Match::new(1), Match::new(2)]
        );
    }

    #[test]
    fn empty_pattern_matches_at_zero() {
        let p = RabinKarp::prepare(b"");
        assert_eq!(RabinKarp::find(&p, b"abc"), Some(Match::new(0)));
        assert_eq!(RabinKarp::find_all(&p, b"abc"), alloc::vec![Match::new(0)]);
    }

    #[test]
    fn empty_haystack_finds_nothing() {
        let p = RabinKarp::prepare(b"abc");
        assert_eq!(RabinKarp::find(&p, b""), None);
        assert!(RabinKarp::find_all(&p, b"").is_empty());
    }

    #[test]
    fn pattern_longer_than_haystack_finds_nothing() {
        let p = RabinKarp::prepare(b"abcdef");
        assert_eq!(RabinKarp::find(&p, b"abc"), None);
    }

    #[test]
    fn pattern_equal_to_haystack_matches_at_zero() {
        let p = RabinKarp::prepare(b"abc");
        assert_eq!(RabinKarp::find(&p, b"abc"), Some(Match::new(0)));
    }

    #[test]
    fn all_positions_of_single_char() {
        let p = RabinKarp::prepare(b"a");
        let matches = RabinKarp::find_all(&p, b"aaaa");
        assert_eq!(matches.len(), 4);
        assert_eq!(matches[0].position, 0);
        assert_eq!(matches[3].position, 3);
    }

    #[test]
    fn preparation_stores_pattern() {
        let p = RabinKarp::prepare(b"hello");
        assert_eq!(p.pattern(), b"hello");
    }
}
