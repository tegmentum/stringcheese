//! Golden plural-classification vectors for the Chinese pack.
//!
//! Chinese lacks grammatical number; CLDR ships `other` only for
//! both cardinals and ordinals. The pack still needs a place to
//! stamp CLDR version provenance and to make
//! [`PluralEngine::supports("zh")`](stringcheese_icu_plural::PluralEngine::supports)
//! return true — the tests below exercise the fall-through-to-
//! `Other` behaviour end-to-end.

#![cfg(all(feature = "plural-scud", not(target_family = "wasm")))]

use stringcheese_icu_plural::{PluralCategory, PluralEngine};
use stringcheese_zh::plural_data::plural_pack;

fn engine() -> PluralEngine<'static> {
    PluralEngine::new(vec![plural_pack().unwrap()])
}

#[test]
fn cardinal_every_integer_is_other() {
    let e = engine();
    for n in [
        0.0,
        1.0,
        2.0,
        3.0,
        4.0,
        5.0,
        10.0,
        11.0,
        12.0,
        20.0,
        21.0,
        50.0,
        100.0,
        1000.0,
        1_000_000.0,
    ] {
        assert_eq!(e.plural_cardinal(n, "zh"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_every_fraction_is_other() {
    let e = engine();
    for n in [0.5, 1.5, 2.7, 10.1, 100.25] {
        assert_eq!(e.plural_cardinal(n, "zh"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_negatives_are_other() {
    let e = engine();
    for n in [-1.0, -2.0, -10.0, -100.5] {
        assert_eq!(e.plural_cardinal(n, "zh"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn ordinal_is_always_other() {
    let e = engine();
    for n in [1.0, 2.0, 3.0, 10.0, 100.0] {
        assert_eq!(e.plural_ordinal(n, "zh"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn zh_cn_falls_back_to_zh() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "zh-CN"), PluralCategory::Other);
    assert_eq!(e.plural_cardinal(5.0, "zh-Hans-CN"), PluralCategory::Other);
    assert_eq!(e.plural_cardinal(2.0, "zh-Hant-TW"), PluralCategory::Other);
    assert_eq!(e.plural_ordinal(1.0, "zh-SG"), PluralCategory::Other);
}

#[test]
fn engine_reports_zh_as_supported() {
    let e = engine();
    assert!(e.supports("zh"));
    assert!(e.supports("zh-CN"));
    assert!(e.supports("zh-Hans-CN"));
    assert!(!e.supports("ja"));
}

#[test]
fn cardinal_coverage_sweep_0_to_20() {
    // All 21 integers land in `other` — but a sweep guards against
    // an accidental rule slipping in.
    let e = engine();
    for n in 0..=20 {
        assert_eq!(
            e.plural_cardinal(f64::from(n), "zh"),
            PluralCategory::Other,
            "n={n}"
        );
    }
}

#[test]
fn non_finite_returns_other() {
    let e = engine();
    assert_eq!(e.plural_cardinal(f64::NAN, "zh"), PluralCategory::Other);
    assert_eq!(
        e.plural_cardinal(f64::INFINITY, "zh"),
        PluralCategory::Other
    );
}
