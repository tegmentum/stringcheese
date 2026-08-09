//! # Tier-2 lingua wrapper (per-language, WIT-shaped)
//!
//! Wraps [`lingua`] behind the shared `tegmentum:lang-detect@0.2.0`
//! WIT contract, with per-language Cargo features selecting which
//! language models get embedded. A build with `--features en,de`
//! compiles in English + German models only (~9 MB of FST data)
//! and produces a detector that scores exactly those two languages.
//!
//! ## Position in the stack
//!
//! Tier 2 sits below tier 1 in the detection cascade:
//!
//! ```text
//! text
//!   │
//!   ▼
//! script-detect (~5 KB, always resident)                       [Tier 0]
//!   │
//!   ▼
//! whatlang-{script}.wasm (~60 KB, lazy per script)             [Tier 1]
//!   │  { lang: "de", confidence: 0.72 }
//!   ▼
//! if confidence >= caller_threshold: use as-is → done
//! else (opt-in escalation):
//!   ▼
//! lingua-{lang}.wasm (~1–2 MB gzipped, lazy per language)      [Tier 2]
//!   │  { lang: "de", confidence: 0.98 }
//!   ▼
//! host dispatches into the matching stringcheese-lang-<code> pack
//! ```
//!
//! Escalation is **caller opt-in** — the host never fetches a tier-2
//! component unless the tier-1 result is below the caller's
//! confidence tolerance. That preserves the browser-friendly bundle
//! budget: pages that don't need high-confidence detection never
//! pay lingua's MB-per-language cost.
//!
//! ## Why lingua is the right backend here
//!
//! Lingua ships per-language crates natively — its
//! `lingua-<lang>-language-model` crates each embed FST-encoded
//! ngram tables via `include_dir!`. Enabling a language feature
//! pulls exactly that crate; disabled languages contribute zero to
//! the binary. This maps 1:1 onto per-language WASM components,
//! with no fork or sharding work required on our side (in contrast
//! to the whatlang tier-1 crate, which required a hard shard of a
//! monolithic `profiles.rs`).
//!
//! ## Feature contract
//!
//! Lingua's `LanguageDetectorBuilder` requires **at least two
//! languages** to instantiate. The convention here is that every
//! non-English pack ships `<lang> + en` — English serves as the
//! universal comparator, so "is this really German?" always has a
//! well-defined pair to score against. Callers who need
//! single-language confidence use [`compute_confidence`], which
//! wraps `lingua::LanguageDetector::compute_language_confidence`
//! and returns a bare score for the target language regardless of
//! what else is enabled.
//!
//! ## Sizes (approximate)
//!
//! Per-language crates embed 4–6 MB of FST data. A tier-2 component
//! carrying `<lang> + en` is roughly 8–11 MB of embedded data
//! before WASM compilation; after `wasm-opt -Oz` + strip + gzip,
//! expect 1–2 MB over the wire. Compared to whatlang's ~60 KB per
//! script, lingua costs 20–40× more per component for meaningfully
//! better accuracy — hence the opt-in escalation model.

#![deny(unsafe_code)]

// WIT bindings + Guest wiring only compile on wasm targets so
// native builds stay lingua-only and don't drag wit-bindgen-rt
// into `cargo test` or downstream Rust hosts.
#[cfg(target_family = "wasm")]
#[allow(
    unsafe_code,
    unsafe_op_in_unsafe_fn,
    missing_docs,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::restriction
)]
mod bindings;
#[cfg(target_family = "wasm")]
#[allow(unsafe_code, unsafe_op_in_unsafe_fn)]
mod wit;

use lingua::{Language, LanguageDetector, LanguageDetectorBuilder};

