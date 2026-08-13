//! WIT-i18n collation SCUD pack for Turkish.
//!
//! Exposes the compiled `collation-tr.scud` blob
//! ([`COLLATION_TR_SCUD`]) plus [`collation_pack`], a helper that
//! wraps it as a [`stringcheese_icu_collation::CollationPack`]
//! ready to hand to a
//! [`stringcheese_icu_collation::CollationEngine`].
//!
//! # Phase 2 deferral: primary-distinct dotless-ı / dotted-i
//!
//! Turkish's alphabetical order interleaves `... h ı i j ...` —
//! dotless `ı` sorts primary-before dotted `i`. Default UCA (which
//! the shipped Phase 2 `CollationEngine` delegates to) treats
//! `ı` and `i` as primary-equal, tertiary-distinct.
//!
//! Bridging the two requires a new SCUD primary-tailoring section
//! plus the `CollationEngine` algorithm changes to consume it —
//! neither of which lands in Phase 6's data-only rollout. The pack
//! ships default UCA behaviour for `ı` / `i` and documents the
//! deferral here; the `collation_golden_tr.rs` tests assert what
//! the engine actually does, with a `primary_distinct_i_deferred`
//! test that a follow-up wave can flip when the algorithm lands.
//!
//! # Coverage
//!
//! * German ß / ẞ expansions (belt-and-braces uniform behaviour).
//! * Default strength tertiary.
//! * Everything else: DUCET-root behaviour via feruca.

use stringcheese_icu_collation::{CollationPack, ScudError};

/// The compiled collation SCUD pack for Turkish.
///
/// Generated at build time and embedded via `include_bytes!`; the
/// exact byte count is available at runtime as
/// `COLLATION_TR_SCUD.len()`.
pub const COLLATION_TR_SCUD: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/collation-tr.scud"));

/// Wrap [`COLLATION_TR_SCUD`] as a [`CollationPack`] ready to feed
/// to a [`stringcheese_icu_collation::CollationEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn collation_pack() -> Result<CollationPack<'static>, ScudError> {
    CollationPack::from_scud_bytes(COLLATION_TR_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "tr";

#[cfg(test)]
mod tests {
    use super::*;
    use core::cmp::Ordering;
    use stringcheese_icu_collation::{CollationEngine, CollationStrength};

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = collation_pack().unwrap();
        assert_eq!(pack.locale(), "tr");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn engine_orders_turkish_ascii_words() {
        let engine = CollationEngine::new(alloc::vec![collation_pack().unwrap()]);
        assert_eq!(
            engine.compare("araba", "bebek", "tr", CollationStrength::Tertiary),
            Ordering::Less,
        );
    }

    #[test]
    fn sharp_s_expansion_via_pack() {
        let engine = CollationEngine::new(alloc::vec![collation_pack().unwrap()]);
        assert_eq!(
            engine.compare("Straße", "Strasse", "tr", CollationStrength::Tertiary),
            Ordering::Equal,
        );
    }

    #[test]
    fn pack_bytes_are_small() {
        assert!(
            COLLATION_TR_SCUD.len() < 1024,
            "collation-tr.scud grew unexpectedly: {} bytes",
            COLLATION_TR_SCUD.len()
        );
    }
}
