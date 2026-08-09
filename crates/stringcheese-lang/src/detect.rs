//! [`LanguageDetector`] — the pluggable natural-language identification
//! trait.
//!
//! # Non-goal: silent detection
//!
//! Detection MUST NOT run inside routine string operations. Short strings
//! (names, addresses, SKUs, ER fields, single words like `"resume"` or
//! `"Roma"`) are systematically poor detection inputs, and silently
//! dispatching normalization / tokenization / stemming through a
//! detected language would introduce hidden compute cost and
//! nondeterminism for the pathological cases every real corpus contains.
//!
//! The intended layering is explicit:
//!
//! 1. **Explicit language** — `english.stem(text)`,
//!    `registry::language("de").normalize(text)`. Deterministic, fastest,
//!    preferred whenever the language is known.
//! 2. **Detected language** — the caller runs
//!    [`LanguageDetector::detect`] itself, inspects the
//!    [`LanguagePrediction`], and dispatches to the appropriate pack.
//! 3. **Auto convenience** — higher-level helpers explicitly named
//!    `..._auto` (or configured with `Language::Detect`) that carry the
//!    detection call in their contract. Never a hidden fall-through in
//!    an operation the caller didn't ask to detect.
//!
//! # Pluggable backends
//!
//! The trait is deliberately backend-neutral. StringCheese ships one
//! implementation (via an opt-in `stringcheese-lang-detect-whatlang`
//! adapter crate); callers can plug in CLD3, fastText, a hand-rolled
//! character n-gram model, or a WebAssembly component that speaks the
//! same trait shape without touching the rest of the toolkit.
//!
//! # WASM component contract
//!
//! Downstream WASM-component authors expose detection through a WIT
//! interface that maps 1:1 onto this trait — a `detect(text) ->
//! prediction` export, with `prediction` carrying the same fields as
//! [`LanguagePrediction`]. Keeping the trait synchronous and
//! non-generic keeps the WIT surface trivial.

use alloc::string::String;

/// A single language-detection result.
///
/// The [`bcp47`](Self::bcp47) field is the BCP-47 primary language
/// subtag callers should pass to
/// [`registry::language`](crate::registry::language). The
/// [`script`](Self::script) field is the ISO 15924 script code the
/// detector inferred (`"Latn"`, `"Cyrl"`, `"Arab"`, …); useful when a
/// language is ambiguous across scripts (Serbian Latin vs. Cyrillic).
///
/// [`confidence`](Self::confidence) is in the range `[0.0, 1.0]`;
/// [`reliable`](Self::reliable) is the detector's own opinion about
/// whether the prediction is trustworthy — a signal for callers who
/// prefer to bail out on unreliable predictions rather than reason
/// about a raw threshold.
#[derive(Clone, Debug, PartialEq)]
pub struct LanguagePrediction {
    /// BCP-47 primary language subtag (`"en"`, `"de"`, `"ja"`, …).
    ///
    /// This is the field callers should feed to
    /// [`registry::language`](crate::registry::language) — no fallback
    /// walk required, since the primary subtag is already canonical.
    pub bcp47: String,

    /// Human-readable name for the language (English, e.g. `"English"`,
    /// `"German"`).
    pub name: String,

    /// ISO 15924 script code (`"Latn"`, `"Cyrl"`, `"Arab"`, `"Hans"`,
    /// `"Hant"`, `"Hang"`, `"Jpan"`, …).
    pub script: String,

    /// Detection confidence, in `[0.0, 1.0]`. Exact interpretation is
    /// backend-specific — callers threshold this value against their
    /// own tolerance rather than treating it as a probability.
    pub confidence: f64,

    /// The detector's own opinion about whether the prediction is
    /// trustworthy. Callers that prefer a binary signal over a raw
    /// threshold check this field; the backend applies whatever
    /// heuristics (input length, secondary-candidate margin, …) it
    /// considers appropriate.
    pub reliable: bool,
}

/// The natural-language identification contract.
///
/// Implementations return `None` when the input carries no reliable
/// language signal (empty text, digit-only strings, mixed-script
/// strings the backend refuses to classify). Callers should treat
/// `None` as "the backend declined", never as "the language is
/// unsupported".
///
/// The trait is object-safe — callers routinely hold
/// `&'static dyn LanguageDetector`.
pub trait LanguageDetector: Send + Sync {
    /// Detect the language of `text`.
    ///
    /// Returns `None` when the input is unclassifiable. Otherwise
    /// returns a [`LanguagePrediction`] whose
    /// [`bcp47`](LanguagePrediction::bcp47) subtag can be fed straight
    /// to [`registry::language`](crate::registry::language) for the
    /// matching pack.
    fn detect(&self, text: &str) -> Option<LanguagePrediction>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    struct ConstDetector(&'static str);

    impl LanguageDetector for ConstDetector {
        fn detect(&self, text: &str) -> Option<LanguagePrediction> {
            if text.is_empty() {
                return None;
            }
            Some(LanguagePrediction {
                bcp47: self.0.to_string(),
                name: "TestLang".to_string(),
                script: "Latn".to_string(),
                confidence: 1.0,
                reliable: true,
            })
        }
    }

    #[test]
    fn detector_returns_none_on_empty_input() {
        let d = ConstDetector("en");
        assert!(d.detect("").is_none());
    }

    #[test]
    fn detector_returns_prediction_on_nonempty_input() {
        let d = ConstDetector("en");
        let p = d
            .detect("hello")
            .expect("nonempty input yields a prediction");
        assert_eq!(p.bcp47, "en");
        assert_eq!(p.script, "Latn");
        // Clippy's `float_cmp` lint fires on direct `==` for `f64`;
        // the test detector sets confidence to a literal 1.0, so an
        // exact-equality check is precisely what we want here — a
        // tolerance would obscure the assertion.
        assert!((p.confidence - 1.0).abs() < f64::EPSILON);
        assert!(p.reliable);
    }

    #[test]
    fn trait_is_object_safe() {
        let d: &dyn LanguageDetector = &ConstDetector("de");
        assert_eq!(
            d.detect("guten tag").map(|p| p.bcp47),
            Some("de".to_string())
        );
    }
}
