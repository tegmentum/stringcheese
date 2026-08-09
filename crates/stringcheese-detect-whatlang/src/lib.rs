//! # Tier-1 whatlang wrapper (per-script, WIT-shaped)
//!
//! Wraps the `whatlang` trigram detector (vendored under
//! `src/vendored/`) behind the shared
//! `tegmentum:lang-detect@0.2.0` WIT contract, with per-script
//! Cargo features selecting the language allowlist. A build with
//! `--features latn` classifies only Latin-script languages; a build
//! with `--features cyrl` classifies only Cyrillic; and so on.
//!
//! ## Position in the stack
//!
//! This crate is the **tier-1** step in the detection pipeline:
//!
//! ```text
//! text
//!   │
//!   ▼
//! stringcheese-detect-script  (~5 KB, always resident)
//!   │  returns ISO 15924 script code, e.g. "Latn"
//!   ▼
//! host lazy-loads stringcheese-detect-whatlang-latn.wasm
//!   │  returns ISO 639-3 language code + confidence
//!   ▼
//! host lazy-loads stringcheese-lang-<code>.wasm (a StringCheese
//!   language pack) OR escalates to
//!   stringcheese-detect-lingua-<code>.wasm for higher accuracy
//! ```
//!
//! Every hop is small and lazy. Tier 0 is one always-resident
//! ~5 KB module; each tier-1 shard is loaded once per script the
//! user actually types in.
//!
//! ## Feature contract
//!
//! Exactly one script feature is expected to be enabled per build.
//! Enabling multiple features widens the allowlist union; enabling
//! zero features falls through to whatlang's default (all languages)
//! and defeats the sharding purpose — the crate `panic!`s at
//! initialization time with a clear message if that happens.
//!
//! ## Current shard fidelity — hard-sharded
//!
//! whatlang's source is vendored under `src/vendored/` with
//! `trigrams/profiles.rs` split into per-script files
//! (`profiles/{latn,cyrl,arab,deva,hebr}.rs`), each gated behind
//! the matching Cargo feature. Disabled scripts' profile tables
//! aren't compiled in at all — the linker drops them entirely.
//!
//! Measured release-build rlib sizes (indicative — the WASM
//! component after `wasm-tools component new --strip` +
//! `opt-level = "z"` is meaningfully smaller):
//!
//! - `--features latn` alone:  ~1.38 MB
//! - `--features cyrl` alone:  ~1.04 MB
//! - all scripts enabled:      ~1.60 MB
//!
//! Cyrillic-only saves ~35 % vs. all-scripts because whatlang's
//! Latin trigram table dominates the profile data (~10 k lines out
//! of 15 k).

#![deny(unsafe_code)]

// WIT bindings + Guest wiring only compile on wasm targets with the
// `wit-component` feature — native builds (including `cargo test`)
// stay free of component-model overhead, and non-component wasm
// consumers (e.g. the `stringcheese-detect` umbrella linking this
// crate as an `rlib` sibling of the other detect backends) also
// skip them. Without the feature-gate, three backends `export!`-ing
// the same interface into one binary collide at link time with
// duplicate exported symbols.
#[cfg(all(target_family = "wasm", feature = "wit-component"))]
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
#[cfg(all(target_family = "wasm", feature = "wit-component"))]
#[allow(unsafe_code, unsafe_op_in_unsafe_fn)]
mod wit;

// The whatlang source is vendored under `src/vendored/` — the hard
// shard splits `trigrams/profiles.rs` into per-script files behind
// Cargo features, so building with only `latn` compiles only the
// Latin trigrams into the binary. See `LICENSE-WHATLANG` at the
// crate root for the upstream attribution.
mod vendored;

use vendored::{Detector, Info, Lang, Script};

/// A single language-detection result. Mirrors the `detection`
/// record in the WIT interface so callers can trivially map into
/// the WIT world when compiled as a component.
///
/// See the WIT source at `wit/lang-detect.wit` for the canonical
/// field-level documentation.
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
    /// Whatlang's own opinion about whether the prediction is
    /// trustworthy.
    pub reliable: bool,
}

