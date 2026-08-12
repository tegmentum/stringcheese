//! WIT-i18n plural-rules SCUD pack for German.
//!
//! Exposes the compiled `plural-de.scud` blob ([`PLURAL_DE_SCUD`])
//! plus [`plural_pack`], a helper that wraps it as a
//! [`stringcheese_icu_plural::PluralPack`] ready to hand to a
//! [`stringcheese_icu_plural::PluralEngine`].
//!
//! # Coverage
//!
//! * **Cardinals** — `one` when `i = 1` and `v = 0` (integer 1),
//!   else `other`. Same shape as English cardinals.
//! * **Ordinals** — every value is `other`. German uses the same
//!   suffix (`.`) for every ordinal, so CLDR ships no distinction.

use stringcheese_icu_plural::{PluralPack, ScudError};

/// The compiled plural-rules SCUD pack for German.
pub const PLURAL_DE_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plural-de.scud"));

/// Wrap [`PLURAL_DE_SCUD`] as a [`PluralPack`].
pub fn plural_pack() -> Result<PluralPack<'static>, ScudError> {
    PluralPack::from_scud_bytes(PLURAL_DE_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "de";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_plural::{PluralCategory, PluralEngine};

    #[test]
    fn pack_loads() {
        let pack = plural_pack().unwrap();
        assert_eq!(pack.locale(), "de");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn cardinal_one_is_one() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        assert_eq!(e.plural_cardinal(1.0, "de"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(2.0, "de"), PluralCategory::Other);
        assert_eq!(e.plural_cardinal(0.0, "de"), PluralCategory::Other);
    }

    #[test]
    fn ordinal_is_always_other() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        for n in [1.0, 2.0, 3.0, 21.0, 100.0] {
            assert_eq!(e.plural_ordinal(n, "de"), PluralCategory::Other, "n={n}");
        }
    }

    #[test]
    fn pack_is_small() {
        assert!(
            PLURAL_DE_SCUD.len() < 512,
            "plural-de.scud grew unexpectedly: {} bytes",
            PLURAL_DE_SCUD.len()
        );
    }
}
