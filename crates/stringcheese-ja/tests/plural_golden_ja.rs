//! Golden plural-classification vectors for the Japanese pack.
//!
//! Japanese lacks grammatical number; CLDR ships `other` only for
//! both cardinals and ordinals. The tests below exercise the
//! fall-through-to-`Other` behaviour end-to-end.

#![cfg(all(feature = "plural-scud", not(target_family = "wasm")))]

use stringcheese_icu_plural::{PluralCategory, PluralEngine};
use stringcheese_ja::plural_data::plural_pack;

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
        assert_eq!(e.plural_cardinal(n, "ja"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_every_fraction_is_other() {
    let e = engine();
    for n in [0.5, 1.5, 2.7, 10.1, 100.25] {
        assert_eq!(e.plural_cardinal(n, "ja"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_negatives_are_other() {
    let e = engine();
    for n in [-1.0, -2.0, -10.0, -100.5] {
        assert_eq!(e.plural_cardinal(n, "ja"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn ordinal_is_always_other() {
    let e = engine();
    for n in [1.0, 2.0, 3.0, 10.0, 100.0] {
        assert_eq!(e.plural_ordinal(n, "ja"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn ja_jp_falls_back_to_ja() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "ja-JP"), PluralCategory::Other);
    assert_eq!(
        e.plural_cardinal(5.0, "ja-JP-u-nu-latn"),
        PluralCategory::Other
    );
    assert_eq!(e.plural_ordinal(1.0, "ja-JP"), PluralCategory::Other);
}

#[test]
fn engine_reports_ja_as_supported() {
    let e = engine();
    assert!(e.supports("ja"));
    assert!(e.supports("ja-JP"));
    assert!(!e.supports("ko"));
    assert!(!e.supports("zh"));
}

#[test]
fn cardinal_coverage_sweep_0_to_20() {
    let e = engine();
    for n in 0..=20 {
        assert_eq!(
            e.plural_cardinal(f64::from(n), "ja"),
            PluralCategory::Other,
            "n={n}"
        );
    }
}

#[test]
fn non_finite_returns_other() {
    let e = engine();
    assert_eq!(e.plural_cardinal(f64::NAN, "ja"), PluralCategory::Other);
    assert_eq!(
        e.plural_cardinal(f64::INFINITY, "ja"),
        PluralCategory::Other
    );
}
