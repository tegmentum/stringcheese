//! Tests for [`crate::pad`].
//!
//! Unit tests cover the empty-input, already-wide, odd-padding,
//! multi-byte-fill, and ASCII/Unicode input scenarios. Property tests
//! confirm the monotonic-length and inclusion invariants that hold
//! across all inputs.

use super::*;

// -----------------------------------------------------------------
// pad_left / pad_right (scalar width)
// -----------------------------------------------------------------

#[test]
fn pad_left_shorter_input() {
    assert_eq!(pad_left("hi", 5, ' '), "   hi");
}

#[test]
fn pad_left_at_target_length() {
    assert_eq!(pad_left("hi", 2, ' '), "hi");
}

#[test]
fn pad_left_longer_input_unchanged() {
    assert_eq!(pad_left("hello", 3, ' '), "hello");
}

#[test]
fn pad_left_zero_target() {
    assert_eq!(pad_left("hello", 0, ' '), "hello");
    assert_eq!(pad_left("", 0, ' '), "");
}

#[test]
fn pad_left_empty_input_is_all_fill() {
    assert_eq!(pad_left("", 3, 'x'), "xxx");
}

#[test]
fn pad_left_multi_byte_fill() {
    // 'é' is one scalar (two bytes), so padding two 'é's onto "x" gives
    // 3 scalars total.
    let out = pad_left("x", 3, 'é');
    assert_eq!(out.chars().count(), 3);
    assert_eq!(out, "ééx");
}

#[test]
fn pad_left_multi_byte_input() {
    // "café" is 4 scalars; pad to 6 with space adds 2 leading spaces.
    let out = pad_left("café", 6, ' ');
    assert_eq!(out.chars().count(), 6);
    assert_eq!(out, "  café");
}

#[test]
fn pad_right_shorter_input() {
    assert_eq!(pad_right("hi", 5, ' '), "hi   ");
}

#[test]
fn pad_right_at_target_length() {
    assert_eq!(pad_right("hi", 2, ' '), "hi");
}

#[test]
fn pad_right_longer_input_unchanged() {
    assert_eq!(pad_right("hello", 3, ' '), "hello");
}

#[test]
fn pad_right_empty_input_is_all_fill() {
    assert_eq!(pad_right("", 3, 'x'), "xxx");
}

// -----------------------------------------------------------------
// center (scalar width)
// -----------------------------------------------------------------

#[test]
fn center_even_padding() {
    assert_eq!(center("hi", 6, ' '), "  hi  ");
    assert_eq!(center("ab", 4, '_'), "_ab_");
}

#[test]
fn center_odd_padding_extra_on_right() {
    // "x" (1) padded to 4 needs 3 → left=1, right=2.
    assert_eq!(center("x", 4, '.'), ".x..");
    // "hi" (2) padded to 5 needs 3 → left=1, right=2.
    assert_eq!(center("hi", 5, '.'), ".hi..");
}

#[test]
fn center_at_target_length() {
    assert_eq!(center("hi", 2, ' '), "hi");
}

#[test]
fn center_longer_input_unchanged() {
    assert_eq!(center("hello", 3, ' '), "hello");
}

#[test]
fn center_empty_input_is_all_fill() {
    // pad=3, left=1, right=2.
    assert_eq!(center("", 3, 'x'), "xxx");
}

// -----------------------------------------------------------------
// Byte-width variants
// -----------------------------------------------------------------

#[test]
fn pad_left_bytes_ascii() {
    assert_eq!(pad_left_bytes("hi", 5, ' '), "   hi");
    assert_eq!(pad_left_bytes("hi", 2, ' '), "hi");
}

#[test]
fn pad_left_bytes_counts_bytes_not_scalars() {
    // "é" is one scalar but two bytes; pad to 4 bytes.
    let out = pad_left_bytes("é", 4, ' ');
    assert!(out.len() >= 4);
    assert!(out.ends_with("é"));
    // Byte count is exactly 4 (2 spaces + 2-byte 'é').
    assert_eq!(out.len(), 4);
}

