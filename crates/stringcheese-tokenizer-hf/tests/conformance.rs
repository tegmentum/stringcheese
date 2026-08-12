//! Small conformance corpus for `stringcheese-tokenizer-hf`.
//!
//! Each fixture under `tests/conformance/` names a real published
//! checkpoint and pairs 20 diverse inputs with the reference token-id
//! output computed by the checkpoint's *upstream* reference tool
//! (`tiktoken` for the `OpenAI` variant; `transformers.AutoTokenizer`
//! for every Hugging Face-shape checkpoint). The runner in this file
//! loads each fixture, materialises the corresponding
//! [`stringcheese_tokenizer_hf`] tokenizer from a real
//! `tokenizer.json` on disk, and asserts each case's ids match.
//!
//! # Feature gate
//!
//! The runner is only *active* when the `parity-real-vocab` cargo
//! feature is enabled — every fixture test carries an
//! `#[cfg_attr(not(feature = "parity-real-vocab"), ignore = ...)]` so
//! `cargo test -p stringcheese-tokenizer-hf` on the default feature
//! set lists the tests (guarding against fixture-format rot) but
//! skips their bodies. Enabling the feature turns them on and the
//! runner then requires the checkpoint's real `tokenizer.json` to be
//! materialised on disk.
//!
//! # Vocab lookup
//!
//! Real upstream vocabularies (tens of MB) are not committed to this
//! repository. When the `parity-real-vocab` feature is on the runner
//! looks for a checkpoint's real HF-shape `tokenizer.json` under two
//! roots, in this priority order:
//!
//! 1. `$STRINGCHEESE_REAL_VOCABS_DIR/<checkpoint>/tokenizer.json` —
//!    honoured when the env-var is set. Recommended for CI, where a
//!    setup step materialises vocabs into a per-job cache directory.
//! 2. `<crate>/tests/conformance/vocabs/<checkpoint>/tokenizer.json`
//!    (relative to `CARGO_MANIFEST_DIR`). Recommended for local
//!    contributor use — drop the file in place, run
//!    `cargo test --features parity-real-vocab`.
//!
//! When neither location resolves the runner *soft-skips* the case
//! (prints a `SKIP conformance_<name>: ...` line via `eprintln!`,
//! visible under `cargo test -- --nocapture`, and returns without
//! failing). This is what keeps `cargo test --workspace --all-features
//! --locked` — the default CI signal — green on a naked checkout;
//! `parity-real-vocab` *enables* the runner but does not itself
//! materialise the vocabs. A malformed or unsupported `tokenizer.json`
//! still panics — those are real regressions the suite must surface.
//! The parallel tiktoken-parity work provisions these files as part
//! of its fetch/build step; both suites share the `parity-real-vocab`
//! feature name so a single flag activates both.
//!
//! # Fixture format
//!
//! ```json
//! {
//!   "checkpoint": "<name>",
//!   "source":     "<how the reference ids were computed>",
//!   "note":       "<free text — what to know about this fixture>",
//!   "cases": [
//!     { "input": "...", "expected_ids": [...], "note": "..." }
//!   ]
//! }
//! ```
//!
//! `expected_ids` is a JSON array of non-negative integers; nothing
//! else is inspected.

// The conformance runner exists as a stand-alone integration test file
// so the fixture harness compiles as part of every host `cargo test`
// invocation — the `#[ignore]` gate is *runtime*, not `cfg`-based, so
// the tests remain visible to `cargo test --list` even without the
// feature. The runner does still need `hf-tokenizer` for the
// [`hf::to_tokenizer`] dispatch, so we `cfg`-gate to that superset —
// callers on a bare `default` build get an empty test binary rather
// than a compile error over missing modules.
#![cfg(feature = "hf-tokenizer")]
// Conformance corpus is filesystem-heavy (loads fixture JSON + real
// tokenizer.json vocabs at test time) and targets Python-oracle-derived
// reference outputs on host toolchains. Skip on wasm targets — the
// wasm-runtime CI job runs under wasmtime's sandboxed WASI filesystem
// and every fixture case's soft-skip branch still exercises the
// `std::fs::read` failure path in a way wasmtime treats as abort.
#![cfg(not(target_family = "wasm"))]

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_hf::hf::{HfTokenizer, parse_tokenizer_json, to_tokenizer};

