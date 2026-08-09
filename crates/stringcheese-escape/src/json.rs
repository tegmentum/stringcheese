//! JSON string escape / unescape.
//!
//! Handles the escape rules for the interior of a JSON string
//! literal — `" \ \b \f \n \r \t` plus every C0 control code as
//! `\u00XX`. Does NOT add the surrounding `"…"` quotes;
//! [`escape`] returns the encoded body only so callers can
//! interpolate it into whatever wrapper (attribute, RPC payload,
//! embedded config) they're building.
//!
//! In-house rather than wrapping `serde_json` — the grammar is
//! ~200 lines of straight-line dispatch, and pulling `serde_json`
//! for a string-escape utility would double the crate's dependency
//! footprint.

use alloc::string::String;

/// Escape the body of a JSON string.
///
/// Every character `c` maps to:
///
/// - `\"` for `"`, `\\` for `\`, `\/` for `/` (optional per RFC but
///   handy for embedding inside `</script>` sequences).
/// - `\b \f \n \r \t` for the named short-form controls.
/// - `\u00XX` for every other C0 control (< U+0020).
/// - The character itself, verbatim, otherwise.
///
/// ## Implementation
///
/// Byte-oriented. Every JSON escape rule fires only for ASCII
/// bytes (U+0000..U+007F); every U+0080+ scalar passes through
/// unchanged. So we walk `input.as_bytes()`, dispatch each ASCII
/// byte through a 128-entry static lookup table, and emit
/// multi-byte scalars in bulk by copying the run of continuation
/// bytes wholesale. The bench-driven redesign (2026-08-09):
/// per-char `match` was 3-4× slower than the wrapped alternatives
/// in the crate's other targets; the table + bulk-copy path
/// closes most of that gap.
#[must_use]
pub fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            // ASCII — dispatch on the escape table.
            match ASCII_JSON_ESCAPE[b as usize] {
                AsciiEscape::Passthrough => {
                    // Coalesce a run of passthrough ASCII bytes so
                    // the hot path is one `push_str` per run, not
                    // per byte.
                    let start = i;
                    while i < bytes.len()
                        && bytes[i] < 0x80
                        && matches!(
                            ASCII_JSON_ESCAPE[bytes[i] as usize],
                            AsciiEscape::Passthrough
                        )
                    {
                        i += 1;
                    }
                    // Safe: `bytes[start..i]` is a run of ASCII bytes.
                    out.push_str(core::str::from_utf8(&bytes[start..i]).unwrap_or(""));
                }
                AsciiEscape::Named(s) => {
                    out.push_str(s);
                    i += 1;
                }
                AsciiEscape::Unicode => {
                    use core::fmt::Write as _;
                    let _ = write!(out, "\\u{b:04X}");
                    i += 1;
                }
            }
        } else {
            // Multi-byte scalar — copy the whole UTF-8 sequence
            // wholesale. Length inferred from the leading byte.
            let width = utf8_width(b);
            let end = (i + width).min(bytes.len());
            out.push_str(core::str::from_utf8(&bytes[i..end]).unwrap_or(""));
            i = end;
        }
    }
    out
}

