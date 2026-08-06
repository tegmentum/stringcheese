//! The three-way linkage decision the Fellegi-Sunter classifier emits.
//!
//! Fellegi and Sunter's 1969 paper defines two thresholds `T_λ` (lower) and
//! `T_μ` (upper) and partitions the real line of record-pair weights into
//! three regions, each mapped to one variant of [`LinkageDecision`].
//! Downstream systems typically route the two extremes to fully-automated
//! merge and non-merge queues respectively, and the middle region to a
//! clerical-review queue.

/// The Fellegi-Sunter three-way decision for a candidate record pair.
///
/// The mapping between a computed weight `W` and one of these variants is
/// deterministic given the model's thresholds:
///
/// * `W >= T_μ`  →  [`LinkageDecision::Match`]
/// * `W <= T_λ`  →  [`LinkageDecision::NonMatch`]
/// * `T_λ < W < T_μ`  →  [`LinkageDecision::PossibleMatch`]
///
/// The bounds are inclusive at both ends of the outer regions and strictly
/// exclusive at both ends of the middle. This matches Fellegi and Sunter's
/// original formulation: the analyst chooses `T_μ` such that a weight equal
/// to `T_μ` is *already good enough* to declare a match, and `T_λ` such that
/// a weight equal to `T_λ` is *already low enough* to declare a non-match.
///
/// The enum is deliberately not `#[non_exhaustive]`: a fourth Fellegi-Sunter
/// decision would be a redefinition of the model, not a backwards-
/// compatible extension, and callers should be able to `match` on it
/// exhaustively without a `_ =>` arm they will never touch.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum LinkageDecision {
    /// The pair's weight is at or above the upper threshold `T_μ`; the
    /// classifier considers it a match. Downstream systems typically
    /// merge these pairs automatically.
    Match,
    /// The pair's weight is at or below the lower threshold `T_λ`; the
    /// classifier considers it a non-match. Downstream systems typically
    /// discard these pairs without further processing.
    NonMatch,
    /// The pair's weight is strictly between the two thresholds; the
    /// classifier declines to decide, and the pair is routed for clerical
    /// review. This is a first-class outcome of the Fellegi-Sunter model,
    /// not an error or an absent decision — the model explicitly reserves
    /// this region for pairs the automated classifier is not confident
    /// enough about in either direction.
    PossibleMatch,
}

impl LinkageDecision {
    /// Returns a short, stable, machine-friendly label for the variant.
    ///
    /// Suitable for logs, JSON tags, and CSV columns.
    #[inline]
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::NonMatch => "non-match",
            Self::PossibleMatch => "possible-match",
        }
    }

    /// Returns `true` iff the decision is [`LinkageDecision::Match`].
    #[inline]
    #[must_use]
    pub const fn is_match(self) -> bool {
        matches!(self, Self::Match)
    }

    /// Returns `true` iff the decision is [`LinkageDecision::NonMatch`].
    #[inline]
    #[must_use]
    pub const fn is_non_match(self) -> bool {
        matches!(self, Self::NonMatch)
    }

    /// Returns `true` iff the decision is [`LinkageDecision::PossibleMatch`].
    #[inline]
    #[must_use]
    pub const fn is_possible_match(self) -> bool {
        matches!(self, Self::PossibleMatch)
    }
}

impl core::fmt::Display for LinkageDecision {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_labels_are_stable() {
        // These strings are part of the crate's public surface; changing
        // them would break log parsers and downstream JSON schemas.
        assert_eq!(LinkageDecision::Match.as_str(), "match");
        assert_eq!(LinkageDecision::NonMatch.as_str(), "non-match");
        assert_eq!(LinkageDecision::PossibleMatch.as_str(), "possible-match");
    }

    #[test]
    fn predicate_helpers_are_mutually_exclusive() {
        for decision in [
            LinkageDecision::Match,
            LinkageDecision::NonMatch,
            LinkageDecision::PossibleMatch,
        ] {
            let flags = [
                decision.is_match(),
                decision.is_non_match(),
                decision.is_possible_match(),
            ];
            let true_count = flags.iter().filter(|f| **f).count();
            assert_eq!(
                true_count, 1,
                "expected exactly one predicate to be true for {decision:?}, got {flags:?}"
            );
        }
    }

    #[test]
    fn display_uses_as_str() {
        assert_eq!(format!("{}", LinkageDecision::Match), "match");
    }
}
