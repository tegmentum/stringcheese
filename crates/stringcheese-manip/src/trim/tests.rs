//! Tests for [`crate::trim`].
//!
//! Unit tests exercise the empty / all-whitespace / single-char /
//! multi-byte-scalar edge cases across every free function and the
//! configured [`Trim`] operation. Property tests confirm the two laws
//! that every trim variant must obey:
//!
//! - **Idempotence.** `trim(trim(s)) == trim(s)` for every input.
//! - **Never grows.** The output's byte length is `<=` the input's.

use super::*;

// -----------------------------------------------------------------
// Whitespace variants
// -----------------------------------------------------------------

#[test]
fn trim_empty_is_empty() {
    assert_eq!(trim(""), "");
    assert_eq!(trim_start(""), "");
    assert_eq!(trim_end(""), "");
}

#[test]
fn trim_all_whitespace_is_empty() {
    assert_eq!(trim("   "), "");
    assert_eq!(trim("\t\n\r "), "");
    // U+00A0 no-break space is whitespace under Unicode.
    assert_eq!(trim("\u{00A0}"), "");
}

#[test]
fn trim_symmetric() {
    assert_eq!(trim("  hi  "), "hi");
}

#[test]
fn trim_start_only() {
    assert_eq!(trim_start("  hi  "), "hi  ");
}

#[test]
fn trim_end_only() {
    assert_eq!(trim_end("  hi  "), "  hi");
}

#[test]
fn trim_leaves_internal_whitespace() {
    assert_eq!(trim(" hello world "), "hello world");
}

// -----------------------------------------------------------------
// Predicate variants
// -----------------------------------------------------------------

#[test]
fn trim_matches_ascii() {
    assert_eq!(trim_matches("///path///", |c: char| c == '/'), "path");
}

#[test]
fn trim_matches_no_match_is_unchanged() {
    assert_eq!(trim_matches("hello", |c: char| c == 'z'), "hello");
}

#[test]
fn trim_matches_all_match_is_empty() {
    assert_eq!(trim_matches("aaaa", |c: char| c == 'a'), "");
}

#[test]
fn trim_start_matches_only_leading() {
    assert_eq!(trim_start_matches("aabbaa", |c: char| c == 'a'), "bbaa");
}

#[test]
fn trim_end_matches_only_trailing() {
    assert_eq!(trim_end_matches("aabbaa", |c: char| c == 'a'), "aabb");
}

#[test]
fn trim_matches_on_multi_byte_scalar() {
    // Strip a non-ASCII scalar (Greek small letter alpha).
    assert_eq!(trim_matches("ααhelloαα", |c: char| c == 'α'), "hello");
}

// -----------------------------------------------------------------
// Char-set variants
// -----------------------------------------------------------------

#[test]
fn trim_chars_multiple_chars() {
    assert_eq!(trim_chars(" \t hi \t ", &[' ', '\t']), "hi");
}

#[test]
fn trim_chars_empty_set_is_no_op() {
    assert_eq!(trim_chars("hello", &[]), "hello");
}

#[test]
fn trim_start_chars_only_leading() {
    assert_eq!(trim_start_chars("//a/b//", &['/']), "a/b//");
}

#[test]
fn trim_end_chars_only_trailing() {
    assert_eq!(trim_end_chars("//a/b//", &['/']), "//a/b");
}

// -----------------------------------------------------------------
// Configured operation
// -----------------------------------------------------------------

#[cfg(feature = "alloc")]
mod configured {
    use super::*;

    #[test]
    fn whitespace_trims_both_ends_by_default() {
        let ws = Trim::whitespace();
        assert_eq!(ws.apply("  hi  "), "hi");
    }

    #[test]
    fn edges_start_only() {
        let ws = Trim::whitespace().edges(TrimEdge::Start);
        assert_eq!(ws.apply("  hi  "), "hi  ");
    }

