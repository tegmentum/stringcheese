//! WIT-i18n plural-rules SCUD pack for Italian.
//!
//! Exposes the compiled `plural-it.scud` blob ([`PLURAL_IT_SCUD`])
//! plus [`plural_pack`], a helper that wraps it as a
//! [`stringcheese_icu_plural::PluralPack`] ready to hand to a
//! [`stringcheese_icu_plural::PluralEngine`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44.1
//! `plurals.xml` for Italian and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 8.3 for the
//! Phase 3 delivery notes.
//!
//! # Coverage
//!
//! * **Cardinals** — Italian CLDR 44.1 ships three categories:
//!     * `one` — `i = 1 and v = 0` (integer 1 only). Italian
//!       differs from Spanish: `1.0` classifies as `other` here.
//!     * `many` — the shipped sub-clause `v = 0 and i != 0 and
//!       i % 1000000 = 0` of CLDR's full rule (the `e ≠ 0`
//!       compact-notation branch is a documented Phase 3
//!       deferral, so `1.5c6 → 1_500_000` sees `other` rather
//!       than `many`).
//!     * `other` — every other integer plus every fractional
//!       input.
//! * **Ordinals** — Italian is one of the very few CLDR locales
//!   that ships a distinct ordinal `many` bucket:
//!     * `many` — `n ∈ {8, 11, 80, 800}` — the four values with
//!       distinct Italian ordinal marking (`ottavo → 8º`,
//!       `undicesimo → 11º`, `ottantesimo → 80º`,
//!       `ottocentesimo → 800º`).
//!     * `other` otherwise.

use stringcheese_icu_plural::{PluralPack, ScudError};

/// The compiled plural-rules SCUD pack for Italian.
pub const PLURAL_IT_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plural-it.scud"));

/// Wrap [`PLURAL_IT_SCUD`] as a [`PluralPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation — a defensive check against a corrupt build
/// artifact.
pub fn plural_pack() -> Result<PluralPack<'static>, ScudError> {
    PluralPack::from_scud_bytes(PLURAL_IT_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "it";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_plural::{PluralCategory, PluralEngine};

    #[test]
    fn pack_loads() {
        let pack = plural_pack().unwrap();
        assert_eq!(pack.locale(), "it");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn cardinal_one_many_other() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        assert_eq!(e.plural_cardinal(1.0, "it"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(2.0, "it"), PluralCategory::Other);
        assert_eq!(e.plural_cardinal(0.0, "it"), PluralCategory::Other);
        assert_eq!(e.plural_cardinal(1_000_000.0, "it"), PluralCategory::Many);
    }

    #[test]
    fn ordinal_many_covers_the_four_italian_values() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        for n in [8.0, 11.0, 80.0, 800.0] {
            assert_eq!(e.plural_ordinal(n, "it"), PluralCategory::Many, "n={n}");
        }
        for n in [1.0, 2.0, 7.0, 9.0, 10.0, 12.0, 100.0] {
            assert_eq!(e.plural_ordinal(n, "it"), PluralCategory::Other, "n={n}");
        }
    }

    #[test]
    fn pack_is_small() {
        assert!(
            PLURAL_IT_SCUD.len() < 512,
            "plural-it.scud grew unexpectedly: {} bytes",
            PLURAL_IT_SCUD.len()
        );
    }
}