/// A single language-detection result. Mirrors the `detection` record
/// in the WIT interface so callers can trivially map into the WIT
/// world when compiled as a component.
///
/// See `wit/lang-detect.wit` at the workspace root for canonical
/// field documentation.
#[derive(Clone, Debug, PartialEq)]
pub struct Detection {
    /// ISO 639-3 language code (`"eng"`, `"deu"`, …).
    pub lang: String,
    /// English name of the language (`"English"`, `"German"`).
    pub lang_name: String,
    /// ISO 15924 script code (`"Latn"`, `"Cyrl"`, …).
    pub script: String,
    /// Detection confidence in `[0.0, 1.0]`.
    pub confidence: f64,
    /// Lingua does not surface a separate "reliable" heuristic; we
    /// derive this from `confidence >= 0.5`, which is lingua's own
    /// default threshold for `detect_language_of` returning `Some`.
    pub reliable: bool,
}

/// Capability record for this backend — matches the WIT
/// `capabilities` record 1:1. Cheap to construct; callers cache it
/// at load time.
#[derive(Clone, Debug, PartialEq)]
pub struct Capabilities {
    /// Backend identifier — always `"lingua"` for this crate.
    pub backend: &'static str,
    /// Semver of this crate.
    pub version: &'static str,
    /// ISO 639-3 codes the enabled language features cover.
    pub supports: Vec<String>,
    /// `true` — lingua exposes ranked per-candidate confidence via
    /// `compute_language_confidence_values`.
    pub can_rank: bool,
    /// `true` — every detector call is naturally constrained to the
    /// features enabled at build time; `detect_from` further
    /// intersects with the caller's runtime allowlist.
    pub can_constrain: bool,
    /// `true` — lingua's `detect_multiple_languages_of` gives real
    /// mixed-language span segmentation.
    pub can_segment: bool,
}

/// Report which backend + version this build is running against and
/// which languages it can distinguish.
#[must_use]
pub fn capabilities() -> Capabilities {
    Capabilities {
        backend: "lingua",
        version: env!("CARGO_PKG_VERSION"),
        supports: enabled_languages()
            .iter()
            .map(|l| iso_639_3(*l).to_string())
            .collect(),
        can_rank: true,
        can_constrain: true,
        can_segment: true,
    }
}

/// Baseline single-best-guess detection. Returns `None` when lingua
/// can't reach its confidence threshold for any enabled language.
#[must_use]
pub fn detect(text: &str) -> Option<Detection> {
    let detector = build_detector(None)?;
    detector
        .detect_language_of(text)
        .map(|lang| to_detection(lang, detector.compute_language_confidence(text, lang)))
}

/// Constrained detection — considers only ISO 639-3 codes in `langs`
/// from the crate's already-enabled feature set. Codes not enabled
/// at compile time are silently dropped (we can't score what wasn't
/// compiled in).
#[must_use]
pub fn detect_from(text: &str, langs: &[&str]) -> Option<Detection> {
    let allow: Vec<Language> = langs
        .iter()
        .filter_map(|code| iso_639_3_to_language(code))
        .filter(|lang| enabled_languages().contains(lang))
        .collect();
    if allow.len() < 2 {
        // Lingua's builder requires ≥2 languages; a caller-supplied
        // list that doesn't intersect the compile-time set (or gives
        // only one intersecting entry) can't produce a detector.
        return None;
    }
    let detector = LanguageDetectorBuilder::from_languages(&allow).build();
    detector
        .detect_language_of(text)
        .map(|lang| to_detection(lang, detector.compute_language_confidence(text, lang)))
}

/// Ranked distribution across the enabled languages, ordered by
/// descending confidence.
#[must_use]
pub fn detect_ranked(text: &str) -> Vec<(String, f64)> {
    let Some(detector) = build_detector(None) else {
        return Vec::new();
    };
    detector
        .compute_language_confidence_values(text)
        .into_iter()
        .map(|(lang, conf)| (iso_639_3(lang).to_string(), conf))
        .collect()
}

/// Compute a bare confidence score for `language` against `text`. Does
/// NOT require the target language to be the winning candidate — this
/// is the "how much does this text look like `<lang>`?" query, useful
/// for tier-2 verification of a tier-1 guess.
///
/// Returns `None` when the language isn't compiled in for this build.
#[must_use]
pub fn compute_confidence(text: &str, iso_639_3_code: &str) -> Option<f64> {
    let language = iso_639_3_to_language(iso_639_3_code)?;
    if !enabled_languages().contains(&language) {
        return None;
    }
    build_detector(None).map(|d| d.compute_language_confidence(text, language))
}

