//! WIT-i18n collation SCUD pack for Chinese.
//!
//! Exposes the compiled `collation-zh.scud` blob
//! ([`COLLATION_ZH_SCUD`]) plus [`collation_pack`], a helper that
//! wraps it as a [`stringcheese_icu_collation::CollationPack`]
//! ready to hand to a
//! [`stringcheese_icu_collation::CollationEngine`].
//!
//! # Stroke-based ordering scaffold (CLDR `zh` `standard`)
//!
//! Ships a **starter subset** of ~230 common CJK Ideographs paired
//! with their stroke count, encoded as
//! `stringcheese_scud::SECT_PRIMARY_OVERRIDES` rows. Weight
//! formula: `1000 + stroke_count * 100 + within_stroke_index`, so
//! shipped characters sort by stroke count first and then by
//! codepoint within the same stroke bucket. Characters outside the
//! shipped table fall through to codepoint order as an
//! approximation.
//!
//! The engine's existing primary-override compare path handles the
//! new data — no `CollationEngine` code change was needed to light
//! up stroke ordering for the shipped subset.
//!
//! # Phase 2 deferrals
//!
//! * **Full stroke dataset** — the ~20 000 CJK Ideograph glyphs
//!   beyond the shipped starter set are a data-only follow-up.
//! * **Pinyin (`zh-u-co-pinyin`) collation** — CLDR's `pinyin`
//!   variant needs a ~40 000-entry Han → pinyin table plus tone
//!   handling. Still deferred.
//!
//! # Coverage
//!
//! * Stroke-ordered primary weights for ~230 common CJK Ideographs.
//! * DUCET root ordering for anything not in the starter set
//!   (delegated to `feruca` via `stringcheese-collate::UcaCollator`
//!   with the primary-override approximation for un-shipped Han).
//! * German ß / ẞ expansions.
//! * Default strength tertiary.

use stringcheese_icu_collation::{CollationPack, ScudError};

/// The compiled collation SCUD pack for Chinese.
pub const COLLATION_ZH_SCUD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/collation-zh.scud"));

/// Wrap [`COLLATION_ZH_SCUD`] as a [`CollationPack`] ready to feed
/// to a [`stringcheese_icu_collation::CollationEngine`].
///
/// # Errors
///
/// Returns a [`ScudError`] if the embedded SCUD blob fails
/// validation.
pub fn collation_pack() -> Result<CollationPack<'static>, ScudError> {
    CollationPack::from_scud_bytes(COLLATION_ZH_SCUD)
}

/// The BCP 47 locale tag associated with this pack.
pub const LOCALE: &str = "zh";

#[cfg(test)]
mod tests {
    use super::*;
    use core::cmp::Ordering;
    use stringcheese_icu_collation::{CollationEngine, CollationStrength};

    #[test]
    fn pack_loads_and_reports_locale() {
        let pack = collation_pack().unwrap();
        assert_eq!(pack.locale(), "zh");
        assert_eq!(pack.cldr_version(), "44.1");
    }

    #[test]
    fn engine_orders_ascii_words_under_zh() {
        let engine = CollationEngine::new(alloc::vec![collation_pack().unwrap()]);
        assert_eq!(
            engine.compare("apple", "banana", "zh", CollationStrength::Tertiary),
            Ordering::Less,
        );
    }

    #[test]
    fn pack_bytes_stay_within_scaffold_budget() {
        // The stroke-based scaffold ships ~230 CJK entries as 16-byte
        // primary-override records plus the tiny expansion +
        // options blob. Budget: 8 KiB while the starter set is small;
        // a data-only follow-up wave will grow this into the
        // ~200 KiB range typical of full ICU-style CJK ordering
        // data, at which point this guard needs re-baselining.
        assert!(
            COLLATION_ZH_SCUD.len() < 8 * 1024,
            "collation-zh.scud grew beyond the stroke-scaffold budget: {} bytes",
            COLLATION_ZH_SCUD.len()
        );
    }
}
