//! WIT-i18n case-mapping SCUD pack for German.
//!
//! Exposes the compiled `case-de.scud` blob ([`CASE_DE_SCUD`]) plus
//! [`case_pack`], a helper that wraps it as a
//! [`stringcheese_icu_case::CasePack`] ready to hand to a
//! [`stringcheese_icu_case::CaseEngine`].
//!
//! The SCUD blob is generated in [`build.rs`](../../../build.rs) from
//! a hand-verified CLDR-derived table and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 6 for the pack
//! shape and § 8.1 for Phase 1 / 6 progress notes.
//!
//! # Coverage
//!
//! * ASCII a-z ↔ A-Z (simple lower / upper / fold)
//! * Latin-1 supplement (À-Þ ↔ à-þ except × ÷)
//! * German ß (full upper → `SS`, full fold → `ss`) plus capital
//!   sharp S ẞ (simple lower → ß, full fold → `ss`).
//!
//! German has no locale-specific case-mapping tailoring (unlike
//! Turkish's dotted / dotless-I). The pack's presence over the default
//! `char::to_lowercase` / `char::to_uppercase` fallback is about
//! **uniform pack-hit ratios** — every German letter resolves through
//! the pack rather than falling through to Rust's built-in tables.

use stringcheese_icu_case::{CasePack, ScudError};

/// The compiled case-mapping SCUD pack for German.
///
/// Generated at build time and embedded via `include_bytes!`; the
/// exact byte count is available at runtime as `CASE_DE_SCUD.len()`.
pub const CASE_DE_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/case-de.scud"));

/// Wrap [`CASE_DE_SCUD`] as a [`CasePack`] ready to feed to a
/// [`stringcheese_icu_case::CaseEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails validation
/// — a defensive check against a corrupt build artifact. In practice
/// this call succeeds on every well-built binary.
pub fn case_pack() -> Result<CasePack<'static>, ScudError> {
    CasePack::from_scud_bytes(CASE_DE_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "de";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_case::{CaseEngine, FoldMode};

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = case_pack().unwrap();
        assert_eq!(pack.locale(), "de");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn ascii_upper_and_lower() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_upper("hallo", "de"), "HALLO");
        assert_eq!(engine.to_lower("HALLO", "de"), "hallo");
    }

    #[test]
    fn german_umlauts_roundtrip() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_upper("mädchen", "de"), "MÄDCHEN");
        assert_eq!(engine.to_lower("MÜNCHEN", "de"), "münchen");
        assert_eq!(engine.to_upper("brötchen", "de"), "BRÖTCHEN");
    }

    #[test]
    fn sharp_s_expands_to_ss() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_upper("straße", "de"), "STRASSE");
        assert_eq!(engine.fold("Straße", FoldMode::Full), "strasse");
    }

    #[test]
    fn capital_sharp_s_lowers_to_sharp_s() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        // ẞ (U+1E9E) → ß (U+00DF) under simple lowercasing.
        assert_eq!(engine.to_lower("\u{1E9E}", "de"), "\u{00DF}");
    }

    #[test]
    fn pack_bytes_are_small() {
        assert!(
            CASE_DE_SCUD.len() < 2 * 1024,
            "case-de.scud grew unexpectedly: {} bytes",
            CASE_DE_SCUD.len()
        );
    }
}
