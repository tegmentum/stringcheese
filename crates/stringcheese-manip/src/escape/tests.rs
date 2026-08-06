//! Tests for [`crate::escape`].
//!
//! Unit tests exercise the empty / no-escape-needed / all-escaped edge
//! cases for every function and every escape mode, plus one round-trip
//! probe per family. Property tests confirm the round-trip law
//! `unescape(escape(s)) == s` for every escape/unescape pair.

use super::*;
use alloc::borrow::Cow;

// -----------------------------------------------------------------
// HTML
// -----------------------------------------------------------------

#[test]
fn escape_html_empty() {
    assert_eq!(escape_html(""), "");
    assert!(matches!(escape_html(""), Cow::Borrowed(_)));
}

#[test]
fn escape_html_no_special_is_borrowed() {
    let s = escape_html("hello world");
    assert_eq!(s, "hello world");
    assert!(matches!(s, Cow::Borrowed(_)));
}

#[test]
fn escape_html_all_five_special_chars() {
    assert_eq!(escape_html("<>&'\""), "&lt;&gt;&amp;&#39;&quot;");
}

#[test]
fn escape_html_mixed() {
    assert_eq!(
        escape_html("<b>Tom & Jerry's \"pet\"</b>"),
        "&lt;b&gt;Tom &amp; Jerry&#39;s &quot;pet&quot;&lt;/b&gt;"
    );
}

#[test]
fn escape_html_non_ascii_passes_through() {
    assert_eq!(escape_html("naïve café"), "naïve café");
}

#[test]
fn unescape_html_empty() {
    assert_eq!(unescape_html(""), "");
    assert!(matches!(unescape_html(""), Cow::Borrowed(_)));
}

#[test]
fn unescape_html_no_amp_is_borrowed() {
    let s = unescape_html("hello");
    assert!(matches!(s, Cow::Borrowed(_)));
}

#[test]
fn unescape_html_named_entities() {
    assert_eq!(
        unescape_html("&lt;&gt;&amp;&quot;&apos;&nbsp;"),
        "<>&\"'\u{00A0}"
    );
}

#[test]
fn unescape_html_numeric_decimal() {
    assert_eq!(unescape_html("&#65;&#66;&#67;"), "ABC");
}

#[test]
fn unescape_html_numeric_hex() {
    assert_eq!(unescape_html("&#x41;&#X42;&#x1F600;"), "AB\u{1F600}");
}

#[test]
fn unescape_html_unknown_entity_passes_through() {
    assert_eq!(unescape_html("&notreal;"), "&notreal;");
}

#[test]
fn unescape_html_stray_ampersand_passes_through() {
    assert_eq!(unescape_html("a & b"), "a & b");
}

#[test]
fn escape_unescape_html_round_trip() {
    let inputs = ["", "hello", "<b>", "a & b", "'\"'", "café"];
    for input in inputs {
        let escaped = escape_html(input);
        let unescaped = unescape_html(&escaped);
        assert_eq!(unescaped, input, "round-trip on {input:?}");
    }
}

// -----------------------------------------------------------------
// JSON
// -----------------------------------------------------------------

#[test]
fn escape_json_empty() {
    assert_eq!(escape_json(""), "");
    assert!(matches!(escape_json(""), Cow::Borrowed(_)));
}

#[test]
fn escape_json_no_special_is_borrowed() {
    assert!(matches!(escape_json("hello"), Cow::Borrowed(_)));
}

#[test]
fn escape_json_named_escapes() {
    assert_eq!(escape_json("\\\"\n\r\t\x08\x0C"), "\\\\\\\"\\n\\r\\t\\b\\f");
}

#[test]
fn escape_json_control_char() {
    // U+0001 has no named escape.
    assert_eq!(escape_json("\x01"), "\\u0001");
    assert_eq!(escape_json("\x1f"), "\\u001f");
}

#[test]
fn escape_json_non_ascii_passes_through() {
    assert_eq!(escape_json("café"), "café");
    assert_eq!(escape_json("😀"), "😀");
}

#[test]
fn unescape_json_empty() {
    assert_eq!(unescape_json("").unwrap(), "");
}

#[test]
fn unescape_json_no_backslash_is_borrowed() {
    assert!(matches!(unescape_json("hello").unwrap(), Cow::Borrowed(_)));
}

