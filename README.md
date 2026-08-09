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
below). Concrete algorithm coverage continues to fill in across
minor releases — the current release surface is best treated as
usable-but-evolving, and cross-crate integration tests catch API
shape issues as new pieces land.

See [`docs/DESIGN.md`](docs/DESIGN.md) for the full project
vision and [`docs/design/scope-and-decomposition.md`](docs/design/scope-and-decomposition.md)
for the subsystem decomposition and the wrap-vs-reimplement
policy that guides each crate.

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
| `stringcheese-tokenizer`             | Tokenizer/segmenter trait + built-in segmenters (whitespace, delimiter, identifier, grapheme, n-gram) |
| `stringcheese-tokenizer-bpe`         | Byte-Pair-Encoding tokenizer (opt-in)                                          |
| `stringcheese-tokenizer-tiktoken`    | tiktoken-compatible pre-configured tokenizer (opt-in)                          |

**Language detection**

| Crate                              | Purpose                                                                          |
|------------------------------------|----------------------------------------------------------------------------------|
| `stringcheese-detect`              | Tier-walking dispatcher over the detection stack                                 |
| `stringcheese-detect-script`       | Tier 0: ~5 KB Unicode-block script classifier                                    |
| `stringcheese-detect-whatlang`     | Tier 1: per-script whatlang shard (WASM component)                               |
| `stringcheese-detect-lingua`       | Tier 2: per-language lingua shard (WASM component)                               |

**Language packs**

Per-language crates (`stringcheese-en`, `stringcheese-de`,
`stringcheese-ja`, …) ship stopwords, stemmers, tokenisers, and
phonetic hookups for each supported language. Opt-in per pack.
See `stringcheese-lang` and `stringcheese-lang-gen` for the
plugin traits and the build-time generator that emits per-pack
capability descriptors.

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
