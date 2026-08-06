//! Canonical golden cases for the four similarity families in this crate.
//!
//! Every case is wired to the `comparand-corpus` [`FloatExpectation`]
//! schema, and every case's descriptor pins the specific algorithm variant
//! being tested — the `every_case_uses_the_correct_descriptor` test at the
//! bottom of the file rejects a mismatch, so a "Jaccard multiset" case
//! cannot silently be validated against the set variant.
//!
//! # Tolerance policy
//!
//! * **[`FloatExpectation::ExactBits`]** for cases whose expected value is
//!   a representable constant such as `0.0`, `1.0`, or `0.5`. Identity,
//!   empty-vs-empty, disjoint, and any hand-computed ratio that reduces to
//!   a power-of-two denominator.
//! * **[`FloatExpectation::Absolute`] with tolerance `1e-12`** for
//!   hand-computed rationals such as `2/3` that do not represent exactly
//!   in `f64`. A tighter tolerance would fail on legitimate round-off in
//!   `2.0 / 3.0` vs `2.0_f64 / 3.0_f64`-style reformulations.
//!
//! # Input shapes
//!
//! Rather than force every case to build a full [`GramSet`] /
//! [`GramMultiSet`] / [`GramVector`] via a generator, the golden cases here
//! carry a small deterministic *builder spec* — a list of characters, and
//! a shape flag — and the harness materializes the correct representation
//! at run time. That keeps the case data legible without adding a new
//! [`Vec`]-typed input for every algorithm variant. See [`SetSimInput`]
//! and [`SetSimShape`] below.
//!
//! One "realistic" case exercises a character-3-gram flow end-to-end:
//! see `partial-overlap/kitten-sitting-char3` below and its runner in the
//! `tests` submodule. It uses [`CharacterGrams`] with padding `None` and
//! [`GramSet::from_generator`] to build the two sets from raw `&[char]`.
//!
//! [`CharacterGrams`]: comparand_ngram::CharacterGrams
//! [`GramSet`]: comparand_ngram::GramSet
//! [`GramSet::from_generator`]: comparand_ngram::GramSet::from_generator
//! [`GramMultiSet`]: comparand_ngram::GramMultiSet
//! [`GramVector`]: comparand_ngram::GramVector
//! [`Vec`]: alloc::vec::Vec

use comparand_corpus::{FloatExpectation, GoldenCase, GoldenSource};

use crate::cosine::Cosine;
use crate::dice::{DiceOverMultiSet, DiceOverSet};
use crate::jaccard::{JaccardOverMultiSet, JaccardOverSet};
use crate::overlap::Overlap;

/// Shape flag for a [`SetSimInput`]: how the character list should be
/// interpreted when the harness materializes it into a gram
/// representation.
///
/// The character list itself is small and comes from the test's hand
/// derivation — one character per gram — so we do not push a full
/// [`comparand_ngram::CharacterGrams`] generator through every case. The
/// `Chars3Gram` variant is the sole exception and drives the one
/// end-to-end realistic case.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SetSimShape {
    /// Each character in the list becomes a single-character `Vec<char>`
    /// gram, deduplicated into a [`comparand_ngram::GramSet`].
    Set,
    /// Same character list, but preserved as a multiset with each
    /// character counted separately, stored in a
    /// [`comparand_ngram::GramMultiSet<char>`].
    MultiSet,
    /// Same character list, materialized as a
    /// [`comparand_ngram::GramVector<char>`] whose per-gram weight is the
    /// count of that character in the list.
    Vector,
    /// The character list is treated as a raw input string, passed through
    /// a character-3-gram generator with [`comparand_ngram::PaddingPolicy::None`],
    /// and consumed as a [`comparand_ngram::GramSet<Vec<char>>`].
    Chars3Gram,
}

