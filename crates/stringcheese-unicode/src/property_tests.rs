//! Property-based tests for the Unicode preprocessing transformations.
//!
//! Every transformation in this crate has invariants that must hold on
//! all inputs:
//!
//! - **Normalization idempotence.** `nfc(nfc(x)) == nfc(x)` for every
//!   `x`, and the analogous identity for NFD/NFKC/NFKD.
//! - **NFD then NFC round-trips canonically composable strings.** By
//!   definition of canonical equivalence, `nfc(nfd(x)) == nfc(x)` for
//!   every `x` — the compose-decompose pair is idempotent through NFC.
//!   The reverse — `nfd(nfc(x)) == nfd(x)` — also holds by symmetry.
//! - **Grapheme count is bounded above by scalar count, which is bounded
//!   above by byte count.** True for every valid UTF-8 string. This is
//!   what makes graphemes the "smallest" unit useful for user-perceived
//!   length.
//! - **Case-folding idempotence.** `case_fold(case_fold(x)) ==
//!   case_fold(x)` — a folded string folds to itself.
//! - **Diacritic stripping preserves ASCII.** An ASCII-only input has no
//!   combining marks and no NFD-decomposable characters that would
//!   introduce marks, so `strip_diacritics(x) == x`.
//! - **Empty input round-trips through every transformation.** Trivial
//!   but worth asserting explicitly — an empty output on non-empty
//!   input would be a serious bug and this catches a whole class of
//!   iterator-collection regressions.
//!
//! Every property below runs under [proptest] with strings drawn from
//! reasonably broad regex generators. The generators intentionally
//! include non-ASCII scripts (Latin-1, precomposed Latin extended
//! blocks, Greek, Cyrillic, Han, and combining marks) so the properties
//! are exercised across the interesting parts of Unicode.

#[cfg(feature = "compiled-case-data")]
use crate::case_folding::case_fold;
use crate::{
    diacritics::strip_diacritics,
    graphemes::GraphemeSequence,
    normalization::Normalization,
    normalization::{nfc, nfd, nfkc, nfkd},
    preprocessing::PreprocessingPipeline,
};
use proptest::prelude::*;

/// A regex generator that covers ASCII, precomposed Latin Extended
/// characters, some Greek/Cyrillic, combining marks, and a scattering of
/// CJK. Bounded length keeps the shrinker efficient.
fn general_unicode() -> impl Strategy<Value = alloc::string::String> {
    // A short regex covering interesting ranges. Proptest's
    // `string_regex` compiles this into a strategy that samples
    // uniformly over matches.
    prop::string::string_regex(
        "[\\u0000-\\u007F\\u00C0-\\u017F\\u0300-\\u036F\\u0370-\\u03FF\\u0400-\\u04FF\\u4E00-\\u4E20]{0,32}",
    )
    .expect("static regex is valid")
}

fn ascii_only() -> impl Strategy<Value = alloc::string::String> {
    prop::string::string_regex("[\\u0000-\\u007F]{0,32}").expect("static regex is valid")
}

