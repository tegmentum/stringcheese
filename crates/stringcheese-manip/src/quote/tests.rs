//! Tests for [`crate::quote`].
//!
//! Unit tests exercise the empty / no-escape-needed / embedded-delimiter
//! edge cases for every quoter, plus the fixed-versus-parameterised
//! agreement. Property tests confirm the round-trip
//! `unquote(double_quote(s)) == Some(s)` on inputs that do not require
//! escaping — the only shape in which `unquote`'s zero-alloc contract
//! can round-trip.

use super::*;

// -----------------------------------------------------------------
// Fixed-delimiter quoters
// -----------------------------------------------------------------

#[cfg(feature = "alloc")]
mod fixed {
    use super::*;

    #[test]
    fn single_quote_empty() {
        assert_eq!(single_quote(""), "''");
    }

    #[test]
    fn single_quote_basic() {
        assert_eq!(single_quote("hi"), "'hi'");
    }

    #[test]
    fn single_quote_escapes_embedded_single() {
        assert_eq!(single_quote("it's"), "'it\\'s'");
        // Multiple embedded quotes.
        assert_eq!(single_quote("'a'"), "'\\'a\\''");
    }

    #[test]
    fn single_quote_escapes_backslash() {
        assert_eq!(single_quote("a\\b"), "'a\\\\b'");
    }

    #[test]
    fn double_quote_empty() {
        assert_eq!(double_quote(""), "\"\"");
    }

    #[test]
    fn double_quote_basic() {
        assert_eq!(double_quote("hi"), "\"hi\"");
    }

    #[test]
    fn double_quote_escapes_embedded_double() {
        assert_eq!(double_quote("say \"hi\""), "\"say \\\"hi\\\"\"");
    }

    #[test]
    fn backtick_quote_empty_and_basic() {
        assert_eq!(backtick_quote(""), "``");
        assert_eq!(backtick_quote("code"), "`code`");
    }

    #[test]
    fn backtick_quote_escapes_embedded_backtick() {
        assert_eq!(backtick_quote("`x`"), "`\\`x\\``");
    }

    #[test]
    fn angle_quote_empty_and_basic() {
        assert_eq!(angle_quote(""), "<>");
        assert_eq!(angle_quote("tag"), "<tag>");
    }

    #[test]
    fn angle_quote_escapes_both_delimiters() {
        assert_eq!(angle_quote("a<b>c"), "<a\\<b\\>c>");
    }

    #[test]
    fn angle_quote_escapes_backslash() {
        assert_eq!(angle_quote("a\\b"), "<a\\\\b>");
    }

    #[test]
    fn custom_quote_matches_double_quote() {
        for s in ["", "hi", "a\"b", "a\\b"] {
            assert_eq!(custom_quote(s, '"', '"', '\\'), double_quote(s), "on {s:?}");
        }
    }

    #[test]
    fn custom_quote_different_open_close() {
        assert_eq!(custom_quote("hi", '[', ']', '\\'), "[hi]");
        assert_eq!(custom_quote("[a]", '[', ']', '\\'), "[\\[a\\]]");
    }

    #[test]
    fn custom_quote_non_backslash_escape() {
        // Use `%` as the escape character.
        assert_eq!(custom_quote("[a]", '[', ']', '%'), "[%[a%]]");
        assert_eq!(custom_quote("a%b", '[', ']', '%'), "[a%%b]");
    }

    // -----------------------------------------------------------------
    // Curly (typographic) quoters
    // -----------------------------------------------------------------

    #[test]
    fn curly_double_wraps_verbatim() {
        assert_eq!(curly_quote(""), "\u{201C}\u{201D}");
        assert_eq!(curly_quote("hi"), "\u{201C}hi\u{201D}");
    }

    #[test]
    fn curly_single_wraps_verbatim() {
        assert_eq!(curly_single_quote(""), "\u{2018}\u{2019}");
        assert_eq!(curly_single_quote("hi"), "\u{2018}hi\u{2019}");
    }

    // -----------------------------------------------------------------
    // quote_smart
    // -----------------------------------------------------------------

    #[test]
    fn quote_smart_prefers_double_when_no_quotes() {
        assert_eq!(quote_smart("hello"), "\"hello\"");
    }

    #[test]
    fn quote_smart_switches_to_single_when_only_doubles_present() {
        assert_eq!(quote_smart("say \"hi\""), "'say \"hi\"'");
    }

    #[test]
    fn quote_smart_switches_to_backtick_when_it_wins() {
        // Two `"` and two `'` — a single backtick would need no escapes,
        // and the counts strictly favour backtick.
        assert_eq!(quote_smart("\"\"''"), "`\"\"''`");
    }

    #[test]
    fn quote_smart_ties_break_to_double() {
        // One `"`, one `'`, one `` ` `` — every choice needs exactly one
        // escape, so the tie breaks to double.
        assert_eq!(quote_smart("\"'`"), "\"\\\"'`\"");
    }

    #[test]
    fn quote_smart_picks_fewest_escapes() {
        // Two `"`, one `'`, zero `` ` `` — backtick wins.
        assert_eq!(quote_smart("it's \"good\""), "`it's \"good\"`");
    }