/// A single-side input to a golden case.
///
/// Carries the raw character list and the shape it should be materialized
/// into. The paired [`SetSimInput`] on the other side of a case must
/// carry the same shape; the driver panics otherwise, and a shape
/// mismatch is a schema error.
#[derive(Copy, Clone, Debug)]
pub struct SetSimInput {
    /// The character list, either the direct gram characters
    /// (`Set` / `MultiSet` / `Vector`) or the raw input to the
    /// character-3-gram generator (`Chars3Gram`).
    pub chars: &'static [char],
    /// How the list should be materialized.
    pub shape: SetSimShape,
}

/// A golden case for one of this crate's four similarity families.
pub type SetSimCase = GoldenCase<(SetSimInput, SetSimInput), FloatExpectation>;

// Convenience constant to keep case rows terse.
const IND: GoldenSource = GoldenSource::IndependentlyDerived;

// --- DiceOverSet cases ---------------------------------------------------

/// Golden cases for [`DiceOverSet`].
pub const GOLDEN_DICE_SET: &[SetSimCase] = &[
    GoldenCase {
        id: "dice-set/empty-empty",
        descriptor: DiceOverSet::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &[],
                shape: SetSimShape::Set,
            },
            SetSimInput {
                chars: &[],
                shape: SetSimShape::Set,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: IND,
        notes: "Empty-vs-empty is treated as identity under the crate-wide convention; dice = 1.0.",
        tags: &["basic", "empty", "identity", "exact-bits"],
    },
    GoldenCase {
        id: "dice-set/identical-abc",
        descriptor: DiceOverSet::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'b', 'c'],
                shape: SetSimShape::Set,
            },
            SetSimInput {
                chars: &['a', 'b', 'c'],
                shape: SetSimShape::Set,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: IND,
        notes: "Identical sets: 2*3 / (3+3) = 1.0 exactly.",
        tags: &["basic", "identity", "exact-bits"],
    },
    GoldenCase {
        id: "dice-set/disjoint",
        descriptor: DiceOverSet::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'b', 'c'],
                shape: SetSimShape::Set,
            },
            SetSimInput {
                chars: &['x', 'y', 'z'],
                shape: SetSimShape::Set,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 0.0_f64.to_bits(),
        },
        source: IND,
        notes: "Disjoint alphabets: intersection = 0, dice = 0.0 exactly.",
        tags: &["basic", "disjoint", "exact-bits"],
    },
    GoldenCase {
        id: "dice-set/partial-overlap-abc-bcd",
        descriptor: DiceOverSet::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'b', 'c'],
                shape: SetSimShape::Set,
            },
            SetSimInput {
                chars: &['b', 'c', 'd'],
                shape: SetSimShape::Set,
            },
        ),
        expected: FloatExpectation::Absolute {
            value: 2.0_f64 / 3.0_f64,
            tolerance: 1e-12,
        },
        source: IND,
        notes: "{a,b,c} vs {b,c,d}: inter = 2, denom = 6, dice = 4/6 = 2/3.",
        tags: &["partial", "derivation"],
    },
    GoldenCase {
        id: "dice-set/subset-ab-abcd",
        descriptor: DiceOverSet::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'b'],
                shape: SetSimShape::Set,
            },
            SetSimInput {
                chars: &['a', 'b', 'c', 'd'],
                shape: SetSimShape::Set,
            },
        ),
        expected: FloatExpectation::Absolute {
            value: 2.0_f64 / 3.0_f64,
            tolerance: 1e-12,
        },
        source: IND,
        notes: "{a,b} vs {a,b,c,d}: inter = 2, denom = 6, dice = 4/6 = 2/3.",
        tags: &["subset", "derivation"],
    },
];

// --- DiceOverMultiSet cases ---------------------------------------------

