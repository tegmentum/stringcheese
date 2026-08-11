//! WIT-i18n collation SCUD pack for German.
//!
//! Exposes the compiled `collation-de.scud` blob
//! ([`COLLATION_DE_SCUD`]) plus [`collation_pack`], a helper
//! that wraps it as a
//! [`stringcheese_icu_collation::CollationPack`] ready to hand
//! to a [`stringcheese_icu_collation::CollationEngine`].
//!
//! The SCUD blob is generated in [`build.rs`](../../../build.rs)
//! from a hand-verified DIN 5007-2 (phonebook) table and
//! embedded here via `include_bytes!`. See
//! `docs/design/wit-i18n.md` § 6 for the pack shape and § 8.2
//! for the Phase 2 delivery notes on why phonebook was chosen
//! over dictionary ordering as the default.
//!
//! # Coverage
//!
//! The pack ships the DIN 5007-2 (phonebook) tailoring:
//!
//! * `ß → ss` and `ẞ → SS`
//! * `ä → ae`, `Ä → AE`
//! * `ö → oe`, `Ö → OE`
//! * `ü → ue`, `Ü → UE`
//!
//! Callers who prefer the dictionary (DIN 5007-1) ordering
//! reach for the native
//! [`crate::GermanCollator::DIN_5007_DICTIONARY`] preset — that
//! path bypasses the SCUD pack entirely and applies the fold
//! rules directly in the native collator.

use stringcheese_icu_collation::{CollationPack, ScudError};

/// The compiled collation SCUD pack for German.
///
/// Generated at build time and embedded via `include_bytes!`; the
/// exact byte count is available at runtime as
/// `COLLATION_DE_SCUD.len()`.
pub const COLLATION_DE_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/collation-de.scud"));

/// Wrap [`COLLATION_DE_SCUD`] as a [`CollationPack`] ready to
/// feed to a [`stringcheese_icu_collation::CollationEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn collation_pack() -> Result<CollationPack<'static>, ScudError> {
    CollationPack::from_scud_bytes(COLLATION_DE_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "de";

#[cfg(test)]
mod tests {
    use super::*;
    use core::cmp::Ordering;
    use stringcheese_icu_collation::{CollationEngine, CollationStrength};

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = collation_pack().unwrap();
        assert_eq!(pack.locale(), "de");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn engine_phonebook_umlaut_expansion() {
        let engine = CollationEngine::new(alloc::vec![collation_pack().unwrap()]);
        // Under DIN-2: Bär → Baer; sorts before Bar.
        assert_eq!(
            engine.compare("Bär", "Bar", "de", CollationStrength::Tertiary),
            Ordering::Less,
        );
        // Bär compares equal to Baer.
        assert_eq!(
            engine.compare("Bär", "Baer", "de", CollationStrength::Tertiary),
            Ordering::Equal,
        );
    }

    #[test]
    fn engine_sharp_s_expansion() {
        let engine = CollationEngine::new(alloc::vec![collation_pack().unwrap()]);
        assert_eq!(
            engine.compare("Straße", "Strasse", "de", CollationStrength::Tertiary),
            Ordering::Equal,
        );
    }

    #[test]
    fn pack_bytes_are_small() {
        assert!(
            COLLATION_DE_SCUD.len() < 1024,
            "collation-de.scud grew unexpectedly: {} bytes",
            COLLATION_DE_SCUD.len()
        );
    }
}
