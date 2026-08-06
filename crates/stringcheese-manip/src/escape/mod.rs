//! Encode a string for a target syntax.
//!
//! Every function in this module transforms a `&str` so that its bytes
//! are safe to embed in a particular downstream syntax — HTML, JSON,
//! shell, URLs, C source, regular expressions. The inverse functions
//! (where applicable) decode the same shape back into raw text.
//!
//! # Return-type policy
//!
//! Whenever the input might not need any escaping at all — the common
//! case for ASCII, alphanumeric content — the function returns
//! [`Cow<str>`] instead of an owned `String`, so callers pay a heap
//! allocation only when the input actually contains characters that
//! must be encoded. Where every call is guaranteed to change the string
//! (percent-encoding, C-string escaping, regex escaping) the return type
//! is `String` directly.
//!
//! # What this module does *not* do
//!
//! - **No parsing.** The encoders take raw `&str`; the decoders return
//!   the raw `&str` they were originally passed. Higher-level parsing —
//!   HTML documents, JSON values, shell command lines — is a downstream
//!   concern.
//! - **No sanitization.** `escape_html` makes a string safe to embed in
//!   HTML text nodes and attribute values, but it does not sanitize
//!   dangerous input (script tags in `href` attributes, style-injection,
//!   etc.). Use a full sanitizer library for user-supplied HTML.
//! - **No context-aware encoding.** The caller picks the encoding; this
//!   module does not detect the target syntax from context.
//!
//! # `no_std`
//!
//! Every item is gated on `feature = "alloc"` — `Cow<str>` and `String`
//! require the heap. A pure-no-alloc build gets an empty surface.

#![cfg(feature = "alloc")]

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

#[cfg(test)]
mod tests;

// =====================================================================
// HTML
// =====================================================================

/// Encodes `s` so that it is safe to embed in HTML text or in a double-
/// or single-quoted attribute value.
///
/// The five characters `<`, `>`, `&`, `'`, `"` become their named
/// entities (`&lt;`, `&gt;`, `&amp;`, `&#39;`, `&quot;`). If the input
/// contains none of these, the function returns `Cow::Borrowed` and
/// performs no allocation.
///
/// `&#39;` is used for the apostrophe rather than `&apos;` because
/// `&apos;` is not defined in HTML 4 and is inconsistently supported by
/// older parsers; the numeric form is universally understood.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::escape;
///
/// assert_eq!(escape::escape_html("hello"), "hello");
/// assert_eq!(escape::escape_html("<b>&amp;</b>"), "&lt;b&gt;&amp;amp;&lt;/b&gt;");
/// assert_eq!(escape::escape_html("Tom's \"pet\""), "Tom&#39;s &quot;pet&quot;");
/// ```
#[must_use]
pub fn escape_html(s: &str) -> Cow<'_, str> {
    let needs = s
        .bytes()
        .any(|b| matches!(b, b'<' | b'>' | b'&' | b'\'' | b'"'));
    if !needs {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&#39;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// Decodes HTML entities in `s`.
///
/// Handles the named entities [`escape_html`] produces (`&lt;`, `&gt;`,
/// `&amp;`, `&quot;`, `&apos;`, `&#39;`), the additional common name
/// `&nbsp;` (U+00A0 no-break space), and numeric character references
/// in decimal (`&#123;`) and hexadecimal (`&#x1F;`, `&#X1F;`).
///
/// Unknown or malformed entities are passed through *verbatim* — a
/// stray `&foo;` in the input becomes `&foo;` in the output. This
/// preserves the original text rather than raising an error, which
/// matches the browser-side "be liberal in what you accept" convention.
/// Numeric references to invalid Unicode scalar values (surrogates,
/// codepoints above U+10FFFF) are also passed through unchanged.
///
/// Returns `Cow::Borrowed(s)` if no entities were present.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::escape;
///
/// assert_eq!(escape::unescape_html("hello"), "hello");
/// assert_eq!(
///     escape::unescape_html("&lt;b&gt;&amp;&lt;/b&gt;"),
///     "<b>&</b>"
/// );
/// assert_eq!(escape::unescape_html("&#65;&#x42;"), "AB");
/// // Unknown entities pass through unchanged:
/// assert_eq!(escape::unescape_html("&notanentity;"), "&notanentity;");
/// ```
#[must_use]
pub fn unescape_html(s: &str) -> Cow<'_, str> {
    if !s.contains('&') {
        return Cow::Borrowed(s);
    }
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'&' {
            // Copy the current UTF-8 scalar as a whole. bytes[i] is the
            // start of a valid scalar because `s` is a `&str`.
            let scalar_len = utf8_scalar_len(bytes[i]);
            // SAFETY-EQUIVALENT: we know `s` is UTF-8, so this slice is
            // a valid scalar. Use `str::get` for defence in depth.
            let end = i + scalar_len;
            if let Some(chunk) = s.get(i..end) {
                out.push_str(chunk);
            }
            i = end;
            continue;
        }
        // Look for the closing `;` within a bounded window. The longest
        // legitimate entity we handle is `&#x10FFFF;` (10 bytes).
        let scan_end = (i + 12).min(bytes.len());
        let semi = bytes[i + 1..scan_end].iter().position(|&b| b == b';');
        let Some(rel) = semi else {
            out.push('&');
            i += 1;
            continue;
        };
        let end = i + 1 + rel + 1; // one past the ';'
        let name = &s[i + 1..end - 1];
        if let Some(resolved) = resolve_entity(name) {
            out.push(resolved);
            i = end;
        } else {
            out.push('&');
            i += 1;
        }
    }
    Cow::Owned(out)
}

