//! WIT-i18n collation SCUD pack for Turkish.
//!
//! Exposes the compiled `collation-tr.scud` blob
//! ([`COLLATION_TR_SCUD`]) plus [`collation_pack`], a helper that
//! wraps it as a [`stringcheese_icu_collation::CollationPack`]
//! ready to hand to a
//! [`stringcheese_icu_collation::CollationEngine`].
//!
//! # Primary-distinct dotless-ı / dotted-i (landed)
//!
//! Turkish's alphabetical order interleaves `... h ı i j ...` —
//! dotless `ı` sorts primary-before dotted `i`. Default UCA
//! (feruca / CLDR-root) treats them as primary-equal,
//! tertiary-distinct.
//!
//! The pack ships primary-weight override rows for the full
//! Turkish lowercase alphabet via the `SECT_PRIMARY_OVERRIDES`
//! section, assigning `ı` a primary weight between `h` and `i`.
//! The engine consults the override table at compare / sort-key
//! time so Turkish text sorts in CLDR-conformant dictionary order.
//!
//! # Coverage
//!
//! * Primary-weight overrides for the full Turkish alphabet
//!   (a, b, c, ç, d, e, f, g, ğ, h, ı, i, j, k, l, m, n, o, ö, p,
//!   r, s, ş, t, u, ü, v, y, z).
//! * German ß / ẞ expansions (belt-and-braces uniform behaviour).
//! * Default strength tertiary.
//! * Characters outside the override table (digits, punctuation,
//!   non-Turkish letters) fall back to their ASCII-lowercased
//!   codepoint as a primary-weight approximation.

use stringcheese_icu_collation::{CollationPack, ScudError};

/// The compiled collation SCUD pack for Turkish.
///
/// Generated at build time and embedded via `include_bytes!`; the
/// exact byte count is available at runtime as
/// `COLLATION_TR_SCUD.len()`.
pub const COLLATION_TR_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/collation-tr.scud"));

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
