//! Property test: `HfNativeTokenizer` produces byte-identical output
//! to the underlying upstream `tokenizers-rs` crate.
//!
//! This is the load-bearing contract of `stringcheese-tokenizer-hf-native`
//! and the whole reason a caller reaches for it over the wasm-friendly
//! runtime — so it earns a property test in its own file rather than a
//! hand-picked corpus of unit assertions.
//!
//! `#[cfg(not(target_family = "wasm"))]` mirrors the crate-level gate
//! in `src/lib.rs`: the crate is compiled to an empty module on wasm
//! targets, and `proptest`'s transitive `wait-timeout` dep is
//! Unix/Windows-only, so linking the test binary against wasm fails
//! outright without this gate. See the tokenizer-hf crate's own
//! `Cargo.toml` note on the same target-gating pattern.

#![cfg(not(target_family = "wasm"))]

use std::fmt::Write as _;

use proptest::prelude::*;

use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_hf_native::HfNativeTokenizer;
use tokenizers::{EncodeInput, Tokenizer as HfLibTokenizer};

/// A BPE config with a rich enough vocab that arbitrary short ASCII
/// inputs still round-trip (byte-fallback covers the rest).
///
/// The 256 `<0xXX>` byte-fallback tokens (ids 4..260) give upstream a
/// safe encode for every byte, so property inputs do not need to be
/// pre-filtered against the vocab. Inline rather than committed to
/// `tests/vocabs/` because the config is tiny and self-contained; it
/// also avoids the "no real vocab bytes" workspace constraint.
fn tiny_bpe_json() -> String {
    let mut vocab: Vec<(String, u32)> = vec![
        ("<unk>".to_string(), 0),
        ("<s>".to_string(), 1),
        ("</s>".to_string(), 2),
        (" ".to_string(), 3),
    ];
    for b in 0u32..=255 {
        vocab.push((format!("<0x{b:02X}>"), 4 + b));
    }
    let vocab_json: String = vocab
        .iter()
        .map(|(s, i)| format!("{}: {i}", serde_json_string(s)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"{{
            "version": "1.0",
            "truncation": null,
            "padding": null,
            "added_tokens": [],
            "normalizer": null,
            "pre_tokenizer": null,
            "post_processor": null,
            "decoder": null,
            "model": {{
                "type": "BPE",
                "dropout": null,
                "unk_token": "<unk>",
                "continuing_subword_prefix": null,
                "end_of_word_suffix": null,
                "fuse_unk": false,
                "byte_fallback": true,
                "ignore_merges": false,
                "vocab": {{ {vocab_json} }},
                "merges": []
            }}
        }}"#
    )
}

/// Minimal JSON-string encoder for the vocab surface strings — the
/// property test needs to be dependency-free beyond `proptest`
/// (`serde_json` is a heavy dev-dep for a helper this small).
fn serde_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                // `Write` on a `String` is infallible.
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

proptest! {
    #[test]
    fn ids_match_upstream_for_arbitrary_ascii(input in "\\PC{0,64}") {
        let json = tiny_bpe_json();
        let ours = HfNativeTokenizer::from_bytes(json.as_bytes())
            .unwrap()
            .with_add_special_tokens(false);
        let theirs = HfLibTokenizer::from_bytes(json.as_bytes()).unwrap();

        let ours_ids = Tokenizer::encode(&ours, &input).unwrap().ids;
        let theirs_ids: Vec<u32> = {
            let hf_input: EncodeInput<'_> = input.as_str().into();
            theirs.encode(hf_input, false).unwrap().get_ids().to_vec()
        };
        prop_assert_eq!(ours_ids, theirs_ids);
    }
}
