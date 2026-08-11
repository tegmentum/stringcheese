//! Host-side smoke test for the real cl100k tokenizer.
//!
//! Only runs when the `parity-real-vocab` feature is enabled AND
//! `build.rs` successfully staged a SHA-256-verified plaintext blob
//! into `$OUT_DIR`. Without the cfg the file compiles to an empty
//! test binary — a stub build passes vacuously, matching the
//! posture the sibling `stringcheese-tokenizer-tiktoken-conformance`
//! harness takes.
//!
//! # What the assertions check
//!
//! The known-good id sequences below are the ids upstream
//! `openai/tiktoken` (via `tiktoken-rs 0.6`) produces for the given
//! inputs; they were captured from an isolated harness run and are
//! pinned here so a regression to the merge builder or the
//! pre-tokenizer regex surfaces as a hard failure.
//!
//! * `"Hello, world!"` → `[9906, 11, 1917, 0]` — the canonical smoke
//!   input the task calls out. `9906` is `"Hello"`, `11` is `","`,
//!   `1917` is `" world"`, `0` is `"!"`. Any change to the merge
//!   priority reconstruction in
//!   `stringcheese_tokenizer_tiktoken::builder::build_scud_from_tiktoken`
//!   would perturb one of these.
//! * `"hello"` → `[15339]`. A single-word input exercises just the
//!   merge loop without pre-tokenizer boundary handling.
//! * `""` → `[]`. Empty input is the degenerate case that any
//!   pre-tokenizer + BPE combo has to handle without allocating a
//!   phantom leading token.
//!
//! # Compare against a specific tiktoken version
//!
//! The pinned ids are stable across every published tiktoken release
//! since 2023 (`cl100k_base`'s vocab has not been re-published), so
//! the assertion does not need to move in lockstep with the CI's
//! `tiktoken-rs` pin. If `OpenAI` ever republishes `cl100k_base`, the
//! SHA-256 constant in `build.rs` catches the drift before the
//! encoder is ever invoked.

// Feature-gated because integration tests do not inherit the
// `stringcheese_cl100k_real_vocab` cfg emitted by build.rs (that cfg
// only applies to the library crate itself). Under `parity-real-vocab`
// the build script guarantees the real blob is embedded — a missing
// blob is a hard build failure — so the feature flag is a reliable
// proxy for "the tokenizer is loadable at test time".
#![cfg(feature = "parity-real-vocab")]

use stringcheese_tokenizer_component_cl100k::{count, decode, encode, get_capabilities};

#[test]
fn hello_world_matches_upstream_tiktoken() {
    let enc = encode("Hello, world!").expect("real vocab is embedded");
    assert_eq!(
        enc.ids,
        vec![9906u32, 11, 1917, 0],
        "cl100k_base id sequence for 'Hello, world!' regressed"
    );
}

#[test]
fn hello_single_word_matches_upstream_tiktoken() {
    let enc = encode("hello").expect("encode succeeds");
    assert_eq!(enc.ids, vec![15339u32]);
}

#[test]
fn empty_input_produces_no_tokens() {
    let enc = encode("").expect("encode succeeds on empty input");
    assert!(enc.ids.is_empty());
    assert_eq!(count("").expect("count succeeds"), 0);
}

#[test]
fn count_matches_encode_length() {
    for input in [
        "Hello, world!",
        "hello",
        "The quick brown fox jumps over the lazy dog.",
        "cafe\u{0301}", // combining accent — exercises Unicode paths.
    ] {
        let enc = encode(input).expect("encode succeeds");
        let n = count(input).expect("count succeeds");
        assert_eq!(
            u32::try_from(enc.ids.len()).unwrap(),
            n,
            "count/encode disagree on {input:?}"
        );
    }
}

#[test]
fn decode_round_trips_hello_world() {
    let text = "Hello, world!";
    let enc = encode(text).expect("encode succeeds");
    let out = decode(&enc.ids).expect("decode succeeds");
    assert_eq!(out, text);
}

#[test]
fn capabilities_report_real_vocab_shape() {
    let caps = get_capabilities();
    assert_eq!(caps.model_type, "bpe");
    assert_eq!(caps.variant_id, "cl100k_base");
    // cl100k's published vocab is 100 261 mergeable-ranks entries
    // (the four <|...|> special tokens are NOT registered — see the
    // module docs on `Cl100kCapabilities::has_special_tokens`).
    assert!(
        caps.vocab_size >= 100_000,
        "cl100k vocab size must be at least 100 000; got {}",
        caps.vocab_size
    );
    assert!(!caps.has_byte_fallback);
    assert!(!caps.has_special_tokens);
}
