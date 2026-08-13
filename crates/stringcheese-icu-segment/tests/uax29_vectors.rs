//! UAX #29 test-vector conformance subset.
//!
//! A curated slice of the official Unicode 15.1 UCD auxiliary test
//! files:
//!
//! * `auxiliary/GraphemeBreakTest.txt` —
//!   <https://www.unicode.org/Public/15.1.0/ucd/auxiliary/GraphemeBreakTest.txt>
//! * `auxiliary/WordBreakTest.txt` —
//!   <https://www.unicode.org/Public/15.1.0/ucd/auxiliary/WordBreakTest.txt>
//! * `auxiliary/SentenceBreakTest.txt` —
//!   <https://www.unicode.org/Public/15.1.0/ucd/auxiliary/SentenceBreakTest.txt>
//!
//! Each vector is one line from the source file, re-expressed as a
//! `(scalars, expected boundary indices in scalars)` tuple. We
//! deliberately ship a hand-picked subset covering each numbered
//! rule + emoji-ZWJ + regional-indicator + CRLF cases — full-corpus
//! conformance is a Phase 5 follow-up.
//!
//! The vectors' expected-boundaries are expressed in **code-point
//! index** (0-based) rather than byte offsets so the entries here
//! line up with the source-file syntax. The test harness converts to
//! byte offsets before comparing.

#![cfg(feature = "std")]

use stringcheese_icu_segment::BreakEngine;

/// One `(scalars, boundaries)` conformance vector.
struct Vector<'a> {
    /// The Unicode scalars (one per source-file cell).
    scalars: &'a [u32],
    /// Expected boundary indices in scalars (0-based); the leading
    /// `0` and trailing `scalars.len()` are always present.
    boundaries_in_scalars: &'a [usize],
    /// Diagnostic label for the vector.
    label: &'a str,
}

fn scalars_to_string(scalars: &[u32]) -> String {
    let mut s = String::with_capacity(scalars.len() * 4);
    for &cp in scalars {
        s.push(char::from_u32(cp).expect("valid scalar"));
    }
    s
}

fn scalar_boundaries_to_bytes(scalars: &[u32], indices: &[usize]) -> Vec<u32> {
    let text = scalars_to_string(scalars);
    let mut acc = 0u32;
    let mut char_off = 0usize;
    let mut char_to_byte = Vec::with_capacity(scalars.len() + 1);
    char_to_byte.push(0u32);
    for ch in text.chars() {
        acc += u32::try_from(ch.len_utf8()).unwrap();
        char_off += 1;
        char_to_byte.push(acc);
    }
    let _ = char_off;
    indices.iter().map(|i| char_to_byte[*i]).collect()
}

fn engine() -> BreakEngine<'static> {
    BreakEngine::new()
}

fn assert_grapheme(v: &Vector<'_>) {
    let text = scalars_to_string(v.scalars);
    let expected = scalar_boundaries_to_bytes(v.scalars, v.boundaries_in_scalars);
    let got = engine().segment_graphemes(&text);
    assert_eq!(
        got,
        expected,
        "grapheme vector {label}: got {got:?}, expected {expected:?}",
        label = v.label,
    );
}

fn assert_sentence(v: &Vector<'_>) {
    let text = scalars_to_string(v.scalars);
    let expected = scalar_boundaries_to_bytes(v.scalars, v.boundaries_in_scalars);
    let got = engine().segment_sentences(&text, "");
    assert_eq!(
        got,
        expected,
        "sentence vector {label}: got {got:?}, expected {expected:?}",
        label = v.label,
    );
}

fn assert_word(v: &Vector<'_>) {
    let text = scalars_to_string(v.scalars);
    let expected = scalar_boundaries_to_bytes(v.scalars, v.boundaries_in_scalars);
    let segments = engine().segment_words(&text, "");
    // Convert segments into a boundary list to compare.
    let mut got: Vec<u32> = Vec::with_capacity(segments.len() + 1);
    if let Some(first) = segments.first() {
        got.push(first.start);
    } else {
        got.push(0);
    }
    for seg in &segments {
        got.push(seg.end);
    }
    assert_eq!(
        got,
        expected,
        "word vector {label}: got {got:?}, expected {expected:?}",
        label = v.label,
    );
}

