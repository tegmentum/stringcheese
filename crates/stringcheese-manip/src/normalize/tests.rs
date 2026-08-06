//! Tests for [`crate::normalize`].
//!
//! Unit tests cover the empty / all-whitespace / mixed-line-ending /
//! ANSI-heavy / typography edge cases across every function. Property
//! tests confirm **idempotence** — each normalization applied twice
//! equals the same normalization applied once — and length monotonicity
//! for the stripping variants.

use super::*;

// -----------------------------------------------------------------
// collapse_whitespace
// -----------------------------------------------------------------

#[test]
fn collapse_whitespace_basic() {
    assert_eq!(collapse_whitespace("  hello   world  "), "hello world");
}

#[test]
fn collapse_whitespace_empty() {
    assert_eq!(collapse_whitespace(""), "");
}

#[test]
fn collapse_whitespace_all_whitespace() {
    assert_eq!(collapse_whitespace("   \t\n"), "");
}

#[test]
fn collapse_whitespace_no_whitespace_unchanged() {
    assert_eq!(collapse_whitespace("hello"), "hello");
}

#[test]
fn collapse_whitespace_tabs_and_newlines() {
    assert_eq!(collapse_whitespace("a\t\tb\n\nc"), "a b c");
}

#[test]
fn collapse_whitespace_unicode_whitespace() {
    // U+00A0 NBSP and U+3000 IDEOGRAPHIC SPACE both count as whitespace.
    assert_eq!(collapse_whitespace("x\u{00A0}\u{00A0}y"), "x y");
    assert_eq!(collapse_whitespace("a\u{3000}b"), "a b");
}

#[test]
fn collapse_whitespace_single_space_unchanged() {
    assert_eq!(collapse_whitespace("a b c"), "a b c");
}

// -----------------------------------------------------------------
// normalize_line_endings
// -----------------------------------------------------------------

#[test]
fn line_endings_to_lf() {
    assert_eq!(
        normalize_line_endings("a\r\nb\nc\rd", LineEnding::Lf),
        "a\nb\nc\nd"
    );
}

#[test]
fn line_endings_to_crlf() {
    assert_eq!(
        normalize_line_endings("a\nb\r\nc\rd", LineEnding::CrLf),
        "a\r\nb\r\nc\r\nd"
    );
}

#[test]
fn line_endings_to_cr() {
    assert_eq!(
        normalize_line_endings("a\nb\r\nc\rd", LineEnding::Cr),
        "a\rb\rc\rd"
    );
}

#[test]
fn line_endings_no_terminators_unchanged() {
    assert_eq!(normalize_line_endings("hello", LineEnding::Lf), "hello");
}

#[test]
fn line_endings_empty() {
    assert_eq!(normalize_line_endings("", LineEnding::Lf), "");
}

#[test]
fn line_endings_bare_cr_at_end() {
    // A trailing bare `\r` becomes the chosen terminator.
    assert_eq!(normalize_line_endings("hi\r", LineEnding::Lf), "hi\n");
}

#[test]
fn line_endings_preserves_multi_byte_scalars() {
    assert_eq!(
        normalize_line_endings("café\r\ncafé", LineEnding::Lf),
        "café\ncafé"
    );
}

// -----------------------------------------------------------------
// strip_control
// -----------------------------------------------------------------

#[test]
fn strip_control_removes_bell_but_keeps_tab_newline() {
    assert_eq!(strip_control("a\x07b\tc\nd"), "ab\tc\nd");
}

#[test]
fn strip_control_removes_del() {
    assert_eq!(strip_control("hi\x7fthere"), "hithere");
}

#[test]
fn strip_control_empty() {
    assert_eq!(strip_control(""), "");
}

#[test]
fn strip_control_keeps_regular_text() {
    assert_eq!(strip_control("hello world"), "hello world");
}

#[test]
fn strip_control_removes_c1_range() {
    // U+0080 through U+009F are the C1 controls.
    assert_eq!(strip_control("a\u{0080}b\u{009F}c"), "abc");
}

// -----------------------------------------------------------------
// strip_ansi
// -----------------------------------------------------------------

#[test]
fn strip_ansi_csi_color() {
    assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
}

#[test]
fn strip_ansi_multi_color() {
    assert_eq!(
        strip_ansi("\x1b[1;31mbold-red\x1b[0m plain \x1b[32mgreen\x1b[0m"),
        "bold-red plain green"
    );
}

#[test]
fn strip_ansi_cursor_movement() {
    // \x1b[H is cursor home, \x1b[2J clears screen.
    assert_eq!(strip_ansi("\x1b[H\x1b[2Jhello"), "hello");
}

#[test]
fn strip_ansi_osc_bel_terminated() {
    // OSC (window title) terminated by BEL.
    assert_eq!(strip_ansi("hi\x1b]0;title\x07there"), "hithere");
}

#[test]
fn strip_ansi_osc_st_terminated() {
    // OSC terminated by ESC \ (String Terminator).
    assert_eq!(strip_ansi("a\x1b]0;title\x1b\\b"), "ab");
}

#[test]
fn strip_ansi_simple_two_byte_escape() {
    // ESC 7 (save cursor) and ESC 8 (restore cursor).
    assert_eq!(strip_ansi("a\x1b7b\x1b8c"), "abc");
}

#[test]
fn strip_ansi_no_escapes_unchanged() {
    assert_eq!(strip_ansi("plain text"), "plain text");
}

