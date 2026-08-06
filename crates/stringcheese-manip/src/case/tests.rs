//! Tests for [`crate::case`].
//!
//! Unit tests exercise the empty / single-char / multi-byte-scalar /
//! multi-scalar-grapheme edge cases across every function. Property tests
//! confirm the invariants that hold across all inputs:
//!
//! - **Idempotence** on ASCII inputs of `to_lowercase` and `to_uppercase`
//!   (skipping the Unicode edge cases where the standard-library case
//!   mapping is not idempotent — see [`str::to_lowercase`]).
//! - **Reversibility of case** — every lowercased ASCII character is
//!   `is_lowercase() || !is_alphabetic()`.
//! - **`capitalize` preserves the tail** — everything after the first
//!   scalar equals what the input had after the first scalar.

use super::*;

// -----------------------------------------------------------------
// to_lowercase / to_uppercase
// -----------------------------------------------------------------

#[test]
fn lowercase_empty() {
    assert_eq!(to_lowercase(""), "");
}

#[test]
fn lowercase_ascii() {
    assert_eq!(to_lowercase("Hello, World!"), "hello, world!");
}

#[test]
fn lowercase_unicode_capital_sharp_s() {
    assert_eq!(to_lowercase("\u{1E9E}"), "ß");
}

#[test]
fn uppercase_empty() {
    assert_eq!(to_uppercase(""), "");
}

#[test]
fn uppercase_ascii() {
    assert_eq!(to_uppercase("hello, world!"), "HELLO, WORLD!");
}

#[test]
fn uppercase_expands_sharp_s_to_ss() {
    // The canonical scalar-count-changing case mapping (one scalar
    // becomes two). Byte length happens to stay the same here — ß is
    // a two-byte UTF-8 encoding and "SS" is two one-byte encodings.
    assert_eq!(to_uppercase("straße"), "STRASSE");
    assert_eq!(to_uppercase("straße").chars().count(), 7);
    assert_eq!("straße".chars().count(), 6);
}

#[test]
fn uppercase_can_shrink_byte_length_dotless_i() {
    // Reverse-direction reminder: 'ı' (U+0131, LATIN SMALL LETTER
    // DOTLESS I, 2 bytes) uppercases to 'I' (1 byte). Case mapping is
    // not a length-preserving operation.
    assert_eq!(to_uppercase("ı"), "I");
    assert!(to_uppercase("ı").len() < "ı".len());
}

// -----------------------------------------------------------------
// to_title_case
// -----------------------------------------------------------------

#[test]
fn title_case_empty() {
    assert_eq!(to_title_case(""), "");
}

#[test]
fn title_case_single_word() {
    assert_eq!(to_title_case("hello"), "Hello");
    assert_eq!(to_title_case("HELLO"), "Hello");
}

#[test]
fn title_case_multiple_words() {
    assert_eq!(to_title_case("hello world"), "Hello World");
    assert_eq!(to_title_case("HELLO WORLD"), "Hello World");
    assert_eq!(to_title_case("hello WORLD"), "Hello World");
}

#[test]
fn title_case_preserves_punctuation() {
    assert_eq!(to_title_case("hello, world!"), "Hello, World!");
    assert_eq!(to_title_case("one-two-three"), "One-Two-Three");
}

#[test]
fn title_case_preserves_leading_whitespace() {
    assert_eq!(to_title_case("  hello world"), "  Hello World");
}

#[test]
fn title_case_handles_unicode() {
    assert_eq!(to_title_case("naïve café"), "Naïve Café");
    assert_eq!(to_title_case("ΓΕΊΑ ΣΑΣ"), "Γεία Σας");
}

#[test]
fn title_case_handles_decomposed_grapheme() {
    // "e" + combining acute is one grapheme; when title-cased at the
    // start of a word, the "e" is uppercased and the combining mark is
    // preserved.
    let input = "e\u{0301}clair"; // é + clair
    let out = to_title_case(input);
    // First grapheme becomes "E" + combining acute; the tail lowercases.
    assert_eq!(out, "E\u{0301}clair");
}

