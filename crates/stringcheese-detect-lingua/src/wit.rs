//! WIT `Guest` implementation for the tier-2 lingua wrapper.
//!
//! Bridges the crate's native API ([`crate::detect`], [`crate::detect_from`],
//! [`crate::detect_ranked`], [`crate::capabilities`]) to the shared
//! `tegmentum:lang-detect@0.2.0` interface so per-language builds ship
//! as WASM components hosts load only when tier 1's confidence falls
//! below the caller's threshold.
//!
//! Only compiled on wasm targets with the `wit-component` feature.
//! Native builds skip the bindings module so `cargo test` doesn't
//! pull in `wit-bindgen-rt`, and wasm builds without the feature
//! (e.g. the `stringcheese-detect` umbrella linking this crate as
//! an `rlib` alongside the other detect backends) also skip it —
//! otherwise three backends `export!`-ing the same
//! `tegmentum:lang-detect` interface into one binary would collide
//! at link time with duplicate exported symbols. The feature-gated
//! build is the standalone-component path (`cargo component build
//! --features wit-component,<lang1>,<lang2>`).

use crate::bindings::exports::tegmentum::lang_detect::detector::{
    Capabilities, Detection, Guest, RankedDetection, SpanDetection,
};

pub struct Component;

impl Guest for Component {
    fn get_capabilities() -> Capabilities {
        let native = crate::capabilities();
        Capabilities {
            backend: native.backend.to_string(),
            version: native.version.to_string(),
            supports: native.supports,
            can_rank: native.can_rank,
            can_constrain: native.can_constrain,
            can_segment: native.can_segment,
        }
    }

    fn detect(text: String) -> Option<Detection> {
        crate::detect(&text).map(to_wit_detection)
    }

    fn detect_ranked(text: String) -> Vec<RankedDetection> {
        crate::detect_ranked(&text)
            .into_iter()
            .map(|(lang, confidence)| RankedDetection { lang, confidence })
            .collect()
    }

    fn detect_from(text: String, langs: Vec<String>) -> Option<Detection> {
        let refs: Vec<&str> = langs.iter().map(String::as_str).collect();
        crate::detect_from(&text, &refs).map(to_wit_detection)
    }

    fn detect_mixed(_text: String) -> Option<Vec<SpanDetection>> {
        // Lingua does support `detect_multiple_languages_of` — the
        // capabilities record advertises `can-segment: true` — but
        // the mixed-span path is not wired here yet. Returning
        // `None` matches the WIT contract for "not implemented for
        // this build" and keeps the export surface stable while the
        // segmentation wiring lands in a follow-up.
        None
    }
}

fn to_wit_detection(d: crate::Detection) -> Detection {
    Detection {
        lang: d.lang,
        lang_name: d.lang_name,
        script: d.script,
        confidence: d.confidence,
        reliable: d.reliable,
    }
}

crate::bindings::export!(Component with_types_in crate::bindings);
