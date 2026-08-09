//! HTML entity escape / unescape.
//!
//! Two contexts, two functions — HTML has different escape rules
//! for tag text vs attribute values, and silently picking one is
//! how XSS bugs happen:
//!
//! - [`escape_text`] — safe for the `<p>…</p>` interior. Escapes
//!   `& < > " ' /`. Wraps [`html_escape::encode_safe`].
//! - [`escape_attribute`] — safe for `<a href="…">`. Escapes the
//!   same set plus surrounding-quote hazards. Wraps
//!   [`html_escape::encode_double_quoted_attribute`].
//! - [`unescape`] — decode named + numeric entities. Best-effort:
//!   unknown entities pass through unchanged (matches browser
//!   behaviour).

use alloc::string::{String, ToString};

/// Escape for HTML text content (`<p>…</p>`).
///
/// Handles `& < > " ' /`. Sufficient for text bodies; use
/// [`escape_attribute`] when the output goes inside an attribute
/// value.
#[must_use]
pub fn escape_text(input: &str) -> String {
    html_escape::encode_safe(input).into_owned()
}

/// Escape for a double-quoted HTML attribute value
/// (`<a href="…">`).
///
/// Strictly a superset of [`escape_text`] — safe to use everywhere
/// but wider than needed for text content.
#[must_use]
pub fn escape_attribute(input: &str) -> String {
    html_escape::encode_double_quoted_attribute(input).into_owned()
}

/// Decode HTML entities — named (`&amp;`, `&nbsp;`) and numeric
/// (`&#65;`, `&#x41;`). Unknown entities pass through unchanged
/// (matches browsers' error-recovery behaviour).
#[must_use]
pub fn unescape(input: &str) -> String {
    html_escape::decode_html_entities(input).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_escapes_the_five_basics() {
        assert_eq!(
            escape_text("<script>alert('x')</script>"),
            "&lt;script&gt;alert(&#x27;x&#x27;)&lt;&#x2F;script&gt;"
        );
    }

    #[test]
    fn attribute_escapes_double_quotes() {
        let out = escape_attribute(r#"say "hi""#);
        // Verifies that a double-quote is escaped rather than left
        // as-is (which would break out of an href="…" context).
        assert!(!out.contains('"'));
    }

    #[test]
    fn unescape_reverses_named_entities() {
        assert_eq!(unescape("Tom &amp; Jerry"), "Tom & Jerry");
    }

    #[test]
    fn unescape_reverses_numeric_entities() {
        assert_eq!(unescape("&#65;&#x42;"), "AB");
    }

    #[test]
    fn round_trip_text_escape() {
        let cases = ["hello", "<>&", "'quote'", "no-metachars"];
        for s in cases {
            let round = unescape(&escape_text(s));
            assert_eq!(round, s);
        }
    }
}