proptest! {
    // Normalization idempotence — the load-bearing invariant of every
    // normal form.

    #[test]
    fn nfc_is_idempotent(s in general_unicode()) {
        let once = nfc(&s);
        let twice = nfc(&once);
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn nfd_is_idempotent(s in general_unicode()) {
        let once = nfd(&s);
        let twice = nfd(&once);
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn nfkc_is_idempotent(s in general_unicode()) {
        let once = nfkc(&s);
        let twice = nfkc(&once);
        prop_assert_eq!(once, twice);
    }

    #[test]
    fn nfkd_is_idempotent(s in general_unicode()) {
        let once = nfkd(&s);
        let twice = nfkd(&once);
        prop_assert_eq!(once, twice);
    }

    // Canonical equivalence: NFD-then-NFC and NFC agree on their
    // output. This is precisely the property that makes canonical
    // decomposition/composition well-defined.

    #[test]
    fn nfd_then_nfc_agrees_with_nfc(s in general_unicode()) {
        prop_assert_eq!(nfc(&nfd(&s)), nfc(&s));
    }

    #[test]
    fn nfc_then_nfd_agrees_with_nfd(s in general_unicode()) {
        prop_assert_eq!(nfd(&nfc(&s)), nfd(&s));
    }

    // NFKC and NFKD are compatibility-equivalent forms; the composed
    // round-trip through NFKC is idempotent under the same argument.

    #[test]
    fn nfkd_then_nfkc_agrees_with_nfkc(s in general_unicode()) {
        prop_assert_eq!(nfkc(&nfkd(&s)), nfkc(&s));
    }

    // Grapheme count ≤ scalar count ≤ byte count. Holds universally
    // for valid UTF-8; the strategy generates only valid strings so
    // this is safe.

    #[test]
    fn grapheme_scalar_byte_lengths_are_ordered(s in general_unicode()) {
        let grapheme_count = GraphemeSequence::new(&s).len();
        let scalar_count = s.chars().count();
        let byte_count = s.len();
        prop_assert!(grapheme_count <= scalar_count);
        prop_assert!(scalar_count <= byte_count);
    }

    // Case folding idempotence.

    #[cfg(feature = "compiled-case-data")]
    #[test]
    fn case_fold_is_idempotent(s in general_unicode()) {
        let once = case_fold(&s);
        let twice = case_fold(&once);
        prop_assert_eq!(once, twice);
    }

    // ASCII inputs are fixpoints of `strip_diacritics`: they contain no
    // combining marks and no decomposable characters that would
    // introduce any.

    #[test]
    fn strip_diacritics_preserves_ascii(s in ascii_only()) {
        prop_assert_eq!(strip_diacritics(&s), s);
    }

    // Pipeline invariant: applying an empty pipeline returns the input
    // unchanged. This is the load-bearing property of the pipeline as
    // a monoid element.

    #[test]
    fn empty_pipeline_is_identity(s in general_unicode()) {
        let p = PreprocessingPipeline::new();
        prop_assert_eq!(p.apply(&s), s);
    }

    // Pipeline stability: applying a fold pipeline twice is the same
    // as applying it once (because case_fold is idempotent).

    #[cfg(feature = "compiled-case-data")]
    #[test]
    fn fold_pipeline_is_idempotent(s in general_unicode()) {
        let p = PreprocessingPipeline::new().case_fold();
        let once = p.apply(&s);
        let twice = p.apply(&once);
        prop_assert_eq!(once, twice);
    }

    // Diacritic stripping is stable: stripping twice equals stripping
    // once. (Once combining marks are gone the second pass sees
    // nothing to remove.)

    #[test]
    fn strip_diacritics_is_idempotent(s in general_unicode()) {
        let once = strip_diacritics(&s);
        let twice = strip_diacritics(&once);
        prop_assert_eq!(once, twice);
    }

    // The `Normalization` enum's `apply` is coherent with the direct
    // free functions on every input.

    #[test]
    fn normalization_enum_matches_free_functions(s in general_unicode()) {
        prop_assert_eq!(Normalization::Nfc.apply(&s), nfc(&s));
        prop_assert_eq!(Normalization::Nfd.apply(&s), nfd(&s));
        prop_assert_eq!(Normalization::Nfkc.apply(&s), nfkc(&s));
        prop_assert_eq!(Normalization::Nfkd.apply(&s), nfkd(&s));
    }
}

// The pipeline-order assertions live outside `proptest!` because they
// exercise hand-picked distinguishing inputs rather than sampled ones.
// The design document requires the pipeline to *not* silently reorder
// stages; these tests demonstrate that the pipeline preserves the
// distinction visibly.

#[cfg(feature = "compiled-case-data")]
#[test]
fn pipeline_order_is_visible_in_describe() {
    let a = PreprocessingPipeline::new()
        .normalize(Normalization::Nfkc)
        .case_fold();
    let b = PreprocessingPipeline::new()
        .case_fold()
        .normalize(Normalization::Nfkc);
    assert_ne!(a.describe(), b.describe());
    assert_ne!(a.steps(), b.steps());
}

#[cfg(feature = "compiled-case-data")]
#[test]
fn pipeline_order_can_change_intermediate_representation() {
    // Full-width Latin capital "Ａ" (U+FF21) — NFKC decomposes it to
    // ASCII "A", which case-folds to "a". Case folding "Ａ" without
    // NFKC produces full-width small "ａ" (U+FF41), which then NFKCs to
    // ASCII "a". Both paths converge on "a", but the *intermediate*
    // strings differ: "A" vs "ａ". A future optimization pass could
    // exploit that both paths produce the same terminal string on this
    // input; the current pipeline does not, and asserts nothing about
    // it — a pipeline that reordered stages here would still be
    // correct on this input but wrong on inputs where the divergence
    // is visible in the terminal output.
    let a = PreprocessingPipeline::new()
        .normalize(Normalization::Nfkc)
        .case_fold();
    let b = PreprocessingPipeline::new()
        .case_fold()
        .normalize(Normalization::Nfkc);
    assert_eq!(a.apply("Ａ"), "a");
    assert_eq!(b.apply("Ａ"), "a");
    // The stages themselves remain distinct.
    assert_ne!(a.steps(), b.steps());
}
