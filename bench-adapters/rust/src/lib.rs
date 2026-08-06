//! Rust-ecosystem bench adapters for Comparand.
//!
//! This crate is the Rust language slice of Comparand's
//! `bench-adapters/` subsystem — a language-agnostic head-to-head
//! benchmark harness. See `bench-adapters/README.md` for the umbrella
//! design and `bench-adapters/rust/README.md` for the caveats specific
//! to running these benches.
//!
//! # Why this crate is standalone
//!
//! `comparand-bench` — the toolkit's own criterion suite — cannot depend
//! on `strsim` or `rapidfuzz` without leaking those crates into every
//! Comparand consumer's dependency graph, so the head-to-head benches
//! live in a separate, workspace-isolated crate under `bench-adapters/`.
//! The isolation is enforced by the empty `[workspace]` sentinel at the
//! top of this crate's `Cargo.toml`.
//!
//! # What lives here
//!
//! Only the shared harness: a deterministic `SplitMix64` RNG and the
//! three canonical pair-construction helpers (random / similar /
//! identical). The per-algorithm head-to-head matrices live in
//! `benches/*.rs`, one per (Comparand algorithm × external library)
//! pair.
//!
//! # Determinism
//!
//! Every helper is seeded from a `u64` and threads that seed through
//! `SplitMix64`. That is the same PRNG family
//! `comparand-bench::inputs` uses, so a bench at length `N` and seed
//! `S` in the adapter walks the same class of input as a bench at the
//! same `(N, S)` in the toolkit's own suite. The generator is
//! duplicated rather than depended on because `comparand-bench` is not
//! (and should not become) a dependency of this crate — its own
//! `[dev-dependencies]` on criterion would pull in an extra copy of
//! criterion, and we want a single, adapter-owned criterion instance
//! driving the head-to-head runs.

#![allow(missing_docs)]
#![forbid(unsafe_code)]

/// A minimal `SplitMix64` PRNG.
///
/// Duplicated from `comparand-bench::inputs` verbatim (`SplitMix64` as
/// published by Sebastiano Vigna). Not cryptographic; the sole purpose
/// is deterministic, seedable, cheap random ASCII for benchmark
/// corpora. Kept private-ish (`pub` on the module surface only because
/// the bench binaries live in a sibling crate slot and cannot see
/// crate-private items).
#[derive(Clone, Copy, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    #[inline]
    #[must_use]
    pub const fn from_seed(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    #[inline]
    #[allow(clippy::cast_possible_truncation)]
    pub fn next_bounded(&mut self, bound: usize) -> usize {
        debug_assert!(bound > 0, "next_bounded needs a nonzero bound");
        (self.next_u64() % bound as u64) as usize
    }

    #[inline]
    pub fn next_ascii_lower(&mut self) -> u8 {
        b'a' + u8::try_from(self.next_bounded(26)).unwrap_or(0)
    }
}

/// Returns a fresh `Vec<u8>` of `len` lowercase-ASCII bytes.
///
/// Alphabet `a`..=`z`; UTF-8-valid so the same buffer can back a
/// `String` via `String::from_utf8` without a copy penalty at bench
/// setup time.
#[must_use]
pub fn random_ascii(len: usize, seed: u64) -> Vec<u8> {
    let mut rng = Rng::from_seed(seed);
    (0..len).map(|_| rng.next_ascii_lower()).collect()
}

/// Two byte-equal random ASCII strings — the "identical" corner.
///
/// Some external kernels short-circuit on an exact-match check. This
/// regime is the price of that short-circuit for kernels that have one
/// and the baseline cost for kernels that do not.
#[must_use]
pub fn identical_pair(len: usize, seed: u64) -> (Vec<u8>, Vec<u8>) {
    let s = random_ascii(len, seed);
    let t = s.clone();
    (s, t)
}

/// Two random ASCII strings of approximately length `len` differing by
/// roughly `edit_rate * len` mixed insertions, deletions, and
/// substitutions.
///
/// Length is only approximate — insertions and deletions cancel on
/// average. Callers that need equal-length inputs (Hamming) should use
/// [`similar_pair_equal_len`].
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
    let mut rng = Rng::from_seed(seed.wrapping_add(0xA5A5_A5A5_A5A5_A5A5));
    for _ in 0..n_edits {
        if right.is_empty() {
            right.push(rng.next_ascii_lower());
            continue;
        }
        let op = rng.next_bounded(3);
        let pos = rng.next_bounded(right.len());
        match op {
            0 => right[pos] = rng.next_ascii_lower(),
            1 => right.insert(pos, rng.next_ascii_lower()),
            _ => {
                right.remove(pos);
            }
        }
    }
    (left, right)
}

/// Two equal-length random ASCII strings differing in approximately
/// `edit_rate * len` positions — substitutions only.
///
/// The Hamming-adjacent regime; positions may collide so the actual
/// mismatch count can be slightly below the target.
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
        let old = right[pos];
        let bump = 1 + u8::try_from(rng.next_bounded(25)).unwrap_or(0);
        right[pos] = b'a' + ((old - b'a' + bump) % 26);
    }
    (left, right)
}

