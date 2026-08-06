//! Streaming state-machine wrappers for the algorithms that admit them.
//!
//! # Motivation
//!
//! [`SinglePatternSearch`](crate::api::SinglePatternSearch) expects the
//! entire haystack in a single `&[u8]`. That is convenient for in-memory
//! data but excludes any workflow where bytes arrive in pieces — network
//! reads, file streams, generated data. The [`StreamingSearch`] trait in
//! this module wraps each algorithm that supports it as a byte-at-a-time
//! state machine that yields matches as they arise.
//!
//! # Which algorithms stream
//!
//! * [`crate::Kmp`] — naturally streaming; the only state is the current
//!   match-length counter plus the total bytes fed.
//! * [`crate::AhoCorasick`] — naturally streaming; the state is the
//!   current automaton state plus the total bytes fed.
//! * [`crate::RabinKarp`] — naturally streaming; the rolling hash *is*
//!   the state, plus a ring buffer of the current window for the
//!   verification step (a hash match must be checked against the actual
//!   window bytes).
//!
//! # Which do not
//!
//! * Boyer-Moore ([`crate::BoyerMoore`], [`crate::BoyerMooreFull`]),
//!   Horspool ([`crate::Horspool`]), and Two-way ([`crate::TwoWay`]) all
//!   scan within a window in a direction that requires the whole window
//!   in memory. A "streaming Boyer-Moore" is possible with an internal
//!   ring buffer sized to the pattern length, but the memory floor
//!   defeats much of the point of "streaming"; and correctness would
//!   have to be established relative to the batch implementation
//!   window-by-window rather than byte-by-byte. This crate deliberately
//!   omits streaming wrappers for these algorithms so the streaming
//!   surface reflects the algorithms that are *natively* streaming; use
//!   the batch [`SinglePatternSearch`] API instead.
//!
//! # Equivalence to batch API
//!
//! The crate-internal property suite pins the invariant
//! `feed_slice(all_bytes) == find_all(all_bytes)` for every streaming
//! algorithm. A related property pins split-invariance:
//! `feed_slice(prefix); feed_slice(suffix)` returns the same matches as
//! `feed_slice(prefix ⊕ suffix)`.
//!
//! # Empty patterns
//!
//! An empty pattern matches at position `0` exactly once (see the
//! crate-wide policy documented on [`crate::api`]). For streaming, that
//! single match is emitted by [`SearchStream::feed_slice`] on the very
//! first call (before any bytes are consumed) and never by
//! [`SearchStream::feed`], because `feed` is byte-driven and the empty
//! pattern's match does not correspond to any byte.
//!
//! [`SinglePatternSearch`]: crate::api::SinglePatternSearch

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::aho_corasick::AhoCorasick;
use crate::api::Match;
use crate::kmp::{Kmp, KmpPrepared};
use crate::rabin_karp::{RabinKarp, RabinKarpPrepared};

/// Rabin-Karp modulus — kept in sync with the batch module's constant.
///
/// Consolidation into a shared `stringcheese-fingerprint` crate is planned;
/// see the TODO in `rabin_karp.rs`.
const RK_MODULUS: u64 = (1u64 << 61) - 1;

/// Rabin-Karp polynomial base — kept in sync with the batch module's
/// constant.
const RK_BASE: u64 = 257;

/// A search algorithm that can be driven as a streaming state machine.
///
/// Wraps an algorithm's [`SearchAlgorithm::Prepared`] state into a
/// [`SearchStream`] that consumes bytes incrementally and yields matches
/// as they occur.
///
/// This trait is separate from
/// [`SinglePatternSearch`](crate::api::SinglePatternSearch) because not
/// every algorithm admits a streaming form — Boyer-Moore, Horspool, and
/// Two-way scan within a window that must be entirely in memory. See the
/// module documentation for the full split.
///
/// [`SearchAlgorithm::Prepared`]: crate::api::SearchAlgorithm::Prepared
pub trait StreamingSearch {
    /// The preprocessed pattern state feeding the stream.
    ///
    /// Identical to the algorithm's
    /// [`SearchAlgorithm::Prepared`](crate::api::SearchAlgorithm::Prepared);
    /// naming it separately here means callers can implement
    /// [`StreamingSearch`] without also implementing
    /// [`SearchAlgorithm`](crate::api::SearchAlgorithm) if some future
    /// algorithm only supports streaming.
    type Prepared;

