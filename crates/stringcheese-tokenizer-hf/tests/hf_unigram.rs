//! Integration tests for the Hugging Face Unigram model loader.
//!
//! Covers the Viterbi forward-DP algorithm and the `unk_id` fallback
//! path. Every fixture is a small hand-crafted `tokenizer.json` blob
//! written inline; no real tokenizer.json files are checked in.

#![cfg(feature = "hf-tokenizer")]

use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_hf::hf::{
    HfConversionError, HfModel, HfTokenizer, UnigramEncodeError, UnigramTokenizer,
    parse_tokenizer_json, to_tokenizer, to_unigram_tokenizer,
};
use stringcheese_tokenizer_hf::normalizer::Normalizer;
use stringcheese_tokenizer_hf::post_processor::{PostProcessor, RobertaProcessing};
use stringcheese_tokenizer_hf::{Metaspace, PreTokenizer, PreTokenizerSequence, PrependScheme};

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
    // emit the `unk_id` (0) for the unknown chars — fused into a
    // single UNK because HF's Unigram default is
    // `fuse_unk = true`: any run of consecutive UNK transitions
    // surfaces as ONE UNK emission on the output side.
    let tok = hello_tokenizer();
    let ids = tok.encode("hi?").unwrap();
    // "h" → 1, then "i" and "?" (both not in vocab) fuse into a
    // single UNK id 0.
    assert_eq!(ids, vec![1, 0]);
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

// ---------------------------------------------------------------------
// Pipeline composition: normalize -> Metaspace -> Viterbi -> post-process
// ---------------------------------------------------------------------

/// Hand-crafted Metaspace-marked vocabulary. Every "word-initial"
/// piece starts with `▁` because that is the shape the `SentencePiece`
/// pre-tokenizer feeds into Viterbi.
const METASPACE_VOCAB_JSON: &str = r#"{
    "added_tokens": [],
    "model": {
        "type": "Unigram",
        "vocab": [
            ["<unk>", 0.0],
            ["▁hello", -1.0],
            ["▁world", -1.0],
            ["▁", -3.0],
            ["h", -5.0],
            ["e", -5.0],
            ["l", -5.0],
            ["o", -5.0]
        ],
        "unk_id": 0
    }
}"#;

#[test]
fn encode_runs_full_pipeline_normalize_metaspace_viterbi() {
    let config = parse_tokenizer_json(METASPACE_VOCAB_JSON).unwrap();
    let tok = to_unigram_tokenizer(&config)
        .unwrap()
        // A no-op normalizer that still exercises the wiring: NFC on
        // ASCII is identity.
        .with_normalizer(Normalizer::Nfc)
        .with_pre_tokenizer(Metaspace::new());
    // Pipeline: "hello world" -normalize-> "hello world"
    //           -metaspace-> ["▁hello", "▁world"]
    //           -viterbi-> [1, 2]
    let ids = tok.encode("hello world").unwrap();
    assert_eq!(ids, vec![1, 2]);
}

#[test]
fn encode_without_pre_tokenizer_preserves_pre_composition_behaviour() {
    // Same vocab, same input, but no Metaspace attached — the whole
    // string is passed to Viterbi as one piece and the marked pieces
    // in the vocab never fire (no `▁` in the raw input, so the
    // fallback single-char path is the only one available).
    let config = parse_tokenizer_json(METASPACE_VOCAB_JSON).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    // "hello" without the ▁ mark: char-by-char via the letter entries.
    let ids = tok.encode("hello").unwrap();
    assert_eq!(ids, vec![4, 5, 6, 6, 7]);
}