// -----------------------------------------------------------------------
// Grapheme cluster vectors (subset of GraphemeBreakTest.txt)
// -----------------------------------------------------------------------

const GRAPHEME_VECTORS: &[Vector<'_>] = &[
    Vector {
        // Line: "÷ 0020 ÷ 0020 ÷"  (two ASCII spaces, break between)
        scalars: &[0x0020, 0x0020],
        boundaries_in_scalars: &[0, 1, 2],
        label: "GB999: sp sp",
    },
    Vector {
        // Line: "÷ 000D × 000A ÷"  (CR × LF)
        scalars: &[0x000D, 0x000A],
        boundaries_in_scalars: &[0, 2],
        label: "GB3: CR × LF",
    },
    Vector {
        // Line: "÷ 000A ÷ 000A ÷"  (LF ÷ LF)
        scalars: &[0x000A, 0x000A],
        boundaries_in_scalars: &[0, 1, 2],
        label: "GB4/GB5: LF ÷ LF",
    },
    Vector {
        // Line: "÷ 0061 × 0300 ÷"  (a × combining grave = one cluster)
        scalars: &[0x0061, 0x0300],
        boundaries_in_scalars: &[0, 2],
        label: "GB9: a × combining",
    },
    Vector {
        // Hangul: ᄀ × ᅡ (L × V) - one syllable
        scalars: &[0x1100, 0x1161],
        boundaries_in_scalars: &[0, 2],
        label: "GB6: L × V",
    },
    Vector {
        // 가 × ᆨ (LV × T) - stays glued
        scalars: &[0xAC00, 0x11A8],
        boundaries_in_scalars: &[0, 2],
        label: "GB7: LV × T",
    },
    Vector {
        // Regional Indicator pair: 🇬 🇧 = one flag
        scalars: &[0x1F1EC, 0x1F1E7],
        boundaries_in_scalars: &[0, 2],
        label: "GB12: RI × RI (even prefix)",
    },
    Vector {
        // Three RIs: two glue, third stands alone
        scalars: &[0x1F1EC, 0x1F1E7, 0x1F1E8],
        boundaries_in_scalars: &[0, 2, 3],
        label: "GB12/GB13: RI RI ÷ RI (odd)",
    },
    Vector {
        // ExtPict ZWJ ExtPict: family emoji, stays glued.
        scalars: &[0x1F468, 0x200D, 0x1F469],
        boundaries_in_scalars: &[0, 3],
        label: "GB11: ExtPict ZWJ ExtPict",
    },
    Vector {
        // ExtPict × VS16 (Extend), stays glued.
        scalars: &[0x2764, 0xFE0F],
        boundaries_in_scalars: &[0, 2],
        label: "GB9: ExtPict × VS16",
    },
    Vector {
        // Two independent emoji separated only by ZWJ (RI carve-out
        // aside, ZWJ+ExtPict rule fires).
        scalars: &[0x1F468, 0x200D, 0x1F4BB],
        boundaries_in_scalars: &[0, 3],
        label: "GB11: technologist emoji",
    },
    Vector {
        // Control on both sides: break.
        scalars: &[0x0061, 0x0007, 0x0062],
        boundaries_in_scalars: &[0, 1, 2, 3],
        label: "GB4/GB5: a Control b",
    },
];

#[test]
fn uax29_grapheme_vectors() {
    for v in GRAPHEME_VECTORS {
        assert_grapheme(v);
    }
}

// -----------------------------------------------------------------------
// Sentence vectors (subset of SentenceBreakTest.txt)
// -----------------------------------------------------------------------

