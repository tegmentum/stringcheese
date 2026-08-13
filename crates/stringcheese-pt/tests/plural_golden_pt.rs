//! Golden plural-classification vectors for the Portuguese pack.
//!
//! Portuguese CLDR 44 cardinals — the shipped pack matches pt-PT
//! rules: `one` (n = 1 and v = 0), `many` (large-number bucket for
//! exact million-multiples, with the compact-notation branch
//! deferred), and `other` for everything else. Ordinals are
//! `other`-only. pt-BR uses the default `pt` rule `i = 0..1` (both
//! 0 and 1 land in `one`), which differs from the shipped pt-PT
//! rule — pt-BR queries fall back to this pack under the shipped
//! configuration and see pt-PT's stricter classification (see the
//! pack docs for the follow-up trade-off).

#![cfg(all(feature = "plural-scud", not(target_family = "wasm")))]

use stringcheese_icu_plural::{PluralCategory, PluralEngine};
use stringcheese_pt::plural_data::plural_pack;

fn engine() -> PluralEngine<'static> {
    PluralEngine::new(vec![plural_pack().unwrap()])
}

#[test]
fn cardinal_one_matches_integer_1_only() {
    let e = engine();
    assert_eq!(e.plural_cardinal(1.0, "pt"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(-1.0, "pt"), PluralCategory::One);
}

#[test]
fn cardinal_zero_is_other_under_pt_pt() {
    let e = engine();
    // pt-PT: 0 → other (unlike default `pt` / pt-BR where 0 → one).
    assert_eq!(e.plural_cardinal(0.0, "pt"), PluralCategory::Other);
}

#[test]
fn cardinal_typical_integers_are_other() {
    let e = engine();
    for n in [2.0, 3.0, 5.0, 10.0, 21.0, 100.0, 999.0, 999_999.0] {
        assert_eq!(e.plural_cardinal(n, "pt"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_many_covers_exact_million_multiples() {
    let e = engine();
    for n in [
        1_000_000.0,
        2_000_000.0,
        3_000_000.0,
        10_000_000.0,
        100_000_000.0,
        1_000_000_000.0,
    ] {
        assert_eq!(e.plural_cardinal(n, "pt"), PluralCategory::Many, "n={n}");
    }
}

#[test]
fn cardinal_non_million_multiples_stay_other() {
    let e = engine();
    for n in [999_999.0, 1_000_001.0, 1_500_000.0, 1_999_999.0] {
        assert_eq!(e.plural_cardinal(n, "pt"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_fractional_million_is_other() {
    let e = engine();
    // v = 0 guard: 1_000_000.5 has v = 1, falls to `other`.
    assert_eq!(e.plural_cardinal(1_000_000.5, "pt"), PluralCategory::Other);
}

#[test]
fn cardinal_fractional_is_other() {
    let e = engine();
    for n in [0.5, 1.5, 2.5, 10.1, 100.25] {
        assert_eq!(e.plural_cardinal(n, "pt"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn cardinal_negative_million_multiple_is_many() {
    let e = engine();
    assert_eq!(e.plural_cardinal(-1_000_000.0, "pt"), PluralCategory::Many);
}

#[test]
fn ordinal_is_always_other() {
    let e = engine();
    for n in [0.0, 1.0, 2.0, 3.0, 8.0, 11.0, 21.0, 100.0, 1_000_000.0] {
        assert_eq!(e.plural_ordinal(n, "pt"), PluralCategory::Other, "n={n}");
    }
}

#[test]
fn pt_br_and_pt_pt_fall_back_to_pt() {
    let e = engine();
    // pt-BR is served by the pt-PT-shape pack under the shipped
    // configuration — a documented follow-up.
    assert_eq!(e.plural_cardinal(1.0, "pt-BR"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(1.0, "pt-PT"), PluralCategory::One);
    assert_eq!(e.plural_cardinal(2.0, "pt-BR"), PluralCategory::Other);
    assert_eq!(
        e.plural_cardinal(1_000_000.0, "pt-PT"),
        PluralCategory::Many
    );
    // pt-BR default rule would classify 0 as `one`; the shipped
    // pt-PT-shape pack classifies it as `other`. Documented delta.
    assert_eq!(e.plural_cardinal(0.0, "pt-BR"), PluralCategory::Other);
}

#[test]
fn cardinal_coverage_sweep_0_to_20() {
    // 0..=20 sweep under pt-PT rules — only 1 lands in `one`.
    let e = engine();
    for n in 0..=20u32 {
        let expected = if n == 1 {
            PluralCategory::One
        } else {
            PluralCategory::Other
        };
        assert_eq!(e.plural_cardinal(f64::from(n), "pt"), expected, "n={n}");
    }
}