/// Capability record for this backend — matches the WIT `capabilities`
/// record 1:1. Cheap to construct; callers cache it at load time.
#[derive(Clone, Debug, PartialEq)]
pub struct Capabilities {
    /// Backend identifier — always `"whatlang"` for this crate.
    pub backend: &'static str,
    /// Semver of this crate (which pins the whatlang version).
    pub version: &'static str,
    /// ISO 639-3 codes the enabled per-script features cover.
    pub supports: Vec<String>,
    /// `true` — whatlang gives per-candidate distances (rank stub
    /// today; real per-language scoring lands via
    /// `whatlang::dev::trigrams_raw_detect` in a follow-up).
    pub can_rank: bool,
    /// `true` — supports caller-supplied allowlist via
    /// `whatlang::Detector::with_allowlist`.
    pub can_constrain: bool,
    /// `false` — no mixed-language segmentation (that's tier 2).
    pub can_segment: bool,
}

/// Report which backend + version this build is running against and
/// which languages it can distinguish (i.e. the union of the enabled
/// per-script allowlists).
#[must_use]
pub fn capabilities() -> Capabilities {
    Capabilities {
        backend: "whatlang",
        version: env!("CARGO_PKG_VERSION"),
        supports: enabled_langs()
            .iter()
            .map(|l| l.code().to_string())
            .collect(),
        can_rank: true,      // whatlang gives per-candidate distance
        can_constrain: true, // via with_allowlist at runtime
        can_segment: false,  // no mixed-language span detection
    }
}

/// Baseline single-best-guess detection. Returns `None` when whatlang
/// can't classify the input (too short, ambiguous, or falls entirely
/// outside the enabled allowlist).
#[must_use]
pub fn detect(text: &str) -> Option<Detection> {
    let detector = build_detector(None);
    detector.detect(text).map(|info| to_detection(&info))
}

/// Constrained detection — considers only the ISO 639-3 codes in
/// `langs` from the crate's already-enabled allowlist. Codes not
/// enabled at compile time are silently dropped (we can't classify
/// what wasn't compiled in).
#[must_use]
pub fn detect_from(text: &str, langs: &[&str]) -> Option<Detection> {
    let allow: Vec<Lang> = langs
        .iter()
        .filter_map(|code| iso_639_3_to_lang(code))
        .filter(|lang| enabled_langs().contains(lang))
        .collect();
    if allow.is_empty() {
        // Caller's allowlist and this build's allowlist don't
        // intersect — no possible answer.
        return None;
    }
    let detector = build_detector(Some(allow));
    detector.detect(text).map(|info| to_detection(&info))
}

/// Ranked distribution across the enabled languages. The vec is
/// ordered by descending confidence; the top element is what
/// [`detect`] would return.
///
/// **Current limitation**: whatlang 0.16 doesn't expose per-candidate
/// scores through its stable public API — this stub returns just the
/// single best guess as a one-element vec so callers that always
/// call `detect_ranked` don't have to special-case. A follow-up
/// switches to `whatlang::dev::trigrams_raw_detect` for real per-
/// language scoring (`RawOutcome` carries the full distribution).
#[must_use]
pub fn detect_ranked(text: &str) -> Vec<(String, f64)> {
    match detect(text) {
        Some(d) => vec![(d.lang, d.confidence)],
        None => Vec::new(),
    }
}

// ---------------------------------------------------------------------
// Internal — allowlist wiring
// ---------------------------------------------------------------------

fn build_detector(explicit_allow: Option<Vec<Lang>>) -> Detector {
    let allow = explicit_allow.unwrap_or_else(|| enabled_langs().to_vec());
    Detector::with_allowlist(allow)
}