fn resolve_entity(name: &str) -> Option<char> {
    match name {
        "lt" => Some('<'),
        "gt" => Some('>'),
        "amp" => Some('&'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{00A0}'),
        _ => {
            let mut chars = name.chars();
            if chars.next() != Some('#') {
                return None;
            }
            let rest = chars.as_str();
            let (radix, digits) =
                if let Some(hex) = rest.strip_prefix('x').or_else(|| rest.strip_prefix('X')) {
                    (16u32, hex)
                } else {
                    (10u32, rest)
                };
            if digits.is_empty() {
                return None;
            }
            let code = u32::from_str_radix(digits, radix).ok()?;
            char::from_u32(code)
        }
    }
}

// A byte at position 0 of a valid UTF-8 scalar tells you how many bytes
// the scalar occupies. `&str` guarantees the input is valid UTF-8, so
// this lookup is total.
fn utf8_scalar_len(lead: u8) -> usize {
    if lead < 0x80 {
        1
    } else if lead < 0xC0 {
        // Continuation byte — should not appear as a leading byte in
        // valid UTF-8, but if it did we advance by 1 to make progress.
        1
    } else if lead < 0xE0 {
        2
    } else if lead < 0xF0 {
        3
    } else {
        4
    }
}

// =====================================================================
// JSON
// =====================================================================

/// Error returned by [`unescape_json`] when the input contains a
/// malformed JSON escape sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonUnescapeError {
    /// Byte offset into the input where the malformed escape starts.
    pub position: usize,
    /// Human-readable description.
    pub kind: JsonUnescapeErrorKind,
}

/// Reason a [`JsonUnescapeError`] was raised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonUnescapeErrorKind {
    /// A `\` appeared at the end of the string with no following
    /// escape character.
    TrailingBackslash,
    /// The character after `\` is not a recognized escape.
    InvalidEscape(char),
    /// `\u` was not followed by four hex digits.
    InvalidUnicodeEscape,
    /// A `\uXXXX` surrogate could not be paired with a valid low
    /// surrogate `\uYYYY` sequence.
    UnpairedSurrogate,
}

