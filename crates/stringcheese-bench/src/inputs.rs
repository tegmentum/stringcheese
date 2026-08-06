//! Deterministic input-generation helpers shared across every benchmark.
//!
//! Each benchmark binary needs the same three shapes of corpus:
//!
//! * A single random ASCII string of a chosen length.
//! * A "similar" pair: two strings of the requested length that differ by
//!   a target *edit rate* — approximately `edit_rate * len` single-symbol
//!   substitutions, insertions, or deletions.
//! * An "identical" pair: two byte-equal copies of a random string.
//!
//! Every helper is seeded from a `u64` and threads that seed through a
//! tiny hand-rolled `SplitMix64` PRNG. No external RNG crate is pulled
//! in — the goal here is a benchmark-only corpus, not cryptography, and
//! the standard-library RNG is unavailable in the deps we ship.
//! `SplitMix64` is a two-line function (see [`Rng`]) that passes `BigCrush`
//! and is what `rand`'s `SeedableRng::seed_from_u64` uses internally to
//! expand a seed. That's more than enough for benchmark inputs.

use alloc::vec::Vec;

/// A minimal `SplitMix64` PRNG.
///
/// Not cryptographic; not the fastest generator on the shelf; not exposed
/// as a general-purpose type. It is here because every benchmark input
/// needs a deterministic seedable RNG, and pulling in `rand` — a
/// several-crate dependency stack — for a two-line generator is
/// disproportionate.
#[derive(Clone, Copy, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Creates a new PRNG from a 64-bit seed.
    #[inline]
    #[must_use]
    pub const fn from_seed(seed: u64) -> Self {
        // The all-zero seed is a legitimate SplitMix64 state, but folding
        // in a non-zero constant here keeps callers who pass `0` from
        // getting an identical prefix to callers who pass, say, `1` after
        // one `next_u64`. Any nonzero mixer works.
        Self {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    /// Returns the next 64-bit output and advances the state.
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        // SplitMix64 as published by Sebastiano Vigna. The three
        // multiply-xor-shift steps are load-bearing; do not "simplify".
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a `usize` uniformly in `[0, bound)`.
    ///
    /// Uses a plain modulo reduction. The bias is negligible for the
    /// small bounds this crate uses (`26` for alphabet lookups, at most
    /// `len` for position picks), and `SplitMix64` mixes both ends of
    /// its output so the low bits are as good as the high bits.
    #[inline]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "targets with 32-bit `usize` truncate the reduction result; the returned value is always `< bound` and `bound` came in as a `usize`, so truncation preserves the semantics on both 32- and 64-bit targets"
    )]
    pub fn next_bounded(&mut self, bound: usize) -> usize {
        debug_assert!(bound > 0, "next_bounded needs a nonzero bound");
        (self.next_u64() % bound as u64) as usize
    }

    /// Returns a random lowercase ASCII byte in `b'a'..=b'z'`.
    #[inline]
    pub fn next_ascii_lower(&mut self) -> u8 {
        b'a' + u8::try_from(self.next_bounded(26)).unwrap_or(0)
    }
}

/// Returns a fresh `Vec<u8>` of `len` lowercase ASCII bytes, seeded
/// deterministically from `seed`.
///
/// The alphabet is `a`–`z`; that is enough entropy for edit-distance
/// benchmarks (a 26-symbol alphabet gives a low expected match rate at
/// random alignment, which is where the DP kernels spend their time) and
/// keeps the generated inputs debuggable when a benchmark fails.
#[must_use]
pub fn random_ascii(len: usize, seed: u64) -> Vec<u8> {
    let mut rng = Rng::from_seed(seed);
    (0..len).map(|_| rng.next_ascii_lower()).collect()
}

/// Returns two byte-equal random ASCII strings of length `len`, seeded
/// from `seed`.
///
/// This is the "identical" corner of the input space — the fast path for
/// every kernel that short-circuits on an exact-match check, and the
/// baseline against which "similar" and "random" pairs should compare.
#[must_use]
pub fn identical_pair(len: usize, seed: u64) -> (Vec<u8>, Vec<u8>) {
    let s = random_ascii(len, seed);
    let t = s.clone();
    (s, t)
}

