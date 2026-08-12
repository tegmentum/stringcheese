//! Conformance corpus for `stringcheese-tokenizer-hf-native`.
//!
//! This runner is the hf-native mirror of the sibling
//! `stringcheese-tokenizer-hf` conformance suite (see
//! `crates/stringcheese-tokenizer-hf/tests/conformance.rs`). It loads
//! the *same* fixture files — one per real HF-shape checkpoint,
//! pairing a small corpus of diverse inputs with the reference
//! upstream token-id output — and asserts each case's ids match after
//! being encoded through this crate's
//! [`HfNativeTokenizer`](stringcheese_tokenizer_hf_native::HfNativeTokenizer),
//! which delegates verbatim to Hugging Face's own `tokenizers-rs`.
//!
//! # What "conformance" means for a delegating adapter
//!
//! Unlike the sibling crate, whose runtime is a from-scratch
//! reimplementation of the HF tokenizer pipeline, this crate's
//! `encode` path is a thin passthrough to
//! `tokenizers::Tokenizer::encode`. The reference the fixtures were
//! computed against — `transformers.AutoTokenizer` — is itself a
//! Python binding over the same `tokenizers-rs` crate, so the two are
//! byte-identical by construction on every case where the fixture
//! author did not run into a Python-vs-Rust wrapper divergence.
//! Practically that means:
//!
//! * A fixture failure here indicates a *fixture bug* (or a subtle
//!   wrapper divergence between the transformers Python surface and
//!   tokenizers-rs itself), not a bug in this crate.
//! * A fixture failure here on a case the sibling crate *passes*
//!   would surface a discrepancy in what "reference" means between
//!   the sibling's fixture record and tokenizers-rs — worth
//!   investigating.
//! * A fixture pass here on a case the sibling crate *fails* is the
//!   expected posture for every case that exercises a normalizer /
//!   pre-tokenizer / post-processor / decoder branch the sibling
//!   crate has not yet implemented (see the "Known runtime gaps"
//!   section of `docs/design/tokenizer-conformance.md`).
//!
//! # Fixture reuse (no duplication of the JSON blobs)
//!
//! Fixture JSON files live in the sibling crate's tests directory and
//! are read *directly* from there via a relative path anchored at
//! this crate's `CARGO_MANIFEST_DIR`. The fixture-format loader below
//! duplicates the sibling crate's `load_fixture` /
//! `find_tokenizer_json` helpers rather than pulling them in as a
//! shared workspace helper — those helpers live inside an integration
//! test binary in the sibling crate, so are not part of a public
//! module the adapter can `use`, and lifting them into a new helper
//! crate would be more invasive than the single-file duplication.
//! Keeping the loader identical to the sibling's copy is the load-
//! bearing property; the meta-test [`fixtures_in_sync_with_sibling`]
//! asserts the fixture-file list matches the sibling's registered
//! set exactly, so a fixture landing in the sibling crate becomes a
//! visible failure here until the runner is updated.
//!
//! # Feature gate
//!
//! Identical to the sibling: per-fixture `#[test]` fns are
//! `#[cfg_attr(not(feature = "parity-real-vocab"), ignore = ...)]` so
//! the corpus is visible to `cargo test --list` on the default
//! feature set but skipped at runtime, and `--features
//! parity-real-vocab` turns them on. The soft-skip when a
//! `tokenizer.json` is not on disk keeps the CI signal green on a
//! naked checkout.
//!
//! # Vocab lookup
//!
//! Same two-root convention as the sibling crate — with one lookup
//! order tweak: because this crate's `tests/conformance/vocabs/`
//! directory does not exist (we do not duplicate the sibling's
//! committed synthetic vocabs), the local fallback root points at
//! the *sibling* crate's `vocabs/` directory so the committed
//! synthetic vocabs (`bpe-byte-fallback-synth`,
//! `unigram-byte-fallback-synth`) are picked up from a single source
//! of truth.
//!
//! 1. `$STRINGCHEESE_REAL_VOCABS_DIR/<checkpoint>/tokenizer.json` —
//!    honoured when set. Recommended for CI: a setup step
//!    materialises vocabs into a per-job cache directory.
//! 2. `../stringcheese-tokenizer-hf/tests/conformance/vocabs/<checkpoint>/tokenizer.json`
//!    (relative to `CARGO_MANIFEST_DIR`). Recommended for local
//!    contributor use — the sibling crate's tree is the single source
//!    of truth for committed synthetic vocabs.
//!
//! When neither root resolves the runner *soft-skips* the case
//! (prints a `SKIP conformance_<name>: ...` line via `eprintln!`,
//! visible under `cargo test -- --nocapture`, and returns). A
//! malformed or unsupported `tokenizer.json` still panics.

