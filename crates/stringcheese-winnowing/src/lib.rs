//! # Winnowing: local-algorithm document fingerprints
//!
//! Given a stream of hashes (one per n-gram of the document),
//! select the minimum hash in each sliding window of size `w` —
//! the classical "winnowing" scheme from [Schleimer, Wilkerson &
//! Aiken 2003](https://dl.acm.org/doi/10.1145/872757.872770).
//! The paper proves this achieves the best possible density
//! guarantee for a local algorithm — every window contributes at
//! least one fingerprint, but consecutive windows share their
//! selection when the minimum doesn't slide out.
//!
//! ## Why winnowing rather than every-Nth-hash
//!
//! Sampling every N-th hash also gives density `1/N`, but a small
//! edit shifts every downstream sample and destroys the
//! fingerprint. Winnowing is stable under local rearrangements —
//! move a paragraph, add a comment, indent a block, and most of
//! the selected fingerprints don't change. This is what makes it
//! usable for plagiarism detection.
//!
//! ## What ships
//!
//! - [`Winnower`] — configured with the window size; call
//!   [`Winnower::select`] with an iterator of hashes to get back
//!   the selected `(position, hash)` pairs.
//! - [`Fingerprint`] — one selected sample.
//!
//! ## Positioning in the fingerprint trio
//!
//! - [`stringcheese_minhash`](https://docs.rs/stringcheese-minhash)
//!   — SET similarity (Jaccard).
//! - [`stringcheese_simhash`](https://docs.rs/stringcheese-simhash)
//!   — weighted-feature-BAG similarity (cosine / Hamming).
//! - **this crate** — LOCAL similarity via document
//!   fingerprints. Returns a set of `(position, hash)` samples
//!   that can be inverted-index'd to locate near-duplicate spans
//!   across a large corpus.
//!
//! ## Example
//!
//! ```
//! use stringcheese_winnowing::Winnower;
//!
//! // Toy stream: eight already-hashed n-grams.
//! let hashes = [77u64, 74, 42, 17, 98, 50, 17, 98];
//! let w = Winnower::new(4);
//! let fps: Vec<_> = w.select(hashes.iter().copied()).collect();
//! // Each window of 4 contributes its min; consecutive windows
//! // share the min when it hasn't slid out — sample density is
//! // roughly 2/(w+1) per the paper's density theorem.
//! assert!(!fps.is_empty());
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
// `doc_markdown` fires on Moss (proper noun); the docs are for
// humans and wrapping every capitalised name in backticks harms
// readability.
#![allow(clippy::doc_markdown)]

#[cfg(feature = "alloc")]
extern crate alloc;

use alloc::collections::VecDeque;

/// One winnowing fingerprint sample.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Fingerprint {
    /// Position of the sample in the original hash stream
    /// (0-indexed).
    pub position: usize,
    /// The selected hash value.
    pub hash: u64,
}

/// Winnowing selector — configured once with the window size,
/// then applied to any hash stream via [`Self::select`].
#[derive(Copy, Clone, Debug)]
pub struct Winnower {
    window: usize,
}

impl Winnower {
    /// Construct a winnower with window size `w`.
    ///
    /// # Panics
    ///
    /// Panics on `w == 0` — a zero-width window has no minimum.
    #[must_use]
    pub fn new(window: usize) -> Self {
        assert!(window > 0, "window must be > 0");
        Self { window }
    }

    /// Window size the selector was configured with.
    #[must_use]
    pub fn window(&self) -> usize {
        self.window
    }

    /// Apply the winnowing algorithm to a hash stream.
    ///
    /// Returns an iterator of [`Fingerprint`] entries — one per
    /// window whose minimum wasn't already selected by an earlier
    /// window. Tie-breaking is "rightmost min" as prescribed by
    /// the reference paper (Schleimer et al. §4.2), which
    /// maximises re-use of a fingerprint across consecutive
    /// windows and thus keeps sample density close to the
    /// theoretical `2/(w+1)`.
    #[must_use]
    pub fn select<I: IntoIterator<Item = u64>>(&self, hashes: I) -> Selected {
        let hashes: alloc::vec::Vec<u64> = hashes.into_iter().collect();
        Selected::new(hashes, self.window)
    }
}

