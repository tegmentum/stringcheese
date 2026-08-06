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

Version 0.1 is **under initial development**. The current crate skeleton
covers only the type-system substrate: result types, metric traits,
mathematical-property descriptors, algorithm-variant registry, workspace and
sequence traits, and the golden-case validation schema. Concrete algorithm
implementations arrive in subsequent milestones.

See [`docs/DESIGN.md`](docs/DESIGN.md) for the full project vision, algorithm
coverage, and validation strategy.

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

| Crate                  | Purpose                                                                        |
|------------------------|--------------------------------------------------------------------------------|
| `stringcheese`            | Facade crate re-exporting the public API                                       |
| `stringcheese-core`       | Foundational traits, result types, algorithm descriptors, workspace and sequence abstractions |
| `stringcheese-corpus`     | Golden-case schema and validation corpus (separately versioned deliverable)    |
| `stringcheese-compare`    | Comparison kernels: Levenshtein, Hamming, Jaro/Jaro-Winkler, Damerau/OSA, LCS, n-gram, set similarity (Dice/Jaccard/Overlap/Cosine), substring search (Rabin-Karp/KMP/Boyer-Moore/Aho-Corasick/Horspool/Two-way), MinHash and LSH |
| `stringcheese-unicode`    | Unicode normalization, case folding, grapheme handling                         |
| `stringcheese-phonetic`   | Soundex, NYSIIS, Double Metaphone, phonetic-matcher composer                   |
| `stringcheese-cdc`        | Content-defined chunking: FastCDC, Rabin CDC, Gear rolling-hash                |
| `stringcheese-index`      | Index structures: BK-tree, VP-tree, q-gram inverted index                      |
| `stringcheese-align`      | Pairwise alignment: Needleman-Wunsch, Smith-Waterman, affine gaps              |
| `stringcheese-bench`      | Benchmark suite and comparative reporting                                      |

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
