//! Simplified ISO 259 transliteration reference input/output pairs.
//!
//! 28 well-known Hebrew words and names, hand-derived character-by-
//! character from the mapping table documented in
//! [`stringcheese_he::phonetic`]. The pairs cover all 22 base letters
//! plus all 5 final forms (each final form folds to its base form's
//! ASCII code in the phonetic key).

extern crate alloc;

use stringcheese_he::Iso259;

/// 28 reference pairs. Each pair is `(hebrew, expected_ascii)`.
const PAIRS: &[(&str, &str)] = &[
    // -----------------------------------------------------------------
    // Everyday words — cover most base consonants.
    // -----------------------------------------------------------------
    ("אבא", "'b'"),   // אב(א) — א→', ב→b, א→'
    ("בית", "byt"),   // house — ב→b, י→y, ת→t
    ("ספר", "spr"),   // book — ס→s, פ→p, ר→r
    ("תודה", "twdh"), // thanks — ת→t, ו→w, ד→d, ה→h
    ("שלום", "$lwm"), // shalom — ש→$, ל→l, ו→w, ם→m (final mem → m)
    ("אני", "'ny"),   // I — א→', נ→n, י→y
    ("אתה", "'th"),   // you-m — א→', ת→t, ה→h
    ("יש", "y$"),     // there is — י→y, ש→$
    ("גדול", "gdwl"), // big — ג→g, ד→d, ו→w, ל→l
    ("קטן", "qTn"),   // small — ק→q, ט→T, ן→n (final nun → n)
    ("אחד", "'Hd"),   // one — א→', ח→H, ד→d
    ("ילד", "yld"),   // boy — י→y, ל→l, ד→d
    ("מים", "mym"),   // water — מ→m, י→y, ם→m (final mem → m)
    ("חמש", "Hm$"),   // five — ח→H, מ→m, ש→$
    ("ארץ", "'rc"),   // land — א→', ר→r, ץ→c (final tsadi → c)
    ("עץ", "`c"),     // tree — ע→`, ץ→c (final tsadi → c)
    ("כתב", "ktb"),   // write/wrote — כ→k, ת→t, ב→b
    ("שופט", "$wpT"), // judge — ש→$, ו→w, פ→p, ט→T
    ("זכר", "zkr"),   // male/remember — ז→z, כ→k, ר→r
    ("חלון", "Hlwn"), // window — ח→H, ל→l, ו→w, ן→n (final nun → n)
    ("כלב", "klb"),   // dog — כ→k, ל→l, ב→b
    ("סוף", "swp"),   // end — ס→s, ו→w, ף→p (final pe → p)
    ("מלך", "mlk"),   // king — מ→m, ל→l, ך→k (final kaf → k)
    ("גן", "gn"),     // garden — ג→g, ן→n (final nun → n)
    ("עולם", "`wlm"), // world — ע→`, ו→w, ל→l, ם→m (final mem → m)
    ("שנה", "$nh"),   // year — ש→$, נ→n, ה→h
    ("צל", "cl"),     // shadow — צ→c, ל→l
    ("צדק", "cdq"),   // justice — צ→c, ד→d, ק→q
];

#[test]
fn iso259_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in PAIRS {
        let got = Iso259.encode(input);
        if got != expected {
            failures.push(alloc::format!(
                "  Iso259({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} ISO 259 reference pair(s) disagreed:\n{}",
        failures.len(),
        PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn reference_pair_count_meets_the_task_floor() {
    // The task spec asks for "at least 15 pairs". Verify we're above
    // that floor.
    assert!(
        PAIRS.len() >= 15,
        "reference pair count {} is below the 15-pair floor",
        PAIRS.len()
    );
}

/// Verify that every one of the 22 base letters is exercised by at
/// least one reference pair.
#[test]
fn every_base_letter_is_exercised() {
    let mut seen = [false; 22];
    for &(input, _) in PAIRS {
        for c in input.chars() {
            if let Some(idx) = base_letter_index(c) {
                seen[idx] = true;
            }
        }
    }
    let missing: alloc::vec::Vec<char> = (0..22)
        .filter(|&i| !seen[i])
        .map(|i| {
            char::from_u32(0x05D0 + base_letter_codepoint_offset(i))
                .expect("in-range hebrew letter")
        })
        .collect();
    assert!(
        missing.is_empty(),
        "base letters not covered by reference pairs: {missing:?}"
    );
}

/// Verify that every one of the 5 final letter forms is exercised by
/// at least one reference pair.
#[test]
fn every_final_form_is_exercised() {
    const FINALS: [char; 5] = [
        '\u{05DA}', // ך
        '\u{05DD}', // ם
        '\u{05DF}', // ן
        '\u{05E3}', // ף
        '\u{05E5}', // ץ
    ];
    let mut seen = [false; 5];
    for &(input, _) in PAIRS {
        for c in input.chars() {
            for (i, &f) in FINALS.iter().enumerate() {
                if c == f {
                    seen[i] = true;
                }
            }
        }
    }
    let missing: alloc::vec::Vec<char> = (0..5).filter(|&i| !seen[i]).map(|i| FINALS[i]).collect();
    assert!(
        missing.is_empty(),
        "final letter forms not covered by reference pairs: {missing:?}"
    );
}

/// Round-trip test: for pairs that use no final letter forms, the
/// round-trip through inverse recovers the original word. Pairs with
/// final forms round-trip to the folded (base-form) version.
#[test]
fn round_trip_preserves_words_without_final_forms() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, _) in PAIRS {
        if input.chars().any(has_final_form) {
            continue;
        }
        let encoded = Iso259.encode(input);
        let back = Iso259.inverse(&encoded);
        if back != input {
            failures.push(alloc::format!(
                "  inverse(encode({input:?})) = {back:?} (expected {input:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} reference pair(s) failed round-trip:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Base-letter codepoint offsets from U+05D0. Skips the five final-form
/// slots (0x05DA, 0x05DD, 0x05DF, 0x05E3, 0x05E5).
const fn base_letter_codepoint_offset(i: usize) -> u32 {
    // The 22 base-letter offsets from U+05D0, in order.
    const OFFSETS: [u32; 22] = [
        0x00, // א U+05D0
        0x01, // ב
        0x02, // ג
        0x03, // ד
        0x04, // ה
        0x05, // ו
        0x06, // ז
        0x07, // ח
        0x08, // ט
        0x09, // י
        0x0B, // כ
        0x0C, // ל
        0x0E, // מ
        0x10, // נ
        0x11, // ס
        0x12, // ע
        0x14, // פ
        0x16, // צ
        0x17, // ק
        0x18, // ר
        0x19, // ש
        0x1A, // ת
    ];
    OFFSETS[i]
}

fn base_letter_index(c: char) -> Option<usize> {
    let cp = c as u32;
    if !(0x05D0..=0x05EA).contains(&cp) {
        return None;
    }
    (0..22).find(|&i| 0x05D0 + base_letter_codepoint_offset(i) == cp)
}

const fn has_final_form(c: char) -> bool {
    matches!(
        c,
        '\u{05DA}' | '\u{05DD}' | '\u{05DF}' | '\u{05E3}' | '\u{05E5}'
    )
}
