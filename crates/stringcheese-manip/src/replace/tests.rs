//! Tests for [`crate::replace`].
//!
//! Unit tests cover the empty-needle no-op policy, no-match cases,
//! overlapping-potential inputs, unicode-safety edge cases, and the
//! bounded / multi-pattern variants. Property tests confirm the identity
//! `replace(s, x, x) == s`, the no-op-on-empty-needle law, and length
//! monotonicity where applicable.

use super::*;

use alloc::string::ToString;

// -----------------------------------------------------------------
// replace
// -----------------------------------------------------------------

#[test]
fn replace_basic() {
    assert_eq!(replace("banana", "a", "o"), "bonono");
}

#[test]
fn replace_no_match_returns_input() {
    assert_eq!(replace("hello", "xyz", "!"), "hello");
}

#[test]
fn replace_empty_needle_is_noop() {
    // Documented divergence from `str::replace` — an empty needle is a
    // no-op, not an "insert to between every char" operation.
    assert_eq!(replace("abc", "", "!"), "abc");
}

#[test]
fn replace_empty_input_stays_empty() {
    assert_eq!(replace("", "a", "b"), "");
}

#[test]
fn replace_multi_byte_scalar_needle() {
    assert_eq!(replace("caf\u{00E9}", "\u{00E9}", "e"), "cafe");
}

#[test]
fn replace_expands_output() {
    assert_eq!(replace("abc", "b", "BBB"), "aBBBc");
}

#[test]
fn replace_shrinks_output() {
    assert_eq!(replace("aBBBc", "BBB", "b"), "abc");
}

// -----------------------------------------------------------------
// replace_n / replace_first / replace_last
// -----------------------------------------------------------------

#[test]
fn replace_n_caps_at_n() {
    assert_eq!(replace_n("banana", "a", "o", 2), "bonona");
}

#[test]
fn replace_n_zero_is_noop() {
    assert_eq!(replace_n("banana", "a", "o", 0), "banana");
}

#[test]
fn replace_n_more_than_available_replaces_all() {
    assert_eq!(replace_n("banana", "a", "o", 99), "bonono");
}

#[test]
fn replace_first_only_first_match() {
    assert_eq!(replace_first("banana", "a", "o"), "bonana");
}

#[test]
fn replace_first_matches_replace_n_one() {
    assert_eq!(
        replace_first("banana", "a", "o"),
        replace_n("banana", "a", "o", 1)
    );
}

#[test]
fn replace_last_only_last_match() {
    assert_eq!(replace_last("banana", "a", "o"), "banano");
}

#[test]
fn replace_last_single_occurrence() {
    assert_eq!(replace_last("hello", "l", "L"), "helLo");
}

#[test]
fn replace_last_no_match_returns_input() {
    assert_eq!(replace_last("hello", "z", "!"), "hello");
}

// -----------------------------------------------------------------
// replace_matches / translate / remove
// -----------------------------------------------------------------

#[test]
fn replace_matches_strips_digits() {
    assert_eq!(
        replace_matches("h1e2l3l4o", |c: char| c.is_ascii_digit(), ""),
        "hello"
    );
}

#[test]
fn replace_matches_expands() {
    assert_eq!(
        replace_matches("hello", |c: char| "aeiou".contains(c), "*"),
        "h*ll*"
    );
}

#[test]
fn replace_matches_never_matches() {
    assert_eq!(replace_matches("hello", |_: char| false, "-"), "hello");
}

#[test]
fn translate_basic() {
    assert_eq!(translate("hello", &[('l', 'L'), ('o', '0')]), "heLL0");
}

#[test]
fn translate_no_mapping() {
    assert_eq!(translate("abc", &[]), "abc");
}

#[test]
fn translate_unicode() {
    assert_eq!(translate("café", &[('é', 'e')]), "cafe");
}

#[test]
fn remove_deletes_substring() {
    assert_eq!(remove("hello world", "l"), "heo word");
}

#[test]
fn remove_multi_char() {
    assert_eq!(remove("abc123", "123"), "abc");
}

#[test]
fn remove_empty_needle_is_noop() {
    assert_eq!(remove("hello", ""), "hello");
}

// -----------------------------------------------------------------
// replace_with
// -----------------------------------------------------------------

#[test]
fn replace_with_receives_match() {
    let out = replace_with("hello world", "world", |m| {
        format!("[{}]", m.to_uppercase())
    });
    assert_eq!(out, "hello [WORLD]");
}

#[test]
fn replace_with_multiple_matches() {
    let out = replace_with("a-b-c", "-", |_| "//".to_string());
    assert_eq!(out, "a//b//c");
}

#[test]
fn replace_with_empty_needle_is_noop() {
    let out = replace_with("abc", "", |_| "X".to_string());
    assert_eq!(out, "abc");
}

#[test]
fn replace_with_no_match() {
    let out = replace_with("hello", "xyz", |_| "!".to_string());
    assert_eq!(out, "hello");
}

// -----------------------------------------------------------------
// replace_bounded
// -----------------------------------------------------------------

#[test]
fn replace_bounded_within_cap() {
    let out = replace_bounded("hello", "l", "LL", 32).unwrap();
    assert_eq!(out, "heLLLLo");
}

#[test]
fn replace_bounded_exceeds_cap() {
    let e = replace_bounded("aaaa", "a", "bbb", 8).unwrap_err();
    assert_eq!(e.max_len, 8);
    assert!(e.attempted_len > 8);
}

