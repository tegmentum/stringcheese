# BPE Conformance Corpus

Small, hand-curated corpus of `(checkpoint, input, expected_ids)`
triples for `stringcheese-tokenizer-hf`. Sits alongside the crate's
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
| `crates/stringcheese-tokenizer-hf/tests/conformance/*.json`                          | One fixture file per checkpoint (see below).  |
| `crates/stringcheese-tokenizer-hf/tests/conformance/vocabs/<checkpoint>/tokenizer.json` | Optional local-lookup root for real vocabs.   |
| `crates/stringcheese-tokenizer-hf/tests/conformance.rs`                              | Runner: loads fixtures, dispatches, asserts.  |

## Feature gate

The runner is compiled unconditionally under the `hf-tokenizer`
feature (which the workspace ships on by default for `--all-features`
runs) but every per-fixture `#[test]` fn is `#[ignore]`d unless the
`parity-real-vocab` cargo feature is on:

```
cargo test -p stringcheese-tokenizer-hf --features parity-real-vocab
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
2. `crates/stringcheese-tokenizer-hf/tests/conformance/vocabs/<checkpoint>/tokenizer.json`
   — drop the file here for local runs.

When neither location resolves the runner *soft-skips* the case
(prints a `SKIP conformance_<name>: ...` line, visible under
`cargo test -- --nocapture`, and returns without failing). This is
what keeps `cargo test --workspace --all-features --locked` — the
default CI signal — green on a naked checkout. A malformed or
unsupported `tokenizer.json` still panics — those are real regressions
the suite must surface.

## Fixtures shipped today

| Checkpoint                       | File                                | Cases | Model family                                     | Source of ids                                                                                             |
| -------------------------------- | ----------------------------------- | ----- | ------------------------------------------------ | --------------------------------------------------------------------------------------------------------- |
| `gpt2`                           | `gpt2.json`                         | 40    | Byte-level BPE                                   | `transformers.AutoTokenizer.from_pretrained('gpt2')` (transformers 5.14.1)                                |
| `cl100k_base`                    | `cl100k_base.json`                  | 40    | tiktoken BPE                                     | `tiktoken.get_encoding('cl100k_base').encode_ordinary` (tiktoken 0.13.0)                                  |
| `bert-base-uncased`              | `bert_base_uncased.json`            | 40    | WordPiece + BertNormalizer                       | `transformers.AutoTokenizer.from_pretrained('bert-base-uncased')` (transformers 5.14.1)                   |
| `xlm-roberta-base`               | `xlm_roberta_base.json`             | 40    | SentencePiece Unigram + Precompiled + Metaspace  | `transformers.AutoTokenizer.from_pretrained('xlm-roberta-base')` (transformers 5.14.1)                    |
| `distilbert-base-uncased`        | `distilbert_base_uncased.json`      | 40    | WordPiece + BertNormalizer (shared bert vocab)   | `transformers.AutoTokenizer.from_pretrained('distilbert-base-uncased')` (transformers 5.14.1)             |
| `roberta-base`                   | `roberta_base.json`                 | 40    | Byte-level BPE + RobertaProcessing               | `transformers.AutoTokenizer.from_pretrained('roberta-base')` (transformers 5.14.1)                        |
| `bert-base-multilingual-cased`   | `bert_base_multilingual_cased.json` | 40    | WordPiece + BertNormalizer (cased, multilingual) | `transformers.AutoTokenizer.from_pretrained('google-bert/bert-base-multilingual-cased')` (transformers 5.14.1) |
| `bart-base`                      | `bart_base.json`                    | 40    | Byte-level BPE + RobertaProcessing               | `transformers.AutoTokenizer.from_pretrained('facebook/bart-base')` (transformers 5.14.1)                  |
| `deberta-v3-base`                | `deberta_v3_base.json`              | 40    | SentencePiece Unigram + Sequence normalizer      | `transformers.AutoTokenizer.from_pretrained('microsoft/deberta-v3-base')` (transformers 5.14.1)           |
| `mdeberta-v3-base`               | `mdeberta_v3_base.json`             | 40    | SentencePiece Unigram + Sequence normalizer (multilingual) | `transformers.AutoTokenizer.from_pretrained('microsoft/mdeberta-v3-base')` (transformers 5.14.1) |

Total: **10 checkpoints × 40 cases = 400 triples**, all
reference-computed against upstream implementations (no
hand-computation).

Between them the ten checkpoints exercise every runtime shape
`stringcheese-tokenizer-hf` accepts:

- Byte-level BPE — `gpt2`, `roberta-base`, `bart-base` (three distinct
  vocabularies over the same BPE runtime).
- tiktoken BPE — `cl100k_base` (leans on the tiktoken parity harness
  for the actual vocab; the runner soft-skips when no `tokenizer.json`
  is provisioned locally for it, since tiktoken has its own format).
- WordPiece + BertNormalizer — `bert-base-uncased`,
  `distilbert-base-uncased`, `bert-base-multilingual-cased` (uncased
  ASCII, DistilBERT sharing the uncased vocab under a distinct config,
  and cased multilingual).
- SentencePiece Unigram — `xlm-roberta-base`, `deberta-v3-base`,
  `mdeberta-v3-base` (three normalizer / pre-tokenizer sequences over
  the same Unigram Viterbi runtime).

`microsoft/deberta-v3-base` and `microsoft/mdeberta-v3-base` do not
publish a `tokenizer.json` under
`https://huggingface.co/<repo>/tokenizer.json` — the SentencePiece
`spiece.model` is the on-hub source of truth. The corresponding
local vocab files are generated by loading the tokenizer via
`transformers.AutoTokenizer(...).save_pretrained(...)`, which converts
the SentencePiece model on the fly. The scratchpad helper script
records the recipe.

