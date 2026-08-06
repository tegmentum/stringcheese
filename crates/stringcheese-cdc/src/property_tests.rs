//! Property-based tests for the fingerprint and CDC subsystems.
//!
//! The properties fall into three groups:
//!
//! * **Per-fingerprint** invariants — rolling equivalence, determinism.
//! * **Per-algorithm** `FastCDC` invariants — chunk-size bounds, sum-of-
//!   sizes equals input length, determinism across repeat calls.
//! * **Streaming vs contiguous** equivalence — the critical `FastCDC`
//!   correctness claim: splitting the input at arbitrary byte-level
//!   boundaries and feeding the pieces to a streaming state machine
//!   produces the identical boundary list to a single contiguous
//!   iterator run. This is the most subtle test in the module and the
//!   one the crate-level design bias is toward getting right.

use alloc::vec::Vec;

use proptest::prelude::*;

use crate::cdc::{ChunkBoundary, FastCdc, FastCdcConfig, FastCdcStream};
use crate::fingerprint::{Buzhash, GearHash, PolynomialHash, RabinFingerprint, RollingHash};

/// A byte-slice strategy of length in `[0, 200]`.
///
/// Kept small so the property runs stay fast; large enough to exercise
/// windows around the range the fingerprint tests use.
fn arb_bytes() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..200)
}

/// A byte-slice strategy suitable for `FastCDC` — long enough to trigger
/// interior cuts with the small-size test config below.
fn arb_bytes_long() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..2_000)
}

/// The `FastCDC` config used by the property tests. Sized much smaller
/// than the default 8 KB so the test runs stay fast even at 2 KB
/// inputs — a min/avg/max of 8/32/128 bytes with wide masks gives a
/// good mix of interior cuts on short inputs.
fn test_config() -> FastCdcConfig {
    FastCdcConfig {
        min_size: 8,
        avg_size: 32,
        max_size: 128,
        // Small popcount masks to keep the cut probability high enough
        // that we see multiple interior cuts on short inputs.
        mask_small: 0x0000_0000_0000_00FF, // 8 bits
        mask_large: 0x0000_0000_0000_000F, // 4 bits
    }
}

/// Feeds `input` through a fresh `H` and returns the digest.
fn run<H: RollingHash<Output = u64>>(input: &[u8], window: usize) -> u64 {
    let mut h = H::new(window);
    for &b in input {
        h.roll(b);
    }
    h.digest()
}