// ---------------------------------------------------------------------
// Internal — detector construction and language routing
// ---------------------------------------------------------------------

fn build_detector(explicit_allow: Option<Vec<Language>>) -> Option<LanguageDetector> {
    let langs = explicit_allow.unwrap_or_else(|| enabled_languages().to_vec());
    if langs.len() < 2 {
        return None;
    }
    Some(LanguageDetectorBuilder::from_languages(&langs).build())
}

/// The `Language` values enabled by the crate's feature set.
///
/// Compile-time selection: every enabled feature contributes its
/// `Language` variant. The result is what lingua's detector considers
/// candidates for `detect_language_of`.
fn enabled_languages() -> &'static [Language] {
    // Build the enabled set once. Written with an explicit `vec![]`
    // literal so clippy doesn't fire `vec-init-then-push` for the
    // feature-gated pushes — the vec always starts populated by
    // whichever features are on. `#[allow(clippy::vec_init_then_push)]`
    // would be equivalent but noisier at each callsite.
    static ENABLED: std::sync::OnceLock<Vec<Language>> = std::sync::OnceLock::new();
    ENABLED.get_or_init(|| {
        let out: Vec<Language> = vec![
            #[cfg(feature = "en")]
            Language::English,
            #[cfg(feature = "de")]
            Language::German,
            #[cfg(feature = "fr")]
            Language::French,
            #[cfg(feature = "es")]
            Language::Spanish,
            #[cfg(feature = "it")]
            Language::Italian,
            #[cfg(feature = "pt")]
            Language::Portuguese,
            #[cfg(feature = "ja")]
            Language::Japanese,
            #[cfg(feature = "ru")]
            Language::Russian,
        ];
        out
    })
}

/// Map lingua's `Language` to ISO 639-3 — matches the codes the
/// tier-0 / tier-1 components emit, so a caller can carry a language
/// code through the tier cascade without translation.
///
/// The `Language` enum only carries the variants for the features
/// this build enabled, so the match arms are per-feature-gated to
/// stay exhaustive without referencing variants that don't exist.
/// The fallback delegates to lingua's own `iso_code_639_3` for any
/// language that gets added upstream faster than we tabulate it.
#[allow(unreachable_patterns)]
fn iso_639_3(lang: Language) -> &'static str {
    match lang {
        #[cfg(feature = "en")]
        Language::English => "eng",
        #[cfg(feature = "de")]
        Language::German => "deu",
        #[cfg(feature = "fr")]
        Language::French => "fra",
        #[cfg(feature = "es")]
        Language::Spanish => "spa",
        #[cfg(feature = "it")]
        Language::Italian => "ita",
        #[cfg(feature = "pt")]
        Language::Portuguese => "por",
        #[cfg(feature = "ja")]
        Language::Japanese => "jpn",
        #[cfg(feature = "ru")]
        Language::Russian => "rus",
        _ => lang_iso_639_3_fallback(lang),
    }
}

fn lang_iso_639_3_fallback(lang: Language) -> &'static str {
    // lingua::Language::iso_code_639_3 returns an IsoCode639_3 whose
    // Display impl produces the 3-letter code. Leaked once per
    // never-tabulated variant — the enumerated set doesn't hit this
    // path, so leak volume is bounded.
    let code = format!("{}", lang.iso_code_639_3()).to_lowercase();
    Box::leak(code.into_boxed_str())
}

fn iso_639_3_to_language(code: &str) -> Option<Language> {
    match code {
        #[cfg(feature = "en")]
        "eng" => Some(Language::English),
        #[cfg(feature = "de")]
        "deu" => Some(Language::German),
        #[cfg(feature = "fr")]
        "fra" => Some(Language::French),
        #[cfg(feature = "es")]
        "spa" => Some(Language::Spanish),
        #[cfg(feature = "it")]
        "ita" => Some(Language::Italian),
        #[cfg(feature = "pt")]
        "por" => Some(Language::Portuguese),
        #[cfg(feature = "ja")]
        "jpn" => Some(Language::Japanese),
        #[cfg(feature = "ru")]
        "rus" => Some(Language::Russian),
        _ => None,
    }
}