// The conformance runner is filesystem-heavy (loads fixture JSON +
// real tokenizer.json vocabs at test time) and targets host-only
// tokenizers-rs — which is itself target-gated `cfg(not(wasm))` in
// this crate's Cargo.toml. Skip on wasm targets to match.
#![cfg(not(target_family = "wasm"))]

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_hf_native::HfNativeTokenizer;

/// The env-var callers set to point at a checkpoint cache directory
/// outside the workspace. Shared with the sibling crate and the
/// tiktoken-parity harness so a single flag activates them all.
const VOCAB_DIR_ENV: &str = "STRINGCHEESE_REAL_VOCABS_DIR";

/// The relative path from this crate's `CARGO_MANIFEST_DIR` to the
/// sibling `stringcheese-tokenizer-hf` crate's `tests/conformance`
/// directory. Both fixture JSON blobs *and* the local-fallback
/// `vocabs/` root live under this prefix; this crate does not
/// duplicate either.
const SIBLING_CONFORMANCE_REL: &str = "../stringcheese-tokenizer-hf/tests/conformance";

// ---------------------------------------------------------------------
// Fixture types. Mirrors the sibling crate's shape verbatim so a diff
// between the two runners is small and legible.
// ---------------------------------------------------------------------

/// One `(input, expected_ids, expected_decoded, note)` tuple.
///
/// `expected_decoded` is optional — fixtures written before the
/// decoder-chain landing omit it; when present the runner asserts
/// `tok.decode(expected_ids) == expected_decoded` as a second
/// per-case oracle.
struct Case {
    input: String,
    expected: Vec<u32>,
    expected_decoded: Option<String>,
    note: String,
}

/// A whole `<checkpoint>.json` fixture: metadata plus every case.
struct Fixture {
    checkpoint: String,
    source: String,
    #[allow(dead_code, reason = "surfaced in assertion messages when a case fails")]
    doc_note: String,
    cases: Vec<Case>,
}

/// Absolute path to a fixture JSON file. Anchored at the sibling
/// crate's `tests/conformance/` directory.
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(SIBLING_CONFORMANCE_REL)
        .join(name)
}

/// Parse one fixture file into a [`Fixture`]. Panics on any
/// structural error — the fixtures are checked in and their shape is
/// under contributor control, so a bad blob is a bug the CI signal
/// should surface loudly rather than skip.
fn load_fixture(name: &str) -> Fixture {
    let path = fixture_path(name);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let v: Value =
        serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));

    let checkpoint = v
        .get("checkpoint")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{name}: missing string field `checkpoint`"))
        .to_owned();
    let source = v
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{name}: missing string field `source`"))
        .to_owned();
    let doc_note = v
        .get("note")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let cases_json = v
        .get("cases")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{name}: missing array field `cases`"));

    let mut cases = Vec::with_capacity(cases_json.len());
    for (i, c) in cases_json.iter().enumerate() {
        let input = c
            .get("input")
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{name}[{i}]: missing string `input`"))
            .to_owned();
        let expected_arr = c
            .get("expected_ids")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{name}[{i}]: missing array `expected_ids`"));
        let mut expected = Vec::with_capacity(expected_arr.len());
        for (j, id) in expected_arr.iter().enumerate() {
            let n = id
                .as_u64()
                .unwrap_or_else(|| panic!("{name}[{i}].expected_ids[{j}]: not a u64"));
            expected.push(u32::try_from(n).unwrap_or_else(|_| {
                panic!("{name}[{i}].expected_ids[{j}] = {n} does not fit in u32")
            }));
        }
        let note = c
            .get("note")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let expected_decoded = c
            .get("expected_decoded")
            .and_then(Value::as_str)
            .map(str::to_owned);
        cases.push(Case {
            input,
            expected,
            expected_decoded,
            note,
        });
    }

    Fixture {
        checkpoint,
        source,
        doc_note,
        cases,
    }
}

