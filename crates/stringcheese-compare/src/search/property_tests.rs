//! Property-based tests for the substring search algorithms.
//!
//! The star property here is the **cross-algorithm differential**: for any
//! pattern and haystack pair, Rabin-Karp, KMP, Boyer-Moore, Horspool, and
//! Two-way must produce the exact same list of matches. A bug in any
//! single implementation is much more likely to surface as a disagreement
//! than as a silent wrong answer.
//!
//! A tighter differential specific to Boyer-Moore pins that the
//! bad-character-only variant and the full (bad-character +
//! good-suffix) variant agree on every input — the good-suffix
//! heuristic only affects performance, not correctness.
//!
//! Additional properties:
//!
//! * Aho-Corasick with a single-pattern set equals the single-pattern
//!   algorithms on that same pattern.
//! * `find_all` results are in ascending `position` order and every
//!   `position + pattern.len()` is a valid haystack index.
//! * Determinism: repeated calls with the same inputs return identical
//!   results.
//! * Empty pattern: matches once at position 0 (per the crate-wide policy).
//! * Streaming equivalence: for the streaming algorithms (KMP,
//!   Aho-Corasick, Rabin-Karp), `feed_slice(all_bytes)` produces the
//!   same matches as batch `find_all(all_bytes)`, and split-invariance
//!   holds — `feed_slice(prefix); feed_slice(suffix)` equals a single
//!   contiguous feed.
//!
//! Input sizes are capped to keep proptest fast: patterns up to 20 bytes,
//! haystacks up to 200 bytes, over a five-symbol alphabet to yield a mix
//! of matches and non-matches.

use proptest::prelude::*;

use crate::search::aho_corasick::AhoCorasick;
use crate::search::api::{Match, SearchAlgorithm, SinglePatternSearch};
use crate::search::boyer_moore::{BoyerMoore, BoyerMooreFull};
use crate::search::horspool::Horspool;
use crate::search::kmp::Kmp;
use crate::search::rabin_karp::RabinKarp;
use crate::search::stream::{SearchStream, StreamingSearch};
use crate::search::two_way::TwoWay;

/// A five-symbol alphabet keeps proptest inputs small enough to run
/// quickly while still producing plenty of matches and near-matches.
fn arb_bytes(max_len: usize) -> impl Strategy<Value = std::vec::Vec<u8>> {
    proptest::collection::vec(b'a'..=b'e', 0..=max_len)
}

fn arb_pattern_and_haystack() -> impl Strategy<Value = (std::vec::Vec<u8>, std::vec::Vec<u8>)> {
    (arb_bytes(20), arb_bytes(200))
}

fn run_single<A: SinglePatternSearch>(pattern: &[u8], haystack: &[u8]) -> std::vec::Vec<Match> {
    let prepared = A::prepare(pattern);
    A::find_all(&prepared, haystack)
}