#[test]
fn unescape_json_all_named() {
    assert_eq!(
        unescape_json("\\\\\\\"\\n\\r\\t\\b\\f\\/").unwrap(),
        "\\\"\n\r\t\x08\x0C/"
    );
}

#[test]
fn unescape_json_unicode_basic() {
    assert_eq!(unescape_json("\\u0041").unwrap(), "A");
}

#[test]
fn unescape_json_unicode_surrogate_pair() {
    // 😀 = U+1F600.
    assert_eq!(unescape_json("\\uD83D\\uDE00").unwrap(), "\u{1F600}");
}

#[test]
fn unescape_json_trailing_backslash_errors() {
    let err = unescape_json("abc\\").unwrap_err();
    assert_eq!(err.position, 3);
    assert!(matches!(err.kind, JsonUnescapeErrorKind::TrailingBackslash));
}

#[test]
fn unescape_json_invalid_escape_errors() {
    let err = unescape_json("\\z").unwrap_err();
    assert!(matches!(
        err.kind,
        JsonUnescapeErrorKind::InvalidEscape('z')
    ));
}

#[test]
fn unescape_json_unpaired_high_surrogate_errors() {
    let err = unescape_json("\\uD83D").unwrap_err();
    assert!(matches!(err.kind, JsonUnescapeErrorKind::UnpairedSurrogate));
}

#[test]
fn unescape_json_lone_low_surrogate_errors() {
    let err = unescape_json("\\uDE00").unwrap_err();
    assert!(matches!(err.kind, JsonUnescapeErrorKind::UnpairedSurrogate));
}

#[test]
fn unescape_json_bad_hex_errors() {
    let err = unescape_json("\\u00XY").unwrap_err();
    assert!(matches!(
        err.kind,
        JsonUnescapeErrorKind::InvalidUnicodeEscape
    ));
}

// -----------------------------------------------------------------
// Shell (POSIX)
// -----------------------------------------------------------------

#[test]
fn escape_shell_posix_empty() {
    // Empty must be quoted so it becomes a real (empty) argument.
    assert_eq!(escape_shell_posix(""), "''");
}

#[test]
fn escape_shell_posix_safe_identifier_borrowed() {
    let s = escape_shell_posix("hello");
    assert_eq!(s, "hello");
    assert!(matches!(s, Cow::Borrowed(_)));
    // The full unreserved set:
    assert!(matches!(
        escape_shell_posix("a_b@c%d+e=f:g,h.i/j-k"),
        Cow::Borrowed(_)
    ));
}

#[test]
fn escape_shell_posix_space_forces_quotes() {
    assert_eq!(escape_shell_posix("hello world"), "'hello world'");
}

#[test]
fn escape_shell_posix_embedded_single_quote() {
    assert_eq!(escape_shell_posix("it's"), "'it'\\''s'");
    assert_eq!(escape_shell_posix("'"), "''\\'''");
}

#[test]
fn escape_shell_posix_dangerous_chars() {
    assert_eq!(escape_shell_posix("$(pwd)"), "'$(pwd)'");
    assert_eq!(escape_shell_posix("`whoami`"), "'`whoami`'");
    assert_eq!(escape_shell_posix("a; rm -rf /"), "'a; rm -rf /'");
}

// -----------------------------------------------------------------
// Shell (Windows)
// -----------------------------------------------------------------

#[test]
fn escape_shell_windows_empty() {
    assert_eq!(escape_shell_windows(""), "\"\"");
}

#[test]
fn escape_shell_windows_safe_identifier_borrowed() {
    let s = escape_shell_windows("hello");
    assert!(matches!(s, Cow::Borrowed(_)));
}

#[test]
fn escape_shell_windows_space_forces_quotes() {
    assert_eq!(escape_shell_windows("hello world"), "\"hello world\"");
}

#[test]
fn escape_shell_windows_embedded_quote() {
    assert_eq!(escape_shell_windows("a\"b"), "\"a\\\"b\"");
}

#[test]
fn escape_shell_windows_trailing_backslash_doubles() {
    assert_eq!(escape_shell_windows("path\\"), "\"path\\\\\"");
    assert_eq!(escape_shell_windows("path\\\\"), "\"path\\\\\\\\\"");
}

#[test]
fn escape_shell_windows_backslash_before_quote_doubles() {
    // Input: a\" — one backslash then quote. The `\` before `"` must
    // be doubled to `\\`, and the `"` itself escaped, giving `a\\\"`.
    assert_eq!(escape_shell_windows("a\\\"b"), "\"a\\\\\\\"b\"");
}