impl fmt::Display for JsonUnescapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "malformed JSON escape at byte {}: ", self.position)?;
        match &self.kind {
            JsonUnescapeErrorKind::TrailingBackslash => f.write_str("trailing backslash"),
            JsonUnescapeErrorKind::InvalidEscape(c) => write!(f, "invalid escape \\{c}"),
            JsonUnescapeErrorKind::InvalidUnicodeEscape => {
                f.write_str("\\u must be followed by four hex digits")
            }
            JsonUnescapeErrorKind::UnpairedSurrogate => f.write_str("unpaired UTF-16 surrogate"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for JsonUnescapeError {}

/// Encodes `s` as the *contents* of a JSON string literal (the surrounding
/// double quotes are not added).
///
/// Applies the standard JSON string escapes: `\` → `\\`, `"` → `\"`,
/// `\n` → `\n`, `\r` → `\r`, `\t` → `\t`, `\x08` → `\b`, `\x0C` → `\f`,
/// and every other C0 control character (U+0000..=U+001F) is escaped
/// as `\u00XX`. All other characters — including non-ASCII scalars —
/// are passed through unchanged.
///
/// Returns `Cow::Borrowed(s)` when no escaping is needed. The output is
/// pure ASCII only when the input is; non-ASCII scalars pass through as
/// their raw UTF-8 bytes because JSON permits arbitrary Unicode in
/// string literals.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::escape;
///
/// assert_eq!(escape::escape_json("hello"), "hello");
/// assert_eq!(escape::escape_json("a\"b\\c"), "a\\\"b\\\\c");
/// assert_eq!(escape::escape_json("line\nfeed"), "line\\nfeed");
/// assert_eq!(escape::escape_json("\x01"), "\\u0001");
/// ```
#[must_use]
pub fn escape_json(s: &str) -> Cow<'_, str> {
    let needs = s.bytes().any(|b| matches!(b, b'"' | b'\\') || b < 0x20);
    if !needs {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                // Any remaining C0 control: emit \u00XX. `c` is < 0x20
                // so the top nibble is either 0 or 1.
                let byte = c as u32;
                out.push_str("\\u00");
                out.push(if byte < 0x10 { '0' } else { '1' });
                // Low nibble is < 16, so masking then a table lookup
                // avoids the u32 -> u8 truncation cast entirely.
                out.push(hex_nibble_from_u32(byte));
            }
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// Decodes the contents of a JSON string literal.
///
/// Handles the escape sequences [`escape_json`] produces plus the
/// `\/` alias for `/` and `\uXXXX` unicode escapes (including surrogate
/// pairs for astral scalars, e.g. `😀` → `😀`).
///
/// Returns `Cow::Borrowed(s)` if `s` contains no backslash. Returns
/// [`JsonUnescapeError`] for any malformed escape.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::escape;
///
/// assert_eq!(escape::unescape_json("hello").unwrap(), "hello");
/// assert_eq!(escape::unescape_json("a\\\"b").unwrap(), "a\"b");
/// assert_eq!(escape::unescape_json("\\u0041").unwrap(), "A");
/// // Surrogate pair — U+1F600 GRINNING FACE.
/// assert_eq!(
///     escape::unescape_json("\\uD83D\\uDE00").unwrap(),
///     "\u{1F600}"
/// );
/// ```
///
/// # Errors
///
/// Returns [`JsonUnescapeError`] on trailing `\`, unknown `\X` escape,
/// malformed `\uXXXX` sequence, or unpaired surrogate.
#[allow(clippy::too_many_lines)] // straight-line escape switch.
pub fn unescape_json(s: &str) -> Result<Cow<'_, str>, JsonUnescapeError> {
    if !s.contains('\\') {
        return Ok(Cow::Borrowed(s));
    }
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' {
            let end = i + utf8_scalar_len(bytes[i]);
            if let Some(chunk) = s.get(i..end) {
                out.push_str(chunk);
            }
            i = end;
            continue;
        }
        // Backslash — consume the escape.
        let esc_pos = i;
        let Some(&next) = bytes.get(i + 1) else {
            return Err(JsonUnescapeError {
                position: esc_pos,
                kind: JsonUnescapeErrorKind::TrailingBackslash,
            });
        };
        match next {
            b'"' => {
                out.push('"');
                i += 2;
            }
            b'\\' => {
                out.push('\\');
                i += 2;
            }
            b'/' => {
                out.push('/');
                i += 2;
            }
            b'n' => {
                out.push('\n');
                i += 2;
            }
            b'r' => {
                out.push('\r');
                i += 2;
            }
            b't' => {
                out.push('\t');
                i += 2;
            }
            b'b' => {
                out.push('\u{0008}');
                i += 2;
            }
            b'f' => {
                out.push('\u{000C}');
                i += 2;
            }
            b'u' => {
                let code = parse_hex4(bytes, i + 2).ok_or(JsonUnescapeError {
                    position: esc_pos,
                    kind: JsonUnescapeErrorKind::InvalidUnicodeEscape,
                })?;
                if (0xD800..=0xDBFF).contains(&code) {
                    // High surrogate — expect a paired low surrogate.
                    if bytes.get(i + 6) != Some(&b'\\') || bytes.get(i + 7) != Some(&b'u') {
                        return Err(JsonUnescapeError {
                            position: esc_pos,
                            kind: JsonUnescapeErrorKind::UnpairedSurrogate,
                        });
                    }
                    let low = parse_hex4(bytes, i + 8).ok_or(JsonUnescapeError {
                        position: esc_pos,
                        kind: JsonUnescapeErrorKind::UnpairedSurrogate,
                    })?;
                    if !(0xDC00..=0xDFFF).contains(&low) {
                        return Err(JsonUnescapeError {
                            position: esc_pos,
                            kind: JsonUnescapeErrorKind::UnpairedSurrogate,
                        });
                    }
                    let scalar = 0x10000 + (((code - 0xD800) << 10) | (low - 0xDC00));
                    let c = char::from_u32(scalar).ok_or(JsonUnescapeError {
                        position: esc_pos,
                        kind: JsonUnescapeErrorKind::UnpairedSurrogate,
                    })?;
                    out.push(c);
                    i += 12;
                } else if (0xDC00..=0xDFFF).contains(&code) {
                    // Low surrogate without a preceding high surrogate.
                    return Err(JsonUnescapeError {
                        position: esc_pos,
                        kind: JsonUnescapeErrorKind::UnpairedSurrogate,
                    });
                } else {
                    let c = char::from_u32(code).ok_or(JsonUnescapeError {
                        position: esc_pos,
                        kind: JsonUnescapeErrorKind::InvalidUnicodeEscape,
                    })?;
                    out.push(c);
                    i += 6;
                }
            }
            other => {
                return Err(JsonUnescapeError {
                    position: esc_pos,
                    kind: JsonUnescapeErrorKind::InvalidEscape(other as char),
                });
            }
        }
    }
    Ok(Cow::Owned(out))
}