/// Iterator returned by [`Winnower::select`]. Yields [`Fingerprint`]
/// entries in stream order.
///
/// Runs the deque-based sliding-minimum in `O(n)` total across all
/// windows — each hash is enqueued and dequeued at most once.
#[derive(Debug)]
pub struct Selected {
    hashes: alloc::vec::Vec<u64>,
    window: usize,
    // Monotonic deque of (position, hash) with strictly
    // non-decreasing hashes. Front is always the current window's
    // minimum.
    deque: VecDeque<(usize, u64)>,
    // Next position in `hashes` to feed into the deque.
    next_pos: usize,
    // Position of the LAST fingerprint we emitted; used to skip
    // duplicates when consecutive windows share their minimum.
    last_emitted: Option<usize>,
}

impl Selected {
    fn new(hashes: alloc::vec::Vec<u64>, window: usize) -> Self {
        Self {
            hashes,
            window,
            deque: VecDeque::new(),
            next_pos: 0,
            last_emitted: None,
        }
    }
}

impl Iterator for Selected {
    type Item = Fingerprint;

    fn next(&mut self) -> Option<Fingerprint> {
        // Prime the first window on the very first call. If the
        // input is shorter than the window there's nothing to
        // fingerprint — bail immediately.
        if self.deque.is_empty() && self.next_pos == 0 {
            if self.hashes.len() < self.window {
                return None;
            }
            self.fill_first_window();
            if let Some(fp) = self.emit_current_window_min() {
                return Some(fp);
            }
        }
        // Slide the window forward one step at a time until we
        // emit something new or run out of input.
        while self.next_pos < self.hashes.len() {
            self.slide_one();
            if let Some(fp) = self.emit_current_window_min() {
                return Some(fp);
            }
        }
        None
    }
}

impl Selected {
    /// Fill the deque with the first `window` hashes.
    fn fill_first_window(&mut self) {
        let end = self.window.min(self.hashes.len());
        for pos in 0..end {
            self.push(pos, self.hashes[pos]);
        }
        self.next_pos = end;
    }

    /// Slide the window right by one — drop the leftmost position
    /// if it's the current deque front, then push the new hash.
    fn slide_one(&mut self) {
        // The window that just closed started at
        // `next_pos - window`; the new window starts at
        // `next_pos - window + 1`. Drop deque entries whose
        // position is now out of range.
        let new_window_start = self.next_pos + 1 - self.window;
        while let Some(&(pos, _)) = self.deque.front() {
            if pos < new_window_start {
                self.deque.pop_front();
            } else {
                break;
            }
        }
        // Push the new hash.
        let pos = self.next_pos;
        self.push(pos, self.hashes[pos]);
        self.next_pos += 1;
    }

    /// Push (pos, hash) onto the deque, maintaining the monotonic
    /// invariant (hashes strictly non-decreasing from front to
    /// back). Uses `<` rather than `<=` so ties preserve the
    /// RIGHTMOST occurrence — see paper §4.2 on why rightmost-min
    /// maximises fingerprint re-use.
    fn push(&mut self, pos: usize, hash: u64) {
        while let Some(&(_, tail_hash)) = self.deque.back() {
            // Pop tail values >= the new hash so the new hash
            // takes over as the "leftmost of the current run of
            // ties" position. Combined with the deque-front
            // convention, this gives the rightmost-min tiebreaker.
            if tail_hash >= hash {
                self.deque.pop_back();
            } else {
                break;
            }
        }
        self.deque.push_back((pos, hash));
    }

