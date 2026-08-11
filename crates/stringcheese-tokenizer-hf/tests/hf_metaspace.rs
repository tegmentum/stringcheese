//! Integration tests for the Hugging Face `Metaspace` pre-tokenizer.
//!
//! `Metaspace` is `SentencePiece`'s canonical space-marking pre-tokenizer
//! — every ASCII space becomes `▁` (U+2581) so downstream Unigram / BPE
//! sees token-initial position uniformly. Llama, Mistral, T5, and
//! XLM-RoBERTa checkpoints all ship one.
//!
//! The reference for the exact semantics is `HuggingFace`'s
//! `tokenizers` crate (`pre_tokenizers::metaspace::Metaspace`); this
//! file exercises the parse-time typed shape and the runtime
//! [`Metaspace::apply`] path materialised through
//! [`to_runtime_metaspace`].

#![cfg(feature = "hf-tokenizer")]

use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_hf::hf::{
    HfPreTokenizer, HfPrependScheme, parse_tokenizer_json, to_bpe_tokenizer, to_runtime_metaspace,
};
use stringcheese_tokenizer_hf::{Metaspace, PrependScheme};

// ---------------------------------------------------------------------
// Deserialisation.
// ---------------------------------------------------------------------

#[test]
fn deserialises_from_full_type_block() {
    let json = r#"{
        "type": "Metaspace",
        "replacement": "▁",
        "prepend_scheme": "always",
        "split": true
    }"#;
    let pt: HfPreTokenizer = serde_json::from_str(json).unwrap();
    match pt {
        HfPreTokenizer::Metaspace {
            replacement,
            prepend_scheme,
            split,
        } => {
            assert_eq!(replacement, '\u{2581}');
            assert_eq!(prepend_scheme, HfPrependScheme::Always);
            assert!(split);
        }
        other => panic!("expected Metaspace, got {other:?}"),
    }
}

#[test]
fn deserialises_from_bare_type_block_with_defaults() {
    // Only the `type` tag — every other field must take the HF-
    // canonical default (▁, "always", true).
    let json = r#"{"type": "Metaspace"}"#;
    let pt: HfPreTokenizer = serde_json::from_str(json).unwrap();
    match pt {
        HfPreTokenizer::Metaspace {
            replacement,
            prepend_scheme,
            split,
        } => {
            assert_eq!(
                replacement, '\u{2581}',
                "default replacement must be U+2581"
            );
            assert_eq!(
                prepend_scheme,
                HfPrependScheme::Always,
                "default prepend_scheme must be Always"
            );
            assert!(split, "default split must be true");
        }
        other => panic!("expected Metaspace, got {other:?}"),
    }
}

