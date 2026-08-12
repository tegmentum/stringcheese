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
| `llama-2-7b-hf`                  | `llama_2_7b.json`                   | 20    | Character-BPE + SentencePiece byte_fallback      | `transformers.AutoTokenizer.from_pretrained('NousResearch/Llama-2-7b-hf')` (transformers 5.14.1) |
| `mistral-7b-v0.1`                | `mistral_7b_v01.json`               | 20    | Character-BPE + SentencePiece byte_fallback + Metaspace pre-tokenizer | `transformers.AutoTokenizer.from_pretrained('mistralai/Mistral-7B-v0.1')` (transformers 5.14.1 / tokenizers 0.22.2) |
| `qwen2-7b`                       | `qwen2_7b.json`                     | 20    | Byte-level BPE (GPT-2 family) + NFC normalizer + Sequence[Split(Regex), ByteLevel] pre-tokenizer | `transformers.AutoTokenizer.from_pretrained('Qwen/Qwen2-7B')` (transformers 5.14.1 / tokenizers 0.22.2) |
| `phi-3-mini-4k-instruct`         | `phi_3_mini_4k_instruct.json`       | 20    | Character-BPE + SentencePiece byte_fallback + Sequence[Prepend(▁), Replace(' '→'▁')] normalizer, no explicit pre-tokenizer | `transformers.AutoTokenizer.from_pretrained('microsoft/Phi-3-mini-4k-instruct')` (transformers 5.14.1 / tokenizers 0.22.2) |
| `gemma-2b`                       | `gemma_2b.json`                     | 20    | SentencePiece-BPE + byte_fallback + Replace(' '→'▁') normalizer, 256k vocabulary with 217 added chat-format tokens | `transformers.AutoTokenizer.from_pretrained('unsloth/gemma-2b')` — an ungated mirror of `google/gemma-2b`'s tokenizer.json (transformers 5.14.1 / tokenizers 0.22.2) |
| `t5-base`                        | `t5_base.json`                      | 20    | SentencePiece Unigram + Precompiled charsmap normalizer + Sequence[WhitespaceSplit, Metaspace] pre-tokenizer + TemplateProcessing(</s>) post-processor | `transformers.AutoTokenizer.from_pretrained('google-t5/t5-base')` (transformers 5.14.1 / tokenizers 0.22.2) |
| `phi-2`                          | `phi_2.json`                        | 20    | GPT-2-family byte-level BPE (no byte_fallback), null normalizer + ByteLevel pre-tokenizer/post-processor/decoder, 50257-entry vocabulary with 39 whitespace-run added tokens (ids 50257–50294) | `transformers.AutoTokenizer.from_pretrained('microsoft/phi-2')` (transformers 5.14.1 / tokenizers 0.22.2) |
| `gemma-7b`                       | `gemma_7b.json`                     | 20    | SentencePiece-BPE + byte_fallback + Replace(' '→'▁') normalizer, 256k vocabulary with 217 added chat-format tokens — **byte-identical tokenizer semantics to `gemma-2b`** (verified via top-level field comparison); the upstream `tokenizer.json` blobs differ in file layout only | `transformers.AutoTokenizer.from_pretrained('unsloth/gemma-7b')` — an ungated mirror of `google/gemma-7b`'s tokenizer.json (transformers 5.14.1 / tokenizers 0.22.2) |
| `falcon-7b`                      | `falcon_7b.json`                    | 20    | Byte-level BPE, null normalizer + Sequence[Punctuation(Contiguous), ByteLevel, Digits, Split(Regex="[0-9][0-9][0-9]")] pre-tokenizer, null post-processor, ByteLevel decoder, 65024-entry vocabulary with 12 special tokens (`>>TITLE<<`, `>>ABSTRACT<<`, …, `<|endoftext|>`) | `transformers.AutoTokenizer.from_pretrained('tiiuae/falcon-7b')` (transformers 5.14.1 / tokenizers 0.22.2) |

Total: **19 checkpoints × (40 or 20) cases = 580 triples**, all
reference-computed against upstream implementations (no
hand-computation).

Between them the checkpoints exercise every runtime shape
`stringcheese-tokenizer-hf` accepts:

- Byte-level BPE — `gpt2`, `roberta-base`, `bart-base`, `qwen2-7b`,
  `phi-2`, `falcon-7b` (six distinct vocabularies over the same BPE
  runtime; `qwen2-7b` adds a `Sequence[Split(Regex), ByteLevel]`
  pre-tokenizer shape not covered by the older three; `phi-2` adds a
  block of 38 whitespace-run added tokens (`normalized: true`,
  non-special) that upstream matches via added-vocab before
  pre-tokenization runs; `falcon-7b` adds a `Sequence[Punctuation,
  ByteLevel, Digits, Split(Regex)]` pre-tokenizer combining four
  distinct pre-tokenizer combinators no other fixture exercises).
