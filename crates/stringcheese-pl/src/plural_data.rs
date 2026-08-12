//! WIT-i18n plural-rules SCUD pack for Polish.
//!
//! Exposes the compiled `plural-pl.scud` blob ([`PLURAL_PL_SCUD`])
//! plus [`plural_pack`], a helper that wraps it as a
//! [`stringcheese_icu_plural::PluralPack`] ready to hand to a
//! [`stringcheese_icu_plural::PluralEngine`].
//!
//! # Coverage
//!
//! * **Cardinals** — Polish is a three-way plural (`one` / `few` /
//!   `many`) plus `other` for fractions:
//!     * `one` — 1 exactly (`i = 1 and v = 0`).
//!     * `few` — 2-4, 22-24, 32-34, … (shared with Russian's
//!       [`SlavFew`](stringcheese_icu_plural::PluralRuleId::SlavFew)
//!       predicate).
//!     * `many` — 0, 5-19, 25-29, 105-119, …
//!       ([`PlMany`](stringcheese_icu_plural::PluralRuleId::PlMany)).
//!     * `other` — every fractional value.
//! * **Ordinals** — CLDR 44 ships `other` only for Polish ordinals;
//!   the pack carries no ordinal rules.

use stringcheese_icu_plural::{PluralPack, ScudError};

/// The compiled plural-rules SCUD pack for Polish.
pub const PLURAL_PL_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plural-pl.scud"));

/// Wrap [`PLURAL_PL_SCUD`] as a [`PluralPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn plural_pack() -> Result<PluralPack<'static>, ScudError> {
    PluralPack::from_scud_bytes(PLURAL_PL_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "pl";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_plural::{PluralCategory, PluralEngine};

    #[test]
    fn pack_loads() {
        let pack = plural_pack().unwrap();
        assert_eq!(pack.locale(), "pl");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn cardinal_three_way() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        assert_eq!(e.plural_cardinal(1.0, "pl"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(2.0, "pl"), PluralCategory::Few);
        assert_eq!(e.plural_cardinal(5.0, "pl"), PluralCategory::Many);
        assert_eq!(e.plural_cardinal(0.0, "pl"), PluralCategory::Many);
    }

    #[test]
    fn ordinal_is_always_other() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        for n in [1.0, 2.0, 3.0, 21.0, 100.0] {
            assert_eq!(e.plural_ordinal(n, "pl"), PluralCategory::Other, "n={n}");
        }
    }

    #[test]
    fn pack_is_small() {
        assert!(
            PLURAL_PL_SCUD.len() < 512,
            "plural-pl.scud grew unexpectedly: {} bytes",
            PLURAL_PL_SCUD.len()
        );
    }
}
