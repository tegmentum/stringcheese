# StringCheese

[![CI](https://github.com/tegmentum/stringcheese/actions/workflows/ci.yml/badge.svg)](https://github.com/tegmentum/stringcheese/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/stringcheese.svg)](https://crates.io/crates/stringcheese)
[![docs.rs](https://docs.rs/stringcheese/badge.svg)](https://docs.rs/stringcheese)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Rigorous sequence comparison for Rust and WebAssembly.

StringCheese is a comprehensive, performance-oriented toolkit for string and
sequence comparison. It aims to be the canonical Rust library for distance,
similarity, alignment, phonetic matching, n-gram, fingerprinting, and
content-defined chunking algorithms — with correctness, memory transparency,
and multilingual support treated as first-class concerns rather than
after-the-fact bolt-ons.

## Status

Version 0.1 is under active development. Every subsystem in
`docs/design/scope-and-decomposition.md`'s implementation-staging
list ships as a dedicated crate today (see the workspace table
below), and each carries its own tests, benchmarks, and — where
relevant — an oracle-backed conformance corpus. Notable
subsystems shipping today:

- **Comparison kernels** (`stringcheese-compare`) — Levenshtein,
  Hamming, Jaro/Jaro-Winkler, Damerau/OSA, LCS, set similarity,
  substring search — with SIMD backends (AVX2 / SSE2 / NEON /
  wasm-simd128) for Levenshtein (Myers), Jaro (wide-block),
  Damerau (Hyyrö), and Hamming.
- **Alignment** (`stringcheese-align`) — Needleman-Wunsch,
  Smith-Waterman, linear + affine gaps.
- **Content-defined chunking** (`stringcheese-cdc`) — FastCDC,
  Rabin, Gear, Buzhash, polynomial rolling hashes, each with a
  vectorised SIMD backend.
- **HF-parity tokenizers** (`stringcheese-tokenizer-hf`) —
  BPE / WordPiece / Unigram / WordLevel across the normalizer,
  pre-tokenizer, post-processor, and decoder chain, byte-for-byte
  against upstream `transformers` on the shipped conformance
  corpus.
- **Tiered language detection** (`stringcheese-detect`,
  `-detect-script`, `-detect-whatlang`, `-detect-lingua`) — script
  classifier → per-script whatlang shard → per-language lingua
  shard, all speaking one WIT contract.
- **45+ language packs** — stopwords, stemmers, tokenizers, and
  phonetic hookups per language; see the workspace table below.
- **Phonetic keys** (`stringcheese-phonetic`) — Soundex, NYSIIS,
  Double Metaphone, Slavic-Metaphone, with language-pack hookups
  for locale-tuned encoders (Kölner, PHONEX-family per Romance /
  Slavic / Semitic / Turkic / East-Asian).
- **Fingerprints and sketches** — MinHash (one-permutation),
  SimHash (64/128-bit + weighted features + banded LSH),
  Winnowing.
- **Diff, pattern, segment, unicode, normalize, textsplit,
  collate, translit, escape, ident, stats, ngram, minhash,
  index** — see the workspace table for each.

Concrete algorithm coverage continues to fill in across minor
releases — the current release surface is best treated as
usable-but-evolving, and cross-crate integration tests catch API
shape issues as new pieces land.

See [`docs/DESIGN.md`](docs/DESIGN.md) for the full project
vision, [`docs/design/scope-and-decomposition.md`](docs/design/scope-and-decomposition.md)
for the subsystem decomposition and the wrap-vs-reimplement
policy that guides each crate, and
[`docs/design/tokenizer-conformance.md`](docs/design/tokenizer-conformance.md)
for the HF-parity conformance corpus.

## MSRV and edition

- **Rust edition:** 2024
- **MSRV:** 1.88
- CI matrix runs the pedantic clippy set as `-D warnings`, plus
  a `no_std` + `alloc`-only matrix and a `wasm32-unknown-unknown`
  + `wasm32-wasip1` matrix across every session crate.

## Design principles

- **Preserve semantics.** Distance is not similarity; a raw score is not a
  metric; a normalized value carries its normalization policy. StringCheese's
  types reflect these distinctions rather than erasing them for uniformity.
- **Performance is a feature.** Runtime, allocation count, peak memory,
  binary size, and WebAssembly footprint are measured and reported alongside
  correctness.
- **WebAssembly first.** Every design decision considers the browser, WASI,
  the Component Model, and embedded targets. The core crate is `no_std`
  compatible.
- **Correctness is demonstrated, not asserted.** The library ships alongside
  a versioned conformance corpus (`stringcheese-corpus`) that is intended to be
  usable by other sequence-comparison libraries.

## Workspace layout

The umbrella `stringcheese` crate re-exports the subsystem crates
grouped below. Language packs and data-heavy algorithm crates
(tokenizer model packs, per-tier language-detection shards, the
regex engine) stay opt-in — callers pull them by direct
dependency so a build that doesn't need them pays nothing.

**Foundations**

| Crate                    | Purpose                                                                        |
|--------------------------|--------------------------------------------------------------------------------|
| `stringcheese`           | Facade crate re-exporting the public API                                       |
| `stringcheese-core`      | Foundational traits, result types, algorithm descriptors, workspace / sequence abstractions |
| `stringcheese-corpus`    | Golden-case schema and validation corpus (separately versioned deliverable)    |
| `stringcheese-bench`     | Benchmark suite and comparative reporting                                      |

**Comparison, search, alignment**

| Crate                    | Purpose                                                                        |
|--------------------------|--------------------------------------------------------------------------------|
| `stringcheese-compare`   | Comparison kernels: Levenshtein, Hamming, Jaro/Jaro-Winkler, Damerau/OSA, LCS, set similarity (Dice/Jaccard/Overlap/Cosine), substring search (Rabin-Karp/KMP/Boyer-Moore/Aho-Corasick/Horspool/Two-way) |
| `stringcheese-align`     | Pairwise alignment: Needleman-Wunsch, Smith-Waterman, affine gaps              |
| `stringcheese-diff`      | Myers + Patience diff, unified-diff format, hunks, patch apply                 |
| `stringcheese-pattern`   | Pattern matching: `Literal`, `Wildcard`, `Glob` behind a shared `Pattern` trait with explicit `MatchUnit` |
| `stringcheese-pattern-regex` | Finite-automata regex engine plugging into the same `Pattern` trait (opt-in) |

**Segmentation, normalization, transliteration**

| Crate                    | Purpose                                                                        |
|--------------------------|--------------------------------------------------------------------------------|
| `stringcheese-segment`   | Bytes, code points, graphemes, words, sentences, lines with explicit `SegmentUnit` |
| `stringcheese-unicode`   | Unicode NFC/NFKC/NFD/NFKD, case folding, diacritic stripping, `PreprocessingPipeline` |
| `stringcheese-normalize` | Named pipeline presets (`identifier`, `display_safe`, `search_key`) + punctuation canonicalisation |
| `stringcheese-collate`   | UCA / natural-order / ASCII-CI collation behind a `Collator` trait             |
| `stringcheese-translit`  | `Transliterator` trait, `deunicode`-backed general path, table-based per-script romanisation |
| `stringcheese-phonetic`  | Soundex, NYSIIS, Double Metaphone, phonetic-matcher composer                   |

**Chunking, sketches, indexing**

| Crate                    | Purpose                                                                        |
|--------------------------|--------------------------------------------------------------------------------|
| `stringcheese-cdc`       | Byte-oriented content-defined chunking: FastCDC, Rabin CDC, Buzhash, Gear rolling-hash |
| `stringcheese-textsplit` | Text splitters for LLM RAG: `RecursiveSplitter`, `ParagraphSplitter`, `SentenceSplitter` |
| `stringcheese-ngram`     | Sliding-window char / byte / token / grapheme n-grams                          |
| `stringcheese-minhash`   | MinHash sketches, Jaccard estimation, LSH banding                              |
| `stringcheese-simhash`   | 64/128-bit SimHash fingerprints, weighted-feature support, permutation-band LSH |
| `stringcheese-winnowing` | Schleimer-Wilkerson-Aiken document fingerprints                                 |
| `stringcheese-index`     | Index structures: BK-tree, VP-tree, q-gram inverted index                       |

**Statistics, identifiers, escaping**

| Crate                    | Purpose                                                                        |
|--------------------------|--------------------------------------------------------------------------------|
| `stringcheese-stats`     | Shannon entropy, Unicode general-category histograms, ratios, lengths          |
| `stringcheese-ident`     | Case conversion, `Case::detect`, slugify, identifier sanitisation              |
| `stringcheese-escape`    | URI / HTML / JSON / POSIX-shell escape and unescape                             |
| `stringcheese-manip`     | Broad string-manipulation utilities (case, trim, find, replace, quote, template) |

**Tokenizers**

| Crate                                | Purpose                                                                        |
|--------------------------------------|--------------------------------------------------------------------------------|
| `stringcheese-tokenizer`             | `Tokenizer` / `Segmenter` / `Encoding` traits, built-in segmenters (whitespace, delimiter, identifier, grapheme, n-gram, byte, char), plus the shared `truncation` and `padding` modules |
| `stringcheese-tokenizer-hf`          | Full Hugging Face `tokenizer.json` loader — BPE / WordPiece / Unigram / WordLevel, HF's normalizer / pre-tokenizer / post-processor / decoder chain, `encode_batch`, `encode_pair`, byte-for-byte parity against upstream `transformers` on the shipped conformance corpus |
| `stringcheese-tokenizer-tiktoken`    | OpenAI tiktoken model pack — `cl100k_base` / `p50k_base` / `r50k_base` / `o200k_base` behind per-variant Cargo features, on top of `stringcheese-tokenizer-hf` |

**Language detection**

| Crate                              | Purpose                                                                          |
|------------------------------------|----------------------------------------------------------------------------------|
| `stringcheese-detect`              | Tier-walking dispatcher over the detection stack                                 |
| `stringcheese-detect-script`       | Tier 0: ~5 KB Unicode-block script classifier                                    |
| `stringcheese-detect-whatlang`     | Tier 1: per-script whatlang shard (WASM component)                               |
| `stringcheese-detect-lingua`       | Tier 2: per-language lingua shard (WASM component)                               |

**Language packs**

45+ per-language crates (`stringcheese-en`, `stringcheese-de`,
`stringcheese-ja`, `stringcheese-zh`, `stringcheese-ko`,
`stringcheese-ar`, `stringcheese-fa`, `stringcheese-he`,
`stringcheese-hi`, `stringcheese-ta`, `stringcheese-th`,
`stringcheese-ka`, `stringcheese-am`, …) ship stopwords,
stemmers, tokenisers, and phonetic hookups for each supported
language. Opt-in per pack — each is a separate crate so a build
that only needs one language does not pay for the others.

Packs self-register into a static `linkme`-backed registry
(`stringcheese_lang::registry`). Callers who pick a language at
runtime (user locale, config file, `Accept-Language` header)
reach for `registry::language(code)`; callers who name the pack
at compile time keep using the pack's `ENGLISH` / `GERMAN` /
`FRENCH` constant. Registry lookup walks the BCP-47 subtag-strip
fallback (`"pt-BR"` → `"pt"`, `"sr-Cyrl-RS"` → `"sr"`); a strict
`language_exact` variant is available where the fallback is
unwanted.

See `stringcheese-lang` and `stringcheese-lang-gen` for the
plugin traits and the build-time generator that emits per-pack
capability descriptors.

## Feature flags

The facade crate re-exports every subsystem behind feature flags
so a caller only compiles what they use. Commonly reached-for
flags:

- `compare` / `align` / `phonetic` / `unicode` / `cdc` / `index`
  — enable the corresponding subsystem's re-exports.
- `simd` — turn on the vectorised backends across `compare` and
  `cdc`. Off by default so a plain scalar build is deterministic.
- `parallel` — Rayon-backed batch APIs where the subsystem has
  them.
- `no_std` + `alloc` — the core crates compile without `std`;
  the wasm-runtime CI job exercises this.
- `wasm-runtime` — enable the wasm-compatible feature subset for
  `wasm32-unknown-unknown` / `wasm32-wasip1` targets.

Subsystem crates each carry their own opt-in features — for
example `stringcheese-tokenizer-tiktoken` gates each vocabulary
behind its own feature (`cl100k`, `o200k`, `p50k`, `r50k`) so a
caller only pays for the packs they need.

## Correctness

Every subsystem ships with its own oracle-backed conformance
suite:

- **Tokenizer HF-parity.** 13 shipped `tokenizer.json` fixtures
  (`gpt2`, `roberta-base`, `bart-base`, `qwen2-7b`,
  `bert-base-uncased`, `distilbert-base-uncased`,
  `bert-base-multilingual-cased`, `xlm-roberta-base`,
  `deberta-v3-base`, `mdeberta-v3-base`, `llama-2-7b-hf`,
  `mistral-7b-v0.1`, `cl100k_base`) exercise byte-level BPE,
  tiktoken BPE, WordPiece + `BertNormalizer`, SentencePiece
  Unigram, and character-BPE + SentencePiece `byte_fallback` —
  all reference-computed against upstream `transformers`. See
  [`docs/design/tokenizer-conformance.md`](docs/design/tokenizer-conformance.md).
- **tiktoken real-vocab parity.** The workspace-excluded
  `stringcheese-tokenizer-tiktoken-conformance` crate fetches
  OpenAI's `mergeable_ranks` blobs by SHA-256 and diffs against
  `tiktoken-rs` under `--features parity-real-vocab`. Current
  parity: **`cl100k_base` 200/200**, **`o200k_base` 200/200**.
- **Property-based + exhaustive small-domain oracles.** Every
  edit-distance and similarity kernel is checked against an
  exhaustive oracle over short inputs and against
  metric-axiom / bound / symmetry / triangle-inequality
  properties. Regressions land in `proptest-regressions/` under
  each crate.
- **Differential testing.** `cargo fuzz` targets cross-check the
  optimised SIMD backends against the scalar oracle and cross-check
  our kernels against `strsim` / `rapidfuzz`.

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
