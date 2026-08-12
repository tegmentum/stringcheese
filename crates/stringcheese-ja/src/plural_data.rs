//! WIT-i18n plural-rules SCUD pack for Japanese.
//!
//! Exposes the compiled `plural-ja.scud` blob ([`PLURAL_JA_SCUD`])
//! plus [`plural_pack`], a helper that wraps it as a
//! [`stringcheese_icu_plural::PluralPack`] ready to hand to a
//! [`stringcheese_icu_plural::PluralEngine`].
//!
//! # Coverage
//!
//! Japanese has no grammatical number — CLDR 44 ships `other` only
//! for both cardinals and ordinals. The pack contains no rule
//! entries; every query falls through to
//! [`PluralCategory::Other`](stringcheese_icu_plural::PluralCategory::Other).
//! Shipping the pack (rather than omitting it) lets
//! [`PluralEngine::supports("ja")`](stringcheese_icu_plural::PluralEngine::supports)
//! return true and advertises the CLDR version provenance.

use stringcheese_icu_plural::{PluralPack, ScudError};

/// The compiled plural-rules SCUD pack for Japanese.
pub const PLURAL_JA_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plural-ja.scud"));

/// Wrap [`PLURAL_JA_SCUD`] as a [`PluralPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn plural_pack() -> Result<PluralPack<'static>, ScudError> {
    PluralPack::from_scud_bytes(PLURAL_JA_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "ja";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_plural::{PluralCategory, PluralEngine};

    #[test]
    fn pack_loads() {
        let pack = plural_pack().unwrap();
        assert_eq!(pack.locale(), "ja");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn every_number_is_other() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        for n in [0.0, 1.0, 2.0, 3.0, 5.0, 100.0, 1.5] {
            assert_eq!(e.plural_cardinal(n, "ja"), PluralCategory::Other, "n={n}");
            assert_eq!(e.plural_ordinal(n, "ja"), PluralCategory::Other, "n={n}");
        }
    }

    #[test]
    fn pack_is_small() {
        assert!(
            PLURAL_JA_SCUD.len() < 256,
            "plural-ja.scud grew unexpectedly: {} bytes",
            PLURAL_JA_SCUD.len()
        );
    }
}