Llama-2 and Mistral-7B remain out of scope until the `byte_fallback`
runtime slice lands: both families depend on that path for CJK and
emoji coverage, and adding fixtures before it lands would tie the
corpus to a runtime gap. Add them as follow-ups once the runtime
work is in.

## The 40 inputs

Every fixture uses the same 40 inputs, each named by its `note` field.
Cases 1–20 are the original Wave-14 corpus; cases 21–40 are the
Wave-15 widening (long inputs, adversarial normalisation, RTL, more
scripts, special-token surface forms):

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
21. `long-prose-paragraph` — >1024 chars of Pride and Prejudice
22. `long-code-paragraph` — >1024 chars of Python (merge-sort + BST)
23. `json-blob-escaped` — JSON with `\"`, `é`, nested arrays
24. `long-url` — >200 char URL with many query params
25. `mixed-nfc-nfd` — same visible glyphs in NFC and NFD interleaved
26. `korean-hangul` — precomposed Jamo syllables
27. `hebrew` — RTL Hebrew
28. `russian-cyrillic` — Cyrillic
29. `hindi-devanagari` — Devanagari with combining marks
30. `long-emoji-zwj-run` — ZWJ sequences + skin-tone modifiers + flags
31. `invisible-chars` — ZWSP + ZWJ + ZWNJ + soft hyphen
32. `rtl-with-latin` — Arabic embedded in an English sentence
33. `cls-surface-form-raw` — literal `[CLS]`/`[SEP]` in input
34. `mask-surface-form-raw` — literal `[MASK]` in input
35. `bos-eos-surface-form-raw` — literal `<s>...</s>` in input
36. `multilingual-single-input` — Latin + CJK + Arabic + Cyrillic + Devanagari
37. `mixed-case-english` — CamelCase / snake_case
38. `aaaa-run` — 128× `"a"` (probes cache / reuse)
39. `numbers-cluster` — `"1234567890 0.0001 1e10 -3.14 +42"`
40. `punctuation-heavy` — quote/parenthesis-heavy prose

The Wave-15 widening was chosen against the audit's brief to add cases
for very long inputs, adversarial normalisation, long emoji runs,
special-token surface forms, and multi-lingual mixes within a single
input. Cases 33–35 in particular are designed to catch regressions in
the tokenizer's handling of registered special tokens appearing as
plain text.

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
   tool) that runs the same 40 inputs through the upstream tool and
   emits the JSON in the format above. Name the file
   `<canonical-name>.json` — lowercase, `_` for `-`, no `.tokenizer`
   suffix (e.g. `qwen2_5_7b.json`, not `qwen2.5-7B.tokenizer.json`).
   Save to `crates/stringcheese-tokenizer-hf/tests/conformance/`.
