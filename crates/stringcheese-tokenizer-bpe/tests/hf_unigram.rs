//! Integration tests for the Hugging Face Unigram model loader.
//!
//! Covers the Viterbi forward-DP algorithm and the `unk_id` fallback
//! path. Every fixture is a small hand-crafted `tokenizer.json` blob
//! written inline; no real tokenizer.json files are checked in.

#![cfg(feature = "hf-tokenizer")]

use stringcheese_tokenizer_bpe::hf::{
    HfConversionError, HfModel, HfTokenizer, UnigramEncodeError, UnigramTokenizer,
    parse_tokenizer_json, to_tokenizer, to_unigram_tokenizer,
};

/// A tiny vocabulary that exercises the "prefer longer / higher-score
/// segmentation" behaviour: the word `"hello"` can be produced either
/// as one piece (`"hello"`), as `"hel" + "lo"`, or as five individual
/// letters. The whole-word entry has the highest log probability, so
/// Viterbi should pick it.
const HELLO_JSON: &str = r#"{
    "added_tokens": [],
    "model": {
        "type": "Unigram",
        "vocab": [
            ["<unk>", 0.0],
            ["h", -5.0],
            ["e", -5.0],
            ["l", -5.0],
            ["o", -5.0],
            ["w", -5.0],
            ["r", -5.0],
            ["d", -5.0],
            ["hel", -3.5],
            ["lo", -3.5],
            ["hello", -2.0],
            ["world", -2.0]
        ],
        "unk_id": 0
    }
}"#;

fn hello_tokenizer() -> UnigramTokenizer {
    let config = parse_tokenizer_json(HELLO_JSON).unwrap();
    to_unigram_tokenizer(&config).unwrap()
}

#[test]
fn parses_typed_unigram_model() {
    let config = parse_tokenizer_json(HELLO_JSON).unwrap();
    match &config.model {
        HfModel::Unigram(uni) => {
            assert_eq!(uni.vocab.len(), 12);
            assert_eq!(uni.unk_id, Some(0));
            assert_eq!(uni.vocab[10].0, "hello");
            assert!((uni.vocab[10].1 - -2.0_f64).abs() < 1e-9);
        }
        other => panic!("expected Unigram model, got {other:?}"),
    }
}

#[test]
fn viterbi_picks_whole_word_when_it_is_the_highest_scoring_path() {
    let tok = hello_tokenizer();
    // "hello" — whole-word (id 10) scores -2.0; "hel"+"lo" (ids 8+9)
    // scores -3.5 + -3.5 = -7.0; five letters would score -25.0. The
    // whole word wins.
    let ids = tok.encode("hello").unwrap();
    assert_eq!(ids, vec![10]);
}

#[test]
fn viterbi_picks_two_piece_split_when_it_beats_char_by_char() {
    // Vocab where the only whole-word covering is "helloworld"
    // (score -1.0), plus prefix/suffix pieces. Verify the two-piece
    // split beats the char-by-char one.
    let json = r#"{
        "added_tokens": [],
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", 0.0],
                ["h", -5.0], ["e", -5.0], ["l", -5.0], ["o", -5.0],
                ["hel", -2.0], ["lo", -2.0]
            ],
            "unk_id": 0
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    // "hello" — "hel"+"lo" scores -4.0; char-by-char scores -25.0.
    // The two-piece split wins.
    let ids = tok.encode("hello").unwrap();
    assert_eq!(ids, vec![5, 6]);
}

#[test]
fn viterbi_falls_back_to_characters_when_no_longer_piece_exists() {
    // Vocab with only single letters — the only reachable path is
    // char-by-char.
    let json = r#"{
        "added_tokens": [],
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", 0.0],
                ["h", -1.0], ["i", -1.0]
            ],
            "unk_id": 0
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    assert_eq!(tok.encode("hi").unwrap(), vec![1, 2]);
}

#[test]
fn oov_character_falls_back_to_unk_when_configured() {
    // Same vocab as HELLO_JSON, but encode a string containing
    // characters *not* in the vocab (`"?"`). The tokenizer should
    // emit the `unk_id` (0) for the unknown char and normal ids for
    // the rest.
    let tok = hello_tokenizer();
    let ids = tok.encode("hi?").unwrap();
    // "h" → 1, "i" is not in vocab → 0 (unk), "?" is not in vocab → 0
    // (unk). Since we tokenize character-by-character on the unk path,
    // the "i" transition also uses unk (no "i" vocab entry).
    assert_eq!(ids, vec![1, 0, 0]);
}