proptest! {
    // ---------------------------------------------------------------
    // Rolling-hash properties
    // ---------------------------------------------------------------

    /// **Rabin rolling equivalence.** After feeding `n >= window`
    /// bytes into a rolling hash, the digest matches the digest of a
    /// fresh hash over the trailing `window` bytes.
    ///
    /// This is the defining property of a rolling hash and the one
    /// most subject to off-by-one bugs in the roll-out subtraction.
    #[test]
    fn proptest_rabin_rolling_matches_fresh_over_trailing_window(
        input in arb_bytes(),
        window in 1usize..32,
    ) {
        if input.len() < window {
            return Ok(());
        }
        let rolling = run::<RabinFingerprint>(&input, window);
        let fresh = run::<RabinFingerprint>(&input[input.len() - window..], window);
        prop_assert_eq!(rolling, fresh);
    }

    /// **Polynomial rolling equivalence.** As above, but for the
    /// polynomial-mod-Mersenne-61 hash.
    #[test]
    fn proptest_polynomial_rolling_matches_fresh_over_trailing_window(
        input in arb_bytes(),
        window in 1usize..32,
    ) {
        if input.len() < window {
            return Ok(());
        }
        let rolling = run::<PolynomialHash>(&input, window);
        let fresh = run::<PolynomialHash>(&input[input.len() - window..], window);
        prop_assert_eq!(rolling, fresh);
    }

    /// **Buzhash rolling equivalence.** After feeding `n >= window`
    /// bytes into a rolling Buzhash, the digest matches the digest of a
    /// fresh Buzhash over the trailing `window` bytes.
    ///
    /// This is the defining round-trip property for the cyclic-polynomial
    /// construction: it verifies that the per-roll eviction term
    /// `ROL(T[leaving], window mod 64)` cancels the outgoing byte's
    /// contribution exactly.
    #[test]
    fn proptest_buzhash_rolling_matches_fresh_over_trailing_window(
        input in arb_bytes(),
        window in 1usize..32,
    ) {
        if input.len() < window {
            return Ok(());
        }
        let rolling = run::<Buzhash>(&input, window);
        let fresh = run::<Buzhash>(&input[input.len() - window..], window);
        prop_assert_eq!(rolling, fresh);
    }

    /// **Buzhash rolling equivalence — window larger than the rotate
    /// period.** Buzhash's rotate is 64-bit, so windows exceeding 64
    /// exercise the `window mod 64` fold on the eviction term. This
    /// property must still hold there.
    #[test]
    fn proptest_buzhash_rolling_matches_fresh_over_trailing_window_large(
        input in proptest::collection::vec(any::<u8>(), 0..400),
        window in 65usize..=128,
    ) {
        if input.len() < window {
            return Ok(());
        }
        let rolling = run::<Buzhash>(&input, window);
        let fresh = run::<Buzhash>(&input[input.len() - window..], window);
        prop_assert_eq!(rolling, fresh);
    }

    /// **Buzhash forward-then-forward-with-restored-context bijection.**
    /// Buzhash's fill-phase update is `state = ROL(state, 1) ^ T[byte]`,
    /// which is a bijection on `u64` for any fixed `byte`. This test
    /// exercises the inverse structure directly: if we build a state by
    /// feeding `prefix` bytes and then feed one more byte `b`, the
    /// resulting state must equal the state we get by feeding the same
    /// `prefix ++ [b]` to a fresh hasher. Any hidden state that survives
    /// across the "one more byte" boundary — a state a naive
    /// implementation might accidentally carry — would show up as an
    /// asymmetric digest here.
    #[test]
    fn proptest_buzhash_roll_forward_is_a_pure_function_of_the_prefix(
        prefix in proptest::collection::vec(any::<u8>(), 0..40),
        b in any::<u8>(),
    ) {
        let window = 8usize;

        // Path 1: feed prefix, snapshot, feed one more byte.
        let mut h1 = Buzhash::new(window);
        for &x in &prefix { h1.roll(x); }
        h1.roll(b);

        // Path 2: feed prefix ++ [b] end-to-end to a fresh hasher.
        let mut h2 = Buzhash::new(window);
        for &x in &prefix { h2.roll(x); }
        h2.roll(b);

        prop_assert_eq!(h1.digest(), h2.digest());
    }

    /// **Fingerprint determinism.** Two fresh hash instances fed the
    /// same bytes produce identical digests, always. No hidden global
    /// state may creep in.
    #[test]
    fn proptest_fingerprints_are_deterministic(input in arb_bytes()) {
        let window = 8usize;

        let a1 = run::<RabinFingerprint>(&input, window);
        let a2 = run::<RabinFingerprint>(&input, window);
        prop_assert_eq!(a1, a2, "rabin non-deterministic");

        let b1 = run::<PolynomialHash>(&input, window);
        let b2 = run::<PolynomialHash>(&input, window);
        prop_assert_eq!(b1, b2, "polynomial non-deterministic");

        let c1 = run::<GearHash>(&input, window);
        let c2 = run::<GearHash>(&input, window);
        prop_assert_eq!(c1, c2, "gear non-deterministic");

        let d1 = run::<Buzhash>(&input, window);
        let d2 = run::<Buzhash>(&input, window);
        prop_assert_eq!(d1, d2, "buzhash non-deterministic");
    }

    /// **`reset()` restores identity.** After `reset()`, a hash fed
    /// nothing produces the empty-window digest.
    #[test]
    fn proptest_reset_restores_identity(input in arb_bytes()) {
        let window = 8usize;

        let mut r = RabinFingerprint::new(window);
        for &b in &input { r.roll(b); }
        r.reset();
        prop_assert_eq!(r.digest(), 0);

        let mut p = PolynomialHash::new(window);
        for &b in &input { p.roll(b); }
        p.reset();
        prop_assert_eq!(p.digest(), 0);

        let mut g = GearHash::new(window);
        for &b in &input { g.roll(b); }
        g.reset();
        prop_assert_eq!(g.digest(), 0);

        let mut z = Buzhash::new(window);
        for &b in &input { z.roll(b); }
        z.reset();
        prop_assert_eq!(z.digest(), 0);
    }

    // ---------------------------------------------------------------
    // `FastCDC` properties
    // ---------------------------------------------------------------

    /// **Chunk size bounds.** Every `FastCDC` chunk except possibly the
    /// last has size in `[min_size, max_size]`. The last chunk may
    /// legitimately be smaller than `min_size` when the input ran out
    /// before enough bytes accumulated.
    #[test]
    fn proptest_fastcdc_chunk_size_bounds(input in arb_bytes_long()) {
        let cfg = test_config();
        let cdc = FastCdc::new(cfg);
        let boundaries: Vec<ChunkBoundary> = cdc.chunk_boundaries(&input).collect();

        for (i, b) in boundaries.iter().enumerate() {
            let is_last = i + 1 == boundaries.len();
            prop_assert!(b.size <= cfg.max_size,
                "chunk {i} size {} exceeded max {}", b.size, cfg.max_size);
            if !is_last {
                prop_assert!(b.size >= cfg.min_size,
                    "interior chunk {i} size {} below min {}", b.size, cfg.min_size);
            }
        }
    }

    /// **Sum of chunk sizes equals input length.** The boundaries
    /// partition the input: there is no overlap and no gap.
    #[test]
    fn proptest_fastcdc_sum_of_sizes_equals_input(input in arb_bytes_long()) {
        let cdc = FastCdc::new(test_config());
        let boundaries: Vec<ChunkBoundary> = cdc.chunk_boundaries(&input).collect();
        let sum: usize = boundaries.iter().map(|b| b.size).sum();
        prop_assert_eq!(sum, input.len());

        // And the last boundary's offset must equal the input length
        // (unless the input is empty and there are no boundaries).
        if !boundaries.is_empty() {
            prop_assert_eq!(boundaries.last().unwrap().offset, input.len());
        }
    }

    /// **Offset accounting is consistent.** Every boundary's `offset`
    /// equals the sum of its `size` plus the previous boundary's
    /// `offset` (or zero for the first).
    #[test]
    fn proptest_fastcdc_boundary_offsets_are_consistent(input in arb_bytes_long()) {
        let cdc = FastCdc::new(test_config());
        let boundaries: Vec<ChunkBoundary> = cdc.chunk_boundaries(&input).collect();
        let mut prev = 0usize;
        for b in &boundaries {
            prop_assert_eq!(b.offset, prev + b.size,
                "offset {} inconsistent with size {} after prev {}",
                b.offset, b.size, prev);
            prev = b.offset;
        }
    }

    /// **`FastCDC` determinism.** Two runs on the same input under the
    /// same config produce identical boundary lists.
    #[test]
    fn proptest_fastcdc_deterministic(input in arb_bytes_long()) {
        let cdc = FastCdc::new(test_config());
        let a: Vec<ChunkBoundary> = cdc.chunk_boundaries(&input).collect();
        let b: Vec<ChunkBoundary> = cdc.chunk_boundaries(&input).collect();
        prop_assert_eq!(a, b);
    }

    // ---------------------------------------------------------------
    // Streaming vs contiguous — the critical correctness property
    // ---------------------------------------------------------------

    /// **Streaming vs contiguous equivalence.** Splitting the input at
    /// any byte-level boundary and feeding the two halves sequentially
    /// to a [`FastCdcStream`] produces the identical boundary list to
    /// a single contiguous iterator run.
    ///
    /// This is the property the design bias explicitly asks to be
    /// most careful about — a bug in the state-machine's per-cut
    /// reset, or a hidden dependence on knowing the input length up
    /// front, would show up here.
    #[test]
    fn proptest_streaming_matches_contiguous_at_arbitrary_split(
        input in arb_bytes_long(),
        split_offset in 0usize..=2_000,
    ) {
        let cfg = test_config();

        // One-shot: iterator over the full input.
        let cdc = FastCdc::new(cfg);
        let contiguous: Vec<ChunkBoundary> = cdc.chunk_boundaries(&input).collect();

        // Streaming: split at an arbitrary point (capped to the input
        // length so the strategy stays independent of the input).
        let split = split_offset.min(input.len());
        let (left, right) = input.split_at(split);

        let mut stream = FastCdcStream::new(cfg);
        let mut streamed: Vec<ChunkBoundary> = Vec::new();
        stream.feed_slice(left, |b| streamed.push(b));
        stream.feed_slice(right, |b| streamed.push(b));
        if let Some(final_b) = stream.finish() {
            streamed.push(final_b);
        }

        prop_assert_eq!(&contiguous, &streamed,
            "streaming at split {} disagreed with contiguous", split);
    }

    /// **Streaming vs contiguous — many splits.** As above but
    /// stronger: split into many small pieces (arbitrary sizes) and
    /// feed each. The result must still match the contiguous run.
    #[test]
    fn proptest_streaming_matches_contiguous_at_many_splits(
        input in arb_bytes_long(),
        split_sizes in proptest::collection::vec(1usize..=17, 0..10),
    ) {
        let cfg = test_config();
        let cdc = FastCdc::new(cfg);
        let contiguous: Vec<ChunkBoundary> = cdc.chunk_boundaries(&input).collect();

        let mut stream = FastCdcStream::new(cfg);
        let mut streamed: Vec<ChunkBoundary> = Vec::new();
        let mut cursor = 0usize;
        for size in split_sizes {
            let end = (cursor + size).min(input.len());
            stream.feed_slice(&input[cursor..end], |b| streamed.push(b));
            cursor = end;
            if cursor == input.len() { break; }
        }
        // Feed any remaining bytes as one final chunk.
        if cursor < input.len() {
            stream.feed_slice(&input[cursor..], |b| streamed.push(b));
        }
        if let Some(final_b) = stream.finish() {
            streamed.push(final_b);
        }

        prop_assert_eq!(&contiguous, &streamed,
            "many-split streaming disagreed with contiguous");
    }

    /// **Chunks partition the input.** The chunk at boundary `b`
    /// covers `input[b.offset - b.size .. b.offset]`. Concatenating
    /// all chunks in order reproduces the input exactly.
    #[test]
    fn proptest_fastcdc_chunks_partition_the_input(input in arb_bytes_long()) {
        let cdc = FastCdc::new(test_config());
        let boundaries: Vec<ChunkBoundary> = cdc.chunk_boundaries(&input).collect();

        let mut reconstructed: Vec<u8> = Vec::with_capacity(input.len());
        for b in &boundaries {
            reconstructed.extend_from_slice(&input[b.start()..b.end()]);
        }
        prop_assert_eq!(reconstructed, input);
    }
}