// ---------------------------------------------------------------------
// Vocab lookup.
// ---------------------------------------------------------------------

/// Resolve the path to `<checkpoint>/tokenizer.json` using the two-root
/// priority documented at the top of this file. Returns `None` when
/// neither root resolves; the caller reports the two paths tried in
/// the skip message.
fn find_tokenizer_json(checkpoint: &str) -> Option<PathBuf> {
    if let Some(root) = env::var_os(VOCAB_DIR_ENV) {
        let p = PathBuf::from(root).join(checkpoint).join("tokenizer.json");
        if p.is_file() {
            return Some(p);
        }
    }
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(SIBLING_CONFORMANCE_REL)
        .join("vocabs")
        .join(checkpoint)
        .join("tokenizer.json");
    if p.is_file() {
        return Some(p);
    }
    None
}

/// Build a runnable [`HfNativeTokenizer`] from the checkpoint's real
/// `tokenizer.json`, or `None` if neither lookup root resolves. The
/// caller soft-skips on `None`; a malformed or unsupported
/// `tokenizer.json` still panics.
fn load_real_tokenizer(checkpoint: &str) -> Option<HfNativeTokenizer> {
    let path = find_tokenizer_json(checkpoint)?;
    let tok = HfNativeTokenizer::from_file(&path).unwrap_or_else(|e| {
        panic!(
            "materialise {} into HfNativeTokenizer: {e:?}",
            path.display()
        )
    });
    Some(tok)
}

// ---------------------------------------------------------------------
// Case execution.
// ---------------------------------------------------------------------

