//! WIT-i18n plural-rules SCUD pack for Portuguese.
//!
//! Exposes the compiled `plural-pt.scud` blob ([`PLURAL_PT_SCUD`])
//! plus [`plural_pack`], a helper that wraps it as a
//! [`stringcheese_icu_plural::PluralPack`] ready to hand to a
//! [`stringcheese_icu_plural::PluralEngine`].
//!
//! The SCUD blob is generated in `build.rs` from CLDR 44
//! `plurals.xml` and embedded here via `include_bytes!`. See
//! `docs/design/wit-i18n.md` § 8.3 for the Phase 3 delivery notes.
//!
//! # Coverage
//!
//! * **Cardinals** — Portuguese CLDR 44 ships three categories.
//!   The shipped pack matches the pt-PT rules:
//!     * `one` (integer 1 only) — `n = 1 and v = 0`.
//!     * `many` (`1_000_000`, `2_000_000`, …) — the shipped sub-clause
//!       `v = 0 and i != 0 and i % 1000000 = 0` of CLDR's full rule
//!       `e = 0 and i != 0 and i % 1000000 = 0 and v = 0 or e !=
//!       0..5`. The compact-notation branch (`e ≠ 0`) is a
//!       documented Phase 3 deferral.
//!     * `other` — every other integer plus every fractional input.
//! * **Ordinals** — CLDR 44 ships `other` only for Portuguese
//!   ordinals; the pack carries no ordinal rules.
//!
//! # pt-PT vs pt-BR
//!
//! The pack labels itself `"pt"` so the fallback chain resolves
//! `pt-BR → pt`, `pt-PT → pt`, and bare `pt` all to this pack.
//! CLDR ships two rule sets:
//!
//! * `pt` (CLDR default, matches pt-BR): `one` when `i = 0..1`
//!   (both 0 and 1 → `one`).
//! * `pt_PT`: `one` when `n = 1 and v = 0` (integer 1 only).
//!
//! This pack ships the pt-PT rule as the portable default; a
//! caller servicing pt-BR would need a separate `plural-pt-BR.scud`
//! or a runtime override. Documented as a Phase 3 follow-up.

use stringcheese_icu_plural::{PluralPack, ScudError};

/// The compiled plural-rules SCUD pack for Portuguese (pt-PT
/// defaults).
pub const PLURAL_PT_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/plural-pt.scud"));

/// Wrap [`PLURAL_PT_SCUD`] as a [`PluralPack`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation — a defensive check against a corrupt build artifact.
pub fn plural_pack() -> Result<PluralPack<'static>, ScudError> {
    PluralPack::from_scud_bytes(PLURAL_PT_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "pt";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_plural::{PluralCategory, PluralEngine};

    #[test]
    fn pack_loads() {
        let pack = plural_pack().unwrap();
        assert_eq!(pack.locale(), "pt");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn cardinal_one_many_other() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        assert_eq!(e.plural_cardinal(1.0, "pt"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(0.0, "pt"), PluralCategory::Other);
        assert_eq!(e.plural_cardinal(2.0, "pt"), PluralCategory::Other);
        assert_eq!(e.plural_cardinal(1_000_000.0, "pt"), PluralCategory::Many);
    }

    #[test]
    fn ordinal_is_always_other() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        for n in [1.0, 2.0, 3.0, 21.0, 100.0] {
            assert_eq!(e.plural_ordinal(n, "pt"), PluralCategory::Other, "n={n}");
        }
    }

    #[test]
    fn pt_br_and_pt_pt_fall_back_to_pt() {
        let e = PluralEngine::new(alloc::vec![plural_pack().unwrap()]);
        assert_eq!(e.plural_cardinal(1.0, "pt-BR"), PluralCategory::One);
        assert_eq!(e.plural_cardinal(1.0, "pt-PT"), PluralCategory::One);
    }

    #[test]
    fn pack_is_small() {
        assert!(
            PLURAL_PT_SCUD.len() < 512,
            "plural-pt.scud grew unexpectedly: {} bytes",
            PLURAL_PT_SCUD.len()
        );
    }
}