#[test]
fn tokenizer_trait_encode_splices_roberta_cls_and_sep() {
    let config = parse_tokenizer_json(METASPACE_VOCAB_JSON).unwrap();
    let tok = to_unigram_tokenizer(&config)
        .unwrap()
        .with_normalizer(Normalizer::Nfc)
        .with_pre_tokenizer(Metaspace::new())
        .with_post_processor(PostProcessor::RobertaProcessing(RobertaProcessing {
            sep: ("</s>".to_string(), 2),
            cls: ("<s>".to_string(), 0),
            trim_offsets: true,
            add_prefix_space: true,
        }));
    // Encode through the Tokenizer trait so the post-processor fires.
    let enc = Tokenizer::encode(&tok, "hello world").unwrap();
    // The primary encoding is [1, 2]; the RoBERTa splice wraps it
    // with CLS on the left and SEP on the right.
    assert_eq!(enc.ids, vec![0, 1, 2, 2]);
}

#[test]
fn decode_reverses_metaspace_substitution() {
    let config = parse_tokenizer_json(METASPACE_VOCAB_JSON).unwrap();
    let tok = to_unigram_tokenizer(&config)
        .unwrap()
        .with_pre_tokenizer(Metaspace::new());
    // Encode + decode round-trip through the trait surface.
    let raw = tok.encode("hello world").unwrap();
    assert_eq!(raw, vec![1, 2]);
    let text = tok.decode(&raw).unwrap();
    assert_eq!(text, "hello world");
}

#[test]
fn tokenizer_trait_decode_widens_ids() {
    // The trait's `decode` takes `&[u32]`; make sure the widen-cast
    // to the inherent method's `usize` slice works.
    let config = parse_tokenizer_json(METASPACE_VOCAB_JSON).unwrap();
    let tok = to_unigram_tokenizer(&config)
        .unwrap()
        .with_pre_tokenizer(Metaspace::new());
    let text = Tokenizer::decode(&tok, &[1u32, 2u32]).unwrap();
    assert_eq!(text, "hello world");
}

#[test]
fn pre_tokenizer_prepend_never_leaves_first_piece_unmarked() {
    // With PrependScheme::Never a non-space-prefixed input's first
    // piece has no leading ▁, so the vocab-marked prefixes don't
    // match at position 0 — this exercises the "Metaspace shape
    // choice matters" pathway.
    let config = parse_tokenizer_json(METASPACE_VOCAB_JSON).unwrap();
    let tok = to_unigram_tokenizer(&config)
        .unwrap()
        .with_pre_tokenizer(Metaspace {
            replacement: Metaspace::DEFAULT_REPLACEMENT,
            prepend_scheme: PrependScheme::Never,
            split: true,
        });
    // "hello world" under Never: pieces are ["hello", "▁world"] →
    // [char-by-char for hello] + [▁world = id 2].
    let ids = tok.encode("hello world").unwrap();
    assert_eq!(ids, vec![4, 5, 6, 6, 7, 2]);
}

// ---------------------------------------------------------------------
// HF-loader integration
// ---------------------------------------------------------------------

#[test]
fn hf_loader_wires_normalizer_metaspace_and_roberta_processor() {
    // Inline minimal xlm-roberta-shape tokenizer.json: SentencePiece
    // vocab with `▁hello` / `▁world` entries, an NFC normalizer, a
    // Metaspace pre-tokenizer, and a RobertaProcessing post-processor.
    // Verifies `to_tokenizer` returns a `HfTokenizer::Unigram` whose
    // encode runs the full pipeline end to end.
    let json = r#"{
        "added_tokens": [],
        "normalizer": {"type": "NFC"},
        "pre_tokenizer": {"type": "Metaspace"},
        "post_processor": {
            "type": "RobertaProcessing",
            "sep": ["</s>", 2],
            "cls": ["<s>", 0],
            "trim_offsets": true,
            "add_prefix_space": true
        },
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", 0.0],
                ["▁hello", -1.0],
                ["▁world", -1.0]
            ],
            "unk_id": 0
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_tokenizer(&config).unwrap();
    match tok {
        HfTokenizer::Unigram(uni) => {
            // Sanity: the loader attached each pipeline piece.
            assert!(uni.normalizer().is_some());
            assert!(uni.pre_tokenizer().is_some());
            assert!(matches!(
                uni.post_processor(),
                PostProcessor::RobertaProcessing(_)
            ));
            // Encode through the Tokenizer trait so the post-processor
            // fires; expected shape: [cls, ▁hello, ▁world, sep].
            let enc = Tokenizer::encode(&uni, "hello world").unwrap();
            assert_eq!(enc.ids, vec![0, 1, 2, 2]);
        }
        other => panic!("expected HfTokenizer::Unigram, got {other:?}"),
    }
}

