//! Canonical search golden cases wired to the `stringcheese-corpus` schema.
//!
//! Each case pairs a search input (pattern + haystack) with an expected
//! list of matches, plus the descriptor of the algorithm the case applies
//! to. Cases live in per-algorithm arrays because the algorithms have
//! different descriptors — a Boyer-Moore case cannot silently be validated
//! against KMP.
//!
//! Multi-pattern Aho-Corasick cases carry the whole pattern set as their
//! input, and their expected value is a list of `(position,
//! pattern_index)` pairs.

use alloc::vec::Vec;

use stringcheese_corpus::{GoldenCase, GoldenSource};

use crate::search::aho_corasick::AhoCorasick;
use crate::search::api::{Match, SinglePatternSearch};
use crate::search::boyer_moore::{BoyerMoore, BoyerMooreFull};
use crate::search::horspool::Horspool;
use crate::search::kmp::Kmp;
use crate::search::rabin_karp::RabinKarp;
use crate::search::two_way::TwoWay;

/// A single-pattern search input: pattern and haystack.
pub type SingleInput = (&'static [u8], &'static [u8]);

/// A single-pattern golden case whose expected value is the concrete
/// vector of matches produced by `find_all`.
pub type SingleCase = GoldenCase<SingleInput, &'static [Match]>;

/// A multi-pattern Aho-Corasick input: pattern set and haystack.
pub type MultiInput = (&'static [&'static [u8]], &'static [u8]);

/// A multi-pattern Aho-Corasick golden case.
pub type MultiCase = GoldenCase<MultiInput, &'static [Match]>;

// ---- Rabin-Karp cases --------------------------------------------------

/// Golden cases exercising the Rabin-Karp implementation.
pub const GOLDEN_RABIN_KARP: &[SingleCase] = &[
    GoldenCase {
        id: "search/rabin-karp/not-found",
        descriptor: RabinKarp::DESCRIPTOR,
        input: (b"xyz", b"abcabcabc"),
        expected: &[],
        source: GoldenSource::IndependentlyDerived,
        notes: "Pattern absent from haystack: no matches.",
        tags: &["basic", "not-found"],
    },
    GoldenCase {
        id: "search/rabin-karp/hash-rolling-repetition",
        descriptor: RabinKarp::DESCRIPTOR,
        input: (b"aaaa", b"aaaaab"),
        expected: &[
            Match {
                position: 0,
                pattern_index: 0,
            },
            Match {
                position: 1,
                pattern_index: 0,
            },
        ],
        source: GoldenSource::IndependentlyDerived,
        notes: "A repeating-character pattern exercises the rolling-hash update on every window.",
        tags: &["rolling-hash", "overlap"],
    },
    GoldenCase {
        id: "search/rabin-karp/single-match-middle",
        descriptor: RabinKarp::DESCRIPTOR,
        input: (b"needle", b"xxneedleyy"),
        expected: &[Match {
            position: 2,
            pattern_index: 0,
        }],
        source: GoldenSource::IndependentlyDerived,
        notes: "Single match in the middle of the haystack.",
        tags: &["basic"],
    },
    GoldenCase {
        id: "search/rabin-karp/empty-pattern",
        descriptor: RabinKarp::DESCRIPTOR,
        input: (b"", b"abc"),
        expected: &[Match {
            position: 0,
            pattern_index: 0,
        }],
        source: GoldenSource::IndependentlyDerived,
        notes: "Empty pattern matches at position 0 exactly once — documented crate-wide policy.",
        tags: &["boundary", "empty-pattern"],
    },
];

// ---- KMP cases ---------------------------------------------------------