/// One escape action for an ASCII byte in JSON string context.
#[derive(Copy, Clone)]
enum AsciiEscape {
    /// Byte passes through unchanged. Most ASCII printables.
    Passthrough,
    /// Byte becomes a named short-form escape (`\"`, `\\`, `\n`, ...).
    Named(&'static str),
    /// Byte becomes `\u00XX`. Applies to C0 controls not covered
    /// by the named short forms.
    Unicode,
}

const ASCII_JSON_ESCAPE: [AsciiEscape; 128] = build_json_escape_table();

const fn build_json_escape_table() -> [AsciiEscape; 128] {
    let mut table = [AsciiEscape::Passthrough; 128];
    // Every C0 control (< 0x20) defaults to \u00XX.
    let mut i = 0usize;
    while i < 0x20 {
        table[i] = AsciiEscape::Unicode;
        i += 1;
    }
    // Named short forms override the Unicode default where they
    // exist.
    table[0x08] = AsciiEscape::Named("\\b");
    table[0x09] = AsciiEscape::Named("\\t");
    table[0x0A] = AsciiEscape::Named("\\n");
    table[0x0C] = AsciiEscape::Named("\\f");
    table[0x0D] = AsciiEscape::Named("\\r");
    // Non-control escapes: quote, backslash, slash.
    table[b'"' as usize] = AsciiEscape::Named("\\\"");
    table[b'\\' as usize] = AsciiEscape::Named("\\\\");
    table[b'/' as usize] = AsciiEscape::Named("\\/");
    table
}

/// UTF-8 byte-length inferred from the leading byte.
const fn utf8_width(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b < 0xC2 {
        // Invalid leader — shouldn't appear in a valid `&str`.
        1
    } else if b < 0xE0 {
        2
    } else if b < 0xF0 {
        3
    } else {
        4
    }
}

/// Reverse [`escape`]. Fails on malformed `\` sequences.
pub fn unescape(input: &str) -> Result<String, UnescapeError> {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let next = chars.next().ok_or(UnescapeError::TrailingBackslash)?;
        match next {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            'b' => out.push('\u{08}'),
            'f' => out.push('\u{0C}'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                if hex.len() != 4 {
                    return Err(UnescapeError::BadUnicodeEscape);
                }
                let cp =
                    u32::from_str_radix(&hex, 16).map_err(|_| UnescapeError::BadUnicodeEscape)?;
                let ch = char::from_u32(cp).ok_or(UnescapeError::BadUnicodeEscape)?;
                out.push(ch);
            }
            other => return Err(UnescapeError::UnknownEscape(other)),
        }
    }
    Ok(out)
}

/// JSON-string decode failure reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnescapeError {
    /// Input ended with a `\` that had no following character.
    TrailingBackslash,
    /// `\u` was not followed by exactly four valid hex digits.
    BadUnicodeEscape,
    /// `\X` where `X` isn't a valid escape character. Carries the
    /// unknown character for diagnostics.
    UnknownEscape(char),
}

impl core::fmt::Display for UnescapeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TrailingBackslash => write!(f, "input ends with a bare backslash"),
            Self::BadUnicodeEscape => write!(f, "\\u must be followed by 4 hex digits"),
            Self::UnknownEscape(c) => write!(f, "unknown escape \\{c}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_and_backslash_escaped() {
        assert_eq!(escape(r#"say "hi""#), r#"say \"hi\""#);
        assert_eq!(escape(r"path\to\file"), r"path\\to\\file");
    }

    #[test]
    fn newline_and_tab_get_short_forms() {
        assert_eq!(escape("a\nb\tc"), "a\\nb\\tc");
    }

    #[test]
    fn other_c0_controls_get_unicode_escape() {
        // \x07 is BEL — not in the named short-form list, must go
        // to .
        assert_eq!(escape("a\x07b"), "a\\u0007b");
    }

    #[test]
    fn unescape_reverses_named() {
        assert_eq!(unescape(r#"say \"hi\""#).unwrap(), r#"say "hi""#);
        assert_eq!(unescape(r"a\nb\tc").unwrap(), "a\nb\tc");
    }

    #[test]
    fn unescape_unicode_hex() {
        assert_eq!(unescape(r"AB").unwrap(), "AB");
    }

    #[test]
    fn round_trip() {
        let cases = ["hello", r#"a"b\c/d"#, "line\n\ttab", "bell\x07here"];
        for s in cases {
            assert_eq!(unescape(&escape(s)).unwrap(), s);
        }
    }

    #[test]
    fn trailing_backslash_is_error() {
        assert_eq!(unescape(r"foo\"), Err(UnescapeError::TrailingBackslash));
    }

    #[test]
    fn short_unicode_escape_is_error() {
        assert_eq!(unescape(r"\u12"), Err(UnescapeError::BadUnicodeEscape));
    }

    #[test]
    fn unknown_escape_is_error() {
        assert!(matches!(
            unescape(r"\q"),
            Err(UnescapeError::UnknownEscape('q'))
        ));
    }
}
