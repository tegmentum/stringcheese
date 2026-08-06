//! Scoring schemes for sequence alignment.
//!
//! An alignment algorithm is parameterized by a [`ScoringScheme`], which
//! determines the reward for matching symbols, the penalty for substitution,
//! and the penalty for opening or extending gaps.
//!
//! Two concrete schemes ship in this module:
//!
//! * [`LinearGap`] — every gap symbol costs the same amount.
//! * [`AffineGap`] — opening a gap costs `gap_open`; each additional symbol
//!   in the same gap costs `gap_extend`. Total k-symbol gap cost is
//!   `gap_open + (k - 1) * gap_extend`.
//!
//! Users may implement [`ScoringScheme`] for their own schemes (for example
//! a wrapper around a BLOSUM-style substitution matrix) as long as the four
//! required methods return the appropriate constants. All scores are `i32`;
//! see the crate-level docs for the rationale.
//!
//! # References
//!
//! * Gotoh, O. (1982). "An improved algorithm for matching biological
//!   sequences." *Journal of Molecular Biology*, 162(3), 705-708.
//!   DOI: <https://doi.org/10.1016/0022-2836(82)90398-9> — the
//!   affine-gap-cost formulation `gap_open + (k - 1) * gap_extend` that
//!   [`AffineGap`] realizes.
//! * Henikoff, S., & Henikoff, J. G. (1992). "Amino acid substitution
//!   matrices from protein blocks." *PNAS*, 89(22), 10915-10919.
//!   DOI: <https://doi.org/10.1073/pnas.89.22.10915> — BLOSUM, cited as
//!   the canonical example of a substitution matrix downstream users
//!   might plug into a custom [`ScoringScheme`] implementation.

/// Contract for an alignment scoring scheme.
///
/// The four required methods return the numeric parameters used by the
/// alignment DPs. By convention, penalties are non-positive integers and
/// rewards are non-negative, but the alignment kernels do not enforce this
/// — they merely maximize the sum of the returned values, so any
/// combination of signs will produce *some* alignment.
///
/// The trait carries a default [`pair_score`](ScoringScheme::pair_score)
/// method that combines [`match_score`](ScoringScheme::match_score) and
/// [`mismatch_score`](ScoringScheme::mismatch_score) via `T: Eq`. Overriding
/// it is the extension point for schemes that need a full substitution
/// matrix (BLOSUM, PAM, or a caller-defined table).
pub trait ScoringScheme {
    /// Reward for aligning two matching symbols.
    fn match_score(&self) -> i32;

    /// Penalty for aligning two non-matching symbols.
    fn mismatch_score(&self) -> i32;

    /// Cost charged when opening a new gap.
    ///
    /// For a [`LinearGap`] scheme this equals
    /// [`gap_extend`](ScoringScheme::gap_extend).
    fn gap_open(&self) -> i32;

    /// Cost charged for each additional symbol in an already-open gap.
    ///
    /// For a [`LinearGap`] scheme this equals
    /// [`gap_open`](ScoringScheme::gap_open).
    fn gap_extend(&self) -> i32;

    /// Score a pair of aligned symbols under this scheme.
    ///
    /// The default implementation returns
    /// [`match_score`](ScoringScheme::match_score) if `a == b` and
    /// [`mismatch_score`](ScoringScheme::mismatch_score) otherwise. Custom
    /// schemes may override to consult a substitution matrix.
    ///
    /// The `where Self: Sized` bound keeps the trait object-safe if a caller
    /// implements it without overriding this generic method.
    #[inline]
    fn pair_score<T: Eq>(&self, a: &T, b: &T) -> i32
    where
        Self: Sized,
    {
        if a == b {
            self.match_score()
        } else {
            self.mismatch_score()
        }
    }
}

/// A linear (also called "constant") gap scheme.
///
/// Every gap symbol costs [`gap_penalty`](LinearGap::gap_penalty); a
/// `k`-symbol gap costs `k * gap_penalty`. The [`ScoringScheme`] impl
/// reports `gap_open == gap_extend == gap_penalty`, which the alignment
/// kernels detect and use to select the simpler single-matrix DP path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinearGap {
    /// Reward for a match (typically positive).
    pub match_score: i32,
    /// Penalty for a substitution (typically non-positive).
    pub mismatch_score: i32,
    /// Per-symbol gap penalty (typically non-positive).
    pub gap_penalty: i32,
}

