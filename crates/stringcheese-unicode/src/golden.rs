//! Golden fixtures for Unicode preprocessing.
//!
//! These fixtures are the crate's canonical test vectors. They follow
//! the `GoldenCase<I, O>` shape from `stringcheese-corpus`
//! (`GoldenCase<&'static str, &'static str>` here — inputs and expected
//! outputs are both static strings). Each fixture records the
//! transformation family, an origin source (`Standard { name: … }` for
//! Unicode-standard cases and `IndependentlyDerived` for cases the
//! author derived from the algorithm definition), and free-form notes.
//!
//! To keep this file executable inside the crate we do **not** import
//! `stringcheese-corpus`'s types — that would create a dev-dependency
//! coupling this preprocessing crate doesn't otherwise need. The
//! fixtures below use a local `Fixture` type with the same shape.
//! The reconciliation pass that re-members this crate into the workspace
//! is where a corpus-crate coupling could be introduced.
//!
//! # Coverage
//!
//! At least 15 fixtures, spanning:
//!
//! - NFC/NFD/NFKC/NFKD, drawn from Unicode's own examples in
//!   `NormalizationTest.txt` (Unicode 15.0 release; the tables the
//!   `unicode-normalization` crate targets are drawn from the same
//!   source, so the fixtures serve as anchor points, not double-checks
//!   of an unrelated implementation).
//! - Case folding — sharp-S expansion, Turkic dotted/dotless I,
//!   ligature interaction with NFKC.
//! - Grapheme clusters — precomposed and decomposed accented
//!   characters, the family emoji, and a regional-indicator flag.
//! - Diacritic stripping — `café → cafe`, `naïve → naive`, `Æ →
//!   Æ` (single scalar, not a combining sequence), and untouched
//!   Cyrillic / CJK text.

use crate::{
    case_fold, case_fold_turkic, graphemes::GraphemeSequence, nfc, nfd, nfkc, nfkd,
    strip_diacritics,
};

