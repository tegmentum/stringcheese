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
/// # ASCII fast path
///
/// When `input.is_ascii()`, all three stages reduce to a byte-wise
/// ASCII lowercase pass:
///
/// * NFKC is the identity on ASCII — every ASCII scalar has
///   `canonical_combining_class = 0` and neither a canonical nor a
///   compatibility decomposition (verified by the `ascii_is_stable_
///   under_all_forms` test in `stringcheese-unicode::normalization`).
/// * Full Unicode case folding on the ASCII range agrees with
///   `to_ascii_lowercase` per `CaseFolding.txt` — `A..Z` map to
///   `a..z` with common / full status, every other ASCII scalar has
///   no mapping (identity).
/// * `strip_diacritics` is NFD → drop-combining-marks → NFC, which
///   is the identity on ASCII (no combining marks, NFD/NFC no-op).
///
/// So `identifier(ascii) == ascii.to_ascii_lowercase()`
/// byte-for-byte, and skipping the ICU pipeline replaces an
/// ~30 MiB/s ICU-bound path with a bulk lowercase loop.
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
    // ASCII fast path — see the doc's "ASCII fast path" section for
    // why this is byte-identical to the full pipeline. Guarded by a
    // single bulk `is_ascii()` scan (SIMD-accelerated on aarch64 /
    // x86_64), which is fractions of a nanosecond per KiB.
    if input.is_ascii() {
        return input.to_ascii_lowercase();
    }
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
/// # ASCII fast path
///
/// When `input.is_ascii()`, the pipeline collapses further than
/// [`identifier`]'s fast path: the intermediate `canonicalize_
/// punctuation` stage is also a no-op because every substitution
/// target it defines is a non-ASCII scalar (see the primitive's
/// substitution table). So on ASCII the pipeline reduces to
/// `trim(collapse_whitespace(input.to_ascii_lowercase()))`, saving
/// one full string copy on top of the NFKC skip.
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
    // ASCII fast path — NFKC / case-fold / strip-diacritics collapse
    // to `to_ascii_lowercase` (see `identifier`), and
    // canonicalize-punctuation has no ASCII substitution targets, so
    // we can skip it entirely on ASCII input. Result is byte-
    // identical to the full pipeline for ASCII inputs (differential
    // test: `search_key_ascii_fast_path_matches_full_pipeline`).
    if input.is_ascii() {
        let lowered = input.to_ascii_lowercase();
        let collapsed = collapse_whitespace(&lowered);
        return trim(&collapsed);
    }
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

    // ------------------------------------------------------------------
    // ASCII fast-path differential tests. The presets short-circuit
    // when `input.is_ascii()` (skipping the ICU NFKC pass, which is
    // ~30 MiB/s and swamps a bulk lowercase loop). The invariant to
    // enforce is byte-for-byte equality with the full pipeline on
    // every ASCII input we can plausibly see. If a future refactor
    // adds a stage whose ASCII behaviour is no longer identity /
    // ASCII-lowercase, these tests fire.
    //
    // Rebuilt-here full pipelines mirror the pre-fast-path body of
    // each preset — deliberately verbatim so the diff between "fast"
    // and "slow" stays visible at review time.
    // ------------------------------------------------------------------

    fn identifier_full_pipeline(input: &str) -> String {
        let p = PreprocessingPipeline::new()
            .normalize(Normalization::Nfkc)
            .case_fold()
            .strip_diacritics();
        p.apply(input)
    }

    fn search_key_full_pipeline(input: &str) -> String {
        let base = identifier_full_pipeline(input);
        let punct = canonicalize_punctuation(&base);
        let collapsed = collapse_whitespace(&punct);
        trim(&collapsed)
    }

    /// Every ASCII input the fast path handles must match the
    /// ICU-driven full pipeline byte-for-byte.
    #[test]
    fn identifier_ascii_fast_path_matches_full_pipeline() {
        let inputs = [
            "",
            "a",
            "A",
            "Hello, World!",
            "MixedCASE_identifier_42",
            "  leading and trailing   ",
            "\thas\ttabs\nand\nnewlines\r",
            "control\x07chars\x1Fembedded",
            "punctuation!?.,;:'\"()[]{}<>-_=+*/\\|@#$%^&`~",
            "digits 0123456789 and letters",
            // Every printable ASCII scalar concatenated once.
            &(0x20u8..=0x7E).map(|b| b as char).collect::<String>(),
            // Every ASCII scalar including controls.
            &(0x00u8..=0x7F).map(|b| b as char).collect::<String>(),
        ];
        for input in inputs {
            assert!(input.is_ascii(), "test corpus stayed ASCII: {input:?}");
            let fast = identifier(input);
            let slow = identifier_full_pipeline(input);
            assert_eq!(fast, slow, "identifier diverged on {input:?}");
        }
    }

    /// Same corpus, `search_key`. Also covers the extra fast-path
    /// step (skipping `canonicalize_punctuation` for ASCII).
    #[test]
    fn search_key_ascii_fast_path_matches_full_pipeline() {
        let inputs = [
            "",
            "a",
            "A",
            "  Hello, World!  ",
            "MixedCASE identifier 42",
            "  leading   and   internal   and   trailing   ",
            "\thas\ttabs\nand\nnewlines\r",
            "punctuation!?.,;:'\"()[]{}<>-_=+*/\\|@#$%^&`~",
            "digits 0123456789 and letters",
            &(0x20u8..=0x7E).map(|b| b as char).collect::<String>(),
        ];
        for input in inputs {
            assert!(input.is_ascii(), "test corpus stayed ASCII: {input:?}");
            let fast = search_key(input);
            let slow = search_key_full_pipeline(input);
            assert_eq!(fast, slow, "search_key diverged on {input:?}");
        }
    }

    /// Any non-ASCII input must take the slow path — the fast-path
    /// gate is `input.is_ascii()`, so this is really a guard against
    /// accidentally widening the gate.
    #[test]
    fn non_ascii_inputs_still_take_the_full_pipeline() {
        // A one-byte non-ASCII scalar is enough to fail `is_ascii()`.
        // Verifying the output matches the full pipeline is what
        // `identifier_folds_case_and_strips_diacritics` already
        // covers; here we just confirm the gate.
        let inputs = ["Café", "STRAßE", "Eﬃcient", "  \u{201C}Café\u{201D}  "];
        for input in inputs {
            assert!(!input.is_ascii(), "test corpus is non-ASCII: {input:?}");
            assert_eq!(identifier(input), identifier_full_pipeline(input));
            assert_eq!(search_key(input), search_key_full_pipeline(input));
        }
    }
}
