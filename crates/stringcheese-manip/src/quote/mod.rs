//! Wrap a string in delimiters with escaping so the result round-trips.
//!
//! Every function in this module produces a *quoted* form of the input:
//! the string wrapped in a pair of delimiter characters, with any embedded
//! occurrences of those delimiters escaped so the outer pair remains the
//! obvious start/end of the value.
//!
//! Two families cover the common cases:
//!
//! - **Fixed delimiters** — [`single_quote`], [`double_quote`],
//!   [`backtick_quote`], [`angle_quote`], [`curly_quote`],
//!   [`curly_single_quote`]. Each picks a specific delimiter pair; the
//!   inner delimiter (if any) is backslash-escaped.
//! - **Parameterised** — [`custom_quote`] takes explicit open, close,
//!   and escape characters so callers can quote with any pair.
//!
//! Style-picking:
//!
//! - [`quote_smart`] chooses `"`, `'`, or `` ` `` — whichever needs the
//!   fewest embedded escapes for the given input.
//!
//! Inverse:
//!
//! - [`unquote`] detects and strips a standard quote pair (`"`, `'`,
//!   `` ` ``, `<>`, `\u{201C}\u{201D}`, `\u{2018}\u{2019}`). It returns
//!   the inner content as a **borrowed sub-slice** (no allocation, no
//!   unescaping — escape sequences in the interior are preserved
//!   verbatim so callers can apply their own escape policy).
//! - [`is_quoted`] is `unquote(s).is_some()`.
//!
//! # `no_std`
//!
//! Every owned-`String`-returning function is gated on `feature = "alloc"`.
//! [`unquote`] and [`is_quoted`] work without any features because they
//! do not allocate.

#[cfg(test)]
mod tests;

#[cfg(feature = "alloc")]
use alloc::string::String;

// =====================================================================
// Fixed-delimiter quoters (owned-`String` output — need `alloc`).
// =====================================================================

/// Wraps `s` in single quotes, escaping every embedded `'` as `\'` and
/// every embedded `\` as `\\`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::quote;
///
/// assert_eq!(quote::single_quote(""), "''");
/// assert_eq!(quote::single_quote("hi"), "'hi'");
/// assert_eq!(quote::single_quote("it's"), "'it\\'s'");
/// assert_eq!(quote::single_quote("a\\b"), "'a\\\\b'");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn single_quote(s: &str) -> String {
    custom_quote(s, '\'', '\'', '\\')
}

/// Wraps `s` in double quotes, escaping every embedded `"` as `\"` and
/// every embedded `\` as `\\`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::quote;
///
/// assert_eq!(quote::double_quote(""), "\"\"");
/// assert_eq!(quote::double_quote("hi"), "\"hi\"");
/// assert_eq!(quote::double_quote("say \"hi\""), "\"say \\\"hi\\\"\"");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn double_quote(s: &str) -> String {
    custom_quote(s, '"', '"', '\\')
}

/// Wraps `s` in backticks, escaping every embedded `` ` `` as `` \` ``
/// and every embedded `\` as `\\`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::quote;
///
/// assert_eq!(quote::backtick_quote("code"), "`code`");
/// assert_eq!(quote::backtick_quote("`x`"), "`\\`x\\``");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn backtick_quote(s: &str) -> String {
    custom_quote(s, '`', '`', '\\')
}

/// Wraps `s` in ASCII angle brackets (`<` and `>`), escaping every
/// embedded `<` as `\<`, every embedded `>` as `\>`, and every embedded
/// `\` as `\\`. Useful for MSF-style tag output where a value must be
/// visibly bounded.
///
/// Because `angle_quote` uses different open/close characters, the
/// inverse [`unquote`] recognises the same pair.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::quote;
///
/// assert_eq!(quote::angle_quote("tag"), "<tag>");
/// assert_eq!(quote::angle_quote("a<b>c"), "<a\\<b\\>c>");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn angle_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('<');
    for c in s.chars() {
        match c {
            '<' | '>' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out.push('>');
    out
}

/// Wraps `s` in an arbitrary quote pair `(open, close)`, escaping every
/// embedded `open`, `close`, and `escape_char` with a leading
/// `escape_char`.
///
/// The `open` and `close` characters may be the same (as with `"`) or
/// different (as with angle brackets). `escape_char` may be any
/// character, but is conventionally `\`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::quote;
///
/// // Standard double-quote style:
/// assert_eq!(quote::custom_quote("hi", '"', '"', '\\'), "\"hi\"");
/// // Bracket style with `%` as the escape:
/// assert_eq!(quote::custom_quote("[a]", '[', ']', '%'), "[%[a%]]");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn custom_quote(s: &str, open: char, close: char, escape_char: char) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push(open);
    for c in s.chars() {
        if c == open || c == close || c == escape_char {
            out.push(escape_char);
        }
        out.push(c);
    }
    out.push(close);
    out
}

// =====================================================================
// Typographic (locale-neutral curly) quoters
// =====================================================================

/// Wraps `s` in Unicode double curly quotes — `\u{201C}` (LEFT DOUBLE
/// QUOTATION MARK) and `\u{201D}` (RIGHT DOUBLE QUOTATION MARK).
///
/// No escaping is applied — the input's contents are copied verbatim
/// between the two curly quotes. Callers who need the *literal*
/// U+201C / U+201D characters escaped inside the value should
/// pre-process the input; this function is intended for display in
/// prose.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::quote;
///
/// assert_eq!(quote::curly_quote("hello"), "\u{201C}hello\u{201D}");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn curly_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 6);
    out.push('\u{201C}');
    out.push_str(s);
    out.push('\u{201D}');
    out
}

