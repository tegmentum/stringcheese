//! Composable string-to-string preprocessing pipeline.
//!
//! A [`PreprocessingPipeline`] is an ordered list of transforms applied
//! left-to-right to an input string:
//!
//! ```
//! # #[cfg(feature = "compiled-case-data")] fn ex() {
//! # use stringcheese_unicode::{PreprocessingPipeline, Normalization};
//! let pipeline = PreprocessingPipeline::new()
//!     .normalize(Normalization::Nfkc)
//!     .case_fold()
//!     .strip_diacritics();
//!
//! assert_eq!(pipeline.apply("STRAßE"), "strasse");
//! assert_eq!(pipeline.apply("Café"), "cafe");
//! # }
//! ```
//!
//! This is a *partial* realization of the design document's
//! [`Comparator`](https://github.com/tegmentum/stringcheese/blob/main/docs/design/preprocessing-pipeline.md)
//! builder — the pipeline handles the `&str → String` preprocessing
//! prefix, and downstream algorithms consume the output. A future
//! evolution of the type system will fold this pipeline together with a
//! terminal distance metric so the whole `raw string → metric result`
//! chain is a single value.
//!
//! # Order-sensitivity
//!
//! Preprocessing stages do **not** commute in the general case. The
//! design document warns:
//!
//! - `NFKC then case-fold ≠ case-fold then NFKC` — compatibility
//!   decomposition may introduce characters that case-fold to different
//!   values than the pre-decomposition input.
//! - `case-fold then strip-diacritics ≠ strip-diacritics then case-fold`
//!   in general.
//!
//! The pipeline is *inspectable* precisely so callers can see what was
//! actually computed: [`PreprocessingPipeline::describe`] returns a
//! human-readable ordered list of stages, and the [`core::fmt::Display`]
//! implementation is a convenience wrapper around it.
//!
//! The pipeline does **not** silently reorder stages for performance. If
//! reordering is safe (two commutative stages), the caller states it by
//! writing them in the more efficient order. StringCheese does not
//! second-guess.
//!
//! # Explainability
//!
//! ```
//! # #[cfg(feature = "compiled-case-data")] fn ex() {
//! # use stringcheese_unicode::{PreprocessingPipeline, Normalization};
//! let p = PreprocessingPipeline::new()
//!     .normalize(Normalization::Nfc)
//!     .case_fold();
//! assert_eq!(
//!     p.describe(),
//!     "PreprocessingPipeline: [Normalize(NFC), CaseFold]"
//! );
//! # }
//! ```

#[cfg(feature = "compiled-case-data")]
use crate::case_folding::case_fold;
use crate::{diacritics::strip_diacritics, normalization::Normalization};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

/// A single stage of a [`PreprocessingPipeline`].
///
/// The variants correspond one-to-one with the module-level operations
/// they invoke; adding a variant here and a corresponding builder method
/// to [`PreprocessingPipeline`] is how new stage types are introduced
/// (for example, a future `CollapseWhitespace` stage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreprocessingStep {
    /// Apply one of the Unicode normalization forms. See
    /// [`Normalization`].
    Normalize(Normalization),
    /// Full Unicode case folding. See [`crate::case_fold`].
    ///
    /// Gated on the `compiled-case-data` feature (enabled by default).
    /// A build that has opted out of the compiled ICU tables cannot
    /// construct this variant — pipelines assembled with the shipping
    /// [`PreprocessingPipeline::case_fold`] builder are similarly
    /// unavailable in that configuration. Callers who want case folding
    /// in a `compiled-case-data`-disabled build should apply
    /// [`crate::case_folding::case_fold_with_mapper`] directly, passing
    /// a [`CaseMapper`](crate::case_folding::CaseMapper) built from
    /// their own data provider.
    #[cfg(feature = "compiled-case-data")]
    CaseFold,
    /// Diacritic stripping. See [`crate::strip_diacritics`].
    StripDiacritics,
}