fn parse_hex4(bytes: &[u8], start: usize) -> Option<u32> {
    if start + 4 > bytes.len() {
        return None;
    }
    let mut n: u32 = 0;
    for &b in &bytes[start..start + 4] {
        let d = match b {
            b'0'..=b'9' => u32::from(b - b'0'),
            b'a'..=b'f' => u32::from(b - b'a') + 10,
            b'A'..=b'F' => u32::from(b - b'A') + 10,
            _ => return None,
        };
        n = (n << 4) | d;
    }
    Some(n)
}

fn hex_nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + (n - 10)) as char,
        _ => '0',
    }
}

// Same as [`hex_nibble`] but accepts a `u32`, masks to the low nibble,
// and returns a hex digit — avoids the `u32 as u8` truncation cast at
// call sites that already have a `u32` in hand.
fn hex_nibble_from_u32(n: u32) -> char {
    // The mask keeps only the low four bits; the lookup is total on
    // 0..=15.
    const HEX: [u8; 16] = *b"0123456789abcdef";
    HEX[(n & 0x0F) as usize] as char
}

// =====================================================================
// Shell
// =====================================================================

/// Escapes `s` for use as a single POSIX shell argument.
///
/// The output is safe to embed unquoted in a shell command line: it will
/// be passed to the program as a single argument whose value is exactly
/// `s`, even in the presence of whitespace, glob metacharacters, `$`,
/// `` ` ``, or embedded quotes.
///
/// The encoding strategy is:
///
/// - **Safe-identifier fast path.** If every byte of `s` is in the
///   ASCII set `[A-Za-z0-9_@%+=:,./-]` and `s` is non-empty, `s` is
///   already shell-safe and is returned as `Cow::Borrowed`.
/// - **Otherwise**, wrap in single quotes, replacing every embedded
///   single quote with `'\''` (close-quote, backslash-quote,
///   open-quote). Empty strings become `''`.
///
/// This is the same rule as Python's `shlex.quote`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::escape;
///
/// assert_eq!(escape::escape_shell_posix("hello"), "hello");
/// assert_eq!(escape::escape_shell_posix("hello world"), "'hello world'");
/// assert_eq!(escape::escape_shell_posix("it's"), "'it'\\''s'");
/// assert_eq!(escape::escape_shell_posix(""), "''");
/// ```
#[must_use]
pub fn escape_shell_posix(s: &str) -> Cow<'_, str> {
    if !s.is_empty() && s.bytes().all(is_shell_posix_safe) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    Cow::Owned(out)
}

fn is_shell_posix_safe(b: u8) -> bool {
    matches!(
        b,
        b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'_'
            | b'@'
            | b'%'
            | b'+'
            | b'='
            | b':'
            | b','
            | b'.'
            | b'/'
            | b'-'
    )
}

