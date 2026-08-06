//! The base [`Jaro`] similarity.
//!
//! Jaro (1989) defines a similarity score in `[0.0, 1.0]` for two sequences
//! based on the number of matching symbols within a bounded matching window
//! and the number of transpositions among those matches. It is the base
//! algorithm that Jaro-Winkler builds on, and — because it happens to be the
//! first floating-point similarity in Comparand — it also serves as the
//! reference for the crate's [`FloatExpectation`]-driven golden-case pattern.
//!
//! The primary reference is Matthew A. Jaro's 1989 record-linkage paper on
//! matching the 1985 Tampa census; the full citation is listed in the
//! crate-level `References` section.
//!
//! # Formula
//!
//! Given sequences `a` (length `|a|`) and `b` (length `|b|`), the matching
//! window is
//!
//! ```text
//!     w = max(0, max(|a|, |b|) / 2 - 1)                     (integer division)
//! ```
//!
//! A symbol `a[i]` matches `b[j]` if `a[i] == b[j]` and `|i - j| <= w`, and
//! each position on each side is used in at most one match. Let `m` be the
//! number of matches. Walk the matched positions in each sequence in order
//! and count the positions at which the symbols disagree; `t` is half that
//! count (a "transposition" is a pair of swapped adjacent matches). The
//! similarity is
//!
//! ```text
//!     jaro(a, b) = 0                                        if m = 0
//!                = (m/|a| + m/|b| + (m - t)/m) / 3          otherwise
//! ```
//!
//! with the boundary conventions that two empty sequences have similarity
//! `1.0` and one-empty-one-not has similarity `0.0`.
//!
//! # Complexity
//!
//! `O(|a| * w)` in the worst case for matching, `O(min(|a|, |b|))` for the
//! transposition scan. Auxiliary space is `O(|a| + |b|)` for the boolean
//! matched-position bitmaps.
//!
//! # Workspace reuse
//!
//! [`Jaro::similarity_with_workspace`] accepts a caller-owned
//! [`JaroWorkspace`] and reuses its two `Vec<bool>` bitmaps across
//! successive calls, dropping the per-call allocation cost that the
//! trait-based [`SimilarityMetric::similarity`] entry point still pays.
//! Batch workloads that compare a fixed query against a corpus should
//! prefer the workspace-aware entry point.
//!
//! [`FloatExpectation`]: https://docs.rs/comparand-corpus
//! [`JaroWorkspace`]: crate::workspace::JaroWorkspace

use core::cmp::{max, min};

use comparand_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, MetricClass,
    MetricProperties, NormalizedSimilarity, Similarity, SimilarityMetric, VariantId,
};

use crate::workspace::JaroWorkspace;

/// The Jaro similarity.
///
/// A zero-size unit struct that implements [`SimilarityMetric`] for any
/// `&[T]` with `T: Eq`. Construct it as `Jaro` and share the value across
/// threads; the algorithm carries no per-call state.
///
/// Introduced by Matthew A. Jaro (1989) in the context of record linkage;
/// see the crate-level `References` section for the full citation.
///
/// Output is a [`Similarity<f64>`] in the closed interval `[0.0, 1.0]`. If a
/// range-checked wrapper is needed downstream, prefer
/// [`Jaro::similarity_normalized`] — it returns a [`NormalizedSimilarity`],
/// which enforces the invariant statically at every subsequent hand-off.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Jaro;

