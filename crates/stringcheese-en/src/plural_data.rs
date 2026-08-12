//! WIT-i18n plural-rules SCUD pack for English.
//!
//! Exposes the compiled `plural-en.scud` blob ([`PLURAL_EN_SCUD`])
//! plus [`plural_pack`], a helper that wraps it as a
//! [`stringcheese_icu_plural::PluralPack`] ready to hand to a
//! [`stringcheese_icu_plural::PluralEngine`].
//!
//! The SCUD blob is generated in `build.rs` from the CLDR 44.1
//! `plurals.xml` for English and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.3 for the
//! Phase 3 delivery notes.
//!
//! # Coverage
//!
//! * **Cardinals** — `one` (i = 1, v = 0), else `other`.
//! * **Ordinals** — `one` (1st, 21st, …), `two` (2nd, 22nd, …),
//!   `few` (3rd, 23rd, …), else `other` (4th, 11th, 12th, 13th,
//!   14th, 15th, …).

use stringcheese_icu_plural::{PluralPack, ScudError};

/// The compiled plural-rules SCUD pack for English.
pub const PLURAL_EN_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plural-en.scud"));

/// Wrap [`PLURAL_EN_SCUD`] as a [`PluralPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation — a defensive check against a corrupt build
/// artifact.
pub fn plural_pack() -> Result<PluralPack<'static>, ScudError> {
    PluralPack::from_scud_bytes(PLURAL_EN_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "en";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_plural::{PluralCategory, PluralEngine};

    #[test]
    fn pack_loads() {
        let pack = plural_pack().unwrap();
        assert_eq!(pack.locale(), "en");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn cardinal_one_is_one() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        assert_eq!(e.plural_cardinal(1.0, "en"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(2.0, "en"), PluralCategory::Other);
    }

    #[test]
    fn ordinal_one_two_few() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        assert_eq!(e.plural_ordinal(1.0, "en"), PluralCategory::One);
        assert_eq!(e.plural_ordinal(2.0, "en"), PluralCategory::Two);
        assert_eq!(e.plural_ordinal(3.0, "en"), PluralCategory::Few);
        assert_eq!(e.plural_ordinal(4.0, "en"), PluralCategory::Other);
        assert_eq!(e.plural_ordinal(11.0, "en"), PluralCategory::Other);
    }

    #[test]
    fn pack_is_small() {
        assert!(
            PLURAL_EN_SCUD.len() < 512,
            "plural-en.scud grew unexpectedly: {} bytes",
            PLURAL_EN_SCUD.len()
        );
    }
}
