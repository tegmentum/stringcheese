//! Small in-house primitives — the pieces not already covered by
//! [`stringcheese_unicode`].
//!
//! These are deliberately kept simple: table-driven substitutions
//! and character-class filters that a hand-written implementation
//! reads clearly. No dependency footprint beyond what's needed
//! for the tables themselves.

use alloc::string::String;

/// Collapse every run of `char::is_whitespace` scalars into a
/// single U+0020 SPACE. Leading and trailing whitespace runs
/// collapse to a single space too — use [`trim`] to strip them.
///
/// Uses Rust's Unicode-aware `is_whitespace` (matches the standard
/// library's convention), so any of U+0009 tab, U+000A newline,
/// U+000B/C/D vertical whitespace, U+0020 space, U+0085 NEL,
/// U+00A0 NBSP, U+1680 OGHAM SPACE MARK, U+2000..U+200A widths,
/// U+2028 line sep, U+2029 para sep, U+202F narrow NBSP,
/// U+205F medium math space, U+3000 ideographic space all count.
///
/// ## Implementation
///
/// Bench 2026-08-09: an attempted ASCII "coalesce non-whitespace
/// runs" fast path regressed by 20 % — real inputs have whitespace
/// every ~5 bytes, so non-whitespace runs are too short to amortise
/// the double-scan overhead. The current implementation walks
/// bytes one at a time; ASCII bytes dispatch through a static
/// `ASCII_IS_WHITESPACE` table (one lookup + branch), non-ASCII
/// scalars fall back to the Unicode `char::is_whitespace` path.
///
/// # Panics
///
/// The internal `.chars().next().expect(...)` on the non-ASCII
/// branch cannot fire — the branch is only reached when
/// `bytes[i] >= 0x80`, guaranteeing `input[i..]` starts with a
/// non-empty non-ASCII scalar in any valid `&str`.
#[must_use]
pub fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_ws = false;
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            if ASCII_IS_WHITESPACE[b as usize] {
                if !in_ws {
                    out.push(' ');
                    in_ws = true;
                }
            } else {
                out.push(b as char);
                in_ws = false;
            }
            i += 1;
        } else {
            let c = input[i..]
                .chars()
                .next()
                .expect("bytes[i] >= 0x80 guarantees a non-empty non-ASCII prefix");
            i += c.len_utf8();
            if c.is_whitespace() {
                if !in_ws {
                    out.push(' ');
                    in_ws = true;
                }
            } else {
                out.push(c);
                in_ws = false;
            }
        }
    }
    out
}

/// ASCII whitespace lookup — the same set `char::is_whitespace`
/// covers within the ASCII range (0x09..=0x0D and 0x20).
const ASCII_IS_WHITESPACE: [bool; 128] = build_ascii_ws();
const fn build_ascii_ws() -> [bool; 128] {
    let mut t = [false; 128];
    let mut b = 0u8;
    while b < 128 {
        t[b as usize] = matches!(b, 0x09..=0x0D | 0x20);
        b += 1;
    }
    t
}

/// Trim leading and trailing whitespace. Returns [`String`] rather
/// than a `&str` slice so it composes with the other primitives
/// in this crate (which all take-and-return `String` /
/// `&str → String`).
#[must_use]
pub fn trim(input: &str) -> String {
    input.trim().into()
}

/// Strip every Unicode general-category Cc (control) code point.
///
/// The seven "whitespace" controls (`\t`, `\n`, vertical tab, form
/// feed, `\r`, plus the CR/LF pair when they appear) are ALSO
/// stripped — this is the "remove anything unprintable" primitive.
/// Callers who want to preserve `\n` should apply
/// [`collapse_whitespace`] afterwards, or filter differently.
///
/// ## Implementation
///
/// Byte-oriented ASCII fast path: runs of non-control ASCII
/// bytes are coalesced into a single `push_str`; control bytes
/// are skipped. Non-ASCII scalars still take the general
/// `char::is_control` path (rare — most non-ASCII scalars are
/// letters or symbols, not controls).
///
/// # Panics
///
/// The internal `.chars().next().expect(...)` on the non-ASCII
/// branch cannot fire — the branch is only reached when
/// `bytes[i] >= 0x80`, guaranteeing `input[i..]` starts with a
/// non-empty non-ASCII scalar in any valid `&str`.
#[must_use]
pub fn strip_controls(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            if ASCII_IS_CONTROL[b as usize] {
                i += 1;
            } else {
                let start = i;
                while i < bytes.len() && bytes[i] < 0x80 && !ASCII_IS_CONTROL[bytes[i] as usize] {
                    i += 1;
                }
                out.push_str(core::str::from_utf8(&bytes[start..i]).unwrap_or(""));
            }
        } else {
            let c = input[i..]
                .chars()
                .next()
                .expect("bytes[i] >= 0x80 guarantees a non-empty non-ASCII prefix");
            i += c.len_utf8();
            if !c.is_control() {
                out.push(c);
            }
        }
    }
    out
}

