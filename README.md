# Comparand

Rigorous sequence comparison for Rust and WebAssembly.

Comparand is a comprehensive, performance-oriented toolkit for string and
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
  metric; a normalized value carries its normalization policy. Comparand's
  types reflect these distinctions rather than erasing them for uniformity.
- **Performance is a feature.** Runtime, allocation count, peak memory,
  binary size, and WebAssembly footprint are measured and reported alongside
  correctness.
- **WebAssembly first.** Every design decision considers the browser, WASI,
  the Component Model, and embedded targets. The core crate is `no_std`
  compatible.
- **Correctness is demonstrated, not asserted.** The library ships alongside
  a versioned conformance corpus (`comparand-corpus`) that is intended to be
  usable by other sequence-comparison libraries.

## Workspace layout

| Crate                  | Purpose                                                                        |
|------------------------|--------------------------------------------------------------------------------|
| `comparand`            | Facade crate re-exporting the public API                                       |
| `comparand-core`       | Foundational traits, result types, algorithm descriptors, workspace and sequence abstractions |
| `comparand-corpus`     | Golden-case schema and validation corpus (separately versioned deliverable)    |
| `comparand-unicode`    | Unicode normalization, case folding, grapheme handling (placeholder)           |
| `comparand-phonetic`   | Soundex, Metaphone, NYSIIS, multilingual phonetic packs (placeholder)          |
| `comparand-search`     | Substring search: Rabin-Karp, KMP, Boyer-Moore, Aho-Corasick (placeholder)     |
| `comparand-cdc`        | Content-defined chunking: FastCDC, Rabin CDC, Gear (placeholder)               |
| `comparand-index`      | Index structures: BK-tree, VP-tree, q-gram, MinHash, LSH (placeholder)         |
| `comparand-bench`      | Benchmark suite and comparative reporting (placeholder)                        |

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