/// The env-var callers set to point at a checkpoint cache directory
/// outside the workspace. See the module doc for the two-root lookup.
const VOCAB_DIR_ENV: &str = "STRINGCHEESE_REAL_VOCABS_DIR";

// Runner-visible message when the runner is `#[ignore]`d in the
// default configuration. `#[ignore = ...]` demands a string literal
// (`syn::MetaNameValue` rejects an ident here), so the message is
// duplicated on each `#[test]` below rather than named as a `const`.
// Keep the phrasing identical across the four uses: grepping for a
// hit surfaces every gated test at once.
//
// Canonical phrase, copy verbatim:
//   "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"

// ---------------------------------------------------------------------
// Fixture types.
// ---------------------------------------------------------------------

/// One `(input, expected_ids, expected_decoded, note)` tuple.
///
/// `expected_decoded` is optional so fixtures written before the
/// decoder-chain landing keep loading unchanged; when present, the
/// runner asserts `tok.decode(expected_ids) == expected_decoded` as a
/// second per-case oracle.
struct Case {
    input: String,
    expected: Vec<u32>,
    expected_decoded: Option<String>,
    note: String,
}

/// A whole `tests/conformance/<name>.json` fixture: metadata plus
/// every case.
struct Fixture {
    checkpoint: String,
    source: String,
    #[allow(dead_code, reason = "surfaced in assertion messages when a case fails")]
    doc_note: String,
    cases: Vec<Case>,
}

