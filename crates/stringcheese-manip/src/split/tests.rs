//! Tests for [`crate::split`].
//!
//! Unit tests cover the empty-input / no-match / all-match / leading-
//! and trailing-separator / multi-byte-scalar edge cases across every
//! variant. Property tests confirm the round-trip law
//! `split(join(items, sep), sep) == items` when `sep` does not itself
//! occur in any item.

use super::*;

// -----------------------------------------------------------------
// split / splitn / split_terminator / rsplit
// -----------------------------------------------------------------

#[test]
fn split_basic() {
    let parts: alloc::vec::Vec<&str> = split("a,b,c", ",").collect();
    assert_eq!(parts, ["a", "b", "c"]);
}

#[test]
fn split_empty_input() {
    // Empty input with a non-empty separator yields exactly one empty
    // item — matches `str::split` semantics.
    let parts: alloc::vec::Vec<&str> = split("", ",").collect();
    assert_eq!(parts, [""]);
}

#[test]
fn split_no_match() {
    let parts: alloc::vec::Vec<&str> = split("abc", ",").collect();
    assert_eq!(parts, ["abc"]);
}

#[test]
fn split_leading_separator_yields_empty_head() {
    let parts: alloc::vec::Vec<&str> = split(",a,b", ",").collect();
    assert_eq!(parts, ["", "a", "b"]);
}

#[test]
fn split_trailing_separator_yields_empty_tail() {
    let parts: alloc::vec::Vec<&str> = split("a,b,", ",").collect();
    assert_eq!(parts, ["a", "b", ""]);
}

#[test]
fn split_consecutive_separators_yield_empty_middle() {
    let parts: alloc::vec::Vec<&str> = split("a,,b", ",").collect();
    assert_eq!(parts, ["a", "", "b"]);
}

#[test]
fn split_multi_char_separator() {
    let parts: alloc::vec::Vec<&str> = split("a::b::c", "::").collect();
    assert_eq!(parts, ["a", "b", "c"]);
}

#[test]
fn split_multi_byte_scalar_separator() {
    // Split at a non-ASCII scalar (Greek middle dot).
    let parts: alloc::vec::Vec<&str> = split("a\u{00B7}b\u{00B7}c", "\u{00B7}").collect();
    assert_eq!(parts, ["a", "b", "c"]);
}

// -----------------------------------------------------------------
// split_whitespace
// -----------------------------------------------------------------

#[test]
fn split_whitespace_basic() {
    let parts: alloc::vec::Vec<&str> = split_whitespace("hello world").collect();
    assert_eq!(parts, ["hello", "world"]);
}

#[test]
fn split_whitespace_collapses_runs() {
    let parts: alloc::vec::Vec<&str> = split_whitespace("  a   b   c  ").collect();
    assert_eq!(parts, ["a", "b", "c"]);
}

#[test]
fn split_whitespace_all_whitespace_is_empty() {
    let parts: alloc::vec::Vec<&str> = split_whitespace("   \t\n  ").collect();
    assert!(parts.is_empty());
}

#[test]
fn split_whitespace_unicode() {
    // U+00A0 NBSP and U+3000 IDEOGRAPHIC SPACE both count as whitespace.
    let parts: alloc::vec::Vec<&str> = split_whitespace("a\u{00A0}b\u{3000}c").collect();
    assert_eq!(parts, ["a", "b", "c"]);
}

// -----------------------------------------------------------------
// split_matches
// -----------------------------------------------------------------

#[test]
fn split_matches_basic() {
    let parts: alloc::vec::Vec<&str> =
        split_matches("a1b2c3", |c: char| c.is_ascii_digit()).collect();
    assert_eq!(parts, ["a", "b", "c", ""]);
}

#[test]
fn split_matches_no_match() {
    let parts: alloc::vec::Vec<&str> = split_matches("hello", |c: char| c == '/').collect();
    assert_eq!(parts, ["hello"]);
}

#[test]
fn split_matches_all_match() {
    // Every scalar matches — an empty fragment appears between each
    // adjacent match plus one on each end.
    let parts: alloc::vec::Vec<&str> = split_matches("abc", |_: char| true).collect();
    assert_eq!(parts, ["", "", "", ""]);
}

// -----------------------------------------------------------------
// split_terminator
// -----------------------------------------------------------------

#[test]
fn split_terminator_trailing_separator_dropped() {
    let parts: alloc::vec::Vec<&str> = split_terminator("a,b,c,", ",").collect();
    assert_eq!(parts, ["a", "b", "c"]);
}

#[test]
fn split_terminator_no_trailing_matches_split() {
    let parts: alloc::vec::Vec<&str> = split_terminator("a,b,c", ",").collect();
    assert_eq!(parts, ["a", "b", "c"]);
}

#[test]
fn split_terminator_leading_still_yields_empty() {
    let parts: alloc::vec::Vec<&str> = split_terminator(",a,b", ",").collect();
    assert_eq!(parts, ["", "a", "b"]);
}

// -----------------------------------------------------------------
// splitn
// -----------------------------------------------------------------

#[test]
fn splitn_cap() {
    let parts: alloc::vec::Vec<&str> = splitn("a,b,c,d", 2, ",").collect();
    assert_eq!(parts, ["a", "b,c,d"]);
}

#[test]
fn splitn_zero_yields_nothing() {
    let parts: alloc::vec::Vec<&str> = splitn("a,b,c", 0, ",").collect();
    assert!(parts.is_empty());
}

#[test]
fn splitn_one_yields_whole_input() {
    let parts: alloc::vec::Vec<&str> = splitn("a,b,c", 1, ",").collect();
    assert_eq!(parts, ["a,b,c"]);
}

