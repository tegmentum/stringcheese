//! URL / filesystem slug generation.
//!
//! Turns arbitrary Unicode text into a lowercase ASCII slug fit for
//! URLs, filenames, and identifiers. Uses [`deunicode`] for
//! Unicode-to-ASCII transliteration (`"café résumé"` →
//! `"cafe-resume"`, `"日本語"` → `"ri-ben-yu"`) rather than
//! stripping non-ASCII entirely — a slug of a Japanese title stays
//! a URL, not an empty string.
//!
//! ## Pipeline
//!
//! 1. **Transliterate** every non-ASCII scalar via [`deunicode`].
//! 2. **Lowercase** the ASCII result.
//! 3. **Filter** to `[a-z0-9]` — everything else becomes a
//!    separator boundary.
//! 4. **Collapse** consecutive separators to one; trim leading /
//!    trailing separators.
//! 5. **Cap** the length if the caller set a maximum.
//!
//! The default separator is `-`; [`Slugger`] configures it.
//!
//! ## Example
//!
//! ```
//! use stringcheese_ident::slugify;
//!
//! assert_eq!(slugify("Hello, World!"), "hello-world");
//! assert_eq!(slugify("café résumé"), "cafe-resume");
//! assert_eq!(slugify("  multiple   spaces  "), "multiple-spaces");
//! ```

use alloc::string::String;

/// Slug generator with configurable separator and max length.
///
/// Cheap to construct. For one-off slugs, [`slugify`] is the
/// zero-config wrapper (uses `-` as the separator and no length
/// cap).
#[derive(Copy, Clone, Debug)]
pub struct Slugger {
    separator: char,
    max_len: Option<usize>,
}

impl Default for Slugger {
    fn default() -> Self {
        Self {
            separator: '-',
            max_len: None,
        }
    }
}

impl Slugger {
    /// Set the separator character. Must be ASCII and not
    /// alphanumeric (otherwise it can't stay a separator after the
    /// filtering step); a panic guards the invariant at
    /// construction time.
    ///
    /// # Panics
    ///
    /// Panics when `sep` is not ASCII or is alphanumeric.
    #[must_use]
    pub fn with_separator(mut self, sep: char) -> Self {
        assert!(
            sep.is_ascii() && !sep.is_ascii_alphanumeric(),
            "separator must be ASCII and non-alphanumeric; got {sep:?}",
        );
        self.separator = sep;
        self
    }

    /// Cap the output length. Truncation happens after collapsing
    /// separators, and the result is re-trimmed of trailing
    /// separators so a mid-word cut doesn't leave a dangling `-`.
    #[must_use]
    pub fn with_max_len(mut self, max: usize) -> Self {
        self.max_len = Some(max);
        self
    }

    /// Produce the slug. See the module docs for the pipeline.
    #[must_use]
    pub fn slugify(&self, input: &str) -> String {
        // Transliterate every scalar; join per-scalar chunks with
        // ' ' so separator collapsing catches word boundaries the
        // upstream table encoded as spaces.
        let translit = deunicode::deunicode(input);
        let mut out = String::with_capacity(translit.len());
        let mut prev_sep = true; // suppress a leading separator
        for c in translit.chars() {
            let lc = c.to_ascii_lowercase();
            if lc.is_ascii_alphanumeric() {
                out.push(lc);
                prev_sep = false;
            } else if !prev_sep {
                out.push(self.separator);
                prev_sep = true;
            }
        }
        // Trim a trailing separator picked up from filtered input.
        while out.ends_with(self.separator) {
            out.pop();
        }
        if let Some(max) = self.max_len {
            if out.len() > max {
                out.truncate(max);
                // Re-trim in case truncation lands on a separator.
                while out.ends_with(self.separator) {
                    out.pop();
                }
            }
        }
        out
    }
}

/// Zero-config slug — `-` separator, no length cap.
#[must_use]
pub fn slugify(input: &str) -> String {
    Slugger::default().slugify(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_words_lowercased_and_joined() {
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn punctuation_becomes_separator_boundary() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
    }

    #[test]
    fn multiple_spaces_collapse_to_one_separator() {
        assert_eq!(slugify("  multiple   spaces  "), "multiple-spaces");
    }

    #[test]
    fn accents_are_transliterated() {
        assert_eq!(slugify("café résumé"), "cafe-resume");
    }

    #[test]
    fn cjk_is_transliterated_not_dropped() {
        // deunicode romanizes Han scalars (approximate pinyin).
        let s = slugify("日本語");
        assert!(!s.is_empty(), "expected non-empty slug, got {s:?}");
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }

    #[test]
    fn custom_separator() {
        let s = Slugger::default()
            .with_separator('_')
            .slugify("hello world");
        assert_eq!(s, "hello_world");
    }

    #[test]
    fn max_len_truncates_and_trims_trailing_separator() {
        let s = Slugger::default()
            .with_max_len(8)
            .slugify("hello world foo bar");
        assert!(s.len() <= 8);
        assert!(!s.ends_with('-'));
    }

    #[test]
    fn empty_input_returns_empty_slug() {
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("   "), "");
        assert_eq!(slugify("!!!"), "");
    }

    #[test]
    #[should_panic(expected = "separator must be ASCII")]
    fn non_ascii_separator_panics() {
        let _ = Slugger::default().with_separator('日');
    }

    #[test]
    #[should_panic(expected = "non-alphanumeric")]
    fn alphanumeric_separator_panics() {
        let _ = Slugger::default().with_separator('a');
    }
}