/// Run every case in `fixture` against `tok`. Collects all mismatches
/// into a single summarising panic so contributors see the whole
/// picture rather than the first failure alone. When a case carries
/// an `expected_decoded` field the runner also asserts the produced
/// decode matches — matching the sibling crate's runner shape.
fn run_cases(fixture: &Fixture, tok: &HfNativeTokenizer) {
    let mut failures = Vec::new();
    for (i, case) in fixture.cases.iter().enumerate() {
        let actual = Tokenizer::encode(tok, &case.input)
            .unwrap_or_else(|e| panic!("encode failed on case[{i}] ({}): {e:?}", case.note))
            .ids;
        if actual != case.expected {
            failures.push(format!(
                "  case[{i}] ({}): input={:?}\n    expected={:?}\n      actual={:?}",
                case.note, case.input, case.expected, actual,
            ));
        }
        if let Some(expected_decoded) = &case.expected_decoded {
            let actual_decoded = Tokenizer::decode(tok, &case.expected)
                .unwrap_or_else(|e| panic!("decode failed on case[{i}] ({}): {e:?}", case.note));
            if &actual_decoded != expected_decoded {
                failures.push(format!(
                    "  case[{i}] ({}) decode mismatch: input={:?}\n    expected_decoded={:?}\n      actual_decoded={:?}",
                    case.note, case.input, expected_decoded, actual_decoded,
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "conformance failures for `{}` (source: {}):\n{}",
        fixture.checkpoint,
        fixture.source,
        failures.join("\n"),
    );
}

/// Shared entry point for every per-checkpoint `#[test]`. Enforces the
/// invariant that a fixture file is scoped to a single checkpoint,
/// same as the sibling crate.
fn run_fixture(file: &str, expected_checkpoint: &str) {
    let fixture = load_fixture(file);
    assert_eq!(
        fixture.checkpoint, expected_checkpoint,
        "fixture `{file}`'s `checkpoint` field ({}) does not match \
         the test's declared checkpoint (`{expected_checkpoint}`)",
        fixture.checkpoint,
    );
    let Some(tok) = load_real_tokenizer(&fixture.checkpoint) else {
        eprintln!(
            "SKIP conformance_{}: no `tokenizer.json` on disk. \
             Provide one at ${VOCAB_DIR_ENV}/{ckpt}/tokenizer.json or \
             ../stringcheese-tokenizer-hf/tests/conformance/vocabs/{ckpt}/tokenizer.json \
             to activate.",
            fixture.checkpoint.replace('-', "_"),
            ckpt = fixture.checkpoint,
        );
        return;
    };
    run_cases(&fixture, &tok);
}

// ---------------------------------------------------------------------
// Meta-test: every fixture the sibling crate registers has a matching
// `#[test]` in this file. The sibling's registered list is the single
// source of truth; if a fixture lands there and not here, this fails.
// Runs on the default feature set (no `#[ignore]`) because it does
// not touch a real vocab.
// ---------------------------------------------------------------------

/// The fixture filenames this file has a matching `#[test]` for.
/// Must equal the sibling crate's `REGISTERED_FIXTURES` list; the
/// meta-test [`fixtures_in_sync_with_sibling`] enforces the equality.
/// When a new fixture is added to the sibling crate, add it here
/// *and* add a `#[test]` below.
const REGISTERED_FIXTURES: &[&str] = &[
    "gpt2.json",
    "cl100k_base.json",
    "bert_base_uncased.json",
    "xlm_roberta_base.json",
    "distilbert_base_uncased.json",
    "roberta_base.json",
    "bert_base_multilingual_cased.json",
    "bart_base.json",
    "deberta_v3_base.json",
    "mdeberta_v3_base.json",
    "unigram_byte_fallback_synth.json",
    "bpe_byte_fallback_synth.json",
    "llama_2_7b.json",
    "mistral_7b_v01.json",
    "qwen2_7b.json",
    "phi_3_mini_4k_instruct.json",
    "gemma_2b.json",
    "t5_base.json",
    "phi_2.json",
    "gemma_7b.json",
    "falcon_7b.json",
];

#[test]
fn fixtures_in_sync_with_sibling() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SIBLING_CONFORMANCE_REL);
    let registered: BTreeSet<&&str> = REGISTERED_FIXTURES.iter().collect();
    let mut on_disk = BTreeSet::new();
    for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display())) {
        let entry = entry.expect("dir entry");
        let name = entry.file_name();
        let name = name.to_string_lossy().into_owned();
        // The `vocabs/` subdirectory is the local-fallback root for
        // real `tokenizer.json` files; skip it.
        if name == "vocabs" {
            continue;
        }
        if std::path::Path::new(&name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            on_disk.insert(name);
        }
    }
    let missing: Vec<&String> = on_disk
        .iter()
        .filter(|n| !registered.contains(&n.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "fixtures present in sibling crate's tests/conformance/ but not \
         registered in REGISTERED_FIXTURES / a `#[test]` fn here: {missing:?}",
    );
    for reg in REGISTERED_FIXTURES {
        assert!(
            on_disk.contains(*reg),
            "REGISTERED_FIXTURES lists `{reg}` but the file is not in the sibling crate",
        );
    }
}

// ---------------------------------------------------------------------
// One `#[test]` per fixture. Ignored by default; runs under
// `--features parity-real-vocab`. The `#[ignore]` message text is
// duplicated verbatim across every fn — grepping for the phrase
// surfaces the whole set at once.
// ---------------------------------------------------------------------

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_gpt2() {
    run_fixture("gpt2.json", "gpt2");
}

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_cl100k_base() {
    run_fixture("cl100k_base.json", "cl100k_base");
}

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_bert_base_uncased() {
    run_fixture("bert_base_uncased.json", "bert-base-uncased");
}

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_xlm_roberta_base() {
    run_fixture("xlm_roberta_base.json", "xlm-roberta-base");
}

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_distilbert_base_uncased() {
    run_fixture("distilbert_base_uncased.json", "distilbert-base-uncased");
}

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_roberta_base() {
    run_fixture("roberta_base.json", "roberta-base");
}

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_bert_base_multilingual_cased() {
    run_fixture(
        "bert_base_multilingual_cased.json",
        "bert-base-multilingual-cased",
    );
}

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_bart_base() {
    run_fixture("bart_base.json", "bart-base");
}

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_deberta_v3_base() {
    run_fixture("deberta_v3_base.json", "deberta-v3-base");
}

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_mdeberta_v3_base() {
    run_fixture("mdeberta_v3_base.json", "mdeberta-v3-base");
}

// Synthetic Unigram + SentencePiece byte-fallback fixture. Vocab is
// committed under the sibling crate's `tests/conformance/vocabs/`;
// exercises the `<0xXX>` byte-fallback path added for Llama /
// Mistral / Qwen checkpoint support.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_unigram_byte_fallback_synth() {
    run_fixture(
        "unigram_byte_fallback_synth.json",
        "unigram-byte-fallback-synth",
    );
}

