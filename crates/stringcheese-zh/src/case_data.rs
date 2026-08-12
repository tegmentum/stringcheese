//! WIT-i18n case-mapping SCUD pack for Chinese.
//!
//! Exposes the compiled `case-zh.scud` blob ([`CASE_ZH_SCUD`]) plus
//! [`case_pack`], a helper that wraps it as a
//! [`stringcheese_icu_case::CasePack`] ready to hand to a
//! [`stringcheese_icu_case::CaseEngine`].
//!
//! # Coverage
//!
//! * ASCII a-z ↔ A-Z (simple lower / upper / fold) — Chinese text
//!   commonly interleaves Latin loanwords and product names; the
//!   pack-hit path covers those deterministically.
//! * German ß / ẞ expansions — uniform composed-engine behaviour.
//!
//! # Han characters — no-op by design
//!
//! Han characters (CJK Unified Ideographs and extensions) have no
//! case. The pack does **not** list them; queries on Han fall
//! through the pack lookup and land on Rust's `char::to_lowercase`
//! / `char::to_uppercase` fallback, which returns Han scalars
//! unchanged. The `han_upper_and_lower_are_identity` test in
//! `case_golden_zh.rs` asserts this behaviour.

use stringcheese_icu_case::{CasePack, ScudError};

/// The compiled case-mapping SCUD pack for Chinese.
pub const CASE_ZH_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/case-zh.scud"));

/// Wrap [`CASE_ZH_SCUD`] as a [`CasePack`] ready to feed to a
/// [`stringcheese_icu_case::CaseEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn case_pack() -> Result<CasePack<'static>, ScudError> {
    CasePack::from_scud_bytes(CASE_ZH_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "zh";

#[cfg(test)]
mod tests {
    use super::*;
    use stringcheese_icu_case::CaseEngine;

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = case_pack().unwrap();
        assert_eq!(pack.locale(), "zh");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn ascii_upper_and_lower_via_zh_pack() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        assert_eq!(engine.to_upper("hello", "zh"), "HELLO");
        assert_eq!(engine.to_lower("HELLO", "zh"), "hello");
    }

    #[test]
    fn han_is_identity_under_case_ops() {
        let engine = CaseEngine::new(alloc::vec![case_pack().unwrap()]);
        // 你好世界 — "Hello world" in Chinese. Han characters have
        // no case; upper and lower are both identity.
        assert_eq!(engine.to_upper("\u{4F60}\u{597D}", "zh"), "\u{4F60}\u{597D}");
        assert_eq!(engine.to_lower("\u{4F60}\u{597D}", "zh"), "\u{4F60}\u{597D}");
    }

    #[test]
    fn pack_bytes_are_small() {
        assert!(
            CASE_ZH_SCUD.len() < 2 * 1024,
            "case-zh.scud grew unexpectedly: {} bytes",
            CASE_ZH_SCUD.len()
        );
    }
}
