//! WIT `Guest` implementation for the tier-0 script classifier.
//!
//! Bridges [`crate::detect_script`] to the shared
//! `tegmentum:lang-detect@0.2.0` interface so this crate can be built
//! as a WASM component that hosts consume through the generated
//! bindings.
//!
//! Only compiled on wasm targets — native builds skip the bindings
//! module entirely so `cargo test` doesn't drag in `wit-bindgen-rt`.

use crate::bindings::exports::tegmentum::lang_detect::detector::{
    Capabilities, Detection, Guest, RankedDetection, SpanDetection,
};

/// The Guest type the export! macro wires to the WIT world's exports.
/// Zero-sized — every method is stateless.
pub struct Component;

impl Guest for Component {
    fn get_capabilities() -> Capabilities {
        // Script-detect is the simplest backend: no ranking, no
        // language allowlist filtering, no mixed-script segmentation.
        // Callers use it as the tier-0 bootstrap that picks which
        // per-script tier-1 shard to fetch, not as an answer in
        // itself.
        Capabilities {
            backend: "script".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            supports: Vec::new(),
            can_rank: false,
            can_constrain: false,
            can_segment: false,
        }
    }

    fn detect(text: String) -> Option<Detection> {
        crate::detect_script(&text).map(|script| Detection {
            // Tier 0 only classifies script — no ISO 639-3 language
            // code available. The host walks tier 1 next for that.
            lang: String::new(),
            lang_name: String::new(),
            script: script.to_string(),
            // Block classification is deterministic; confidence is
            // fixed at 1.0 by construction.
            confidence: 1.0,
            reliable: true,
        })
    }

    fn detect_ranked(text: String) -> Vec<RankedDetection> {
        // Script-detect has no ranked distribution — return the
        // single answer as a one-element list when it fires, empty
        // otherwise. Matches the WIT contract for backends without
        // `can-rank`.
        match crate::detect_script(&text) {
            Some(_) => Vec::new(),
            None => Vec::new(),
        }
    }

    fn detect_from(text: String, _langs: Vec<String>) -> Option<Detection> {
        // No `can-constrain` — ignore the caller's allowlist and
        // return the unconstrained script classification. Documented
        // in the WIT contract.
        Self::detect(text)
    }

    fn detect_mixed(_text: String) -> Option<Vec<SpanDetection>> {
        // No `can-segment` — always `None`.
        None
    }
}

crate::bindings::export!(Component with_types_in crate::bindings);