/// Golden cases for [`DiceOverMultiSet`].
pub const GOLDEN_DICE_MULTISET: &[SetSimCase] = &[
    GoldenCase {
        id: "dice-multiset/aab-abb",
        descriptor: DiceOverMultiSet::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'a', 'b'],
                shape: SetSimShape::MultiSet,
            },
            SetSimInput {
                chars: &['a', 'b', 'b'],
                shape: SetSimShape::MultiSet,
            },
        ),
        expected: FloatExpectation::Absolute {
            value: 2.0_f64 / 3.0_f64,
            tolerance: 1e-12,
        },
        source: IND,
        notes: "Min counts a=1, b=1 → inter = 2; total counts 3+3 = 6; dice = 4/6 = 2/3.",
        tags: &["multiset", "derivation"],
    },
    GoldenCase {
        id: "dice-multiset/identical-repeats",
        descriptor: DiceOverMultiSet::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['x', 'x', 'y', 'y', 'y'],
                shape: SetSimShape::MultiSet,
            },
            SetSimInput {
                chars: &['x', 'x', 'y', 'y', 'y'],
                shape: SetSimShape::MultiSet,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: IND,
        notes: "Identical multisets: dice = 1.0 exactly regardless of multiplicity.",
        tags: &["multiset", "identity", "exact-bits"],
    },
];

// --- JaccardOverSet cases ------------------------------------------------

/// Golden cases for [`JaccardOverSet`].
pub const GOLDEN_JACCARD_SET: &[SetSimCase] = &[
    GoldenCase {
        id: "jaccard-set/empty-empty",
        descriptor: JaccardOverSet::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &[],
                shape: SetSimShape::Set,
            },
            SetSimInput {
                chars: &[],
                shape: SetSimShape::Set,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: IND,
        notes: "Empty-vs-empty: jaccard = 1.0 under the crate-wide identity convention.",
        tags: &["basic", "empty", "identity", "exact-bits"],
    },
    GoldenCase {
        id: "jaccard-set/partial-overlap-abc-bcd",
        descriptor: JaccardOverSet::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'b', 'c'],
                shape: SetSimShape::Set,
            },
            SetSimInput {
                chars: &['b', 'c', 'd'],
                shape: SetSimShape::Set,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 0.5_f64.to_bits(),
        },
        source: IND,
        notes: "{a,b,c} vs {b,c,d}: inter = 2, union = 4, jaccard = 2/4 = 0.5 exactly.",
        tags: &["partial", "exact-bits"],
    },
    GoldenCase {
        id: "jaccard-set/subset-ab-abcd",
        descriptor: JaccardOverSet::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'b'],
                shape: SetSimShape::Set,
            },
            SetSimInput {
                chars: &['a', 'b', 'c', 'd'],
                shape: SetSimShape::Set,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 0.5_f64.to_bits(),
        },
        source: IND,
        notes: "{a,b} vs {a,b,c,d}: inter = 2, union = 4, jaccard = 2/4 = 0.5 — distinguishes from Overlap, which yields 1.0.",
        tags: &["subset", "exact-bits", "distinguishing"],
    },
    GoldenCase {
        id: "jaccard-set/disjoint",
        descriptor: JaccardOverSet::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'b'],
                shape: SetSimShape::Set,
            },
            SetSimInput {
                chars: &['c', 'd'],
                shape: SetSimShape::Set,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 0.0_f64.to_bits(),
        },
        source: IND,
        notes: "Disjoint sets: inter = 0, jaccard = 0.0 exactly.",
        tags: &["basic", "disjoint", "exact-bits"],
    },
    GoldenCase {
        id: "jaccard-set/kitten-sitting-char3",
        descriptor: JaccardOverSet::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['k', 'i', 't', 't', 'e', 'n'],
                shape: SetSimShape::Chars3Gram,
            },
            SetSimInput {
                chars: &['s', 'i', 't', 't', 'i', 'n', 'g'],
                shape: SetSimShape::Chars3Gram,
            },
        ),
        // Trigrams over "kitten" (no padding): kit, itt, tte, ten → 4 distinct.
        // Trigrams over "sitting" (no padding): sit, itt, tti, tin, ing → 5 distinct.
        // Intersection = {itt} = 1. Union = 4 + 5 - 1 = 8. jaccard = 1/8.
        expected: FloatExpectation::ExactBits {
            value: (1.0_f64 / 8.0_f64).to_bits(),
        },
        source: IND,
        notes: "Character 3-gram Jaccard for 'kitten' vs 'sitting' (no padding): intersection = {itt}, union has 8 grams, jaccard = 1/8.",
        tags: &["realistic", "chars-3gram", "exact-bits"],
    },
];