/// Golden cases exercising the KMP implementation.
pub const GOLDEN_KMP: &[SingleCase] = &[
    GoldenCase {
        id: "search/kmp/failure-function-classic",
        descriptor: Kmp::DESCRIPTOR,
        input: (b"abcabd", b"xxabcabdyy"),
        expected: &[Match {
            position: 2,
            pattern_index: 0,
        }],
        source: GoldenSource::IndependentlyDerived,
        notes: "'abcabd' has a non-trivial failure function [0,0,0,1,2,0]; the match at position 2 exercises the continuation branch.",
        tags: &["failure-function"],
    },
    GoldenCase {
        id: "search/kmp/periodic-pattern-overlap",
        descriptor: Kmp::DESCRIPTOR,
        input: (b"abab", b"ababab"),
        expected: &[
            Match {
                position: 0,
                pattern_index: 0,
            },
            Match {
                position: 2,
                pattern_index: 0,
            },
        ],
        source: GoldenSource::IndependentlyDerived,
        notes: "Periodic pattern with overlap — after each match the algorithm falls along the failure link.",
        tags: &["failure-function", "overlap"],
    },
    GoldenCase {
        id: "search/kmp/pattern-equals-haystack",
        descriptor: Kmp::DESCRIPTOR,
        input: (b"abc", b"abc"),
        expected: &[Match {
            position: 0,
            pattern_index: 0,
        }],
        source: GoldenSource::IndependentlyDerived,
        notes: "Pattern and haystack are equal — a boundary case for the end-of-loop match.",
        tags: &["boundary"],
    },
    GoldenCase {
        id: "search/kmp/pattern-longer-than-haystack",
        descriptor: Kmp::DESCRIPTOR,
        input: (b"abcdef", b"abc"),
        expected: &[],
        source: GoldenSource::IndependentlyDerived,
        notes: "Pattern longer than haystack yields no matches.",
        tags: &["boundary"],
    },
];

// ---- Boyer-Moore cases -------------------------------------------------

/// Golden cases exercising the Boyer-Moore (bad-character) implementation.
pub const GOLDEN_BOYER_MOORE: &[SingleCase] = &[
    GoldenCase {
        id: "search/boyer-moore/big-bad-character-shift",
        descriptor: BoyerMoore::DESCRIPTOR,
        input: (b"BCDFGH", b"aaaaaaBCDFGH"),
        expected: &[Match {
            position: 6,
            pattern_index: 0,
        }],
        source: GoldenSource::IndependentlyDerived,
        notes: "Pattern shares no bytes with the leading haystack; the bad-character shift jumps by the full pattern length on every window.",
        tags: &["bad-character", "large-shift"],
    },
    GoldenCase {
        id: "search/boyer-moore/single-match-at-end",
        descriptor: BoyerMoore::DESCRIPTOR,
        input: (b"end", b"the very end"),
        expected: &[Match {
            position: 9,
            pattern_index: 0,
        }],
        source: GoldenSource::IndependentlyDerived,
        notes: "Match at the tail of the haystack; verifies the loop terminates correctly.",
        tags: &["boundary"],
    },
    GoldenCase {
        id: "search/boyer-moore/utf8-multibyte-pattern",
        descriptor: BoyerMoore::DESCRIPTOR,
        // "café" in "café latte" — the é is two bytes in UTF-8 (0xC3 0xA9).
        // Because UTF-8 is prefix-free, byte-level search returns
        // exactly one match.
        input: (
            &[0x63, 0x61, 0x66, 0xC3, 0xA9],
            &[
                0x63, 0x61, 0x66, 0xC3, 0xA9, 0x20, 0x6C, 0x61, 0x74, 0x74, 0x65,
            ],
        ),
        expected: &[Match {
            position: 0,
            pattern_index: 0,
        }],
        source: GoldenSource::IndependentlyDerived,
        notes: "Byte-level search for a multi-byte UTF-8 pattern in a UTF-8 haystack works because UTF-8 is prefix-free — but the caller has to have made the byte-level representation choice deliberately.",
        tags: &["unicode", "boundary"],
    },
    GoldenCase {
        id: "search/boyer-moore/empty-haystack",
        descriptor: BoyerMoore::DESCRIPTOR,
        input: (b"abc", b""),
        expected: &[],
        source: GoldenSource::IndependentlyDerived,
        notes: "Non-empty pattern against an empty haystack yields no matches; boundary check on the outer window loop.",
        tags: &["boundary", "empty-haystack"],
    },
];

// ---- Boyer-Moore (full) cases -----------------------------------------

