//! Integration tests for the Hugging Face `BertNormalizer` support.
//!
//! Covers:
//!
//! * Deserialisation of the `{"type": "BertNormalizer", ...}` block
//!   into [`HfNormalizer::BertNormalizer`].
//! * Materialisation via `to_bpe_tokenizer` into a runtime tokenizer
//!   whose normalizer is [`Normalizer::Bert`].
//! * The four transformation passes — `clean_text`,
//!   `handle_chinese_chars`, `strip_accents`, `lowercase` — in the
//!   HF-canonical order.
//!
//! The reference for the exact semantics is `HuggingFace`'s
//! `tokenizers` crate (`normalizers::bert::BertNormalizer`): every
//! pass mirrors upstream's Rust implementation, with the one
//! documented deviation that accent stripping uses
//! canonical-combining-class not equal to zero (rather than
//! `is_mark_nonspacing`) as its filter — see the
//! `Normalizer::Bert` variant docs.

#![cfg(feature = "hf-tokenizer")]

use core::fmt::Write as _;

use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_bpe::hf::{HfNormalizer, parse_tokenizer_json, to_bpe_tokenizer};
use stringcheese_tokenizer_bpe::normalizer::{Normalizer, normalize};

/// A BPE config whose vocabulary is the full 0..=255 byte alphabet
/// (as one-char strings) — combined with a `BertNormalizer` that
/// normalises the input before the BPE pass runs. Every ASCII input
/// remains encodable and the encoded byte string reveals what the
/// normalizer emitted.
fn bert_config(normalizer_json: &str) -> String {
    // Byte-alphabet vocab so any post-normalization ASCII input
    // encodes without hitting an unknown token. Han characters that
    // survive the normalizer will surface as their UTF-8 byte ids —
    // and the ` 你  好 ` pattern of `handle_chinese_chars` will show
    // up in the byte-decoded round-trip.
    let mut vocab_entries = String::from("{");
    for b in 0u8..=255 {
        if b > 0 {
            vocab_entries.push(',');
        }
        // Serialise the byte as a one-char JSON string. A handful of
        // bytes (`"`, `\`, control chars) need escaping.
        let s = char::from(b);
        let key = match s {
            '"' => "\\\"".to_string(),
            '\\' => "\\\\".to_string(),
            c if (c as u32) < 0x20 => format!("\\u{:04x}", c as u32),
            c => c.to_string(),
        };
        write!(vocab_entries, "\"{key}\":{b}").unwrap();
    }
    vocab_entries.push('}');
    format!(
        r#"{{
            "added_tokens": [],
            "normalizer": {normalizer_json},
            "model": {{
                "type": "BPE",
                "vocab": {vocab_entries},
                "merges": []
            }}
        }}"#
    )
}

#[test]
fn bert_normalizer_deserialises_from_full_type_block() {
    // Every field explicit.
    let json = r#"{
        "added_tokens": [],
        "normalizer": {
            "type": "BertNormalizer",
            "clean_text": true,
            "handle_chinese_chars": true,
            "strip_accents": null,
            "lowercase": true
        },
        "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    match config.normalizer {
        Some(HfNormalizer::BertNormalizer {
            clean_text,
            handle_chinese_chars,
            strip_accents,
            lowercase,
        }) => {
            assert!(clean_text);
            assert!(handle_chinese_chars);
            assert_eq!(strip_accents, None);
            assert!(lowercase);
        }
        other => panic!("expected BertNormalizer, got {other:?}"),
    }
}

#[test]
fn bert_normalizer_deserialises_from_bare_type_block() {
    // Only `type` — every other field takes its default.
    let json = r#"{
        "added_tokens": [],
        "normalizer": {"type": "BertNormalizer"},
        "model": {"type": "BPE", "vocab": {"a": 0}, "merges": []}
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    match config.normalizer {
        Some(HfNormalizer::BertNormalizer {
            clean_text,
            handle_chinese_chars,
            strip_accents,
            lowercase,
        }) => {
            assert!(clean_text, "clean_text default must be true");
            assert!(
                handle_chinese_chars,
                "handle_chinese_chars default must be true"
            );
            assert_eq!(strip_accents, None, "strip_accents default must be None");
            assert!(lowercase, "lowercase default must be true");
        }
        other => panic!("expected BertNormalizer, got {other:?}"),
    }
}