#[test]
fn escape_shell_windows_cmd_metachar_caret_prefixes() {
    // `&` triggers cmd-mode escaping; the outer quotes get caret-prefixed.
    let out = escape_shell_windows("a & b");
    assert!(out.contains("^\""));
    assert!(out.contains("^&"));
}

// -----------------------------------------------------------------
// Percent-encoding
// -----------------------------------------------------------------

#[test]
fn percent_encode_empty() {
    assert_eq!(percent_encode("", PercentSet::Path), "");
}

#[test]
fn percent_encode_unreserved() {
    assert_eq!(
        percent_encode("ABCabc123-._~", PercentSet::Path),
        "ABCabc123-._~"
    );
}

#[test]
fn percent_encode_space_and_slash_in_path() {
    // Path allows `:@,;=&+$!'()*` but not `/` in a segment.
    assert_eq!(percent_encode("a b", PercentSet::Path), "a%20b");
    assert_eq!(percent_encode("a/b", PercentSet::Path), "a%2Fb");
}

#[test]
fn percent_encode_query_permits_slash() {
    // In a query, `/` and `?` are legal.
    assert_eq!(percent_encode("a/b?c", PercentSet::Query), "a/b?c");
}

#[test]
fn percent_encode_userinfo_denies_at() {
    // The `@` marker terminates userinfo — it must be encoded inside.
    assert_eq!(percent_encode("a@b", PercentSet::Userinfo), "a%40b");
    // Path permits `@`.
    assert_eq!(percent_encode("a@b", PercentSet::Path), "a@b");
}

#[test]
fn percent_encode_multibyte_scalar() {
    assert_eq!(percent_encode("é", PercentSet::Path), "%C3%A9");
}

#[test]
fn percent_decode_empty() {
    assert_eq!(percent_decode("").unwrap(), "");
}

#[test]
fn percent_decode_no_percent() {
    assert_eq!(percent_decode("hello").unwrap(), "hello");
}

#[test]
fn percent_decode_hex_lowercase_and_upper() {
    assert_eq!(percent_decode("%c3%A9").unwrap(), "é");
}

#[test]
fn percent_decode_invalid_hex_errors() {
    let err = percent_decode("%GG").unwrap_err();
    assert_eq!(err.position, 0);
    assert!(matches!(err.kind, PercentDecodeErrorKind::InvalidEscape));
}

#[test]
fn percent_decode_short_escape_errors() {
    let err = percent_decode("%A").unwrap_err();
    assert!(matches!(err.kind, PercentDecodeErrorKind::InvalidEscape));
}

#[test]
fn percent_decode_invalid_utf8_errors() {
    // 0xFF alone is never valid UTF-8.
    let err = percent_decode("%FF").unwrap_err();
    assert!(matches!(err.kind, PercentDecodeErrorKind::InvalidUtf8));
}

// -----------------------------------------------------------------
// C-string
// -----------------------------------------------------------------

#[test]
fn escape_c_string_empty() {
    assert_eq!(escape_c_string(""), "");
}

#[test]
fn escape_c_string_printable_unchanged() {
    assert_eq!(escape_c_string("hello 123"), "hello 123");
}

#[test]
fn escape_c_string_all_named() {
    assert_eq!(escape_c_string("\\\"\n\r\t"), "\\\\\\\"\\n\\r\\t");
}

#[test]
fn escape_c_string_control_char_hex() {
    assert_eq!(escape_c_string("\x01"), "\\x01");
    assert_eq!(escape_c_string("\x7F"), "\\x7f");
}

#[test]
fn escape_c_string_non_ascii_byte_escaped() {
    // é = 0xC3 0xA9 in UTF-8.
    assert_eq!(escape_c_string("é"), "\\xc3\\xa9");
}

#[test]
fn unescape_c_string_named() {
    assert_eq!(
        unescape_c_string("\\\\\\\"\\n\\r\\t").unwrap(),
        "\\\"\n\r\t"
    );
}

#[test]
fn unescape_c_string_hex() {
    assert_eq!(unescape_c_string("\\x41").unwrap(), "A");
}

#[test]
fn unescape_c_string_unicode_u() {
    assert_eq!(unescape_c_string("\\u00E9").unwrap(), "é");
}

#[test]
fn unescape_c_string_unicode_upper_u() {
    assert_eq!(unescape_c_string("\\U0001F600").unwrap(), "\u{1F600}");
}