/// Parse one fixture file into a [`Fixture`]. Panics on any structural
/// error — fixtures are checked in and their shape is under
/// contributor control, so a bad blob is a bug the CI signal should
/// surface loudly rather than skip.
fn load_fixture(name: &str) -> Fixture {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("conformance")
        .join(name);
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
        // Optional — fixtures written before the decoder-chain landing
        // omit the field. When present the runner asserts
        // `tok.decode(expected_ids) == expected_decoded`.
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

/// Resolve the path to `<checkpoint>/tokenizer.json` using the
/// two-root priority documented at the top of this file. Returns
/// `None` when neither root resolves; the caller reports the two
/// paths that were tried.
fn find_tokenizer_json(checkpoint: &str) -> Option<PathBuf> {
    if let Some(root) = env::var_os(VOCAB_DIR_ENV) {
        let p = PathBuf::from(root).join(checkpoint).join("tokenizer.json");
        if p.is_file() {
            return Some(p);
        }
    }
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("conformance")
        .join("vocabs")
        .join(checkpoint)
        .join("tokenizer.json");
    if p.is_file() {
        return Some(p);
    }
    None
}

/// Build a runnable [`HfTokenizer`] from the checkpoint's real
/// `tokenizer.json`, or `None` if neither lookup root resolves.
///
/// The runner *soft-skips* on `None` rather than panicking so
/// `cargo test --workspace --all-features` in CI stays green on a
/// naked checkout — the `parity-real-vocab` feature only *enables* the
/// runner, it does not itself materialise the vocabs. Contributors
/// who run the parity suite provide the vocabs via one of the two
/// lookup roots documented at the top of this file; when both are
/// empty the test emits a skip message via `eprintln!` (visible under
/// `cargo test -- --nocapture`) and returns.
///
/// A malformed `tokenizer.json` or a conversion error *does* still
/// panic — a corrupted or unsupported vocab is a real signal the
/// suite must surface, not paper over.
fn load_real_tokenizer(checkpoint: &str) -> Option<HfTokenizer> {
    let path = find_tokenizer_json(checkpoint)?;
    let json = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let config =
        parse_tokenizer_json(&json).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    let tok = to_tokenizer(&config)
        .unwrap_or_else(|e| panic!("materialise {} into HfTokenizer: {e:?}", path.display()));
    Some(tok)
}

// ---------------------------------------------------------------------
// Case execution.
// ---------------------------------------------------------------------

/// Decode `ids` under `tok`, returning the produced string regardless
/// of the underlying tokenizer variant. Mirrors [`encode_ids`] — the
/// runner exercises the trait's `decode` path so the full decoder
/// chain (when a fixture carries an `expected_decoded` field) is
/// applied.
fn decode_ids(tok: &HfTokenizer, ids: &[u32]) -> String {
    match tok {
        HfTokenizer::Bpe(bpe) => Tokenizer::decode(bpe.as_ref(), ids).expect("BPE decode"),
        HfTokenizer::WordPiece(wp) => Tokenizer::decode(wp, ids).expect("WordPiece decode"),
        HfTokenizer::Unigram(uni) => Tokenizer::decode(uni, ids).expect("Unigram decode"),
        other => panic!("conformance runner: unrecognised HfTokenizer variant {other:?}"),
    }
}

/// Encode `input` under `tok`, returning the produced ids as `Vec<u32>`
/// regardless of the underlying tokenizer variant. Only the token ids
/// are compared — offsets and the special-mask are not exercised here
/// because the reference tools do not surface them uniformly (the
/// full ~1000-per-model parity harness will).
fn encode_ids(tok: &HfTokenizer, input: &str) -> Vec<u32> {
    match tok {
        // BPE and WordPiece both implement the [`Tokenizer`] trait,
        // whose `encode` returns an `Encoding<TokenId>`. Route both
        // through the trait so the runner exercises the full
        // normalize → pre-tokenize → model → post-process pipeline
        // (WordPiece's inherent `encode` returns `Vec<TokenId>` and
        // *skips* the post-processor, which we want to test here).
        HfTokenizer::Bpe(bpe) => {
            let enc = Tokenizer::encode(bpe.as_ref(), input).expect("BPE encode");
            enc.ids
        }
        HfTokenizer::WordPiece(wp) => {
            let enc = Tokenizer::encode(wp, input).expect("WordPiece encode");
            enc.ids
        }
        HfTokenizer::Unigram(uni) => {
            // Route Unigram through the `Tokenizer` trait so the
            // runner exercises the full normalize → pre-tokenize →
            // Viterbi → post-process pipeline. The inherent
            // `UnigramTokenizer::encode` returns raw `Vec<usize>` and
            // *skips* the post-processor, which we want firing here
            // (RobertaProcessing wraps the primary ids with CLS/SEP).
            let enc = Tokenizer::encode(uni, input).expect("Unigram encode");
            enc.ids
        }
        // `HfTokenizer` is `#[non_exhaustive]`; keep the runner
        // compiling if a new variant lands upstream by rejecting it
        // with a clear diagnostic rather than silently accepting.
        other => panic!("conformance runner: unrecognised HfTokenizer variant {other:?}"),
    }
}

/// Run every case in `fixture` against `tok`, formatting a single
/// summarising error if any case fails so contributors see the whole
/// picture rather than the first mismatch alone. When a case carries
/// an `expected_decoded` field the runner also asserts the produced
/// decode matches — the decoder chain landing added this second
/// oracle so Llama-2 and its byte-for-byte
/// `transformers.AutoTokenizer.decode` parity are exercised end to
/// end.
fn run_cases(fixture: &Fixture, tok: &HfTokenizer) {
    let mut failures = Vec::new();
    for (i, case) in fixture.cases.iter().enumerate() {
        let actual = encode_ids(tok, &case.input);
        if actual != case.expected {
            failures.push(format!(
                "  case[{i}] ({}): input={:?}\n    expected={:?}\n      actual={:?}",
                case.note, case.input, case.expected, actual,
            ));
        }
        if let Some(expected_decoded) = &case.expected_decoded {
            let actual_decoded = decode_ids(tok, &case.expected);
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

/// Shared entry point for every per-checkpoint `#[test]`. Enforces
/// the invariant that a fixture file is scoped to a single
/// checkpoint (the JSON's `checkpoint` field must match `name`),
/// which is what makes each `#[test]` line a stable reference to one
/// upstream vocabulary.
fn run_fixture(file: &str, expected_checkpoint: &str) {
    let fixture = load_fixture(file);
    assert_eq!(
        fixture.checkpoint, expected_checkpoint,
        "fixture `{file}`'s `checkpoint` field ({}) does not match \
         the test's declared checkpoint (`{expected_checkpoint}`)",
        fixture.checkpoint,
    );
    let Some(tok) = load_real_tokenizer(&fixture.checkpoint) else {
        // Soft skip — see [`load_real_tokenizer`] for the rationale.
        eprintln!(
            "SKIP conformance_{}: no `tokenizer.json` on disk. \
             Provide one at ${VOCAB_DIR_ENV}/{ckpt}/tokenizer.json or \
             <crate>/tests/conformance/vocabs/{ckpt}/tokenizer.json to activate.",
            fixture.checkpoint.replace('-', "_"),
            ckpt = fixture.checkpoint,
        );
        return;
    };
    run_cases(&fixture, &tok);
}

// ---------------------------------------------------------------------
// Meta-test: every fixture file listed on disk is exercised by some
// `#[test]` in this file. Guards against a contributor adding a JSON
// blob and forgetting to add the test line; runs on the default
// feature set too (no `#[ignore]`) because it does not touch a real
// vocab.
// ---------------------------------------------------------------------

/// Every fixture filename this file has a matching `#[test]` for. When
/// you add a new fixture, add it here *and* add a `#[test]` below.
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
fn every_fixture_on_disk_has_a_test() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("conformance");
    let registered: BTreeSet<&&str> = REGISTERED_FIXTURES.iter().collect();
    let mut on_disk = BTreeSet::new();
    for entry in fs::read_dir(&dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display())) {
        let entry = entry.expect("dir entry");
        let name = entry.file_name();
        let name = name.to_string_lossy().into_owned();
        // The `vocabs/` subdirectory is the local lookup root for real
        // `tokenizer.json` files (see `find_tokenizer_json`); skip it.
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
        "fixtures present on disk but not registered in \
         REGISTERED_FIXTURES / a `#[test]` fn: {missing:?}",
    );
    for reg in REGISTERED_FIXTURES {
        assert!(
            on_disk.contains(*reg),
            "REGISTERED_FIXTURES lists `{reg}` but the file is not on disk",
        );
    }
}

// ---------------------------------------------------------------------
// One `#[test]` per fixture. Ignored by default; runs under
// `--features parity-real-vocab`.
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

// The synthetic `unigram-byte-fallback-synth` fixture ships its own
// tokenizer.json under `tests/conformance/vocabs/` (it is a hand-crafted
// Unigram vocab, not a real upstream one — see the fixture's `source`
// field for the rationale). It exercises the SentencePiece `<0xXX>`
// byte-fallback path added to the Unigram runtime for Llama / Mistral /
// Qwen checkpoint support. The `#[ignore]` gate still applies for
// consistency with the rest of the conformance runner — activating the
// suite requires `--features parity-real-vocab`.
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

// The synthetic `bpe-byte-fallback-synth` fixture ships its own
// tokenizer.json under `tests/conformance/vocabs/` (it is a hand-
// crafted character-BPE vocab, not a real upstream one — see the
// fixture's `source` field for the rationale). It exercises the
// SentencePiece `<0xXX>` byte-fallback path added to the BPE runtime
// for Llama-2 / Mistral / Qwen checkpoint support (real Llama-2 /
// Mistral / Qwen tokenizer.json blobs ship as `model.type == "BPE"`
// with the same 256 reserved tokens embedded). The `#[ignore]` gate
// still applies for consistency with the rest of the conformance
// runner — activating the suite requires `--features
// parity-real-vocab`.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_bpe_byte_fallback_synth() {
    run_fixture("bpe_byte_fallback_synth.json", "bpe-byte-fallback-synth");
}

// Real Llama-2 tokenizer.json — the primary target for the BPE-side
// byte-fallback landing. Unlike the two synthetic fixtures above this
// one requires a real 1.8 MB `tokenizer.json` on disk (we never
// commit real vocab bytes); the runner soft-skips when none is
// present. Reference ids come from
// `transformers.AutoTokenizer.from_pretrained('NousResearch/Llama-2-7b-hf')`
// so byte-for-byte parity here means our loader matches upstream on
// the primary shipped Llama-2 BPE contract, byte-fallback included.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_llama_2_7b() {
    run_fixture("llama_2_7b.json", "llama-2-7b-hf");
}

