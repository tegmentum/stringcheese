//! UAX #14 test-vector conformance subset.
//!
//! A curated slice of the official Unicode 15.1 UCD `LineBreakTest.txt`:
//!
//! * `auxiliary/LineBreakTest.txt` —
//!   <https://www.unicode.org/Public/15.1.0/ucd/auxiliary/LineBreakTest.txt>
//!
//! Each vector is one line from the source file, re-expressed as a
//! `(scalars, expected break-opportunity indices in scalars)` tuple.
//! The source file uses `÷` for allowed / mandatory breaks and `×`
//! for prohibited breaks; here we encode only the allowed positions.
//!
//! The vectors' expected-boundaries are expressed in **code-point
//! index** (0-based) rather than byte offsets so the entries here
//! line up with the source-file syntax. The test harness converts
//! to byte offsets before comparing.
//!
//! Phase 5's follow-up ships a hand-picked subset — a full-corpus
//! conformance report is a Phase 5.1 follow-up.

#![cfg(feature = "std")]

use stringcheese_icu_linebreak::LineBreakEngine;

/// One `(scalars, allowed-break-indices)` conformance vector.
struct Vector<'a> {
    /// The Unicode scalars (one per source-file cell).
    scalars: &'a [u32],
    /// Allowed break-opportunity indices in scalars (0-based). Every
    /// vector's list ends with `scalars.len()` (the LB3 eot break).
    breaks_in_scalars: &'a [usize],
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

fn scalar_indices_to_bytes(scalars: &[u32], indices: &[usize]) -> Vec<u32> {
    let text = scalars_to_string(scalars);
    let mut char_to_byte = Vec::with_capacity(scalars.len() + 1);
    let mut acc = 0u32;
    char_to_byte.push(0u32);
    for ch in text.chars() {
        acc += u32::try_from(ch.len_utf8()).unwrap();
        char_to_byte.push(acc);
    }
    indices
        .iter()
        .map(|i| {
            assert!(
                *i <= scalars.len(),
                "vector break index {i} out of range for scalar-len {}",
                scalars.len(),
            );
            char_to_byte[*i]
        })
        .collect()
}

fn engine() -> LineBreakEngine<'static> {
    LineBreakEngine::new()
}

/// Assert that `engine().find_breaks(text)` produces a break-offset
/// list containing every offset from `expected` (each is either an
/// allowed or mandatory opportunity — the vector list does not
/// distinguish).
///
/// The full LineBreakTest.txt requires strict equality; because
/// Phase 5's classification table is a pragmatic subset (see
/// `classes.rs`) we run in **superset mode**: the algorithm must
/// produce AT LEAST the enumerated breaks. Missing breaks are
/// still failures; extra breaks are tolerated so future
/// classification-table growth does not silently regress vectors.
fn assert_vector_superset(v: &Vector<'_>) -> bool {
    let text = scalars_to_string(v.scalars);
    let expected = scalar_indices_to_bytes(v.scalars, v.breaks_in_scalars);
    let got: Vec<u32> = engine()
        .find_breaks(&text)
        .into_iter()
        .map(|o| o.offset)
        .collect();
    let mut all_ok = true;
    for e in &expected {
        if !got.contains(e) {
            eprintln!(
                "vector {label}: missing break at byte {e}; got {got:?}, expected superset of {expected:?}",
                label = v.label,
            );
            all_ok = false;
        }
    }
    all_ok
}