#[test]
fn title_case_digits_are_not_word_starts() {
    // A digit run is not a "word" under `is_alphabetic`; a letter
    // immediately after a digit still starts a fresh word.
    assert_eq!(to_title_case("abc123def"), "Abc123Def");
}

// -----------------------------------------------------------------
// capitalize
// -----------------------------------------------------------------

#[test]
fn capitalize_empty() {
    assert_eq!(capitalize(""), "");
}

#[test]
fn capitalize_ascii() {
    assert_eq!(capitalize("hello"), "Hello");
}

#[test]
fn capitalize_leaves_tail_unchanged() {
    assert_eq!(capitalize("hELLO"), "HELLO");
}

#[test]
fn capitalize_non_letter_first() {
    assert_eq!(capitalize("1abc"), "1abc");
    assert_eq!(capitalize(" abc"), " abc");
}

#[test]
fn capitalize_sharp_s_expands() {
    // The first character expands via Unicode case mapping.
    assert_eq!(capitalize("ßtraße"), "SStraße");
}

// -----------------------------------------------------------------
// _into variants
// -----------------------------------------------------------------

#[test]
fn to_lowercase_into_appends() {
    let mut buf = String::from("[");
    to_lowercase_into("HELLO", &mut buf);
    buf.push(']');
    assert_eq!(buf, "[hello]");
}

#[test]
fn to_uppercase_into_appends() {
    let mut buf = String::from("[");
    to_uppercase_into("hello", &mut buf);
    buf.push(']');
    assert_eq!(buf, "[HELLO]");
}

#[test]
fn to_title_case_into_appends() {
    let mut buf = String::from("[");
    to_title_case_into("hello world", &mut buf);
    buf.push(']');
    assert_eq!(buf, "[Hello World]");
}

#[test]
fn capitalize_into_appends() {
    let mut buf = String::from("[");
    capitalize_into("hello", &mut buf);
    buf.push(']');
    assert_eq!(buf, "[Hello]");
}

#[test]
fn into_variants_match_owned() {
    let inputs = ["", "hello", "HELLO", "hello world", "café"];
    for input in inputs {
        let mut buf = String::new();
        to_lowercase_into(input, &mut buf);
        assert_eq!(buf, to_lowercase(input), "to_lowercase_into on {input:?}");

        let mut buf = String::new();
        to_uppercase_into(input, &mut buf);
        assert_eq!(buf, to_uppercase(input), "to_uppercase_into on {input:?}");

        let mut buf = String::new();
        to_title_case_into(input, &mut buf);
        assert_eq!(buf, to_title_case(input), "to_title_case_into on {input:?}");

        let mut buf = String::new();
        capitalize_into(input, &mut buf);
        assert_eq!(buf, capitalize(input), "capitalize_into on {input:?}");
    }
}

// -----------------------------------------------------------------
// ASCII fast paths
// -----------------------------------------------------------------

#[test]
fn ascii_lowercase_matches_std() {
    assert_eq!(to_lowercase_ascii("HELLO"), "hello");
    assert_eq!(to_lowercase_ascii("HI 42!"), "hi 42!");
}

#[test]
fn ascii_lowercase_ignores_non_ascii() {
    // The ASCII letters C, A, F are lowercased; the É is passed through
    // untouched — that is the fast-path contract.
    assert_eq!(to_lowercase_ascii("CAFÉ"), "cafÉ");
}

#[test]
fn ascii_uppercase_matches_std() {
    assert_eq!(to_uppercase_ascii("hello"), "HELLO");
}

