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

use stringcheese_tokenizer_hf::hf::{HfPreTokenizer, HfPrependScheme, to_runtime_metaspace};
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