/// The full corpus fixture a head-to-head bench needs.
///
/// Precomputes every representation of the two inputs — bytes, an owned
/// `String`, and a `Vec<char>` — outside the timing loop, so each
/// per-implementation bench pays only the algorithm's cost, not the
/// representation-materialisation cost that the algorithm's own
/// caller would.
///
/// The `Vec<char>` variant is present because a Comparand kernel called
/// with `&[char]` is doing the same character-level work `strsim`'s
/// `levenshtein(&str, &str)` does internally; the byte-slice variant
/// is present because that is Comparand's fast path.
#[derive(Clone)]
pub struct Pair {
    pub a_bytes: Vec<u8>,
    pub b_bytes: Vec<u8>,
    pub a_string: String,
    pub b_string: String,
    pub a_chars: Vec<char>,
    pub b_chars: Vec<char>,
}

impl Pair {
    /// Builds a `Pair` from two byte buffers.
    ///
    /// Assumes ASCII; the `String`/`Vec<char>` derivations short-circuit
    /// on that assumption (`from_utf8` is O(n) but does no allocation
    /// beyond the vec, and `.chars()` on ASCII lifts to a `map(char::from)`).
    ///
    /// # Panics
    ///
    /// Panics if either input is not valid UTF-8. Callers in this
    /// crate always pass output from [`random_ascii`], which is
    /// valid UTF-8 by construction; the panic path is unreachable
    /// under normal use and exists only so that a future accidental
    /// caller with binary data fails loudly at setup time rather
    /// than silently at first bench iteration.
    #[must_use]
    pub fn from_bytes(a: Vec<u8>, b: Vec<u8>) -> Self {
        let a_string =
            String::from_utf8(a.clone()).expect("random_ascii yields valid UTF-8 by construction");
        let b_string =
            String::from_utf8(b.clone()).expect("random_ascii yields valid UTF-8 by construction");
        let a_chars: Vec<char> = a_string.chars().collect();
        let b_chars: Vec<char> = b_string.chars().collect();
        Self {
            a_bytes: a,
            b_bytes: b,
            a_string,
            b_string,
            a_chars,
            b_chars,
        }
    }
}

/// The three canonical similarity regimes, keyed by string tag so the
/// bench-name axis reads the same across every head-to-head file and
/// against `comparand-bench`'s own suite.
#[must_use]
pub fn build_pair(len: usize, kind: &str, salts: (u64, u64, u64)) -> Pair {
    let (r_a, r_b, sim_or_ident) = salts;
    let (a, b) = match kind {
        "random" => (
            random_ascii(len, seed_for(len, r_a)),
            random_ascii(len, seed_for(len, r_b)),
        ),
        "similar" => similar_pair(len, 0.05, seed_for(len, sim_or_ident)),
        "identical" => identical_pair(len, seed_for(len, sim_or_ident)),
        _ => unreachable!("unknown similarity regime: {kind}"),
    };
    Pair::from_bytes(a, b)
}

/// Equal-length variant of [`build_pair`] for Hamming.
#[must_use]
pub fn build_pair_equal_len(len: usize, kind: &str, salts: (u64, u64, u64)) -> Pair {
    let (r_a, r_b, sim_or_ident) = salts;
    let (a, b) = match kind {
        "random" => (
            random_ascii(len, seed_for(len, r_a)),
            random_ascii(len, seed_for(len, r_b)),
        ),
        "similar" => similar_pair_equal_len(len, 0.05, seed_for(len, sim_or_ident)),
        "identical" => identical_pair(len, seed_for(len, sim_or_ident)),
        _ => unreachable!("unknown similarity regime: {kind}"),
    };
    Pair::from_bytes(a, b)
}

/// Deterministic per-length seed derived from length × golden-ratio
/// constant, xor-ed with a per-bench salt.
///
/// Matches the `seed_for` helper each bench in `comparand-bench` uses,
/// so at any fixed `(len, salt)` this adapter and the toolkit's own
/// suite see the same corpus family.
#[inline]
#[must_use]
pub const fn seed_for(len: usize, salt: u64) -> u64 {
    (len as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt
}

/// The canonical input-length sweep, mirroring `comparand-bench` so
/// per-length data points line up on a chart.
pub const LENGTHS: &[usize] = &[8, 32, 128, 512, 2048];

/// The three similarity regimes, in the order the toolkit's own
/// suite emits them.
pub const REGIMES: &[&str] = &["random", "similar", "identical"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_is_reproducible() {
        let a = random_ascii(64, 0x00C0_FFEE);
        let b = random_ascii(64, 0x00C0_FFEE);
        assert_eq!(a, b);
    }

    #[test]
    fn pair_representations_agree() {
        let p = Pair::from_bytes(random_ascii(32, 1), random_ascii(32, 2));
        assert_eq!(p.a_string.as_bytes(), p.a_bytes.as_slice());
        assert_eq!(p.a_chars.len(), p.a_bytes.len());
        assert!(p.a_string.is_ascii());
    }

    #[test]
    fn identical_regime_is_byte_equal() {
        let p = build_pair(64, "identical", (0x01, 0x02, 0x04));
        assert_eq!(p.a_bytes, p.b_bytes);
        assert_eq!(p.a_string, p.b_string);
    }

    #[test]
    fn similar_regime_diverges() {
        let p = build_pair(128, "similar", (0x01, 0x02, 0x03));
        // 5% edit rate on len=128 is ~6 edits; extremely unlikely to
        // land on a no-op.
        assert_ne!(p.a_bytes, p.b_bytes);
    }

    #[test]
    fn equal_len_stays_equal_length() {
        let p = build_pair_equal_len(64, "similar", (0x11, 0x12, 0x13));
        assert_eq!(p.a_bytes.len(), p.b_bytes.len());
    }
}
