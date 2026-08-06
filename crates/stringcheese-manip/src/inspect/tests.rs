//! Tests for [`crate::inspect`].
//!
//! Unit tests exercise the empty-string, single-char, multi-byte-scalar,
//! and multi-scalar-grapheme edge cases explicitly. Property tests confirm
//! the ordering `bytes >= scalars >= graphemes` that every valid UTF-8
//! string obeys, as well as the emptiness / first-character laws.

use super::*;

#[test]
fn is_empty_on_empty() {
    assert!(is_empty(""));
}

#[test]
fn is_empty_on_ascii() {
    assert!(!is_empty("x"));
    assert!(!is_empty("hello"));
}

#[test]
fn is_empty_on_non_ascii() {
    assert!(!is_empty("é"));
    assert!(!is_empty("🇬🇧"));
}

#[test]
fn byte_len_matches_str_len() {
    assert_eq!(byte_len(""), 0);
    assert_eq!(byte_len("hello"), 5);
    assert_eq!(byte_len("é"), 2);
    assert_eq!(byte_len("🇬🇧"), 8);
}

#[test]
fn scalar_count_matches_chars_count() {
    assert_eq!(scalar_count(""), 0);
    assert_eq!(scalar_count("hello"), 5);
    // Precomposed é is one scalar (U+00E9).
    assert_eq!(scalar_count("\u{00E9}"), 1);
    // Decomposed é is two scalars (U+0065, U+0301).
    assert_eq!(scalar_count("e\u{0301}"), 2);
    // Family emoji: five scalars, one grapheme.
    assert_eq!(
        scalar_count("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"),
        5
    );
}

#[test]
fn first_char_on_empty_is_none() {
    assert_eq!(first_char(""), None);
}

#[test]
fn first_char_on_ascii() {
    assert_eq!(first_char("hello"), Some('h'));
}

#[test]
fn first_char_on_multi_byte() {
    assert_eq!(first_char("éclair"), Some('é'));
}

#[test]
fn last_char_on_empty_is_none() {
    assert_eq!(last_char(""), None);
}

#[test]
fn last_char_on_ascii() {
    assert_eq!(last_char("hello"), Some('o'));
}

#[test]
fn last_char_on_decomposed_grapheme_yields_combining_mark() {
    // Documented boundary behaviour — the last *scalar* of "e\u{0301}"
    // is the combining acute, not the base letter.
    assert_eq!(last_char("e\u{0301}"), Some('\u{0301}'));
}

#[cfg(feature = "alloc")]
mod grapheme_tests {
    use super::*;

    #[test]
    fn grapheme_count_on_empty() {
        assert_eq!(grapheme_count(""), 0);
    }

    #[test]
    fn grapheme_count_on_ascii() {
        assert_eq!(grapheme_count("hello"), 5);
    }

    #[test]
    fn grapheme_count_precomposed_and_decomposed_agree() {
        // "café" as one precomposed é and as e + combining acute both
        // count as four graphemes.
        assert_eq!(grapheme_count("caf\u{00E9}"), 4);
        assert_eq!(grapheme_count("cafe\u{0301}"), 4);
    }

    #[test]
    fn grapheme_count_uk_flag_is_one() {
        assert_eq!(grapheme_count("\u{1F1EC}\u{1F1E7}"), 1);
    }

    #[test]
    fn grapheme_count_family_emoji_is_one() {
        assert_eq!(
            grapheme_count("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"),
            1
        );
    }

    #[test]
    fn first_grapheme_on_empty_is_none() {
        assert_eq!(first_grapheme(""), None);
    }

    #[test]
    fn first_grapheme_on_ascii() {
        assert_eq!(first_grapheme("hello"), Some("h"));
    }

    #[test]
    fn first_grapheme_returns_whole_cluster() {
        // The decomposed é is one grapheme with two scalars.
        assert_eq!(first_grapheme("e\u{0301}bc"), Some("e\u{0301}"));
    }

    #[test]
    fn last_grapheme_on_empty_is_none() {
        assert_eq!(last_grapheme(""), None);
    }

    #[test]
    fn last_grapheme_on_ascii() {
        assert_eq!(last_grapheme("hello"), Some("o"));
    }

    #[test]
    fn last_grapheme_returns_whole_cluster() {
        assert_eq!(last_grapheme("abe\u{0301}"), Some("e\u{0301}"));
    }

    #[test]
    fn last_grapheme_flag_is_the_flag() {
        // Only one grapheme in the input.
        assert_eq!(
            last_grapheme("\u{1F1EC}\u{1F1E7}"),
            Some("\u{1F1EC}\u{1F1E7}")
        );
    }
}

// Property tests. Only compiled when `std` is enabled because `proptest`
// (in its default configuration) needs `std`. Tests run under
// `cargo test`, which enables the default features.
#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn general_unicode() -> impl Strategy<Value = String> {
        prop::string::string_regex(
            "[\\u0000-\\u007F\\u00C0-\\u017F\\u0300-\\u036F\\u0370-\\u03FF\\u1F1E6-\\u1F1FF]{0,32}",
        )
        .expect("static regex is valid")
    }

    proptest! {
        // The load-bearing shape of these three counts on any UTF-8 string.
        #[test]
        fn bytes_ge_scalars_ge_graphemes(s in general_unicode()) {
            prop_assert!(byte_len(&s) >= scalar_count(&s));
            prop_assert!(scalar_count(&s) >= grapheme_count(&s));
        }

        #[test]
        fn is_empty_matches_str_is_empty(s in general_unicode()) {
            prop_assert_eq!(is_empty(&s), s.is_empty());
        }

        #[test]
        fn first_char_some_iff_non_empty(s in general_unicode()) {
            prop_assert_eq!(first_char(&s).is_some(), !s.is_empty());
        }

        #[test]
        fn last_char_some_iff_non_empty(s in general_unicode()) {
            prop_assert_eq!(last_char(&s).is_some(), !s.is_empty());
        }

        #[test]
        fn first_grapheme_some_iff_non_empty(s in general_unicode()) {
            prop_assert_eq!(first_grapheme(&s).is_some(), !s.is_empty());
        }

        #[test]
        fn last_grapheme_some_iff_non_empty(s in general_unicode()) {
            prop_assert_eq!(last_grapheme(&s).is_some(), !s.is_empty());
        }

        #[test]
        fn byte_len_matches_str_len(s in general_unicode()) {
            prop_assert_eq!(byte_len(&s), s.len());
        }
    }
}
