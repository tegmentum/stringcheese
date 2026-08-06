//! Golden test cases for the alignment algorithms.
//!
//! Because this module is `#[cfg(test)]`, its cases are compiled only into
//! the crate's test binaries — `comparand-corpus` is declared as a
//! dev-dependency for exactly that reason.
//!
//! Cases are grouped by algorithm and scoring scheme:
//!
//! * [`NW_LINEAR_CASES`] — Needleman-Wunsch under [`LinearGap`].
//! * [`NW_AFFINE_CASES`] — Needleman-Wunsch under [`AffineGap`].
//! * [`SW_LINEAR_CASES`] — Smith-Waterman under [`LinearGap`].
//! * [`SW_AFFINE_CASES`] — Smith-Waterman under [`AffineGap`].

use comparand_core::Score;
use comparand_corpus::{GoldenCase, GoldenSource};

use crate::needleman_wunsch::NeedlemanWunsch;
use crate::scoring::{AffineGap, LinearGap};
use crate::smith_waterman::SmithWaterman;

/// A pair of byte slices used as alignment input.
pub type BytesInput = (&'static [u8], &'static [u8]);

/// A golden case whose expected output is a single [`Score<i32>`].
pub type ScoreCase = GoldenCase<BytesInput, Score<i32>>;

// ---------------------------------------------------------------------------
// Needleman-Wunsch under LinearGap::simple(): match=1, mismatch=-1, gap=-1.
// ---------------------------------------------------------------------------

/// Golden cases for [`NeedlemanWunsch`] under [`LinearGap::simple`].
pub const NW_LINEAR_CASES: &[ScoreCase] = &[
    ScoreCase {
        id: "align/nw-linear/empty-empty",
        descriptor: NeedlemanWunsch::<LinearGap>::LINEAR_DESCRIPTOR,
        input: (b"", b""),
        expected: Score::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Zero-length global alignment has score 0 under any scheme.",
        tags: &["basic", "empty", "identity"],
    },
    ScoreCase {
        id: "align/nw-linear/identical-runs-of-matches",
        descriptor: NeedlemanWunsch::<LinearGap>::LINEAR_DESCRIPTOR,
        input: (b"AAAA", b"AAAA"),
        expected: Score::new(4),
        source: GoldenSource::IndependentlyDerived,
        notes: "Four matches at reward=1 give score 4.",
        tags: &["basic", "identity"],
    },
    ScoreCase {
        id: "align/nw-linear/a-empty-all-gaps",
        descriptor: NeedlemanWunsch::<LinearGap>::LINEAR_DESCRIPTOR,
        input: (b"AAAA", b""),
        expected: Score::new(-4),
        source: GoldenSource::IndependentlyDerived,
        notes: "Four deletions at gap=-1 give score -4.",
        tags: &["basic", "empty", "deletion"],
    },
    ScoreCase {
        id: "align/nw-linear/b-empty-all-inserts",
        descriptor: NeedlemanWunsch::<LinearGap>::LINEAR_DESCRIPTOR,
        input: (b"", b"AAAA"),
        expected: Score::new(-4),
        source: GoldenSource::IndependentlyDerived,
        notes: "Four insertions at gap=-1 give score -4.",
        tags: &["basic", "empty", "insertion"],
    },
    ScoreCase {
        id: "align/nw-linear/single-substitution",
        descriptor: NeedlemanWunsch::<LinearGap>::LINEAR_DESCRIPTOR,
        input: (b"AC", b"AG"),
        expected: Score::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "One match (+1) and one substitution (-1) sum to 0.",
        tags: &["basic", "substitution"],
    },
    ScoreCase {
        id: "align/nw-linear/textbook-gattaca-gcatgcu",
        descriptor: NeedlemanWunsch::<LinearGap>::LINEAR_DESCRIPTOR,
        input: (b"GATTACA", b"GCATGCU"),
        expected: Score::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Textbook example. Optimal DP score under match=1, \
                mismatch=-1, gap=-1 is 0.",
        tags: &["canonical", "textbook"],
    },
];

// ---------------------------------------------------------------------------
// Needleman-Wunsch under AffineGap::default_affine(): match=1, mismatch=-1,
// open=-2, extend=-1.
// ---------------------------------------------------------------------------