    /// The streaming state machine tied to a borrow of `Prepared`.
    ///
    /// The associated lifetime lets a stream borrow tables from the
    /// prepared state (Aho-Corasick's automaton, KMP's failure function,
    /// Rabin-Karp's pattern hash and modular factor) rather than
    /// cloning them.
    type Stream<'a>: SearchStream + 'a
    where
        Self: 'a,
        Self::Prepared: 'a;

    /// Constructs a fresh stream over the given prepared state.
    ///
    /// Streams start at byte position `0` with no bytes fed. Reusing a
    /// prepared state across multiple streams is the intended usage
    /// pattern — the state is immutable once built.
    fn stream(prepared: &Self::Prepared) -> Self::Stream<'_>;
}

/// A running streaming search over bytes fed one at a time.
///
/// Yields matches as they arise via [`feed`](SearchStream::feed) or
/// batched via [`feed_slice`](SearchStream::feed_slice). Match positions
/// are in the coordinate system of the *total* byte stream seen so far,
/// not the current call's slice.
pub trait SearchStream {
    /// Feeds a single byte into the stream. Returns `Some(Match)` iff a
    /// match ends at the byte just fed.
    ///
    /// For algorithms that can report multiple matches ending at the
    /// same byte (e.g., Aho-Corasick with several patterns), only one is
    /// returned by `feed`; use [`feed_slice`](SearchStream::feed_slice)
    /// for the full batched surface. The single-pattern streams in this
    /// module never report more than one match per byte.
    ///
    /// Empty-pattern matches are **not** returned by this method — see
    /// the module-level "Empty patterns" note.
    fn feed(&mut self, byte: u8) -> Option<Match>;

    /// Feeds many bytes and returns every match that ended within this
    /// batch, in ascending order by end position.
    ///
    /// Splitting a stream's input into any number of `feed_slice` calls
    /// yields the same match sequence as a single call with the
    /// concatenated bytes — the split-invariance property tested in the
    /// property suite.
    ///
    /// On the very first call, if the pattern is empty, the returned
    /// vector begins with a `Match` at position `0` (per the crate-wide
    /// empty-pattern policy). Subsequent calls do not repeat that
    /// initial match.
    fn feed_slice(&mut self, bytes: &[u8]) -> Vec<Match>;

    /// Returns the total number of bytes fed to this stream so far.
    fn bytes_fed(&self) -> u64;
}

// ---------------------------------------------------------------------------
// KMP streaming.
// ---------------------------------------------------------------------------

/// KMP streaming state.
///
/// See [`crate::Kmp`] for the batch algorithm. The streaming form keeps
/// only the current match-length counter `j` plus the total bytes fed;
/// no ring buffer of past bytes is needed because KMP never re-reads a
/// haystack byte.
#[derive(Debug)]
pub struct KmpStream<'a> {
    /// Borrow of the prepared pattern and failure function.
    prepared: &'a KmpPrepared,
    /// Number of pattern bytes matched so far.
    j: usize,
    /// Total bytes fed since construction.
    bytes_fed: u64,
    /// Whether [`feed_slice`](SearchStream::feed_slice) has been called yet.
    ///
    /// Used to emit the empty-pattern match at position `0` on the first
    /// call and not on subsequent calls.
    initial: bool,
}

impl StreamingSearch for Kmp {
    type Prepared = KmpPrepared;
    type Stream<'a>
        = KmpStream<'a>
    where
        Self: 'a,
        KmpPrepared: 'a;

    fn stream(prepared: &Self::Prepared) -> Self::Stream<'_> {
        KmpStream {
            prepared,
            j: 0,
            bytes_fed: 0,
            initial: true,
        }
    }
}