/// The union of languages every enabled per-script feature covers.
/// The static allowlist is what makes the detector deterministic —
/// a Latin-script build never even considers Russian.
fn enabled_langs() -> &'static [Lang] {
    // Compile-time selection: each feature contributes its language
    // slice; the union is what this build can classify.
    #[allow(unused_mut)]
    static ALLOW: std::sync::OnceLock<Vec<Lang>> = std::sync::OnceLock::new();
    ALLOW.get_or_init(|| {
        let mut out: Vec<Lang> = Vec::new();
        #[cfg(feature = "latn")]
        out.extend(langs_for(Script::Latin));
        #[cfg(feature = "cyrl")]
        out.extend(langs_for(Script::Cyrillic));
        #[cfg(feature = "arab")]
        out.extend(langs_for(Script::Arabic));
        #[cfg(feature = "hebr")]
        out.extend(langs_for(Script::Hebrew));
        #[cfg(feature = "grek")]
        out.extend(langs_for(Script::Greek));
        #[cfg(feature = "deva")]
        out.extend(langs_for(Script::Devanagari));
        #[cfg(feature = "hans")]
        out.extend(langs_for(Script::Mandarin));
        #[cfg(feature = "hant")]
        out.extend(langs_for(Script::Mandarin));
        #[cfg(feature = "hang")]
        out.extend(langs_for(Script::Hangul));
        #[cfg(feature = "jpan")]
        {
            out.extend(langs_for(Script::Hiragana));
            out.extend(langs_for(Script::Katakana));
        }
        assert!(
            !out.is_empty(),
            "stringcheese-detect-whatlang built with zero script features; \
             enable at least one of latn/cyrl/arab/hebr/grek/deva/hans/hant/hang/jpan",
        );
        out.sort_by_key(Lang::code);
        out.dedup();
        out
    })
}

/// Enumerate whatlang's languages for a given script.
///
/// whatlang 0.16 keeps its per-script language mapping in a
/// `pub(crate)` module (`scripts::lang_mapping::script_langs`), so
/// the table isn't reachable through the public API. We mirror the
/// mapping here — it's small (~70 lines total), tracks whatlang's
/// own coverage, and stays 1:1 with `scripts/lang_mapping.rs` in
/// whatlang's source. When we vendor whatlang's data outright the
/// duplication drops away.
fn langs_for(script: Script) -> &'static [Lang] {
    // Mirror of `whatlang::scripts::lang_mapping::script_langs`.
    // Kept as `const` slices so the OnceLock init below just
    // concatenates references without cloning per language.
    const LATIN: &[Lang] = &[
        Lang::Spa,
        Lang::Eng,
        Lang::Por,
        Lang::Ind,
        Lang::Fra,
        Lang::Deu,
        Lang::Jav,
        Lang::Vie,
        Lang::Ita,
        Lang::Tur,
        Lang::Pol,
        Lang::Ron,
        Lang::Hrv,
        Lang::Nld,
        Lang::Uzb,
        Lang::Hun,
        Lang::Aze,
        Lang::Ces,
        Lang::Zul,
        Lang::Swe,
        Lang::Aka,
        Lang::Sna,
        Lang::Afr,
        Lang::Fin,
        Lang::Slk,
        Lang::Tgl,
        Lang::Tuk,
        Lang::Dan,
        Lang::Nob,
        Lang::Cat,
        Lang::Lit,
        Lang::Slv,
        Lang::Epo,
        Lang::Lav,
        Lang::Est,
        Lang::Lat,
    ];
    const CYRILLIC: &[Lang] = &[
        Lang::Rus,
        Lang::Ukr,
        Lang::Srp,
        Lang::Bel,
        Lang::Bul,
        Lang::Mkd,
    ];
    const ARABIC: &[Lang] = &[Lang::Ara, Lang::Urd, Lang::Pes];
    const DEVANAGARI: &[Lang] = &[Lang::Hin, Lang::Mar, Lang::Nep];
    const HEBREW: &[Lang] = &[Lang::Heb, Lang::Yid];
    match script {
        Script::Latin => LATIN,
        Script::Cyrillic => CYRILLIC,
        Script::Arabic => ARABIC,
        Script::Devanagari => DEVANAGARI,
        Script::Hebrew => HEBREW,
        Script::Mandarin => &[Lang::Cmn],
        Script::Bengali => &[Lang::Ben],
        Script::Hangul => &[Lang::Kor],
        Script::Georgian => &[Lang::Kat],
        Script::Greek => &[Lang::Ell],
        Script::Kannada => &[Lang::Kan],
        Script::Tamil => &[Lang::Tam],
        Script::Thai => &[Lang::Tha],
        Script::Gujarati => &[Lang::Guj],
        Script::Gurmukhi => &[Lang::Pan],
        Script::Telugu => &[Lang::Tel],
        Script::Malayalam => &[Lang::Mal],
        Script::Oriya => &[Lang::Ori],
        Script::Myanmar => &[Lang::Mya],
        Script::Sinhala => &[Lang::Sin],
        Script::Khmer => &[Lang::Khm],
        Script::Ethiopic => &[Lang::Amh],
        Script::Armenian => &[Lang::Hye],
        Script::Katakana | Script::Hiragana => &[Lang::Jpn],
    }
}