/// Golden cases for [`NeedlemanWunsch`] under [`AffineGap::default_affine`].
pub const NW_AFFINE_CASES: &[ScoreCase] = &[
    ScoreCase {
        id: "align/nw-affine/empty-empty",
        descriptor: NeedlemanWunsch::<AffineGap>::AFFINE_DESCRIPTOR,
        input: (b"", b""),
        expected: Score::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "Zero-length global alignment has score 0 under any scheme.",
        tags: &["basic", "empty", "identity"],
    },
    ScoreCase {
        id: "align/nw-affine/all-gap-open-plus-extends",
        descriptor: NeedlemanWunsch::<AffineGap>::AFFINE_DESCRIPTOR,
        input: (b"AAA", b""),
        expected: Score::new(-4),
        source: GoldenSource::IndependentlyDerived,
        notes: "One gap of length 3: open + 2*extend = -2 + 2*-1 = -4.",
        tags: &["basic", "empty", "deletion", "variant-boundary"],
    },
    ScoreCase {
        id: "align/nw-affine/prefers-one-long-gap-over-many-short",
        descriptor: NeedlemanWunsch::<AffineGap>::AFFINE_DESCRIPTOR,
        input: (b"AAAA", b"AABBBBAA"),
        expected: Score::new(-1),
        source: GoldenSource::IndependentlyDerived,
        notes: "Optimal alignment: AA----AA / AABBBBAA. Score = \
                4*match + (open + 3*extend) = 4 + (-2 + -3) = -1. \
                Alternative alignments that split the insert into two \
                shorter gaps pay two opens (2 * -2 = -4) plus fewer \
                extends but also produce mismatches, netting a worse \
                total.",
        tags: &["structure", "canonical"],
    },
];

// ---------------------------------------------------------------------------
// Smith-Waterman under LinearGap::simple(): match=1, mismatch=-1, gap=-1.
// ---------------------------------------------------------------------------

/// Golden cases for [`SmithWaterman`] under [`LinearGap::simple`].
pub const SW_LINEAR_CASES: &[ScoreCase] = &[
    ScoreCase {
        id: "align/sw-linear/no-positive-alignment",
        descriptor: SmithWaterman::<LinearGap>::LINEAR_DESCRIPTOR,
        input: (b"AAA", b"BBB"),
        expected: Score::new(0),
        source: GoldenSource::IndependentlyDerived,
        notes: "No matching pair exists; the empty local alignment (0) wins.",
        tags: &["basic", "empty"],
    },
    ScoreCase {
        id: "align/sw-linear/identical-runs-of-matches",
        descriptor: SmithWaterman::<LinearGap>::LINEAR_DESCRIPTOR,
        input: (b"AAAA", b"AAAA"),
        expected: Score::new(4),
        source: GoldenSource::IndependentlyDerived,
        notes: "The full identical sequence is also the best local match.",
        tags: &["basic", "identity"],
    },
    ScoreCase {
        id: "align/sw-linear/aligned-substring-inside-garbage",
        descriptor: SmithWaterman::<LinearGap>::LINEAR_DESCRIPTOR,
        input: (b"XXACGTYY", b"ZZACGTWW"),
        expected: Score::new(4),
        source: GoldenSource::IndependentlyDerived,
        notes: "The shared substring ACGT scores 4; flanking symbols are \
                discarded by the max-cell backtrace.",
        tags: &["canonical", "structure"],
    },
    ScoreCase {
        id: "align/sw-linear/textbook-agcacaca-acacacta",
        descriptor: SmithWaterman::<LinearGap>::LINEAR_DESCRIPTOR,
        input: (b"AGCACACA", b"ACACACTA"),
        expected: Score::new(5),
        source: GoldenSource::IndependentlyDerived,
        notes: "Best local alignment is a length-5 shared substring \
                (CACAC or ACACA) with all five positions matching, so \
                score = 5*match = 5. The trailing G/T mismatches and \
                the leading offset do not enter the reported score \
                thanks to the zero-flooring reset.",
        tags: &["canonical", "textbook"],
    },
];

// ---------------------------------------------------------------------------
// Smith-Waterman under AffineGap::default_affine().
// ---------------------------------------------------------------------------

