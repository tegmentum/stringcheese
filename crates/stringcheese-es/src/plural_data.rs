//! WIT-i18n plural-rules SCUD pack for Spanish.
//!
//! Exposes the compiled `plural-es.scud` blob ([`PLURAL_ES_SCUD`])
//! plus [`plural_pack`], a helper that wraps it as a
//! [`stringcheese_icu_plural::PluralPack`] ready to hand to a
//! [`stringcheese_icu_plural::PluralEngine`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44
//! `plurals.xml` for Spanish and embedded here via `include_bytes!`.
//! See `docs/design/wit-i18n.md` § 8.3 for the Phase 3 delivery
//! notes.
//!
//! # Coverage
//!
//! * **Cardinals** — Spanish CLDR 44 ships three categories:
//!     * `one` (integer 1 and decimal 1.0) — `n = 1`.
//!     * `many` (`1_000_000`, `2_000_000`, …) — the shipped sub-clause
//!       `v = 0 and i != 0 and i % 1000000 = 0` of CLDR's full rule
//!       `e = 0 and i != 0 and i % 1000000 = 0 and v = 0 or e !=
//!       0..5`. The compact-notation branch (`e ≠ 0`) is a
//!       documented Phase 3 deferral, so `1.5c6 → 1_500_000` sees
//!       `other` rather than `many`.
//!     * `other` — every other integer plus every fractional input
//!       that is not exactly `1.0`.
//! * **Ordinals** — CLDR 44 ships `other` only for Spanish
//!   ordinals; the pack carries no ordinal rules.

use stringcheese_icu_plural::{PluralPack, ScudError};

/// The compiled plural-rules SCUD pack for Spanish.
pub const PLURAL_ES_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plural-es.scud"));

/// Wrap [`PLURAL_ES_SCUD`] as a [`PluralPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation — a defensive check against a corrupt build artifact.
pub fn plural_pack() -> Result<PluralPack<'static>, ScudError> {
    PluralPack::from_scud_bytes(PLURAL_ES_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "es";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_plural::{PluralCategory, PluralEngine};

    #[test]
    fn pack_loads() {
        let pack = plural_pack().unwrap();
        assert_eq!(pack.locale(), "es");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn cardinal_one_many_other() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        assert_eq!(e.plural_cardinal(1.0, "es"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(2.0, "es"), PluralCategory::Other);
        assert_eq!(e.plural_cardinal(0.0, "es"), PluralCategory::Other);
        assert_eq!(e.plural_cardinal(1_000_000.0, "es"), PluralCategory::Many);
    }

    #[test]
    fn ordinal_is_always_other() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        for n in [1.0, 2.0, 3.0, 21.0, 100.0] {
            assert_eq!(e.plural_ordinal(n, "es"), PluralCategory::Other, "n={n}");
        }
    }

    #[test]
    fn pack_is_small() {
        assert!(
            PLURAL_ES_SCUD.len() < 512,
            "plural-es.scud grew unexpectedly: {} bytes",
            PLURAL_ES_SCUD.len()
        );
    }
}