fn iso_639_3_to_lang(code: &str) -> Option<Lang> {
    Lang::all().iter().copied().find(|l| l.code() == code)
}

// Returns `Option` even though it always yields `Some` — kept for
// the ergonomic caller pattern `detector.detect(text).and_then(...)`
// where `detect` returns `Option<Info>`.
fn to_detection(info: &Info) -> Detection {
    Detection {
        lang: info.lang().code().to_string(),
        lang_name: info.lang().eng_name().to_string(),
        script: script_iso_15924(info.script()).to_string(),
        confidence: info.confidence(),
        reliable: info.is_reliable(),
    }
}

/// Map whatlang's `Script` variant to its ISO 15924 four-letter code.
/// whatlang names scripts as English words (`"Latin"`, `"Cyrillic"`);
/// the WIT contract uses ISO 15924 for interop with the tier-0 script
/// detector and every downstream language pack, so we translate at
/// the boundary.
fn script_iso_15924(script: Script) -> &'static str {
    match script {
        Script::Latin => "Latn",
        Script::Cyrillic => "Cyrl",
        Script::Arabic => "Arab",
        Script::Hebrew => "Hebr",
        Script::Greek => "Grek",
        Script::Devanagari => "Deva",
        Script::Mandarin => "Hans",
        Script::Hangul => "Hang",
        Script::Hiragana | Script::Katakana => "Jpan",
        Script::Bengali => "Beng",
        Script::Georgian => "Geor",
        Script::Armenian => "Armn",
        Script::Ethiopic => "Ethi",
        Script::Gujarati => "Gujr",
        Script::Gurmukhi => "Guru",
        Script::Kannada => "Knda",
        Script::Khmer => "Khmr",
        Script::Malayalam => "Mlym",
        Script::Myanmar => "Mymr",
        Script::Oriya => "Orya",
        Script::Sinhala => "Sinh",
        Script::Tamil => "Taml",
        Script::Telugu => "Telu",
        Script::Thai => "Thai",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "latn")]
    #[test]
    fn detects_english_in_latn_build() {
        let d = detect(
            "This is a longer sample of the English language for detection to work reliably.",
        )
        .expect("english text classifies under latn");
        assert_eq!(d.lang, "eng");
        assert_eq!(d.script, "Latn");
        assert!(d.confidence > 0.0);
    }

    #[cfg(feature = "cyrl")]
    #[test]
    fn detects_russian_in_cyrl_build() {
        let d = detect("Это пример текста на русском языке для определения языка.")
            .expect("russian text classifies under cyrl");
        assert_eq!(d.lang, "rus");
        assert_eq!(d.script, "Cyrl");
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(detect("").is_none());
    }

    #[test]
    fn capabilities_advertises_backend() {
        let caps = capabilities();
        assert_eq!(caps.backend, "whatlang");
        assert!(caps.can_rank);
        assert!(caps.can_constrain);
        assert!(!caps.can_segment);
        assert!(
            !caps.supports.is_empty(),
            "at least one language enabled per build"
        );
    }

    #[cfg(feature = "latn")]
    #[test]
    fn detect_from_respects_allowlist() {
        // Force whatlang to only consider German — even for an
        // English-looking input, the German bucket wins.
        let text = "This is a sample English sentence.";
        let d = detect_from(text, &["deu"]).expect("some result comes back");
        assert_eq!(d.lang, "deu");
    }

    #[cfg(feature = "latn")]
    #[test]
    fn detect_from_returns_none_on_disjoint_allowlist() {
        // Russian isn't in the Latin allowlist even if the caller
        // asks for it under a latn-only build.
        assert!(detect_from("hello world", &["rus"]).is_none());
    }

    #[cfg(feature = "latn")]
    #[test]
    fn detect_ranked_returns_at_least_the_best_guess() {
        let ranked = detect_ranked(
            "This is a longer sample of the English language for detection to work reliably.",
        );
        assert!(!ranked.is_empty());
        // Ranked list ordered by descending confidence.
        for pair in ranked.windows(2) {
            assert!(pair[0].1 >= pair[1].1);
        }
    }
}