    // -----------------------------------------------------------------
    // Round-trip via unquote
    // -----------------------------------------------------------------

    #[test]
    fn round_trip_double_no_escape() {
        for s in ["", "hi", "hello world", "café"] {
            let q = double_quote(s);
            assert_eq!(unquote(&q), Some(s), "on {s:?}");
        }
    }

    #[test]
    fn round_trip_single_no_escape() {
        for s in ["", "hi", "hello", "abc"] {
            let q = single_quote(s);
            assert_eq!(unquote(&q), Some(s), "on {s:?}");
        }
    }

    #[test]
    fn round_trip_angle_no_escape() {
        for s in ["", "hi", "abc"] {
            let q = angle_quote(s);
            assert_eq!(unquote(&q), Some(s), "on {s:?}");
        }
    }

    #[test]
    fn round_trip_curly() {
        for s in ["", "hi", "café"] {
            let q = curly_quote(s);
            assert_eq!(unquote(&q), Some(s), "on {s:?}");
        }
    }
}

// -----------------------------------------------------------------
// unquote / is_quoted — available without `alloc`.
// -----------------------------------------------------------------

#[test]
fn unquote_empty_is_none() {
    assert_eq!(unquote(""), None);
    assert!(!is_quoted(""));
}

#[test]
fn unquote_single_char_is_none() {
    // A one-character input cannot possibly be a quote pair.
    assert_eq!(unquote("\""), None);
    assert_eq!(unquote("'"), None);
    assert_eq!(unquote("a"), None);
}

#[test]
fn unquote_double_pair() {
    assert_eq!(unquote("\"hi\""), Some("hi"));
    assert_eq!(unquote("\"\""), Some(""));
}

#[test]
fn unquote_single_pair() {
    assert_eq!(unquote("'hi'"), Some("hi"));
}

#[test]
fn unquote_backtick_pair() {
    assert_eq!(unquote("`code`"), Some("code"));
}

#[test]
fn unquote_angle_pair() {
    assert_eq!(unquote("<tag>"), Some("tag"));
}

#[test]
fn unquote_curly_pair() {
    assert_eq!(unquote("\u{201C}hi\u{201D}"), Some("hi"));
    assert_eq!(unquote("\u{2018}hi\u{2019}"), Some("hi"));
}

#[test]
fn unquote_mismatched_pair_is_none() {
    assert_eq!(unquote("\"hi'"), None);
    assert_eq!(unquote("<hi]"), None);
}

#[test]
fn unquote_no_delims_is_none() {
    assert_eq!(unquote("hello"), None);
    assert!(!is_quoted("hello"));
}

#[test]
fn unquote_does_not_unescape_interior() {
    // `\"` inside stays as two literal characters — the caller decides
    // how to interpret it.
    assert_eq!(unquote("\"a\\\"b\""), Some("a\\\"b"));
}

#[test]
fn is_quoted_agrees_with_unquote() {
    for s in ["\"hi\"", "'hi'", "`x`", "<t>", "hi", "", "\""] {
        assert_eq!(is_quoted(s), unquote(s).is_some(), "on {s:?}");
    }
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    // Inputs safe for round-tripping through double_quote — no `"` and
    // no `\` means no escaping will be introduced, so `unquote` (which
    // does not resolve escapes) will recover the original.
    fn safe_for_double() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0020-\\u0021\\u0023-\\u005B\\u005D-\\u007E]{0,32}")
            .expect("static regex is valid")
    }

    fn safe_for_single() -> impl Strategy<Value = String> {
        // No `'` (U+0027) and no `\` (U+005C).
        prop::string::string_regex("[\\u0020-\\u0026\\u0028-\\u005B\\u005D-\\u007E]{0,32}")
            .expect("static regex is valid")
    }

    fn safe_for_backtick() -> impl Strategy<Value = String> {
        // No `` ` `` (U+0060) and no `\` (U+005C).
        prop::string::string_regex("[\\u0020-\\u005B\\u005D-\\u005F\\u0061-\\u007E]{0,32}")
            .expect("static regex is valid")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn double_round_trip(s in safe_for_double()) {
            let q = double_quote(&s);
            prop_assert_eq!(unquote(&q), Some(s.as_str()));
        }

        #[test]
        fn single_round_trip(s in safe_for_single()) {
            let q = single_quote(&s);
            prop_assert_eq!(unquote(&q), Some(s.as_str()));
        }

        #[test]
        fn backtick_round_trip(s in safe_for_backtick()) {
            let q = backtick_quote(&s);
            prop_assert_eq!(unquote(&q), Some(s.as_str()));
        }

        // is_quoted is a total predicate — it never panics, and it
        // agrees with unquote.
        #[test]
        fn is_quoted_matches_unquote(s in safe_for_double()) {
            prop_assert_eq!(is_quoted(&s), unquote(&s).is_some());
        }

        // quote_smart always produces a value that unquote can strip
        // back to *something* — round-trip may differ after escaping,
        // but the outer pair is always recognisable.
        #[test]
        fn quote_smart_is_always_quoted(s in safe_for_double()) {
            let out = quote_smart(&s);
            prop_assert!(is_quoted(&out), "output {out:?} not recognised as quoted");
        }
    }
}