#[test]
fn hf_loader_wires_metaspace_inside_sequence_wrapper() {
    // Some real configs wrap Metaspace inside a single-entry Sequence.
    // The loader must unwrap it and still attach a Metaspace runtime.
    let json = r#"{
        "added_tokens": [],
        "pre_tokenizer": {
            "type": "Sequence",
            "pretokenizers": [{"type": "Metaspace"}]
        },
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", 0.0],
                ["▁hi", -1.0]
            ],
            "unk_id": 0
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    assert!(tok.pre_tokenizer().is_some());
    assert_eq!(tok.encode("hi").unwrap(), vec![1]);
}

// ---------------------------------------------------------------------
// Byte-fallback (SentencePiece `byte_fallback: true`)
// ---------------------------------------------------------------------

/// Build a vocabulary containing the 256 reserved `<0x00>`..`<0xFF>`
/// byte-fallback tokens (at ids 3..259) plus the word tokens listed in
/// `extra_words`. Every byte token gets score `-3.0`, matching HF's
/// on-disk convention (byte tokens are cheaper than a real `unk` so
/// the fallback path is the natural loser when a vocab-only path
/// exists but the natural winner over `unk`). Extra words get score
/// `-1.0` — cheap enough that the whole-word path beats the byte
/// fallback whenever the word is in the vocab.
///
/// Returned tuple: `(json, byte_id_base)`. `byte_id_base` is the id of
/// the `<0x00>` token (so id of `<0xXX>` is `byte_id_base + XX`);
/// tests use this to spell out expected id lists.
fn byte_fallback_vocab_json(extra_words: &[&str]) -> (String, u32) {
    // ids 0..=2 reserved for <unk>, <s>, </s> — matches the Llama
    // convention. The byte-fallback tokens follow.
    let mut vocab_entries: Vec<String> = Vec::new();
    vocab_entries.push(r#"["<unk>", 0.0]"#.to_string());
    vocab_entries.push(r#"["<s>", 0.0]"#.to_string());
    vocab_entries.push(r#"["</s>", 0.0]"#.to_string());
    let byte_id_base: u32 = 3;
    for b in 0u32..=255 {
        vocab_entries.push(format!(r#"["<0x{b:02X}>", -3.0]"#));
    }
    for w in extra_words {
        // Extra words: score cheap enough to win against byte-fallback.
        vocab_entries.push(format!("[{w:?}, -1.0]"));
    }
    let json = format!(
        r#"{{
            "added_tokens": [],
            "model": {{
                "type": "Unigram",
                "vocab": [{}],
                "unk_id": 0,
                "byte_fallback": true
            }}
        }}"#,
        vocab_entries.join(",")
    );
    (json, byte_id_base)
}

#[test]
fn byte_fallback_construction_detects_all_256_tokens() {
    // A well-formed vocab: the 256 <0xXX> tokens are present so
    // construction succeeds.
    let (json, _base) = byte_fallback_vocab_json(&[]);
    let config = parse_tokenizer_json(&json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    assert!(tok.byte_fallback_enabled());
}

#[test]
fn byte_fallback_missing_tokens_are_rejected() {
    // A vocab that turns byte_fallback on but is missing the byte
    // tokens must fail construction with the specific error.
    let json = r#"{
        "added_tokens": [],
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", 0.0],
                ["<0x00>", -3.0],
                ["<0x01>", -3.0]
            ],
            "unk_id": 0,
            "byte_fallback": true
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let err = to_unigram_tokenizer(&config).unwrap_err();
    match err {
        HfConversionError::ByteFallbackTokensMissing {
            missing_count,
            first_missing_byte,
        } => {
            // 256 total - 2 present = 254 missing, first missing byte
            // is 0x02.
            assert_eq!(missing_count, 254);
            assert_eq!(first_missing_byte, 0x02);
        }
        other => panic!("expected ByteFallbackTokensMissing, got {other:?}"),
    }
}

#[test]
fn byte_fallback_disabled_still_returns_untokenizable_char() {
    // A config with `byte_fallback: false` (and no unk_id) should
    // still surface `UntokenizableChar` for an OOV character — the
    // byte-fallback path is opt-in.
    let json = r#"{
        "added_tokens": [],
        "model": {
            "type": "Unigram",
            "vocab": [
                ["h", -1.0],
                ["i", -1.0]
            ],
            "byte_fallback": false
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    assert!(!tok.byte_fallback_enabled());
    let err = tok.encode("h?").unwrap_err();
    match err {
        UnigramEncodeError::UntokenizableChar { char_offset } => {
            assert_eq!(char_offset, 1);
        }
        other => panic!("expected UntokenizableChar, got {other:?}"),
    }
}

#[test]
fn byte_fallback_emits_utf8_bytes_for_oov_char() {
    // A vocab with byte-fallback enabled AND a word entry: the word
    // must survive (whole-word path wins against bytes) and any OOV
    // char is emitted as its UTF-8 byte ids in order.
    let (json, base) = byte_fallback_vocab_json(&["hi"]);
    let config = parse_tokenizer_json(&json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();

    // "hi" is in the vocab, so it wins over 2 byte-fallback tokens.
    // With 256 byte tokens at ids 3..=258, the word "hi" sits at id 259.
    let ids = tok.encode("hi").unwrap();
    assert_eq!(ids, vec![259_usize]);

    // A single ASCII OOV char — `?` (0x3F). Byte fallback emits one
    // id: base + 0x3F.
    let ids = tok.encode("?").unwrap();
    assert_eq!(ids, vec![(base as usize) + 0x3F]);

    // A multi-byte UTF-8 OOV char — `é` (U+00E9 = 0xC3 0xA9). Byte
    // fallback emits two ids in forward byte order.
    let ids = tok.encode("é").unwrap();
    assert_eq!(ids, vec![(base as usize) + 0xC3, (base as usize) + 0xA9]);

    // A 3-byte OOV char — snowman `☃` (U+2603 = 0xE2 0x98 0x83).
    let ids = tok.encode("☃").unwrap();
    assert_eq!(
        ids,
        vec![
            (base as usize) + 0xE2,
            (base as usize) + 0x98,
            (base as usize) + 0x83
        ]
    );

    // A 4-byte OOV char — grinning-face emoji `😀`
    // (U+1F600 = 0xF0 0x9F 0x98 0x80).
    let ids = tok.encode("😀").unwrap();
    assert_eq!(
        ids,
        vec![
            (base as usize) + 0xF0,
            (base as usize) + 0x9F,
            (base as usize) + 0x98,
            (base as usize) + 0x80
        ]
    );

    // Mixed word + OOV char + word — the byte-fallback path composes
    // with the vocab path.
    let ids = tok.encode("hi?hi").unwrap();
    assert_eq!(ids, vec![259_usize, (base as usize) + 0x3F, 259_usize]);
}

#[test]
fn byte_fallback_round_trips_through_decode() {
    // Round-trip: encode → decode reconstructs the original input,
    // even for OOV characters that go through the byte-fallback path.
    let (json, _base) = byte_fallback_vocab_json(&["hi"]);
    let config = parse_tokenizer_json(&json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();

    // Every input here contains characters that must go through byte
    // fallback (none of them are in the extra_words list).
    let inputs = [
        "?",      // ASCII OOV
        "é",      // 2-byte
        "☃",      // 3-byte
        "😀",     // 4-byte
        "hi?",    // vocab + 1-byte
        "hi😀hi", // vocab + 4-byte + vocab
        "?éhi",   // mixed
        "?????",  // repeated 1-byte
    ];
    let mut passed = 0usize;
    for input in inputs {
        let ids = tok.encode(input).unwrap();
        let decoded = tok.decode(&ids).unwrap();
        assert_eq!(decoded, input, "round-trip failed for {input:?}");
        passed += 1;
    }
    assert_eq!(passed, inputs.len());
}

#[test]
fn byte_fallback_wins_over_unk_when_both_configured() {
    // With byte-fallback on and an unk_id also configured, an OOV
    // char takes the byte-fallback path (not the unk path).
    let (json, base) = byte_fallback_vocab_json(&[]);
    let config = parse_tokenizer_json(&json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    assert_eq!(tok.unk_id(), Some(0));
    // "?" — ASCII 0x3F. byte-fallback emits one id at base + 0x3F.
    // The unk_id (0) must NOT appear.
    let ids = tok.encode("?").unwrap();
    assert_eq!(ids, vec![(base as usize) + 0x3F]);
    assert!(!ids.contains(&0_usize));
}

#[test]
fn byte_fallback_accepts_lowercase_hex_surface() {
    // Some vocabs might ship `<0xff>` instead of `<0xFF>`. Both must
    // be recognised as the byte-0xFF token. Build a vocab by hand
    // that uses lowercase hex for byte 0xFF only.
    let mut vocab_entries: Vec<String> = Vec::new();
    vocab_entries.push(r#"["<unk>", 0.0]"#.to_string());
    for b in 0u32..0xFF {
        vocab_entries.push(format!(r#"["<0x{b:02X}>", -3.0]"#));
    }
    // The last byte token uses lowercase hex — still valid.
    vocab_entries.push(r#"["<0xff>", -3.0]"#.to_string());
    let json = format!(
        r#"{{
            "added_tokens": [],
            "model": {{
                "type": "Unigram",
                "vocab": [{}],
                "unk_id": 0,
                "byte_fallback": true
            }}
        }}"#,
        vocab_entries.join(",")
    );
    let config = parse_tokenizer_json(&json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    assert!(tok.byte_fallback_enabled());
    // Encoding a character whose UTF-8 includes byte 0xFF hits the
    // lowercase-hex entry. Byte 0xFF alone is not valid UTF-8, but a
    // 2-byte char like `ÿ` (U+00FF = 0xC3 0xBF) doesn't hit 0xFF; use
    // the 4-byte char U+10FFFF (last valid Unicode scalar) which
    // encodes as F4 8F BF BF — still no 0xFF.
    // Confirm via a synthetic decode instead: build an id list that
    // uses the 0xFF entry and check its byte is recovered on decode.
    // <unk> at 0, byte tokens at 1..=256. Byte 0xFF is at id 256.
    let decoded = tok.decode(&[256_usize]).unwrap();
    // Byte 0xFF alone is invalid UTF-8; from_utf8_lossy emits U+FFFD.
    assert_eq!(decoded, "\u{FFFD}");
}

#[test]
fn byte_fallback_survives_normalization_ordering() {
    // The audit called out a specific interaction: byte-fallback fires
    // in the Viterbi loop, which runs AFTER the normalizer. A
    // Precompiled charsmap (or NFC, or any other normalizer) may
    // rewrite the input before Viterbi sees it — the byte-fallback
    // path must apply to the normalized bytes, not the raw ones.
    //
    // NFC on ASCII is identity, so use a decomposed input that NFC
    // composes: "e\u{0301}" (e + combining acute accent) → "é" under
    // NFC. Without byte-fallback the composed char would be OOV; with
    // byte-fallback + NFC the composed 2-byte char goes through byte
    // fallback correctly.
    let (json, base) = byte_fallback_vocab_json(&[]);
    let config = parse_tokenizer_json(&json).unwrap();
    let tok = to_unigram_tokenizer(&config)
        .unwrap()
        .with_normalizer(Normalizer::Nfc);
    // "e\u{0301}" is 3 bytes (`e` + 2-byte combining acute). NFC
    // composes it to `é` (2 bytes: 0xC3 0xA9). Byte fallback should
    // emit the 2-byte encoding, not the raw 3-byte pre-NFC one.
    let ids = tok.encode("e\u{0301}").unwrap();
    assert_eq!(ids, vec![(base as usize) + 0xC3, (base as usize) + 0xA9]);
}

#[test]
fn byte_fallback_hf_loader_wires_it_end_to_end() {
    // A minimal Llama-shape config: Unigram model with byte_fallback,
    // no normalizer, no pre-tokenizer. Verify to_tokenizer routes
    // through the Unigram materialiser and the byte-fallback path is
    // active on the returned tokenizer.
    let (json, base) = byte_fallback_vocab_json(&["hi"]);
    let config = parse_tokenizer_json(&json).unwrap();
    let tok = to_tokenizer(&config).unwrap();
    match tok {
        HfTokenizer::Unigram(uni) => {
            assert!(uni.byte_fallback_enabled());
            // ASCII `?` (0x3F) — the byte-fallback path fires.
            let ids = uni.encode("?").unwrap();
            assert_eq!(ids, vec![(base as usize) + 0x3F]);
        }
        other => panic!("expected HfTokenizer::Unigram, got {other:?}"),
    }
}

#[test]
fn hf_loader_wires_whitespace_split_metaspace_sequence() {
    // The xlm-roberta-base composition: WhitespaceSplit followed by
    // Metaspace. The loader must materialise a two-stage sequence
    // whose apply collapses runs of whitespace before Metaspace
    // inserts its `▁` markers — three interior spaces must not become
    // three consecutive `▁` pieces.
    let json = r#"{
        "added_tokens": [],
        "pre_tokenizer": {
            "type": "Sequence",
            "pretokenizers": [
                {"type": "WhitespaceSplit"},
                {"type": "Metaspace"}
            ]
        },
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", 0.0],
                ["▁hello", -1.0],
                ["▁world", -1.0]
            ],
            "unk_id": 0
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();

    // Sanity: two stages in the expected order.
    let seq = tok
        .pre_tokenizer()
        .expect("pre_tokenizer sequence was not attached");
    assert_eq!(seq.stages().len(), 2);
    assert!(matches!(seq.stages()[0], PreTokenizer::WhitespaceSplit));
    assert!(matches!(seq.stages()[1], PreTokenizer::Metaspace(_)));

    // Single-space input: identical to the bare-Metaspace baseline.
    assert_eq!(tok.encode("hello world").unwrap(), vec![1, 2]);
    // Three interior spaces: collapse under WhitespaceSplit, then a
    // single `▁` per remaining word.
    assert_eq!(tok.encode("hello   world").unwrap(), vec![1, 2]);
    // Leading and trailing whitespace runs are dropped by
    // WhitespaceSplit before Metaspace sees anything.
    assert_eq!(tok.encode("   hello world   ").unwrap(), vec![1, 2]);
}

#[test]
fn hf_loader_bare_whitespace_split_wires_single_stage_sequence() {
    // A bare {"type":"WhitespaceSplit"} block (no Metaspace) is
    // accepted as a single-stage sequence — this is not what any real
    // Unigram checkpoint ships, but the sequence acceptance rule
    // permits it and it exercises the trivial-composition path.
    let json = r#"{
        "added_tokens": [],
        "pre_tokenizer": {"type": "WhitespaceSplit"},
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", 0.0],
                ["hello", -1.0],
                ["world", -1.0]
            ],
            "unk_id": 0
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    let seq = tok
        .pre_tokenizer()
        .expect("pre_tokenizer sequence was not attached");
    assert_eq!(seq.stages().len(), 1);
    assert!(matches!(seq.stages()[0], PreTokenizer::WhitespaceSplit));
    assert_eq!(tok.encode("hello world").unwrap(), vec![1, 2]);
    assert_eq!(tok.encode("hello   world").unwrap(), vec![1, 2]);
}

#[test]
fn hf_loader_bare_metaspace_still_wires_single_stage_sequence() {
    // A bare Metaspace (the pre-composition shape most Llama / Mistral
    // / T5 configs use) must still work — the loader wraps it in a
    // single-stage sequence via `From<Metaspace>`.
    let json = r#"{
        "added_tokens": [],
        "pre_tokenizer": {"type": "Metaspace"},
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", 0.0],
                ["▁hi", -1.0]
            ],
            "unk_id": 0
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_unigram_tokenizer(&config).unwrap();
    let seq = tok
        .pre_tokenizer()
        .expect("pre_tokenizer sequence was not attached");
    assert_eq!(seq.stages().len(), 1);
    assert!(matches!(seq.stages()[0], PreTokenizer::Metaspace(_)));
    // The Metaspace helper still surfaces the wrapped Metaspace for
    // decode-side callers (backward compat with pre-composition code).
    assert!(seq.metaspace().is_some());
    assert_eq!(tok.encode("hi").unwrap(), vec![1]);
}

#[test]
fn with_pre_tokenizer_accepts_bare_metaspace_via_into() {
    // Backward compatibility of the `with_pre_tokenizer` builder: the
    // pre-composition call site `tok.with_pre_tokenizer(Metaspace{...})`
    // still compiles and behaves identically, because Metaspace now
    // implements `Into<PreTokenizerSequence>` via a From impl.
    let json = r#"{
        "added_tokens": [],
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", 0.0],
                ["▁hello", -1.0]
            ],
            "unk_id": 0
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let tok = to_unigram_tokenizer(&config)
        .unwrap()
        .with_pre_tokenizer(Metaspace::new());
    assert_eq!(tok.encode("hello").unwrap(), vec![1]);
}

#[test]
fn with_pre_tokenizer_accepts_composed_sequence() {
    let json = r#"{
        "added_tokens": [],
        "model": {
            "type": "Unigram",
            "vocab": [
                ["<unk>", 0.0],
                ["▁hello", -1.0],
                ["▁world", -1.0]
            ],
            "unk_id": 0
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let seq = PreTokenizerSequence::new(vec![
        PreTokenizer::WhitespaceSplit,
        PreTokenizer::Metaspace(Metaspace::new()),
    ]);
    let tok = to_unigram_tokenizer(&config)
        .unwrap()
        .with_pre_tokenizer(seq);
    assert_eq!(tok.encode("hello   world").unwrap(), vec![1, 2]);
}

#[test]
fn hf_loader_rejects_ambiguous_multi_entry_pre_tokenizer_sequence() {
    // A Sequence with two children is ambiguous — the loader must
    // surface `AmbiguousSequencePreTokenizer` instead of picking one.
    let json = r#"{
        "added_tokens": [],
        "pre_tokenizer": {
            "type": "Sequence",
            "pretokenizers": [
                {"type": "Metaspace"},
                {"type": "Whitespace"}
            ]
        },
        "model": {
            "type": "Unigram",
            "vocab": [["<unk>", 0.0]],
            "unk_id": 0
        }
    }"#;
    let config = parse_tokenizer_json(json).unwrap();
    let err = to_unigram_tokenizer(&config).unwrap_err();
    match err {
        HfConversionError::AmbiguousSequencePreTokenizer { child_count } => {
            assert_eq!(child_count, 2);
        }
        other => panic!("expected AmbiguousSequencePreTokenizer, got {other:?}"),
    }
}