/// Golden cases exercising the full Boyer-Moore variant (bad-character
/// plus good-suffix). The differential property test pins these outputs
/// against the bad-character-only variant on random inputs; the golden
/// cases here focus on classical textbook patterns that historically
/// exercise the good-suffix path.
pub const GOLDEN_BOYER_MOORE_FULL: &[SingleCase] = &[
    GoldenCase {
        id: "search/boyer-moore-full/textbook-anpanman",
        descriptor: BoyerMooreFull::DESCRIPTOR,
        // Boyer & Moore's own paper uses "ANPANMAN" as a pedagogical
        // pattern that exercises the good-suffix shift after a partial
        // right-to-left match.
        input: (b"ANPANMAN", b"WOWANPANMANMANPANMANANPANMAN"),
        expected: &[
            Match {
                position: 3,
                pattern_index: 0,
            },
            Match {
                position: 12,
                pattern_index: 0,
            },
            Match {
                position: 20,
                pattern_index: 0,
            },
        ],
        source: GoldenSource::IndependentlyDerived,
        notes: "Boyer-Moore's canonical example — a repeating substring that exercises the good-suffix jump on repeated near-matches.",
        tags: &["good-suffix", "canonical"],
    },
    GoldenCase {
        id: "search/boyer-moore-full/periodic-pattern-overlap",
        descriptor: BoyerMooreFull::DESCRIPTOR,
        input: (b"abab", b"ababab"),
        expected: &[
            Match {
                position: 0,
                pattern_index: 0,
            },
            Match {
                position: 2,
                pattern_index: 0,
            },
        ],
        source: GoldenSource::IndependentlyDerived,
        notes: "Periodic pattern with overlap; good-suffix's period-based fallback shift interacts with the +1 overlap step.",
        tags: &["good-suffix", "overlap", "period"],
    },
    GoldenCase {
        id: "search/boyer-moore-full/empty-haystack",
        descriptor: BoyerMooreFull::DESCRIPTOR,
        input: (b"abc", b""),
        expected: &[],
        source: GoldenSource::IndependentlyDerived,
        notes: "Boundary check identical in shape to the bad-character-only variant.",
        tags: &["boundary", "empty-haystack"],
    },
];

// ---- Horspool cases ---------------------------------------------------

/// Golden cases exercising the Horspool implementation. The differential
/// property test pins these outputs against KMP on random inputs; the
/// golden cases here fix Horspool's distinguishing behavior — the shift
/// table always driven by the byte aligned with the rightmost pattern
/// position.
pub const GOLDEN_HORSPOOL: &[SingleCase] = &[
    GoldenCase {
        id: "search/horspool/rightmost-byte-shift",
        descriptor: Horspool::DESCRIPTOR,
        input: (b"BCDFGH", b"aaaaaaBCDFGH"),
        expected: &[Match {
            position: 6,
            pattern_index: 0,
        }],
        source: GoldenSource::IndependentlyDerived,
        notes: "The initial 'a' byte at the window's rightmost position is not in the pattern; Horspool shifts by the full pattern length each time.",
        tags: &["shift-table", "large-shift"],
    },
    GoldenCase {
        id: "search/horspool/overlapping-matches",
        descriptor: Horspool::DESCRIPTOR,
        input: (b"aa", b"aaaa"),
        expected: &[
            Match {
                position: 0,
                pattern_index: 0,
            },
            Match {
                position: 1,
                pattern_index: 0,
            },
            Match {
                position: 2,
                pattern_index: 0,
            },
        ],
        source: GoldenSource::IndependentlyDerived,
        notes: "Overlapping matches of a two-byte pattern; verifies the +1 advance after each match.",
        tags: &["overlap"],
    },
    GoldenCase {
        id: "search/horspool/empty-pattern",
        descriptor: Horspool::DESCRIPTOR,
        input: (b"", b"abc"),
        expected: &[Match {
            position: 0,
            pattern_index: 0,
        }],
        source: GoldenSource::IndependentlyDerived,
        notes: "Empty pattern matches at position 0 exactly once — same as every other single-pattern algorithm in this crate.",
        tags: &["boundary", "empty-pattern"],
    },
];

// ---- Two-way cases ----------------------------------------------------

