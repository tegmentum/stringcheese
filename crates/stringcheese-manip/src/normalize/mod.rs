//! Canonicalize the shape of a string.
//!
//! Where [`crate::case`] adjusts *letter case*, this module adjusts
//! everything else — whitespace, line endings, control bytes, ANSI
//! escapes, Unicode composition, and typographic punctuation. Every
//! function returns a fresh owned [`String`]; borrowing variants are not
//! offered because these transformations almost always change the byte
//! length of the input.
//!
//! # Function map
//!
//! - **[`collapse_whitespace`]** — consecutive whitespace runs become a
//!   single ASCII space; leading and trailing whitespace are dropped.
//!   Uses the Unicode `White_Space` property to identify whitespace
//!   scalars.
//! - **[`normalize_line_endings`]** — rewrites `\r\n`, `\n`, and `\r` to
//!   one of [`LineEnding::Lf`], [`LineEnding::CrLf`], or
//!   [`LineEnding::Cr`].
//! - **[`strip_control`]** — removes Unicode control scalars (categories
//!   `Cc`, `Cf`, plus the C0 and C1 ranges) but preserves the common
//!   whitespace controls (`\t`, `\n`, `\r`, `\u{0020}`).
//! - **[`strip_ansi`]** — removes ANSI escape sequences: CSI (`ESC [ ...
//!   final`), OSC (`ESC ] ... ST/BEL`), and simple two-byte `ESC X`
//!   escapes. This is the common "clean up my terminal output" helper.
//! - **[`nfc`], [`nfd`], [`nfkc`], [`nfkd`]** — thin re-exports of the
//!   Unicode normalization forms exposed by [`stringcheese_unicode`].
//! - **[`normalize_quotes`]** — typographic single and double quotes
//!   become ASCII `'` and `"`.
//! - **[`normalize_dashes`]** — em-dash `—` becomes `--`; en-dash `–`
//!   becomes `-`.
//! - **[`normalize_ellipsis`]** — the single-scalar horizontal ellipsis
//!   `…` becomes the three-dot ASCII sequence `...`.
//!
//! # Idempotence
//!
//! Every function in this module is **idempotent**: applying it twice
//! yields the same result as applying it once. Property tests confirm
//! this for every variant.
//!
//! # Allocation profile
//!
//! Every owned-output function allocates exactly one `String` for the
//! result. Internal buffers are reused where possible.
//!
//! # `no_std`
//!
//! Every item in this module is gated on `feature = "alloc"`: an owned
//! `String` output requires the heap. The Unicode normalization re-exports
//! additionally require [`stringcheese_unicode`], which itself requires
//! `alloc`.

#![cfg(feature = "alloc")]

use alloc::string::String;

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------
// Whitespace / line endings / control / ANSI cleanup.
// ---------------------------------------------------------------------

/// Collapses consecutive whitespace runs in `s` into a single ASCII
/// space and strips leading and trailing whitespace.
///
/// Whitespace is identified by the Unicode `White_Space` property (same
/// definition [`str::trim`] uses). The output contains only non-space
/// scalars separated by exactly one `' '` (`U+0020`), regardless of
/// which whitespace scalars appeared in the input.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::normalize;
///
/// assert_eq!(normalize::collapse_whitespace("  hello   world  "), "hello world");
/// assert_eq!(normalize::collapse_whitespace("a\t\tb\n\nc"), "a b c");
/// // U+00A0 NBSP is whitespace under Unicode:
/// assert_eq!(normalize::collapse_whitespace("x\u{00A0}\u{00A0}y"), "x y");
/// // All-whitespace input becomes empty.
/// assert_eq!(normalize::collapse_whitespace("   "), "");
/// ```
#[must_use]
pub fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_run = false;
    let mut any_written = false;
    for c in s.chars() {
        if c.is_whitespace() {
            in_run = true;
        } else {
            if in_run && any_written {
                out.push(' ');
            }
            out.push(c);
            any_written = true;
            in_run = false;
        }
    }
    out
}

/// Which line ending [`normalize_line_endings`] should rewrite to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineEnding {
    /// Unix / Linux / macOS convention: `\n`.
    Lf,
    /// Windows convention: `\r\n`.
    CrLf,
    /// Classic Mac / some device output: `\r`.
    Cr,
}

impl LineEnding {
    /// Returns the byte string this line-ending style writes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
            LineEnding::Cr => "\r",
        }
    }
}

