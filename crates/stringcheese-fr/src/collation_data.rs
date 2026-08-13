//! WIT-i18n collation SCUD pack for French.
//!
//! Exposes the compiled `collation-fr.scud` blob
//! ([`COLLATION_FR_SCUD`]) plus [`collation_pack`], a helper that
//! wraps it as a [`stringcheese_icu_collation::CollationPack`]
//! ready to hand to a
//! [`stringcheese_icu_collation::CollationEngine`].
//!
//! # Coverage
//!
//! * DUCET root ordering (delegated to `feruca` via
//!   `stringcheese-collate::UcaCollator`) — French alphabetical
//!   ordering matches DUCET-root for the Latin script.
//! * Ligature expansions Æ/æ → AE/ae, Œ/œ → OE/oe.
//! * **Backwards-secondary rule** — accents tie-break right-to-left
//!   within a word, producing the classic French sort
//!   `cote < côte < coté < côté`. The tailoring rides on the
//!   `SECT_COLLATION_OPTIONS` backwards-secondary bit; the
//!   `stringcheese-icu-collation` engine reverses the per-position
//!   secondary sequence before compare when the bit is set.
//! * Default strength tertiary (case-sensitive).

use stringcheese_icu_collation::{CollationPack, ScudError};

/// The compiled collation SCUD pack for French.
///
/// Generated at build time and embedded via `include_bytes!`; the
/// exact byte count is available at runtime as
/// `COLLATION_FR_SCUD.len()`.
pub const COLLATION_FR_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/collation-fr.scud"));

/// Wrap [`COLLATION_FR_SCUD`] as a [`CollationPack`] ready to feed
/// to a [`stringcheese_icu_collation::CollationEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn collation_pack() -> Result<CollationPack<'static>, ScudError> {
    CollationPack::from_scud_bytes(COLLATION_FR_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "fr";

#[cfg(test)]
mod tests {
    use super::*;
    use core::cmp::Ordering;
    use stringcheese_icu_collation::{CollationEngine, CollationStrength};

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = collation_pack().unwrap();
        assert_eq!(pack.locale(), "fr");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn engine_orders_french_words() {
        let engine = CollationEngine::new(alloc::vec![collation_pack().unwrap()]);
        assert_eq!(
            engine.compare("pomme", "poire", "fr", CollationStrength::Tertiary),
            Ordering::Greater,
        );
        assert_eq!(
            engine.compare("chat", "chien", "fr", CollationStrength::Tertiary),
            Ordering::Less,
        );
    }

    #[test]
    fn oe_ligature_expands_via_pack() {
        let engine = CollationEngine::new(alloc::vec![collation_pack().unwrap()]);
        assert_eq!(
            engine.compare("Œuvre", "OEuvre", "fr", CollationStrength::Tertiary),
            Ordering::Equal,
        );
    }

    #[test]
    fn pack_bytes_are_small() {
        assert!(
            COLLATION_FR_SCUD.len() < 1024,
            "collation-fr.scud grew unexpectedly: {} bytes",
            COLLATION_FR_SCUD.len()
        );
    }
}
