//! WIT-i18n case-mapping SCUD pack for French.
//!
//! Exposes the compiled `case-fr.scud` blob ([`CASE_FR_SCUD`]) plus
//! [`case_pack`], a helper that wraps it as a
//! [`stringcheese_icu_case::CasePack`] ready to hand to a
//! [`stringcheese_icu_case::CaseEngine`].
//!
//! The SCUD blob is generated in [`build.rs`](../../../build.rs) from
//! a hand-verified CLDR-derived table and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 6 for the pack
//! shape and § 8.1 for Phase 1 progress notes.
//!
//! # Coverage
//!
//! * ASCII a-z ↔ A-Z (simple lower / upper / fold)
//! * Latin-1 supplement (À-Þ ↔ à-þ except × ÷)
//! * Ÿ ↔ ÿ pair
//! * Ligatures Œ/œ, Æ/æ (both common in French)
//! * German ß (full upper → "SS", full fold → "ss") + capital sharp
//!   S ẞ (simple lower → ß). Shipped so a composed engine sees the
//!   expansion regardless of which pack the query resolves through
//!   first.
//!
//! French has no locale-specific case-mapping tailoring (unlike
//! Turkish's dotted / dotless-I). The pack's presence over the
//! default `char::to_lowercase` / `char::to_uppercase` fallback is
//! about **uniform pack-hit ratios** — every French letter resolves
//! through the pack rather than falling through to Rust's built-in
//! tables.

use stringcheese_icu_case::{CasePack, ScudError};

/// The compiled case-mapping SCUD pack for French.
///
/// Generated at build time and embedded via `include_bytes!`; the
/// exact byte count is available at runtime as `CASE_FR_SCUD.len()`.
pub const CASE_FR_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/case-fr.scud"));

/// Wrap [`CASE_FR_SCUD`] as a [`CasePack`] ready to feed to a
/// [`stringcheese_icu_case::CaseEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails validation
/// — a defensive check against a corrupt build artifact. In practice
/// this call succeeds on every well-built binary.
pub fn case_pack() -> Result<CasePack<'static>, ScudError> {
    CasePack::from_scud_bytes(CASE_FR_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "fr";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_case::{CaseEngine, FoldMode};

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = case_pack().unwrap();
        assert_eq!(pack.locale(), "fr");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn ascii_upper_and_lower() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_upper("bonjour", "fr"), "BONJOUR");
        assert_eq!(engine.to_lower("BONJOUR", "fr"), "bonjour");
    }

    #[test]
    fn french_accented_letters_roundtrip() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_lower("ÉCOLE", "fr"), "école");
        assert_eq!(engine.to_upper("école", "fr"), "ÉCOLE");
        assert_eq!(engine.to_lower("ÇA VA", "fr"), "ça va");
    }

    #[test]
    fn oe_ligature_expansion() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_upper("œuvre", "fr"), "ŒUVRE");
        assert_eq!(engine.to_lower("ŒIL", "fr"), "œil");
    }

    #[test]
    fn german_sharp_s_still_expands() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_upper("straße", "fr"), "STRASSE");
        assert_eq!(engine.fold("Straße", FoldMode::Full), "strasse");
    }

    #[test]
    fn pack_bytes_are_small() {
        assert!(
            CASE_FR_SCUD.len() < 4 * 1024,
            "case-fr.scud grew unexpectedly: {} bytes",
            CASE_FR_SCUD.len()
        );
    }
}
