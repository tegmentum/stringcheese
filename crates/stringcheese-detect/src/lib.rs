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

    // ---------------------------------------------------------------
    // Constructor / builder shape
    // ---------------------------------------------------------------

    #[test]
    fn with_threshold_sets_threshold_and_leaves_escalate_off() {
        let d = Dispatcher::with_threshold(0.42);
        assert!((d.threshold - 0.42).abs() < f64::EPSILON);
        assert!(!d.always_escalate);
    }

    #[test]
    fn with_threshold_accepts_zero_and_one_edges() {
        // The dispatcher doesn't validate the threshold — it's a
        // configuration knob, not a bounded scalar. Both zero and one
        // are perfectly cromulent thresholds meaning "any result is
        // reliable" and "only a perfect answer is reliable"
        // respectively.
        let zero = Dispatcher::with_threshold(0.0);
        let one = Dispatcher::with_threshold(1.0);
        assert!(zero.threshold.abs() < f64::EPSILON);
        assert!((one.threshold - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dispatcher_is_copy_and_reusable() {
        // Copy semantics matter — callers construct once and use
        // the dispatcher from many spots without wrapping in Arc.
        let d = Dispatcher::default();
        let clone: Dispatcher = d; // implicit Copy
        assert!((d.threshold - clone.threshold).abs() < f64::EPSILON);
    }

    #[test]
    fn always_escalate_is_idempotent_via_builder() {
        // Calling the builder twice should still leave the flag on
        // (no toggle behavior — the builder sets, doesn't flip).
        let d = Dispatcher::default().always_escalate().always_escalate();
        assert!(d.always_escalate);
    }

    // ---------------------------------------------------------------
    // Detection type basics
    // ---------------------------------------------------------------

    #[test]
    fn detection_is_clone_and_eq() {
        // Detection derives Clone + PartialEq — asserting the derive
        // still holds keeps callers who rely on equality checks safe.
        let a = Detection {
            bcp47: "eng".into(),
            name: "English".into(),
            script: "Latn".into(),
            confidence: 0.9,
            reliable: true,
            tier: Tier::Whatlang,
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn tier_is_copy_hash_eq() {
        // Tier is a small enum useful as a HashMap key for
        // billing/routing — verify the derives are intact.
        use core::hash::{Hash, Hasher};
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        Tier::Whatlang.hash(&mut h1);
        Tier::Whatlang.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
        assert_ne!(Tier::Whatlang, Tier::Lingua);
        assert_ne!(Tier::Script, Tier::Whatlang);
    }

    // ---------------------------------------------------------------
    // Tier 0 (script-only) dispatch — no tier features required
    // ---------------------------------------------------------------

    #[test]
    fn tier0_returns_none_for_whitespace_only() {
        let d = Dispatcher::default();
        assert!(d.detect("   ").is_none());
        assert!(d.detect("\t\n\r  ").is_none());
    }

    #[test]
    fn tier0_returns_none_for_punctuation_only() {
        let d = Dispatcher::default();
        assert!(d.detect("!!!???...,,,").is_none());
        assert!(d.detect("()[]{}<>").is_none());
    }

    #[test]
    fn tier0_returns_none_for_single_digit() {
        let d = Dispatcher::default();
        assert!(d.detect("7").is_none());
    }

    #[test]
    fn tier0_returns_none_for_mixed_whitespace_and_digits() {
        let d = Dispatcher::default();
        assert!(d.detect("  \n 42 \t 100 ").is_none());
    }

    // ---------------------------------------------------------------
    // Tier 0 script classification through the dispatcher.
    // These pass under any feature combo — script-detect is always
    // in and produces a valid Detection even when tier 1/2 don't
    // fire (short input, script not covered by allowlist, etc.).
    // ---------------------------------------------------------------

    #[test]
    fn dispatcher_reports_greek_script_for_greek_only_text() {
        // Greek script isn't in any tier-1 feature, so this stays a
        // tier-0-only result regardless of what's compiled in.
        let d = Dispatcher::default();
        let det = d.detect("αβγδεζη").expect("greek is classifiable");
        assert_eq!(det.script, "Grek");
        // Whether tier is Script or Whatlang depends on which
        // features are enabled — but bcp47 is empty only for Script.
        if det.tier == Tier::Script {
            assert!(det.bcp47.is_empty());
            assert!(det.name.is_empty());
            assert!(det.reliable);
            assert!((det.confidence - 1.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn dispatcher_reports_thai_script() {
        let d = Dispatcher::default();
        let det = d.detect("สวัสดีชาวโลก").expect("thai is classifiable");
        assert_eq!(det.script, "Thai");
    }

    #[test]
    fn dispatcher_reports_han_script_for_chinese_only() {
        let d = Dispatcher::default();
        let det = d
            .detect("这是一段中文文本用于测试")
            .expect("han is classifiable");
        // No kana / hangul present → Hans.
        assert_eq!(det.script, "Hans");
    }

    #[test]
    fn dispatcher_reports_japanese_when_kana_present() {
        // Japanese wins over Han when even one kana scalar is present.
        let d = Dispatcher::default();
        let det = d
            .detect("これは日本語のテスト")
            .expect("japanese classifiable");
        assert_eq!(det.script, "Jpan");
    }

    #[test]
    fn dispatcher_reports_korean_when_hangul_present() {
        let d = Dispatcher::default();
        let det = d
            .detect("안녕하세요 세계 이것은 한국어 텍스트입니다")
            .expect("korean classifiable");
        assert_eq!(det.script, "Hang");
    }

    #[test]
    fn dispatcher_ignores_digits_and_punctuation_in_mixed_text() {
        // Numbers and punctuation don't skew script detection.
        let d = Dispatcher::default();
        let det = d
            .detect("Hello, world! 12345 — testing 1, 2, 3.")
            .expect("latin classifiable");
        assert_eq!(det.script, "Latn");
    }

    #[test]
    fn dispatcher_picks_dominant_script_in_mixed_input() {
        // Mostly Latin + a couple Han glyphs → Latn wins (majority
        // rule at tier 0).
        let d = Dispatcher::default();
        let det = d
            .detect(
                "This is mostly an English sentence with just a stray 中 character in it \
                 for testing purposes.",
            )
            .expect("dominant script wins");
        assert_eq!(det.script, "Latn");
    }

    #[test]
    fn dispatcher_returns_some_for_kana_han_mix() {
        // A Han-majority string with a few kana characters is always
        // classifiable. The **specific** script value depends on
        // whether tier 1 fires and rewrites the script field (with
        // tier1-latn only, tier 0's `Jpan` stays; with wider tier 1
        // features, whatlang's Chinese/Japanese classifier weighs
        // in). The dispatcher-level invariant here is only that the
        // result is Some and its script is one of the CJK codes.
        let d = Dispatcher::default();
        let det = d
            .detect("这是一段中文文本の中に一つだけカナ")
            .expect("cjk mix classifies");
        assert!(matches!(det.script.as_str(), "Jpan" | "Hans" | "Cmn"));
    }

    // ---------------------------------------------------------------
    // Tier 1 — whatlang paths (feature-gated per script)
    // ---------------------------------------------------------------

    #[cfg(feature = "tier1-latn")]
    #[test]
    fn tier1_latn_classifies_german_sample() {
        let d = Dispatcher::default();
        let det = d
            .detect(
                "Das ist ein längerer deutscher Beispieltext für die Sprach\
                 erkennung. Er enthält genug Wörter, damit der Trigram-Detektor \
                 zuverlässig antwortet.",
            )
            .expect("german should classify");
        assert_eq!(det.script, "Latn");
        // whatlang uses ISO 639-3 codes.
        assert_eq!(det.bcp47, "deu");
        assert_eq!(det.tier, Tier::Whatlang);
    }

    #[cfg(feature = "tier1-latn")]
    #[test]
    fn tier1_latn_classifies_spanish_sample() {
        let d = Dispatcher::default();
        let det = d
            .detect(
                "Este es un texto de ejemplo en español que sirve para probar \
                 la detección automática de idioma. Contiene suficientes palabras \
                 para dar al detector una respuesta confiable.",
            )
            .expect("spanish should classify");
        assert_eq!(det.script, "Latn");
        assert_eq!(det.bcp47, "spa");
    }

    #[cfg(feature = "tier1-latn")]
    #[test]
    fn tier1_reliable_flag_agrees_with_threshold_comparison() {
        // For confidently-classified text at the default 0.75
        // threshold, reliable == true implies confidence >= 0.75.
        let d = Dispatcher::default();
        let det = d
            .detect(
                "The quick brown fox jumps over the lazy dog. Sphinx of black \
                 quartz, judge my vow. Pack my box with five dozen liquor jugs.",
            )
            .expect("english classifies");
        if det.reliable {
            assert!(det.confidence >= d.threshold);
        }
    }

    #[cfg(feature = "tier1-latn")]
    #[test]
    fn tier1_high_threshold_marks_borderline_as_unreliable() {
        // A threshold of 1.0 makes anything below perfect confidence
        // unreliable (and no tier 2 to escalate, since features
        // gate that).
        let d = Dispatcher::with_threshold(1.0);
        let det = d.detect("The quick brown fox jumps over the lazy dog.");
        if let Some(det) = det {
            // Confidence is never exactly 1.0 from whatlang on real text.
            #[cfg(not(feature = "tier2"))]
            assert!(!det.reliable || det.confidence >= 1.0);
            // With tier 2 the reliability can flip based on lingua's
            // rescoring — don't assert.
            let _ = det;
        }
    }

    #[cfg(feature = "tier1-cyrl")]
    #[test]
    fn tier1_cyrl_classifies_russian_sample() {
        let d = Dispatcher::default();
        let det = d
            .detect(
                "Это довольно длинный пример русского текста для проверки \
                 автоматического определения языка. Он содержит достаточно \
                 слов, чтобы детектор дал уверенный ответ.",
            )
            .expect("russian should classify");
        assert_eq!(det.script, "Cyrl");
        assert_eq!(det.bcp47, "rus");
        assert_eq!(det.tier, Tier::Whatlang);
    }

    #[cfg(feature = "tier1-arab")]
    #[test]
    fn tier1_arab_classifies_arabic_sample() {
        let d = Dispatcher::default();
        let det = d
            .detect(
                "هذا نص عربي طويل نسبيا يستخدم لاختبار الكشف التلقائي عن اللغة. \
                 يحتوي على كلمات كافية ليعطي الكاشف إجابة موثوقة.",
            )
            .expect("arabic should classify");
        assert_eq!(det.script, "Arab");
        assert_eq!(det.tier, Tier::Whatlang);
    }

    #[cfg(feature = "tier1-hebr")]
    #[test]
    fn tier1_hebr_classifies_hebrew_sample() {
        let d = Dispatcher::default();
        let det = d
            .detect(
                "זהו טקסט לדוגמה בעברית לצורך בדיקת זיהוי שפה אוטומטי. הוא \
                 מכיל מספיק מילים כדי לתת גלאי תשובה אמינה.",
            )
            .expect("hebrew should classify");
        assert_eq!(det.script, "Hebr");
        assert_eq!(det.bcp47, "heb");
    }

    #[cfg(feature = "tier1-deva")]
    #[test]
    fn tier1_deva_classifies_hindi_sample() {
        let d = Dispatcher::default();
        let det = d
            .detect(
                "यह एक हिंदी नमूना पाठ है जिसका उपयोग स्वचालित भाषा पहचान का \
                 परीक्षण करने के लिए किया जाता है। इसमें पर्याप्त शब्द हैं \
                 जो डिटेक्टर को विश्वसनीय उत्तर देते हैं।",
            )
            .expect("hindi should classify");
        assert_eq!(det.script, "Deva");
    }

    // Tier-1 handles the too-short input case: whatlang returns
    // None, so the dispatcher falls back to the tier-0 result. Under
    // the default `tier1-latn` feature the Detection is still Some
    // — script alone remains a valid answer.
    #[cfg(feature = "tier1-latn")]
    #[test]
    fn tier1_too_short_input_falls_back_to_tier0() {
        let d = Dispatcher::default();
        let det = d.detect("a");
        if let Some(det) = det {
            // Very short text — tier 1 typically returns None, so the
            // dispatcher's result stays at Script tier with an empty
            // bcp47.
            if det.tier == Tier::Script {
                assert!(det.bcp47.is_empty());
                assert!(det.reliable);
            }
        }
    }

    // ---------------------------------------------------------------
    // Tier 2 — escalation semantics
    // ---------------------------------------------------------------

    #[cfg(all(feature = "tier1-latn", feature = "tier2-en", feature = "tier2-de"))]
    #[test]
    fn tier2_confidence_lies_in_unit_interval() {
        let d = Dispatcher::default().always_escalate();
        let det = d
            .detect(
                "This is a longer sample of the English language, chosen to give \
                 the tier-1 trigram detector a comfortably-above-threshold \
                 confidence.",
            )
            .expect("english classifies");
        assert!(det.confidence >= 0.0 && det.confidence <= 1.0);
    }

    #[cfg(all(feature = "tier1-latn", feature = "tier2-en", feature = "tier2-de"))]
    #[test]
    fn tier2_escalation_preserves_bcp47_from_tier1() {
        // The dispatcher asks tier 2 for the tier-1 candidate's
        // confidence; the language code is not overwritten.
        let d = Dispatcher::default().always_escalate();
        let det = d
            .detect(
                "This is a longer sample of the English language for detection \
                 to work reliably in tier one.",
            )
            .expect("english classifies");
        assert_eq!(det.bcp47, "eng");
        assert_eq!(det.tier, Tier::Lingua);
    }

    #[cfg(all(feature = "tier1-latn", feature = "tier2-en", feature = "tier2-de"))]
    #[test]
    fn tier2_low_threshold_keeps_tier1_when_confident() {
        // Threshold 0.0 → nothing is ever below threshold and
        // always_escalate is off, so tier 1 stands.
        let d = Dispatcher::with_threshold(0.0);
        let det = d
            .detect(
                "The quick brown fox jumps over the lazy dog. Sphinx of black \
                 quartz, judge my vow. Waltz, nymph, for quick jigs vex bud.",
            )
            .expect("english classifies");
        assert_eq!(det.tier, Tier::Whatlang);
    }

    #[cfg(all(feature = "tier1-latn", feature = "tier2-en", feature = "tier2-de"))]
    #[test]
    fn tier2_high_threshold_triggers_escalation() {
        // Threshold above the reliable band → always try tier 2.
        let d = Dispatcher::with_threshold(0.99);
        let det = d
            .detect(
                "The quick brown fox jumps over the lazy dog. Sphinx of black \
                 quartz, judge my vow.",
            )
            .expect("english classifies");
        assert_eq!(det.tier, Tier::Lingua);
    }

    // ---------------------------------------------------------------
    // Detection.script is always set to a non-empty string
    // whenever the dispatcher returns Some.
    // ---------------------------------------------------------------

    #[test]
    fn detection_script_field_never_empty_when_some() {
        let d = Dispatcher::default();
        for &s in &[
            "Hello world, this is a test.",
            "αβγδε ζητα ηθικ λμνξο πρστυ",
            "Это тест",
            "これはテスト",
            "한국어 테스트",
        ] {
            if let Some(det) = d.detect(s) {
                assert!(!det.script.is_empty(), "empty script for {s:?}");
            }
        }
    }

    // ---------------------------------------------------------------
    // Property tests — invariants over arbitrary trailing/leading
    // whitespace and repetition. Only enabled off wasm (proptest
    // isn't available there — see Cargo.toml).
    // ---------------------------------------------------------------

    #[cfg(not(target_family = "wasm"))]
    mod props {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Trailing whitespace never changes the detected script
            /// — tier 0 skips whitespace by construction, so adding
            /// or removing spaces at the boundary is a no-op.
            #[test]
            fn trailing_whitespace_preserves_script(
                trailing in "[ \t\n]{0,10}",
            ) {
                let d = Dispatcher::default();
                let base = "Hello world this is a lengthy english sample.";
                let with_trail = format!("{base}{trailing}");
                let a = d.detect(base).map(|d| d.script.clone());
                let b = d.detect(&with_trail).map(|d| d.script.clone());
                prop_assert_eq!(a, b);
            }

            /// Leading whitespace never changes the detected script.
            #[test]
            fn leading_whitespace_preserves_script(
                leading in "[ \t\n]{0,10}",
            ) {
                let d = Dispatcher::default();
                let base = "Hello world this is a lengthy english sample.";
                let with_lead = format!("{leading}{base}");
                let a = d.detect(base).map(|d| d.script.clone());
                let b = d.detect(&with_lead).map(|d| d.script.clone());
                prop_assert_eq!(a, b);
            }

            /// Arbitrary ASCII digit runs never move the script
            /// classifier off Latin for English input — digits are
            /// stripped before scoring.
            #[test]
            fn digit_padding_preserves_script(
                digits in "[0-9]{0,20}",
            ) {
                let d = Dispatcher::default();
                let base = "The quick brown fox jumps over the lazy dog.";
                let padded = format!("{digits} {base} {digits}");
                let det = d.detect(&padded).expect("english classifies");
                prop_assert_eq!(det.script, "Latn");
            }

            /// The dispatcher never panics on arbitrary UTF-8 input,
            /// even garbage. This is a robustness invariant.
            #[test]
            fn detect_never_panics_on_arbitrary_text(text in ".*") {
                let d = Dispatcher::default();
                let _ = d.detect(&text);
            }

            /// `with_threshold` and setting the field directly are
            /// observationally equivalent modulo the always_escalate
            /// default.
            #[test]
            fn with_threshold_matches_default_then_field(t in 0.0f64..1.0) {
                let via_ctor = Dispatcher::with_threshold(t);
                let via_field = Dispatcher { threshold: t, always_escalate: false };
                prop_assert!((via_ctor.threshold - via_field.threshold).abs() < f64::EPSILON);
                prop_assert_eq!(via_ctor.always_escalate, via_field.always_escalate);
            }
        }
    }

    // Extra plain #[test] cases that exercise inputs which
    // proptest strategies won't easily reach.

    #[test]
    fn empty_string_is_unclassifiable() {
        let d = Dispatcher::default();
        assert!(d.detect("").is_none());
    }

    #[test]
    fn single_ascii_letter_returns_latin_script() {
        // Even one letter is enough for tier 0 to name a script.
        let d = Dispatcher::default();
        let det = d.detect("a").expect("single letter classifies");
        assert_eq!(det.script, "Latn");
    }

    #[test]
    fn single_cjk_char_returns_han_script() {
        let d = Dispatcher::default();
        let det = d.detect("中").expect("single han classifies");
        assert_eq!(det.script, "Hans");
    }

    #[test]
    fn single_kana_char_returns_japanese() {
        let d = Dispatcher::default();
        let det = d.detect("あ").expect("single kana classifies");
        assert_eq!(det.script, "Jpan");
    }
}