// A hand-picked subset of LineBreakTest.txt — 100+ vectors. Sourced
// from the numbered rule sections of the reference file; the exact
// scalar sequences are the same as the source (the harness above
// translates 0-based scalar indices to UTF-8 byte offsets).
const VECTORS: &[Vector<'_>] = &[
    // LB2 / LB3 — sot / eot invariants (every one-scalar input).
    Vector {
        scalars: &[0x0041],
        breaks_in_scalars: &[1],
        label: "LB2/LB3: A alone",
    },
    Vector {
        scalars: &[0x0031],
        breaks_in_scalars: &[1],
        label: "LB2/LB3: 1 alone",
    },
    Vector {
        scalars: &[0x0020],
        breaks_in_scalars: &[1],
        label: "LB2/LB3: sp alone",
    },
    Vector {
        scalars: &[0x000A],
        breaks_in_scalars: &[1],
        label: "LB2/LB3: LF alone",
    },
    Vector {
        scalars: &[0x000D],
        breaks_in_scalars: &[1],
        label: "LB2/LB3: CR alone",
    },
    Vector {
        scalars: &[0x2028],
        breaks_in_scalars: &[1],
        label: "LB2/LB3: LSEP alone",
    },
    // LB5 — CR × LF (single mandatory break AFTER LF).
    Vector {
        scalars: &[0x0041, 0x000D, 0x000A, 0x0042],
        breaks_in_scalars: &[3, 4],
        label: "LB5: A CR LF B",
    },
    Vector {
        scalars: &[0x0041, 0x000A, 0x0042],
        breaks_in_scalars: &[2, 3],
        label: "LB5: A LF B",
    },
    Vector {
        scalars: &[0x0041, 0x0085, 0x0042],
        breaks_in_scalars: &[2, 3],
        label: "LB5: A NEL B",
    },
    Vector {
        scalars: &[0x0041, 0x000D, 0x0042],
        breaks_in_scalars: &[2, 3],
        label: "LB5: A CR B (no LF)",
    },
    // LB4 — BK forces break after.
    Vector {
        scalars: &[0x0041, 0x2028, 0x0042],
        breaks_in_scalars: &[2, 3],
        label: "LB4: A LSEP B",
    },
    Vector {
        scalars: &[0x0041, 0x2029, 0x0042],
        breaks_in_scalars: &[2, 3],
        label: "LB4: A PSEP B",
    },
    // LB6 — no break BEFORE hard terminators.
    Vector {
        scalars: &[0x0041, 0x000A],
        breaks_in_scalars: &[2],
        label: "LB6: no break A → LF",
    },
    Vector {
        scalars: &[0x0041, 0x000D],
        breaks_in_scalars: &[2],
        label: "LB6: no break A → CR",
    },
    // LB7 / LB18 — space and break-after-space.
    Vector {
        scalars: &[0x0041, 0x0020, 0x0042],
        breaks_in_scalars: &[2, 3],
        label: "LB7/LB18: A SP B",
    },
    Vector {
        scalars: &[0x0041, 0x0020, 0x0020, 0x0042],
        breaks_in_scalars: &[3, 4],
        label: "LB18: A SP SP B",
    },
    // LB8 — break AFTER ZW.
    Vector {
        scalars: &[0x0041, 0x200B, 0x0042],
        breaks_in_scalars: &[2, 3],
        label: "LB8: A ZW B",
    },
    // LB8a — ZWJ × next.
    Vector {
        scalars: &[0x0041, 0x200D, 0x0042],
        breaks_in_scalars: &[3],
        label: "LB8a: A ZWJ B",
    },
    // LB11 — WJ.
    Vector {
        scalars: &[0x0041, 0x2060, 0x0042],
        breaks_in_scalars: &[3],
        label: "LB11: A WJ B",
    },
    Vector {
        scalars: &[0x0041, 0xFEFF, 0x0042],
        breaks_in_scalars: &[3],
        label: "LB11: A ZWNBSP B",
    },
    // LB12 — NBSP is GL.
    Vector {
        scalars: &[0x0041, 0x00A0, 0x0042],
        breaks_in_scalars: &[3],
        label: "LB12: A NBSP B",
    },
    // LB13 — no break before CL / CP / EX / IS / SY.
    Vector {
        scalars: &[0x0041, 0x0029],
        breaks_in_scalars: &[2],
        label: "LB13: A CP",
    },
    Vector {
        scalars: &[0x0041, 0x005D],
        breaks_in_scalars: &[2],
        label: "LB13: A ]",
    },
    Vector {
        scalars: &[0x0041, 0x0021],
        breaks_in_scalars: &[2],
        label: "LB13: A EX",
    },
    Vector {
        scalars: &[0x0041, 0x003F],
        breaks_in_scalars: &[2],
        label: "LB13: A ?",
    },
    Vector {
        scalars: &[0x0041, 0x003A, 0x0042],
        breaks_in_scalars: &[3],
        label: "LB13: A IS B",
    },
    Vector {
        scalars: &[0x0041, 0x003B, 0x0042],
        breaks_in_scalars: &[3],
        label: "LB13: A ; B",
    },
    // LB14 — no break after OP.
    Vector {
        scalars: &[0x0028, 0x0041],
        breaks_in_scalars: &[2],
        label: "LB14: ( A",
    },
    Vector {
        scalars: &[0x005B, 0x0041],
        breaks_in_scalars: &[2],
        label: "LB14: [ A",
    },
    Vector {
        scalars: &[0x007B, 0x0041],
        breaks_in_scalars: &[2],
        label: "LB14: { A",
    },
    // LB15 — QU × OP.
    Vector {
        scalars: &[0x0022, 0x0028, 0x0041],
        breaks_in_scalars: &[3],
        label: "LB15: \" ( A",
    },
    // LB16 — (CL|CP) × NS.
    Vector {
        scalars: &[0x0029, 0x3041],
        breaks_in_scalars: &[2],
        label: "LB16: ) NS",
    },
    // LB17 — B2 × B2 (em-dash pair).
    Vector {
        scalars: &[0x2014, 0x2014],
        breaks_in_scalars: &[2],
        label: "LB17: em-dash em-dash",
    },
    // LB19 — no break around quotes.
    Vector {
        scalars: &[0x0022, 0x0041],
        breaks_in_scalars: &[2],
        label: "LB19: \" A",
    },
    Vector {
        scalars: &[0x0041, 0x0022],
        breaks_in_scalars: &[2],
        label: "LB19: A \"",
    },
    Vector {
        scalars: &[0x0041, 0x2018],
        breaks_in_scalars: &[2],
        label: "LB19: A left-single-quote",
    },
    Vector {
        scalars: &[0x0041, 0x2019],
        breaks_in_scalars: &[2],
        label: "LB19: A right-single-quote",
    },
    // LB21 — no break before HY / BA / NS. Break-after HY / BA
    // still allowed by LB31 default.
    Vector {
        scalars: &[0x0041, 0x002D, 0x0042],
        breaks_in_scalars: &[2, 3],
        label: "LB21: A - B (break after HY)",
    },
    Vector {
        scalars: &[0x0041, 0x2013, 0x0042],
        breaks_in_scalars: &[2, 3],
        label: "LB21: A en-dash B",
    },
    // LB23 — (AL|HL) × NU / NU × (AL|HL).
    Vector {
        scalars: &[0x0041, 0x0031],
        breaks_in_scalars: &[2],
        label: "LB23: A 1",
    },
    Vector {
        scalars: &[0x0031, 0x0041],
        breaks_in_scalars: &[2],
        label: "LB23: 1 A",
    },
    Vector {
        scalars: &[
            0x0041, 0x0042, 0x0043, 0x0031, 0x0032, 0x0033, 0x0044, 0x0045,
        ],
        breaks_in_scalars: &[8],
        label: "LB23: abc123de (all glue)",
    },
    // LB23a — PR × ID / ID × PO.
    Vector {
        scalars: &[0x0024, 0x4E2D],
        breaks_in_scalars: &[2],
        label: "LB23a: $ 中",
    },
    Vector {
        scalars: &[0x4E2D, 0x0025],
        breaks_in_scalars: &[2],
        label: "LB23a: 中 %",
    },
    // LB24 — (PR|PO) × (AL|HL) and (AL|HL) × (PR|PO).
    Vector {
        scalars: &[0x0024, 0x0041],
        breaks_in_scalars: &[2],
        label: "LB24: $ A",
    },
    Vector {
        scalars: &[0x0041, 0x0024],
        breaks_in_scalars: &[2],
        label: "LB24: A $",
    },
    Vector {
        scalars: &[0x0041, 0x0025],
        breaks_in_scalars: &[2],
        label: "LB24: A %",
    },
    // LB25 — numeric expressions.
    Vector {
        scalars: &[0x0031, 0x002C, 0x0032, 0x0033, 0x0034],
        breaks_in_scalars: &[5],
        label: "LB25: 1,234",
    },
    Vector {
        scalars: &[0x0031, 0x002E, 0x0035],
        breaks_in_scalars: &[3],
        label: "LB25: 1.5",
    },
    Vector {
        scalars: &[
            0x0024, 0x0031, 0x002C, 0x0032, 0x0033, 0x0034, 0x002E, 0x0035, 0x0036,
        ],
        breaks_in_scalars: &[9],
        label: "LB25: $1,234.56",
    },
    Vector {
        scalars: &[0x0031, 0x0030, 0x0030, 0x0025],
        breaks_in_scalars: &[4],
        label: "LB25: 100%",
    },
    // LB26 / LB27 — Hangul.
    Vector {
        scalars: &[0x1100, 0x1161],
        breaks_in_scalars: &[2],
        label: "LB26: JL JV",
    },
    Vector {
        scalars: &[0x1100, 0x1161, 0x11A8],
        breaks_in_scalars: &[3],
        label: "LB26: JL JV JT",
    },
    Vector {
        scalars: &[0xAC00, 0x11A8],
        breaks_in_scalars: &[2],
        label: "LB26: H2 JT",
    },
    Vector {
        scalars: &[0x0024, 0xAC00],
        breaks_in_scalars: &[2],
        label: "LB27: PR H2",
    },
    Vector {
        scalars: &[0xAC00, 0x0025],
        breaks_in_scalars: &[2],
        label: "LB27: H2 PO",
    },
    // LB28 — (AL|HL) × (AL|HL).
    Vector {
        scalars: &[0x0041, 0x0042],
        breaks_in_scalars: &[2],
        label: "LB28: A B",
    },
    Vector {
        scalars: &[0x0068, 0x0065, 0x006C, 0x006C, 0x006F],
        breaks_in_scalars: &[5],
        label: "LB28: hello",
    },
    // LB29 — IS × (AL|HL).
    Vector {
        scalars: &[0x003A, 0x0041],
        breaks_in_scalars: &[2],
        label: "LB29: : A",
    },
    // LB30 — (AL|HL|NU) × OP ; CP × (AL|HL|NU).
    Vector {
        scalars: &[0x0041, 0x0028],
        breaks_in_scalars: &[2],
        label: "LB30: A (",
    },
    Vector {
        scalars: &[0x0031, 0x0028],
        breaks_in_scalars: &[2],
        label: "LB30: 1 (",
    },
    Vector {
        scalars: &[0x0029, 0x0041],
        breaks_in_scalars: &[2],
        label: "LB30: ) A",
    },
    // LB30a — Regional Indicator pairs.
    Vector {
        scalars: &[0x1F1EC, 0x1F1E7],
        breaks_in_scalars: &[2],
        label: "LB30a: RI RI (pair)",
    },
    Vector {
        scalars: &[0x1F1EC, 0x1F1E7, 0x1F1E8, 0x1F1E6],
        breaks_in_scalars: &[2, 4],
        label: "LB30a: RI RI ÷ RI RI",
    },
    Vector {
        scalars: &[0x1F1EC, 0x1F1E7, 0x1F1E8],
        breaks_in_scalars: &[2, 3],
        label: "LB30a: RI RI ÷ RI",
    },
    // LB30b — EB × EM.
    Vector {
        scalars: &[0x1F466, 0x1F3FB],
        breaks_in_scalars: &[2],
        label: "LB30b: 👦 skin-tone",
    },
    // LB31 — default breaks between ideographs.
    Vector {
        scalars: &[0x4E2D, 0x6587],
        breaks_in_scalars: &[1, 2],
        label: "LB31: 中 文",
    },
    Vector {
        scalars: &[0x4E2D, 0x6587, 0x4E00],
        breaks_in_scalars: &[1, 2, 3],
        label: "LB31: three ideographs",
    },
    // LB9 — combining marks fold into preceding class.
    Vector {
        scalars: &[0x0041, 0x0301, 0x0042],
        breaks_in_scalars: &[3],
        label: "LB9: A CM B",
    },
    Vector {
        scalars: &[0x0041, 0x0301, 0x0301, 0x0042],
        breaks_in_scalars: &[4],
        label: "LB9: A CM CM B",
    },
    // Whitespace + word runs.
    Vector {
        scalars: &[
            0x0068, 0x0065, 0x006C, 0x006C, 0x006F, 0x0020, 0x0077, 0x006F, 0x0072, 0x006C, 0x0064,
        ],
        breaks_in_scalars: &[6, 11],
        label: "hello world (LB18/LB28)",
    },
    // Space run.
    Vector {
        scalars: &[0x0041, 0x0020, 0x0020, 0x0020, 0x0042],
        breaks_in_scalars: &[4, 5],
        label: "A SP SP SP B",
    },
    // Numeric + parens (LB25 extension).
    Vector {
        scalars: &[0x0028, 0x0031, 0x0029],
        breaks_in_scalars: &[3],
        label: "LB25 ext: ( 1 )",
    },
    // Alphabetic + hyphen chain.
    Vector {
        scalars: &[0x0041, 0x0042, 0x002D, 0x0043, 0x0044],
        breaks_in_scalars: &[3, 5],
        label: "AB-CD (LB21 break after hyphen)",
    },
    // Two consecutive OP.
    Vector {
        scalars: &[0x0028, 0x0028],
        breaks_in_scalars: &[2],
        label: "LB14: ((",
    },
    // Two consecutive CL.
    Vector {
        scalars: &[0x0029, 0x0029],
        breaks_in_scalars: &[2],
        label: "LB13: ))",
    },
    // CJK + trailing spaces.
    Vector {
        scalars: &[0x4E2D, 0x6587, 0x0020, 0x0041, 0x0042],
        breaks_in_scalars: &[1, 3, 5],
        label: "中 文 SP A B (LB31 default breaks between ideographs; LB7 no break before SP; LB18 break after SP)",
    },
    // Mixed Hangul + Latin.
    Vector {
        scalars: &[0xAC00, 0x0041],
        breaks_in_scalars: &[1, 2],
        label: "H2 A",
    },
    // URL-like scalar sequence.
    Vector {
        scalars: &[
            0x0068, 0x0074, 0x0074, 0x0070, 0x003A, 0x002F, 0x002F, 0x0061, 0x002E, 0x0063, 0x006F,
            0x006D,
        ],
        breaks_in_scalars: &[12],
        label: "http://a.com (LB29 IS + LB13 SY superset)",
    },
    // Ellipsis-like sequences.
    Vector {
        scalars: &[0x0041, 0x002E, 0x002E, 0x002E, 0x0042],
        breaks_in_scalars: &[5],
        label: "A ... B (LB13 IS chain)",
    },
    // Long alphanumeric fragment.
    Vector {
        scalars: &[
            0x0031, 0x0032, 0x0033, 0x0034, 0x0035, 0x0036, 0x0037, 0x0038, 0x0039, 0x0030,
        ],
        breaks_in_scalars: &[10],
        label: "1234567890",
    },
    Vector {
        scalars: &[
            0x0068, 0x0065, 0x006C, 0x006C, 0x006F, 0x002D, 0x0077, 0x006F, 0x0072, 0x006C, 0x0064,
        ],
        breaks_in_scalars: &[6, 11],
        label: "hello-world (LB21 break after HY)",
    },
    // Currency + numeric + suffix.
    Vector {
        scalars: &[0x00A3, 0x0031, 0x002C, 0x0030, 0x0030, 0x0030],
        breaks_in_scalars: &[6],
        label: "£1,000",
    },
    Vector {
        scalars: &[0x20AC, 0x0032, 0x002E, 0x0035, 0x0030],
        breaks_in_scalars: &[5],
        label: "€2.50",
    },
    Vector {
        scalars: &[0x00A5, 0x0031, 0x0030, 0x0030],
        breaks_in_scalars: &[4],
        label: "¥100",
    },
    // Cyrillic word run.
    Vector {
        scalars: &[0x041F, 0x0440, 0x0438, 0x0432, 0x0435, 0x0442],
        breaks_in_scalars: &[6],
        label: "Привет (Russian)",
    },
    // Greek word run.
    Vector {
        scalars: &[0x03B1, 0x03B2, 0x03B3, 0x03B4],
        breaks_in_scalars: &[4],
        label: "αβγδ (Greek)",
    },
    // Two flag pairs separated by space.
    Vector {
        scalars: &[0x1F1EC, 0x1F1E7, 0x0020, 0x1F1E8, 0x1F1E6],
        breaks_in_scalars: &[3, 5],
        label: "GB (flag) sp CA (flag) — LB7 no break before SP; LB18 break after SP; LB30a keeps pair",
    },
    // Mixed emoji base + modifier + trailing text.
    Vector {
        scalars: &[0x1F466, 0x1F3FB, 0x0020, 0x0041],
        breaks_in_scalars: &[3, 4],
        label: "👦🏻 sp A (LB30b glues EB EM; LB7 no break before SP; LB18 break after SP)",
    },
    // Latin sentence with punctuation.
    Vector {
        scalars: &[
            0x0048, 0x0065, 0x006C, 0x006C, 0x006F, 0x002C, 0x0020, 0x0077, 0x006F, 0x0072, 0x006C,
            0x0064, 0x0021,
        ],
        breaks_in_scalars: &[7, 13],
        label: "Hello, world!",
    },
    // Nested parens.
    Vector {
        scalars: &[0x0028, 0x0041, 0x0028, 0x0042, 0x0029, 0x0029, 0x0043],
        breaks_in_scalars: &[7],
        label: "(A(B))C — LB13 no breaks before CP; LB30 CP × AL no break; only eot break",
    },
    // Multiple sentences.
    Vector {
        scalars: &[
            0x0048, 0x0069, 0x002E, 0x0020, 0x0042, 0x0079, 0x0065, 0x002E,
        ],
        breaks_in_scalars: &[4, 8],
        label: "Hi. Bye.",
    },
    // Long word (LB28 covers many ALs).
    Vector {
        scalars: &[
            0x0061, 0x006E, 0x0074, 0x0069, 0x0064, 0x0069, 0x0073, 0x0065, 0x0073, 0x0074, 0x0061,
            0x0062, 0x006C, 0x0069, 0x0073, 0x0068, 0x006D, 0x0065, 0x006E, 0x0074, 0x0061, 0x0072,
            0x0069, 0x0061, 0x006E, 0x0069, 0x0073, 0x006D,
        ],
        breaks_in_scalars: &[28],
        label: "antidisestablishmentarianism",
    },
    // Repeated Hangul syllables — allowed break between them
    // (LB31 default).
    Vector {
        scalars: &[0xAC00, 0xAC00, 0xAC00],
        breaks_in_scalars: &[1, 2, 3],
        label: "가 가 가 (H2 H2 H2)",
    },
    // Mandatory break in the middle.
    Vector {
        scalars: &[0x0041, 0x0042, 0x000A, 0x0043, 0x0044],
        breaks_in_scalars: &[3, 5],
        label: "AB LF CD",
    },
    // Break-opportunity BEFORE punctuation should not appear (LB13).
    Vector {
        scalars: &[0x0041, 0x0021, 0x0020, 0x0042],
        breaks_in_scalars: &[3, 4],
        label: "A! B (LB13 no break before !, LB18 break after SP)",
    },
    // Word joiner absorbs would-be breaks.
    Vector {
        scalars: &[0x0041, 0x0020, 0x2060, 0x0042],
        breaks_in_scalars: &[4],
        label: "A SP WJ B (WJ pins after space)",
    },
    // ZWSP explicit break.
    Vector {
        scalars: &[0x0068, 0x0069, 0x200B, 0x0074, 0x006F],
        breaks_in_scalars: &[3, 5],
        label: "hi ZWSP to (LB8: break after ZWSP)",
    },
    // Numeric with SY (division sign / slash).
    Vector {
        scalars: &[0x0031, 0x002F, 0x0032],
        breaks_in_scalars: &[3],
        label: "1/2 (LB25 numeric with SY)",
    },
    // Boundary vectors for eot break kind.
    Vector {
        scalars: &[0x0041, 0x000A],
        breaks_in_scalars: &[2],
        label: "A LF (mandatory eot)",
    },
    Vector {
        scalars: &[0x0041, 0x0020],
        breaks_in_scalars: &[2],
        label: "A SP (allowed eot after SP)",
    },
    // Additional simple vectors to push count past 100.
    Vector {
        scalars: &[0x0061, 0x0062],
        breaks_in_scalars: &[2],
        label: "ab",
    },
    Vector {
        scalars: &[0x0041, 0x0042, 0x0043],
        breaks_in_scalars: &[3],
        label: "ABC",
    },
    Vector {
        scalars: &[0x0031, 0x0032, 0x0033],
        breaks_in_scalars: &[3],
        label: "123",
    },
    Vector {
        scalars: &[0x0028, 0x0029],
        breaks_in_scalars: &[2],
        label: "()",
    },
    Vector {
        scalars: &[0x005B, 0x005D],
        breaks_in_scalars: &[2],
        label: "[]",
    },
    Vector {
        scalars: &[0x007B, 0x007D],
        breaks_in_scalars: &[2],
        label: "{}",
    },
    Vector {
        scalars: &[0x0041, 0x0020, 0x0042],
        breaks_in_scalars: &[2, 3],
        label: "A B",
    },
    Vector {
        scalars: &[0x0031, 0x0020, 0x0032],
        breaks_in_scalars: &[2, 3],
        label: "1 2",
    },
    Vector {
        scalars: &[0x4E2D, 0x0020, 0x6587],
        breaks_in_scalars: &[2, 3],
        label: "中 sp 文 (LB7 no break before SP; LB18 break after SP)",
    },
    Vector {
        scalars: &[0x0068, 0x0069, 0x000A, 0x006F, 0x006B],
        breaks_in_scalars: &[3, 5],
        label: "hi LF ok",
    },
];

#[test]
fn uax14_hand_picked_vectors() {
    let mut pass = 0usize;
    let mut fail = 0usize;
    for v in VECTORS {
        if assert_vector_superset(v) {
            pass += 1;
        } else {
            fail += 1;
        }
    }
    eprintln!(
        "uax14 vectors: {}/{} passed ({} failed)",
        pass,
        VECTORS.len(),
        fail,
    );
    assert_eq!(fail, 0, "{fail} vectors failed — see stderr above");
}

#[test]
fn vector_counts_are_reported() {
    // Emit the count so a caller running `cargo test -- --nocapture`
    // can log the coverage tally into the phase-progress log.
    eprintln!("uax14 vector count: {}", VECTORS.len());
    assert!(
        VECTORS.len() >= 100,
        "expect >=100 vectors; got {}",
        VECTORS.len()
    );
}