- tiktoken BPE — `cl100k_base` (leans on the tiktoken parity harness
  for the actual vocab; the runner soft-skips when no `tokenizer.json`
  is provisioned locally for it, since tiktoken has its own format).
- WordPiece + BertNormalizer — `bert-base-uncased`,
  `distilbert-base-uncased`, `bert-base-multilingual-cased` (uncased
  ASCII, DistilBERT sharing the uncased vocab under a distinct config,
  and cased multilingual).
- SentencePiece Unigram — `xlm-roberta-base`, `deberta-v3-base`,
  `mdeberta-v3-base`, `t5-base` (four normalizer / pre-tokenizer
  sequences over the same Unigram Viterbi runtime; `t5-base` is the
  first fixture that pairs a `Precompiled` normalizer with a
  `Sequence[WhitespaceSplit, Metaspace]` pre-tokenizer and a
  `TemplateProcessing` post-processor that appends `</s>`).
- Character-BPE + SentencePiece `byte_fallback` — `llama-2-7b-hf`,
  `mistral-7b-v0.1`, `phi-3-mini-4k-instruct`, `gemma-2b`, `gemma-7b`
  (Llama-family BPE with the `<0xXX>` byte-fallback path; Mistral
  additionally exercises the `Metaspace` pre-tokenizer variant of the
  same semantics — see the runtime gap below; `phi-3-mini-4k-instruct`
  uses the Llama-2 `Sequence[Prepend, Replace]` normalizer shape over
  a distinct 32k vocabulary with Phi-3 chat-format specials;
  `gemma-2b` ships a much larger 256k vocabulary, a bare `Replace`
  normalizer with no `Prepend`, and 217 chat-format added tokens;
  `gemma-7b` reuses `gemma-2b`'s tokenizer semantics *verbatim* — the
  upstream `tokenizer.json` blobs differ in file layout only, so the
  fixture serves as a distinct real-vocab checkpoint whose parity
  covaries with gemma-2b by construction).

`microsoft/deberta-v3-base` and `microsoft/mdeberta-v3-base` do not
publish a `tokenizer.json` under
`https://huggingface.co/<repo>/tokenizer.json` — the SentencePiece
`spiece.model` is the on-hub source of truth. The corresponding
local vocab files are generated by loading the tokenizer via
`transformers.AutoTokenizer(...).save_pretrained(...)`, which converts
the SentencePiece model on the fly. The scratchpad helper script
records the recipe.

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
- **`Metaspace` pre-tokenizer on the BPE side (blocks Mistral).**
  `mistralai/Mistral-7B-v0.1` ships its SentencePiece prepend + " "→"▁"
  substitution as a `pre_tokenizer.type == "Metaspace"`
  (`replacement="▁"`, `prepend_scheme="first"`, `split=false`) with a
  `null` normalizer, whereas `NousResearch/Llama-2-7b-hf` encodes the
  same semantics as a `normalizer.type == "Sequence"` of
  `Prepend("▁")` + `Replace(" " → "▁")` with a `null` pre-tokenizer.
  The current BPE loader accepts the Llama-2 shape but rejects the
  Mistral shape with
  `UnsupportedPreTokenizer { type_name: "Metaspace", reason: "SentencePiece Metaspace is not part of the BPE pipeline; ..." }`,
  so `conformance_mistral_7b_v01` currently fails at
  `to_tokenizer(&config)` and none of the 20 cases run. The fix is a
  BPE-side branch that accepts `Metaspace` with `split=false` and
  desugars it into the equivalent Llama-2-style normalizer prefix. All
  20 fixture ids are already recorded so the next-agent landing can
  flip the test from panic-on-load to 20/20 with no fixture churn.
- **Strict byte-fallback coverage rejects Gemma-2b at load.**
  `google/gemma-2b` ships `model.byte_fallback: true` alongside 255 of
  the 256 `<0xXX>` byte-fallback tokens: `<0x09>` is omitted because
  the tab character has its own dedicated literal token (id 226).
  The current BPE loader treats missing byte-fallback entries as
  fatal and rejects with
  `ByteFallbackTokensMissing { missing_count: 1, first_missing_byte: 9 }`,
  so `conformance_gemma_2b` fails at `to_tokenizer(&config)` and none
  of the 20 cases run. The fix is to relax the check to accept a
  subset of the 256-byte range when the omitted bytes have alternative
  literal tokens in the vocabulary; the byte-fallback path already
  falls back to the literal token when one is present, so the loader
  gate is the only blocker. All 20 fixture ids are recorded so the
  next-agent landing can flip the test from panic-on-load to (up to)
  20/20 with no fixture churn.