// Feature-gated arms mean the fallback `_` is unreachable under some
// feature sets but essential under others. Silence the lint at the
// function level so we don't have to hand-craft the wildcard's
// applicability per feature combination.
#[allow(unreachable_patterns)]
fn iso_15924(lang: Language) -> &'static str {
    match lang {
        #[cfg(any(
            feature = "en",
            feature = "de",
            feature = "fr",
            feature = "es",
            feature = "it",
            feature = "pt"
        ))]
        _ if is_latin(lang) => "Latn",
        #[cfg(feature = "ru")]
        Language::Russian => "Cyrl",
        #[cfg(feature = "ja")]
        Language::Japanese => "Jpan",
        _ => "Zzzz",
    }
}

#[cfg(any(
    feature = "en",
    feature = "de",
    feature = "fr",
    feature = "es",
    feature = "it",
    feature = "pt"
))]
#[allow(unreachable_patterns, clippy::match_like_matches_macro)]
fn is_latin(lang: Language) -> bool {
    match lang {
        #[cfg(feature = "en")]
        Language::English => true,
        #[cfg(feature = "de")]
        Language::German => true,
        #[cfg(feature = "fr")]
        Language::French => true,
        #[cfg(feature = "es")]
        Language::Spanish => true,
        #[cfg(feature = "it")]
        Language::Italian => true,
        #[cfg(feature = "pt")]
        Language::Portuguese => true,
        _ => false,
    }
}

fn to_detection(lang: Language, confidence: f64) -> Detection {
    Detection {
        lang: iso_639_3(lang).to_string(),
        lang_name: format!("{lang:?}"),
        script: iso_15924(lang).to_string(),
        confidence,
        reliable: confidence >= 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(feature = "en", feature = "de"))]
    #[test]
    fn detects_english_when_en_and_de_are_enabled() {
        let d = detect(
            "This is a longer sample of the English language for detection to work reliably.",
        )
        .expect("english text should classify with en+de enabled");
        assert_eq!(d.lang, "eng");
        assert_eq!(d.script, "Latn");
        assert!(
            d.confidence > 0.5,
            "confidence {} should exceed 0.5",
            d.confidence
        );
    }

    #[cfg(all(feature = "en", feature = "de"))]
    #[test]
    fn detects_german_when_en_and_de_are_enabled() {
        let d = detect("Dies ist ein längeres Beispiel der deutschen Sprache für die Erkennung.")
            .expect("german text should classify with en+de enabled");
        assert_eq!(d.lang, "deu");
    }

    #[cfg(all(feature = "en", feature = "de"))]
    #[test]
    fn ranked_returns_enabled_languages_in_descending_order() {
        let ranked = detect_ranked(
            "This is a longer sample of the English language for detection to work reliably.",
        );
        assert!(
            !ranked.is_empty(),
            "ranking includes every enabled language"
        );
        for window in ranked.windows(2) {
            assert!(window[0].1 >= window[1].1, "descending confidence");
        }
    }

    #[cfg(all(feature = "en", feature = "de"))]
    #[test]
    fn compute_confidence_scores_arbitrary_language() {
        let english_conf = compute_confidence(
            "This is a longer sample of the English language for detection to work reliably.",
            "eng",
        )
        .expect("eng is enabled");
        assert!(english_conf > 0.5);
    }

    // Runs under a build that has `en` + `de` but NOT `ja`. Under
    // `--all-features` this test's precondition doesn't hold; the
    // negative-space assertion is inherently feature-set-specific.
    #[cfg(all(feature = "en", feature = "de", not(feature = "ja")))]
    #[test]
    fn compute_confidence_returns_none_for_disabled_language() {
        assert!(compute_confidence("hello", "jpn").is_none());
    }

    #[test]
    fn capabilities_advertises_backend() {
        let caps = capabilities();
        assert_eq!(caps.backend, "lingua");
        assert!(caps.can_rank);
        assert!(caps.can_constrain);
        assert!(caps.can_segment);
    }
}