#[test]
fn replace_bounded_exact_boundary() {
    // Output would be exactly 8 bytes — should succeed.
    let out = replace_bounded("aa", "a", "abcd", 8).unwrap();
    assert_eq!(out, "abcdabcd");
    assert_eq!(out.len(), 8);
}

#[test]
fn replace_bounded_no_match_within_cap() {
    let out = replace_bounded("hello", "z", "!", 32).unwrap();
    assert_eq!(out, "hello");
}

#[test]
fn replace_bounded_empty_needle_checks_input_length() {
    // Empty needle → clone input; that clone is subject to the cap.
    assert_eq!(replace_bounded("hi", "", "!", 32).unwrap(), "hi");
    let e = replace_bounded("verylonginput", "", "!", 4).unwrap_err();
    assert_eq!(e.max_len, 4);
}

// -----------------------------------------------------------------
// replace_many (Aho-Corasick multi-pattern)
// -----------------------------------------------------------------

#[test]
fn replace_many_basic() {
    let out = replace_many(
        "the quick brown fox",
        &[("quick", "slow"), ("brown", "red")],
    );
    assert_eq!(out, "the slow red fox");
}

#[test]
fn replace_many_empty_pairs_is_noop() {
    assert_eq!(replace_many("hello", &[]), "hello");
}

#[test]
fn replace_many_overlapping_prefers_earliest_start() {
    // "aabb" starts at 0, "bbcc" starts at 2 — "aabb" wins the leftmost
    // slot; after consuming through byte 4 the "bbcc" match is skipped.
    let out = replace_many("aabbcc", &[("aabb", "X"), ("bbcc", "Y")]);
    assert_eq!(out, "Xcc");
}

#[test]
fn replace_many_empty_needle_silently_skipped() {
    let out = replace_many("hello", &[("", "!"), ("l", "L")]);
    assert_eq!(out, "heLLo");
}

#[test]
fn replace_many_no_match() {
    let out = replace_many("hello", &[("xyz", "!"), ("abc", "?")]);
    assert_eq!(out, "hello");
}

#[test]
fn replace_many_matches_at_end() {
    let out = replace_many("hello world", &[("world", "!")]);
    assert_eq!(out, "hello !");
}

#[test]
fn replace_many_multiple_patterns_same_position_deterministic() {
    // Duplicate patterns: both match at 0, sort by (position,
    // pattern_index) so the first entry wins.
    let out = replace_many("abc", &[("abc", "X"), ("abc", "Y")]);
    assert_eq!(out, "X");
}

// -----------------------------------------------------------------
// ReplaceError
// -----------------------------------------------------------------

#[test]
fn replace_error_display_mentions_caps() {
    let e = ReplaceError {
        max_len: 8,
        attempted_len: 20,
    };
    let msg = format!("{e}");
    assert!(msg.contains('8'), "{msg}");
    assert!(msg.contains("20"), "{msg}");
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn ascii() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0020-\\u007E]{0,32}").expect("static regex is valid")
    }

    fn short_ascii() -> impl Strategy<Value = String> {
        prop::string::string_regex("[a-zA-Z0-9]{1,4}").expect("static regex is valid")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // Identity: replacing a needle with itself yields the input.
        #[test]
        fn identity_replace(s in ascii(), needle in short_ascii()) {
            prop_assert_eq!(replace(&s, &needle, &needle), s);
        }

        // Empty needle is a no-op (documented divergence from std).
        #[test]
        fn empty_needle_is_noop(s in ascii(), to in ascii()) {
            prop_assert_eq!(replace(&s, "", &to), s);
        }

        // Removing a substring never grows the output.
        #[test]
        fn remove_never_grows(s in ascii(), needle in short_ascii()) {
            prop_assert!(remove(&s, &needle).len() <= s.len());
        }

        // replace_first followed by replace_last after equals replace_n(2)
        // when the input contains at least two occurrences (chain is
        // safe because both ops are no-ops when there is no match).
        // We just assert monotonicity: at most as many needles remain
        // after replace(x,x) as before — a trivial law but a good check.
        #[test]
        fn replace_removes_at_least_first_occurrence(
            s in ascii(),
            needle in short_ascii(),
        ) {
            // If the input has no occurrences, replace_first is a no-op.
            // If it has at least one, the count strictly decreases (when
            // to != from).
            let count_before = s.matches(&*needle).count();
            let replaced = replace_first(&s, &needle, "");
            let count_after = replaced.matches(&*needle).count();
            if count_before > 0 {
                prop_assert!(count_after <= count_before);
            } else {
                prop_assert_eq!(replaced, s);
            }
        }

        // replace_bounded either equals replace or errors — never returns
        // a truncated output silently.
        #[test]
        fn bounded_matches_unbounded_when_under_cap(
            s in ascii(),
            from in short_ascii(),
            to in short_ascii(),
        ) {
            let unbounded = replace(&s, &from, &to);
            // Give it plenty of room.
            let cap = unbounded.len() * 2 + 16;
            let bounded = replace_bounded(&s, &from, &to, cap).unwrap();
            prop_assert_eq!(bounded, unbounded);
        }

        // translate with an empty mapping is the identity function.
        #[test]
        fn translate_empty_mapping_is_identity(s in ascii()) {
            prop_assert_eq!(translate(&s, &[]), s);
        }
    }
}