    #[test]
    fn edges_end_only() {
        let ws = Trim::whitespace().edges(TrimEdge::End);
        assert_eq!(ws.apply("  hi  "), "  hi");
    }

    #[test]
    fn chars_variant() {
        let quoted = Trim::chars(&['"', '\'']);
        assert_eq!(quoted.apply("\"hi\""), "hi");
        assert_eq!(quoted.apply("'hi'"), "hi");
        assert_eq!(quoted.apply("\"hi'"), "hi");
    }

    #[test]
    fn chars_variant_reusable() {
        // Building once and applying many times is the whole point.
        let slashes = Trim::chars(&['/']);
        assert_eq!(slashes.apply("//a//"), "a");
        assert_eq!(slashes.apply("/b/"), "b");
        assert_eq!(slashes.apply("noslashes"), "noslashes");
    }

    #[test]
    fn predicate_variant() {
        let digits = Trim::predicate(|c: char| c.is_ascii_digit());
        assert_eq!(digits.apply("42hello99"), "hello");
    }

    #[test]
    fn predicate_variant_edges() {
        let digits = Trim::predicate(|c: char| c.is_ascii_digit()).edges(TrimEdge::Start);
        assert_eq!(digits.apply("42hello99"), "hello99");
    }

    #[test]
    fn trim_debug_mentions_strategy_and_edge() {
        let ws = Trim::whitespace();
        let s = alloc::format!("{ws:?}");
        assert!(s.contains("Whitespace"), "{s}");
        assert!(s.contains("Both"), "{s}");

        let cs = Trim::chars(&['/']).edges(TrimEdge::End);
        let s = alloc::format!("{cs:?}");
        assert!(s.contains("Chars"), "{s}");
        assert!(s.contains("End"), "{s}");

        let pr = Trim::predicate(|_| true);
        let s = alloc::format!("{pr:?}");
        assert!(s.contains("Predicate"), "{s}");
    }
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn general_unicode() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0000-\\u007F\\u00A0-\\u017F\\u2000-\\u200F\\u3000]{0,32}")
            .expect("static regex is valid")
    }

    proptest! {
        // trim is idempotent.
        #[test]
        fn trim_is_idempotent(s in general_unicode()) {
            let once = trim(&s);
            let twice = trim(once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn trim_start_is_idempotent(s in general_unicode()) {
            let once = trim_start(&s);
            let twice = trim_start(once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn trim_end_is_idempotent(s in general_unicode()) {
            let once = trim_end(&s);
            let twice = trim_end(once);
            prop_assert_eq!(once, twice);
        }

        // trim never grows the string.
        #[test]
        fn trim_never_grows(s in general_unicode()) {
            prop_assert!(trim(&s).len() <= s.len());
            prop_assert!(trim_start(&s).len() <= s.len());
            prop_assert!(trim_end(&s).len() <= s.len());
        }

        // Charset trim never grows either.
        #[test]
        fn trim_chars_never_grows(s in general_unicode()) {
            let chars = [' ', '\t', '\n'];
            prop_assert!(trim_chars(&s, &chars).len() <= s.len());
        }

        // Configured Trim matches free-function trim for whitespace / both.
        #[test]
        fn configured_whitespace_matches_free_fn(s in general_unicode()) {
            let policy = Trim::whitespace();
            prop_assert_eq!(policy.apply(&s), trim(&s));
        }

        // Configured Trim start/end policies agree with the free
        // functions.
        #[test]
        fn configured_whitespace_start_matches(s in general_unicode()) {
            let policy = Trim::whitespace().edges(TrimEdge::Start);
            prop_assert_eq!(policy.apply(&s), trim_start(&s));
        }

        #[test]
        fn configured_whitespace_end_matches(s in general_unicode()) {
            let policy = Trim::whitespace().edges(TrimEdge::End);
            prop_assert_eq!(policy.apply(&s), trim_end(&s));
        }
    }
}