/// Escapes `s` for use as a single argument to the Windows `cmd.exe`
/// shell.
///
/// Windows argument parsing has two layers: `CommandLineToArgvW` (used
/// by most programs to split their command line into argv) and
/// `cmd.exe`'s own metacharacter handling. This function protects against
/// both:
///
/// 1. The value is wrapped in double quotes so that `CommandLineToArgvW`
///    treats it as one argument, with embedded `"` escaped as `\"` and
///    trailing backslashes doubled per the documented parsing rules.
/// 2. The `cmd.exe` metacharacters `& | < > ^ ( ) %` are prefixed with
///    `^` outside of the quoted portion — but since we always wrap in
///    quotes, embedded metacharacters are already protected by the quote
///    layer. The `^`-prefixing kicks in only when the outer quote itself
///    needs to survive `cmd.exe`'s pre-processing, which is handled by
///    escaping the whole result with a leading `^` on each such
///    character.
///
/// The fast path: if `s` is non-empty and every byte is in
/// `[A-Za-z0-9_.\-]`, the input is returned as `Cow::Borrowed`.
///
/// **Caveat.** Windows quoting is a labyrinth of edge cases; this
/// function targets the common case of "pass this exact string as a
/// single argument to a well-behaved program". Programs with unusual
/// argv parsers (some PowerShell scenarios, batch files invoked via
/// `cmd /c`) may require additional care.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::escape;
///
/// assert_eq!(escape::escape_shell_windows("hello"), "hello");
/// assert_eq!(escape::escape_shell_windows("hello world"), "\"hello world\"");
/// assert_eq!(escape::escape_shell_windows("a\"b"), "\"a\\\"b\"");
/// assert_eq!(escape::escape_shell_windows("path\\"), "\"path\\\\\"");
/// ```
#[must_use]
pub fn escape_shell_windows(s: &str) -> Cow<'_, str> {
    if !s.is_empty() && s.bytes().all(is_shell_windows_safe) {
        return Cow::Borrowed(s);
    }
    // CommandLineToArgvW rules: to embed a `"` inside a double-quoted
    // token, precede it with a backslash; and a run of N backslashes
    // immediately before a `"` (or before the closing `"`) must be
    // doubled.
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let mut backslashes = 0;
        while i < bytes.len() && bytes[i] == b'\\' {
            backslashes += 1;
            i += 1;
        }
        if i == bytes.len() {
            // Trailing backslashes before the closing quote — double them.
            for _ in 0..(backslashes * 2) {
                out.push('\\');
            }
        } else if bytes[i] == b'"' {
            for _ in 0..=(backslashes * 2) {
                out.push('\\');
            }
            out.push('"');
            i += 1;
        } else {
            for _ in 0..backslashes {
                out.push('\\');
            }
            // Push one scalar starting at i.
            let end = i + utf8_scalar_len(bytes[i]);
            if let Some(chunk) = s.get(i..end) {
                out.push_str(chunk);
            }
            i = end;
        }
    }
    out.push('"');
    // The outer double quotes survive `cmd.exe` only if no cmd
    // metacharacters appear in the raw string; when they do, prefix each
    // cmd metacharacter of the escaped output (including its own `"`)
    // with `^`. We do this at the byte layer and only over the ASCII
    // metachar set, so multi-byte scalars are unaffected.
    if s.bytes().any(is_cmd_metachar) {
        let mut carat = String::with_capacity(out.len() * 2);
        for b in out.bytes() {
            if is_cmd_metachar(b) || b == b'"' {
                carat.push('^');
            }
            carat.push(b as char);
        }
        return Cow::Owned(carat);
    }
    Cow::Owned(out)
}

fn is_shell_windows_safe(b: u8) -> bool {
    matches!(
        b,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'
    )
}

fn is_cmd_metachar(b: u8) -> bool {
    matches!(
        b,
        b'&' | b'|' | b'<' | b'>' | b'^' | b'(' | b')' | b'%' | b'!'
    )
}

// =====================================================================
// Percent-encoding (RFC 3986)
// =====================================================================

/// Which characters do *not* need percent-encoding.
///
/// RFC 3986 partitions URI characters into "unreserved" (safe in every
/// component) and "reserved" (delimiters that carry meaning). Different
/// components allow different subsets of the reserved set unencoded;
/// this enum names the four common component encodings.
///
/// The "unreserved" set — `A-Za-z0-9`, `-`, `.`, `_`, `~` — is safe in
/// every variant. Each variant additionally permits its component's
/// sub-delims and legal reserved chars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PercentSet {
    /// Encoding for a URI path segment. In addition to the unreserved
    /// set, permits `!$&'()*+,;=:@`.
    Path,
    /// Encoding for a URI query component. Permits the unreserved set
    /// plus `!$'()*+,;:@/?`. Space becomes `%20` (not `+`; use a
    /// dedicated `application/x-www-form-urlencoded` encoder for form
    /// data).
    Query,
    /// Encoding for a URI fragment. Same permitted set as [`PercentSet::Query`].
    Fragment,
    /// Encoding for the userinfo component (before the `@` in an
    /// authority). Permits the unreserved set plus `!$&'()*+,;=`.
    Userinfo,
}

impl PercentSet {
    /// `true` if `b` is safe in this encoding and must NOT be
    /// percent-encoded.
    fn is_allowed(self, b: u8) -> bool {
        // The unreserved set is common to all four variants.
        if matches!(
            b,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~'
        ) {
            return true;
        }
        match self {
            PercentSet::Path => matches!(
                b,
                b'!' | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
            ),
            PercentSet::Query | PercentSet::Fragment => matches!(
                b,
                b'!' | b'$'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b':'
                    | b'@'
                    | b'/'
                    | b'?'
            ),
            PercentSet::Userinfo => matches!(
                b,
                b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
            ),
        }
    }
}

