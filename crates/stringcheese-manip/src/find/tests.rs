//! Tests for [`crate::find`].
//!
//! Unit tests exercise the empty-pattern, single-match, multi-match,
//! overlapping-vs-non-overlapping, multi-byte-scalar, and multi-pattern
//! edge cases. Property tests confirm the identity, contains-iff-find,
//! count-matches-length invariants that hold across all inputs.

use super::*;
use alloc::vec;

// -----------------------------------------------------------------
// find
// -----------------------------------------------------------------

#[test]
fn find_empty_pattern_matches_at_zero() {
    assert_eq!(find("hello", ""), Some(0));
    assert_eq!(find("", ""), Some(0));
}

#[test]
fn find_absent_pattern_is_none() {
    assert_eq!(find("hello", "z"), None);
    assert_eq!(find("", "hello"), None);
}

#[test]
fn find_present_pattern_returns_first_offset() {
    assert_eq!(find("hello world", "world"), Some(6));
    assert_eq!(find("hello world hello", "hello"), Some(0));
}

#[test]
fn find_matches_string_against_itself() {
    assert_eq!(find("hello", "hello"), Some(0));
}

#[test]
fn find_multi_byte_scalar() {
    // "café" is 5 bytes; "é" starts at byte 3.
    assert_eq!(find("café", "é"), Some(3));
}

// -----------------------------------------------------------------
// rfind
// -----------------------------------------------------------------

#[test]
fn rfind_returns_last_occurrence() {
    assert_eq!(rfind("hello world hello", "hello"), Some(12));
    assert_eq!(rfind("banana", "an"), Some(3));
}

#[test]
fn rfind_absent_is_none() {
    assert_eq!(rfind("hello", "z"), None);
}

// -----------------------------------------------------------------
// contains / starts_with / ends_with
// -----------------------------------------------------------------

#[test]
fn contains_agrees_with_find() {
    assert!(contains("hello world", "world"));
    assert!(!contains("hello", "z"));
    // Every string contains the empty string.
    assert!(contains("", ""));
    assert!(contains("hello", ""));
}

#[test]
fn starts_with_basic() {
    assert!(starts_with("hello world", "hello"));
    assert!(!starts_with("hello world", "world"));
    assert!(starts_with("hello", ""));
}

#[test]
fn ends_with_basic() {
    assert!(ends_with("hello world", "world"));
    assert!(!ends_with("hello world", "hello"));
    assert!(ends_with("hello", ""));
}

// -----------------------------------------------------------------
// find_all / find_iter / count_matches (non-overlapping)
// -----------------------------------------------------------------

#[test]
fn find_all_no_matches() {
    assert_eq!(find_all("hello", "z"), vec![]);
}

#[test]
fn find_all_disjoint_matches() {
    assert_eq!(find_all("banana", "an"), vec![1, 3]);
}

#[test]
fn find_all_non_overlapping_semantics() {
    // "aa" in "aaaa": positions 0 and 2 (non-overlapping), not 0, 1, 2.
    assert_eq!(find_all("aaaa", "aa"), vec![0, 2]);
    // "aa" in "aaaaa": positions 0, 2 (5 bytes; the trailing single 'a'
    // is not a match).
    assert_eq!(find_all("aaaaa", "aa"), vec![0, 2]);
}

#[test]
fn find_all_empty_pattern_yields_single_zero() {
    // Empty pattern matches at position 0 exactly once — matches the
    // policy of `stringcheese_compare::search`.
    assert_eq!(find_all("hello", ""), vec![0]);
    assert_eq!(find_all("", ""), vec![0]);
}

#[test]
fn find_iter_matches_find_all() {
    let via_iter: Vec<usize> = find_iter("banana", "an").collect();
    assert_eq!(via_iter, find_all("banana", "an"));
}

#[test]
fn count_matches_agrees_with_find_all_len() {
    let inputs: &[(&str, &str)] = &[
        ("banana", "an"),
        ("aaaa", "aa"),
        ("hello", "z"),
        ("abc", ""),
        ("", ""),
        ("abcabcabc", "abc"),
    ];
    for &(h, p) in inputs {
        assert_eq!(
            count_matches(h, p),
            find_all(h, p).len(),
            "haystack={h:?} pat={p:?}"
        );
    }
}

