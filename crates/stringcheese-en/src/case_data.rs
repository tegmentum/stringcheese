//! WIT-i18n case-mapping SCUD pack for English.
//!
//! Exposes the compiled `case-en.scud` blob (`CASE_EN_SCUD`) plus
//! [`case_pack`], a helper that wraps it as a
//! [`stringcheese_icu_case::CasePack`] ready to hand to a
//! [`stringcheese_icu_case::CaseEngine`].
//!
//! The SCUD blob is generated in [`build.rs`](../../../build.rs) from a
//! hand-verified CLDR-derived table and embedded here via
//! `include_bytes!`. See `docs/design/wit-i18n.md` § 6 for the pack
//! shape and the reason this crate does not vendor a full CLDR dump.
//!
//! # Coverage
//!
//! * ASCII a-z ↔ A-Z (simple lower / upper / fold)
//! * Latin-1 supplement (À-Þ ↔ à-þ except × ÷)
//! * Common Latin Extended-A letters
//! * German ß (full upper → "SS", full fold → "ss") and capital
//!   sharp S ẞ (simple lower → ß)
//! * Ligatures Œ/œ, Æ/æ
//!
//! Locale-specific tailorings (Turkish dotted / dotless-I, Lithuanian
//! dot-above, Dutch `ij` titlecasing) live in the corresponding
//! per-locale pack, not here.

use stringcheese_icu_case::{CasePack, ScudError};

/// The compiled case-mapping SCUD pack for English.
///
/// Generated at build time and embedded via `include_bytes!`; the
/// exact byte count is available at runtime as `CASE_EN_SCUD.len()`.
pub const CASE_EN_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/case-en.scud"));

/// Wrap [`CASE_EN_SCUD`] as a [`CasePack`] ready to feed to a
/// [`stringcheese_icu_case::CaseEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails validation
/// — a defensive check against a corrupt build artifact. In practice
/// this call succeeds on every well-built binary.
pub fn case_pack() -> Result<CasePack<'static>, ScudError> {
    CasePack::from_scud_bytes(CASE_EN_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "en";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_case::{CaseEngine, FoldMode};

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = case_pack().unwrap();
        assert_eq!(pack.locale(), "en");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn ascii_upper_and_lower() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_upper("hello", "en"), "HELLO");
        assert_eq!(engine.to_lower("HELLO", "en"), "hello");
    }

    #[test]
    fn german_sharp_s_full_upper() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_upper("straße", "en"), "STRASSE");
    }

    #[test]
    fn german_sharp_s_full_fold() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.fold("Straße", FoldMode::Full), "strasse");
    }

    #[test]
    fn latin1_supplement_lowercased() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_lower("ÉCOLE", "en"), "école");
        assert_eq!(engine.to_upper("école", "en"), "ÉCOLE");
    }

    #[test]
    fn pack_bytes_are_small() {
        // The pack should be well under 4 KiB — a marker that we did
        // not accidentally check in a real CLDR dump. Adjust upward
        // deliberately if a real coverage expansion pushes past this.
        assert!(
            CASE_EN_SCUD.len() < 4 * 1024,
            "case-en.scud grew unexpectedly: {} bytes",
            CASE_EN_SCUD.len()
        );
    }
}