/// Rewrites every line terminator in `s` to `to`.
///
/// The three forms that are recognized as line terminators on input are
/// `\r\n`, `\n`, and `\r` — this covers Windows, Unix, and classic Mac
/// conventions. `\r\n` is treated as a single terminator (not two).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::normalize::{normalize_line_endings, LineEnding};
///
/// assert_eq!(
///     normalize_line_endings("a\r\nb\nc\rd", LineEnding::Lf),
///     "a\nb\nc\nd"
/// );
/// assert_eq!(
///     normalize_line_endings("a\nb", LineEnding::CrLf),
///     "a\r\nb"
/// );
/// ```
#[must_use]
pub fn normalize_line_endings(s: &str, to: LineEnding) -> String {
    let out_sep = to.as_str();
    // Worst case: every existing single-byte '\n' becomes "\r\n". Reserve
    // for that, capped at input length + input line count * (to.len() - 1).
    let mut out = String::with_capacity(s.len() + out_sep.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\r' {
            out.push_str(out_sep);
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                i += 2;
            } else {
                i += 1;
            }
        } else if b == b'\n' {
            out.push_str(out_sep);
            i += 1;
        } else {
            // Multi-byte scalars start with a byte in 0x80..=0xFF; they
            // never look like `\r` or `\n`, so we can safely copy a
            // scalar at a time without breaking UTF-8. Use `char_len` to
            // step over the whole scalar.
            let len = char_len(b);
            out.push_str(&s[i..i + len]);
            i += len;
        }
    }
    out
}

/// Byte length of the UTF-8 scalar whose lead byte is `b`.
#[inline]
fn char_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b < 0xC0 {
        // Continuation byte in isolation — should not happen in valid
        // UTF-8, but treat as one byte to make forward progress rather
        // than panic.
        1
    } else if b < 0xE0 {
        2
    } else if b < 0xF0 {
        3
    } else {
        4
    }
}

/// Removes Unicode control scalars from `s`, preserving the common
/// whitespace controls (`\t`, `\n`, `\r`, space).
///
/// A scalar is treated as a control if it satisfies [`char::is_control`]
/// — that covers the C0 (`U+0000..=U+001F`), the delete character
/// (`U+007F`), and the C1 (`U+0080..=U+009F`) ranges. Space, tab,
/// newline, and carriage return are explicitly *kept* because they are
/// almost always meaningful in text.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::normalize;
///
/// // Bell (\x07) is dropped, tab and newline are kept.
/// assert_eq!(normalize::strip_control("a\x07b\tc\nd"), "ab\tc\nd");
/// // DEL is stripped.
/// assert_eq!(normalize::strip_control("hi\x7fthere"), "hithere");
/// ```
#[must_use]
pub fn strip_control(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_control() {
            if matches!(c, '\t' | '\n' | '\r') {
                out.push(c);
            }
            // else: drop the control scalar.
        } else {
            out.push(c);
        }
    }
    out
}

/// Removes ANSI escape sequences from `s`.
///
/// The common terminal-control shapes are recognized:
///
/// - **CSI** — `ESC [` parameter-bytes intermediate-bytes final-byte.
///   The final byte is any of `@..=~` (`0x40..=0x7E`). Covers colored
///   output (`\x1b[31m`), cursor movement, screen clearing, etc.
/// - **OSC** — `ESC ]` any bytes, terminated by `BEL` (`\x07`) or
///   `ESC \` (the "String Terminator", `\x1b\\`).
/// - **Simple two-byte escapes** — `ESC` followed by any single byte in
///   `0x20..=0x7E`. Covers the DEC private `ESC 7` / `ESC 8` (save /
///   restore cursor), the `Fe` C1-control escapes, and the `Fs`
///   standard escapes.
///
/// Any stray `ESC` at end-of-input is dropped alone. Non-escape bytes
/// pass through unchanged.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::normalize;
///
/// // Colored "red text" reduces to just "red text".
/// assert_eq!(normalize::strip_ansi("\x1b[31mred\x1b[0m"), "red");
/// // OSC (window title) is stripped whole.
/// assert_eq!(normalize::strip_ansi("hi\x1b]0;title\x07there"), "hithere");
/// // No escapes → unchanged.
/// assert_eq!(normalize::strip_ansi("plain text"), "plain text");
/// ```
#[must_use]
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1B {
            i += 1;
            if i >= bytes.len() {
                break;
            }
            match bytes[i] {
                b'[' => {
                    // CSI: consume until a byte in 0x40..=0x7E (final byte).
                    i += 1;
                    while i < bytes.len() && !(0x40..=0x7E).contains(&bytes[i]) {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1; // skip the final byte
                    }
                }
                b']' => {
                    // OSC: consume until BEL or ESC \.
                    i += 1;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1B && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                0x20..=0x7E => {
                    // Simple two-byte escape (any printable byte other
                    // than '[' and ']' handled above): DEC ESC 7 /
                    // ESC 8, Fe C1-control escapes, Fs standard
                    // escapes. Consume the one following byte.
                    i += 1;
                }
                _ => {
                    // Unknown escape (non-printable follower) — drop
                    // just the ESC byte and reconsider the next byte at
                    // the top of the loop.
                }
            }
        } else {
            // Copy a whole scalar so we never split multi-byte UTF-8.
            let len = char_len(bytes[i]);
            out.push_str(&s[i..i + len]);
            i += len;
        }
    }
    out
}