/// Returns two random ASCII strings of approximately length `len` that
/// differ by roughly `edit_rate * len` edits.
///
/// The exact number of edits is `(edit_rate * len as f64).round() as
/// usize`, and each edit is chosen uniformly at random among substitution,
/// insertion at a random position, and deletion at a random position.
/// The result is not the *minimal* edit distance between the two strings
/// — a substitution followed by a matching insertion may cancel out —
/// which is exactly what a benchmark wants: representative "similar"
/// inputs of the requested magnitude, not perfectly-tuned edit-scripts.
///
/// Because insertions and deletions cancel out on average, the returned
/// right-hand-side length is close to but not exactly `len`. Callers that
/// need equal-length inputs (Hamming) should use [`similar_pair_equal_len`]
/// instead.
///
/// # Panics
///
/// Debug-only: panics if `edit_rate` is negative or non-finite. Release
/// builds silently coerce to `0`.
#[must_use]
pub fn similar_pair(len: usize, edit_rate: f64, seed: u64) -> (Vec<u8>, Vec<u8>) {
    debug_assert!(
        edit_rate.is_finite() && edit_rate >= 0.0,
        "edit_rate must be finite and non-negative"
    );
    let left = random_ascii(len, seed);
    let mut right = left.clone();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let n_edits = ((len as f64) * edit_rate).round().max(0.0) as usize;
    // Fresh RNG seeded distinctly so the edit script is uncorrelated with
    // the source sequence.
    let mut rng = Rng::from_seed(seed.wrapping_add(0xA5A5_A5A5_A5A5_A5A5));
    for _ in 0..n_edits {
        if right.is_empty() {
            // Only insertions are meaningful once the string is empty.
            right.push(rng.next_ascii_lower());
            continue;
        }
        let op = rng.next_bounded(3);
        let pos = rng.next_bounded(right.len());
        match op {
            0 => {
                // Substitution: replace with a fresh (possibly equal) symbol.
                right[pos] = rng.next_ascii_lower();
            }
            1 => {
                // Insertion at pos.
                right.insert(pos, rng.next_ascii_lower());
            }
            _ => {
                // Deletion at pos.
                right.remove(pos);
            }
        }
    }
    (left, right)
}

/// Returns two equal-length random ASCII strings of length `len` that
/// differ in approximately `edit_rate * len` positions.
///
/// Every edit is a substitution at a randomly chosen position. Because
/// positions may collide, the actual differing-position count can be
/// slightly below the target — that is acceptable for benchmark inputs
/// and matches the "approximately" contract [`similar_pair`] documents.
///
/// This helper exists so that Hamming (which is only defined for
/// equal-length inputs) has a legitimate "similar" corpus. Use
/// [`similar_pair`] where insertions and deletions are also allowed.
///
/// # Panics
///
/// Debug-only: panics if `edit_rate` is outside `[0.0, 1.0]` or
/// non-finite. Release builds coerce to `[0, len]`.
#[must_use]
pub fn similar_pair_equal_len(len: usize, edit_rate: f64, seed: u64) -> (Vec<u8>, Vec<u8>) {
    debug_assert!(
        edit_rate.is_finite() && (0.0..=1.0).contains(&edit_rate),
        "edit_rate must be finite and in [0.0, 1.0]"
    );
    let left = random_ascii(len, seed);
    let mut right = left.clone();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let n_edits = ((len as f64) * edit_rate).round().max(0.0) as usize;
    let n_edits = n_edits.min(len);
    if n_edits == 0 || len == 0 {
        return (left, right);
    }
    let mut rng = Rng::from_seed(seed.wrapping_add(0xC3C3_C3C3_C3C3_C3C3));
    for _ in 0..n_edits {
        let pos = rng.next_bounded(len);
        // Force a genuine substitution: cycle to the next letter mod 26.
        let old = right[pos];
        let bump = 1 + u8::try_from(rng.next_bounded(25)).unwrap_or(0);
        right[pos] = b'a' + ((old - b'a' + bump) % 26);
    }
    (left, right)
}

/// Returns a fresh batch of `count` random ASCII candidate strings, each
/// of length `len`.
///
/// Used by the batch benchmark (a query vs. a corpus of candidates) so
/// that every iteration walks the same deterministic candidate set.
/// The `seed` advances by `count` under the hood, so two calls with the
/// same base seed and disjoint count ranges yield disjoint corpora.
#[must_use]
pub fn random_candidates(count: usize, len: usize, seed: u64) -> Vec<Vec<u8>> {
    (0..count)
        .map(|i| random_ascii(len, seed.wrapping_add(i as u64)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_yields_same_bytes() {
        let a = random_ascii(64, 0x00C0_FFEE_u64);
        let b = random_ascii(64, 0x00C0_FFEE_u64);
        assert_eq!(a, b);
    }

    #[test]
    fn different_seeds_diverge() {
        let a = random_ascii(64, 1);
        let b = random_ascii(64, 2);
        assert_ne!(a, b);
    }

    #[test]
    fn random_ascii_stays_in_alphabet() {
        let s = random_ascii(256, 42);
        assert!(s.iter().all(u8::is_ascii_lowercase));
    }

    #[test]
    fn identical_pair_is_byte_equal() {
        let (a, b) = identical_pair(128, 7);
        assert_eq!(a, b);
    }

    #[test]
    fn similar_pair_at_zero_rate_is_identical() {
        let (a, b) = similar_pair(64, 0.0, 3);
        assert_eq!(a, b);
    }

    #[test]
    fn similar_pair_at_positive_rate_diverges() {
        let (a, b) = similar_pair(128, 0.25, 5);
        assert_ne!(a, b);
    }

    #[test]
    fn similar_pair_equal_len_preserves_length() {
        let (a, b) = similar_pair_equal_len(64, 0.1, 11);
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn random_candidates_are_distinct_for_distinct_seeds() {
        let cs = random_candidates(4, 16, 100);
        assert_eq!(cs.len(), 4);
        // With len=16 and a 26-symbol alphabet the birthday probability
        // of a collision is astronomically low; assert distinctness.
        for i in 0..cs.len() {
            for j in (i + 1)..cs.len() {
                assert_ne!(cs[i], cs[j]);
            }
        }
    }
}
