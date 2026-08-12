//! Golden plural-classification vectors for the English pack.
//!
//! At least 50 vectors per locale covering all cardinal + ordinal
//! category boundaries. Phase 3 of the WIT-i18n design commits to
//! ~50 per-locale vectors sourced from CLDR 44.1 plural rules.
//!
//! Golden vectors are hand-derived from the CLDR
//! [`plurals.xml`](https://github.com/unicode-org/cldr/blob/release-44-1/common/supplemental/plurals.xml)
//! reference file. A regression that shifts an entry in the pack
//! will fail here loudly rather than surface as a subtle
//! integration bug downstream.

#![cfg(all(feature = "plural-scud", not(target_family = "wasm")))]

use stringcheese_en::plural_data::plural_pack;
use stringcheese_icu_plural::{PluralCategory, PluralEngine};

fn engine() -> PluralEngine<'static> {
    PluralEngine::new(vec![plural_pack().unwrap()])
}

// -----------------------------------------------------------------------
// Cardinals — 30 vectors
// -----------------------------------------------------------------------

#[test]
fn cardinal_one_is_singular_integer() {
    let e = engine();
    for n in [1.0, -1.0] {
        assert_eq!(
            e.plural_cardinal(n, "en"),
            PluralCategory::One,
            "n={n} should be One"
        );
    }
}

#[test]
fn cardinal_other_covers_zero_two_and_larger_integers() {
    let e = engine();
    for n in [0.0, 2.0, 3.0, 5.0, 10.0, 42.0, 100.0, 1000.0, 1_000_000.0] {
        assert_eq!(
            e.plural_cardinal(n, "en"),
            PluralCategory::Other,
            "n={n} should be Other"
        );
    }
}

#[test]
fn cardinal_other_covers_fractional() {
    let e = engine();
    // Even 1.5 or 1.0000001 count as Other in English cardinals
    // because v > 0 breaks the `i = 1 and v = 0` rule.
    for n in [0.5, 1.5, 1.000_000_1, 2.7, 10.1] {
        assert_eq!(
            e.plural_cardinal(n, "en"),
            PluralCategory::Other,
            "n={n} should be Other"
        );
    }
}

#[test]
fn cardinal_negative_absolute_value() {
    let e = engine();
    // CLDR uses absolute value for n, so -1 → One.
    assert_eq!(e.plural_cardinal(-1.0, "en"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(-2.5, "en"), PluralCategory::Other);
}

// -----------------------------------------------------------------------
// Ordinals — 30 vectors
// -----------------------------------------------------------------------

#[test]
fn ordinal_one_1st_21st_101st() {
    let e = engine();
    for n in [
        1.0, 21.0, 31.0, 41.0, 51.0, 61.0, 71.0, 81.0, 91.0, 101.0, 121.0, 1001.0,
    ] {
        assert_eq!(
            e.plural_ordinal(n, "en"),
            PluralCategory::One,
            "n={n} should be One"
        );
    }
}

#[test]
fn ordinal_two_2nd_22nd_102nd() {
    let e = engine();
    for n in [2.0, 22.0, 32.0, 42.0, 52.0, 102.0, 122.0] {
        assert_eq!(
            e.plural_ordinal(n, "en"),
            PluralCategory::Two,
            "n={n} should be Two"
        );
    }
}

#[test]
fn ordinal_few_3rd_23rd_103rd() {
    let e = engine();
    for n in [3.0, 23.0, 33.0, 43.0, 53.0, 103.0, 123.0] {
        assert_eq!(
            e.plural_ordinal(n, "en"),
            PluralCategory::Few,
            "n={n} should be Few"
        );
    }
}

#[test]
fn ordinal_teens_are_other() {
    let e = engine();
    // 11th, 12th, 13th, 111th, 112th, 113th all fall to Other.
    for n in [11.0, 12.0, 13.0, 111.0, 112.0, 113.0] {
        assert_eq!(
            e.plural_ordinal(n, "en"),
            PluralCategory::Other,
            "n={n} should be Other (teens exception)"
        );
    }
}

#[test]
fn ordinal_other_covers_4th_through_10th_20th() {
    let e = engine();
    for n in [
        0.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 20.0, 30.0, 40.0, 100.0,
    ] {
        assert_eq!(
            e.plural_ordinal(n, "en"),
            PluralCategory::Other,
            "n={n} should be Other"
        );
    }
}

// -----------------------------------------------------------------------
// Fallback / miscellaneous
// -----------------------------------------------------------------------

#[test]
fn en_us_falls_back_to_en() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "en-US"), PluralCategory::One);
    assert_eq!(e.plural_ordinal(2.0, "en-US"), PluralCategory::Two);
}

#[test]
fn unknown_locale_returns_other() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "xx"), PluralCategory::Other);
    assert_eq!(e.plural_ordinal(2.0, "xx"), PluralCategory::Other);
}
