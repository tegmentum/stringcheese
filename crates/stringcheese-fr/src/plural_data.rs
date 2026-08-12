//! WIT-i18n plural-rules SCUD pack for French.
//!
//! Exposes the compiled `plural-fr.scud` blob ([`PLURAL_FR_SCUD`])
//! plus [`plural_pack`], a helper that wraps it as a
//! [`stringcheese_icu_plural::PluralPack`] ready to hand to a
//! [`stringcheese_icu_plural::PluralEngine`].
//!
//! # Coverage
//!
//! * **Cardinals** — `one` when `i in 0..1` (both 0 and 1 use the
//!   singular in French: "0 chose", "1 chose"), else `other`.
//! * **Ordinals** — `one` when `n = 1` (1er / 1re), else `other`
//!   (2e, 3e, 4e, …).
//!
//! The CLDR `many` category (compact large-number notation with
//! e ≥ 0 and i % 1000000 = 0) is a documented Phase 3 deferral —
//! the plural crate does not evaluate the `e` compact operand.

use stringcheese_icu_plural::{PluralPack, ScudError};

/// The compiled plural-rules SCUD pack for French.
pub const PLURAL_FR_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plural-fr.scud"));

/// Wrap [`PLURAL_FR_SCUD`] as a [`PluralPack`].
pub fn plural_pack() -> Result<PluralPack<'static>, ScudError> {
    PluralPack::from_scud_bytes(PLURAL_FR_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "fr";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_plural::{PluralCategory, PluralEngine};

    #[test]
    fn pack_loads() {
        let pack = plural_pack().unwrap();
        assert_eq!(pack.locale(), "fr");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn cardinal_zero_and_one_are_one() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        assert_eq!(e.plural_cardinal(0.0, "fr"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(1.0, "fr"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(2.0, "fr"), PluralCategory::Other);
    }

    #[test]
    fn ordinal_one_is_one() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        assert_eq!(e.plural_ordinal(1.0, "fr"), PluralCategory::One);
        assert_eq!(e.plural_ordinal(2.0, "fr"), PluralCategory::Other);
        assert_eq!(e.plural_ordinal(21.0, "fr"), PluralCategory::Other);
    }

    #[test]
    fn pack_is_small() {
        assert!(
            PLURAL_FR_SCUD.len() < 512,
            "plural-fr.scud grew unexpectedly: {} bytes",
            PLURAL_FR_SCUD.len()
        );
    }
}