/// Golden cases exercising the Two-way (Crochemore-Perrin 1991)
/// implementation. The differential property test pins these outputs
/// against KMP on random inputs; the golden cases here focus on
/// factorization shapes — periodic vs non-periodic, textbook worst-case
/// patterns for naive algorithms.
pub const GOLDEN_TWO_WAY: &[SingleCase] = &[
    GoldenCase {
        id: "search/two-way/periodic-pattern",
        descriptor: TwoWay::DESCRIPTOR,
        input: (b"abcabc", b"abcabcabc"),
        expected: &[
            Match {
                position: 0,
                pattern_index: 0,
            },
            Match {
                position: 3,
                pattern_index: 0,
            },
        ],
        source: GoldenSource::IndependentlyDerived,
        notes: "Periodic pattern (period 3, length 6) exercises the memory-based branch of the two-way scan.",
        tags: &["periodic", "critical-factorization"],
    },
    GoldenCase {
        id: "search/two-way/non-periodic-pattern",
        descriptor: TwoWay::DESCRIPTOR,
        input: (b"abcdef", b"xxabcdefyyabcdef"),
        expected: &[
            Match {
                position: 2,
                pattern_index: 0,
            },
            Match {
                position: 10,
                pattern_index: 0,
            },
        ],
        source: GoldenSource::IndependentlyDerived,
        notes: "Non-periodic pattern uses the widened-period, no-memory branch of the two-way scan.",
        tags: &["non-periodic", "critical-factorization"],
    },
    GoldenCase {
        id: "search/two-way/naive-worst-case",
        descriptor: TwoWay::DESCRIPTOR,
        input: (b"aaaaab", b"aaaaaaaaaab"),
        expected: &[Match {
            position: 5,
            pattern_index: 0,
        }],
        source: GoldenSource::IndependentlyDerived,
        notes: "A pattern that is O(n*m) for naive left-to-right search; two-way handles it in linear time.",
        tags: &["worst-case", "linear-guarantee"],
    },
];

// ---- Aho-Corasick cases ------------------------------------------------

const AC_HE_SET: &[&[u8]] = &[b"he", b"she", b"his", b"hers"];
const AC_TWO_OVERLAP: &[&[u8]] = &[b"ab", b"bc"];
const AC_SINGLETON: &[&[u8]] = &[b"abc"];

/// Golden cases exercising the Aho-Corasick multi-pattern automaton.
pub const GOLDEN_AHO_CORASICK: &[MultiCase] = &[
    MultiCase {
        id: "search/aho-corasick/canonical-she-he-his-hers",
        descriptor: AhoCorasick::DESCRIPTOR,
        input: (AC_HE_SET, b"ushers"),
        expected: &[
            Match {
                position: 1,
                pattern_index: 1,
            }, // "she"
            Match {
                position: 2,
                pattern_index: 0,
            }, // "he"
            Match {
                position: 2,
                pattern_index: 3,
            }, // "hers"
        ],
        source: GoldenSource::IndependentlyDerived,
        notes: "Textbook Aho-Corasick example. Confirms overlapping matches across three patterns; two of the matches share position 2.",
        tags: &["canonical", "overlap", "multi-pattern"],
    },
    MultiCase {
        id: "search/aho-corasick/two-adjacent-overlaps",
        descriptor: AhoCorasick::DESCRIPTOR,
        input: (AC_TWO_OVERLAP, b"abc"),
        expected: &[
            Match {
                position: 0,
                pattern_index: 0,
            }, // "ab"
            Match {
                position: 1,
                pattern_index: 1,
            }, // "bc"
        ],
        source: GoldenSource::IndependentlyDerived,
        notes: "Two patterns that overlap on the middle byte both fire in a single pass.",
        tags: &["overlap", "multi-pattern"],
    },
    MultiCase {
        id: "search/aho-corasick/singleton-agrees-with-single-pattern",
        descriptor: AhoCorasick::DESCRIPTOR,
        input: (AC_SINGLETON, b"xxabcxxabc"),
        expected: &[
            Match {
                position: 2,
                pattern_index: 0,
            },
            Match {
                position: 7,
                pattern_index: 0,
            },
        ],
        source: GoldenSource::IndependentlyDerived,
        notes: "A one-pattern Aho-Corasick set should behave identically to a single-pattern algorithm; property tests generalize this.",
        tags: &["single-pattern-equivalence"],
    },
];