// --- JaccardOverMultiSet cases ------------------------------------------

/// Golden cases for [`JaccardOverMultiSet`].
pub const GOLDEN_JACCARD_MULTISET: &[SetSimCase] = &[GoldenCase {
    id: "jaccard-multiset/aab-abb",
    descriptor: JaccardOverMultiSet::DESCRIPTOR,
    input: (
        SetSimInput {
            chars: &['a', 'a', 'b'],
            shape: SetSimShape::MultiSet,
        },
        SetSimInput {
            chars: &['a', 'b', 'b'],
            shape: SetSimShape::MultiSet,
        },
    ),
    // min: a=1, b=1 → sum 2. max: a=2, b=2 → sum 4. weighted-jac = 2/4 = 0.5.
    expected: FloatExpectation::ExactBits {
        value: 0.5_f64.to_bits(),
    },
    source: IND,
    notes: "Multiplicity differs from set: set-jaccard is 1.0, weighted-multiset is 0.5.",
    tags: &["multiset", "distinguishing", "exact-bits"],
}];

// --- Overlap cases -------------------------------------------------------

/// Golden cases for [`Overlap`].
pub const GOLDEN_OVERLAP: &[SetSimCase] = &[
    GoldenCase {
        id: "overlap/empty-empty",
        descriptor: Overlap::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &[],
                shape: SetSimShape::Set,
            },
            SetSimInput {
                chars: &[],
                shape: SetSimShape::Set,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: IND,
        notes: "Empty-vs-empty: overlap = 1.0 under the crate-wide identity convention.",
        tags: &["basic", "empty", "identity", "exact-bits"],
    },
    GoldenCase {
        id: "overlap/subset-yields-one",
        descriptor: Overlap::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'b'],
                shape: SetSimShape::Set,
            },
            SetSimInput {
                chars: &['a', 'b', 'c', 'd'],
                shape: SetSimShape::Set,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: IND,
        notes: "Subset relation: overlap = |A|/|A| = 1.0. This is the trip-wire case that distinguishes overlap from Jaccard.",
        tags: &["subset", "trip-wire", "exact-bits"],
    },
    GoldenCase {
        id: "overlap/partial-abc-bcd",
        descriptor: Overlap::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'b', 'c'],
                shape: SetSimShape::Set,
            },
            SetSimInput {
                chars: &['b', 'c', 'd'],
                shape: SetSimShape::Set,
            },
        ),
        expected: FloatExpectation::Absolute {
            value: 2.0_f64 / 3.0_f64,
            tolerance: 1e-12,
        },
        source: IND,
        notes: "{a,b,c} vs {b,c,d}: inter = 2, min(3, 3) = 3, overlap = 2/3.",
        tags: &["partial", "derivation"],
    },
];

// --- Cosine cases -------------------------------------------------------

