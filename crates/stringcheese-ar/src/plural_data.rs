//! WIT-i18n plural-rules SCUD pack for Arabic.
//!
//! Exposes the compiled `plural-ar.scud` blob ([`PLURAL_AR_SCUD`])
//! plus [`plural_pack`], a helper that wraps it as a
//! [`stringcheese_icu_plural::PluralPack`] ready to hand to a
//! [`stringcheese_icu_plural::PluralEngine`].
//!
//! # Coverage
//!
//! Arabic is the **maximum-category-count locale** in CLDR — it uses
//! all six plural categories. Every non-`other` category ships as a
//! rule; the engine falls through to
//! [`PluralCategory::Other`](stringcheese_icu_plural::PluralCategory::Other)
//! when none matches:
//!
//! * `zero` — `n = 0`.
//! * `one` — `n = 1`.
//! * `two` — `n = 2`.
//! * `few` — `n % 100 in 3..10` (3-10, 103-110, 203-210, …).
//! * `many` — `n % 100 in 11..99` (11-99, 111-199, …).
//! * `other` — 100, 101, 102, 200, 201, 202, … and every fractional
//!   input except 0.0.
//!
//! **Ordinals** — CLDR 44 ships `other` only for Arabic ordinals;
//! the pack carries no ordinal rules.

use stringcheese_icu_plural::{PluralPack, ScudError};

/// The compiled plural-rules SCUD pack for Arabic.
pub const PLURAL_AR_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plural-ar.scud"));

/// Wrap [`PLURAL_AR_SCUD`] as a [`PluralPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn plural_pack() -> Result<PluralPack<'static>, ScudError> {
    PluralPack::from_scud_bytes(PLURAL_AR_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "ar";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_plural::{PluralCategory, PluralEngine};

    #[test]
    fn pack_loads() {
        let pack = plural_pack().unwrap();
        assert_eq!(pack.locale(), "ar");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn cardinal_all_six_categories_exercised() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        assert_eq!(e.plural_cardinal(0.0, "ar"), PluralCategory::Zero);
        assert_eq!(e.plural_cardinal(1.0, "ar"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(2.0, "ar"), PluralCategory::Two);
        assert_eq!(e.plural_cardinal(3.0, "ar"), PluralCategory::Few);
        assert_eq!(e.plural_cardinal(11.0, "ar"), PluralCategory::Many);
        assert_eq!(e.plural_cardinal(100.0, "ar"), PluralCategory::Other);
    }

    #[test]
    fn ordinal_is_always_other() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        for n in [1.0, 2.0, 3.0, 21.0, 100.0] {
            assert_eq!(e.plural_ordinal(n, "ar"), PluralCategory::Other, "n={n}");
        }
    }

    #[test]
    fn pack_is_small() {
        assert!(
            PLURAL_AR_SCUD.len() < 512,
            "plural-ar.scud grew unexpectedly: {} bytes",
            PLURAL_AR_SCUD.len()
        );
    }
}