fn run_single<A: SinglePatternSearch>(case: &SingleCase) -> Vec<Match> {
    let prepared = A::prepare(case.input.0);
    A::find_all(&prepared, case.input.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_uses_the_correct_descriptor() {
        for c in GOLDEN_RABIN_KARP {
            assert_eq!(
                c.descriptor,
                RabinKarp::DESCRIPTOR,
                "wrong descriptor in {}",
                c.id
            );
        }
        for c in GOLDEN_KMP {
            assert_eq!(
                c.descriptor,
                Kmp::DESCRIPTOR,
                "wrong descriptor in {}",
                c.id
            );
        }
        for c in GOLDEN_BOYER_MOORE {
            assert_eq!(
                c.descriptor,
                BoyerMoore::DESCRIPTOR,
                "wrong descriptor in {}",
                c.id
            );
        }
        for c in GOLDEN_BOYER_MOORE_FULL {
            assert_eq!(
                c.descriptor,
                BoyerMooreFull::DESCRIPTOR,
                "wrong descriptor in {}",
                c.id
            );
        }
        for c in GOLDEN_HORSPOOL {
            assert_eq!(
                c.descriptor,
                Horspool::DESCRIPTOR,
                "wrong descriptor in {}",
                c.id
            );
        }
        for c in GOLDEN_TWO_WAY {
            assert_eq!(
                c.descriptor,
                TwoWay::DESCRIPTOR,
                "wrong descriptor in {}",
                c.id
            );
        }
        for c in GOLDEN_AHO_CORASICK {
            assert_eq!(
                c.descriptor,
                AhoCorasick::DESCRIPTOR,
                "wrong descriptor in {}",
                c.id
            );
        }
    }

    #[test]
    fn every_rabin_karp_case_matches_algorithm() {
        for c in GOLDEN_RABIN_KARP {
            let observed = run_single::<RabinKarp>(c);
            assert_eq!(&observed[..], c.expected, "case {} disagreed", c.id);
        }
    }

    #[test]
    fn every_kmp_case_matches_algorithm() {
        for c in GOLDEN_KMP {
            let observed = run_single::<Kmp>(c);
            assert_eq!(&observed[..], c.expected, "case {} disagreed", c.id);
        }
    }

    #[test]
    fn every_boyer_moore_case_matches_algorithm() {
        for c in GOLDEN_BOYER_MOORE {
            let observed = run_single::<BoyerMoore>(c);
            assert_eq!(&observed[..], c.expected, "case {} disagreed", c.id);
        }
    }

    #[test]
    fn every_boyer_moore_full_case_matches_algorithm() {
        for c in GOLDEN_BOYER_MOORE_FULL {
            let observed = run_single::<BoyerMooreFull>(c);
            assert_eq!(&observed[..], c.expected, "case {} disagreed", c.id);
        }
    }

    #[test]
    fn every_horspool_case_matches_algorithm() {
        for c in GOLDEN_HORSPOOL {
            let observed = run_single::<Horspool>(c);
            assert_eq!(&observed[..], c.expected, "case {} disagreed", c.id);
        }
    }

    #[test]
    fn every_two_way_case_matches_algorithm() {
        for c in GOLDEN_TWO_WAY {
            let observed = run_single::<TwoWay>(c);
            assert_eq!(&observed[..], c.expected, "case {} disagreed", c.id);
        }
    }

    #[test]
    fn every_aho_corasick_case_matches_algorithm() {
        for c in GOLDEN_AHO_CORASICK {
            let ac = AhoCorasick::build(c.input.0);
            let observed = ac.find_all(c.input.1);
            assert_eq!(&observed[..], c.expected, "case {} disagreed", c.id);
        }
    }

    #[test]
    fn corpus_meets_minimum_size() {
        // Spec asks for at least 15 golden cases across the algorithms;
        // the current corpus significantly exceeds that.
        let total = GOLDEN_RABIN_KARP.len()
            + GOLDEN_KMP.len()
            + GOLDEN_BOYER_MOORE.len()
            + GOLDEN_BOYER_MOORE_FULL.len()
            + GOLDEN_HORSPOOL.len()
            + GOLDEN_TWO_WAY.len()
            + GOLDEN_AHO_CORASICK.len();
        assert!(
            total >= 15,
            "expected at least 15 golden cases across the crate, got {total}"
        );
    }

    #[test]
    fn every_case_has_a_unique_id() {
        let mut ids: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
        ids.extend(GOLDEN_RABIN_KARP.iter().map(|c| c.id));
        ids.extend(GOLDEN_KMP.iter().map(|c| c.id));
        ids.extend(GOLDEN_BOYER_MOORE.iter().map(|c| c.id));
        ids.extend(GOLDEN_BOYER_MOORE_FULL.iter().map(|c| c.id));
        ids.extend(GOLDEN_HORSPOOL.iter().map(|c| c.id));
        ids.extend(GOLDEN_TWO_WAY.iter().map(|c| c.id));
        ids.extend(GOLDEN_AHO_CORASICK.iter().map(|c| c.id));
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate golden-case id detected");
    }
}
