//! Property-based tests for the substring search algorithms.
//!
//! The star property here is the **cross-algorithm differential**: for any
//! pattern and haystack pair, Rabin-Karp, KMP, and Boyer-Moore must
//! produce the exact same list of matches. A bug in any single
//! implementation is much more likely to surface as a disagreement than as
//! a silent wrong answer.
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
//!
//! Input sizes are capped to keep proptest fast: patterns up to 20 bytes,
//! haystacks up to 200 bytes, over a five-symbol alphabet to yield a mix
//! of matches and non-matches.

use proptest::prelude::*;

use crate::aho_corasick::AhoCorasick;
use crate::api::{Match, SearchAlgorithm, SinglePatternSearch};
use crate::boyer_moore::BoyerMoore;
use crate::kmp::Kmp;
use crate::rabin_karp::RabinKarp;

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
}
