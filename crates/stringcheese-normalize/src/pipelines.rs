//! Named preset pipelines — every function documents its exact
//! stage order in its doc comment.
//!
//! The presets aim at the specific jobs the design doc calls
//! out: identifier normalisation, display-safe rendering, and
//! search-index key generation. Any other pipeline the caller
//! wants is composable from [`crate::primitives`] + the
//! [`stringcheese_unicode`] surface.

use alloc::string::String;

use stringcheese_unicode::{Normalization, PreprocessingPipeline};

use crate::primitives::{canonicalize_punctuation, collapse_whitespace, strip_controls, trim};

/// Normalise a human name-ish string for identifier comparison /
/// deduplication.
///
/// **Stages** (in order):
///
/// 1. NFKC — compatibility decomposition + canonical composition.
///    Folds compatibility variants (e.g. `ﬃ` → `ffi`, half-width
///    Latin, super/subscript digits) into their base forms.
/// 2. Case-fold — Unicode-aware, not `to_lowercase`. `İ` folds
///    to `i̇`, `ß` to `ss`, `ﬃ` was already handled by NFKC.
/// 3. Strip diacritics — combining marks removed. `café` → `cafe`.
///
/// **Not applied**: whitespace normalisation. Names with embedded
/// spaces stay as-is; use [`search_key`] when you want whitespace
/// collapsed too.
///
/// # Example
///
/// ```
/// use stringcheese_normalize::identifier;
///
/// assert_eq!(identifier("Café"), "cafe");
/// // NFKC compatibility form: ligature `ﬃ` decomposes to `ffi`,
/// // then case-folds and diacritic-strips (both no-ops here).
/// assert_eq!(identifier("Eﬃcient"), "efficient");
/// ```
#[must_use]
pub fn identifier(input: &str) -> String {
    let pipeline = PreprocessingPipeline::new()
        .normalize(Normalization::Nfkc)
        .case_fold()
        .strip_diacritics();
    pipeline.apply(input)
}

/// Normalise arbitrary user-supplied text for safe display.
///
/// **Stages** (in order):
///
/// 1. Strip control characters — every Cc code point is removed
///    (including embedded `\n`, `\r`, `\t`). Prevents terminal
///    escape sequences and layout-breaking whitespace from
///    leaking into rendered output.
/// 2. NFC — canonical composition. Guarantees the output is in
///    the composed form that most renderers expect and produces
///    stable byte-for-byte comparisons downstream.
/// 3. Collapse whitespace — every whitespace run becomes one
///    ASCII space. (No trim — leading/trailing space may be
///    semantically meaningful in a chat/comment context.)
///
/// # Example
///
/// ```
/// use stringcheese_normalize::display_safe;
///
/// assert_eq!(display_safe("hello\x07 \tworld"), "hello world");
/// ```
#[must_use]
pub fn display_safe(input: &str) -> String {
    let stripped = strip_controls(input);
    let normalised = PreprocessingPipeline::new()
        .normalize(Normalization::Nfc)
        .apply(&stripped);
    collapse_whitespace(&normalised)
}

/// Generate the sort/search key for a fuzzy-match index.
///
/// **Stages** (in order):
///
/// 1. NFKC — same as [`identifier`], folds compatibility variants.
/// 2. Case-fold — Unicode-aware case folding.
/// 3. Strip diacritics — combining marks removed.
/// 4. Canonicalise punctuation — smart quotes, dashes, ellipsis,
///    NBSP folded to ASCII equivalents (see
///    [`crate::canonicalize_punctuation`]).
/// 5. Collapse whitespace — every run → single ASCII space.
/// 6. Trim — leading/trailing space stripped.
///
/// **Semantics**: two strings that a human would consider "the
/// same query" produce the same key. Use as the equality key in a
/// suggest/autocomplete index; original text stays alongside for
/// display.
///
/// # Example
///
/// ```
/// use stringcheese_normalize::search_key;
///
/// let a = "  \u{201C}Café  \u{2014}  résumé\u{201D}  ";
/// let b = "\"cafe - resume\"";
/// assert_eq!(search_key(a), b);
/// ```
#[must_use]
pub fn search_key(input: &str) -> String {
    let base = identifier(input);
    let punct = canonicalize_punctuation(&base);
    let collapsed = collapse_whitespace(&punct);
    trim(&collapsed)
}

/// Canonicalise punctuation only. Shorthand for
/// [`crate::canonicalize_punctuation`] exposed under the pipeline
/// namespace so a caller can pick the whole pipeline family from
/// one import.
#[must_use]
pub fn punctuation_canonical(input: &str) -> String {
    canonicalize_punctuation(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_folds_case_and_strips_diacritics() {
        assert_eq!(identifier("Café"), "cafe");
        assert_eq!(identifier("STRAßE"), "strasse");
    }

    #[test]
    fn identifier_folds_nfkc_compatibility_forms() {
        // Ligature ﬃ (U+FB03) decomposes under NFKC to "ffi";
        // case-fold and diacritic-strip are no-ops for the
        // decomposed form.
        assert_eq!(identifier("Eﬃcient"), "efficient");
    }

    #[test]
    fn display_safe_strips_control_and_collapses_whitespace() {
        // BEL + tab + multiple spaces → one space.
        assert_eq!(display_safe("hello\x07 \tworld"), "hello world");
    }

    #[test]
    fn display_safe_preserves_leading_trailing_space() {
        // Callers may care about leading/trailing space
        // (comment indentation etc.) — no trim.
        assert_eq!(display_safe("  hello  "), " hello ");
    }

    #[test]
    fn search_key_folds_everything() {
        let a = "  \u{201C}Café  \u{2014}  résumé\u{201D}  ";
        let b = "\"cafe - resume\"";
        assert_eq!(search_key(a), b);
    }

    #[test]
    fn search_key_is_idempotent() {
        let s = "The quick brown fox";
        let k1 = search_key(s);
        let k2 = search_key(&k1);
        assert_eq!(k1, k2);
    }

    #[test]
    fn punctuation_canonical_is_the_alias() {
        assert_eq!(punctuation_canonical("\u{201C}hi\u{201D}"), "\"hi\"",);
    }
}