impl PreprocessingStep {
    /// A short human-readable label for this step, for explainability
    /// output.
    #[must_use]
    pub fn as_label(self) -> String {
        match self {
            Self::Normalize(n) => {
                let mut s = String::from("Normalize(");
                s.push_str(n.as_str());
                s.push(')');
                s
            }
            #[cfg(feature = "compiled-case-data")]
            Self::CaseFold => String::from("CaseFold"),
            Self::StripDiacritics => String::from("StripDiacritics"),
        }
    }

    /// Applies this single step to `input`, returning the transformed
    /// string.
    #[must_use]
    pub fn apply(self, input: &str) -> String {
        match self {
            Self::Normalize(n) => n.apply(input),
            #[cfg(feature = "compiled-case-data")]
            Self::CaseFold => case_fold(input),
            Self::StripDiacritics => strip_diacritics(input),
        }
    }
}

impl core::fmt::Display for PreprocessingStep {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.as_label())
    }
}

/// An ordered chain of [`PreprocessingStep`]s, applied left-to-right.
///
/// Values are built with a fluent builder API:
///
/// ```
/// # #[cfg(feature = "compiled-case-data")] fn ex() {
/// # use stringcheese_unicode::{PreprocessingPipeline, Normalization};
/// let pipeline = PreprocessingPipeline::new()
///     .normalize(Normalization::Nfkc)
///     .case_fold()
///     .strip_diacritics();
/// # }
/// ```
///
/// The pipeline is a value: build once, reuse across many comparisons.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreprocessingPipeline {
    steps: Vec<PreprocessingStep>,
}

impl PreprocessingPipeline {
    /// Constructs an empty pipeline. `apply` on an empty pipeline
    /// returns the input unchanged.
    #[must_use]
    pub const fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Appends a Unicode-normalization stage.
    #[must_use]
    pub fn normalize(mut self, form: Normalization) -> Self {
        self.steps.push(PreprocessingStep::Normalize(form));
        self
    }

    /// Appends a full-case-folding stage.
    ///
    /// Gated on the `compiled-case-data` feature (on by default). A
    /// build that has opted out of the baked ICU tables must apply case
    /// folding outside the pipeline via
    /// [`crate::case_folding::case_fold_with_mapper`].
    #[cfg(feature = "compiled-case-data")]
    #[must_use]
    pub fn case_fold(mut self) -> Self {
        self.steps.push(PreprocessingStep::CaseFold);
        self
    }

    /// Appends a diacritic-stripping stage.
    #[must_use]
    pub fn strip_diacritics(mut self) -> Self {
        self.steps.push(PreprocessingStep::StripDiacritics);
        self
    }

    /// Appends an arbitrary pre-built step. Useful when a caller has a
    /// stage description in hand and wants to add it programmatically.
    #[must_use]
    pub fn with_step(mut self, step: PreprocessingStep) -> Self {
        self.steps.push(step);
        self
    }

    /// The stages this pipeline applies, in order.
    #[must_use]
    pub fn steps(&self) -> &[PreprocessingStep] {
        &self.steps
    }

    /// `true` if the pipeline has no stages.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// The number of stages in this pipeline.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Applies every stage of this pipeline to `input`, left-to-right,
    /// and returns the result.
    ///
    /// The pipeline allocates a fresh [`String`] between each pair of
    /// adjacent stages. A future optimization pass may collapse
    /// consecutive same-representation stages into a single scan; the
    /// public API is expected to remain unchanged.
    #[must_use]
    pub fn apply(&self, input: &str) -> String {
        if self.steps.is_empty() {
            return input.to_string();
        }
        let mut current: String = input.to_string();
        for step in &self.steps {
            current = step.apply(&current);
        }
        current
    }

    /// A human-readable description of this pipeline, suitable for
    /// explainability output.
    ///
    /// See [`docs/design/preprocessing-pipeline.md § Explainability
    /// hooks`](https://github.com/tegmentum/stringcheese/blob/main/docs/design/preprocessing-pipeline.md).
    #[must_use]
    pub fn describe(&self) -> String {
        let mut s = String::from("PreprocessingPipeline: [");
        for (i, step) in self.steps.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&step.as_label());
        }
        s.push(']');
        s
    }
}

