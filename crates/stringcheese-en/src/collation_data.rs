//! WIT-i18n collation SCUD pack for English.
//!
//! Exposes the compiled `collation-en.scud` blob
//! ([`COLLATION_EN_SCUD`]) plus [`collation_pack`], a helper that
//! wraps it as a [`stringcheese_icu_collation::CollationPack`]
//! ready to hand to a
//! [`stringcheese_icu_collation::CollationEngine`].
//!
//! The SCUD blob is generated in [`build.rs`](../../../build.rs)
//! from a hand-curated DUCET-root-plus-ligature-expansions table
//! and embedded here via `include_bytes!`. See
//! `docs/design/wit-i18n.md` § 6 for the pack shape and § 8.2
//! for the Phase 2 delivery notes.
//!
//! # Coverage
//!
//! * DUCET root ordering (delegated to `feruca` via
//!   `stringcheese-collate::UcaCollator`).
//! * Ligature expansions Æ/æ → AE/ae, Œ/œ → OE/oe. Shipped as
//!   explicit expansions so `sort_key` stays bytewise-consistent
//!   with `compare` for these characters.
//!
//! Not tailored: German ß (default DUCET behaviour is close
//! enough for English text), Turkish dotted / dotless I,
//! Scandinavian å ä ö. Those live in the corresponding
//! per-locale packs, not here.

use stringcheese_icu_collation::{CollationPack, ScudError};

/// The compiled collation SCUD pack for English.
///
/// Generated at build time and embedded via `include_bytes!`; the
/// exact byte count is available at runtime as
/// `COLLATION_EN_SCUD.len()`.
pub const COLLATION_EN_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/collation-en.scud"));

/// Wrap [`COLLATION_EN_SCUD`] as a [`CollationPack`] ready to
/// feed to a [`stringcheese_icu_collation::CollationEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation — a defensive check against a corrupt build
/// artifact. In practice this call succeeds on every well-built
/// binary.
pub fn collation_pack() -> Result<CollationPack<'static>, ScudError> {
    CollationPack::from_scud_bytes(COLLATION_EN_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "en";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_collation::{CollationEngine, CollationStrength};

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = collation_pack().unwrap();
        assert_eq!(pack.locale(), "en");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn engine_can_compare() {
        let engine = CollationEngine::new(alloc::vec![collation_pack().unwrap()]);
        assert_eq!(
            engine.compare("apple", "banana", "en", CollationStrength::Tertiary),
            core::cmp::Ordering::Less,
        );
    }

    #[test]
    fn pack_bytes_are_small() {
        // The pack should be well under 4 KiB — a marker that we
        // did not accidentally check in a real CLDR dump. Adjust
        // upward deliberately if a real coverage expansion
        // pushes past this.
        assert!(
            COLLATION_EN_SCUD.len() < 4 * 1024,
            "collation-en.scud grew unexpectedly: {} bytes",
            COLLATION_EN_SCUD.len()
        );
    }
}
