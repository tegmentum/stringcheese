//! Identifier sanitization — coerce arbitrary text into something
//! valid as a variable / column / filename identifier.
//!
//! Configurable via [`Sanitizer`]:
//!
//! - **Replacement character** — what to substitute for disallowed
//!   input scalars. Default `_`.
//! - **Leading-digit policy** — identifiers in most languages can't
//!   start with a digit; by default a leading digit is prefixed
//!   with the replacement char.
//! - **Max length** — cap the output at N bytes.
//! - **Allow set** — the classifier that decides which scalars pass
//!   through unchanged; defaults to `[a-zA-Z0-9_]` (Rust/C
//!   identifier rules).
//!
//! ## Unit
//!
//! Code points. Multi-byte scalars are either passed through (if
//! the allow-set accepts them) or replaced as one unit.
//!
//! ## Example
//!
//! ```
//! use stringcheese_ident::Sanitizer;
//!
//! let s = Sanitizer::default();
//! assert_eq!(s.sanitize("hello world"), "hello_world");
//! assert_eq!(s.sanitize("42foo"), "_42foo");
//! assert_eq!(s.sanitize(""), "");
//! ```

use alloc::string::String;

/// Identifier sanitizer.
///
/// Zero-config default: replacement `_`, prefix leading digit,
/// allow ASCII alphanumerics + `_`, no length cap.
#[derive(Clone, Copy)]
pub struct Sanitizer {
    replacement: char,
    fix_leading_digit: bool,
    max_len: Option<usize>,
    allow: fn(char) -> bool,
}

impl core::fmt::Debug for Sanitizer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Sanitizer")
            .field("replacement", &self.replacement)
            .field("fix_leading_digit", &self.fix_leading_digit)
            .field("max_len", &self.max_len)
            .field("allow", &"<fn>")
            .finish()
    }
}

impl Default for Sanitizer {
    fn default() -> Self {
        Self {
            replacement: '_',
            fix_leading_digit: true,
            max_len: None,
            allow: default_allow,
        }
    }
}

fn default_allow(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

impl Sanitizer {
    /// Override the replacement character. Must be accepted by the
    /// current allow-set — otherwise the output could contain the
    /// replacement char which itself would be replaced (obvious
    /// footgun; the constructor checks).
    ///
    /// # Panics
    ///
    /// Panics when `c` is not accepted by the current allow-set.
    #[must_use]
    pub fn with_replacement(mut self, c: char) -> Self {
        assert!(
            (self.allow)(c),
            "replacement {c:?} isn't accepted by this Sanitizer's allow-set",
        );
        self.replacement = c;
        self
    }

    /// When `true` (default), a leading digit is prefixed with the
    /// replacement char to keep the output valid as a
    /// most-languages identifier. Turn it off for domains that
    /// permit digit-leading names (CSV column headers, JSON keys).
    #[must_use]
    pub fn with_fix_leading_digit(mut self, fix: bool) -> Self {
        self.fix_leading_digit = fix;
        self
    }

    /// Cap the output length in bytes. When the cap lands mid-scalar
    /// the output is trimmed back to the previous character
    /// boundary; the resulting string is always valid UTF-8.
    #[must_use]
    pub fn with_max_len(mut self, max: usize) -> Self {
        self.max_len = Some(max);
        self
    }

    /// Replace the default `[a-zA-Z0-9_]` allow-set with a caller-
    /// supplied predicate. Useful when the target grammar has a
    /// different alphabet — e.g. `is_alphanumeric` for a Unicode-
    /// aware column-name rule.
    #[must_use]
    pub fn with_allow(mut self, allow: fn(char) -> bool) -> Self {
        self.allow = allow;
        self
    }

    /// Produce the sanitized identifier.
    #[must_use]
    pub fn sanitize(&self, input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for c in input.chars() {
            if (self.allow)(c) {
                out.push(c);
            } else {
                out.push(self.replacement);
            }
        }
        if self.fix_leading_digit {
            if let Some(first) = out.chars().next() {
                if first.is_ascii_digit() {
                    out.insert(0, self.replacement);
                }
            }
        }
        if let Some(max) = self.max_len {
            if out.len() > max {
                // Trim back to a character boundary <= max.
                let mut end = max;
                while end > 0 && !out.is_char_boundary(end) {
                    end -= 1;
                }
                out.truncate(end);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_whitespace_and_punctuation() {
        let s = Sanitizer::default();
        assert_eq!(s.sanitize("hello world!"), "hello_world_");
    }

    #[test]
    fn fixes_leading_digit_by_default() {
        let s = Sanitizer::default();
        assert_eq!(s.sanitize("42foo"), "_42foo");
    }

    #[test]
    fn leaves_leading_digit_when_configured() {
        let s = Sanitizer::default().with_fix_leading_digit(false);
        assert_eq!(s.sanitize("42foo"), "42foo");
    }

    #[test]
    fn passes_through_valid_identifier_unchanged() {
        let s = Sanitizer::default();
        assert_eq!(s.sanitize("valid_name_123"), "valid_name_123");
    }

    #[test]
    fn max_len_caps_output() {
        let s = Sanitizer::default().with_max_len(5);
        assert_eq!(s.sanitize("hello_world_foo"), "hello");
    }

    #[test]
    fn max_len_respects_char_boundary() {
        // Multibyte scalars pass through only if the allow-set
        // accepts them. With a Unicode-aware allow-set, truncating
        // mid-scalar must back off to a valid char boundary.
        let s = Sanitizer::default()
            .with_allow(|c| c.is_alphanumeric() || c == '_')
            .with_max_len(4);
        // "日本語" is 9 bytes; capping at 4 must back off to 3 (one
        // scalar) or 0. The output must be valid UTF-8.
        let out = s.sanitize("日本語");
        assert!(out.len() <= 4);
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
    }

    #[test]
    fn custom_allow_set() {
        // Only lowercase letters and dash.
        let s = Sanitizer::default()
            .with_allow(|c| c.is_ascii_lowercase() || c == '-')
            .with_replacement('-');
        assert_eq!(s.sanitize("Hello World"), "-ello--orld");
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(Sanitizer::default().sanitize(""), "");
    }

    #[test]
    #[should_panic(expected = "isn't accepted by this Sanitizer's allow-set")]
    fn replacement_must_pass_allow_set() {
        let _ = Sanitizer::default().with_replacement('!');
    }
}
