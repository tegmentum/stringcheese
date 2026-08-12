//! WIT-i18n case-mapping SCUD pack for Russian.
//!
//! Exposes the compiled `case-ru.scud` blob ([`CASE_RU_SCUD`]) plus
//! [`case_pack`], a helper that wraps it as a
//! [`stringcheese_icu_case::CasePack`] ready to hand to a
//! [`stringcheese_icu_case::CaseEngine`].
//!
//! # Coverage
//!
//! * ASCII a-z ↔ A-Z (simple lower / upper / fold)
//! * Modern Russian alphabet U+0410..=U+042F ↔ U+0430..=U+044F
//!   (А..Я / а..я), 32 pairs.
//! * Ё ↔ ё (U+0401 ↔ U+0451) — the one irregular Cyrillic case
//!   pair (non-adjacent codepoints).
//! * German ß / ẞ expansions — belt-and-braces for composed-engine
//!   behaviour.
//!
//! Russian has no locale-specific case-mapping tailoring; the
//! Cyrillic upper/lower rules match default Unicode.

use stringcheese_icu_case::{CasePack, ScudError};

/// The compiled case-mapping SCUD pack for Russian.
pub const CASE_RU_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/case-ru.scud"));

/// Wrap [`CASE_RU_SCUD`] as a [`CasePack`] ready to feed to a
/// [`stringcheese_icu_case::CaseEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn case_pack() -> Result<CasePack<'static>, ScudError> {
    CasePack::from_scud_bytes(CASE_RU_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "ru";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_case::CaseEngine;

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = case_pack().unwrap();
        assert_eq!(pack.locale(), "ru");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn russian_alphabet_upper_lower() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_lower("МОСКВА", "ru"), "москва");
        assert_eq!(engine.to_upper("санкт-петербург", "ru"), "САНКТ-ПЕТЕРБУРГ");
    }

    #[test]
    fn yo_letter_roundtrip() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        // Ё ↔ ё — the one irregular Cyrillic case pair.
        assert_eq!(engine.to_lower("Ё", "ru"), "ё");
        assert_eq!(engine.to_upper("ё", "ru"), "Ё");
        assert_eq!(engine.to_lower("ЁЖИК", "ru"), "ёжик");
    }

    #[test]
    fn pack_bytes_are_small() {
        assert!(
            CASE_RU_SCUD.len() < 4 * 1024,
            "case-ru.scud grew unexpectedly: {} bytes",
            CASE_RU_SCUD.len()
        );
    }
}