#[test]
fn splitn_more_than_needed_matches_split() {
    let parts_n: alloc::vec::Vec<&str> = splitn("a,b,c", 99, ",").collect();
    let parts: alloc::vec::Vec<&str> = split("a,b,c", ",").collect();
    assert_eq!(parts_n, parts);
}

// -----------------------------------------------------------------
// rsplit, split_once, rsplit_once
// -----------------------------------------------------------------

#[test]
fn rsplit_reverse_order() {
    let parts: alloc::vec::Vec<&str> = rsplit("a,b,c", ",").collect();
    assert_eq!(parts, ["c", "b", "a"]);
}

#[test]
fn split_once_matches() {
    assert_eq!(split_once("key=value", "="), Some(("key", "value")));
    assert_eq!(split_once("key=", "="), Some(("key", "")));
    assert_eq!(split_once("=value", "="), Some(("", "value")));
}

#[test]
fn split_once_no_match() {
    assert_eq!(split_once("nokey", "="), None);
}

#[test]
fn rsplit_once_matches() {
    assert_eq!(rsplit_once("a.b.c", "."), Some(("a.b", "c")));
    assert_eq!(rsplit_once("only", "."), None);
}

// -----------------------------------------------------------------
// split_lines
// -----------------------------------------------------------------

#[test]
fn split_lines_lf_and_crlf() {
    let parts: alloc::vec::Vec<&str> = split_lines("a\nb\r\nc").collect();
    assert_eq!(parts, ["a", "b", "c"]);
}

#[test]
fn split_lines_trailing_newline_no_empty_final() {
    let parts: alloc::vec::Vec<&str> = split_lines("a\nb\n").collect();
    assert_eq!(parts, ["a", "b"]);
}

#[test]
fn split_lines_empty() {
    let parts: alloc::vec::Vec<&str> = split_lines("").collect();
    assert!(parts.is_empty());
}

// -----------------------------------------------------------------
// split_graphemes
// -----------------------------------------------------------------

#[cfg(feature = "alloc")]
mod grapheme_tests {
    use super::*;

    #[test]
    fn split_graphemes_ascii() {
        let parts: alloc::vec::Vec<&str> = split_graphemes("abc").collect();
        assert_eq!(parts, ["a", "b", "c"]);
    }

    #[test]
    fn split_graphemes_precomposed_and_decomposed() {
        // Precomposed é is one grapheme, decomposed é (e + combining
        // acute) is also one grapheme.
        let precomposed: alloc::vec::Vec<&str> = split_graphemes("caf\u{00E9}").collect();
        let decomposed: alloc::vec::Vec<&str> = split_graphemes("cafe\u{0301}").collect();
        assert_eq!(precomposed.len(), 4);
        assert_eq!(decomposed.len(), 4);
        assert_eq!(precomposed[3], "\u{00E9}");
        assert_eq!(decomposed[3], "e\u{0301}");
    }

    #[test]
    fn split_graphemes_flag_is_one_item() {
        let parts: alloc::vec::Vec<&str> = split_graphemes("\u{1F1EC}\u{1F1E7}").collect();
        assert_eq!(parts, ["\u{1F1EC}\u{1F1E7}"]);
    }

    #[test]
    fn split_graphemes_empty() {
        let parts: alloc::vec::Vec<&str> = split_graphemes("").collect();
        assert!(parts.is_empty());
    }
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn ascii_no_comma() -> impl Strategy<Value = String> {
        // ASCII printable minus comma so the separator never occurs inside
        // an item.
        prop::string::string_regex("[\\u0020-\\u002B\\u002D-\\u007E]{0,16}")
            .expect("static regex is valid")
    }

    fn general_unicode() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0020-\\u007E\\u00A0-\\u017F]{0,32}")
            .expect("static regex is valid")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // Round-trip: splitting the string that `join` builds returns the
        // original items (as long as no item contains the separator).
        #[test]
        fn split_round_trip(items in proptest::collection::vec(ascii_no_comma(), 0..8)) {
            let joined = items.join(",");
            let back: Vec<&str> = split(&joined, ",").collect();
            if items.is_empty() {
                // Joining an empty list gives an empty string; splitting
                // that yields one empty item.
                prop_assert_eq!(back, vec![""]);
            } else {
                let expected: Vec<&str> = items.iter().map(String::as_str).collect();
                prop_assert_eq!(back, expected);
            }
        }

        // split and rsplit produce the same set of items (just in
        // reverse order).
        #[test]
        fn split_and_rsplit_agree(s in general_unicode()) {
            let sep = " ";
            let mut fwd: Vec<&str> = split(&s, sep).collect();
            let rev: Vec<&str> = rsplit(&s, sep).collect();
            fwd.reverse();
            prop_assert_eq!(fwd, rev);
        }

        // split_terminator matches split, except that a trailing empty
        // fragment (if any) is suppressed.
        #[test]
        fn split_terminator_drops_trailing_empty(s in general_unicode()) {
            let sep = " ";
            let full: Vec<&str> = split(&s, sep).collect();
            let term: Vec<&str> = split_terminator(&s, sep).collect();
            if full.last().copied() == Some("") && !full.is_empty() {
                prop_assert_eq!(&term[..], &full[..full.len() - 1]);
            } else {
                prop_assert_eq!(term, full);
            }
        }

        // splitn caps the piece count at n.
        #[test]
        fn splitn_never_exceeds_cap(s in general_unicode(), n in 0usize..8) {
            let parts: Vec<&str> = splitn(&s, n, " ").collect();
            prop_assert!(parts.len() <= n);
        }

        // Grapheme splitting concatenated back equals the original.
        #[test]
        fn grapheme_split_reconstructs_input(s in general_unicode()) {
            let parts: String = split_graphemes(&s).collect();
            prop_assert_eq!(parts, s);
        }
    }
}