/// Golden cases for [`Cosine`].
///
/// These are gated on `std` because [`Cosine`] itself requires `std` for
/// `f64::sqrt`; the module-level cfg on the driver test below mirrors the
/// gate.
#[cfg(feature = "std")]
pub const GOLDEN_COSINE: &[SetSimCase] = &[
    GoldenCase {
        id: "cosine/empty-empty",
        descriptor: Cosine::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &[],
                shape: SetSimShape::Vector,
            },
            SetSimInput {
                chars: &[],
                shape: SetSimShape::Vector,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: IND,
        notes: "Both vectors empty (zero-norm): cosine = 1.0 by the crate-wide identity convention.",
        tags: &["basic", "empty", "identity", "exact-bits"],
    },
    GoldenCase {
        id: "cosine/identical",
        descriptor: Cosine::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'b', 'c'],
                shape: SetSimShape::Vector,
            },
            SetSimInput {
                chars: &['a', 'b', 'c'],
                shape: SetSimShape::Vector,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 1.0_f64.to_bits(),
        },
        source: IND,
        notes: "Identical unit-count vectors: dot = 3, norm² = 3 each, cosine = 3 / 3 = 1.0.",
        tags: &["basic", "identity", "exact-bits"],
    },
    GoldenCase {
        id: "cosine/disjoint",
        descriptor: Cosine::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'b'],
                shape: SetSimShape::Vector,
            },
            SetSimInput {
                chars: &['x', 'y'],
                shape: SetSimShape::Vector,
            },
        ),
        expected: FloatExpectation::ExactBits {
            value: 0.0_f64.to_bits(),
        },
        source: IND,
        notes: "Disjoint supports: dot = 0, cosine = 0.0 exactly.",
        tags: &["basic", "disjoint", "exact-bits"],
    },
    GoldenCase {
        id: "cosine/partial-overlap-abc-abd",
        descriptor: Cosine::DESCRIPTOR,
        input: (
            SetSimInput {
                chars: &['a', 'b', 'c'],
                shape: SetSimShape::Vector,
            },
            SetSimInput {
                chars: &['a', 'b', 'd'],
                shape: SetSimShape::Vector,
            },
        ),
        expected: FloatExpectation::Absolute {
            value: 2.0_f64 / 3.0_f64,
            tolerance: 1e-12,
        },
        source: IND,
        notes: "Unit-count vectors: dot = 2, ||a|| = ||b|| = sqrt(3), cosine = 2 / 3.",
        tags: &["partial", "derivation"],
    },
];

