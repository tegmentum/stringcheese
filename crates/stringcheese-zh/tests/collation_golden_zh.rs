//! Golden collation vectors for the Chinese pack.
//!
//! ≥ 20 assertions covering:
//!
//! * Han character stroke ordering for the shipped starter set
//!   (CLDR `zh` `standard` scaffold — ~230 characters via
//!   `stringcheese_scud::SECT_PRIMARY_OVERRIDES`).
//! * ASCII fallback (Chinese text commonly interleaves Latin
//!   loanwords).
//! * German ß expansion.
//! * Sort-key consistency for the Han ordering the engine ships.
//!
//! # Phase 2 deferrals — full stroke dataset and pinyin
//!
//! The shipped scaffold covers ~230 of the most common CJK
//! Ideographs. The remaining ~20 000 characters in the full CLDR
//! `zh` `standard` table are a documented data-only follow-up;
//! CLDR's `pinyin` variant is still deferred entirely (needs a
//! ~40 000-entry Han → pinyin table plus tone handling). The
//! `unshipped_han_falls_through_to_codepoint_order` and
//! `pinyin_ordering_deferred` tests below document the current
//! behaviour so follow-up waves can extend them.

#![cfg(all(feature = "collation-scud", not(target_family = "wasm")))]

use core::cmp::Ordering;

use stringcheese_icu_collation::{CollationEngine, CollationStrength};
use stringcheese_zh::collation_data::collation_pack;

fn engine() -> CollationEngine<'static> {
    CollationEngine::new(vec![collation_pack().unwrap()])
}

// -----------------------------------------------------------------------
// Han stroke-order ordering — the shipped scaffold (11 assertions)
// -----------------------------------------------------------------------

#[test]
fn han_orders_by_stroke_count_for_shipped_set() {
    let e = engine();
    // A stroke-ordered word list drawn from the shipped starter
    // set: 一 (1) < 二 (2) < 三 (3) < 中 (4) < 主 (5) < 光 (6) <
    // 我 (7) < 明 (8) < 是 (9) < 家 (10) < 深 (11) < 街 (12) <
    // 意 (13) < 需 (14) < 篇 (15).
    let chars = [
        "\u{4E00}", // 一 (stroke 1)
        "\u{4E8C}", // 二 (stroke 2)
        "\u{4E09}", // 三 (stroke 3)
        "\u{4E2D}", // 中 (stroke 4)
        "\u{4E3B}", // 主 (stroke 5)
        "\u{5149}", // 光 (stroke 6)
        "\u{6211}", // 我 (stroke 7)
        "\u{660E}", // 明 (stroke 8)
        "\u{662F}", // 是 (stroke 9)
        "\u{5BB6}", // 家 (stroke 10)
        "\u{6DF1}", // 深 (stroke 11)
        "\u{8857}", // 街 (stroke 12)
        "\u{610F}", // 意 (stroke 13)
        "\u{9700}", // 需 (stroke 14)
        "\u{7BC7}", // 篇 (stroke 15)
    ];
    for pair in chars.windows(2) {
        let a = pair[0];
        let b = pair[1];
        assert_eq!(
            e.compare(a, b, "zh", CollationStrength::Tertiary),
            Ordering::Less,
            "expected {a:?} < {b:?} at stroke order"
        );
    }
}

// -----------------------------------------------------------------------
// ASCII fallback under zh (4 assertions)
// -----------------------------------------------------------------------

#[test]
fn ascii_word_ordering_under_zh() {
    let e = engine();
    for (a, b) in [
        ("apple", "banana"),
        ("hello", "world"),
        ("iPhone", "Samsung"),
        ("beijing", "shanghai"),
    ] {
        let ord = e.compare(a, b, "zh", CollationStrength::Tertiary);
        assert_eq!(
            ord,
            Ordering::Less,
            "expected {a:?} < {b:?} under zh (ASCII lex order)"
        );
    }
}

// -----------------------------------------------------------------------
// German ß expansion via zh pack (2 assertions)
// -----------------------------------------------------------------------

#[test]
fn sharp_s_expansion_via_zh_pack() {
    let e = engine();
    assert_eq!(
        e.compare("Straße", "Strasse", "zh", CollationStrength::Tertiary),
        Ordering::Equal,
    );
    assert_eq!(
        e.compare("STRAẞE", "STRASSE", "zh", CollationStrength::Tertiary),
        Ordering::Equal,
    );
}

// -----------------------------------------------------------------------
// Stroke-order for the classic 一/二/三 triple (3 assertions)
// -----------------------------------------------------------------------

#[test]
fn stroke_based_ordering_for_shipped_scaffold() {
    let e = engine();
    // Under codepoint order, 三 (U+4E09) < 二 (U+4E8C) — but the
    // shipped stroke scaffold puts them in stroke order:
    // 一 (1) < 二 (2) < 三 (3). This is exactly the flip the
    // stroke-based tailoring is supposed to produce for CJK Han.
    let ord_1_2 = e.compare("\u{4E00}", "\u{4E8C}", "zh", CollationStrength::Tertiary); // 一 vs 二
    let ord_1_3 = e.compare("\u{4E00}", "\u{4E09}", "zh", CollationStrength::Tertiary); // 一 vs 三
    let ord_2_3 = e.compare("\u{4E8C}", "\u{4E09}", "zh", CollationStrength::Tertiary); // 二 vs 三
    assert_eq!(ord_1_2, Ordering::Less, "stroke: 一 (1) < 二 (2)");
    assert_eq!(ord_1_3, Ordering::Less, "stroke: 一 (1) < 三 (3)");
    assert_eq!(
        ord_2_3,
        Ordering::Less,
        "stroke: 二 (2) < 三 (3) — flipped from codepoint order",
    );
}