impl SearchStream for KmpStream<'_> {
    fn feed(&mut self, byte: u8) -> Option<Match> {
        let pattern = self.prepared.pattern();
        let m = pattern.len();
        // Empty pattern: no per-byte match ever emitted; the position-0
        // match is a feed_slice-only concern.
        self.bytes_fed += 1;
        if m == 0 {
            return None;
        }
        let failure = self.prepared.failure();
        // Standard KMP step, identical to the batch algorithm's inner
        // loop.
        while self.j > 0 && pattern[self.j] != byte {
            self.j = failure[self.j - 1];
        }
        if pattern[self.j] == byte {
            self.j += 1;
        }
        if self.j == m {
            let position = usize::try_from(self.bytes_fed).expect(
                "bytes_fed fits in usize on any 32/64-bit target within u32/u64 haystack sizes",
            ) - m;
            // Continue for overlap by following the failure link — same
            // trick as the batch find_all.
            self.j = failure[m - 1];
            Some(Match::new(position))
        } else {
            None
        }
    }

    fn feed_slice(&mut self, bytes: &[u8]) -> Vec<Match> {
        let mut out = Vec::new();
        if self.initial {
            self.initial = false;
            if self.prepared.pattern().is_empty() {
                out.push(Match::new(0));
            }
        }
        for &b in bytes {
            if let Some(m) = self.feed(b) {
                out.push(m);
            }
        }
        out
    }

    fn bytes_fed(&self) -> u64 {
        self.bytes_fed
    }
}

// ---------------------------------------------------------------------------
// Aho-Corasick streaming.
// ---------------------------------------------------------------------------

/// Aho-Corasick streaming state.
///
/// Feeds bytes through the automaton one at a time, emitting matches
/// from the output set of the current state. Because a single state can
/// carry multiple pattern outputs, a batched buffer of pending matches
/// is drained across successive [`feed`](SearchStream::feed) calls.
#[derive(Debug)]
pub struct AhoCorasickStream<'a> {
    /// Borrow of the built automaton.
    automaton: &'a AhoCorasick,
    /// Current automaton state.
    current: u32,
    /// Total bytes fed since construction.
    bytes_fed: u64,
    /// Matches produced by the most-recent byte but not yet drained.
    ///
    /// A state can carry any number of pattern outputs; `feed` returns
    /// one per call, and `feed_slice` drains all of them.
    pending: VecDeque<Match>,
    /// Whether [`feed_slice`](SearchStream::feed_slice) has been called yet.
    ///
    /// Used to emit each empty-pattern match at position `0` on the
    /// first call.
    initial: bool,
}

impl StreamingSearch for AhoCorasick {
    type Prepared = AhoCorasick;
    type Stream<'a>
        = AhoCorasickStream<'a>
    where
        Self: 'a,
        AhoCorasick: 'a;

    fn stream(prepared: &Self::Prepared) -> Self::Stream<'_> {
        AhoCorasickStream {
            automaton: prepared,
            current: 0,
            bytes_fed: 0,
            pending: VecDeque::new(),
            initial: true,
        }
    }
}

impl AhoCorasickStream<'_> {
    /// Advances the automaton by one byte, queueing any matches into
    /// `pending`.
    fn advance(&mut self, byte: u8) {
        // A byte-by-byte re-implementation of the AhoCorasick::find_all
        // scan, without the final sort (matches from one byte end at the
        // same position, so their relative ordering across bytes is
        // already ascending).
        self.bytes_fed += 1;
        let mut current = self.current;
        // Follow failure links until a transition on `byte` exists or we
        // bottom out at the root. Same logic as the batch find_all.
        while current != 0 && !self.automaton.goto_contains_key(current, byte) {
            current = self.automaton.failure_of(current);
        }
        if let Some(next) = self.automaton.goto_transition(current, byte) {
            current = next;
        }
        self.current = current;
        // Emit every pattern that terminates at the current state.
        let bytes_fed = self.bytes_fed;
        for pattern_index in self.automaton.outputs_of(current) {
            let length = self.automaton.pattern_length(pattern_index);
            #[allow(
                clippy::cast_possible_truncation,
                reason = "haystack indices fit in usize; bytes_fed is bounded above by usize::MAX in any realistic workload"
            )]
            let position = (bytes_fed as usize) - length;
            self.pending
                .push_back(Match::with_pattern(position, pattern_index));
        }
    }
}