#[test]
fn bert_normalizer_materialises_into_runtime_variant() {
    let json = bert_config(r#"{"type": "BertNormalizer"}"#);
    let config = parse_tokenizer_json(&json).unwrap();
    let tok = to_bpe_tokenizer(&config).unwrap();
    match tok.normalizer() {
        Some(Normalizer::Bert {
            clean_text,
            handle_chinese_chars,
            strip_accents,
            lowercase,
        }) => {
            assert!(*clean_text);
            assert!(*handle_chinese_chars);
            assert_eq!(*strip_accents, None);
            assert!(*lowercase);
        }
        other => panic!("expected Normalizer::Bert, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Individual pass semantics — verified through direct calls to
// `normalize` for readability. Full-pipeline end-to-end coverage
// lives in the encode-through-BPE tests below.
// ---------------------------------------------------------------------

#[test]
fn strip_accents_via_lowercase_pass_produces_cafe_from_cafe() {
    // The HF-canonical shape — `strip_accents: None` defers to
    // `lowercase: true`, so `"café"` becomes `"cafe"`. This is the
    // acceptance example from the wave-12 task brief.
    let n = Normalizer::Bert {
        clean_text: true,
        handle_chinese_chars: true,
        strip_accents: None,
        lowercase: true,
    };
    assert_eq!(normalize("café", &n), "cafe");
    // Uppercase input reaches the same output — lowercase runs after
    // strip_accents, so `É` decomposes to `E` + combining acute, the
    // acute is dropped, and `E` lower-cases to `e`.
    assert_eq!(normalize("CAFÉ", &n), "cafe");
}

#[test]
fn handle_chinese_chars_pads_han_scalars_with_ascii_space() {
    // Reference behaviour from HuggingFace's Python tokenizers:
    //
    //     >>> from tokenizers.normalizers import BertNormalizer
    //     >>> BertNormalizer(clean_text=False, handle_chinese_chars=True,
    //     ...                strip_accents=False, lowercase=False)
    //     ...     .normalize_str("你好world")
    //     ' 你  好 world'
    //
    // Every CJK codepoint (`你` and `好` here — both in the CJK
    // Unified Ideographs block) is padded with an ASCII space on
    // each side; adjacent Han chars therefore produce two spaces in
    // a row (one padding right of `你`, one padding left of `好`).
    let n = Normalizer::Bert {
        clean_text: false,
        handle_chinese_chars: true,
        strip_accents: Some(false),
        lowercase: false,
    };
    assert_eq!(normalize("你好world", &n), " 你  好 world");
}

#[test]
fn clean_text_drops_nul_and_replaces_tab_with_space() {
    // `\u{0000}` is dropped. `\t` and `\n` (whitespace, not
    // controls in HF's book) are replaced with a single ASCII space.
    let n = Normalizer::Bert {
        clean_text: true,
        handle_chinese_chars: false,
        strip_accents: Some(false),
        lowercase: false,
    };
    assert_eq!(normalize("a\u{0000}b\tc\nd", &n), "ab c d");
    // The Unicode replacement character (`U+FFFD`) is also dropped.
    assert_eq!(normalize("a\u{FFFD}b", &n), "ab");
    // Other C0 controls (SOH here) are dropped as controls.
    assert_eq!(normalize("a\u{0001}b", &n), "ab");
}

#[test]
fn clean_text_alone_leaves_ordinary_ascii_untouched() {
    let n = Normalizer::Bert {
        clean_text: true,
        handle_chinese_chars: false,
        strip_accents: Some(false),
        lowercase: false,
    };
    assert_eq!(normalize("Hello, world!", &n), "Hello, world!");
}

#[test]
fn strip_accents_explicit_false_preserves_accents_even_when_lowercasing() {
    // `strip_accents: Some(false)` overrides the "default to
    // lowercase" rule.
    let n = Normalizer::Bert {
        clean_text: false,
        handle_chinese_chars: false,
        strip_accents: Some(false),
        lowercase: true,
    };
    assert_eq!(normalize("CAFÉ", &n), "café");
}

// ---------------------------------------------------------------------
// End-to-end: the runtime normalizer is applied at encode time.
// ---------------------------------------------------------------------

#[test]
fn encode_through_bpe_applies_bert_normalizer() {
    // Full pipeline: parse config, materialise BpeTokenizer, encode
    // input, verify the encoded byte string equals what the normalizer
    // produced. The byte-alphabet vocab means each ASCII byte encodes
    // to itself and the decoded round-trip reads back as a String.
    let json = bert_config(
        r#"{"type": "BertNormalizer", "clean_text": true,
             "handle_chinese_chars": false, "strip_accents": true,
             "lowercase": true}"#,
    );
    let config = parse_tokenizer_json(&json).unwrap();
    let tok = to_bpe_tokenizer(&config).unwrap();
    // "CAFÉ" → strip_accents drops the acute → lowercase → "cafe".
    // Every char is a single byte after normalization.
    let enc = tok.encode("CAFÉ").unwrap();
    let ids: Vec<u32> = "cafe".bytes().map(u32::from).collect();
    assert_eq!(enc.ids, ids);
    // Decode reads back the normalized byte string.
    assert_eq!(tok.decode(&enc.ids).unwrap(), "cafe");
}

#[test]
fn produced_tokenizer_normalizer_matches_config_toggle_by_toggle() {
    // For every combination of the four toggles, verify that the
    // materialised `BpeTokenizer` carries a `Normalizer::Bert` whose
    // fields match the source config exactly. Belt-and-braces check
    // that no wiring on the conversion path silently rewrites a
    // toggle.
    for &clean_text in &[false, true] {
        for &handle_chinese_chars in &[false, true] {
            for &strip_accents in &[None, Some(false), Some(true)] {
                for &lowercase in &[false, true] {
                    let strip_json = match strip_accents {
                        None => "null".to_string(),
                        Some(b) => b.to_string(),
                    };
                    let normalizer_json = format!(
                        r#"{{"type": "BertNormalizer",
                            "clean_text": {clean_text},
                            "handle_chinese_chars": {handle_chinese_chars},
                            "strip_accents": {strip_json},
                            "lowercase": {lowercase}}}"#
                    );
                    let json = bert_config(&normalizer_json);
                    let config = parse_tokenizer_json(&json).unwrap();
                    let tok = to_bpe_tokenizer(&config).unwrap();
                    match tok.normalizer() {
                        Some(Normalizer::Bert {
                            clean_text: ct,
                            handle_chinese_chars: hc,
                            strip_accents: sa,
                            lowercase: lc,
                        }) => {
                            assert_eq!(*ct, clean_text);
                            assert_eq!(*hc, handle_chinese_chars);
                            assert_eq!(*sa, strip_accents);
                            assert_eq!(*lc, lowercase);
                        }
                        other => panic!("expected Normalizer::Bert, got {other:?}"),
                    }
                }
            }
        }
    }
}
