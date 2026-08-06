//! Edit operations and reconstructed alignments.
//!
//! The alignment kernels produce an [`Alignment<T>`], which pairs the
//! numeric alignment score with an ordered sequence of [`EditOp<T>`]s. Each
//! op records what the DP backtrace concluded happened at that column of
//! the alignment: a match, a substitution, an insert (gap in `a`), or a
//! delete (gap in `b`).
//!
//! Ops own their symbols (`T: Clone`) rather than borrow them from the
//! inputs so that an [`Alignment`] outlives the slices it was built from.
//! For workloads where allocations are the bottleneck, callers can use the
//! score-only kernel (`score_with_workspace`) instead of `align`.

use alloc::vec::Vec;

/// A single step in an alignment's edit script.
///
/// The script is read left-to-right in the same order the DP backtrace was
/// reversed into. Applying the ops in order to `a` (or the aligned prefix
/// of `a` for a local alignment) reconstructs `b` (or its aligned prefix).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EditOp<T: Clone> {
    /// The symbols at this position of the alignment are equal.
    Match {
        /// Symbol from the first sequence at this position.
        a: T,
        /// Symbol from the second sequence at this position; `a == b`.
        b: T,
    },
    /// The symbols at this position of the alignment are aligned but differ.
    Substitute {
        /// Symbol from the first sequence at this position.
        a: T,
        /// Symbol from the second sequence at this position; `a != b`.
        b: T,
    },
    /// A gap in the first sequence — a symbol from `b` is inserted here.
    Insert {
        /// The inserted symbol from the second sequence.
        b: T,
    },
    /// A gap in the second sequence — a symbol from `a` is deleted here.
    Delete {
        /// The deleted symbol from the first sequence.
        a: T,
    },
}

impl<T: Clone> EditOp<T> {
    /// Return `true` if this op consumes a symbol from `a` (Match,
    /// Substitute, or Delete).
    #[must_use]
    pub const fn consumes_a(&self) -> bool {
        matches!(
            self,
            Self::Match { .. } | Self::Substitute { .. } | Self::Delete { .. }
        )
    }

    /// Return `true` if this op consumes a symbol from `b` (Match,
    /// Substitute, or Insert).
    #[must_use]
    pub const fn consumes_b(&self) -> bool {
        matches!(
            self,
            Self::Match { .. } | Self::Substitute { .. } | Self::Insert { .. }
        )
    }
}

/// A reconstructed pairwise alignment.
///
/// Holds the numeric alignment score under the scoring scheme used, the
/// ordered edit-op script, and — for local alignments — the start indices
/// of the aligned substrings within the original inputs.
///
/// For a global alignment ([`crate::NeedlemanWunsch`]),
/// `a_start == b_start == 0` and the script spans the entire pair.
///
/// For a local alignment ([`crate::SmithWaterman`]),
/// `a_start` and `b_start` are the indices into the original `a` and `b`
/// where the aligned substrings begin. The end indices can be computed as
/// [`Self::a_end`] and [`Self::b_end`].
#[derive(Debug, Clone, PartialEq)]
pub struct Alignment<T: Clone> {
    /// The alignment score under the scoring scheme used to compute it.
    pub score: i32,
    /// The ordered sequence of edit operations. Reading the script in order
    /// walks the aligned pair from left to right.
    pub script: Vec<EditOp<T>>,
    /// Start index in `a` of the aligned substring. `0` for global.
    pub a_start: usize,
    /// Start index in `b` of the aligned substring. `0` for global.
    pub b_start: usize,
}

impl<T: Clone> Alignment<T> {
    /// End index in `a` of the aligned substring (exclusive).
    ///
    /// Equals [`Self::a_start`] plus the count of ops that consume a symbol
    /// from `a` (matches, substitutions, deletes).
    #[must_use]
    pub fn a_end(&self) -> usize {
        self.a_start + self.script.iter().filter(|op| op.consumes_a()).count()
    }