#[test]
fn strip_ansi_empty() {
    assert_eq!(strip_ansi(""), "");
}

#[test]
fn strip_ansi_preserves_unicode() {
    assert_eq!(strip_ansi("café\x1b[0m"), "café");
}

// -----------------------------------------------------------------
// nfc / nfd / nfkc / nfkd
// -----------------------------------------------------------------

#[test]
fn nfc_composes_decomposed_accent() {
    assert_eq!(nfc("cafe\u{0301}"), "caf\u{00E9}");
}

#[test]
fn nfd_decomposes_precomposed_accent() {
    assert_eq!(nfd("caf\u{00E9}"), "cafe\u{0301}");
}

#[test]
fn nfkc_reduces_fullwidth_digit() {
    assert_eq!(nfkc("\u{FF11}"), "1");
}

#[test]
fn nfkd_reduces_ligature() {
    // U+FB01 LATIN SMALL LIGATURE FI decomposes under compatibility.
    assert_eq!(nfkd("\u{FB01}"), "fi");
}

// -----------------------------------------------------------------
// normalize_quotes / dashes / ellipsis
// -----------------------------------------------------------------

#[test]
fn normalize_quotes_double_and_single() {
    assert_eq!(
        normalize_quotes("\u{201C}hi\u{201D} \u{2018}there\u{2019}"),
        "\"hi\" 'there'"
    );
}

#[test]
fn normalize_quotes_no_typographic_unchanged() {
    assert_eq!(normalize_quotes("\"hi\" 'there'"), "\"hi\" 'there'");
}

#[test]
fn normalize_dashes_em() {
    assert_eq!(normalize_dashes("a\u{2014}b"), "a--b");
}

#[test]
fn normalize_dashes_en() {
    assert_eq!(normalize_dashes("1\u{2013}2"), "1-2");
}

#[test]
fn normalize_dashes_mixed() {
    assert_eq!(normalize_dashes("a\u{2014}b-c\u{2013}d"), "a--b-c-d");
}

#[test]
fn normalize_ellipsis_basic() {
    assert_eq!(normalize_ellipsis("wait\u{2026}"), "wait...");
}

#[test]
fn normalize_ellipsis_multiple() {
    assert_eq!(normalize_ellipsis("a\u{2026}b\u{2026}"), "a...b...");
}

#[test]
fn normalize_ellipsis_no_glyph_unchanged() {
    assert_eq!(normalize_ellipsis("wait..."), "wait...");
}

// -----------------------------------------------------------------
// LineEnding
// -----------------------------------------------------------------

#[test]
fn line_ending_as_str() {
    assert_eq!(LineEnding::Lf.as_str(), "\n");
    assert_eq!(LineEnding::CrLf.as_str(), "\r\n");
    assert_eq!(LineEnding::Cr.as_str(), "\r");
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn general_unicode() -> impl Strategy<Value = String> {
        // ASCII + Latin-1 supplement + combining marks + a bit of CJK
        // whitespace — enough shape variety to exercise the normalizer.
        prop::string::string_regex("[\\u0020-\\u007E\\u00A0-\\u017F\\u0300-\\u036F\\u3000]{0,32}")
            .expect("static regex is valid")
    }

    fn ansi_input() -> impl Strategy<Value = String> {
        // Mix of plain text, CSI-shaped escapes, and stray ESC bytes.
        prop::string::string_regex(
            "(\\x1b\\[[0-9;]*[a-zA-Z]|[a-zA-Z0-9 ]|\\x1b\\][^\\x07]*\\x07|\\x1b7|\\x1b8){0,16}",
        )
        .expect("static regex is valid")
    }

    fn line_ending_choice() -> impl Strategy<Value = LineEnding> {
        prop_oneof![
            Just(LineEnding::Lf),
            Just(LineEnding::CrLf),
            Just(LineEnding::Cr),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // Idempotence: applying twice equals applying once.
        #[test]
        fn collapse_whitespace_idempotent(s in general_unicode()) {
            let once = collapse_whitespace(&s);
            let twice = collapse_whitespace(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn strip_control_idempotent(s in general_unicode()) {
            let once = strip_control(&s);
            let twice = strip_control(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn strip_ansi_idempotent(s in ansi_input()) {
            let once = strip_ansi(&s);
            let twice = strip_ansi(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn nfc_idempotent(s in general_unicode()) {
            let once = nfc(&s);
            let twice = nfc(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn nfd_idempotent(s in general_unicode()) {
            let once = nfd(&s);
            let twice = nfd(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn normalize_quotes_idempotent(s in general_unicode()) {
            let once = normalize_quotes(&s);
            let twice = normalize_quotes(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn normalize_line_endings_idempotent(s in general_unicode(), to in line_ending_choice()) {
            let once = normalize_line_endings(&s, to);
            let twice = normalize_line_endings(&once, to);
            prop_assert_eq!(once, twice);
        }

        // Stripping never grows the output.
        #[test]
        fn strip_control_never_grows(s in general_unicode()) {
            prop_assert!(strip_control(&s).len() <= s.len());
        }

        #[test]
        fn strip_ansi_never_grows(s in ansi_input()) {
            prop_assert!(strip_ansi(&s).len() <= s.len());
        }

        // collapse_whitespace never grows.
        #[test]
        fn collapse_whitespace_never_grows(s in general_unicode()) {
            prop_assert!(collapse_whitespace(&s).len() <= s.len());
        }
    }
}
