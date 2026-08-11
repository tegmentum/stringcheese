//! WIT-i18n case-mapping SCUD pack for Turkish.
//!
//! Exposes the compiled `case-tr.scud` blob (`CASE_TR_SCUD`) plus
//! [`case_pack`], a helper that wraps it as a
//! [`stringcheese_icu_case::CasePack`] ready to feed to a
//! [`stringcheese_icu_case::CaseEngine`].
//!
//! The SCUD blob is generated in [`build.rs`](../../../build.rs) at
//! crate build time and embedded here via `include_bytes!`. See
//! `docs/design/wit-i18n.md` § 6 for the pack shape.
//!
//! # Coverage
//!
//! * Turkish dotted / dotless-I contextual overrides:
//!   - `I` (U+0049) → `ı` (U+0131) under Turkish lowercasing.
//!   - `i` (U+0069) → `İ` (U+0130) under Turkish uppercasing.
//! * The symmetric simple pair `İ ↔ i` and `ı ↔ I`.
//! * Turkish letters that already fold correctly under default
//!   Unicode (Ç, Ğ, Ö, Ş, Ü), included so pack-hit ratios are
//!   uniform for the Turkish alphabet.
//! * German ß → "SS" (full upper) — kept in the pack so a composed
//!   `[en, tr]` engine behaves identically regardless of which pack
//!   the query resolves through first.
//!
//! # Cross-locale composition
//!
//! Phase 1 of the WIT-i18n design (`docs/design/wit-i18n.md` § 8)
//! commits to "Turkish `i` via the `tr` pack loaded alongside" the
//! English pack. That commitment is exercised by the
//! `case_cross_locale.rs` integration test: the same input
//! `"ISTANBUL"` produces `"istanbul"` under English lowercasing but
//! `"ıstanbul"` under Turkish lowercasing, both from the same
//! `CaseEngine` instance.

use stringcheese_icu_case::{CasePack, ScudError};

/// The compiled case-mapping SCUD pack for Turkish.
///
/// Generated at build time and embedded via `include_bytes!`.
pub const CASE_TR_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/case-tr.scud"));

/// Wrap [`CASE_TR_SCUD`] as a [`CasePack`] ready to feed to a
/// [`stringcheese_icu_case::CaseEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation. In practice this call succeeds on every well-built
/// binary.
pub fn case_pack() -> Result<CasePack<'static>, ScudError> {
    CasePack::from_scud_bytes(CASE_TR_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "tr";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_case::CaseEngine;

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = case_pack().unwrap();
        assert_eq!(pack.locale(), "tr");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn turkish_dotless_i_lowercased() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_lower("ISTANBUL", "tr"), "ıstanbul");
    }

    #[test]
    fn turkish_dotted_i_uppercased() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_upper("istanbul", "tr"), "İSTANBUL");
    }

    #[test]
    fn pack_bytes_are_small() {
        assert!(
            CASE_TR_SCUD.len() < 1024,
            "case-tr.scud grew unexpectedly: {} bytes",
            CASE_TR_SCUD.len(),
        );
    }
}