/// Local mirror of the `GoldenSource` variants relevant to this crate,
/// kept here to avoid a cross-crate coupling for tests alone.
#[derive(Debug, Clone, Copy)]
enum Source {
    /// Derived from a formal standard.
    Standard(&'static str),
    /// Derived by hand from the algorithm's definition.
    IndependentlyDerived,
}

/// Local `GoldenCase<&str, &str>` mirror.
#[derive(Debug, Clone, Copy)]
struct Fixture {
    id: &'static str,
    input: &'static str,
    expected: &'static str,
    source: Source,
    notes: &'static str,
}

// --- Normalization fixtures --------------------------------------------------

// The following NFC/NFD/NFKC/NFKD cases are anchored to Unicode
// Consortium test data from `NormalizationTest.txt`. Where a case is
// visually short we spell out the code points explicitly rather than
// paste hard-to-read UTF-8.

const NFC_CASES: &[Fixture] = &[
    Fixture {
        id: "normalization/nfc/precomposed-latin-small-e-acute",
        // Input: "e" + combining acute.
        input: "e\u{0301}",
        // Expected: precomposed "é" (U+00E9).
        expected: "\u{00E9}",
        source: Source::Standard("Unicode 15.0 NormalizationTest.txt"),
        notes: "Canonical composition of Latin small e + combining acute.",
    },
    Fixture {
        id: "normalization/nfc/angstrom-sign-to-a-ring",
        // Input: Angstrom sign (U+212B).
        input: "\u{212B}",
        // Expected: Latin capital A with ring above (U+00C5).
        expected: "\u{00C5}",
        source: Source::Standard("Unicode 15.0 NormalizationTest.txt"),
        notes: "Angstrom sign canonically composes to Å (U+00C5).",
    },
    Fixture {
        id: "normalization/nfc/hangul-syllable-decomposed-jamos",
        // Input: HANGUL CHOSEONG KIYEOK + HANGUL JUNGSEONG A (U+1100 U+1161).
        input: "\u{1100}\u{1161}",
        // Expected: HANGUL SYLLABLE GA (U+AC00).
        expected: "\u{AC00}",
        source: Source::Standard("Unicode 15.0 NormalizationTest.txt"),
        notes: "Conjoining Jamos compose to the precomposed Hangul syllable.",
    },
];

const NFD_CASES: &[Fixture] = &[
    Fixture {
        id: "normalization/nfd/precomposed-e-acute-to-decomposed",
        input: "\u{00E9}",
        expected: "e\u{0301}",
        source: Source::Standard("Unicode 15.0 NormalizationTest.txt"),
        notes: "Canonical decomposition of Latin small e with acute.",
    },
    Fixture {
        id: "normalization/nfd/hangul-syllable-to-jamos",
        input: "\u{AC00}",
        expected: "\u{1100}\u{1161}",
        source: Source::Standard("Unicode 15.0 NormalizationTest.txt"),
        notes: "Precomposed Hangul syllable decomposes to conjoining jamos.",
    },
];

const NFKC_CASES: &[Fixture] = &[
    Fixture {
        id: "normalization/nfkc/fi-ligature-to-fi",
        // Input: U+FB01 LATIN SMALL LIGATURE FI.
        input: "\u{FB01}",
        expected: "fi",
        source: Source::Standard("Unicode 15.0 NormalizationTest.txt"),
        notes: "Compatibility decomposition of the fi-ligature.",
    },
    Fixture {
        id: "normalization/nfkc/roman-numeral-four-to-iv",
        // Input: U+2163 ROMAN NUMERAL FOUR.
        input: "\u{2163}",
        expected: "IV",
        source: Source::Standard("Unicode 15.0 NormalizationTest.txt"),
        notes: "Roman numeral four decomposes to ASCII IV under NFKC.",
    },
];

const NFKD_CASES: &[Fixture] = &[Fixture {
    id: "normalization/nfkd/superscript-two-to-two",
    input: "\u{00B2}",
    expected: "2",
    source: Source::Standard("Unicode 15.0 NormalizationTest.txt"),
    notes: "Superscript two decomposes to ASCII 2 under NFKD.",
}];

// --- Case-folding fixtures ---------------------------------------------------

const CASE_FOLD_CASES: &[Fixture] = &[
    Fixture {
        id: "case-folding/strasse-full-folds-to-strasse",
        input: "STRAßE",
        expected: "strasse",
        source: Source::Standard("Unicode 15.0 CaseFolding.txt (F mapping)"),
        notes: "German sharp S expands to 'ss' under full folding — the \
                exact behavior that distinguishes full from simple case \
                folding.",
    },
    Fixture {
        id: "case-folding/masse-and-masse-agree",
        input: "MASSE",
        // "Maße" folds to "masse"; MASSE folds to "masse" also.
        expected: "masse",
        source: Source::IndependentlyDerived,
        notes: "The design's motivating example: full folding makes \
                MASSE and Maße compare equal.",
    },
    Fixture {
        id: "case-folding/dotted-capital-i-default",
        // Input: U+0130 (Latin capital I with dot above).
        input: "\u{0130}",
        // Expected: i + combining dot above (default, non-Turkic fold).
        expected: "i\u{0307}",
        source: Source::Standard("Unicode 15.0 CaseFolding.txt (F mapping)"),
        notes: "Under default (non-Turkic) full folding, dotted capital I \
                becomes i + combining dot above.",
    },
];

/// Extra Turkic fixture — not part of the `Fixture` scan below because
/// it uses a different transformation. Kept alongside for organizational
/// symmetry.
const TURKIC_FOLD_ISTANBUL: (&str, &str) = ("İstanbul", "istanbul");

// --- Grapheme fixtures -------------------------------------------------------

/// Grapheme fixtures are (input, expected grapheme count). The scanner
/// checks that [`GraphemeSequence::new`]'s len matches.
const GRAPHEME_CASES: &[(&str, usize, &str)] = &[
    ("naïve", 5, "grapheme/naive-precomposed-is-five-graphemes"),
    (
        "cafe\u{0301}",
        4,
        "grapheme/cafe-decomposed-is-four-graphemes",
    ),
    ("\u{1F1EC}\u{1F1E7}", 1, "grapheme/uk-flag-is-one-grapheme"),
    (
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
        1,
        "grapheme/family-emoji-is-one-grapheme",
    ),
];

// --- Diacritic-strip fixtures ------------------------------------------------

const STRIP_CASES: &[Fixture] = &[
    Fixture {
        id: "strip-diacritics/cafe-strips",
        input: "café",
        expected: "cafe",
        source: Source::IndependentlyDerived,
        notes: "Precomposed é decomposes to e + combining acute; the \
                acute is a Mn and is dropped.",
    },
    Fixture {
        id: "strip-diacritics/naive-strips",
        input: "naïve",
        expected: "naive",
        source: Source::IndependentlyDerived,
        notes: "Diaeresis over i is dropped.",
    },
    Fixture {
        id: "strip-diacritics/ae-ligature-preserved",
        input: "Æ",
        expected: "Æ",
        source: Source::IndependentlyDerived,
        notes: "Æ is a single scalar with no decomposition into base + \
                mark, so it is preserved. Transliteration (Æ → AE) is \
                a separate, future concern.",
    },
    Fixture {
        id: "strip-diacritics/cyrillic-without-precomposed-short-i-unchanged",
        input: "Санкт-Петербург",
        expected: "Санкт-Петербург",
        source: Source::IndependentlyDerived,
        notes: "Cyrillic text with no decomposable combining marks \
                (Москва, Санкт-Петербург) round-trips unchanged. Note \
                that a string containing precomposed short-i (й, \
                U+0439) *would* be affected — that is the character's \
                canonical decomposition to И + combining short.",
    },
    Fixture {
        id: "strip-diacritics/cjk-unchanged",
        input: "東京",
        expected: "東京",
        source: Source::IndependentlyDerived,
        notes: "CJK characters have no combining marks.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    // The following four tests are the actual golden checks. They
    // execute each fixture and assert the observed output matches
    // `Fixture::expected`. A failure here means either a regression
    // in a StringCheese transformation or an update to a dependency
    // (unicode-normalization, icu_casemap, unicode-segmentation)
    // whose behavior changed for one of the anchored inputs.
    //
    // Total case count (sanity-checked in `golden_case_count`
    // below): 3 (NFC) + 2 (NFD) + 2 (NFKC) + 1 (NFKD) + 3 (case
    // fold) + 4 (graphemes) + 5 (strip) + 1 (Turkic) = 21.

    #[test]
    fn golden_normalization_cases() {
        for f in NFC_CASES {
            assert_eq!(nfc(f.input), f.expected, "id={}", f.id);
        }
        for f in NFD_CASES {
            assert_eq!(nfd(f.input), f.expected, "id={}", f.id);
        }
        for f in NFKC_CASES {
            assert_eq!(nfkc(f.input), f.expected, "id={}", f.id);
        }
        for f in NFKD_CASES {
            assert_eq!(nfkd(f.input), f.expected, "id={}", f.id);
        }
    }

    #[test]
    fn golden_case_folding_cases() {
        for f in CASE_FOLD_CASES {
            assert_eq!(case_fold(f.input), f.expected, "id={}", f.id);
        }
        let (input, expected) = TURKIC_FOLD_ISTANBUL;
        assert_eq!(case_fold_turkic(input), expected);
    }

    #[test]
    fn golden_grapheme_cases() {
        for &(input, expected_len, id) in GRAPHEME_CASES {
            let seq = GraphemeSequence::new(input);
            assert_eq!(seq.len(), expected_len, "id={id}");
        }
    }

    #[test]
    fn golden_diacritic_strip_cases() {
        for f in STRIP_CASES {
            assert_eq!(strip_diacritics(f.input), f.expected, "id={}", f.id);
        }
    }

    #[test]
    fn golden_case_count() {
        let count = NFC_CASES.len()
            + NFD_CASES.len()
            + NFKC_CASES.len()
            + NFKD_CASES.len()
            + CASE_FOLD_CASES.len()
            + GRAPHEME_CASES.len()
            + STRIP_CASES.len()
            + 1 /* Turkic */;
        assert!(count >= 15, "golden-case coverage regressed: {count} < 15");
    }

    // Sources are recorded via the `Source` enum on each fixture; a
    // silent conversion to another type would erase provenance. The
    // following smoke test just ensures every fixture's source variant
    // is one of the two we permit here.
    #[test]
    fn golden_sources_are_recorded() {
        let all: &[&[Fixture]] = &[
            NFC_CASES,
            NFD_CASES,
            NFKC_CASES,
            NFKD_CASES,
            CASE_FOLD_CASES,
            STRIP_CASES,
        ];
        for group in all {
            for f in *group {
                match f.source {
                    Source::Standard(name) => assert!(!name.is_empty(), "id={}", f.id),
                    Source::IndependentlyDerived => {
                        assert!(!f.notes.is_empty(), "id={}", f.id);
                    }
                }
            }
        }
    }
}