impl Jaro {
    /// The algorithm descriptor for the base Jaro variant.
    ///
    /// The variant slug `"classic-generic-eq"` distinguishes this from
    /// future refinements (weighted match, phoneme-space match, and so on).
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::Jaro,
        variant: VariantId("classic-generic-eq"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Advances in Record-Linkage Methodology as Applied to Matching the 1985 Census of Tampa, Florida",
            authors: "Matthew A. Jaro",
            year: 1989,
        },
    };

    /// Returns the algorithm descriptor for this variant.
    ///
    /// A `const` accessor is provided so descriptors can be pinned in `const`
    /// context — for example, as the `descriptor` field of a `GoldenCase`.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Computes the Jaro similarity between `left` and `right`, returning
    /// the range-checked [`NormalizedSimilarity`] wrapper.
    ///
    /// This is the preferred API for consumers that carry the result across
    /// module boundaries: the wrapper's constructor rejects any out-of-range
    /// value, so subsequent code can rely on the `[0.0, 1.0]` invariant
    /// without re-checking.
    ///
    /// The [`SimilarityMetric::similarity`] trait implementation exists for
    /// interface conformance and returns a plain [`Similarity<f64>`]; the
    /// two methods produce the same numeric value.
    ///
    /// # Panics
    ///
    /// Never; the computation is provably in `[0.0, 1.0]`, and
    /// `new_unchecked` is used to skip the runtime re-check.
    #[inline]
    #[must_use]
    pub fn similarity_normalized<T: Eq>(&self, left: &[T], right: &[T]) -> NormalizedSimilarity {
        NormalizedSimilarity::new_unchecked(jaro_similarity(left, right))
    }

    /// Workspace-aware Jaro similarity: computes the score reusing the
    /// bitmaps in `ws` instead of allocating fresh `Vec<bool>` buffers per
    /// call.
    ///
    /// The workspace is grown to `left.len() + right.len()` cells on the
    /// first call and reused thereafter. This is the entry point batch
    /// workloads should prefer.
    #[inline]
    #[must_use]
    pub fn similarity_with_workspace<T: Eq>(
        self,
        left: &[T],
        right: &[T],
        ws: &mut JaroWorkspace,
    ) -> Similarity<f64> {
        Similarity::new(jaro_similarity_with_workspace(left, right, ws))
    }

    /// Workspace-aware Jaro similarity in the range-checked
    /// [`NormalizedSimilarity`] wrapper.
    ///
    /// Same numeric result as [`similarity_with_workspace`](Self::similarity_with_workspace);
    /// the wrapper carries the `[0.0, 1.0]` invariant across module
    /// boundaries.
    #[inline]
    #[must_use]
    pub fn similarity_with_workspace_normalized<T: Eq>(
        self,
        left: &[T],
        right: &[T],
        ws: &mut JaroWorkspace,
    ) -> NormalizedSimilarity {
        NormalizedSimilarity::new_unchecked(jaro_similarity_with_workspace(left, right, ws))
    }
}

impl<T: Eq> SimilarityMetric<[T]> for Jaro {
    type Output = f64;

    #[inline]
    fn similarity(&self, left: &[T], right: &[T]) -> Similarity<Self::Output> {
        Similarity::new(jaro_similarity(left, right))
    }

    #[inline]
    fn properties(&self) -> MetricProperties {
        // Not any of the metric-preset constants: Jaro is a bounded
        // similarity, not a metric. Bounded similarities generally violate
        // the triangle inequality (their range collapses), so we spell out
        // the axioms individually rather than picking a preset that would
        // misdescribe the algorithm.
        MetricProperties {
            symmetric: true,
            identity_of_indiscernibles: true,
            triangle_inequality: false,
            non_negative: true,
            normalized: true,
        }
    }

    #[inline]
    fn class(&self) -> MetricClass {
        MetricClass::Similarity
    }
}

/// The core Jaro-similarity kernel.
///
/// Exposed at crate scope (not `pub`) so `jaro_winkler` can reuse it without
/// re-materializing the matching window. Returns a raw `f64` in `[0.0, 1.0]`;
/// callers that need the range-checked wrapper should go through
/// [`Jaro::similarity_normalized`] or wrap the value themselves.
///
/// Allocating wrapper around [`jaro_similarity_with_workspace`] that spins
/// up a throw-away workspace per call. Batch callers should reach for the
/// workspace-aware entry point directly.
#[must_use]
pub(crate) fn jaro_similarity<T: Eq>(a: &[T], b: &[T]) -> f64 {
    let mut ws = JaroWorkspace::new();
    jaro_similarity_with_workspace(a, b, &mut ws)
}