// A build without `std` still needs a `GOLDEN_COSINE` symbol so the
// driver tests compile without cfg-splitting.
#[cfg(not(feature = "std"))]
pub const GOLDEN_COSINE: &[SetSimCase] = &[];

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use comparand_core::{AlgorithmDescriptor, SimilarityMetric};
    use comparand_ngram::{CharacterGrams, GramMultiSet, GramSet, GramVector, PaddingPolicy};

    fn all_cases() -> Vec<&'static SetSimCase> {
        let mut v: Vec<&'static SetSimCase> = Vec::new();
        v.extend(GOLDEN_DICE_SET.iter());
        v.extend(GOLDEN_DICE_MULTISET.iter());
        v.extend(GOLDEN_JACCARD_SET.iter());
        v.extend(GOLDEN_JACCARD_MULTISET.iter());
        v.extend(GOLDEN_OVERLAP.iter());
        #[cfg(feature = "std")]
        v.extend(GOLDEN_COSINE.iter());
        v
    }

    fn build_set(input: &SetSimInput) -> GramSet<alloc::vec::Vec<char>> {
        input.chars.iter().map(|c| alloc::vec![*c]).collect()
    }

    fn build_multiset(input: &SetSimInput) -> GramMultiSet<char> {
        let mut ms = GramMultiSet::new();
        for c in input.chars {
            ms.add(*c);
        }
        ms
    }

    fn build_vector(input: &SetSimInput) -> GramVector<char> {
        let mut v: GramVector<char> = GramVector::new();
        for c in input.chars {
            v.add(*c, 1.0);
        }
        v
    }

    fn build_chars3gram_set(input: &SetSimInput) -> GramSet<alloc::vec::Vec<char>> {
        let generator = CharacterGrams::new(3, PaddingPolicy::<char>::None);
        GramSet::from_generator(&generator, input.chars)
    }

    fn descriptor_for(case: &SetSimCase) -> AlgorithmDescriptor {
        case.descriptor
    }

    fn run_case(case: &SetSimCase) -> (f64, bool) {
        let d = descriptor_for(case);
        let (lhs, rhs) = &case.input;
        assert_eq!(
            lhs.shape, rhs.shape,
            "golden case {} has mismatched shapes on the two sides",
            case.id
        );

        // Dispatch on the descriptor. A more elaborate registry would
        // fit better once the crate is folded back into the workspace;
        // for now the switch here is exhaustive across the four
        // families this crate ships.
        let observed = match d {
            _ if d == DiceOverSet::DESCRIPTOR => {
                let a = build_set(lhs);
                let b = build_set(rhs);
                DiceOverSet.similarity(&a, &b).into_inner()
            }
            _ if d == DiceOverMultiSet::DESCRIPTOR => {
                let a = build_multiset(lhs);
                let b = build_multiset(rhs);
                DiceOverMultiSet.similarity(&a, &b).into_inner()
            }
            _ if d == JaccardOverSet::DESCRIPTOR => match lhs.shape {
                SetSimShape::Chars3Gram => {
                    let a = build_chars3gram_set(lhs);
                    let b = build_chars3gram_set(rhs);
                    JaccardOverSet.similarity(&a, &b).into_inner()
                }
                SetSimShape::Set => {
                    let a = build_set(lhs);
                    let b = build_set(rhs);
                    JaccardOverSet.similarity(&a, &b).into_inner()
                }
                other => panic!(
                    "Jaccard-set case {} used unsupported shape {other:?}",
                    case.id
                ),
            },
            _ if d == JaccardOverMultiSet::DESCRIPTOR => {
                let a = build_multiset(lhs);
                let b = build_multiset(rhs);
                JaccardOverMultiSet.similarity(&a, &b).into_inner()
            }
            _ if d == Overlap::DESCRIPTOR => {
                let a = build_set(lhs);
                let b = build_set(rhs);
                Overlap.similarity(&a, &b).into_inner()
            }
            #[cfg(feature = "std")]
            _ if d == Cosine::DESCRIPTOR => {
                let a = build_vector(lhs);
                let b = build_vector(rhs);
                Cosine.similarity(&a, &b).into_inner()
            }
            other => panic!(
                "golden case {} carries an unknown descriptor {other:?}",
                case.id
            ),
        };

        (observed, case.expected.matches(observed))
    }

    #[test]
    fn every_case_uses_the_correct_descriptor() {
        for c in GOLDEN_DICE_SET {
            assert_eq!(c.descriptor, DiceOverSet::DESCRIPTOR, "case {}", c.id);
        }
        for c in GOLDEN_DICE_MULTISET {
            assert_eq!(c.descriptor, DiceOverMultiSet::DESCRIPTOR, "case {}", c.id);
        }
        for c in GOLDEN_JACCARD_SET {
            assert_eq!(c.descriptor, JaccardOverSet::DESCRIPTOR, "case {}", c.id);
        }
        for c in GOLDEN_JACCARD_MULTISET {
            assert_eq!(
                c.descriptor,
                JaccardOverMultiSet::DESCRIPTOR,
                "case {}",
                c.id
            );
        }
        for c in GOLDEN_OVERLAP {
            assert_eq!(c.descriptor, Overlap::DESCRIPTOR, "case {}", c.id);
        }
        #[cfg(feature = "std")]
        for c in GOLDEN_COSINE {
            assert_eq!(c.descriptor, Cosine::DESCRIPTOR, "case {}", c.id);
        }
    }

    #[test]
    fn every_case_matches_algorithm() {
        for case in all_cases() {
            let (observed, ok) = run_case(case);
            assert!(
                ok,
                "golden case {} disagreed: expected {:?}, observed {observed}",
                case.id, case.expected
            );
        }
    }

    #[test]
    fn corpus_meets_minimum_size() {
        // Spec requires at least 12 cases across the crate.
        let count = all_cases().len();
        assert!(
            count >= 12,
            "expected at least 12 golden cases across the crate, got {count}"
        );
    }

    #[test]
    fn every_case_has_a_unique_id() {
        let ids: Vec<&str> = all_cases().iter().map(|c| c.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate golden-case id detected");
    }
}
