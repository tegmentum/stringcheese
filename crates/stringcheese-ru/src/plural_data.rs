//! WIT-i18n plural-rules SCUD pack for Russian.
//!
//! Exposes the compiled `plural-ru.scud` blob ([`PLURAL_RU_SCUD`])
//! plus [`plural_pack`], a helper that wraps it as a
//! [`stringcheese_icu_plural::PluralPack`] ready to hand to a
//! [`stringcheese_icu_plural::PluralEngine`].
//!
//! The SCUD blob is generated in `build.rs` from the CLDR 44
//! `plurals.xml` for Russian and embedded here via `include_bytes!`.
//! See `docs/design/wit-i18n.md` § 8.3 for the Phase 3 delivery
//! notes.
//!
//! # Coverage
//!
//! * **Cardinals** — Russian is one of the classic Slavic three-way
//!   plurals. The four CLDR categories the pack encodes:
//!     * `one` (1, 21, 31, …) — `v = 0 and i % 10 = 1 and i % 100
//!       != 11`.
//!     * `few` (2-4, 22-24, …) — `v = 0 and i % 10 in 2..4 and
//!       i % 100 not in 12..14`.
//!     * `many` (0, 5-20, 25-30, …) — `v = 0 and (i % 10 = 0 or
//!       i % 10 in 5..9 or i % 100 in 11..14)`.
//!     * `other` — every fractional input; no integer input under
//!       CLDR 44's Russian rules lands here.
//! * **Ordinals** — CLDR 44 ships `other` only for Russian ordinals;
//!   the pack carries no ordinal rules.

use stringcheese_icu_plural::{PluralPack, ScudError};

/// The compiled plural-rules SCUD pack for Russian.
pub const PLURAL_RU_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plural-ru.scud"));

/// Wrap [`PLURAL_RU_SCUD`] as a [`PluralPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation — a defensive check against a corrupt build artifact.
pub fn plural_pack() -> Result<PluralPack<'static>, ScudError> {
    PluralPack::from_scud_bytes(PLURAL_RU_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "ru";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_plural::{PluralCategory, PluralEngine};

    #[test]
    fn pack_loads() {
        let pack = plural_pack().unwrap();
        assert_eq!(pack.locale(), "ru");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn cardinal_slavic_three_way() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        assert_eq!(e.plural_cardinal(1.0, "ru"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(2.0, "ru"), PluralCategory::Few);
        assert_eq!(e.plural_cardinal(5.0, "ru"), PluralCategory::Many);
        assert_eq!(e.plural_cardinal(0.0, "ru"), PluralCategory::Many);
    }

    #[test]
    fn ordinal_is_always_other() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        for n in [1.0, 2.0, 3.0, 21.0, 100.0] {
            assert_eq!(e.plural_ordinal(n, "ru"), PluralCategory::Other, "n={n}");
        }
    }

    #[test]
    fn pack_is_small() {
        assert!(
            PLURAL_RU_SCUD.len() < 512,
            "plural-ru.scud grew unexpectedly: {} bytes",
            PLURAL_RU_SCUD.len()
        );
    }
}