#[test]
fn oov_character_errors_when_no_unk_configured() {
    // Same vocab shape as HELLO_JSON but without unk_id; encoding an
    // OOV character should surface `UntokenizableChar`.
    let json = r#"{
        "added_tokens": [],
        "model": {
            "type": "Unigram",
            "vocab": [
                ["h", -1.0],
                ["e", -1.0],
                ["l", -1.0],
                ["o", -1.0]
            ]
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    // "he!lo" — the `!` at char offset 2 is not in vocab and no
    // fallback is configured.
    let err = tok.encode("he!lo").unwrap_err();
    match err {
        UnigramEncodeError::UntokenizableChar { char_offset } => {
            assert_eq!(char_offset, 2);
        }
        other => panic!("expected UntokenizableChar, got {other:?}"),
    }
}

#[test]
fn empty_input_encodes_to_empty_output() {
    let tok = hello_tokenizer();
    assert_eq!(tok.encode("").unwrap(), Vec::<usize>::new());
}

#[test]
fn multibyte_characters_are_segmented_by_character_not_by_byte() {
    // A Unicode-heavy vocab: `é` is a two-byte character in UTF-8.
    // The Viterbi loop should treat it as one character, so a
    // whole-word `"café"` entry beats piece-wise segmentation.
    let json = r#"{
        "added_tokens": [],
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", 0.0],
                ["c", -5.0],
                ["a", -5.0],
                ["f", -5.0],
                ["é", -5.0],
                ["café", -1.0]
            ],
            "unk_id": 0
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    let ids = tok.encode("café").unwrap();
    assert_eq!(ids, vec![5]);
    // And falling back to character pieces when the whole word is
    // absent still respects the character (not byte) boundary.
    let json2 = r#"{
        "added_tokens": [],
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", 0.0],
                ["c", -1.0],
                ["a", -1.0],
                ["f", -1.0],
                ["é", -1.0]
            ],
            "unk_id": 0
        }
    }"#;
    let config2 = parse_tokenizer_json(json2).unwrap();
    let tok2 = to_unigram_tokenizer(&config2).unwrap();
    assert_eq!(tok2.encode("café").unwrap(), vec![1, 2, 3, 4]);
}

#[test]
fn to_tokenizer_produces_unigram_variant() {
    let config = parse_tokenizer_json(HELLO_JSON).unwrap();
    let tok = to_tokenizer(&config).unwrap();
    match tok {
        HfTokenizer::Unigram(uni) => {
            assert_eq!(uni.encode("hello").unwrap(), vec![10]);
        }
        other => panic!("expected HfTokenizer::Unigram, got {other:?}"),
    }
}

#[test]
fn unigram_unk_id_out_of_range_is_rejected() {
    // unk_id points past the end of the vocab.
    let json = r#"{
        "added_tokens": [],
        "model": {
            "type": "Unigram",
            "vocab": [["a", 0.0], ["b", -1.0]],
            "unk_id": 99
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let err = to_unigram_tokenizer(&config).unwrap_err();
    match err {
        HfConversionError::UnigramUnkIdOutOfRange { unk_id, vocab_size } => {
            assert_eq!(unk_id, 99);
            assert_eq!(vocab_size, 2);
        }
        other => panic!("expected UnigramUnkIdOutOfRange, got {other:?}"),
    }
}

#[test]
fn to_unigram_tokenizer_rejects_bpe_model() {
    let json = r#"{
        "added_tokens": [],
        "model": {
            "type": "BPE",
            "vocab": {"a": 0, "b": 1},
            "merges": []
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let err = to_unigram_tokenizer(&config).unwrap_err();
    match err {
        HfConversionError::UnsupportedModelForUnigram { type_name } => {
            assert_eq!(type_name, "BPE");
        }
        other => panic!("expected UnsupportedModelForUnigram(BPE), got {other:?}"),
    }
}

#[test]
fn unk_fallback_is_not_preferred_over_vocab_path() {
    // A vocab where a valid vocab-only path exists but is more
    // expensive than a naive unk-fallback would be at face value.
    // Confirms the unk_penalty is large enough that a vocab-only
    // path always wins.
    let json = r#"{
        "added_tokens": [],
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", -0.1],
                ["a", -100.0],
                ["b", -100.0]
            ],
            "unk_id": 0
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    // "ab" — vocab path scores -200.0. An unk-only path would score
    // 2 * (-0.1 - unk_penalty) = ~ -20.2. Even though the raw unk
    // score is better, the penalty is applied, but a vocab path is
    // available — and we implement the fallback only when a
    // position is *unreachable* via vocab. So the vocab path wins.
    assert_eq!(tok.encode("ab").unwrap(), vec![1, 2]);
}