#[test]
fn count_matches_non_overlapping() {
    assert_eq!(count_matches("aaaa", "aa"), 2);
    assert_eq!(count_matches("abcabcabc", "abc"), 3);
}

// -----------------------------------------------------------------
// find_any (multi-pattern)
// -----------------------------------------------------------------

#[test]
fn find_any_returns_leftmost() {
    let needles = &["dog", "cat", "at"];
    // "cat" starts at byte 4; "at" starts at byte 5; "cat" wins because
    // its position is earlier.
    assert_eq!(find_any("the cat sat", needles), Some((4, 1)));
}

#[test]
fn find_any_tiebreak_by_needle_index() {
    // Both "aa" and "aabb" start at byte 0; the tie is broken by
    // needle_index (lower wins) — so whichever appears first in the
    // input slice wins regardless of length.
    assert_eq!(find_any("aabbcc", &["aa", "aabb"]), Some((0, 0)));
    assert_eq!(find_any("aabbcc", &["aabb", "aa"]), Some((0, 0)));
}

#[test]
fn find_any_no_match_is_none() {
    assert_eq!(find_any("xyz", &["a", "b"]), None);
}

#[test]
fn find_any_empty_needle_set_is_none() {
    let empty: &[&str] = &[];
    assert_eq!(find_any("hello", empty), None);
}

#[test]
fn find_any_single_needle_matches_find() {
    // Sanity check: with a single needle, find_any and find agree on
    // position.
    let pos = find_any("hello world", &["world"]).map(|(p, _)| p);
    assert_eq!(pos, find("hello world", "world"));
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn ascii_string() -> impl Strategy<Value = String> {
        prop::string::string_regex("[a-z]{0,32}").expect("static regex is valid")
    }

    fn short_ascii_pattern() -> impl Strategy<Value = String> {
        prop::string::string_regex("[a-z]{0,4}").expect("static regex is valid")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // A string always finds itself at offset 0.
        #[test]
        fn find_self_at_zero(s in ascii_string()) {
            prop_assert_eq!(find(&s, &s), Some(0));
        }

        // contains(s, p) <=> find(s, p).is_some().
        #[test]
        fn contains_iff_find_some(h in ascii_string(), p in short_ascii_pattern()) {
            prop_assert_eq!(contains(&h, &p), find(&h, &p).is_some());
        }

        // count_matches == find_all.len().
        #[test]
        fn count_matches_equals_find_all_len(h in ascii_string(), p in short_ascii_pattern()) {
            prop_assert_eq!(count_matches(&h, &p), find_all(&h, &p).len());
        }

        // find_all is sorted ascending, and matches are non-overlapping.
        #[test]
        fn find_all_is_sorted_and_disjoint(h in ascii_string(), p in short_ascii_pattern()) {
            let all = find_all(&h, &p);
            let plen = p.len();
            for w in all.windows(2) {
                prop_assert!(w[0] < w[1], "not sorted: {:?}", all);
                if plen > 0 {
                    prop_assert!(w[1] >= w[0] + plen, "overlapping: {:?} (plen={})", all, plen);
                }
            }
        }

        // starts_with(h, "") is always true; ends_with(h, "") is always true.
        #[test]
        fn empty_pattern_is_always_prefix_and_suffix(h in ascii_string()) {
            prop_assert!(starts_with(&h, ""));
            prop_assert!(ends_with(&h, ""));
        }

        // find agrees with str::find on ASCII inputs.
        #[test]
        fn find_agrees_with_str_find(h in ascii_string(), p in short_ascii_pattern()) {
            if p.is_empty() {
                // str::find of empty is Some(0); ours is too.
                prop_assert_eq!(find(&h, &p), Some(0));
            } else {
                prop_assert_eq!(find(&h, &p), h.find(&p));
            }
        }
    }
}