proptest! {
    /// The star property: all three single-pattern algorithms agree on
    /// every input.
    #[test]
    fn proptest_single_pattern_algorithms_agree(
        (pattern, haystack) in arb_pattern_and_haystack(),
    ) {
        let rk = run_single::<RabinKarp>(&pattern, &haystack);
        let kmp = run_single::<Kmp>(&pattern, &haystack);
        let bm = run_single::<BoyerMoore>(&pattern, &haystack);
        prop_assert_eq!(&rk, &kmp,
            "Rabin-Karp / KMP disagreed on pattern={:?} haystack={:?}", pattern, haystack);
        prop_assert_eq!(&rk, &bm,
            "Rabin-Karp / Boyer-Moore disagreed on pattern={:?} haystack={:?}", pattern, haystack);
    }

    /// Aho-Corasick with a one-pattern set matches the single-pattern
    /// algorithms exactly.
    #[test]
    fn proptest_aho_corasick_single_pattern_agrees(
        (pattern, haystack) in arb_pattern_and_haystack(),
    ) {
        let ac = AhoCorasick::build(&[&pattern]);
        let ac_matches = ac.find_all(&haystack);
        let kmp_matches = run_single::<Kmp>(&pattern, &haystack);
        prop_assert_eq!(&ac_matches, &kmp_matches,
            "Aho-Corasick / KMP disagreed on pattern={:?} haystack={:?}", pattern, haystack);
    }

    /// Every `Match.position + pattern.len()` fits within the haystack;
    /// KMP is used as the reference (property is symmetric under
    /// differential agreement).
    #[test]
    fn proptest_match_positions_are_valid_indices(
        (pattern, haystack) in arb_pattern_and_haystack(),
    ) {
        let matches = run_single::<Kmp>(&pattern, &haystack);
        for m in &matches {
            if pattern.is_empty() {
                prop_assert_eq!(m.position, 0);
            } else {
                prop_assert!(
                    m.position + pattern.len() <= haystack.len(),
                    "match at {} overflows haystack (pattern.len()={})",
                    m.position, pattern.len(),
                );
                prop_assert_eq!(&haystack[m.position..m.position + pattern.len()], &pattern[..]);
            }
        }
    }

    /// `find_all` returns matches in ascending position order.
    #[test]
    fn proptest_find_all_is_sorted_ascending(
        (pattern, haystack) in arb_pattern_and_haystack(),
    ) {
        let matches = run_single::<Kmp>(&pattern, &haystack);
        for pair in matches.windows(2) {
            prop_assert!(
                pair[0].position <= pair[1].position,
                "find_all returned an out-of-order match sequence: {} then {}",
                pair[0].position, pair[1].position,
            );
        }
    }

    /// Determinism: repeated calls with the same input return identical
    /// results.
    #[test]
    fn proptest_determinism(
        (pattern, haystack) in arb_pattern_and_haystack(),
    ) {
        let a = run_single::<Kmp>(&pattern, &haystack);
        let b = run_single::<Kmp>(&pattern, &haystack);
        prop_assert_eq!(a, b);
    }

    /// Empty pattern: single match at position 0, across all algorithms.
    #[test]
    fn proptest_empty_pattern_policy(haystack in arb_bytes(200)) {
        let expected = std::vec![Match::new(0)];
        prop_assert_eq!(run_single::<RabinKarp>(b"", &haystack), expected.clone());
        prop_assert_eq!(run_single::<Kmp>(b"", &haystack), expected.clone());
        prop_assert_eq!(run_single::<BoyerMoore>(b"", &haystack), expected);
    }

    /// `find` returns `Some(m)` iff `find_all` returns at least one match,
    /// and `m` equals the first element of `find_all`.
    #[test]
    fn proptest_find_agrees_with_find_all_head(
        (pattern, haystack) in arb_pattern_and_haystack(),
    ) {
        let all = run_single::<Kmp>(&pattern, &haystack);
        let first = {
            let prepared = Kmp::prepare(&pattern);
            Kmp::find(&prepared, &haystack)
        };
        match (all.first(), first) {
            (None, None) => (),
            (Some(a), Some(b)) => prop_assert_eq!(*a, b),
            (a, b) => prop_assert!(
                false,
                "find/find_all disagreed: find_all.first()={:?}, find()={:?}", a, b,
            ),
        }
    }

    /// Boyer-Moore (bad-character-only) and Boyer-Moore (full, with
    /// good-suffix) must produce identical match sets on every input —
    /// the good-suffix heuristic only changes performance, not
    /// correctness. This is the key differential test that pins the
    /// good-suffix table construction against the simpler, more
    /// obviously-correct bad-character-only implementation.
    #[test]
    fn proptest_boyer_moore_bad_char_matches_full_variant(
        (pattern, haystack) in arb_pattern_and_haystack(),
    ) {
        let bad = run_single::<BoyerMoore>(&pattern, &haystack);
        let full = run_single::<BoyerMooreFull>(&pattern, &haystack);
        prop_assert_eq!(&bad, &full,
            "BoyerMoore / BoyerMooreFull disagreed on pattern={:?} haystack={:?}",
            pattern, haystack);
    }

    /// Horspool must agree with the KMP reference on every input.
    #[test]
    fn proptest_horspool_agrees_with_kmp(
        (pattern, haystack) in arb_pattern_and_haystack(),
    ) {
        let hs = run_single::<Horspool>(&pattern, &haystack);
        let kmp = run_single::<Kmp>(&pattern, &haystack);
        prop_assert_eq!(&hs, &kmp,
            "Horspool / KMP disagreed on pattern={:?} haystack={:?}", pattern, haystack);
    }

    /// Two-way (Crochemore-Perrin) must agree with KMP on every input.
    #[test]
    fn proptest_two_way_agrees_with_kmp(
        (pattern, haystack) in arb_pattern_and_haystack(),
    ) {
        let tw = run_single::<TwoWay>(&pattern, &haystack);
        let kmp = run_single::<Kmp>(&pattern, &haystack);
        prop_assert_eq!(&tw, &kmp,
            "TwoWay / KMP disagreed on pattern={:?} haystack={:?}", pattern, haystack);
    }

    /// KMP streaming feed of all bytes at once equals batch find_all.
    #[test]
    fn proptest_kmp_stream_equals_batch(
        (pattern, haystack) in arb_pattern_and_haystack(),
    ) {
        let prepared = Kmp::prepare(&pattern);
        let mut s = <Kmp as StreamingSearch>::stream(&prepared);
        let stream = s.feed_slice(&haystack);
        let batch = Kmp::find_all(&prepared, &haystack);
        prop_assert_eq!(&stream, &batch,
            "KMP stream vs batch disagreed on pattern={:?} haystack={:?}",
            pattern, haystack);
    }

    /// Rabin-Karp streaming feed of all bytes at once equals batch find_all.
    #[test]
    fn proptest_rabin_karp_stream_equals_batch(
        (pattern, haystack) in arb_pattern_and_haystack(),
    ) {
        let prepared = RabinKarp::prepare(&pattern);
        let mut s = <RabinKarp as StreamingSearch>::stream(&prepared);
        let stream = s.feed_slice(&haystack);
        let batch = RabinKarp::find_all(&prepared, &haystack);
        prop_assert_eq!(&stream, &batch,
            "Rabin-Karp stream vs batch disagreed on pattern={:?} haystack={:?}",
            pattern, haystack);
    }

    /// Aho-Corasick streaming feed of all bytes at once equals batch find_all.
    #[test]
    fn proptest_aho_corasick_stream_equals_batch(
        (pattern, haystack) in arb_pattern_and_haystack(),
    ) {
        let ac = AhoCorasick::build(&[&pattern]);
        let mut s = <AhoCorasick as StreamingSearch>::stream(&ac);
        let stream = s.feed_slice(&haystack);
        let batch = ac.find_all(&haystack);
        prop_assert_eq!(&stream, &batch,
            "Aho-Corasick stream vs batch disagreed on pattern={:?} haystack={:?}",
            pattern, haystack);
    }

    /// KMP streaming split-invariance: feeding the input in two pieces
    /// yields the same matches as one contiguous feed.
    #[test]
    fn proptest_kmp_stream_split_invariance(
        (pattern, haystack) in arb_pattern_and_haystack(),
        split in 0usize..=200,
    ) {
        let prepared = Kmp::prepare(&pattern);
        let split = split.min(haystack.len());
        let (a, b) = haystack.split_at(split);
        let mut s1 = <Kmp as StreamingSearch>::stream(&prepared);
        let contiguous = s1.feed_slice(&haystack);
        let mut s2 = <Kmp as StreamingSearch>::stream(&prepared);
        let mut chunked = s2.feed_slice(a);
        chunked.extend(s2.feed_slice(b));
        prop_assert_eq!(&contiguous, &chunked,
            "KMP stream split disagreed: split={}, pattern={:?}, haystack={:?}",
            split, pattern, haystack);
    }

    /// Rabin-Karp streaming split-invariance.
    #[test]
    fn proptest_rabin_karp_stream_split_invariance(
        (pattern, haystack) in arb_pattern_and_haystack(),
        split in 0usize..=200,
    ) {
        let prepared = RabinKarp::prepare(&pattern);
        let split = split.min(haystack.len());
        let (a, b) = haystack.split_at(split);
        let mut s1 = <RabinKarp as StreamingSearch>::stream(&prepared);
        let contiguous = s1.feed_slice(&haystack);
        let mut s2 = <RabinKarp as StreamingSearch>::stream(&prepared);
        let mut chunked = s2.feed_slice(a);
        chunked.extend(s2.feed_slice(b));
        prop_assert_eq!(&contiguous, &chunked,
            "Rabin-Karp stream split disagreed: split={}, pattern={:?}, haystack={:?}",
            split, pattern, haystack);
    }

    /// Aho-Corasick streaming split-invariance.
    #[test]
    fn proptest_aho_corasick_stream_split_invariance(
        (pattern, haystack) in arb_pattern_and_haystack(),
        split in 0usize..=200,
    ) {
        let ac = AhoCorasick::build(&[&pattern]);
        let split = split.min(haystack.len());
        let (a, b) = haystack.split_at(split);
        let mut s1 = <AhoCorasick as StreamingSearch>::stream(&ac);
        let mut contiguous = s1.feed_slice(&haystack);
        let mut s2 = <AhoCorasick as StreamingSearch>::stream(&ac);
        let mut chunked = s2.feed_slice(a);
        chunked.extend(s2.feed_slice(b));
        // Normalize before comparing — see the stream module tests for
        // the sort rationale.
        contiguous.sort_by_key(|m| (m.position, m.pattern_index));
        chunked.sort_by_key(|m| (m.position, m.pattern_index));
        prop_assert_eq!(&contiguous, &chunked,
            "Aho-Corasick stream split disagreed: split={}, pattern={:?}, haystack={:?}",
            split, pattern, haystack);
    }
}
