//! Unicode case folding — full and simple.
//!
//! # Why "case folding" is not the same as `to_lowercase()`
//!
//! Rust's [`str::to_lowercase`] and [`str::to_uppercase`] perform
//! **case mapping**: they change a string's presentation to lower- or
//! upper-case for display. Case mapping is locale-sensitive and its
//! output is intended to be read.
//!
//! **Case folding** is a different operation, specified by
//! [`CaseFolding.txt`] in the Unicode Character Database. It exists for
//! *case-insensitive comparison*: the folded form is a canonical
//! per-string representative such that two strings are considered equal
//! ignoring case iff their folded forms are byte-identical. Case folding
//! is not intended to be displayed and is not locale-aware by default.
//!
//! Concretely the operations diverge on characters that behave differently
//! in comparison and display:
//!
//! - **German sharp S.** `"ß".to_lowercase() == "ß"` — sharp S is already
//!   lowercase. Under full case folding, `"ß"` folds to `"ss"`, so
//!   `"MASSE"` and `"Maße"` compare equal. This is exactly what a
//!   surname-matching pipeline wants and exactly what a display-lowering
//!   operation must not do.
//! - **Turkish dotless I.** The Latin uppercase `I` (U+0049) lowercases
//!   to `i` (U+0069) — but in Turkish orthography the uppercase form of
//!   dotless `ı` is a dotless capital I. Full case folding produces the
//!   default (Latin) mapping; [`case_fold_turkic`] uses the Turkic
//!   mapping table instead.
//! - **Greek final sigma.** `"ς"` (final sigma, U+03C2) case-folds to
//!   `"σ"` (medial sigma, U+03C3) — case folding erases the positional
//!   distinction that display lowercasing preserves.
//! - **Multi-character expansions.** Simple folding is a one-character to
//!   one-character mapping, so it cannot express `ß → ss` or the
//!   ligatures like `ﬁ → fi` at all. Full folding produces a string whose
//!   length may differ from the input.
//!
//! # Simple vs. full folding
//!
//! - [`simple_case_fold`] performs the *simple* mapping: each input
//!   character maps to at most one output character. This is
//!   allocation-cheaper and preserves character count, at the cost of
//!   missing multi-character expansions.
//! - [`case_fold`] performs the *full* mapping (the default) and is
//!   what a comparison pipeline almost always wants. When the design
//!   documents say "case-fold" without qualification, this is the
//!   operation meant.
//!
//! [`str::to_lowercase`]: str::to_lowercase
//! [`CaseFolding.txt`]: https://www.unicode.org/Public/UCD/latest/ucd/CaseFolding.txt
//!
//! # References
//!
//! * Unicode Standard Annex #21. *Case Mappings*. URL:
//!   <https://www.unicode.org/reports/tr21/> — the specification governing
//!   case mapping and case folding.
//! * Unicode Consortium. *`CaseFolding.txt`* — the authoritative full-folding
//!   table this module's full-folding implementation is bound to. URL:
//!   <https://www.unicode.org/Public/UCD/latest/ucd/CaseFolding.txt>
//! * Unicode Consortium (2022). *The Unicode Standard, Version 15.0.0*.
//!   Mountain View, CA: The Unicode Consortium. ISBN 978-1-936213-32-0.

use alloc::string::String;
use icu_casemap::CaseMapper;

/// The single [`CaseMapper`] used by all folding functions.
///
/// Constructed via `CaseMapper::new()` (a `const fn` that references the
/// compiled-in Unicode data), so this is zero-cost to obtain per call —
/// the value itself is `Copy`-like and holds only a pointer into
/// baked-in static tables.
#[inline]
fn mapper() -> CaseMapper {
    CaseMapper::new()
}

/// Full Unicode case folding of `input` per Unicode's `CaseFolding.txt`.
///
/// Produces a string suitable for case-insensitive comparison. This is
/// the default folding — multi-character expansions (`ß → ss`, ligatures
/// like `ﬁ → fi` when combined with normalization, and Greek final
/// sigma) are all resolved.
///
/// This is *not* the same as `input.to_lowercase()`. See the [module
/// documentation](self) for the differences.
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::case_fold;
/// // Basic lowercasing.
/// assert_eq!(case_fold("Hello"), "hello");
/// // German sharp S expands to "ss" — a full-folding-only behavior.
/// assert_eq!(case_fold("STRAßE"), "strasse");
/// // Greek final sigma folds to medial sigma.
/// assert_eq!(case_fold("ΟΔΥΣΣΕΎΣ").ends_with("σ"), true);
/// ```
#[must_use]
pub fn case_fold(input: &str) -> String {
    mapper().fold_string(input)
}

