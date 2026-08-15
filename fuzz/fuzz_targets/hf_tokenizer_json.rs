//! Fuzz target: Hugging Face `tokenizer.json` deserialiser.
//!
//! `serde_json` + a large collection of tagged enums (normalizer,
//! pre-tokenizer, model, post-processor, decoder) is a broad
//! attack-surface for an untrusted-JSON parser. Malformed tag strings,
//! missing required fields, unexpected variant discriminants, and
//! adversarial JSON depth are all the classic bug-attractor for a
//! serde-driven loader. This target hands libFuzzer arbitrary bytes,
//! attempts to interpret them as UTF-8 JSON, and asserts that
//! [`parse_tokenizer_json`] returns either `Ok` or a typed
//! [`HfParseError`] — never panics.
//!
//! Non-UTF-8 inputs are skipped early rather than counted as findings;
//! the JSON layer itself already requires valid UTF-8 and there is no
//! deserialiser code to exercise on a byte sequence that cannot be
//! promoted to `&str`.
//!
//! `to_bpe_tokenizer` / `to_tokenizer` are *not* driven from this
//! target — those are the conversion layer and their surface is
//! typed-value-in / typed-error-out, structurally distinct from the
//! JSON-parse surface this target focuses on. A follow-up target can
//! add differential coverage over the conversion stage once the parse
//! side is stable.

#![no_main]

use libfuzzer_sys::fuzz_target;
use stringcheese_tokenizer_hf::hf::parse_tokenizer_json;

fuzz_target!(|data: &[u8]| {
    // JSON requires valid UTF-8. Skip non-UTF-8 inputs — libFuzzer will
    // learn to produce valid UTF-8 bytes on its own from the seed
    // corpus (all of which are valid UTF-8 by construction).
    let Ok(json) = core::str::from_utf8(data) else {
        return;
    };

    // The invariant: whatever the input, the parser returns Ok or a
    // typed HfParseError. Any panic is a bug — either in the parser or
    // in a serde-derived deserialiser downstream.
    let _ = parse_tokenizer_json(json);
});
