//! WIT-i18n collation SCUD pack for Russian.
//!
//! Exposes the compiled `collation-ru.scud` blob
//! ([`COLLATION_RU_SCUD`]) plus [`collation_pack`], a helper that
//! wraps it as a [`stringcheese_icu_collation::CollationPack`]
//! ready to hand to a
//! [`stringcheese_icu_collation::CollationEngine`].
//!
//! # Coverage
//!
//! * DUCET root ordering for the Cyrillic block (delegated to
//!   `feruca` via `stringcheese-collate::UcaCollator`) — modern
//!   Russian alphabet sorts by codepoint order, with Ё between Е
//!   and Ж, matching CLDR's `ru` collation.
//! * German ß / ẞ expansions — uniform composed-engine behaviour.
//! * Default strength tertiary.
//!
//! # Phase 2 deferral
//!
//! * **Russian case-second variant.** CLDR ships two variants for
//!   `ru`: lowercase-first (default) and uppercase-first. The
//!   Phase 2 `CollationEngine`'s tertiary compare is fixed to
//!   lowercase-first (feruca's DUCET-root default); the
//!   uppercase-first variant requires an options-section extension
//!   plus algorithm changes to consume it. See
//!   `docs/design/wit-i18n.md` § 8.2.

use stringcheese_icu_collation::{CollationPack, ScudError};

/// The compiled collation SCUD pack for Russian.
pub const COLLATION_RU_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/collation-ru.scud"));

/// Wrap [`COLLATION_RU_SCUD`] as a [`CollationPack`] ready to feed
/// to a [`stringcheese_icu_collation::CollationEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn collation_pack() -> Result<CollationPack<'static>, ScudError> {
    CollationPack::from_scud_bytes(COLLATION_RU_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "ru";

#[cfg(test)]
mod tests {
    use super::*;
    use core::cmp::Ordering;
    use stringcheese_icu_collation::{CollationEngine, CollationStrength};

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = collation_pack().unwrap();
        assert_eq!(pack.locale(), "ru");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn engine_orders_cyrillic_words() {
        let engine = CollationEngine::new(alloc::vec![collation_pack().unwrap()]);
        assert_eq!(
            engine.compare("Москва", "Санкт", "ru", CollationStrength::Tertiary),
            Ordering::Less,
        );
    }

    #[test]
    fn pack_bytes_are_small() {
        assert!(
            COLLATION_RU_SCUD.len() < 1024,
            "collation-ru.scud grew unexpectedly: {} bytes",
            COLLATION_RU_SCUD.len()
        );
    }
}