/// Percent-encodes `s` for use in the given URI component.
///
/// Every byte that is not in the allowed set for `allowed` is replaced
/// with `%XX` where `XX` is the byte's two-digit uppercase hexadecimal
/// value. Multi-byte scalars are percent-encoded byte-by-byte, which is
/// what RFC 3986 requires.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::escape::{percent_encode, PercentSet};
///
/// assert_eq!(percent_encode("hello", PercentSet::Path), "hello");
/// assert_eq!(percent_encode("a b", PercentSet::Path), "a%20b");
/// assert_eq!(percent_encode("a/b", PercentSet::Path), "a%2Fb");
/// // Non-ASCII scalars encode byte-by-byte:
/// assert_eq!(percent_encode("é", PercentSet::Path), "%C3%A9");
/// ```
#[must_use]
pub fn percent_encode(s: &str, allowed: PercentSet) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if allowed.is_allowed(b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(hex_nibble(b >> 4).to_ascii_uppercase());
            out.push(hex_nibble(b & 0x0F).to_ascii_uppercase());
        }
    }
    out
}

/// Error returned by [`percent_decode`] when the input contains an
/// invalid `%XX` sequence or the decoded byte stream is not valid UTF-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PercentDecodeError {
    /// Byte offset into the input where the malformed sequence starts.
    pub position: usize,
    /// Human-readable description.
    pub kind: PercentDecodeErrorKind,
}

/// Reason a [`PercentDecodeError`] was raised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PercentDecodeErrorKind {
    /// A `%` was not followed by two hex digits.
    InvalidEscape,
    /// The decoded byte sequence is not valid UTF-8.
    InvalidUtf8,
}

impl fmt::Display for PercentDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "percent-decode error at byte {}: ", self.position)?;
        match self.kind {
            PercentDecodeErrorKind::InvalidEscape => {
                f.write_str("% must be followed by two hex digits")
            }
            PercentDecodeErrorKind::InvalidUtf8 => f.write_str("decoded bytes are not valid UTF-8"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PercentDecodeError {}

/// Decodes every `%XX` sequence in `s` and returns the resulting string.
///
/// Non-`%` bytes are passed through unchanged. The decoded byte stream
/// must be valid UTF-8; if it is not, returns [`PercentDecodeError`]
/// with kind [`PercentDecodeErrorKind::InvalidUtf8`].
///
/// # Examples
///
/// ```
/// use stringcheese_manip::escape;
///
/// assert_eq!(escape::percent_decode("hello").unwrap(), "hello");
/// assert_eq!(escape::percent_decode("a%20b").unwrap(), "a b");
/// assert_eq!(escape::percent_decode("%C3%A9").unwrap(), "é");
/// ```
///
/// # Errors
///
/// Returns [`PercentDecodeError`] on a malformed `%XX` escape or on a
/// decoded byte sequence that is not valid UTF-8.
pub fn percent_decode(s: &str) -> Result<String, PercentDecodeError> {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(PercentDecodeError {
                    position: i,
                    kind: PercentDecodeErrorKind::InvalidEscape,
                });
            }
            let hi = hex_value(bytes[i + 1]).ok_or(PercentDecodeError {
                position: i,
                kind: PercentDecodeErrorKind::InvalidEscape,
            })?;
            let lo = hex_value(bytes[i + 2]).ok_or(PercentDecodeError {
                position: i,
                kind: PercentDecodeErrorKind::InvalidEscape,
            })?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|e| PercentDecodeError {
        position: e.utf8_error().valid_up_to(),
        kind: PercentDecodeErrorKind::InvalidUtf8,
    })
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

// =====================================================================
// C-string
// =====================================================================

/// Encodes `s` as the *contents* of a C source-code string literal (the
/// surrounding double quotes are not added).
///
/// Applies the standard C escapes: `\` → `\\`, `"` → `\"`, `\n` → `\n`,
/// `\r` → `\r`, `\t` → `\t`. Every other non-printable ASCII byte
/// (below 0x20 or exactly 0x7F) is escaped as `\xHH`. Non-ASCII bytes
/// are also escaped as `\xHH`, byte-by-byte — this is safer than passing
/// raw UTF-8 through a source-code string literal, because many C
/// compilers do not treat source files as UTF-8 by default.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::escape;
///
/// assert_eq!(escape::escape_c_string("hello"), "hello");
/// assert_eq!(escape::escape_c_string("a\"b\\c"), "a\\\"b\\\\c");
/// assert_eq!(escape::escape_c_string("line\nfeed"), "line\\nfeed");
/// assert_eq!(escape::escape_c_string("é"), "\\xc3\\xa9");
/// ```
#[must_use]
pub fn escape_c_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7E => out.push(b as char),
            _ => {
                out.push_str("\\x");
                out.push(hex_nibble(b >> 4));
                out.push(hex_nibble(b & 0x0F));
            }
        }
    }
    out
}