/// Golden cases for [`SmithWaterman`] under [`AffineGap::default_affine`].
pub const SW_AFFINE_CASES: &[ScoreCase] = &[
    ScoreCase {
        id: "align/sw-affine/identical-full",
        descriptor: SmithWaterman::<AffineGap>::AFFINE_DESCRIPTOR,
        input: (b"AAAA", b"AAAA"),
        expected: Score::new(4),
        source: GoldenSource::IndependentlyDerived,
        notes: "Identical sequences have a perfect local match with no \
                gaps, so affine open/extend costs do not enter.",
        tags: &["basic", "identity"],
    },
    ScoreCase {
        id: "align/sw-affine/aligned-substring-inside-garbage",
        descriptor: SmithWaterman::<AffineGap>::AFFINE_DESCRIPTOR,
        input: (b"XXACGTYY", b"ZZACGTWW"),
        expected: Score::new(4),
        source: GoldenSource::IndependentlyDerived,
        notes: "Same substring outcome as the linear case; the affine \
                open cost does not apply to a gap-free local alignment.",
        tags: &["structure"],
    },
];

// ---------------------------------------------------------------------------
// In-crate tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeSet;

    fn count_cases() -> usize {
        NW_LINEAR_CASES.len()
            + NW_AFFINE_CASES.len()
            + SW_LINEAR_CASES.len()
            + SW_AFFINE_CASES.len()
    }

    #[test]
    fn corpus_meets_minimum_size() {
        assert!(
            count_cases() >= 12,
            "expected at least 12 golden cases, have {}",
            count_cases()
        );
    }

    #[test]
    fn every_case_has_a_unique_id() {
        let mut ids: BTreeSet<&str> = BTreeSet::new();
        for c in NW_LINEAR_CASES
            .iter()
            .chain(NW_AFFINE_CASES.iter())
            .chain(SW_LINEAR_CASES.iter())
            .chain(SW_AFFINE_CASES.iter())
        {
            assert!(ids.insert(c.id), "duplicate golden case id: {}", c.id);
        }
    }

    #[test]
    fn every_case_uses_the_correct_descriptor() {
        for c in NW_LINEAR_CASES {
            assert_eq!(
                c.descriptor,
                NeedlemanWunsch::<LinearGap>::LINEAR_DESCRIPTOR,
                "wrong descriptor on {}",
                c.id
            );
        }
        for c in NW_AFFINE_CASES {
            assert_eq!(
                c.descriptor,
                NeedlemanWunsch::<AffineGap>::AFFINE_DESCRIPTOR,
                "wrong descriptor on {}",
                c.id
            );
        }
        for c in SW_LINEAR_CASES {
            assert_eq!(
                c.descriptor,
                SmithWaterman::<LinearGap>::LINEAR_DESCRIPTOR,
                "wrong descriptor on {}",
                c.id
            );
        }
        for c in SW_AFFINE_CASES {
            assert_eq!(
                c.descriptor,
                SmithWaterman::<AffineGap>::AFFINE_DESCRIPTOR,
                "wrong descriptor on {}",
                c.id
            );
        }
    }

    #[test]
    fn every_nw_linear_case_matches_the_algorithm() {
        let nw = NeedlemanWunsch::new(LinearGap::simple());
        for c in NW_LINEAR_CASES {
            let got = nw.score(c.input.0, c.input.1);
            assert_eq!(got, c.expected, "case {} scored {:?}", c.id, got);
        }
    }

    #[test]
    fn every_nw_affine_case_matches_the_algorithm() {
        let nw = NeedlemanWunsch::new(AffineGap::default_affine());
        for c in NW_AFFINE_CASES {
            let got = nw.score(c.input.0, c.input.1);
            assert_eq!(got, c.expected, "case {} scored {:?}", c.id, got);
        }
    }

    #[test]
    fn every_sw_linear_case_matches_the_algorithm() {
        let sw = SmithWaterman::new(LinearGap::simple());
        for c in SW_LINEAR_CASES {
            let got = sw.score(c.input.0, c.input.1);
            assert_eq!(got, c.expected, "case {} scored {:?}", c.id, got);
        }
    }

    #[test]
    fn every_sw_affine_case_matches_the_algorithm() {
        let sw = SmithWaterman::new(AffineGap::default_affine());
        for c in SW_AFFINE_CASES {
            let got = sw.score(c.input.0, c.input.1);
            assert_eq!(got, c.expected, "case {} scored {:?}", c.id, got);
        }
    }

    #[test]
    fn needleman_wunsch_and_smith_waterman_share_no_family() {
        // The two aligners share the alignment domain but live in different
        // families so downstream registries can key on family without
        // collision.
        assert_ne!(
            NeedlemanWunsch::<LinearGap>::LINEAR_DESCRIPTOR.family,
            SmithWaterman::<LinearGap>::LINEAR_DESCRIPTOR.family,
        );
    }
}