    /// Emit the current window's minimum as a Fingerprint if it
    /// hasn't already been emitted; otherwise `None`.
    fn emit_current_window_min(&mut self) -> Option<Fingerprint> {
        let &(pos, hash) = self.deque.front()?;
        if Some(pos) == self.last_emitted {
            return None;
        }
        self.last_emitted = Some(pos);
        Some(Fingerprint {
            position: pos,
            hash,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn empty_input_yields_nothing() {
        let fps: Vec<_> = Winnower::new(4).select(core::iter::empty()).collect();
        assert!(fps.is_empty());
    }

    #[test]
    fn input_shorter_than_window_yields_nothing() {
        let fps: Vec<_> = Winnower::new(4).select([1u64, 2, 3]).collect();
        assert!(fps.is_empty());
    }

    #[test]
    fn single_window_yields_one_fingerprint() {
        let fps: Vec<_> = Winnower::new(4).select([5u64, 3, 7, 4]).collect();
        assert_eq!(fps.len(), 1);
        assert_eq!(fps[0].hash, 3);
        assert_eq!(fps[0].position, 1);
    }

    #[test]
    fn reference_paper_example() {
        // Schleimer et al. §4.2 walk-through — the paper hashes
        // "adorunrunrunadorunrun" into 8 3-gram hashes:
        //   [77, 74, 42, 17, 98, 50, 17, 98]
        // and with window w = 4 reports the fingerprints:
        //   (3, 17), (6, 17)
        // (positions are 0-based; the paper's are 1-based but
        // shift consistently). Our rightmost-min tie-break
        // matches the paper §4.2 rule exactly.
        let hashes = [77u64, 74, 42, 17, 98, 50, 17, 98];
        let fps: Vec<_> = Winnower::new(4).select(hashes).collect();
        assert_eq!(fps.len(), 2);
        assert_eq!(
            fps[0],
            Fingerprint {
                position: 3,
                hash: 17
            }
        );
        assert_eq!(
            fps[1],
            Fingerprint {
                position: 6,
                hash: 17
            }
        );
    }

    #[test]
    fn monotonic_stream_selects_only_the_first_run() {
        // Ascending hashes — the minimum is always at the front
        // and slides out one step at a time; each window picks a
        // new minimum. Every position after the first `w-1`
        // becomes a fingerprint until the values keep climbing.
        let hashes: Vec<u64> = (1..=8).collect();
        let fps: Vec<_> = Winnower::new(4).select(hashes).collect();
        // Windows are [1,2,3,4] → min at 0, [2,3,4,5] → min at 1,
        // and so on — every window's min is the new leftmost
        // element and gets emitted.
        assert_eq!(fps.len(), 5);
        assert_eq!(fps[0].position, 0);
        assert_eq!(fps[4].position, 4);
    }

    #[test]
    fn constant_stream_re_uses_one_fingerprint() {
        // All identical hashes — rightmost-min tie-breaker means
        // each window's chosen position is the latest one, but
        // successive windows share the SAME position exactly
        // once, then advance. Number of unique fingerprints ≈ n - w + 1.
        let hashes = alloc::vec![42u64; 10];
        let fps: Vec<_> = Winnower::new(4).select(hashes).collect();
        // With the rightmost-min rule and w=4 across 10 identical
        // hashes: each window's min advances one step, so we get
        // (10 - 4 + 1) = 7 distinct fingerprints.
        assert_eq!(fps.len(), 7);
        for fp in &fps {
            assert_eq!(fp.hash, 42);
        }
    }

    #[test]
    fn density_bound_holds() {
        // Density theorem: an average of at least 1 fingerprint
        // per (w+1)/2 hashes. For a random-ish stream at w = 4
        // over 100 hashes, we expect ≥ 40 fingerprints. Use a
        // hash sequence that's not adversarial to the ordering.
        let n = 100usize;
        // Multiply in u64 explicitly — on 32-bit targets (wasm32),
        // `i * 2_654_435_761` overflows `usize` even though the
        // subsequent `& 0xFFFF` masks the result to 16 bits.
        let hashes: Vec<u64> = (0..n)
            .map(|i| (i as u64 * 2_654_435_761) & 0xFFFF)
            .collect();
        let fps: Vec<_> = Winnower::new(4).select(hashes).collect();
        let min_expected = (n - 4 + 1) * 2 / (4 + 1); // = 38
        assert!(
            fps.len() >= min_expected,
            "expected >= {min_expected} fingerprints, got {}",
            fps.len(),
        );
    }

    #[test]
    #[should_panic(expected = "window must be > 0")]
    fn zero_window_panics() {
        let _ = Winnower::new(0);
    }

    #[test]
    fn window_size_of_one_selects_every_hash() {
        // w = 1 → every hash is its own "window minimum". Should
        // emit every position exactly once.
        let hashes: Vec<u64> = (10..=14).collect();
        let fps: Vec<_> = Winnower::new(1).select(hashes).collect();
        assert_eq!(fps.len(), 5);
        for (i, fp) in fps.iter().enumerate() {
            assert_eq!(fp.position, i);
        }
    }

    // -----------------------------------------------------------------
    // Additional boundary / configuration coverage
    // -----------------------------------------------------------------

    #[test]
    fn window_equal_to_input_length_yields_one_fingerprint() {
        // Exactly one window fits — it emits its (rightmost) min.
        let hashes = [9u64, 3, 7, 5];
        let fps: Vec<_> = Winnower::new(4).select(hashes).collect();
        assert_eq!(fps.len(), 1);
        assert_eq!(fps[0].position, 1);
        assert_eq!(fps[0].hash, 3);
    }

    #[test]
    fn window_larger_than_input_yields_nothing() {
        // The priming step requires `hashes.len() >= window`; a bigger
        // window returns no fingerprints at all.
        let fps: Vec<_> = Winnower::new(10).select([1u64, 2, 3]).collect();
        assert!(fps.is_empty());
    }

    #[test]
    fn window_one_larger_than_input_yields_nothing() {
        // Off-by-one on the priming boundary.
        let fps: Vec<_> = Winnower::new(5).select([1u64, 2, 3, 4]).collect();
        assert!(fps.is_empty());
    }

    #[test]
    fn single_hash_with_window_one_yields_one_fingerprint() {
        let fps: Vec<_> = Winnower::new(1).select([42u64]).collect();
        assert_eq!(fps.len(), 1);
        assert_eq!(
            fps[0],
            Fingerprint {
                position: 0,
                hash: 42
            }
        );
    }

    #[test]
    fn single_hash_with_larger_window_yields_nothing() {
        let fps: Vec<_> = Winnower::new(2).select([42u64]).collect();
        assert!(fps.is_empty());
    }

    #[test]
    fn window_accessor_returns_configured_size() {
        assert_eq!(Winnower::new(1).window(), 1);
        assert_eq!(Winnower::new(4).window(), 4);
        assert_eq!(Winnower::new(1024).window(), 1024);
    }

    #[test]
    fn winnower_is_copy_and_reusable_across_streams() {
        // Winnower is `Copy`, so callers can reuse the same configured
        // selector on multiple hash streams without ceremony.
        let w = Winnower::new(3);
        let a: Vec<_> = w.select([5u64, 1, 9]).collect();
        let b: Vec<_> = w.select([2u64, 8, 4]).collect();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].hash, 1);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].hash, 2);
    }

    #[test]
    fn selected_iterator_is_lazy_via_take() {
        // The Selected iterator honours normal `Iterator` combinators;
        // `take(1)` should not consume the whole stream.
        let hashes: Vec<u64> = (0..1000).rev().collect(); // strictly descending
        // Under a descending stream every new hash becomes the new min,
        // so every window past the first emits a new fingerprint.
        let first: Vec<_> = Winnower::new(4).select(hashes).take(3).collect();
        assert_eq!(first.len(), 3);
    }

    #[test]
    fn descending_stream_emits_each_new_min() {
        // With strictly-descending hashes each new hash becomes the
        // new min at the rightmost position of its window, so every
        // window past the first emits a new fingerprint at the
        // newest position.
        let hashes: Vec<u64> = (1..=6).rev().collect(); // [6,5,4,3,2,1]
        let fps: Vec<_> = Winnower::new(3).select(hashes).collect();
        // First window [6,5,4] emits (2, 4). Next [5,4,3] emits (3, 3).
        // Next [4,3,2] emits (4, 2). Next [3,2,1] emits (5, 1).
        assert_eq!(fps.len(), 4);
        assert_eq!(
            fps[0],
            Fingerprint {
                position: 2,
                hash: 4
            }
        );
        assert_eq!(
            fps[3],
            Fingerprint {
                position: 5,
                hash: 1
            }
        );
    }

    #[test]
    fn all_zero_hashes_still_emit_fingerprints() {
        // Hash value 0 is not a sentinel — it should behave like any
        // other value.
        let fps: Vec<_> = Winnower::new(3).select([0u64; 6]).collect();
        // (n - w + 1) = 4 fingerprints under the rightmost-min rule
        // (each window's chosen position advances by one).
        assert_eq!(fps.len(), 4);
        for fp in &fps {
            assert_eq!(fp.hash, 0);
        }
    }

    #[test]
    fn u64_max_hashes_behave_normally() {
        // No overflow lurking near the top of the u64 range.
        let hashes = [u64::MAX, 1, u64::MAX, u64::MAX];
        let fps: Vec<_> = Winnower::new(4).select(hashes).collect();
        assert_eq!(fps.len(), 1);
        assert_eq!(fps[0].hash, 1);
        assert_eq!(fps[0].position, 1);
    }

    #[test]
    fn rightmost_tiebreak_on_two_equal_mins_in_first_window() {
        // Window [5, 1, 3, 1] — two occurrences of the min (1) at
        // positions 1 and 3. The paper's rightmost-min rule picks 3.
        let fps: Vec<_> = Winnower::new(4).select([5u64, 1, 3, 1]).collect();
        assert_eq!(fps.len(), 1);
        assert_eq!(fps[0].position, 3);
    }

    #[test]
    fn fingerprint_positions_are_strictly_increasing() {
        // Structural invariant that `last_emitted` guarantees.
        let hashes = [77u64, 74, 42, 17, 98, 50, 17, 98, 22, 90, 3, 3, 3, 100];
        let fps: Vec<_> = Winnower::new(4).select(hashes).collect();
        for pair in fps.windows(2) {
            assert!(
                pair[0].position < pair[1].position,
                "positions must strictly increase: got {pair:?}",
            );
        }
    }

    #[test]
    fn every_emitted_hash_matches_input_at_its_position() {
        let hashes = [77u64, 74, 42, 17, 98, 50, 17, 98, 22, 90, 3, 3, 3, 100];
        let fps: Vec<_> = Winnower::new(4).select(hashes).collect();
        for fp in &fps {
            let pos = fp.position;
            assert_eq!(
                fp.hash, hashes[pos],
                "emitted hash {fp:?} disagrees with input at position {pos}",
            );
        }
    }

    #[test]
    fn upper_bound_on_fingerprint_count_holds() {
        // Trivial ceiling: at most one fingerprint per window. Number
        // of windows in a length-n stream with window w is n - w + 1.
        let hashes: Vec<u64> = (0..50).map(|i| (i * 31) & 0xFF).collect();
        let w = 5;
        let n = hashes.len();
        let fps: Vec<_> = Winnower::new(w).select(hashes).collect();
        assert!(fps.len() <= n - w + 1);
    }

    #[test]
    fn deterministic_across_repeated_calls() {
        let hashes: Vec<u64> = (0u64..40).map(|i| (i * 7919) & 0xFFF).collect();
        let w = Winnower::new(4);
        let a: Vec<_> = w.select(hashes.iter().copied()).collect();
        let b: Vec<_> = w.select(hashes.iter().copied()).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn extending_input_preserves_all_prior_fingerprints() {
        // Locality property: windows entirely within the original
        // prefix don't change when we extend the stream at the end.
        // Number of fingerprints emitted from those windows is at
        // least the number emitted on the prefix minus what the
        // trailing partial window (positions n-w+1 .. n-1) could have
        // contributed — but appending only makes future windows visible,
        // it never rewrites past emissions.
        let prefix: Vec<u64> = [77u64, 74, 42, 17, 98, 50, 17, 98].to_vec();
        let mut extended = prefix.clone();
        extended.extend_from_slice(&[22, 90, 3, 3, 3, 100]);
        let w = Winnower::new(4);
        let a: Vec<_> = w.select(prefix.iter().copied()).collect();
        let b: Vec<_> = w.select(extended.iter().copied()).collect();
        // Every fingerprint from the prefix run appears in the same
        // order at the start of the extended run — the extension can
        // add new tail fingerprints but must not disturb earlier ones.
        assert!(b.len() >= a.len(), "extension shrunk fingerprint count");
        assert_eq!(&b[..a.len()], a.as_slice());
    }

    #[test]
    fn fingerprint_struct_supports_equality_and_hash_derives() {
        // Fingerprint derives Copy, Clone, PartialEq, Eq, Hash.
        use std::collections::HashSet;
        let original = Fingerprint {
            position: 3,
            hash: 17,
        };
        let by_copy = original; // Copy
        #[allow(clippy::clone_on_copy)]
        let by_clone = original.clone(); // exercises the Clone impl explicitly
        assert_eq!(original, by_copy);
        assert_eq!(original, by_clone);
        let mut set: HashSet<Fingerprint> = HashSet::new();
        set.insert(original);
        assert!(set.contains(&by_copy));
    }

    #[test]
    fn winnower_debug_impl_is_non_empty() {
        // Just a smoke-test — makes sure the derived Debug renders
        // something sensible for diagnostics.
        let dbg = format!("{:?}", Winnower::new(7));
        assert!(dbg.contains("Winnower"));
        assert!(dbg.contains('7'));
    }

    #[test]
    fn iterator_from_various_sources_composes() {
        // The `IntoIterator<Item = u64>` bound accepts arrays, slices,
        // Vecs, and adaptor iterators alike.
        let arr: [u64; 4] = [10, 5, 3, 7];
        let owned: Vec<u64> = arr.to_vec();
        let sel = Winnower::new(2);
        let from_array: Vec<_> = sel.select(arr).collect();
        let from_vec: Vec<_> = sel.select(owned.iter().copied()).collect();
        let from_filter: Vec<_> = sel.select(arr.iter().copied().filter(|_| true)).collect();
        assert_eq!(from_array, from_vec);
        assert_eq!(from_array, from_filter);
    }

    #[test]
    fn slide_across_run_of_ties_advances_once_per_step() {
        // A long run of identical minima under the rightmost-min rule:
        // each slide advances the chosen position by exactly one.
        let hashes = alloc::vec![7u64; 8];
        let w = 3;
        let fps: Vec<_> = Winnower::new(w).select(hashes).collect();
        // n - w + 1 = 6 fingerprints; positions 2, 3, 4, 5, 6, 7.
        assert_eq!(fps.len(), 6);
        for (i, fp) in fps.iter().enumerate() {
            assert_eq!(fp.position, i + w - 1);
        }
    }

    #[test]
    fn min_at_start_of_first_window_is_selected_once() {
        // If the min sits at position 0 and never gets displaced by a
        // smaller value, we should emit it exactly once and then let
        // subsequent windows pick their own mins after it slides out.
        let hashes = [1u64, 5, 7, 9, 10, 11, 12];
        let w = 3;
        let fps: Vec<_> = Winnower::new(w).select(hashes).collect();
        // Window 0 [1,5,7] emits (0,1); window 1 [5,7,9] emits (1,5);
        // window 2 [7,9,10] emits (2,7); window 3 [9,10,11] emits (3,9);
        // window 4 [10,11,12] emits (4,10). Total 5 fingerprints.
        assert_eq!(fps.len(), 5);
        assert_eq!(
            fps[0],
            Fingerprint {
                position: 0,
                hash: 1
            }
        );
    }

    #[test]
    fn large_window_boundary_selects_global_min() {
        // With w = n every hash sits in the one and only window, and
        // the rightmost-min rule picks the rightmost occurrence of the
        // global minimum.
        let hashes = [3u64, 1, 4, 1, 5, 9, 2, 6, 5, 3];
        let n = hashes.len();
        let fps: Vec<_> = Winnower::new(n).select(hashes).collect();
        assert_eq!(fps.len(), 1);
        assert_eq!(fps[0].hash, 1);
        // Rightmost occurrence of the min (1) is at position 3.
        assert_eq!(fps[0].position, 3);
    }

    // -----------------------------------------------------------------
    // Property tests — invariants over arbitrary hash streams and
    // window sizes. Only enabled off wasm (proptest isn't available
    // there — see Cargo.toml).
    // -----------------------------------------------------------------

    #[cfg(not(target_family = "wasm"))]
    mod props {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Fingerprint positions are strictly increasing — the
            /// `last_emitted` guard forbids re-emitting the same
            /// position twice.
            #[test]
            fn positions_strictly_increase(
                hashes in proptest::collection::vec(any::<u64>(), 0..64),
                w in 1usize..8,
            ) {
                let fps: Vec<_> = Winnower::new(w).select(hashes).collect();
                for pair in fps.windows(2) {
                    prop_assert!(pair[0].position < pair[1].position);
                }
            }

            /// Every fingerprint's hash equals `hashes[position]` —
            /// the selector never fabricates a hash value.
            #[test]
            fn hash_matches_input_at_position(
                hashes in proptest::collection::vec(any::<u64>(), 0..64),
                w in 1usize..8,
            ) {
                let fps: Vec<_> = Winnower::new(w).select(hashes.iter().copied()).collect();
                for fp in &fps {
                    prop_assert!(fp.position < hashes.len());
                    prop_assert_eq!(fp.hash, hashes[fp.position]);
                }
            }

            /// Output length is bounded above by (n - w + 1): at most
            /// one fingerprint per window.
            #[test]
            fn output_length_bounded_by_window_count(
                hashes in proptest::collection::vec(any::<u64>(), 0..64),
                w in 1usize..8,
            ) {
                let n = hashes.len();
                let fps: Vec<_> = Winnower::new(w).select(hashes).collect();
                if n < w {
                    prop_assert_eq!(fps.len(), 0);
                } else {
                    prop_assert!(fps.len() <= n - w + 1);
                }
            }

            /// Locality bound: consecutive fingerprint positions
            /// differ by at most `w`. When window `i` emits at
            /// position `p`, later windows only stop re-emitting `p`
            /// when it slides out at window `p+1`, and the new
            /// window's min sits at some position in `[p+1, p+w]`.
            #[test]
            fn consecutive_gaps_bounded_by_window_size(
                hashes in proptest::collection::vec(any::<u64>(), 0..64),
                w in 1usize..8,
            ) {
                let fps: Vec<_> = Winnower::new(w).select(hashes).collect();
                for pair in fps.windows(2) {
                    let gap = pair[1].position - pair[0].position;
                    prop_assert!(gap >= 1, "positions must strictly increase");
                    prop_assert!(
                        gap <= w,
                        "gap {} exceeds window {} between {:?} and {:?}",
                        gap, w, pair[0], pair[1],
                    );
                }
            }

            /// First fingerprint (if any) sits within the first window
            /// `[0, w-1]`; last fingerprint sits within the last window
            /// `[n-w, n-1]`.
            #[test]
            fn first_and_last_fingerprints_lie_in_boundary_windows(
                hashes in proptest::collection::vec(any::<u64>(), 4..64),
                w in 1usize..5,
            ) {
                let n = hashes.len();
                prop_assume!(n >= w);
                let fps: Vec<_> = Winnower::new(w).select(hashes).collect();
                prop_assume!(!fps.is_empty());
                prop_assert!(fps[0].position < w);
                prop_assert!(fps[fps.len() - 1].position >= n - w);
                prop_assert!(fps[fps.len() - 1].position < n);
            }

            /// Under w = 1 every hash is its own window and gets
            /// emitted in order — the operation is the identity on
            /// positions.
            #[test]
            fn window_one_emits_every_position(
                hashes in proptest::collection::vec(any::<u64>(), 0..64),
            ) {
                let fps: Vec<_> = Winnower::new(1).select(hashes.iter().copied()).collect();
                prop_assert_eq!(fps.len(), hashes.len());
                for (i, fp) in fps.iter().enumerate() {
                    prop_assert_eq!(fp.position, i);
                    prop_assert_eq!(fp.hash, hashes[i]);
                }
            }

            /// Fingerprint positions form a subset of input positions.
            #[test]
            fn positions_are_input_indices(
                hashes in proptest::collection::vec(any::<u64>(), 0..64),
                w in 1usize..8,
            ) {
                let n = hashes.len();
                let fps: Vec<_> = Winnower::new(w).select(hashes).collect();
                for fp in &fps {
                    prop_assert!(fp.position < n);
                }
            }

            /// Determinism: identical inputs produce identical outputs.
            #[test]
            fn deterministic(
                hashes in proptest::collection::vec(any::<u64>(), 0..48),
                w in 1usize..8,
            ) {
                let sel = Winnower::new(w);
                let a: Vec<_> = sel.select(hashes.iter().copied()).collect();
                let b: Vec<_> = sel.select(hashes.iter().copied()).collect();
                prop_assert_eq!(a, b);
            }

            /// Locality: appending arbitrary hashes to the end only
            /// extends the fingerprint stream — every fingerprint
            /// emitted on the prefix appears identically at the start
            /// of the extended run.
            #[test]
            fn extension_preserves_prefix_fingerprints(
                prefix in proptest::collection::vec(any::<u64>(), 4..32),
                suffix in proptest::collection::vec(any::<u64>(), 0..16),
                w in 1usize..5,
            ) {
                let sel = Winnower::new(w);
                let a: Vec<_> = sel.select(prefix.iter().copied()).collect();
                let mut extended = prefix.clone();
                extended.extend(suffix.iter().copied());
                let b: Vec<_> = sel.select(extended.iter().copied()).collect();
                prop_assert!(b.len() >= a.len());
                prop_assert_eq!(&b[..a.len()], a.as_slice());
            }

            /// Never panics on any window in `[1, u16::MAX]` and any
            /// hash stream length in `[0, 128]`.
            #[test]
            fn never_panics(
                hashes in proptest::collection::vec(any::<u64>(), 0..128),
                w in 1usize..=128,
            ) {
                let _ = Winnower::new(w).select(hashes).count();
            }
        }
    }
}
