//! Persian-Buckwalter transliteration reference input/output pairs.
//!
//! Coverage over all **32 letters of the Persian alphabet** — the four
//! Persian-only additions (پ چ ژ گ), the two Persian-specific variants
//! (ک ی), and the 26 shared Arabic letters. Each pair is a real
//! Persian word chosen to exercise a specific letter; the expected
//! Buckwalter output is derived character-by-character from the
//! mapping table documented in [`stringcheese_fa::phonetic`].

extern crate alloc;

use stringcheese_fa::PersianBuckwalter;

/// 32 reference pairs — one per Persian letter.
///
/// Each pair covers a specific letter with a real Persian word. The
/// expected output uses the Persian-Buckwalter mapping (see the
/// [`stringcheese_fa::phonetic`] module docs), which differs from
/// classical Arabic-Buckwalter for ghain (`G` instead of `g`) and adds
/// codes for the four Persian consonants.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // The four Persian additions.
    // -----------------------------------------------------------------
    ("پارسی", "pArsy"), // پ (peh) — "Persian"
    ("چرا", "crA"),     // چ (cheh) — "why"
    ("ژرف", "Jrf"),     // ژ (jeh)  — "deep"
    ("گل", "gl"),       // گ (gaf)  — "flower"
    // -----------------------------------------------------------------
    // The two Persian-specific variants.
    // -----------------------------------------------------------------
    ("کتاب", "ktAb"), // ک (Persian kaf) — "book"
    ("علی", "Ely"),   // ی (Persian yeh) — "Ali"
    // -----------------------------------------------------------------
    // The 26 shared Arabic letters (or 24; several are covered by the
    // pairs above and are listed here for their primary letter).
    // -----------------------------------------------------------------
    ("ابر", "Abr"),   // ا (alef) — "cloud"
    ("باد", "bAd"),   // ب (be) — "wind"
    ("توت", "twt"),   // ت (te) — "berry"
    ("ثابت", "vAbt"), // ث (se) — "constant"
    ("جان", "jAn"),   // ج (jim) — "soul"
    ("حال", "HAl"),   // ح (he-jimi) — "state"
    ("خوب", "xwb"),   // خ (khe) — "good"
    ("در", "dr"),     // د (dal) — "in"
    ("ذره", "*rh"),   // ذ (zal) — "particle"
    ("راه", "rAh"),   // ر (re) — "road"
    ("زن", "zn"),     // ز (ze) — "woman"
    ("سر", "sr"),     // س (sin) — "head"
    ("شب", "$b"),     // ش (shin) — "night"
    ("صبر", "Sbr"),   // ص (sad) — "patience"
    ("ضرب", "Drb"),   // ض (zad) — "beat"
    ("طرف", "Trf"),   // ط (ta) — "side"
    ("ظرف", "Zrf"),   // ظ (za) — "container"
    ("عمر", "Emr"),   // ع (ain) — "life"
    ("مغز", "mGz"),   // غ (ghain, Persian-adapted G) — "brain"
    ("فرد", "frd"),   // ف (fe) — "person"
    ("قند", "qnd"),   // ق (qaf) — "sugar"
    ("لب", "lb"),     // ل (lam) — "lip"
    ("ماه", "mAh"),   // م (mim) — "moon"
    ("نان", "nAn"),   // ن (nun) — "bread"
    ("و", "w"),       // و (vav) — "and"
    ("هوا", "hwA"),   // ه (he) — "air"
    // -----------------------------------------------------------------
    // Persian yeh / Arabic yeh both encode to `y`.
    // -----------------------------------------------------------------
    ("علي", "Ely"), // ي (Arabic yeh) — same encoding as علی
    // -----------------------------------------------------------------
    // Arabic kaf → same code as Persian kaf.
    // -----------------------------------------------------------------
    ("كتاب", "ktAb"), // ك (Arabic kaf) — same encoding as کتاب
];

#[test]
fn persian_buckwalter_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = PersianBuckwalter.encode(input);
        if got != expected {
            failures.push(alloc::format!(
                "  PersianBuckwalter({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} Persian-Buckwalter reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for "15+ pairs" covering all 32 letters.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

/// Round-trip: `PersianBuckwalter.inverse(PersianBuckwalter.encode(x)) == x`
/// for every pair whose input uses **Persian canonical** yeh / kaf.
/// Pairs whose input uses Arabic yeh / Arabic kaf round-trip through
/// the *Persian* form — that is the documented normalization behaviour
/// of the inverse.
#[test]
fn persian_canonical_pairs_round_trip() {
    // Only Persian-canonical inputs: skip the Arabic-yeh and
    // Arabic-kaf pairs, which are deliberately normalized on
    // round-trip.
    for &(input, _expected) in PAIRS {
        if input.contains('\u{064A}') || input.contains('\u{0643}') {
            continue;
        }
        let encoded = PersianBuckwalter.encode(input);
        let back = PersianBuckwalter.inverse(&encoded);
        assert_eq!(
            back, input,
            "round-trip failed on {input:?}: encode → {encoded:?} → inverse → {back:?}"
        );
    }
}
