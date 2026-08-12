//! WIT-i18n plural-rules SCUD pack for Chinese (Simplified).
//!
//! Exposes the compiled `plural-zh.scud` blob ([`PLURAL_ZH_SCUD`])
//! plus [`plural_pack`], a helper that wraps it as a
//! [`stringcheese_icu_plural::PluralPack`] ready to hand to a
//! [`stringcheese_icu_plural::PluralEngine`].
//!
//! # Coverage
//!
//! Chinese has no grammatical number — CLDR 44 ships `other` only
//! for both cardinals and ordinals. The pack contains no rule
//! entries; every query falls through to
//! [`PluralCategory::Other`](stringcheese_icu_plural::PluralCategory::Other).
//! Shipping the pack (rather than omitting it) lets
//! [`PluralEngine::supports("zh")`](stringcheese_icu_plural::PluralEngine::supports)
//! return true and advertises the CLDR version provenance the SCUD
//! header carries.

use stringcheese_icu_plural::{PluralPack, ScudError};

/// The compiled plural-rules SCUD pack for Chinese.
pub const PLURAL_ZH_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plural-zh.scud"));

/// Wrap [`PLURAL_ZH_SCUD`] as a [`PluralPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn plural_pack() -> Result<PluralPack<'static>, ScudError> {
    PluralPack::from_scud_bytes(PLURAL_ZH_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "zh";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_plural::{PluralCategory, PluralEngine};

    #[test]
    fn pack_loads() {
        let pack = plural_pack().unwrap();
        assert_eq!(pack.locale(), "zh");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn every_number_is_other() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        for n in [0.0, 1.0, 2.0, 3.0, 5.0, 100.0, 1.5] {
            assert_eq!(e.plural_cardinal(n, "zh"), PluralCategory::Other, "n={n}");
            assert_eq!(e.plural_ordinal(n, "zh"), PluralCategory::Other, "n={n}");
        }
    }

    #[test]
    fn pack_is_small() {
        assert!(
            PLURAL_ZH_SCUD.len() < 256,
            "plural-zh.scud grew unexpectedly: {} bytes",
            PLURAL_ZH_SCUD.len()
        );
    }
}