- **`Punctuation` pre-tokenizer unsupported (blocks Falcon-7b).**
  `tiiuae/falcon-7b` ships a `pre_tokenizer.type == "Sequence"` whose
  four children are `Punctuation(behavior=Contiguous)`, `ByteLevel`,
  `Digits(individual_digits=false)`, and
  `Split(pattern=Regex("[0-9][0-9][0-9]"))`. The BPE loader rejects
  the first child with
  `UnsupportedPreTokenizer { type_name: "Punctuation", reason: "deferred to a later landing" }`,
  so `conformance_falcon_7b` currently fails at `to_tokenizer(&config)`
  and none of the 20 cases run. All 20 fixture ids are recorded so
  the next-agent landing can flip the test from panic-on-load to
  (up to) 20/20 with no fixture churn. Adding the `Punctuation` and
  `Digits` combinators to the pre-tokenizer runtime unblocks Falcon-7b
  and any other Falcon-family checkpoint that shares the shape.
- **Whitespace-run added tokens skipped by pre-tokenization (2/20
  cases on Phi-2).** `microsoft/phi-2` ships 38
  added-vocabulary entries (`id 50257..=50294`) whose content is a run
  of literal ASCII space characters of decreasing length
  (`"                               "` down to `"  "`), each marked
  `normalized: true` and `special: false`. Upstream
  `transformers.AutoTokenizer` matches these against the *raw* input
  before ByteLevel pre-tokenization runs, collapsing a run of N spaces
  into a single added-token id when a matching length exists (so
  `\n    return` in Python code becomes `[…, 198, 50284, 7783, …]`
  where `50284` is the "4 spaces" added token). The BPE runtime today
  matches only `normalized: false` added tokens against the raw input;
  `normalized: true` non-special added tokens are ignored, so both the
  `python-code` and `whitespace-heavy` cases on Phi-2 currently fall
  through to per-space ByteLevel encoding (`220` × N) and mismatch.
  Adds pressure on the added-vocabulary matcher to route
  `normalized: true` entries through the same raw-input scan.
- **Phi-3-mini's four Llama-family gaps** are all fixed as of the
  latest landing — `conformance_phi_3_mini_4k_instruct` runs 20/20
  cases to parity against the hand-crafted vocab. Two commits closed
  the four fixture cases:
  - `python-code`: `BpeTokenizer::encode_region_bpe_inner` now
    short-circuits `pre_tokenize` when byte-fallback is enabled and no
    pre-tokenizer pattern is configured, passing the whole region to
    the merge loop as a single word. The previous whitespace-split
    fallback silently dropped `\n` (and every other non-space
    whitespace) before byte-fallback could route those bytes to the
    reserved `<0xXX>` tokens. Also relevant to the Llama-2, Mistral,
    and Gemma checkpoints, whose fixtures happen not to contain a
    newline case but would surface the same bug.
  - `empty`, `bos-eos-surface-form-raw`, `chat-end-surface-form-raw`:
    `BpeTokenizer::encode_pieces_with_policy` now extracts registered
    special-token surfaces from the RAW input first, then applies the
    configured normalizer to each between-specials region
    independently. Mirrors HF's `added_vocabulary::extract_and_normalize`
    ordering that the WordPiece and Unigram runtimes already ship.
    Consequences: empty raw input yields no regions to normalize (so
    the `Prepend` marker is not emitted), and specials embedded in the
    input are matched before the normalizer prepends `▁` in front of
    them (so `<s>hi</s>` encodes to `[1, 7251, 2]` and `<|end|>` to
    `[32007]`, matching upstream). Unit tests
    `phi3_shape_*` in `bpe.rs` lock the four fixture behaviours in
    place against a hand-crafted Phi-3-shape vocab so the parity
    coverage does not depend on the real `tokenizer.json` on disk.
    **Verification against the real vocab (with the parity fix from
    the byte-fallback landing) surfaces one residual gap (1/20 cases):
    `<s>hi</s>` currently encodes to `[1, 7251, 829, 29879, 29958]`
    against the real vocab, expected `[1, 7251, 2]`. The Phi-3 real
    `tokenizer.json` records `</s>` as an added-vocab entry with
    `special: false, rstrip: true` (only `<s>` is `special: true`),
    and the raw-input scan the hand-crafted-vocab tests exercise
    matches on `special: true` and does not currently recognise the
    non-special/`rstrip`-flagged `</s>` entry as an added-vocab hit.
    Same class of gap as the Phi-2 whitespace-run finding above — the
    added-vocabulary matcher needs to cover `special: false` /
    `normalized: true` / `rstrip` / `lstrip` variants that upstream
    treats uniformly.**

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