impl SearchStream for AhoCorasickStream<'_> {
    fn feed(&mut self, byte: u8) -> Option<Match> {
        self.advance(byte);
        self.pending.pop_front()
    }

    fn feed_slice(&mut self, bytes: &[u8]) -> Vec<Match> {
        let mut out = Vec::new();
        if self.initial {
            self.initial = false;
            // Empty patterns fire once each at position 0, mirroring
            // batch AhoCorasick::find_all's behavior.
            for pattern_index in 0..self.automaton.pattern_count() {
                if self.automaton.pattern_length(pattern_index) == 0 {
                    out.push(Match::with_pattern(0, pattern_index));
                }
            }
        }
        for &b in bytes {
            self.advance(b);
            while let Some(m) = self.pending.pop_front() {
                out.push(m);
            }
        }
        // Sort each byte's outputs by (position, pattern_index) as the
        // batch scan does. Matches from a single byte share an end
        // position, and across bytes they're already ascending by end,
        // so a full sort is only needed inside per-byte groups; sorting
        // the whole vector is simpler and remains O(n log n) worst-case.
        out.sort_by_key(|m| (m.position, m.pattern_index));
        out
    }

    fn bytes_fed(&self) -> u64 {
        self.bytes_fed
    }
}

// ---------------------------------------------------------------------------
// Rabin-Karp streaming.
// ---------------------------------------------------------------------------

/// Rabin-Karp streaming state.
///
/// Maintains the rolling hash of the current window plus a ring buffer
/// of the current window's bytes — the verification step must compare
/// window bytes against the pattern, and streaming means the caller can
/// no longer hand us the whole haystack. The ring buffer's size is the
/// pattern length; overall streaming memory is `O(pattern.len())`.
#[derive(Debug)]
pub struct RabinKarpStream<'a> {
    /// Borrow of the prepared pattern hash and factor.
    prepared: &'a RabinKarpPrepared,
    /// The current window's rolling hash.
    hash: u64,
    /// The current window's bytes, most-recent-last, cyclically stored.
    ///
    /// Only allocated when `pattern.len() > 0`. Capacity equals the
    /// pattern length; the oldest byte in the window sits at index
    /// `write_pos` (where the next write will overwrite it), and the
    /// newest byte sits at `(write_pos - 1 + m) % m`.
    window: Vec<u8>,
    /// The index in `window` where the next-fed byte will be written.
    write_pos: usize,
    /// Total bytes fed since construction.
    bytes_fed: u64,
    /// Whether [`feed_slice`](SearchStream::feed_slice) has been called yet.
    initial: bool,
}

impl StreamingSearch for RabinKarp {
    type Prepared = RabinKarpPrepared;
    type Stream<'a>
        = RabinKarpStream<'a>
    where
        Self: 'a,
        RabinKarpPrepared: 'a;

    fn stream(prepared: &Self::Prepared) -> Self::Stream<'_> {
        let m = prepared.pattern().len();
        RabinKarpStream {
            prepared,
            hash: 0,
            window: alloc::vec![0u8; m],
            write_pos: 0,
            bytes_fed: 0,
            initial: true,
        }
    }
}

impl RabinKarpStream<'_> {
    /// Compares the current window (logical, in feed order) against the
    /// prepared pattern.
    ///
    /// Returns `true` when the window fully matches. Callers only invoke
    /// this after the rolling hashes agree.
    fn window_matches(&self) -> bool {
        let m = self.prepared.pattern().len();
        let pattern = self.prepared.pattern();
        // The window's oldest byte sits at `write_pos` (next to be
        // overwritten). Walk forward from there to reconstruct the
        // logical window order.
        for (k, &expected) in pattern.iter().enumerate() {
            let ring_idx = (self.write_pos + k) % m;
            if self.window[ring_idx] != expected {
                return false;
            }
        }
        true
    }
}