/// Wraps `s` in Unicode single curly quotes — `\u{2018}` (LEFT SINGLE
/// QUOTATION MARK) and `\u{2019}` (RIGHT SINGLE QUOTATION MARK).
///
/// See [`curly_quote`] for the escaping caveat.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::quote;
///
/// assert_eq!(quote::curly_single_quote("hi"), "\u{2018}hi\u{2019}");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn curly_single_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 6);
    out.push('\u{2018}');
    out.push_str(s);
    out.push('\u{2019}');
    out
}

// =====================================================================
// Style picker
// =====================================================================

/// Quotes `s` using whichever of `"`, `'`, or `` ` `` requires the
/// fewest embedded escapes. Ties break in favour of `"`, then `'`, then
/// `` ` ``.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::quote;
///
/// // No quotes present — use double.
/// assert_eq!(quote::quote_smart("hello"), "\"hello\"");
/// // Contains double quotes but no singles — use single.
/// assert_eq!(quote::quote_smart("say \"hi\""), "'say \"hi\"'");
/// // Two doubles, one single, no backticks — backtick wins with zero
/// // escapes needed.
/// assert_eq!(quote::quote_smart("it's \"good\""), "`it's \"good\"`");
/// // Contains one of each — a three-way tie breaks to double.
/// assert_eq!(quote::quote_smart("\"'`"), "\"\\\"'`\"");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn quote_smart(s: &str) -> String {
    let mut n_double = 0usize;
    let mut n_single = 0usize;
    let mut n_back = 0usize;
    for c in s.chars() {
        match c {
            '"' => n_double += 1,
            '\'' => n_single += 1,
            '`' => n_back += 1,
            _ => {}
        }
    }
    // Preference order: double, single, backtick. Only switch away
    // from `"` if a later choice is *strictly* less costly.
    if n_single < n_double && n_single <= n_back {
        single_quote(s)
    } else if n_back < n_double && n_back < n_single {
        backtick_quote(s)
    } else {
        double_quote(s)
    }
}

// =====================================================================
// Inverse: detect and strip a standard quote pair.
// =====================================================================

/// The set of quote-pair delimiters recognised by [`unquote`] and
/// [`is_quoted`], in the order they are tried.
const QUOTE_PAIRS: &[(char, char)] = &[
    ('"', '"'),
    ('\'', '\''),
    ('`', '`'),
    ('<', '>'),
    ('\u{201C}', '\u{201D}'), // “ ”
    ('\u{2018}', '\u{2019}'), // ‘ ’
];

/// If `s` starts and ends with a matching pair of standard quote
/// delimiters, returns the inner content as a borrowed sub-slice;
/// otherwise `None`.
///
/// Recognised delimiter pairs, in the order checked:
///
/// | Open | Close | Producer |
/// | ---- | ----- | -------- |
/// | `"`  | `"`   | [`double_quote`]        |
/// | `'`  | `'`   | [`single_quote`]        |
/// | `` ` `` | `` ` `` | [`backtick_quote`] |
/// | `<`  | `>`   | [`angle_quote`]         |
/// | `\u{201C}` | `\u{201D}` | [`curly_quote`] |
/// | `\u{2018}` | `\u{2019}` | [`curly_single_quote`] |
///
/// **`unquote` does not resolve escape sequences.** The returned slice
/// is the interior of the quote pair verbatim — any `\"`, `\'`, `` \` ``
/// inside is preserved as two characters. Callers who need the escape
/// interpretation should pair `unquote` with the matching escape
/// decoder (e.g. treat the interior as a JSON string body and pass it
/// to [`crate::escape::unescape_json`]).
///
/// This is zero-allocation.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::quote;
///
/// assert_eq!(quote::unquote("\"hi\""), Some("hi"));
/// assert_eq!(quote::unquote("'hi'"), Some("hi"));
/// assert_eq!(quote::unquote("<tag>"), Some("tag"));
/// assert_eq!(quote::unquote("\u{201C}hi\u{201D}"), Some("hi"));
/// // No wrapping quote pair:
/// assert_eq!(quote::unquote("hi"), None);
/// // Mismatched pair:
/// assert_eq!(quote::unquote("\"hi'"), None);
/// // Just one character — cannot be both open and close:
/// assert_eq!(quote::unquote("\""), None);
/// ```
#[must_use]
pub fn unquote(s: &str) -> Option<&str> {
    let first = s.chars().next()?;
    let last = s.chars().next_back()?;
    // Length in bytes to strip on each side. We must not strip more than
    // half the string, so we require at least the two delimiter bytes.
    for &(open, close) in QUOTE_PAIRS {
        if first == open && last == close {
            let open_len = open.len_utf8();
            let close_len = close.len_utf8();
            // Reject the degenerate "single character that happens to
            // equal both open and close" case (e.g. `"` alone).
            if s.len() < open_len + close_len {
                return None;
            }
            return Some(&s[open_len..s.len() - close_len]);
        }
    }
    None
}

/// Returns `true` if `s` is wrapped in a standard quote pair —
/// equivalent to `unquote(s).is_some()`.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::quote;
///
/// assert!(quote::is_quoted("\"hi\""));
/// assert!(quote::is_quoted("<tag>"));
/// assert!(!quote::is_quoted("hi"));
/// ```
#[must_use]
#[inline]
pub fn is_quoted(s: &str) -> bool {
    unquote(s).is_some()
}
