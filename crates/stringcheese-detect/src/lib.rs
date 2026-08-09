//! # Composed language-detection dispatcher
//!
//! Walks the StringCheese detection tier stack from tier 0 (Unicode-
//! block script classifier, always resident) through tier 1
//! (whatlang trigram detector, one shard per script) and, when the
//! caller demands higher confidence, tier 2 (lingua FST detector,
//! one shard per language). Each tier is opt-in via Cargo features
//! that pass through to the underlying per-tier crates, so a build
//! declares exactly which scripts and languages it wants.
//!
//! ## What this crate is
//!
//! The **Rust host analog** of the browser-side lazy-loading
//! dispatcher documented in `docs/design/language-architecture.md`
//! §4. The design commitments are the same:
//!
//! - **Explicit, never silent.** Callers construct a [`Dispatcher`]
//!   and call [`Dispatcher::detect`]; detection never sneaks into
//!   an ordinary string-processing method.
//! - **Escalation is threshold-based.** Below the caller's
//!   confidence tolerance, the dispatcher fetches the tier-2 answer
//!   for the tier-1 candidate; above it, tier 1's answer stands.
//! - **Pay for what you enable.** Only the enabled per-script /
//!   per-language tier features get compiled in. A build with
//!   `tier1-latn` alone carries the ~1.4 MB Latin whatlang shard
//!   and nothing else.
//!
//! ## Not this crate
//!
//! The **browser-side** dispatcher is a JavaScript concern (the
//! host loads WASM components lazily via `wit-js-bindgen` +
//! `wasmos`). That side lives outside Rust. This crate is what a
//! Rust host (a server, a CLI, a native app) uses.
//!
//! ## Example
//!
//! ```
//! use stringcheese_detect::Dispatcher;
//!
//! let dispatcher = Dispatcher::default(); // 0.75 threshold
//! if let Some(det) = dispatcher.detect("This is English text.") {
//!     assert_eq!(det.bcp47, "eng");
//!     // `det.tier` names which tier produced this answer.
//! }
//! ```

#![deny(unsafe_code)]

// ---------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------

/// Which tier produced a detection result.
///
/// Useful when the caller wants to log or bill differently based
/// on which detector fired — tier 0 is essentially free, tier 1 is
/// cheap, tier 2 is heavy.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Tier {
    /// Tier 0 — `stringcheese-detect-script`. Answers script only;
    /// `bcp47` is empty when this tier is the sole producer.
    Script,
    /// Tier 1 — `stringcheese-detect-whatlang`, one shard per script.
    Whatlang,
    /// Tier 2 — `stringcheese-detect-lingua`, one shard per language.
    Lingua,
}

/// One detection result.
///
/// Same field shape as `LanguagePrediction` in `stringcheese-lang`
/// (BCP-47, name, script, confidence, reliable), plus the [`Tier`]
/// annotation naming which detector produced it.
#[derive(Clone, Debug, PartialEq)]
pub struct Detection {
    /// ISO 639-3 language code (`"eng"`, `"deu"`, …). Empty when
    /// tier 0 was the sole producer and only classified script.
    pub bcp47: String,
    /// English name of the language. Empty when `bcp47` is empty.
    pub name: String,
    /// ISO 15924 script code (`"Latn"`, `"Cyrl"`, …).
    pub script: String,
    /// Detection confidence in `[0.0, 1.0]`.
    pub confidence: f64,
    /// The detector's own opinion about whether the prediction is
    /// trustworthy. `false` when confidence is below the caller's
    /// threshold AND no tier-2 escalation was available.
    pub reliable: bool,
    /// Which tier produced this result.
    pub tier: Tier,
}

// ---------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------

/// The tier-walking dispatcher.
///
/// Construct once with a confidence threshold; reuse across
/// detection calls. Cheap — carries only the threshold value.
#[derive(Copy, Clone, Debug)]
pub struct Dispatcher {
    /// Below this confidence, tier 1 will attempt to escalate to
    /// tier 2 if a matching language feature is compiled in.
    pub threshold: f64,
    /// When `true`, escalate to tier 2 whenever a matching language
    /// feature is enabled — even when tier 1's confidence is
    /// already above the threshold. Useful for callers that always
    /// want the higher-quality answer when it's available.
    pub always_escalate: bool,
}

impl Default for Dispatcher {
    fn default() -> Self {
        // 0.75 is whatlang's own "reliable" boundary — under it, the
        // tier-1 detector considers the prediction uncertain and
        // upstream tooling typically flags it. Matches what the
        // tier-1 crate reports as `reliable: false`.
        Self {
            threshold: 0.75,
            always_escalate: false,
        }
    }
}

impl Dispatcher {
    /// Construct with a specific threshold.
    #[must_use]
    pub fn with_threshold(threshold: f64) -> Self {
        Self {
            threshold,
            always_escalate: false,
        }
    }

    /// Flip the always-escalate bit; returns `self` for
    /// builder-style composition.
    #[must_use]
    pub fn always_escalate(mut self) -> Self {
        self.always_escalate = true;
        self
    }

