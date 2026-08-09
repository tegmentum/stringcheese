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
    // Pending fingerprint emitted from the priming phase but not
    // yet returned by `next()`.
    pending: Option<Fingerprint>,
}

impl Selected {
    fn new(hashes: alloc::vec::Vec<u64>, window: usize) -> Self {
        Self {
            hashes,
            window,
            deque: VecDeque::new(),
            next_pos: 0,
            last_emitted: None,
            pending: None,
        }
    }
}

impl Iterator for Selected {
    type Item = Fingerprint;

    fn next(&mut self) -> Option<Fingerprint> {
        if let Some(fp) = self.pending.take() {
            return Some(fp);
        }
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
        // shift consistently). Our leftmost-min-of-window with
        // rightmost-tiebreak matches exactly.
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
        let hashes: Vec<u64> = (0..n).map(|i| (i as u64 * 2_654_435_761) & 0xFFFF).collect();
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
}