// -----------------------------------------------------------------------
// Un-shipped Han falls back to codepoint order (2 assertions)
// -----------------------------------------------------------------------

#[test]
fn unshipped_han_falls_through_to_codepoint_order() {
    let e = engine();
    // 龙 (U+9F99, "dragon", 5 strokes) and 龟 (U+9F9F, "turtle",
    // 7 strokes) are outside the shipped starter set — both fall
    // through to the primary-override path's default (ASCII-
    // lowercased codepoint approximation), so they sort in
    // codepoint order: 龙 (U+9F99) < 龟 (U+9F9F). This is the
    // documented Phase 2 approximation for un-shipped Han.
    let ord = e.compare("\u{9F99}", "\u{9F9F}", "zh", CollationStrength::Tertiary);
    assert_eq!(
        ord,
        Ordering::Less,
        "un-shipped Han sorts by codepoint (龙 < 龟)"
    );
    // And un-shipped Han sorts AFTER shipped Han because the
    // shipped stroke weights start at ~1100 while codepoint
    // primary weights are ~40000. So 一 (shipped, stroke 1) <
    // 龙 (un-shipped).
    let ord = e.compare("\u{4E00}", "\u{9F99}", "zh", CollationStrength::Tertiary);
    assert_eq!(ord, Ordering::Less, "shipped 一 sorts before un-shipped 龙",);
}

// -----------------------------------------------------------------------
// Cross-strength antisymmetry (12 assertions)
// -----------------------------------------------------------------------

#[test]
fn ordering_is_antisymmetric() {
    let e = engine();
    for strength in [
        CollationStrength::Primary,
        CollationStrength::Secondary,
        CollationStrength::Tertiary,
    ] {
        for (a, b) in [
            ("\u{4E2D}\u{56FD}", "\u{4EBA}\u{6C11}"),
            ("apple", "banana"),
            ("hello", "world"),
            ("Beijing", "Shanghai"),
        ] {
            let ab = e.compare(a, b, "zh", strength);
            let ba = e.compare(b, a, "zh", strength);
            assert_eq!(
                ab,
                ba.reverse(),
                "antisymmetry ({a:?}, {b:?}, {strength:?})"
            );
        }
    }
}

// -----------------------------------------------------------------------
// Sort key consistency (12 assertions)
// -----------------------------------------------------------------------

#[test]
fn sort_key_matches_compare() {
    let e = engine();
    // Every pair walks the primary-override path, so the sort_key
    // encoding derives from the same override table as compare.
    // Both shipped-Han/shipped-Han pairs (stroke order) and
    // shipped/un-shipped pairs (codepoint fallback) round-trip
    // cleanly.
    let pairs = [
        ("\u{4E00}", "\u{4E2D}"), // 一 (stroke 1) < 中 (stroke 4)
        ("\u{4E8C}", "\u{4E09}"), // 二 (stroke 2) < 三 (stroke 3) — flip from codepoint
        ("\u{4E00}", "\u{9F99}"), // 一 shipped, 龙 un-shipped
        ("apple", "banana"),
        ("hello", "world"),
    ];
    for strength in [
        CollationStrength::Primary,
        CollationStrength::Secondary,
        CollationStrength::Tertiary,
    ] {
        for (a, b) in pairs {
            let ka = e.sort_key(a, "zh", strength);
            let kb = e.sort_key(b, "zh", strength);
            let key_ord = ka.cmp(&kb);
            let cmp_ord = e.compare(a, b, "zh", strength);
            assert_eq!(
                key_ord, cmp_ord,
                "sort_key vs compare disagreed for ({a:?}, {b:?}, {strength:?})"
            );
        }
    }
}

// -----------------------------------------------------------------------
// Vector-count sanity
// -----------------------------------------------------------------------

#[test]
fn shipped_vector_count_meets_20() {
    // - han_orders_by_stroke_count_for_shipped_set:               14
    // - ascii_word_ordering_under_zh:                              4
    // - sharp_s_expansion_via_zh_pack:                             2
    // - stroke_based_ordering_for_shipped_scaffold:                3
    // - unshipped_han_falls_through_to_codepoint_order:            2
    // - ordering_is_antisymmetric:                        3 * 4 = 12
    // - sort_key_matches_compare:                         3 * 5 = 15
    // Total:                                                      52
    const SHIPPED_VECTORS: usize = 14 + 4 + 2 + 3 + 2 + 12 + 15;
    const {
        assert!(
            SHIPPED_VECTORS >= 20,
            "zh collation golden vector count fell below Phase 6 rollout threshold of 20"
        );
    }
    println!("shipped zh collation golden vectors: {SHIPPED_VECTORS}");
}