// Synthetic character-BPE + SentencePiece byte-fallback fixture. Vocab
// is committed under the sibling crate's `tests/conformance/vocabs/`;
// exercises the BPE-side `<0xXX>` byte-fallback path (real Llama-2 /
// Mistral / Qwen tokenizer.json blobs ship as `model.type == "BPE"`
// with the same 256 reserved tokens embedded).
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_bpe_byte_fallback_synth() {
    run_fixture("bpe_byte_fallback_synth.json", "bpe-byte-fallback-synth");
}

// Real Llama-2 tokenizer.json — requires a real 1.8 MB `tokenizer.json`
// on disk (we never commit real vocab bytes); the runner soft-skips
// when none is present. Reference ids come from
// `transformers.AutoTokenizer.from_pretrained('NousResearch/Llama-2-7b-hf')`.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_llama_2_7b() {
    run_fixture("llama_2_7b.json", "llama-2-7b-hf");
}

// Real Mistral-7B-v0.1 tokenizer.json — same Llama-family
// character-BPE + SentencePiece byte_fallback shape over a distinct
// 32k vocabulary. Reference ids come from
// `transformers.AutoTokenizer.from_pretrained('mistralai/Mistral-7B-v0.1')`.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_mistral_7b_v01() {
    run_fixture("mistral_7b_v01.json", "mistral-7b-v0.1");
}

// Real Qwen2-7B tokenizer.json — byte-level BPE (GPT-2-family) with an
// NFC normalizer and a Sequence[Split(Regex), ByteLevel] pre-tokenizer
// over a 151k vocabulary. Reference ids come from
// `transformers.AutoTokenizer.from_pretrained('Qwen/Qwen2-7B')`.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_qwen2_7b() {
    run_fixture("qwen2_7b.json", "qwen2-7b");
}

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_phi_3_mini_4k_instruct() {
    run_fixture("phi_3_mini_4k_instruct.json", "phi-3-mini-4k-instruct");
}

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_gemma_2b() {
    run_fixture("gemma_2b.json", "gemma-2b");
}

#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_t5_base() {
    run_fixture("t5_base.json", "t5-base");
}

// Real microsoft/phi-2 tokenizer.json — GPT-2-family byte-level BPE
// with no normalizer, a ByteLevel pre-tokenizer/post-processor/decoder
// chain, and no SentencePiece byte_fallback.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_phi_2() {
    run_fixture("phi_2.json", "phi-2");
}

// Real google/gemma-7b tokenizer.json — same SentencePiece-BPE shape
// as gemma-2b (byte-identical vocab / added_tokens / normalizer /
// pre_tokenizer / post_processor / decoder). Local vocab is fetched
// via the ungated `unsloth/gemma-7b` mirror.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_gemma_7b() {
    run_fixture("gemma_7b.json", "gemma-7b");
}

// Real tiiuae/falcon-7b tokenizer.json — byte-level BPE with a
// Sequence pre-tokenizer combining Punctuation(Contiguous),
// ByteLevel, Digits and Split(Regex="[0-9][0-9][0-9]"). No normalizer,
// no post-processor (raw ids), ByteLevel decoder.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_falcon_7b() {
    run_fixture("falcon_7b.json", "falcon-7b");
}
