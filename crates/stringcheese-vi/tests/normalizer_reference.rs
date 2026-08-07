//! Vietnamese normalizer reference input/output pairs.
//!
//! Exercises the three normalizer flags — NFC canonicalization (on
//! by default), tone-mark stripping (opt-in), and full-diacritic
//! stripping (opt-in) — with hand-verified expected outputs.

extern crate alloc;

use stringcheese_vi::normalize::{VietnameseNormalizer, normalize};

/// NFC-round-trip reference pairs (input, expected NFC output).
///
/// Each entry pairs an NFD-decomposed form with its NFC precomposed
/// form. The default normalizer applies NFC canonicalization only.
const NFC_PAIRS: &[(&str, &str)] = &[
    // `e` + combining circumflex U+0302 + combining dot-below U+0323
    //   → precomposed `ệ` U+1EC7.
    ("e\u{0302}\u{0323}", "ệ"),
    // `a` + combining breve U+0306 + combining grave U+0300 → `ằ`.
    ("a\u{0306}\u{0300}", "ằ"),
    // `a` + combining circumflex + combining acute → `ấ`.
    ("a\u{0302}\u{0301}", "ấ"),
    // `o` + combining circumflex + combining hook-above → `ổ`.
    ("o\u{0302}\u{0309}", "ổ"),
    // `u` + combining horn + combining dot-below → `ự`.
    ("u\u{031B}\u{0323}", "ự"),
    // Already-NFC input passes through unchanged.
    ("Học sinh đọc sách", "Học sinh đọc sách"),
    ("và", "và"),
];

/// Tone-mark-strip reference pairs (input, expected output with tone
/// marks stripped but letter modifiers preserved).
const TONE_STRIP_PAIRS: &[(&str, &str)] = &[
    // All five tone marks on `a` fold to bare `a`.
    ("à", "a"),
    ("á", "a"),
    ("ả", "a"),
    ("ã", "a"),
    ("ạ", "a"),
    // Six tone variants of `ban` all collapse.
    ("ban", "ban"),
    ("bàn", "ban"),
    ("bán", "ban"),
    ("bản", "ban"),
    ("bãn", "ban"),
    ("bạn", "ban"),
    // Letter modifiers survive.
    ("ăn", "ăn"),
    ("đường", "đương"), // tone stripped from `ờ`, `ư` and `ng` untouched
    // Modifier + tone: strip tone, keep modifier.
    ("ằ", "ă"),
    ("ầ", "â"),
    ("ộ", "ô"),
    ("ự", "ư"),
    ("ệ", "ê"),
];

/// Full-diacritic-strip reference pairs (input, expected plain-ASCII
/// output).
const FULL_STRIP_PAIRS: &[(&str, &str)] = &[
    // Tone marks stripped.
    ("à", "a"),
    ("á", "a"),
    // Letter modifiers stripped.
    ("ă", "a"),
    ("â", "a"),
    ("đ", "d"),
    ("ê", "e"),
    ("ô", "o"),
    ("ơ", "o"),
    ("ư", "u"),
    // Uppercase Đ folds.
    ("Đ", "D"),
    // Stacked diacritics.
    ("ằ", "a"),
    ("ự", "u"),
    ("ệ", "e"),
    // Full sentence.
    ("Học sinh đọc sách.", "Hoc sinh doc sach."),
    ("Việt Nam", "Viet Nam"),
    ("Nguyễn", "Nguyen"),
];

#[test]
fn nfc_default_matches_reference_pairs() {
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in NFC_PAIRS {
        let got = normalize(input);
        if got != expected {
            failures.push(alloc::format!(
                "  normalize({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} NFC reference pair(s) disagreed:\n{}",
        failures.len(),
        NFC_PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn tone_strip_matches_reference_pairs() {
    let n = VietnameseNormalizer::builder().with_strip_tone_marks(true);
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in TONE_STRIP_PAIRS {
        let got = n.normalize(input);
        if got != expected {
            failures.push(alloc::format!(
                "  tone_strip({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} tone-strip reference pair(s) disagreed:\n{}",
        failures.len(),
        TONE_STRIP_PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn full_strip_matches_reference_pairs() {
    let n = VietnameseNormalizer::builder().with_strip_all_diacritics(true);
    let mut failures = alloc::vec::Vec::new();
    for &(input, expected) in FULL_STRIP_PAIRS {
        let got = n.normalize(input);
        if got != expected {
            failures.push(alloc::format!(
                "  full_strip({input:?}) = {got:?} (expected {expected:?})"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} full-strip reference pair(s) disagreed:\n{}",
        failures.len(),
        FULL_STRIP_PAIRS.len(),
        failures.join("\n")
    );
}

#[test]
fn combined_reference_pair_count_meets_the_task_floor() {
    // The task spec asks for at least 15 pairs total across the
    // three normalizer configurations.
    let total = NFC_PAIRS.len() + TONE_STRIP_PAIRS.len() + FULL_STRIP_PAIRS.len();
    assert!(
        total >= 15,
        "combined reference pair count {total} is below the 15-pair floor",
    );
}