    /// End index in `b` of the aligned substring (exclusive).
    ///
    /// Equals [`Self::b_start`] plus the count of ops that consume a symbol
    /// from `b` (matches, substitutions, inserts).
    #[must_use]
    pub fn b_end(&self) -> usize {
        self.b_start + self.script.iter().filter(|op| op.consumes_b()).count()
    }

    /// Reconstruct the aligned substring of `a` by walking the script.
    ///
    /// The returned vector equals `original_a[self.a_start..self.a_end()]`
    /// when the script was produced from that same `original_a`.
    #[must_use]
    pub fn extract_a(&self) -> Vec<T> {
        self.script
            .iter()
            .filter_map(|op| match op {
                EditOp::Match { a, .. } | EditOp::Substitute { a, .. } | EditOp::Delete { a } => {
                    Some(a.clone())
                }
                EditOp::Insert { .. } => None,
            })
            .collect()
    }

    /// Reconstruct the aligned substring of `b` by walking the script.
    ///
    /// The returned vector equals `original_b[self.b_start..self.b_end()]`
    /// when the script was produced from that same `original_b`.
    #[must_use]
    pub fn extract_b(&self) -> Vec<T> {
        self.script
            .iter()
            .filter_map(|op| match op {
                EditOp::Match { b, .. } | EditOp::Substitute { b, .. } | EditOp::Insert { b } => {
                    Some(b.clone())
                }
                EditOp::Delete { .. } => None,
            })
            .collect()
    }

    /// Apply this alignment's script to `a` and return the reconstructed
    /// aligned substring of `b`.
    ///
    /// For a global alignment, this equals `b`. For a local alignment, this
    /// equals `b[self.b_start..self.b_end()]`.
    ///
    /// This is a convenience wrapper: the `a` argument is not actually read
    /// (the script already carries every symbol it needs); it is accepted
    /// so the call site reads as an application of the script to a witness.
    #[must_use]
    pub fn apply_to(&self, a: &[T]) -> Vec<T> {
        let _ = a;
        self.extract_b()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumes_a_and_consumes_b_report_correctly() {
        let m: EditOp<u8> = EditOp::Match { a: b'A', b: b'A' };
        assert!(m.consumes_a() && m.consumes_b());
        let s: EditOp<u8> = EditOp::Substitute { a: b'A', b: b'C' };
        assert!(s.consumes_a() && s.consumes_b());
        let ins: EditOp<u8> = EditOp::Insert { b: b'C' };
        assert!(!ins.consumes_a() && ins.consumes_b());
        let del: EditOp<u8> = EditOp::Delete { a: b'A' };
        assert!(del.consumes_a() && !del.consumes_b());
    }

    #[test]
    fn extract_a_and_extract_b_reproduce_the_inputs() {
        // Alignment of "GATTACA" (a) and "GCATGCU" (b) — hand-built script
        // just to exercise extract_a / extract_b.
        let script = alloc::vec![
            EditOp::Match { a: b'G', b: b'G' },
            EditOp::Substitute { a: b'A', b: b'C' },
            EditOp::Match { a: b'T', b: b'A' },
            EditOp::Substitute { a: b'T', b: b'T' },
            EditOp::Insert { b: b'G' },
            EditOp::Delete { a: b'A' },
            EditOp::Match { a: b'C', b: b'C' },
            EditOp::Substitute { a: b'A', b: b'U' },
        ];
        let al = Alignment {
            score: 0,
            script,
            a_start: 0,
            b_start: 0,
        };
        assert_eq!(al.extract_a(), b"GATTACA".to_vec());
        assert_eq!(al.extract_b(), b"GCATGCU".to_vec());
        assert_eq!(al.a_end(), 7);
        assert_eq!(al.b_end(), 7);
    }

    #[test]
    fn apply_to_returns_the_b_side() {
        let script = alloc::vec![
            EditOp::Match { a: b'A', b: b'A' },
            EditOp::Insert { b: b'B' },
            EditOp::Delete { a: b'C' },
        ];
        let al = Alignment {
            score: 0,
            script,
            a_start: 0,
            b_start: 0,
        };
        assert_eq!(al.apply_to(b"AC"), b"AB".to_vec());
    }
}