impl SearchStream for RabinKarpStream<'_> {
    fn feed(&mut self, byte: u8) -> Option<Match> {
        let m = self.prepared.pattern().len();
        self.bytes_fed += 1;
        if m == 0 {
            return None;
        }

        // Not enough bytes yet — accumulate into the window and hash
        // without any rolling subtraction.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "bytes_fed <= m at this branch; m fits in usize"
        )]
        let filled = self.bytes_fed as usize <= m;
        if filled {
            // Grow the hash by multiplying and adding — same as the
            // batch prepare step.
            let product = u128::from(self.hash) * u128::from(RK_BASE);
            let reduced = product % u128::from(RK_MODULUS);
            #[allow(
                clippy::cast_possible_truncation,
                reason = "reduced < MODULUS (61 bits); fits in u64"
            )]
            let mut new_hash = reduced as u64;
            new_hash += u64::from(byte);
            if new_hash >= RK_MODULUS {
                new_hash -= RK_MODULUS;
            }
            self.hash = new_hash;
            // Fill the window's next slot; ring index equals sequential
            // index during the initial fill.
            self.window[self.write_pos] = byte;
            self.write_pos = (self.write_pos + 1) % m;
            // Emit a match only if we've just filled the window and it
            // matches. Otherwise, defer to the rolling-hash branch.
            #[allow(
                clippy::cast_possible_truncation,
                reason = "same rationale as `filled` above"
            )]
            let just_full = self.bytes_fed as usize == m;
            if just_full && self.hash == self.prepared.pattern_hash() && self.window_matches() {
                return Some(Match::new(0));
            }
            return None;
        }

        // Rolling update: subtract the leaving byte's contribution, then
        // multiply by BASE and add the entering byte.
        let leading_factor = self.prepared.leading_factor();
        // The leaving byte is the one currently at `write_pos` — that's
        // the oldest slot, about to be overwritten.
        let leaving = u64::from(self.window[self.write_pos]);
        let leaving_contribution_product = u128::from(leaving) * u128::from(leading_factor);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "product % MODULUS < 2^61; fits in u64"
        )]
        let leaving_contribution = (leaving_contribution_product % u128::from(RK_MODULUS)) as u64;

        let mut new_hash = self.hash + RK_MODULUS - leaving_contribution;
        if new_hash >= RK_MODULUS {
            new_hash -= RK_MODULUS;
        }
        let product = u128::from(new_hash) * u128::from(RK_BASE);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "product % MODULUS < 2^61; fits in u64"
        )]
        {
            new_hash = (product % u128::from(RK_MODULUS)) as u64;
        }
        new_hash += u64::from(byte);
        if new_hash >= RK_MODULUS {
            new_hash -= RK_MODULUS;
        }
        self.hash = new_hash;

        // Overwrite the leaving slot with the entering byte and rotate.
        self.window[self.write_pos] = byte;
        self.write_pos = (self.write_pos + 1) % m;

        // Verify on hash match. The match starts at `bytes_fed - m`.
        if self.hash == self.prepared.pattern_hash() && self.window_matches() {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "bytes_fed - m is a valid haystack index; fits in usize"
            )]
            let position = (self.bytes_fed as usize) - m;
            return Some(Match::new(position));
        }
        None
    }

    fn feed_slice(&mut self, bytes: &[u8]) -> Vec<Match> {
        let mut out = Vec::new();
        if self.initial {
            self.initial = false;
            if self.prepared.pattern().is_empty() {
                out.push(Match::new(0));
            }
        }
        for &b in bytes {
            if let Some(m) = self.feed(b) {
                out.push(m);
            }
        }
        out
    }

    fn bytes_fed(&self) -> u64 {
        self.bytes_fed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::SearchAlgorithm;

    #[test]
    fn kmp_stream_matches_batch_on_a_simple_input() {
        let prepared = Kmp::prepare(b"abc");
        let mut s = <Kmp as StreamingSearch>::stream(&prepared);
        let matches = s.feed_slice(b"xxabcxxabc");
        assert_eq!(matches, alloc::vec![Match::new(2), Match::new(7)]);
        assert_eq!(s.bytes_fed(), 10);
    }

    #[test]
    fn kmp_stream_split_matches_contiguous() {
        let prepared = Kmp::prepare(b"abc");
        let mut s1 = <Kmp as StreamingSearch>::stream(&prepared);
        let contiguous = s1.feed_slice(b"xxabcxxabc");
        let mut s2 = <Kmp as StreamingSearch>::stream(&prepared);
        let mut split = s2.feed_slice(b"xxab");
        split.extend(s2.feed_slice(b"cxxabc"));
        assert_eq!(contiguous, split);
    }

    #[test]
    fn kmp_stream_matches_across_a_split() {
        // The match "abc" straddles the split; the streaming state must
        // carry over.
        let prepared = Kmp::prepare(b"abc");
        let mut s = <Kmp as StreamingSearch>::stream(&prepared);
        let mut all = s.feed_slice(b"xxa");
        all.extend(s.feed_slice(b"bcyy"));
        assert_eq!(all, alloc::vec![Match::new(2)]);
    }

    #[test]
    fn kmp_stream_overlapping_matches() {
        let prepared = Kmp::prepare(b"aa");
        let mut s = <Kmp as StreamingSearch>::stream(&prepared);
        let matches = s.feed_slice(b"aaaa");
        assert_eq!(
            matches,
            alloc::vec![Match::new(0), Match::new(1), Match::new(2)]
        );
    }

    #[test]
    fn kmp_stream_empty_pattern() {
        let prepared = Kmp::prepare(b"");
        let mut s = <Kmp as StreamingSearch>::stream(&prepared);
        let matches = s.feed_slice(b"abc");
        assert_eq!(matches, alloc::vec![Match::new(0)]);
        // Subsequent feed_slice calls do not repeat the position-0 match.
        let more = s.feed_slice(b"def");
        assert!(more.is_empty());
    }

    #[test]
    fn rabin_karp_stream_matches_batch() {
        let prepared = RabinKarp::prepare(b"needle");
        let mut s = <RabinKarp as StreamingSearch>::stream(&prepared);
        let matches = s.feed_slice(b"xxneedleyyneedle");
        assert_eq!(matches, alloc::vec![Match::new(2), Match::new(10)]);
    }

    #[test]
    fn rabin_karp_stream_overlapping_matches() {
        let prepared = RabinKarp::prepare(b"aa");
        let mut s = <RabinKarp as StreamingSearch>::stream(&prepared);
        let matches = s.feed_slice(b"aaaa");
        assert_eq!(
            matches,
            alloc::vec![Match::new(0), Match::new(1), Match::new(2)]
        );
    }

    #[test]
    fn rabin_karp_stream_empty_pattern() {
        let prepared = RabinKarp::prepare(b"");
        let mut s = <RabinKarp as StreamingSearch>::stream(&prepared);
        let matches = s.feed_slice(b"abc");
        assert_eq!(matches, alloc::vec![Match::new(0)]);
    }

    #[test]
    fn rabin_karp_stream_split_matches_contiguous() {
        let prepared = RabinKarp::prepare(b"needle");
        let mut s1 = <RabinKarp as StreamingSearch>::stream(&prepared);
        let contiguous = s1.feed_slice(b"xxneedleyyneedle");
        let mut s2 = <RabinKarp as StreamingSearch>::stream(&prepared);
        let mut split = s2.feed_slice(b"xxneed");
        split.extend(s2.feed_slice(b"leyyneedle"));
        assert_eq!(contiguous, split);
    }

    #[test]
    fn aho_corasick_stream_matches_batch() {
        let ac = AhoCorasick::build(&[b"he", b"she", b"his", b"hers"]);
        let batch = ac.find_all(b"ushers");
        let mut s = <AhoCorasick as StreamingSearch>::stream(&ac);
        let stream = s.feed_slice(b"ushers");
        assert_eq!(batch, stream);
    }

    #[test]
    fn aho_corasick_stream_split_matches_contiguous() {
        let ac = AhoCorasick::build(&[b"he", b"she", b"his", b"hers"]);
        let mut s1 = <AhoCorasick as StreamingSearch>::stream(&ac);
        let contiguous = s1.feed_slice(b"ushers");
        let mut s2 = <AhoCorasick as StreamingSearch>::stream(&ac);
        let mut split = s2.feed_slice(b"ush");
        split.extend(s2.feed_slice(b"ers"));
        // The split point can permute matches ending at different bytes
        // in different sort orders — normalize before comparing.
        let mut a = contiguous;
        let mut b = split;
        a.sort_by_key(|m| (m.position, m.pattern_index));
        b.sort_by_key(|m| (m.position, m.pattern_index));
        assert_eq!(a, b);
    }
}