#[test]
fn pad_left_bytes_multi_byte_fill_overshoots() {
    // "hi" is 2 bytes. Target 5. Need 3 more bytes. 'é' is 2 bytes,
    // so ceil(3/2) = 2 'é's = 4 bytes → total 6, over target by 1.
    let out = pad_left_bytes("hi", 5, 'é');
    assert!(out.len() >= 5);
    assert!(out.ends_with("hi"));
    // Exactly 2 'é' fills of 2 bytes each + 2 bytes of "hi" = 6.
    assert_eq!(out.len(), 6);
    assert_eq!(out.chars().filter(|&c| c == 'é').count(), 2);
}

#[test]
fn pad_right_bytes_basic() {
    assert_eq!(pad_right_bytes("hi", 5, ' '), "hi   ");
    assert_eq!(pad_right_bytes("hi", 2, ' '), "hi");
}

#[test]
fn center_bytes_even() {
    assert_eq!(center_bytes("hi", 6, ' '), "  hi  ");
}

#[test]
fn center_bytes_odd_right_heavy() {
    assert_eq!(center_bytes("x", 4, '.'), ".x..");
}

#[test]
fn center_bytes_longer_input_unchanged() {
    assert_eq!(center_bytes("hello", 3, ' '), "hello");
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn ascii_string() -> impl Strategy<Value = String> {
        prop::string::string_regex("[a-z]{0,16}").expect("static regex is valid")
    }

    fn general_unicode() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0020-\\u007E\\u00A0-\\u017F]{0,16}")
            .expect("static regex is valid")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // The output has at least `target` scalars.
        #[test]
        fn pad_left_reaches_target_scalars(s in general_unicode(), t in 0usize..32) {
            let out = pad_left(&s, t, '.');
            prop_assert!(out.chars().count() >= t);
            prop_assert!(out.chars().count() >= s.chars().count());
        }

        #[test]
        fn pad_right_reaches_target_scalars(s in general_unicode(), t in 0usize..32) {
            let out = pad_right(&s, t, '.');
            prop_assert!(out.chars().count() >= t);
            prop_assert!(out.chars().count() >= s.chars().count());
        }

        #[test]
        fn center_reaches_target_scalars(s in general_unicode(), t in 0usize..32) {
            let out = center(&s, t, '.');
            prop_assert!(out.chars().count() >= t);
            prop_assert!(out.chars().count() >= s.chars().count());
        }

        // The output always ends with (pad_left) or starts with
        // (pad_right) the input.
        #[test]
        fn pad_left_output_ends_with_input(s in ascii_string(), t in 0usize..32) {
            prop_assert!(pad_left(&s, t, ' ').ends_with(&s));
        }

        #[test]
        fn pad_right_output_starts_with_input(s in ascii_string(), t in 0usize..32) {
            prop_assert!(pad_right(&s, t, ' ').starts_with(&s));
        }

        // The output contains the input (as a substring) after centering.
        #[test]
        fn center_output_contains_input(s in ascii_string(), t in 0usize..32) {
            prop_assert!(center(&s, t, ' ').contains(&s));
        }

        // Byte-width pad reaches or exceeds target bytes.
        #[test]
        fn pad_left_bytes_reaches_target(s in general_unicode(), t in 0usize..32) {
            prop_assert!(pad_left_bytes(&s, t, ' ').len() >= t);
        }

        // Padding to a target no larger than current length is identity.
        #[test]
        fn pad_at_or_under_length_is_identity(s in general_unicode()) {
            let n = s.chars().count();
            prop_assert_eq!(pad_left(&s, n, ' '), s.clone());
            prop_assert_eq!(pad_right(&s, n, ' '), s.clone());
            prop_assert_eq!(center(&s, n, ' '), s.clone());
        }
    }
}