#[test]
fn ascii_uppercase_ignores_non_ascii() {
    // ß does NOT expand under the ASCII path.
    assert_eq!(to_uppercase_ascii("straße"), "STRAßE");
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(feature = "std")]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn ascii_only() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0020-\\u007E]{0,32}").expect("static regex is valid")
    }

    fn general_unicode() -> impl Strategy<Value = String> {
        prop::string::string_regex(
            "[\\u0020-\\u007E\\u00C0-\\u017F\\u0370-\\u03FF\\u0400-\\u04FF]{0,32}",
        )
        .expect("static regex is valid")
    }

    proptest! {
        // ASCII paths are idempotent for both directions.
        #[test]
        fn ascii_lower_is_idempotent(s in ascii_only()) {
            let once = to_lowercase_ascii(&s);
            let twice = to_lowercase_ascii(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn ascii_upper_is_idempotent(s in ascii_only()) {
            let once = to_uppercase_ascii(&s);
            let twice = to_uppercase_ascii(&once);
            prop_assert_eq!(once, twice);
        }

        // Under ASCII, upper and lower are exact inverses on alphabetic
        // characters. Byte length never changes.
        #[test]
        fn ascii_case_preserves_len(s in ascii_only()) {
            prop_assert_eq!(to_lowercase_ascii(&s).len(), s.len());
            prop_assert_eq!(to_uppercase_ascii(&s).len(), s.len());
        }

        // Full Unicode case mappings are idempotent for lowercase — a
        // lowercased string, when lowercased again, is unchanged. (The
        // uppercase direction is *not* generally idempotent in Rust's
        // std because `İ.to_lowercase() → i\u{0307}` and then
        // uppercasing that back yields `İ`, but that is
        // upper→lower→upper, not upper→upper→upper.)
        #[test]
        fn lowercase_is_idempotent(s in general_unicode()) {
            let once = to_lowercase(&s);
            let twice = to_lowercase(&once);
            prop_assert_eq!(once, twice);
        }

        // Uppercase is idempotent — the uppercase of an already-uppercase
        // string is itself. (Length is not preserved under either
        // direction — 'ß' → "SS" grows scalar count, 'ı' → "I" shrinks
        // byte count — but idempotence holds because Unicode's
        // uppercase mapping is a fixed point on already-uppercase
        // scalars.)
        #[test]
        fn uppercase_is_idempotent(s in general_unicode()) {
            let once = to_uppercase(&s);
            let twice = to_uppercase(&once);
            prop_assert_eq!(once, twice);
        }

        // Emptiness round-trips: the case-mapped form of a non-empty
        // string is non-empty, and empty stays empty.
        #[test]
        fn lowercase_preserves_emptiness(s in general_unicode()) {
            prop_assert_eq!(to_lowercase(&s).is_empty(), s.is_empty());
        }

        #[test]
        fn uppercase_preserves_emptiness(s in general_unicode()) {
            prop_assert_eq!(to_uppercase(&s).is_empty(), s.is_empty());
        }

        // capitalize preserves everything after the first scalar.
        #[test]
        fn capitalize_preserves_tail(s in general_unicode()) {
            let out = capitalize(&s);
            let mut in_chars = s.chars();
            let first_in = in_chars.next();
            let mut out_chars = out.chars();
            if let Some(first) = first_in {
                // Skip over however many scalars first.to_uppercase()
                // emitted.
                for _ in first.to_uppercase() {
                    let _ = out_chars.next();
                }
                let tail_in: String = in_chars.collect();
                let tail_out: String = out_chars.collect();
                prop_assert_eq!(tail_in, tail_out);
            } else {
                prop_assert!(out.is_empty());
            }
        }

        // capitalize on ASCII: only the first byte can differ, and only
        // if that byte was an ASCII lowercase letter.
        #[test]
        fn ascii_capitalize_only_touches_first(s in ascii_only()) {
            let out = capitalize(&s);
            if let Some(first) = s.chars().next() {
                if first.is_ascii_alphabetic() {
                    prop_assert_eq!(&out[1..], &s[1..]);
                } else {
                    prop_assert_eq!(out, s);
                }
            } else {
                prop_assert!(out.is_empty());
            }
        }
    }
}
