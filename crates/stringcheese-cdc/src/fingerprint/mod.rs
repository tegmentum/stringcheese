//! Rolling-hash fingerprints and the shared [`RollingHash`] trait.
//!
//! A rolling hash maintains a fixed-size sliding window over an input byte
//! stream and reports a hash of the current window in `O(1)` per byte fed.
//! StringCheese ships four families, distinguished by their numerical
//! properties:
//!
//! * [`RabinFingerprint`] — polynomial hash over `GF(2)`, defined by an
//!   irreducible polynomial. Strong collision properties; the classical
//!   choice for content-defined chunking and deduplication.
//! * [`PolynomialHash`] — Horner-form polynomial over the Mersenne-61
//!   prime field. Very fast; fine for cryptographically uninteresting
//!   workloads such as n-gram set hashing or Rabin-Karp substring search.
//! * [`Buzhash`] — Uzgalis's cyclic-polynomial rolling hash, a XOR of
//!   rotate-left'd substitution-table entries. The `rsync` and `restic`
//!   lineage of content-defined-chunking implementations use this
//!   construction.
//! * [`GearHash`] — the byte-indexed table-plus-shift hash that underlies
//!   `FastCDC`. Extremely cheap per byte on modern superscalar CPUs.
//!
//! All four implement the [`RollingHash`] trait, so downstream code can
//! pick a fingerprint at instantiation time and continue against a uniform
//! interface. Each implementation also exposes a `descriptor()` function
//! returning an [`AlgorithmDescriptor`] that pins down the specific
//! polynomial or table the implementation uses, so a golden case tied to
//! one parameter choice cannot silently be validated against another.
//!
//! [`AlgorithmDescriptor`]: stringcheese_core::AlgorithmDescriptor

#[cfg(feature = "alloc")]
pub mod buzhash;
pub mod gear;
#[cfg(feature = "alloc")]
pub mod polynomial;
#[cfg(feature = "alloc")]
pub mod rabin;

#[cfg(feature = "alloc")]
pub use buzhash::Buzhash;
pub use gear::GearHash;
#[cfg(feature = "alloc")]
pub use polynomial::PolynomialHash;
#[cfg(feature = "alloc")]
pub use rabin::RabinFingerprint;

/// A rolling hash over a sliding window of bytes.
///
/// Implementations maintain enough state to answer [`digest`] in constant
/// time after each call to [`roll`], regardless of how many bytes have been
/// fed. Once `window` bytes have been fed, subsequent calls to [`roll`]
/// slide out the oldest byte from the window's contribution — that is,
/// the digest after feeding a stream reflects only the last `window`
/// bytes.
///
/// # Contract
///
/// * `roll(byte)` runs in `O(1)` time and mutates the hash state.
/// * `digest()` runs in `O(1)` time and does not mutate the hash state.
/// * After `reset()` the hash state is identical to what a fresh
///   [`new(window)`] call would produce.
/// * Feeding fewer than `window` bytes and calling `digest()` returns a
///   hash over exactly the bytes fed so far — the window fills up rather
///   than being pre-padded with zeros.
///
/// [`digest`]: RollingHash::digest
/// [`roll`]: RollingHash::roll
/// [`new(window)`]: RollingHash::new
pub trait RollingHash {
    /// The hash's output type. Typically `u64`.
    type Output: Copy + Eq + Ord;

    /// Constructs a new rolling hash with the given window size.
    ///
    /// A `window` of zero is legal but degenerate — the hash never
    /// accumulates any bytes and always reports its identity digest.
    fn new(window: usize) -> Self;

    /// Feeds a byte into the hash, sliding the window forward by one byte.
    ///
    /// Once `window` bytes have been fed, subsequent calls slide out the
    /// oldest byte from the hash's contribution.
    fn roll(&mut self, byte: u8);

    /// Returns the current hash value.
    ///
    /// The digest reflects the last `window` bytes fed, or fewer if fewer
    /// have been fed since the last [`reset`].
    ///
    /// [`reset`]: RollingHash::reset
    fn digest(&self) -> Self::Output;

    /// Resets the hash to its initial (empty) state.
    ///
    /// The window size configured at construction is preserved.
    fn reset(&mut self);
}