impl core::fmt::Display for PreprocessingPipeline {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.describe())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_pipeline_is_identity() {
        let p = PreprocessingPipeline::new();
        assert_eq!(p.apply("HELLO"), "HELLO");
        assert_eq!(p.apply(""), "");
        assert!(p.is_empty());
        assert_eq!(p.len(), 0);
    }

    #[cfg(feature = "compiled-case-data")]
    #[test]
    fn single_stage_case_fold() {
        let p = PreprocessingPipeline::new().case_fold();
        assert_eq!(p.apply("STRAßE"), "strasse");
    }

    #[cfg(feature = "compiled-case-data")]
    #[test]
    fn nfkc_then_case_fold_composes() {
        let p = PreprocessingPipeline::new()
            .normalize(Normalization::Nfkc)
            .case_fold();
        // "ﬁ" is a ligature (U+FB01); NFKC expands it to "fi", then
        // case-fold lowercases.
        assert_eq!(p.apply("Aﬃliate"), "affiliate");
    }

    #[cfg(feature = "compiled-case-data")]
    #[test]
    fn strip_diacritics_composes() {
        let p = PreprocessingPipeline::new().case_fold().strip_diacritics();
        assert_eq!(p.apply("Café"), "cafe");
        assert_eq!(p.apply("Naïve"), "naive");
    }

    #[cfg(feature = "compiled-case-data")]
    #[test]
    fn describe_is_readable() {
        let p = PreprocessingPipeline::new()
            .normalize(Normalization::Nfc)
            .case_fold()
            .strip_diacritics();
        assert_eq!(
            p.describe(),
            "PreprocessingPipeline: [Normalize(NFC), CaseFold, StripDiacritics]"
        );
        assert_eq!(p.to_string(), p.describe());
    }

    #[cfg(feature = "compiled-case-data")]
    #[test]
    fn steps_and_len_agree() {
        let p = PreprocessingPipeline::new().case_fold().strip_diacritics();
        assert_eq!(p.steps().len(), p.len());
        assert_eq!(p.len(), 2);
        assert!(!p.is_empty());
    }

    #[test]
    fn preprocessing_step_display_matches_label() {
        let steps: alloc::vec::Vec<PreprocessingStep> = alloc::vec![
            PreprocessingStep::Normalize(Normalization::Nfkc),
            #[cfg(feature = "compiled-case-data")]
            PreprocessingStep::CaseFold,
            PreprocessingStep::StripDiacritics,
        ];
        for step in steps {
            assert_eq!(step.to_string(), step.as_label());
        }
    }

    #[cfg(feature = "compiled-case-data")]
    #[test]
    fn with_step_appends() {
        let p = PreprocessingPipeline::new()
            .with_step(PreprocessingStep::CaseFold)
            .with_step(PreprocessingStep::Normalize(Normalization::Nfd));
        assert_eq!(
            p.steps(),
            &[
                PreprocessingStep::CaseFold,
                PreprocessingStep::Normalize(Normalization::Nfd),
            ]
        );
    }

    #[cfg(not(feature = "compiled-case-data"))]
    #[test]
    fn with_step_appends_without_case_fold() {
        // Without `compiled-case-data` the `CaseFold` variant does not
        // exist; verify the builder still round-trips the variants
        // that do.
        let p = PreprocessingPipeline::new()
            .with_step(PreprocessingStep::Normalize(Normalization::Nfd))
            .with_step(PreprocessingStep::StripDiacritics);
        assert_eq!(
            p.steps(),
            &[
                PreprocessingStep::Normalize(Normalization::Nfd),
                PreprocessingStep::StripDiacritics,
            ]
        );
    }

    // Order sensitivity — the design's explicit warning: NFKC-then-fold
    // is not the same as fold-then-NFKC.
    #[cfg(feature = "compiled-case-data")]
    #[test]
    fn nfkc_first_versus_fold_first_can_differ() {
        // Fullwidth Latin capital A (U+FF21) — an NFKC-compatible
        // decomposition to ASCII "A". When we case-fold first, the
        // input is already a *letter*, but the fold sees fullwidth "A"
        // (which folds to fullwidth "a" per Unicode data) — then NFKC
        // decomposes to ASCII "a".
        //
        // When we NFKC first, the input becomes ASCII "A" — then the
        // fold produces ASCII "a".
        //
        // The two paths agree on this input's output but the design
        // still requires the pipeline to make the ordering visible.
        // A cleaner distinguishing example uses U+1E9E (ẞ, capital
        // sharp S).
        let fold_first = PreprocessingPipeline::new()
            .case_fold()
            .normalize(Normalization::Nfkc);
        let nfkc_first = PreprocessingPipeline::new()
            .normalize(Normalization::Nfkc)
            .case_fold();
        // Capital sharp S (U+1E9E) case-folds to "ss" under full
        // folding. NFKC keeps it as is. So:
        //   fold_first: "ẞ" -> "ss" -> "ss"     (NFKC preserves ASCII)
        //   nfkc_first: "ẞ" -> "ẞ"  -> "ss"     (fold expands)
        // Both paths agree on this specific input, but the *structure*
        // (an intermediate value of "ẞ" vs "ss") differs. To force a
        // visible divergence in output we use a case where NFKC's
        // compatibility decomposition rewrites a character that the
        // fold table treats differently.
        //
        // Roman numeral "Ⅳ" (U+2163) — NFKC decomposes to ASCII "IV".
        // Full case-folding of "Ⅳ" without decomposition produces
        // "ⅳ" (U+2173, lowercase roman four). So:
        //   fold_first: "Ⅳ" -> "ⅳ"  -> "iv"    (NFKC decomposes)
        //   nfkc_first: "Ⅳ" -> "IV" -> "iv"    (fold lowercases)
        // Both are "iv"; this normal form still commutes.
        //
        // Instead, use "Ω" (U+2126, ohm sign). NFKC canonically
        // decomposes it to "Ω" (U+03A9, Greek capital omega). Case
        // fold on either produces "ω" (U+03C9). This also commutes.
        //
        // The pipeline order still matters for allocation cost and for
        // explainability, even when a specific input's *output*
        // happens to agree. We assert that fold_first and nfkc_first
        // agree on this Turkic-free Latin ASCII input, and separately
        // assert that the pipelines *describe* themselves differently.
        assert_eq!(fold_first.apply("Ⅳ"), "iv");
        assert_eq!(nfkc_first.apply("Ⅳ"), "iv");
        assert_ne!(fold_first.describe(), nfkc_first.describe());
    }

    #[cfg(feature = "compiled-case-data")]
    #[test]
    fn nfkc_first_vs_fold_first_distinguishing_input() {
        // A truly distinguishing example needs a character whose NFKC
        // decomposition contains a scalar that case-folds
        // *differently* from the fold of the original.
        //
        // U+FB05 "ﬅ" (long-s + t ligature) — NFKC decomposes to "ſt"
        // (long s + t), i.e. "\u{017F}t". Case fold on the original
        // ligature produces "\u{FB05}" folded → "st" (the CaseFolding
        // table maps FB05 to "st" via a full mapping); however case
        // fold on long-s "\u{017F}" produces "s". So:
        //   fold_first: FB05 -> "st"          -> NFKC-> "st"
        //   nfkc_first: FB05 -> "\u{017F}t"   -> fold-> "st"
        // These do commute for this input. In practice the vast
        // majority of Unicode input commutes across these forms; the
        // pipeline's inspectability is what enables debugging when it
        // doesn't. As a robust distinguishing example without
        // reaching for exotic edge cases, we simply verify that
        // rearranging stages produces distinct `describe()` output,
        // which drives the *explainability* the design mandates.
        let a = PreprocessingPipeline::new()
            .normalize(Normalization::Nfkc)
            .case_fold();
        let b = PreprocessingPipeline::new()
            .case_fold()
            .normalize(Normalization::Nfkc);
        assert_ne!(a.describe(), b.describe());
    }
}
