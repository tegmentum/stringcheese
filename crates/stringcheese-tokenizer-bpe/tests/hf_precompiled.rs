//! Integration tests for the Hugging Face `Precompiled` normalizer.
//!
//! `Precompiled` is `SentencePiece`'s serialized character-remapping
//! table — every Llama, Mistral, T5, and XLM-RoBERTa checkpoint ships
//! one, usually inside a `Sequence` alongside `Prepend` and `Replace`.
//! Full runtime execution requires re-implementing `SentencePiece`'s
//! `Normalizer::Normalize()` over the decoded charsmap; the current
//! landing parses the field successfully and applies a passthrough at
//! runtime so real `tokenizer.json` blobs load and encode instead of
//! erroring at load time.
//!
//! See [`Normalizer::Precompiled`] for the documented passthrough
//! contract and the follow-up TODO.

#![cfg(feature = "hf-tokenizer")]

use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_bpe::hf::{HfNormalizer, parse_tokenizer_json, to_bpe_tokenizer};
use stringcheese_tokenizer_bpe::normalizer::{Normalizer, normalize};

// ---------------------------------------------------------------------
// Deserialisation.
// ---------------------------------------------------------------------

#[test]
fn precompiled_deserialises_from_typed_block() {
    // The base64 payload is preserved verbatim; the loader does not
    // decode or validate it — see the variant docs for the contract.
    let json = r#"{
        "type": "Precompiled",
        "precompiled_charsmap": "AAAAABAAAAAA"
    }"#;
    let n: HfNormalizer = serde_json::from_str(json).unwrap();
    match n {
        HfNormalizer::Precompiled {
            precompiled_charsmap,
        } => {
            assert_eq!(precompiled_charsmap, "AAAAABAAAAAA");
        }
        other => panic!("expected Precompiled, got {other:?}"),
    }
}

#[test]
fn precompiled_deserialises_inside_full_tokenizer_json() {
    let json = r#"{
        "added_tokens": [],
        "normalizer": {
            "type": "Precompiled",
            "precompiled_charsmap": "AAAA"
        },
        "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    match &config.normalizer {
        Some(HfNormalizer::Precompiled {
            precompiled_charsmap,
        }) => {
            assert_eq!(precompiled_charsmap, "AAAA");
        }
        other => panic!("expected Precompiled, got {other:?}"),
    }
}

#[test]
fn precompiled_deserialises_inside_sequence_alongside_prepend_and_replace() {
    // The exact shape every Llama / Mistral / T5 tokenizer.json ships:
    // Sequence [Precompiled, Prepend, Replace]. Verify the sequence
    // parses and the Precompiled entry carries its charsmap.
    let json = r#"{
        "added_tokens": [],
        "normalizer": {
            "type": "Sequence",
            "normalizers": [
                {"type": "Precompiled", "precompiled_charsmap": "AAAA"},
                {"type": "Prepend", "prepend": "▁"},
                {"type": "Replace", "pattern": {"String": " "}, "content": "▁"}
            ]
        },
        "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    match config.normalizer.as_ref().unwrap() {
        HfNormalizer::Sequence { normalizers } => {
            assert_eq!(normalizers.len(), 3);
            assert!(matches!(
                &normalizers[0],
                HfNormalizer::Precompiled { precompiled_charsmap } if precompiled_charsmap == "AAAA"
            ));
            assert!(matches!(&normalizers[1], HfNormalizer::Prepend { .. }));
            assert!(matches!(&normalizers[2], HfNormalizer::Replace { .. }));
        }
        other => panic!("expected Sequence, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Runtime — passthrough contract.
// ---------------------------------------------------------------------

#[test]
fn precompiled_apply_returns_input_unchanged() {
    // The variant is a documented passthrough today — see
    // `Normalizer::Precompiled`'s doc-comment. Verify that concretely
    // across a mix of ASCII, multi-byte scalars, and controls that a
    // real SentencePiece charsmap would touch.
    let n = Normalizer::Precompiled {
        charsmap_base64: "AAAA".to_string(),
    };
    for input in [
        "",
        "hello world",
        "café",
        // U+FF11 FULLWIDTH DIGIT ONE — a real SentencePiece charsmap
        // would collapse this to ASCII "1"; the passthrough leaves it
        // alone, which is documented behaviour.
        "\u{FF11}",
        "line1\nline2",
    ] {
        assert_eq!(
            normalize(input, &n),
            input,
            "passthrough failed on {input:?}"
        );
    }
}

// ---------------------------------------------------------------------
// End-to-end: parse -> materialise -> encode.
// ---------------------------------------------------------------------

#[test]
fn precompiled_materialises_into_runtime_variant() {
    let json = r#"{
        "added_tokens": [],
        "normalizer": {
            "type": "Precompiled",
            "precompiled_charsmap": "hello-charsmap"
        },
        "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_bpe_tokenizer(&config).unwrap();
    match tok.normalizer() {
        Some(Normalizer::Precompiled { charsmap_base64 }) => {
            assert_eq!(charsmap_base64, "hello-charsmap");
        }
        other => panic!("expected Normalizer::Precompiled, got {other:?}"),
    }
}

#[test]
fn precompiled_encode_is_pass_through_end_to_end() {
    // Vocab covers just "a"; because the normalizer is a passthrough,
    // an input of "a" encodes to id 0 and decodes back to "a" — the
    // charsmap never fires.
    let json = r#"{
        "added_tokens": [],
        "normalizer": {
            "type": "Precompiled",
            "precompiled_charsmap": "AAAA"
        },
        "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_bpe_tokenizer(&config).unwrap();
    let enc = tok.encode("a").unwrap();
    assert_eq!(enc.ids, vec![0]);
    assert_eq!(tok.decode(&enc.ids).unwrap(), "a");
}