    /// Detect the language of `text` by walking the tier stack.
    ///
    /// Returns `None` when tier 0 can't classify the script (empty
    /// text, digits-only, punctuation-only). When tier 1 or tier 2
    /// features are absent, the result carries whatever the highest
    /// available tier produced — tier 0's script alone is a valid
    /// result when no language-detection tiers are compiled in.
    #[must_use]
    pub fn detect(&self, text: &str) -> Option<Detection> {
        // Tier 0 — always available; no feature gate.
        let script = stringcheese_detect_script::detect_script(text)?;

        // Baseline: return the script-only result if higher tiers
        // aren't compiled in or don't fire.
        let mut result = Detection {
            bcp47: String::new(),
            name: String::new(),
            script: script.to_string(),
            confidence: 1.0, // script classification is deterministic
            reliable: true,
            tier: Tier::Script,
        };

        // Tier 1 — whatlang, feature-gated per script. The
        // underlying crate's `detect()` already restricts to its
        // compiled allowlist; we just need to try it.
        #[cfg(feature = "tier1")]
        if let Some(t1) = tier1_detect(text) {
            #[cfg_attr(not(feature = "tier2"), allow(unused_variables))]
            let tier1_confidence = t1.confidence;
            result = Detection {
                bcp47: t1.lang,
                name: t1.lang_name,
                script: t1.script,
                confidence: t1.confidence,
                reliable: t1.reliable && t1.confidence >= self.threshold,
                tier: Tier::Whatlang,
            };

            // Tier 2 — lingua, feature-gated per language. Asks
            // lingua for its assessment of the tier-1 candidate
            // specifically (not "what language is this?" but "how
            // much does this look like the language tier 1 said?").
            //
            // **Requires ≥2 tier2 language features enabled** —
            // lingua's `LanguageDetectorBuilder::build()` errors
            // out below two languages. Convention: pair the target
            // language with `tier2-en` as the anchor. If the tier-2
            // builder can't materialise (e.g. only one language
            // enabled), the shim returns `None` and the dispatcher
            // keeps the tier-1 answer unchanged.
            #[cfg(feature = "tier2")]
            if self.always_escalate || tier1_confidence < self.threshold {
                if let Some(score) = tier2_confidence_for(text, &result.bcp47) {
                    // Lingua's confidence for the tier-1 candidate.
                    // Keep tier 1's bcp47/name/script; overwrite
                    // confidence with lingua's more accurate
                    // assessment.
                    result.confidence = score;
                    result.reliable = score >= self.threshold;
                    result.tier = Tier::Lingua;
                }
            }
        }

        Some(result)
    }
}

// ---------------------------------------------------------------------
// Tier-1 and tier-2 shims — feature-gated so the crate compiles
// against any subset of tier features.
// ---------------------------------------------------------------------

#[cfg(feature = "tier1")]
fn tier1_detect(text: &str) -> Option<stringcheese_detect_whatlang::Detection> {
    stringcheese_detect_whatlang::detect(text)
}

#[cfg(feature = "tier2")]
fn tier2_confidence_for(text: &str, bcp47_iso_639_3: &str) -> Option<f64> {
    // Ask lingua for its confidence in the tier-1 candidate
    // specifically. Uses `compute_confidence` (single-language
    // query) rather than `detect_from` (needs ≥2 languages by
    // lingua's builder contract). Returns None when the language
    // isn't compiled into tier 2 for this build — in which case
    // the caller keeps tier 1's answer as-is.
    stringcheese_detect_lingua::compute_confidence(text, bcp47_iso_639_3)
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatcher_default_has_a_threshold() {
        let d = Dispatcher::default();
        assert!(d.threshold > 0.0 && d.threshold <= 1.0);
        assert!(!d.always_escalate);
    }

    #[test]
    fn builder_flips_always_escalate() {
        let d = Dispatcher::default().always_escalate();
        assert!(d.always_escalate);
    }

    #[test]
    fn tier0_only_classifies_script_on_empty_language_signal() {
        // Digit-only text carries no language signal but tier 0
        // returns `None` (there's no script either). Empty input
        // returns None likewise.
        let d = Dispatcher::default();
        assert!(d.detect("").is_none());
        assert!(d.detect("12345").is_none());
    }

    #[cfg(feature = "tier1-latn")]
    #[test]
    fn latin_english_hits_tier1() {
        let d = Dispatcher::default();
        let det = d
            .detect("This is a longer sample of the English language for detection to work.")
            .expect("english text should classify");
        assert_eq!(det.script, "Latn");
        assert_eq!(det.bcp47, "eng");
        // Threshold is the default 0.75; whatlang's confidence on
        // this sample is comfortably above that.
        assert!(det.confidence >= 0.75);
        assert_eq!(det.tier, Tier::Whatlang);
    }

    // Requires ≥2 tier2 languages to satisfy lingua's builder
    // constraint. `tier2-en` + `tier2-de` is the pair every anchor
    // shape uses; a single-tier2-language build has no way to
    // instantiate a lingua detector and tier 2 stays a no-op.
    #[cfg(all(feature = "tier1-latn", feature = "tier2-en", feature = "tier2-de"))]
    #[test]
    fn always_escalate_reaches_tier2() {
        // Force escalation even when tier 1 is already confident;
        // the returned detection should come from tier 2.
        let d = Dispatcher::default().always_escalate();
        let det = d
            .detect(
                "This is a longer sample of the English language, chosen to give the tier-1 \
                 trigram detector a comfortably-above-threshold confidence.",
            )
            .expect("english text classifies");
        // Any positive test — tier 2 fired and reported a language.
        assert_eq!(det.tier, Tier::Lingua);
        assert_eq!(det.bcp47, "eng");
    }

    #[cfg(feature = "tier1-latn")]
    #[test]
    fn tier_annotation_marks_result_provenance() {
        let d = Dispatcher::default();
        // Something confidently English — tier 1 answers, no escalation.
        let det = d
            .detect("The quick brown fox jumps over the lazy dog many times.")
            .unwrap();
        assert_eq!(det.tier, Tier::Whatlang);
    }
}