/// The core Jaro kernel with an explicit caller-owned workspace.
///
/// The workspace's two bitmaps are grown to `a.len()` and `b.len()` cells
/// respectively (via a shared `Vec<bool>` split at the top of the call) and
/// left at that capacity on return so repeated calls of the same size
/// perform no further allocation.
#[must_use]
#[allow(
    clippy::many_single_char_names,
    reason = "the names `a`, `b`, `m`, `t`, `k` are the canonical notation Jaro's paper uses for the two inputs, the match count, the transposition count, and the inner-loop cursor; renaming for clippy would obscure the direct correspondence between this kernel and the published definition"
)]
pub(crate) fn jaro_similarity_with_workspace<T: Eq>(
    a: &[T],
    b: &[T],
    ws: &mut JaroWorkspace,
) -> f64 {
    let len_a = a.len();
    let len_b = b.len();

    // Boundary conventions:
    //
    // - Two empty inputs are considered identical: similarity is 1.0. Some
    //   older references leave this case undefined; we follow the modern
    //   convention that identity of indiscernibles applies at the empty
    //   sequence too.
    // - One empty and one non-empty have no matches possible: similarity is
    //   0.0. This is unambiguous in every published formulation.
    if len_a == 0 && len_b == 0 {
        return 1.0;
    }
    if len_a == 0 || len_b == 0 {
        return 0.0;
    }

    // Matching window: `max(0, max(|a|, |b|) / 2 - 1)`. Integer arithmetic
    // with a saturating subtraction handles the small-input case (max_len
    // <= 1 collapses to a window of zero, which means "match at the same
    // index only").
    let max_len = max(len_a, len_b);
    let window = (max_len / 2).saturating_sub(1);

    // Track which positions in each sequence have already been matched.
    // Split-borrow the two bitmaps out of the workspace in one call —
    // `split_bitmaps_mut` also resets the requested cells to `false`, so
    // the previous call's match residue is gone.
    let (a_matched, b_matched) = ws.split_bitmaps_mut(len_a, len_b);

    let mut matches: usize = 0;
    for i in 0..len_a {
        // Window in `b`: `[max(0, i - window), min(len_b, i + window + 1))`.
        // Using saturating_sub sidesteps the underflow when `i < window`.
        let start = i.saturating_sub(window);
        let end = min(len_b, i + window + 1);
        for j in start..end {
            if b_matched[j] {
                continue;
            }
            if a[i] != b[j] {
                continue;
            }
            a_matched[i] = true;
            b_matched[j] = true;
            matches += 1;
            break;
        }
    }

    // With zero matches the formula's third term would be `0/0`; the
    // published definition folds that case into a returned similarity of
    // 0.0. Handling it here also lets the arithmetic below assume `m > 0`.
    if matches == 0 {
        return 0.0;
    }

    // Transpositions: walk the matched positions of both sequences in order
    // and count the pairs that disagree. Every disagreement is one half of
    // a swap, so the transposition count is half the disagreement count.
    // The disagreement count is always even for a well-formed matching,
    // which is why integer division here is exact rather than lossy.
    let mut disagreements: usize = 0;
    let mut k: usize = 0;
    for i in 0..len_a {
        if !a_matched[i] {
            continue;
        }
        while !b_matched[k] {
            k += 1;
        }
        if a[i] != b[k] {
            disagreements += 1;
        }
        k += 1;
    }
    let transpositions = disagreements / 2;

    // Cast to f64 once each; a `usize` up to 2^53 fits exactly in an f64
    // mantissa, and Jaro is never applied to inputs approaching that scale.
    #[allow(
        clippy::cast_precision_loss,
        reason = "inputs approaching 2^53 symbols exceed every practical Jaro use; the cast is exact for anything smaller"
    )]
    let m = matches as f64;
    #[allow(clippy::cast_precision_loss, reason = "see above")]
    let a_len_f = len_a as f64;
    #[allow(clippy::cast_precision_loss, reason = "see above")]
    let b_len_f = len_b as f64;
    #[allow(clippy::cast_precision_loss, reason = "see above")]
    let t = transpositions as f64;

    (m / a_len_f + m / b_len_f + (m - t) / m) / 3.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_matches_family_and_variant() {
        let d = Jaro::descriptor();
        assert_eq!(d.family, AlgorithmFamily::Jaro);
        assert_eq!(d.variant, VariantId("classic-generic-eq"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1989, .. }
        ));
    }

    #[test]
    fn descriptor_is_const() {
        const D: AlgorithmDescriptor = Jaro::DESCRIPTOR;
        assert_eq!(D.variant.0, "classic-generic-eq");
    }

    #[test]
    fn class_and_properties_declare_bounded_similarity() {
        let alg = Jaro;
        // The trait's associated methods need a concrete `S` to resolve;
        // pin it at the call site.
        assert_eq!(
            <Jaro as SimilarityMetric<[u8]>>::class(&alg),
            MetricClass::Similarity
        );
        let p = <Jaro as SimilarityMetric<[u8]>>::properties(&alg);
        assert!(p.symmetric);
        assert!(p.identity_of_indiscernibles);
        assert!(!p.triangle_inequality);
        assert!(p.non_negative);
        assert!(p.normalized);
    }

    #[test]
    fn empty_empty_is_one() {
        assert_eq!(jaro_similarity::<u8>(&[], &[]).to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn one_empty_is_zero() {
        assert_eq!(
            jaro_similarity::<u8>(&[], b"abc").to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(
            jaro_similarity::<u8>(b"abc", &[]).to_bits(),
            0.0_f64.to_bits()
        );
    }

    #[test]
    fn identical_is_one() {
        assert_eq!(
            jaro_similarity(b"kitten", b"kitten").to_bits(),
            1.0_f64.to_bits()
        );
    }

    #[test]
    fn martha_marhta_matches_published_value() {
        // Jaro (1989); expected value is 17/18 ≈ 0.9444.
        let s = jaro_similarity(b"MARTHA", b"MARHTA");
        assert!((s - 17.0_f64 / 18.0_f64).abs() < 1e-12);
    }

    #[test]
    fn similarity_normalized_matches_similarity() {
        let alg = Jaro;
        let n = alg
            .similarity_normalized(b"kitten", b"sitting")
            .into_inner();
        let s = alg.similarity(b"kitten", b"sitting").into_inner();
        assert_eq!(n.to_bits(), s.to_bits());
    }

    #[test]
    fn similarity_with_workspace_matches_similarity_bit_exact() {
        let alg = Jaro;
        let mut ws = JaroWorkspace::new();
        let with_ws = alg
            .similarity_with_workspace(b"MARTHA", b"MARHTA", &mut ws)
            .into_inner();
        let without_ws = alg.similarity(b"MARTHA", b"MARHTA").into_inner();
        assert_eq!(with_ws.to_bits(), without_ws.to_bits());
    }

    #[test]
    fn similarity_with_workspace_reuse_across_shapes_bit_exact() {
        let alg = Jaro;
        let mut hot = JaroWorkspace::new();
        let pairs: &[(&[u8], &[u8])] = &[
            (b"", b""),
            (b"", b"z"),
            (b"a", b""),
            (b"MARTHA", b"MARHTA"),
            (b"kitten", b"sitting"),
            (b"DIXON", b"DICKSONX"),
            (b"aaaaaaaa", b"aaaaaaaa"),
            (b"abcdefghij", b"jihgfedcba"),
        ];
        for (a, b) in pairs {
            let mut cold = JaroWorkspace::new();
            let per_call = alg.similarity_with_workspace(a, b, &mut cold).into_inner();
            let reused = alg.similarity_with_workspace(a, b, &mut hot).into_inner();
            assert_eq!(
                per_call.to_bits(),
                reused.to_bits(),
                "reused workspace disagreed on ({a:?}, {b:?})"
            );
        }
    }
}
