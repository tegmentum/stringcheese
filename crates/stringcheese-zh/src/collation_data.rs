//! WIT-i18n collation SCUD pack for Chinese.
//!
//! Exposes the compiled `collation-zh.scud` blob
//! ([`COLLATION_ZH_SCUD`]) plus [`collation_pack`], a helper that
//! wraps it as a [`stringcheese_icu_collation::CollationPack`]
//! ready to hand to a
//! [`stringcheese_icu_collation::CollationEngine`].
//!
//! # Phase 2 deferrals
//!
//! CLDR ships four `zh` collation variants — `standard` (stroke-
//! based, numeric stroke order), `pinyin` (Latin pinyin
//! transliteration), `stroke`, and `zhuyin`. **All four are
//! documented Phase 2 `CollationEngine` deferrals** — each requires
//! a large Han-to-order or Han-to-pinyin lookup table (tens of
//! thousands of entries) plus algorithm support that isn't in
//! Phase 2. The shipped pack uses feruca's DUCET-root ordering,
//! which sorts CJK Han by codepoint order — deterministic but
//! not linguistically meaningful. See
//! `docs/design/wit-i18n.md` § 8.2 for the deferral rationale.
//!
//! # Coverage
//!
//! * DUCET root ordering (delegated to `feruca` via
//!   `stringcheese-collate::UcaCollator`).
//! * German ß / ẞ expansions.
//! * Default strength tertiary.

use stringcheese_icu_collation::{CollationPack, ScudError};

/// The compiled collation SCUD pack for Chinese.
pub const COLLATION_ZH_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/collation-zh.scud"));

/// Wrap [`COLLATION_ZH_SCUD`] as a [`CollationPack`] ready to feed
/// to a [`stringcheese_icu_collation::CollationEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn collation_pack() -> Result<CollationPack<'static>, ScudError> {
    CollationPack::from_scud_bytes(COLLATION_ZH_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "zh";

#[cfg(test)]
mod tests {
    use super::*;
    use core::cmp::Ordering;
    use stringcheese_icu_collation::{CollationEngine, CollationStrength};

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = collation_pack().unwrap();
        assert_eq!(pack.locale(), "zh");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn engine_orders_ascii_words_under_zh() {
        let engine = CollationEngine::new(alloc::vec![collation_pack().unwrap()]);
        assert_eq!(
            engine.compare("apple", "banana", "zh", CollationStrength::Tertiary),
            Ordering::Less,
        );
    }

    #[test]
    fn pack_bytes_are_small() {
        assert!(
            COLLATION_ZH_SCUD.len() < 1024,
            "collation-zh.scud grew unexpectedly: {} bytes",
            COLLATION_ZH_SCUD.len()
        );
    }
}