/// ASCII control lookup — the same set `char::is_control` covers
/// within the ASCII range (0x00..=0x1F and 0x7F).
const ASCII_IS_CONTROL: [bool; 128] = build_ascii_ctrl();
const fn build_ascii_ctrl() -> [bool; 128] {
    let mut t = [false; 128];
    let mut b = 0u8;
    while b < 128 {
        t[b as usize] = b <= 0x1F || b == 0x7F;
        b += 1;
    }
    t
}

/// Canonicalise common ambiguously-encoded punctuation.
///
/// Substitutions performed:
///
/// | From | To | Description |
/// |------|-----|-------------|
/// | `“` `”` `„` `‟` `«` `»` | `"` | Curly / low / angle quotes → ASCII double quote |
/// | `‘` `’` `‚` `‛` `‹` `›` | `'` | Curly / low / single-angle quotes → ASCII apostrophe |
/// | `–` `—` `‒` `―` `‐` `‑` `⸺` | `-` | En/em/figure/horizontal/hyphen/non-breaking-hyphen/two-em dash → hyphen-minus |
/// | `…` | `...` | Horizontal ellipsis → three ASCII dots |
/// | `\u{A0}` `\u{202F}` | ` ` | NBSP / narrow NBSP → ASCII space |
///
/// Every other scalar passes through unchanged.
/// ## Implementation
///
/// Every substitution target is a non-ASCII scalar; for ASCII
/// input the output is byte-identical to the input. Bench-driven
/// redesign (2026-08-09): coalesce runs of ASCII bytes into one
/// `push_str` and only invoke the substitution match on non-ASCII
/// scalars. Same pattern as the JSON escape fast path.
///
/// # Panics
///
/// The internal `.chars().next().expect(...)` on the non-ASCII
/// branch cannot fire — the branch is only reached when
/// `bytes[i] >= 0x80`, guaranteeing `input[i..]` starts with a
/// non-empty non-ASCII scalar in any valid `&str`.
#[must_use]
pub fn canonicalize_punctuation(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] < 0x80 {
            // Coalesce a run of ASCII passthrough bytes.
            let start = i;
            while i < bytes.len() && bytes[i] < 0x80 {
                i += 1;
            }
            // Safe: bytes[start..i] is a run of ASCII, valid UTF-8.
            out.push_str(core::str::from_utf8(&bytes[start..i]).unwrap_or(""));
            continue;
        }
        // Non-ASCII — parse one scalar and match against the
        // substitution table.
        let c = input[i..]
            .chars()
            .next()
            .expect("bytes[i] >= 0x80 guarantees a non-empty non-ASCII prefix");
        i += c.len_utf8();
        match c {
            // Double quotes and angle brackets used as quotes.
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' | '\u{00AB}' | '\u{00BB}' => {
                out.push('"');
            }
            // Single quotes / apostrophes.
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' | '\u{2039}' | '\u{203A}' => {
                out.push('\'');
            }
            // Dashes — en, em, figure, horizontal-bar, hyphen,
            // non-breaking hyphen, two-em dash.
            '\u{2013}' | '\u{2014}' | '\u{2012}' | '\u{2015}' | '\u{2010}' | '\u{2011}'
            | '\u{2E3A}' => {
                out.push('-');
            }
            // Ellipsis.
            '\u{2026}' => out.push_str("..."),
            // Non-breaking spaces.
            '\u{00A0}' | '\u{202F}' => out.push(' '),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_reduces_runs_and_normalises_widths() {
        assert_eq!(collapse_whitespace("hello   world"), "hello world");
        // Tab + newline treated as whitespace runs.
        assert_eq!(collapse_whitespace("a\t\nb"), "a b");
        // Preserves leading/trailing single spaces (but as one).
        assert_eq!(collapse_whitespace("   x"), " x");
        assert_eq!(collapse_whitespace("x   "), "x ");
    }

    #[test]
    fn collapse_handles_unicode_whitespace() {
        // NBSP + ideographic space count.
        assert_eq!(collapse_whitespace("a\u{00A0}\u{3000}b"), "a b");
    }

    #[test]
    fn trim_wraps_str_trim() {
        assert_eq!(trim("  hello  "), "hello");
        assert_eq!(trim(""), "");
    }

    #[test]
    fn strip_controls_removes_c0_and_c1() {
        // BEL + normal text.
        assert_eq!(strip_controls("a\x07b"), "ab");
        // Newlines removed too (they're Cc).
        assert_eq!(strip_controls("a\nb"), "ab");
    }

    #[test]
    fn canonicalize_quotes() {
        assert_eq!(
            canonicalize_punctuation("\u{201C}hello\u{201D}"),
            "\"hello\"",
        );
        assert_eq!(canonicalize_punctuation("it\u{2019}s ok"), "it's ok",);
    }

    #[test]
    fn canonicalize_dashes() {
        assert_eq!(canonicalize_punctuation("a\u{2013}b\u{2014}c"), "a-b-c");
    }

    #[test]
    fn canonicalize_ellipsis_and_nbsp() {
        assert_eq!(canonicalize_punctuation("wait\u{2026}"), "wait...");
        assert_eq!(canonicalize_punctuation("a\u{00A0}b"), "a b");
    }

    #[test]
    fn canonicalize_leaves_ascii_alone() {
        let s = "The quick brown fox jumps over the lazy dog.";
        assert_eq!(canonicalize_punctuation(s), s);
    }
}