2. **Register the fixture.** In `tests/conformance.rs`:
   - Add the file to `REGISTERED_FIXTURES`.
   - Add a `#[test]` fn following the pattern of the existing four,
     carrying the same
     `#[cfg_attr(not(feature = "parity-real-vocab"), ignore = ...)]`
     line.
3. **Verify locally.** `cargo test -p stringcheese-tokenizer-hf
   --test conformance` should show the new test as `ignored` on the
   default feature set. With `--features parity-real-vocab` and a
   `tokenizer.json` provisioned at either lookup root, the test runs
   and must pass — mismatches are either fixture bugs (re-run the
   reference tool, verify its output) or crate bugs (a real regression
   the corpus caught).
4. **Update this doc.** Add a row to the "Fixtures shipped today"
   table.

## Known runtime gaps surfaced by the Wave-15 widening

Widening the corpus from 4 × 20 to 10 × 40 immediately surfaced several
runtime gaps that are legitimate follow-ups (the corpus doing its job).
Each is filed here so a follow-up can pick them up without re-deriving:

- **`Replace(Regex)` normalizer unsupported.** The `deberta-v3-base`
  and `mdeberta-v3-base` `tokenizer.json` blobs (produced by
  `save_pretrained` because Microsoft does not publish a
  `tokenizer.json` on the hub) start with a `Normalizer::Sequence`
  whose first entry is `Replace` with a regex pattern. The loader
  rejects with `UnsupportedNormalizer { type_name: "Replace(Regex)" }`
  and both fixtures fail to load at all.
- **Registered `[CLS]` / `[SEP]` / `[MASK]` surface forms.** Cases
  `cls-surface-form-raw`, `mask-surface-form-raw`, and
  `bos-eos-surface-form-raw` place literal special-token strings in
  the input. Every WordPiece and Unigram checkpoint's reference
  tokenizer treats them as the registered special ids (`101`, `102`,
  `103`, `0`, `2` for the BERT/XLM-R families) rather than as raw
  text; the runtime splits them piece-by-piece.
- **`BertNormalizer` strips no zero-width chars.** Case
  `invisible-chars` embeds U+200B (ZWSP), U+200C (ZWNJ), U+200D (ZWJ),
  and U+00AD (soft hyphen). The reference `bert-base-uncased` /
  `distilbert-base-uncased` / `bert-base-multilingual-cased`
  tokenizers strip these before WordPiece; the runtime lets them
  through, producing UNK tokens.
- **`bert-base-uncased` / `distilbert-base-uncased` Devanagari
  coverage.** Cases `hindi-devanagari` and `multilingual-single-input`
  hit real WordPiece entries in the reference (`1327`, `29867`,
  `29874`, `29859`, ...) but the runtime emits `100` (UNK). Likely a
  BertNormalizer accent-stripping interaction with combining marks
  in NFD-decomposed Devanagari.
- **`Sequence(WhitespaceSplit + Metaspace)` pre-tokenizer.** The raw
  `xlm-roberta-base` `tokenizer.json` wraps `Metaspace` in a
  two-child `Sequence` alongside `WhitespaceSplit`. The loader
  rejects with `AmbiguousSequencePreTokenizer { child_count: 2 }`.
  The local vocab is patched to drop the `WhitespaceSplit` so the
  tokenizer loads at all — see `scratchpad/patch_vocabs.py` — but
  this changes the semantics on multi-whitespace inputs, so the
  `single-space`, `python-code`, `many-newlines`, `whitespace-heavy`,
  and `long-code-paragraph` cases now surface the runtime gap where
  each whitespace character becomes its own `▁` token instead of
  being collapsed.

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
It is the *bootstrap* signal: 400 triples are enough to catch every
"the loader stopped materialising post-processors" or "the merge table
started dropping the last byte pair" regression, without needing the
larger harness's cross-crate fetch machinery to exist first. The full
harness will subsume this file when it lands.
