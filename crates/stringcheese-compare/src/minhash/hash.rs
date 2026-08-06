//! Hash primitives underlying the k-permutation `MinHash` construction.
//!
//! # Two-hash trick
//!
//! `MinHash`'s statistical guarantees require, for each of the `k`
//! permutations, an effectively pairwise-independent hash on the gram
//! space. Rather than run `k` full byte-hashers over every gram, this
//! module implements the well-known two-hash trick:
//!
//! ```text
//!     base_hash        = PortableHash::hash(gram)
//!     permuted_hash_i  = splitmix64(base_hash XOR permutation_seed_i)
//! ```
//!
//! The base hash is computed once per gram; the per-permutation values
//! are produced by a cheap `SplitMix64` finalizer applied to `base_hash
//! XOR seed_i`. `splitmix64` has excellent avalanche properties (it
//! passes the `SmallCrush` and Crush suites without failure), so the
//! per-permutation values behave, for `MinHash` purposes, as `k`
//! independent random projections of the underlying gram.
//!
//! # Portability
//!
//! Both primitives here are byte-for-byte deterministic across native and
//! WebAssembly targets and across debug vs. release builds. The sketch
//! and the `LSH` index depend on this determinism — a golden case's stored
//! signatures would silently rot otherwise.
//!
//! # References
//!
//! * Fowler, G., Noll, L. C., & Vo, K.-P. "FNV Hash."
//!   <http://www.isthe.com/chongo/tech/comp/fnv/> — reference for the
//!   `FNV-1a` construction underlying [`PortableHasher`].
//! * Steele, G. L., Lea, D., & Flood, C. H. (2014). "Fast splittable
//!   pseudorandom number generators." *ACM SIGPLAN Notices*, 49(10),
//!   453-472. <https://doi.org/10.1145/2714064.2660195> — the
//!   `SplitMix64` finalizer used by [`splitmix64`].

use core::hash::{Hash, Hasher};

/// `SplitMix64` finalizer (Sebastiano Vigna, `xoshiro` family).
///
/// This is the pure bit-mixing step from `SplitMix64` — the state-advance
/// half is not needed here because callers supply the state (the
/// per-permutation seed) explicitly.
///
/// Its role in the k-permutation `MinHash` construction is to turn a
/// merely-decent base hash into `k` effectively-independent permuted
/// hashes: `splitmix64(base XOR seed_i)` is what the sketch's per-item
/// per-permutation reduction consumes.
#[inline]
#[must_use]
pub const fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// `FNV-1a` offset basis. Not something a caller ever needs, but exposed
/// as a constant so the seeded-hasher variant can be reconstructed
/// deterministically from a `u64` seed.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// `FNV-1a` prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A deterministic, portable `FNV-1a`-based [`Hasher`].
///
/// Two callers on different platforms hashing the same byte sequence
/// with the same seed always get the same `u64` back. This is what makes
/// `MinHash` sketches transferable across a native producer and a
/// WebAssembly consumer.
///
/// # Not for hash-map use
///
/// `FNV-1a` is intentionally weak against adversarial inputs. This hasher
/// exists solely as the *base* hash in a two-hash construction; the
/// per-permutation `splitmix64` step is what supplies the mixing `MinHash`
/// actually needs. Do not reach for [`PortableHasher`] as a general-purpose
/// hash-map hasher.
#[derive(Copy, Clone, Debug)]
pub struct PortableHasher {
    /// The `FNV-1a` running state, mixed with the caller-supplied seed on
    /// construction so different seeds yield independent hash streams.
    state: u64,
}

impl PortableHasher {
    /// Constructs a fresh hasher seeded with `seed`.
    ///
    /// The seed is XOR'd into the `FNV` offset basis before hashing begins,
    /// so different seeds produce different `u64` outputs for the same
    /// input byte sequence.
    #[inline]
    #[must_use]
    pub const fn with_seed(seed: u64) -> Self {
        Self {
            state: FNV_OFFSET ^ seed,
        }
    }
}

impl Default for PortableHasher {
    #[inline]
    fn default() -> Self {
        Self::with_seed(0)
    }
}