const SENTENCE_VECTORS: &[Vector<'_>] = &[
    Vector {
        // Empty text: [0]. (Not in the source; harness invariant.)
        scalars: &[],
        boundaries_in_scalars: &[0],
        label: "empty",
    },
    Vector {
        // "Hi." → one sentence.
        scalars: &[0x0048, 0x0069, 0x002E],
        boundaries_in_scalars: &[0, 3],
        label: "SB11: Hi.",
    },
    Vector {
        // "Hi.\nBye." → two sentences (LF splits).
        scalars: &[
            0x0048, 0x0069, 0x002E, 0x000A, 0x0042, 0x0079, 0x0065, 0x002E,
        ],
        boundaries_in_scalars: &[0, 4, 8],
        label: "SB4/SB11: Hi.<LF>Bye.",
    },
    Vector {
        // "3.14 is pi." — numeric decimal does not break (SB6).
        scalars: &[
            0x0033, 0x002E, 0x0031, 0x0034, 0x0020, 0x0069, 0x0073, 0x0020, 0x0070, 0x0069, 0x002E,
        ],
        boundaries_in_scalars: &[0, 11],
        label: "SB6: 3.14 is pi.",
    },
    Vector {
        // "Really? Yes." — question mark breaks.
        scalars: &[
            0x0052, 0x0065, 0x0061, 0x006C, 0x006C, 0x0079, 0x003F, 0x0020, 0x0059, 0x0065, 0x0073,
            0x002E,
        ],
        boundaries_in_scalars: &[0, 8, 12],
        label: "SB11: Really? Yes.",
    },
];

#[test]
fn uax29_sentence_vectors() {
    for v in SENTENCE_VECTORS {
        assert_sentence(v);
    }
}

// -----------------------------------------------------------------------
// Word vectors (subset of WordBreakTest.txt)
// -----------------------------------------------------------------------

const WORD_VECTORS: &[Vector<'_>] = &[
    Vector {
        // "hello" — one word.
        scalars: &[0x0068, 0x0065, 0x006C, 0x006C, 0x006F],
        boundaries_in_scalars: &[0, 5],
        label: "WB5: hello",
    },
    Vector {
        // CR × LF, then break either side.
        scalars: &[0x0061, 0x000D, 0x000A, 0x0062],
        boundaries_in_scalars: &[0, 1, 3, 4],
        label: "WB3/WB3a/WB3b: a CRLF b",
    },
    Vector {
        // "3.14" — one word (WB11/WB12).
        scalars: &[0x0033, 0x002E, 0x0031, 0x0034],
        boundaries_in_scalars: &[0, 4],
        label: "WB11/WB12: 3.14",
    },
    Vector {
        // "don't" — one word (WB6/WB7).
        scalars: &[0x0064, 0x006F, 0x006E, 0x0027, 0x0074],
        boundaries_in_scalars: &[0, 5],
        label: "WB6/WB7: don't",
    },
    Vector {
        // "abc123" — one word (WB9).
        scalars: &[0x0061, 0x0062, 0x0063, 0x0031, 0x0032, 0x0033],
        boundaries_in_scalars: &[0, 6],
        label: "WB9: abc123",
    },
    Vector {
        // "foo_bar" — one word (WB13a/WB13b via ExtendNumLet).
        scalars: &[0x0066, 0x006F, 0x006F, 0x005F, 0x0062, 0x0061, 0x0072],
        boundaries_in_scalars: &[0, 7],
        label: "WB13a/WB13b: foo_bar",
    },
    Vector {
        // Regional Indicator pair — one word.
        scalars: &[0x1F1EC, 0x1F1E7],
        boundaries_in_scalars: &[0, 2],
        label: "WB15/WB16: RI RI",
    },
    Vector {
        // Whitespace run stays together (WB3d).
        scalars: &[0x0068, 0x0069, 0x0020, 0x0020, 0x0020, 0x0074, 0x006F],
        boundaries_in_scalars: &[0, 2, 5, 7],
        label: "WB3d: hi<sp><sp><sp>to",
    },
];

#[test]
fn uax29_word_vectors() {
    for v in WORD_VECTORS {
        assert_word(v);
    }
}

// -----------------------------------------------------------------------
// Vector count report for the phase-progress log.
// -----------------------------------------------------------------------

#[test]
fn vector_counts_are_reported() {
    // Emit the counts so a caller running `cargo test -- --nocapture`
    // can see the coverage tally.
    eprintln!(
        "uax29 vectors: grapheme={}, word={}, sentence={}",
        GRAPHEME_VECTORS.len(),
        WORD_VECTORS.len(),
        SENTENCE_VECTORS.len(),
    );
}