// Real Mistral-7B-v0.1 tokenizer.json — a second real target for the
// BPE-side byte-fallback landing. Mistral ships the same Llama-family
// character-BPE + SentencePiece byte_fallback shape as Llama-2, over a
// distinct 32k vocabulary, so parity here confirms our BPE runtime
// handles the byte-fallback path independent of vocab. Reference ids
// come from `transformers.AutoTokenizer.from_pretrained(
// 'mistralai/Mistral-7B-v0.1')`. The runner soft-skips when no real
// `tokenizer.json` is provisioned.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_mistral_7b_v01() {
    run_fixture("mistral_7b_v01.json", "mistral-7b-v0.1");
}

// Real Qwen2-7B tokenizer.json — a byte-level BPE (GPT-2-family)
// checkpoint with an NFC normalizer and a Sequence[Split(Regex),
// ByteLevel] pre-tokenizer over a 151k vocabulary. Distinct model
// family from Llama/Mistral (no SentencePiece byte_fallback; the
// GPT-2 byte↔char mapping is what covers non-vocab bytes), so parity
// here exercises the byte-level BPE side of the loader against a
// modern LLM checkpoint. Reference ids come from
// `transformers.AutoTokenizer.from_pretrained('Qwen/Qwen2-7B')`.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_qwen2_7b() {
    run_fixture("qwen2_7b.json", "qwen2-7b");
}