/// Error returned by [`unescape_c_string`] when the input contains a
/// malformed escape sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CStringUnescapeError {
    /// Byte offset into the input where the malformed escape starts.
    pub position: usize,
    /// Human-readable description.
    pub kind: CStringUnescapeErrorKind,
}

/// Reason a [`CStringUnescapeError`] was raised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CStringUnescapeErrorKind {
    /// A `\` appeared at the end of the string with no following
    /// escape character.
    TrailingBackslash,
    /// The character after `\` is not a recognized escape.
    InvalidEscape(char),
    /// `\xHH` was not followed by two hex digits.
    InvalidHexEscape,
    /// `\uHHHH` / `\UHHHHHHHH` was not followed by enough hex digits,
    /// or the numeric value is not a valid Unicode scalar.
    InvalidUnicodeEscape,
    /// The final decoded byte stream is not valid UTF-8.
    InvalidUtf8,
}

impl fmt::Display for CStringUnescapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "malformed C-string escape at byte {}: ", self.position)?;
        match &self.kind {
            CStringUnescapeErrorKind::TrailingBackslash => f.write_str("trailing backslash"),
            CStringUnescapeErrorKind::InvalidEscape(c) => write!(f, "invalid escape \\{c}"),
            CStringUnescapeErrorKind::InvalidHexEscape => {
                f.write_str("\\x must be followed by two hex digits")
            }
            CStringUnescapeErrorKind::InvalidUnicodeEscape => {
                f.write_str("invalid \\u or \\U escape")
            }
            CStringUnescapeErrorKind::InvalidUtf8 => {
                f.write_str("decoded bytes are not valid UTF-8")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CStringUnescapeError {}

/// Decodes the contents of a C string literal.
///
/// Handles:
///
/// - Simple escapes: `\\`, `\'`, `\"`, `\?`, `\a`, `\b`, `\f`, `\n`,
///   `\r`, `\t`, `\v`, `\0`.
/// - Hex escapes: `\xHH` (exactly two hex digits).
/// - Unicode escapes: `\uHHHH` (four hex digits) and `\UHHHHHHHH`
///   (eight hex digits).
/// - Octal escapes: `\ooo` (one, two, or three octal digits, matching
///   C99 semantics — the longest prefix of up to three octal digits is
///   consumed).
///
/// The decoded byte stream is validated as UTF-8; if it is not valid,
/// returns [`CStringUnescapeError`] with kind
/// [`CStringUnescapeErrorKind::InvalidUtf8`].
///
/// # Examples
///
/// ```
/// use stringcheese_manip::escape;
///
/// assert_eq!(escape::unescape_c_string("hello").unwrap(), "hello");
/// assert_eq!(escape::unescape_c_string("a\\nb").unwrap(), "a\nb");
/// assert_eq!(escape::unescape_c_string("\\x41").unwrap(), "A");
/// assert_eq!(escape::unescape_c_string("\\u00E9").unwrap(), "é");
/// assert_eq!(escape::unescape_c_string("\\101").unwrap(), "A"); // octal
/// ```
///
/// # Errors
///
/// Returns [`CStringUnescapeError`] on any malformed escape.
#[allow(clippy::too_many_lines)] // straight-line escape switch.
pub fn unescape_c_string(s: &str) -> Result<String, CStringUnescapeError> {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let esc_pos = i;
        let Some(&next) = bytes.get(i + 1) else {
            return Err(CStringUnescapeError {
                position: esc_pos,
                kind: CStringUnescapeErrorKind::TrailingBackslash,
            });
        };
        match next {
            b'\\' => {
                out.push(b'\\');
                i += 2;
            }
            b'\'' => {
                out.push(b'\'');
                i += 2;
            }
            b'"' => {
                out.push(b'"');
                i += 2;
            }
            b'?' => {
                out.push(b'?');
                i += 2;
            }
            b'a' => {
                out.push(0x07);
                i += 2;
            }
            b'b' => {
                out.push(0x08);
                i += 2;
            }
            b'f' => {
                out.push(0x0C);
                i += 2;
            }
            b'n' => {
                out.push(b'\n');
                i += 2;
            }
            b'r' => {
                out.push(b'\r');
                i += 2;
            }
            b't' => {
                out.push(b'\t');
                i += 2;
            }
            b'v' => {
                out.push(0x0B);
                i += 2;
            }
            b'x' => {
                if i + 4 > bytes.len() {
                    return Err(CStringUnescapeError {
                        position: esc_pos,
                        kind: CStringUnescapeErrorKind::InvalidHexEscape,
                    });
                }
                let hi = hex_value(bytes[i + 2]).ok_or(CStringUnescapeError {
                    position: esc_pos,
                    kind: CStringUnescapeErrorKind::InvalidHexEscape,
                })?;
                let lo = hex_value(bytes[i + 3]).ok_or(CStringUnescapeError {
                    position: esc_pos,
                    kind: CStringUnescapeErrorKind::InvalidHexEscape,
                })?;
                out.push((hi << 4) | lo);
                i += 4;
            }
            b'u' => {
                let code = parse_hex4(bytes, i + 2).ok_or(CStringUnescapeError {
                    position: esc_pos,
                    kind: CStringUnescapeErrorKind::InvalidUnicodeEscape,
                })?;
                let c = char::from_u32(code).ok_or(CStringUnescapeError {
                    position: esc_pos,
                    kind: CStringUnescapeErrorKind::InvalidUnicodeEscape,
                })?;
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                out.extend_from_slice(s.as_bytes());
                i += 6;
            }
            b'U' => {
                if i + 10 > bytes.len() {
                    return Err(CStringUnescapeError {
                        position: esc_pos,
                        kind: CStringUnescapeErrorKind::InvalidUnicodeEscape,
                    });
                }
                let mut code: u32 = 0;
                for &b in &bytes[i + 2..i + 10] {
                    let d = hex_value(b).ok_or(CStringUnescapeError {
                        position: esc_pos,
                        kind: CStringUnescapeErrorKind::InvalidUnicodeEscape,
                    })?;
                    code = (code << 4) | u32::from(d);
                }
                let c = char::from_u32(code).ok_or(CStringUnescapeError {
                    position: esc_pos,
                    kind: CStringUnescapeErrorKind::InvalidUnicodeEscape,
                })?;
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                out.extend_from_slice(s.as_bytes());
                i += 10;
            }
            b'0'..=b'7' => {
                // Octal escape: 1 to 3 octal digits.
                let mut n: u32 = 0;
                let mut consumed = 0;
                while consumed < 3 && i + 1 + consumed < bytes.len() {
                    let b = bytes[i + 1 + consumed];
                    if !(b'0'..=b'7').contains(&b) {
                        break;
                    }
                    n = n * 8 + u32::from(b - b'0');
                    consumed += 1;
                }
                let byte = u8::try_from(n).map_err(|_| CStringUnescapeError {
                    position: esc_pos,
                    kind: CStringUnescapeErrorKind::InvalidEscape('0'),
                })?;
                out.push(byte);
                i += 1 + consumed;
            }
            other => {
                return Err(CStringUnescapeError {
                    position: esc_pos,
                    kind: CStringUnescapeErrorKind::InvalidEscape(other as char),
                });
            }
        }
    }
    String::from_utf8(out).map_err(|e| CStringUnescapeError {
        position: e.utf8_error().valid_up_to(),
        kind: CStringUnescapeErrorKind::InvalidUtf8,
    })
}

// =====================================================================
// Regex
// =====================================================================

/// Escapes `s` so that its bytes are treated as literal text by a regex
/// engine.
///
/// Every character in the standard regex metacharacter set — `.`, `^`,
/// `$`, `*`, `+`, `?`, `(`, `)`, `[`, `]`, `{`, `}`, `|`, `\`, `/`, and
/// `#` — is prefixed with a `\`. All other characters, including
/// non-ASCII scalars, are passed through unchanged.
///
/// The set covers the union of metachars across PCRE / Rust `regex` /
/// POSIX ERE conventions; passing the escaped output to any of those
/// engines yields a pattern that matches `s` literally.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::escape;
///
/// assert_eq!(escape::escape_regex("hello"), "hello");
/// assert_eq!(escape::escape_regex("a.b"), "a\\.b");
/// assert_eq!(escape::escape_regex("$100"), "\\$100");
/// assert_eq!(escape::escape_regex("(a|b)+"), "\\(a\\|b\\)\\+");
/// ```
#[must_use]
pub fn escape_regex(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if is_regex_metachar(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn is_regex_metachar(c: char) -> bool {
    matches!(
        c,
        '.' | '^'
            | '$'
            | '*'
            | '+'
            | '?'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '|'
            | '\\'
            | '/'
            | '#'
    )
}
