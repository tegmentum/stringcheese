# BPE Conformance Corpus

Small, hand-curated corpus of `(checkpoint, input, expected_ids)`
triples for `stringcheese-tokenizer-bpe`. Sits alongside the crate's
unit tests as a *cheap* regression backstop **before** the full
1000-per-model parity harness lands — see
[`tokenizers.md` §11](./tokenizers.md#11-phased-implementation-plan)
Phase 5–6.

The audit motivating this corpus phrased the ask as:

> Add a small conformance corpus (checkpoint-under-test → hand-computed
> expected output for ~20 diverse inputs each) even before the full
> 1000-per-model harness. Catches regressions cheaply.

## Where the code lives

| Path                                                                                  | Role                                          |
| ------------------------------------------------------------------------------------- | --------------------------------------------- |
| `crates/stringcheese-tokenizer-bpe/tests/conformance/*.json`                          | One fixture file per checkpoint (see below).  |
| `crates/stringcheese-tokenizer-bpe/tests/conformance/vocabs/<checkpoint>/tokenizer.json` | Optional local-lookup root for real vocabs.   |
| `crates/stringcheese-tokenizer-bpe/tests/conformance.rs`                              | Runner: loads fixtures, dispatches, asserts.  |

## Feature gate

The runner is compiled unconditionally under the `hf-tokenizer`
feature (which the workspace ships on by default for `--all-features`
runs) but every per-fixture `#[test]` fn is `#[ignore]`d unless the
`parity-real-vocab` cargo feature is on:

```
cargo test -p stringcheese-tokenizer-bpe --features parity-real-vocab
```

The `#[ignore]` path keeps the corpus visible to `cargo test --list`
so contributors see the coverage inventory even without provisioning
real vocabs; the meta-test `every_fixture_on_disk_has_a_test` runs on
the default feature set and fails loudly if a fixture is added
without a matching `#[test]` line.

The feature name `parity-real-vocab` is deliberately shared with the
parallel tiktoken-parity work. A single flag activates every
vocab-backed test across the crates.

## Vocab lookup

Real upstream `tokenizer.json` files are tens of MB and out of scope
for in-tree commits (licence + repo bloat). When the
`parity-real-vocab` feature is on, the runner resolves each
checkpoint's `tokenizer.json` from the first of these two roots:

1. `${STRINGCHEESE_REAL_VOCABS_DIR}/<checkpoint>/tokenizer.json` — set
   the env-var in CI to point at a per-job cache directory. This is
   how the parallel fetch mechanism (which materialises the real
   `mergeable_ranks` blobs and downloads the HF `tokenizer.json`
   blobs) will hand vocabs to the runner.
2. `crates/stringcheese-tokenizer-bpe/tests/conformance/vocabs/<checkpoint>/tokenizer.json`
   — drop the file here for local runs.

When neither location resolves the runner *soft-skips* the case
(prints a `SKIP conformance_<name>: ...` line, visible under
`cargo test -- --nocapture`, and returns without failing). This is
what keeps `cargo test --workspace --all-features --locked` — the
default CI signal — green on a naked checkout. A malformed or
unsupported `tokenizer.json` still panics — those are real regressions
the suite must surface.

## Fixtures shipped today (Wave-14)

| Checkpoint                | File                          | Cases | Model family      | Source of ids                                                        |
| ------------------------- | ----------------------------- | ----- | ----------------- | -------------------------------------------------------------------- |
| `gpt2`                    | `gpt2.json`                   | 20    | Byte-level BPE    | `transformers.AutoTokenizer.from_pretrained('gpt2')` (transformers 5.14.1) |
| `cl100k_base`             | `cl100k_base.json`            | 20    | tiktoken BPE      | `tiktoken.get_encoding('cl100k_base').encode_ordinary` (tiktoken 0.13.0) |
| `bert-base-uncased`       | `bert_base_uncased.json`      | 20    | WordPiece + BertNormalizer | `transformers.AutoTokenizer.from_pretrained('bert-base-uncased')` (transformers 5.14.1) |
| `xlm-roberta-base`        | `xlm_roberta_base.json`       | 20    | SentencePiece Unigram + Precompiled + Metaspace | `transformers.AutoTokenizer.from_pretrained('xlm-roberta-base')` (transformers 5.14.1) |

Total: **4 checkpoints × 20 cases = 80 triples**, all
reference-computed against upstream implementations (no
hand-computation).

The four families between them exercise the loader's four "shape"
axes: byte-level BPE (gpt2), tiktoken BPE (cl100k_base), WordPiece
with a Bert-style normalizer / post-processor (bert-base-uncased),
and Unigram over SentencePiece with a Precompiled charsmap + Metaspace
pre-tokenizer + BOS/EOS splice (xlm-roberta-base).

Llama-2 and Mistral-7B were considered but skipped in this first
landing: both are HF-gated and force every consumer of the corpus
through an access-token workflow, which the audit's cheap-first
framing argues against. `xlm-roberta-base` covers the same
SentencePiece-Unigram slice without the licence hurdle. When the
parity harness lands and a gated-vocab flow becomes routine, add
Llama-2 via the recipe below.

## The 20 inputs

Every fixture uses the same 20 inputs, each named by its `note` field.
Chosen to span the audit's suggested categories with no overlap:

1. `empty` — `""`
2. `single-char-ascii` — `"a"`
3. `single-space` — `" "`
4. `english-greeting` — `"Hello, world!"`
5. `english-prose-pangram`
6. `python-code` — with `\n` and 4-space indent
7. `rust-code` — with `println!` macro
8. `japanese` — CJK punctuation and full-width chars
9. `arabic` — RTL script
10. `mixed-scripts` — Latin + CJK + Arabic in one line
11. `emoji` — multi-code-point sequences
12. `many-newlines` — bare `\n\n\n` between letters
13. `whitespace-heavy` — leading + interior runs
14. `long-repeated-pangram` — the pangram × 3 (probes cache/reuse)
15. `special-surface-endoftext-raw` — `"<|endoftext|>"` as raw text
16. `nfc-cafe-precomposed` — `"café"` in NFC
17. `nfd-cafe-decomposed` — `"café"` in NFD (`e` + combining acute)
18. `tab-and-newline` — tab + newline between letters
19. `numbers-and-signs` — `"3.14159 and -42"`
20. `url-with-query`

## Fixture file format

```json
{
  "checkpoint": "gpt2",
  "source":     "how the reference ids were computed",
  "note":       "free-form: gotchas / policy choices for this fixture",
  "cases": [
    { "input": "Hello, world!", "expected_ids": [15496, 11, 995, 0], "note": "english-greeting" }
  ]
}
```

Fields:

- `checkpoint` — must equal the argument the corresponding `#[test]`
  line passes to `run_fixture`. Enforced.
- `source` — free text; identify the reference tool *and its version*.
- `note` — free text on the fixture as a whole (special-token policy,
  add-special-tokens setting, etc.).
- `cases[i].input` — arbitrary UTF-8; JSON string escapes decode.
- `cases[i].expected_ids` — array of non-negative integers ≤ `u32::MAX`.
- `cases[i].note` — free-form; usually the category label.

## Adding a new checkpoint

1. **Author the fixture.** Pick a checkpoint, write a script (see
   `scratchpad/gen_fixtures.py` or the equivalent for your reference
   tool) that runs the same 20 inputs through the upstream tool and
   emits the JSON in the format above. Name the file
   `<canonical-name>.json` — lowercase, `_` for `-`, no `.tokenizer`
   suffix (e.g. `qwen2_5_7b.json`, not `qwen2.5-7B.tokenizer.json`).
   Save to `crates/stringcheese-tokenizer-bpe/tests/conformance/`.
2. **Register the fixture.** In `tests/conformance.rs`:
   - Add the file to `REGISTERED_FIXTURES`.
   - Add a `#[test]` fn following the pattern of the existing four,
     carrying the same
     `#[cfg_attr(not(feature = "parity-real-vocab"), ignore = ...)]`
     line.
3. **Verify locally.** `cargo test -p stringcheese-tokenizer-bpe
   --test conformance` should show the new test as `ignored` on the
   default feature set. With `--features parity-real-vocab` and a
   `tokenizer.json` provisioned at either lookup root, the test runs
   and must pass — mismatches are either fixture bugs (re-run the
   reference tool, verify its output) or crate bugs (a real regression
   the corpus caught).
4. **Update this doc.** Add a row to the "Fixtures shipped today"
   table.

## Hand-computed fallback

If a checkpoint's reference tool is not installable in your
environment, hand-computed ids are still valuable as long as they are
clearly labelled. Set the fixture's top-level `source` field to
something like:

```json
"source": "hand-computed against openai/gpt-2 (byte-level BPE); no reference tool run"
```

Every case whose expected ids are hand-computed should also carry a
brief `note` explaining the reasoning. The runner does not care where
the ids came from — the assertion is the same — but a hand-computed
fixture is a *promise* to future contributors that the ids match the
algorithm, and the `source` field is the load-bearing record of that
promise.

## Relationship to the full parity harness

This corpus is not a replacement for the ~1000-per-model parity
harness the Phase 5 / Phase 6 gates specify (see
[`tokenizers.md` §11](./tokenizers.md#11-phased-implementation-plan)).
It is the *bootstrap* signal: 80 triples are enough to catch every
"the loader stopped materialising post-processors" or "the merge table
started dropping the last byte pair" regression, without needing the
larger harness's cross-crate fetch machinery to exist first. The full
harness will subsume this file when it lands.