impl Hasher for PortableHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.state
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        // Classical `FNV-1a`: XOR then multiply per byte. Wrapping is the
        // intended arithmetic; the multiplication should overflow modulo
        // 2^64.
        let mut h = self.state;
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(FNV_PRIME);
        }
        self.state = h;
    }
}

/// Hashes a [`Hash`] value with [`PortableHasher`] seeded by `seed`.
///
/// This is the "base hash" step of the k-permutation two-hash `MinHash`
/// construction; the per-permutation step is [`permuted_hash`].
#[inline]
#[must_use]
pub fn portable_hash<T: Hash + ?Sized>(seed: u64, value: &T) -> u64 {
    let mut h = PortableHasher::with_seed(seed);
    value.hash(&mut h);
    h.finish()
}

/// Per-permutation hash for `MinHash`'s k-permutation scheme.
///
/// Given a gram's `base_hash` and a permutation seed `permutation_seed`,
/// returns the permuted `u64` the sketch's `min`-reduction consumes.
///
/// The seed for permutation `i` is derived from the sketch's caller
/// seed and `i` by [`permutation_seed`]; the two functions together
/// implement the seed derivation the sketch uses at construction time.
#[inline]
#[must_use]
pub const fn permuted_hash(base_hash: u64, permutation_seed: u64) -> u64 {
    splitmix64(base_hash ^ permutation_seed)
}

/// Derives permutation seed `i` from a sketch's caller-supplied `seed`.
///
/// Two sketches constructed with the same caller seed and the same `k`
/// produce the same permutation seed sequence, which is what makes
/// [`crate::MinHashSketch::estimated_jaccard`] well-defined across a
/// pair of sketches.
///
/// The derivation is a `splitmix64` of `seed XOR i` — this gives well-mixed,
/// well-separated per-permutation seeds even when the caller passes
/// `seed = 0` and the permutation indices are small consecutive integers.
#[inline]
#[must_use]
pub const fn permutation_seed(seed: u64, index: usize) -> u64 {
    // `index` is bounded by the sketch size `k`; a lossy cast to `u64`
    // is safe here because k > 2^64 is unrepresentable and impossible.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "usize -> u64 is widening on all supported platforms"
    )]
    let ix = index as u64;
    splitmix64(seed ^ ix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_is_deterministic_and_seed_sensitive() {
        assert_eq!(splitmix64(42), splitmix64(42));
        assert_ne!(splitmix64(42), splitmix64(43));
        // Well-mixed outputs never sit near a boundary for adjacent
        // seeds — every additional guard against a broken finalizer.
        assert!(splitmix64(0) != 0);
        assert!(splitmix64(1) != 1);
    }

    #[test]
    fn splitmix64_neighbors_differ_in_many_bits() {
        // A good finalizer's outputs on adjacent inputs should differ in
        // roughly half their bits — a strict "at least 16" check is far
        // below the expected 32 but far above the couple of bits a broken
        // mixing constant would produce.
        let d = (splitmix64(0) ^ splitmix64(1)).count_ones();
        assert!(d >= 16, "SplitMix64 neighbor bit-diff too small: {d}");
    }

    #[test]
    fn portable_hash_is_deterministic() {
        assert_eq!(portable_hash(7, "abc"), portable_hash(7, "abc"));
    }

    #[test]
    fn portable_hash_varies_by_seed() {
        assert_ne!(portable_hash(0, "abc"), portable_hash(1, "abc"));
    }

    #[test]
    fn portable_hash_varies_by_input() {
        assert_ne!(portable_hash(7, "abc"), portable_hash(7, "abd"));
    }

    #[test]
    fn permutation_seed_is_deterministic_and_index_sensitive() {
        assert_eq!(permutation_seed(42, 0), permutation_seed(42, 0));
        assert_ne!(permutation_seed(42, 0), permutation_seed(42, 1));
        assert_ne!(permutation_seed(42, 0), permutation_seed(43, 0));
    }

    #[test]
    fn permuted_hash_is_deterministic() {
        assert_eq!(permuted_hash(0xdead_beef, 7), permuted_hash(0xdead_beef, 7));
    }
}