// Real microsoft/Phi-3-mini-4k-instruct tokenizer.json — a Llama-family
// character-BPE with SentencePiece byte_fallback and a
// `Sequence[Prepend("▁"), Replace(" " → "▁")]` normalizer (no explicit
// pre-tokenizer). Same runtime shape as Llama-2, exercised over a
// distinct 32k vocabulary with 14 added tokens covering Phi-3's
// chat-format specials (<|end|>, <|user|>, <|assistant|>). Reference
// ids come from `transformers.AutoTokenizer.from_pretrained(
// 'microsoft/Phi-3-mini-4k-instruct')`.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_phi_3_mini_4k_instruct() {
    run_fixture("phi_3_mini_4k_instruct.json", "phi-3-mini-4k-instruct");
}

// Real google/gemma-2b tokenizer.json — a SentencePiece-BPE with
// byte_fallback and a bare `Replace(" " → "▁")` normalizer (no
// `Prepend`, no pre-tokenizer) over a 256k vocabulary that includes
// 217 dedicated added-tokens for Gemma's chat format
// (<start_of_turn>, <end_of_turn>, ...). Distinct shape from the
// Llama-family fixtures: no Prepend and a much larger vocabulary.
// The local vocab is fetched via the ungated `unsloth/gemma-2b` mirror
// because `google/gemma-2b` requires access approval. Reference ids
// come from `transformers.AutoTokenizer.from_pretrained(
// 'unsloth/gemma-2b')` against the same tokenizer.json.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_gemma_2b() {
    run_fixture("gemma_2b.json", "gemma-2b");
}

// Real google-t5/t5-base tokenizer.json — a SentencePiece Unigram
// tokenizer with a `Precompiled` charsmap normalizer, a
// `Sequence[WhitespaceSplit, Metaspace]` pre-tokenizer, and a
// `TemplateProcessing` post-processor that appends the </s> EOS
// (id 1). 32100-entry vocabulary including 100 <extra_id_N> sentinel
// tokens. First fixture exercising the T5-style Unigram+Metaspace
// combination. Reference ids come from
// `transformers.AutoTokenizer.from_pretrained('google-t5/t5-base')`.
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
// chain, and no SentencePiece byte_fallback (the byte-to-char mapping
// is what covers non-vocab bytes). Distinct from Phi-3-mini
// (Llama-family character-BPE + byte_fallback + Prepend normalizer);
// same model family (Phi) but a different tokenizer shape entirely.
// Reference ids come from `transformers.AutoTokenizer.from_pretrained(
// 'microsoft/phi-2')`.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_phi_2() {
    run_fixture("phi_2.json", "phi-2");
}

// Real google/gemma-7b tokenizer.json — same SentencePiece-BPE shape
// as gemma-2b: byte-identical vocabulary, added_tokens, normalizer,
// pre_tokenizer, post_processor and decoder (the upstream
// tokenizer.json blobs themselves differ in file layout only). A
// distinct fixture is kept so a regression against either the 2b or
// 7b real vocab surfaces as a separately-labelled failure line.
// Local vocab is fetched via the ungated `unsloth/gemma-7b` mirror.
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
// ByteLevel, Digits and Split(Regex="[0-9][0-9][0-9]"). No
// normalizer, no post-processor (raw ids), ByteLevel decoder. First
// fixture exercising the Sequence pre-tokenizer with these four
// combinators end to end. Reference ids come from
// `transformers.AutoTokenizer.from_pretrained('tiiuae/falcon-7b')`.
#[test]
#[cfg_attr(
    not(feature = "parity-real-vocab"),
    ignore = "requires the `parity-real-vocab` feature and a materialised tokenizer.json on disk"
)]
fn conformance_falcon_7b() {
    run_fixture("falcon_7b.json", "falcon-7b");
}