#[test]
fn deserialises_prepend_scheme_never_and_first() {
    for (raw, expected) in [
        ("never", HfPrependScheme::Never),
        ("first", HfPrependScheme::First),
        ("always", HfPrependScheme::Always),
    ] {
        let json = format!(r#"{{"type": "Metaspace", "prepend_scheme": "{raw}"}}"#);
        let pt: HfPreTokenizer = serde_json::from_str(&json).unwrap();
        match pt {
            HfPreTokenizer::Metaspace { prepend_scheme, .. } => {
                assert_eq!(prepend_scheme, expected, "failed on {raw}");
            }
            other => panic!("expected Metaspace for {raw}, got {other:?}"),
        }
    }
}

#[test]
fn deserialises_custom_replacement_char() {
    let json = r#"{"type": "Metaspace", "replacement": "_"}"#;
    let pt: HfPreTokenizer = serde_json::from_str(json).unwrap();
    match pt {
        HfPreTokenizer::Metaspace { replacement, .. } => assert_eq!(replacement, '_'),
        other => panic!("expected Metaspace, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Materialisation via `to_runtime_metaspace`.
// ---------------------------------------------------------------------

#[test]
fn to_runtime_metaspace_carries_defaults_forward() {
    let json = r#"{"type": "Metaspace"}"#;
    let pt: HfPreTokenizer = serde_json::from_str(json).unwrap();
    let runtime = to_runtime_metaspace(&pt).unwrap();
    assert_eq!(runtime.replacement, Metaspace::DEFAULT_REPLACEMENT);
    assert_eq!(runtime.prepend_scheme, PrependScheme::Always);
    assert!(runtime.split);
}

#[test]
fn to_runtime_metaspace_carries_explicit_fields_forward() {
    let json = r#"{
        "type": "Metaspace",
        "replacement": "_",
        "prepend_scheme": "never",
        "split": false
    }"#;
    let pt: HfPreTokenizer = serde_json::from_str(json).unwrap();
    let runtime = to_runtime_metaspace(&pt).unwrap();
    assert_eq!(runtime.replacement, '_');
    assert_eq!(runtime.prepend_scheme, PrependScheme::Never);
    assert!(!runtime.split);
}

#[test]
fn to_runtime_metaspace_rejects_non_metaspace_pre_tokenizer() {
    let json = r#"{"type": "Whitespace"}"#;
    let pt: HfPreTokenizer = serde_json::from_str(json).unwrap();
    to_runtime_metaspace(&pt)
        .expect_err("to_runtime_metaspace must reject non-Metaspace pre-tokenizers");
}

// ---------------------------------------------------------------------
// End-to-end apply semantics — direct against the runtime `Metaspace`.
//
// Cases verified against the Hugging Face Python reference:
//
//     >>> from tokenizers.pre_tokenizers import Metaspace
//     >>> Metaspace().pre_tokenize_str("hello world")
//     [('▁hello', (0, 5)), ('▁world', (5, 11))]
//     >>> Metaspace(prepend_scheme="never").pre_tokenize_str("hello world")
//     [('hello', (0, 5)), ('▁world', (5, 11))]
//     >>> Metaspace(prepend_scheme="first").pre_tokenize_str("▁already prefixed")
//     [('▁already', ...), ('▁prefixed', ...)]
// ---------------------------------------------------------------------

#[test]
fn apply_hello_world_with_defaults_yields_two_marked_pieces() {
    // The canonical acceptance example from the task brief.
    let ms = to_runtime_metaspace(
        &serde_json::from_str::<HfPreTokenizer>(r#"{"type": "Metaspace"}"#).unwrap(),
    )
    .unwrap();
    assert_eq!(
        ms.apply("hello world"),
        vec!["\u{2581}hello".to_string(), "\u{2581}world".to_string()]
    );
}

#[test]
fn apply_prepend_never_leaves_first_piece_unmarked() {
    // `prepend_scheme: never` on "hello world" — every space becomes
    // `▁` but nothing is prepended. Split then gives two pieces, the
    // first one unmarked.
    let ms = to_runtime_metaspace(
        &serde_json::from_str::<HfPreTokenizer>(
            r#"{"type": "Metaspace", "prepend_scheme": "never"}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        ms.apply("hello world"),
        vec!["hello".to_string(), "\u{2581}world".to_string()]
    );
}

#[test]
fn apply_prepend_first_does_not_double_prepend_when_already_marked() {
    // The acceptance case in the task brief: an input that already
    // starts with `▁` must not gain another one under `first`.
    let ms = to_runtime_metaspace(
        &serde_json::from_str::<HfPreTokenizer>(
            r#"{"type": "Metaspace", "prepend_scheme": "first"}"#,
        )
        .unwrap(),
    )
    .unwrap();
    let pieces = ms.apply("\u{2581}already prefixed");
    // Only two pieces, each starting with a single `▁`.
    assert_eq!(pieces.len(), 2);
    assert_eq!(pieces[0], "\u{2581}already");
    assert_eq!(pieces[1], "\u{2581}prefixed");
    // Assert-no-double-prepend: no piece may start with two `▁`.
    for p in &pieces {
        assert!(
            !p.starts_with("\u{2581}\u{2581}"),
            "piece {p:?} was double-prepended"
        );
    }
}

#[test]
fn apply_prepend_first_prepends_when_missing() {
    // `first` on an input that does not start with `▁` behaves like
    // `always`.
    let ms = to_runtime_metaspace(
        &serde_json::from_str::<HfPreTokenizer>(
            r#"{"type": "Metaspace", "prepend_scheme": "first"}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        ms.apply("hello world"),
        vec!["\u{2581}hello".to_string(), "\u{2581}world".to_string()]
    );
}

#[test]
fn apply_split_false_returns_single_transformed_piece() {
    let ms = to_runtime_metaspace(
        &serde_json::from_str::<HfPreTokenizer>(r#"{"type": "Metaspace", "split": false}"#)
            .unwrap(),
    )
    .unwrap();
    // Default prepend_scheme = always; without splitting the whole
    // transformed string comes back as one piece.
    assert_eq!(
        ms.apply("hello world"),
        vec!["\u{2581}hello\u{2581}world".to_string()]
    );
}

#[test]
fn apply_empty_input_is_empty_output() {
    let ms = to_runtime_metaspace(
        &serde_json::from_str::<HfPreTokenizer>(r#"{"type": "Metaspace"}"#).unwrap(),
    )
    .unwrap();
    assert!(ms.apply("").is_empty());
}

// ---------------------------------------------------------------------
// BPE loader — Mistral-family shape (Metaspace on the pre-tokenizer
// side, character-BPE with SentencePiece byte_fallback). Every real
// Mistral tokenizer.json ships this layout; the tests below assemble
// an inline synthetic vocab that reproduces the shape without needing
// the real 32k vocabulary on disk.
// ---------------------------------------------------------------------

/// Build a Mistral-shape `tokenizer.json` string with a hand-crafted
/// vocab / merges: byte-alphabet at ids 0..256 (so byte-fallback has
/// its 256 reserved tokens), plus a few UTF-8 character surfaces to
/// exercise the Metaspace + character-BPE path end-to-end.
fn mistral_shape_tokenizer_json(pre_tok_json: &str) -> String {
    use std::fmt::Write as _;

    // The 256 reserved `<0xXX>` byte-fallback tokens are declared as
    // added_tokens so `to_bpe_tokenizer` finds them; the actual vocab
    // entries carry the same ids so the tokenizer can round-trip.
    let mut added = String::new();
    for b in 0u32..256 {
        let _ = write!(
            added,
            r#"{{"id": {b}, "content": "<0x{b:02X}>", "special": false, "single_word": false, "lstrip": false, "rstrip": false, "normalized": false}},"#,
        );
    }
    // Trim trailing comma so the array is well-formed.
    if added.ends_with(',') {
        added.pop();
    }

    // Vocab: byte-fallback names for 0..256, then UTF-8 pieces used
    // by the tests. `▁` = ▁ = U+2581. IDs 300+ picked so they
    // don't collide with the byte range.
    let mut vocab_entries = String::new();
    for b in 0u32..256 {
        let _ = write!(vocab_entries, r#""<0x{b:02X}>": {b},"#);
    }
    vocab_entries.push_str(r#""▁": 300,"#);
    vocab_entries.push_str(r#""▁h": 301,"#);
    vocab_entries.push_str(r#""▁he": 302,"#);
    vocab_entries.push_str(r#""▁hi": 303,"#);
    vocab_entries.push_str(r#""▁hel": 304,"#);
    vocab_entries.push_str(r#""▁hell": 305,"#);
    vocab_entries.push_str(r#""▁hello": 306,"#);
    vocab_entries.push_str(r#""▁w": 307,"#);
    vocab_entries.push_str(r#""▁wo": 308,"#);
    vocab_entries.push_str(r#""▁wor": 309,"#);
    vocab_entries.push_str(r#""▁worl": 310,"#);
    vocab_entries.push_str(r#""▁world": 311"#);

    // Merges: compose the byte alphabet up to `▁hello` / `▁world` /
    // `▁hi` in the same left-to-right order the BPE loop walks.
    let merges = r#"[
        ["▁", "h"],
        ["▁h", "e"],
        ["▁h", "i"],
        ["▁he", "l"],
        ["▁hel", "l"],
        ["▁hell", "o"],
        ["▁", "w"],
        ["▁w", "o"],
        ["▁wo", "r"],
        ["▁wor", "l"],
        ["▁worl", "d"]
    ]"#;

    format!(
        r#"{{
            "added_tokens": [{added}],
            "pre_tokenizer": {pre_tok_json},
            "model": {{
                "type": "BPE",
                "vocab": {{{vocab_entries}}},
                "merges": {merges},
                "byte_fallback": true
            }}
        }}"#,
    )
}

#[test]
fn bpe_loader_accepts_bare_metaspace_pre_tokenizer() {
    // Before Wave-14 the BPE loader rejected any Metaspace shape with
    // `UnsupportedPreTokenizer { type_name: "Metaspace" }`. Now the
    // Mistral-canonical `{ "type": "Metaspace", "prepend_scheme":
    // "first", "split": false }` block loads.
    let json = mistral_shape_tokenizer_json(
        r#"{"type": "Metaspace", "replacement": "▁", "prepend_scheme": "first", "split": false}"#,
    );
    let config = parse_tokenizer_json(&json).expect("parse tokenizer.json");
    let tok = to_bpe_tokenizer(&config).expect("loader must accept Metaspace on BPE");
    // Sanity check that the sequence stuck.
    assert!(tok.pre_tokenizer_sequence().is_some());
    assert!(tok.byte_fallback_enabled());
}

#[test]
fn bpe_loader_accepts_metaspace_inside_sequence_wrapper() {
    // A `Sequence[Metaspace]` block — HF often wraps single stages in
    // a Sequence for uniformity — is treated the same as a bare
    // Metaspace.
    let json = mistral_shape_tokenizer_json(
        r#"{"type": "Sequence", "pretokenizers": [{"type": "Metaspace", "prepend_scheme": "always", "split": true}]}"#,
    );
    let config = parse_tokenizer_json(&json).expect("parse tokenizer.json");
    let tok = to_bpe_tokenizer(&config).expect("loader must accept Sequence[Metaspace] on BPE");
    let seq = tok
        .pre_tokenizer_sequence()
        .expect("sequence must be wired");
    assert!(seq.metaspace().is_some());
}

#[test]
fn bpe_loader_end_to_end_encodes_mistral_shape_input() {
    // Full end-to-end: load a Mistral-shape config, encode a canonical
    // input, and verify the character-BPE + Metaspace path produces the
    // expected id sequence for the hand-crafted vocab.
    let json = mistral_shape_tokenizer_json(
        r#"{"type": "Metaspace", "replacement": "▁", "prepend_scheme": "first", "split": false}"#,
    );
    let config = parse_tokenizer_json(&json).expect("parse");
    let tok = to_bpe_tokenizer(&config).expect("materialise");

    // "hello world" → Metaspace(split=false, prepend=first) transforms
    // to "▁hello▁world"; character-BPE merges compose `▁hello` (306)
    // and `▁world` (311).
    let enc = Tokenizer::encode(&tok, "hello world").expect("encode");
    assert_eq!(enc.ids, vec![306, 311]);

    // "hi" → "▁hi" → 303.
    let enc = Tokenizer::encode(&tok, "hi").expect("encode");
    assert_eq!(enc.ids, vec![303]);

    // Already-prefixed input with `first` scheme must NOT double-prepend.
    let enc = Tokenizer::encode(&tok, "\u{2581}hello").expect("encode");
    assert_eq!(enc.ids, vec![306]);
}

#[test]
fn bpe_loader_rejects_metaspace_sequence_with_split_regex_sibling() {
    // A Sequence combining Metaspace with an incompatible sibling
    // (Split(Regex)) must surface AmbiguousSequencePreTokenizer.
    let json = mistral_shape_tokenizer_json(
        r#"{"type": "Sequence", "pretokenizers": [
            {"type": "Metaspace"},
            {"type": "Split", "pattern": {"Regex": "\\s+"}, "behavior": "removed", "invert": false}
        ]}"#,
    );
    let config = parse_tokenizer_json(&json).expect("parse");
    let err = to_bpe_tokenizer(&config)
        .expect_err("loader must reject Metaspace mixed with Split(Regex)");
    let msg = err.to_string();
    assert!(
        msg.contains("Sequence") || msg.contains("ambiguous"),
        "unexpected error: {msg}"
    );
}