// ---------------------------------------------------------------------
// Unicode normalization forms — delegates to `stringcheese_unicode`.
// ---------------------------------------------------------------------

/// Returns the NFC (Normalization Form Canonical Composition) of `s`.
///
/// Thin re-export of [`stringcheese_unicode::nfc`]. Two strings that
/// look identical but were entered differently — e.g. precomposed
/// `"caf\u{00E9}"` and decomposed `"cafe\u{0301}"` — become byte-for-byte
/// equal after NFC.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::normalize;
///
/// // Decomposed é (e + combining acute) composes to precomposed é.
/// assert_eq!(normalize::nfc("cafe\u{0301}"), "caf\u{00E9}");
/// ```
#[must_use]
#[inline]
pub fn nfc(s: &str) -> String {
    stringcheese_unicode::nfc(s)
}

/// Returns the NFD (Normalization Form Canonical Decomposition) of `s`.
///
/// Thin re-export of [`stringcheese_unicode::nfd`]. Precomposed accented
/// letters decompose into base + combining marks.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::normalize;
///
/// assert_eq!(normalize::nfd("caf\u{00E9}"), "cafe\u{0301}");
/// ```
#[must_use]
#[inline]
pub fn nfd(s: &str) -> String {
    stringcheese_unicode::nfd(s)
}

/// Returns the NFKC (Normalization Form Compatibility Composition) of
/// `s`.
///
/// Thin re-export of [`stringcheese_unicode::nfkc`]. Applies
/// compatibility mappings — full-width ASCII becomes plain ASCII, and
/// so on — then composes.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::normalize;
///
/// // Full-width digit '１' (U+FF11) reduces to plain '1'.
/// assert_eq!(normalize::nfkc("\u{FF11}"), "1");
/// ```
#[must_use]
#[inline]
pub fn nfkc(s: &str) -> String {
    stringcheese_unicode::nfkc(s)
}

/// Returns the NFKD (Normalization Form Compatibility Decomposition) of
/// `s`.
///
/// Thin re-export of [`stringcheese_unicode::nfkd`].
///
/// # Examples
///
/// ```
/// use stringcheese_manip::normalize;
///
/// // Precomposed é (with a compatibility form) decomposes.
/// assert_eq!(normalize::nfkd("caf\u{00E9}"), "cafe\u{0301}");
/// ```
#[must_use]
#[inline]
pub fn nfkd(s: &str) -> String {
    stringcheese_unicode::nfkd(s)
}

// ---------------------------------------------------------------------
// Typographic normalization.
// ---------------------------------------------------------------------

/// Replaces typographic single and double quotes in `s` with their
/// ASCII equivalents.
///
/// The following scalars are rewritten:
///
/// | From | To |
/// |------|----|
/// | `\u{201C}` `"` LEFT DOUBLE QUOTATION MARK | `"` |
/// | `\u{201D}` `"` RIGHT DOUBLE QUOTATION MARK | `"` |
/// | `\u{2018}` `'` LEFT SINGLE QUOTATION MARK | `'` |
/// | `\u{2019}` `'` RIGHT SINGLE QUOTATION MARK | `'` |
///
/// Every other character passes through unchanged.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::normalize;
///
/// assert_eq!(
///     normalize::normalize_quotes("\u{201C}hi\u{201D} \u{2018}there\u{2019}"),
///     "\"hi\" 'there'"
/// );
/// ```
#[must_use]
pub fn normalize_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let replaced = match c {
            '\u{201C}' | '\u{201D}' => '"',
            '\u{2018}' | '\u{2019}' => '\'',
            _ => c,
        };
        out.push(replaced);
    }
    out
}

/// Replaces the em-dash and en-dash in `s` with ASCII `--` and `-`
/// respectively.
///
/// | From | To |
/// |------|----|
/// | `\u{2014}` `—` EM DASH | `--` |
/// | `\u{2013}` `–` EN DASH | `-` |
///
/// # Examples
///
/// ```
/// use stringcheese_manip::normalize;
///
/// assert_eq!(normalize::normalize_dashes("a\u{2014}b"), "a--b");
/// assert_eq!(normalize::normalize_dashes("1\u{2013}2"), "1-2");
/// ```
#[must_use]
pub fn normalize_dashes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{2014}' => out.push_str("--"),
            '\u{2013}' => out.push('-'),
            _ => out.push(c),
        }
    }
    out
}

/// Replaces the single-scalar horizontal ellipsis `\u{2026}` in `s` with
/// the three-dot ASCII sequence `...`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::normalize;
///
/// assert_eq!(normalize::normalize_ellipsis("wait\u{2026}"), "wait...");
/// ```
#[must_use]
pub fn normalize_ellipsis(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '\u{2026}' {
            out.push_str("...");
        } else {
            out.push(c);
        }
    }
    out
}