/// Simple Unicode case folding — each input character maps to at most
/// one output character.
///
/// Preserves character count but cannot express multi-character
/// expansions. For most comparison work [`case_fold`] is what you want;
/// `simple_case_fold` is here for callers that need a length-preserving
/// mapping (grapheme-level algorithms whose kernels assume `len(fold(x))
/// == len(x)`, for example).
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::simple_case_fold;
/// assert_eq!(simple_case_fold("Hello"), "hello");
/// // Simple folding does *not* expand sharp S — it stays as-is because
/// // the simple mapping is empty for it.
/// assert_eq!(simple_case_fold("STRAßE"), "straße");
/// ```
#[must_use]
pub fn simple_case_fold(input: &str) -> String {
    let mapper = mapper();
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        out.push(mapper.simple_fold(c));
    }
    out
}

/// Full case folding with Turkic dotted/dotless-I mappings.
///
/// In Turkic orthography the Latin uppercase `I` (U+0049) is *not* the
/// upper form of `i` (U+0069): Turkish has a dotless `ı` (U+0131) whose
/// uppercase is `I`, and a dotted `İ` (U+0130) whose lowercase is `i`.
/// Default folding uses the Latin mappings; this function uses the
/// Turkic table.
///
/// # Examples
///
/// ```
/// # use stringcheese_unicode::{case_fold, case_fold_turkic};
/// // Default folding: "İ" → "i̇" (i + combining dot above).
/// // Turkic folding: "İ" → "i" (dotted capital I → i).
/// assert_eq!(case_fold_turkic("İstanbul"), "istanbul");
/// // And "I" → "ı" under Turkic folding, while default folds to "i".
/// assert_eq!(case_fold_turkic("I"), "ı");
/// assert_eq!(case_fold("I"), "i");
/// ```
#[must_use]
pub fn case_fold_turkic(input: &str) -> String {
    mapper().fold_turkic_string(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_lowercased() {
        assert_eq!(case_fold("Hello, World!"), "hello, world!");
    }

    #[test]
    fn sharp_s_expands_under_full_fold() {
        assert_eq!(case_fold("STRAßE"), "strasse");
        assert_eq!(case_fold("Maße"), "masse");
    }

    #[test]
    fn sharp_s_stays_under_simple_fold() {
        // Simple folding cannot express the ß → ss expansion.
        assert_eq!(simple_case_fold("STRAßE"), "straße");
    }

    #[test]
    fn masse_and_masse_agree_after_full_folding() {
        // The design's motivating example: full folding makes "MASSE"
        // and "Maße" compare equal.
        assert_eq!(case_fold("MASSE"), case_fold("Maße"));
    }

    #[test]
    fn turkic_folds_i_to_dotless() {
        assert_eq!(case_fold_turkic("I"), "ı");
    }

    #[test]
    fn turkic_folds_dotted_capital_i_to_i() {
        assert_eq!(case_fold_turkic("İstanbul"), "istanbul");
    }

    #[test]
    fn default_fold_of_dotted_capital_i_is_i_plus_combining_dot() {
        // "İ" (U+0130) full-folds to "i" (U+0069) + combining dot above
        // (U+0307) in the default (non-Turkic) mapping.
        assert_eq!(case_fold("İ"), "i\u{0307}");
    }

    #[test]
    fn empty_input_is_empty_output() {
        assert_eq!(case_fold(""), "");
        assert_eq!(simple_case_fold(""), "");
        assert_eq!(case_fold_turkic(""), "");
    }

    #[test]
    fn folding_is_idempotent() {
        for s in ["Hello", "STRAßE", "ΟΔΥΣΣΕΎΣ", "İstanbul"] {
            let once = case_fold(s);
            let twice = case_fold(&once);
            assert_eq!(once, twice);
        }
    }
}