impl LinearGap {
    /// Simple textbook scoring: `match = 1, mismatch = -1, gap = -1`.
    #[must_use]
    pub const fn simple() -> Self {
        Self {
            match_score: 1,
            mismatch_score: -1,
            gap_penalty: -1,
        }
    }

    /// BLAST-inspired nucleotide scoring: `match = 1, mismatch = -3, gap = -2`.
    ///
    /// These values approximate BLASTN's default `-reward 1 -penalty -3
    /// -gapopen 5 -gapextend 2` when collapsed to a linear gap.
    #[must_use]
    pub const fn blast() -> Self {
        Self {
            match_score: 1,
            mismatch_score: -3,
            gap_penalty: -2,
        }
    }
}

impl ScoringScheme for LinearGap {
    #[inline]
    fn match_score(&self) -> i32 {
        self.match_score
    }
    #[inline]
    fn mismatch_score(&self) -> i32 {
        self.mismatch_score
    }
    #[inline]
    fn gap_open(&self) -> i32 {
        self.gap_penalty
    }
    #[inline]
    fn gap_extend(&self) -> i32 {
        self.gap_penalty
    }
}

/// An affine gap scheme (Gotoh 1982).
///
/// Opening a gap costs [`gap_open`](AffineGap::gap_open); each additional
/// symbol of the same gap costs [`gap_extend`](AffineGap::gap_extend). Total
/// `k`-symbol gap cost is `gap_open + (k - 1) * gap_extend`.
///
/// Choosing `gap_open == gap_extend` degenerates to a linear scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AffineGap {
    /// Reward for a match (typically positive).
    pub match_score: i32,
    /// Penalty for a substitution (typically non-positive).
    pub mismatch_score: i32,
    /// Penalty charged when opening a new gap (typically non-positive; often
    /// more negative than [`gap_extend`](AffineGap::gap_extend) to discourage
    /// spurious short gaps).
    pub gap_open: i32,
    /// Per-symbol penalty for extending an already-open gap (typically
    /// non-positive).
    pub gap_extend: i32,
}

impl AffineGap {
    /// Default affine scoring: `match = 1, mismatch = -1, gap_open = -2,
    /// gap_extend = -1`.
    #[must_use]
    pub const fn default_affine() -> Self {
        Self {
            match_score: 1,
            mismatch_score: -1,
            gap_open: -2,
            gap_extend: -1,
        }
    }
}

impl ScoringScheme for AffineGap {
    #[inline]
    fn match_score(&self) -> i32 {
        self.match_score
    }
    #[inline]
    fn mismatch_score(&self) -> i32 {
        self.mismatch_score
    }
    #[inline]
    fn gap_open(&self) -> i32 {
        self.gap_open
    }
    #[inline]
    fn gap_extend(&self) -> i32 {
        self.gap_extend
    }
}

/// Return `true` when the scheme uses distinct open and extend costs.
///
/// The alignment kernels use this to dispatch between the linear-gap and
/// affine-gap DPs; callers can query the same predicate to introspect
/// which DP shape a given scheme will select.
#[must_use]
#[inline]
pub fn is_affine<S: ScoringScheme>(scheme: &S) -> bool {
    scheme.gap_open() != scheme.gap_extend()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_gap_open_equals_extend() {
        let s = LinearGap::simple();
        assert_eq!(s.gap_open(), s.gap_extend());
        assert!(!is_affine(&s));
    }

    #[test]
    fn affine_gap_open_may_differ_from_extend() {
        let s = AffineGap::default_affine();
        assert_ne!(s.gap_open(), s.gap_extend());
        assert!(is_affine(&s));
    }

    #[test]
    fn affine_with_equal_open_and_extend_is_treated_as_linear() {
        let s = AffineGap {
            match_score: 1,
            mismatch_score: -1,
            gap_open: -1,
            gap_extend: -1,
        };
        assert!(!is_affine(&s));
    }

    #[test]
    fn pair_score_uses_match_for_equal_and_mismatch_otherwise() {
        let s = LinearGap::simple();
        assert_eq!(s.pair_score(&b'A', &b'A'), 1);
        assert_eq!(s.pair_score(&b'A', &b'C'), -1);
    }

    #[test]
    fn blast_scheme_constants() {
        let s = LinearGap::blast();
        assert_eq!(s.match_score, 1);
        assert_eq!(s.mismatch_score, -3);
        assert_eq!(s.gap_penalty, -2);
    }
}