#[test]
fn unescape_c_string_octal() {
    assert_eq!(unescape_c_string("\\101").unwrap(), "A"); // 0o101 = 65
    assert_eq!(unescape_c_string("\\0").unwrap(), "\0");
    // Runs stop at first non-octal digit or after 3 digits.
    assert_eq!(unescape_c_string("\\1018").unwrap(), "A8");
}

#[test]
fn unescape_c_string_trailing_backslash_errors() {
    assert!(unescape_c_string("abc\\").is_err());
}

#[test]
fn unescape_c_string_bad_hex_errors() {
    let err = unescape_c_string("\\xZZ").unwrap_err();
    assert!(matches!(
        err.kind,
        CStringUnescapeErrorKind::InvalidHexEscape
    ));
}

#[test]
fn escape_unescape_c_string_round_trip() {
    let inputs = ["", "hello", "a\"b\\c", "line\nfeed", "café"];
    for input in inputs {
        let escaped = escape_c_string(input);
        let unescaped = unescape_c_string(&escaped).unwrap();
        assert_eq!(unescaped, input, "round-trip on {input:?}");
    }
}

// -----------------------------------------------------------------
// Regex
// -----------------------------------------------------------------

#[test]
fn escape_regex_empty() {
    assert_eq!(escape_regex(""), "");
}

#[test]
fn escape_regex_no_metachar() {
    assert_eq!(escape_regex("hello123"), "hello123");
}

#[test]
fn escape_regex_all_metachars() {
    assert_eq!(
        escape_regex(".^$*+?()[]{}|\\/#"),
        "\\.\\^\\$\\*\\+\\?\\(\\)\\[\\]\\{\\}\\|\\\\\\/\\#"
    );
}

#[test]
fn escape_regex_mixed() {
    assert_eq!(escape_regex("a.b+c"), "a\\.b\\+c");
    assert_eq!(escape_regex("$100"), "\\$100");
    assert_eq!(escape_regex("(a|b)"), "\\(a\\|b\\)");
}

#[test]
fn escape_regex_non_ascii_passes_through() {
    assert_eq!(escape_regex("caf.é"), "caf\\.é");
}

// -----------------------------------------------------------------
// Property tests
// -----------------------------------------------------------------

#[cfg(all(feature = "std", not(target_family = "wasm")))]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn general_unicode() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0000-\\u007F\\u00A0-\\u017F]{0,64}")
            .expect("static regex is valid")
    }

    // ASCII-only input for the shell/regex round trips whose encoders
    // do not have a stated inverse.
    fn ascii_only() -> impl Strategy<Value = String> {
        prop::string::string_regex("[\\u0020-\\u007E]{0,64}").expect("static regex is valid")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn html_round_trip(s in general_unicode()) {
            let esc = escape_html(&s);
            let back = unescape_html(&esc);
            prop_assert_eq!(back.as_ref(), s.as_str());
        }

        #[test]
        fn json_round_trip(s in general_unicode()) {
            let esc = escape_json(&s);
            let back = unescape_json(&esc).expect("escape_json output must decode");
            prop_assert_eq!(back.as_ref(), s.as_str());
        }

        #[test]
        fn percent_round_trip_path(s in general_unicode()) {
            let enc = percent_encode(&s, PercentSet::Path);
            let dec = percent_decode(&enc).expect("percent_encode output must decode");
            prop_assert_eq!(dec, s);
        }

        #[test]
        fn percent_round_trip_query(s in general_unicode()) {
            let enc = percent_encode(&s, PercentSet::Query);
            let dec = percent_decode(&enc).expect("percent_encode output must decode");
            prop_assert_eq!(dec, s);
        }

        #[test]
        fn c_string_round_trip(s in general_unicode()) {
            let esc = escape_c_string(&s);
            let back = unescape_c_string(&esc).expect("escape_c_string output must decode");
            prop_assert_eq!(back, s);
        }

        // The regex-escaped output never contains an unescaped metachar
        // that could accidentally form a group / repetition.
        #[test]
        fn regex_escape_no_bare_metachar(s in ascii_only()) {
            let esc = escape_regex(&s);
            let mut chars = esc.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    // Consume the escaped char.
                    chars.next();
                } else {
                    prop_assert!(!is_regex_metachar(c), "bare metachar {c:?} in {esc:?}");
                }
            }
        }
    }
}
