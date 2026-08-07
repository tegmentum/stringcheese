# StringCheese — Design Document

Status: Design Proposal
Target Version: 0.1 (Foundation)

This document is the north-star vision for StringCheese. It records the
philosophy, scope, algorithm coverage, memory model, and validation strategy
the project is being built against. Where the code has not yet caught up, this
document reflects intent rather than current state.

Subordinate design documents — comparison type system, preprocessing pipeline,
phonetic subsystem, n-gram and fingerprinting subsystem, WebAssembly/WIT
interface — will be added under `docs/` as their scope is fleshed out.

---

## Vision

StringCheese is a comprehensive, high-performance Rust and WebAssembly
toolkit for **string processing** — the full arc from inspecting and
shaping text (comparing, transforming, segmenting, encoding) through to
the language-specific and locale-aware operations that most string work
sooner or later needs.

The umbrella pursues three commitments existing string libraries treat
as tradeoffs, not as coherent whole:

1. **Explicit Unicode semantics at every boundary.** Every operation
   names the level it works at — bytes, Unicode Scalar Values, extended
   grapheme clusters, or display width. Nothing silently picks a
   segmentation.
2. **Allocation-conscious layered APIs.** Where the operation permits,
   borrowed / iterator / into-buffer / owned variants are all exposed;
   the pleasant default doesn't preclude the tight-loop form.
3. **Pluggable, opt-in globalization.** ICU-alternative i18n via the
   WebAssembly Component Model, with locale/capability data loaded
   from compressed data packs rather than the full monolithic ICU
   binary. Callers pay for the languages and features they use.

The library emphasizes:

- Mathematical correctness (for comparison)
- Explicit semantics (for every text-touching API)
- Performance (runtime, allocation count, peak memory, binary size)
- Predictable allocation behavior
- WebAssembly support (Wasm-first, no assumption of host runtime)
- Composability (pipelines, extension traits, configured operations)
- Explainability (algorithm descriptors, inspectable pipelines)
- Multilingual support (language packs; locale-aware Unicode)

Unlike existing libraries, the goal is not simply to expose implementations
of known algorithms. The goal is to provide a coherent, semantically
rigorous string-processing framework where the meaning, properties,
costs, and limitations of every operation are explicit.

## Scope

StringCheese is an **umbrella** — a set of coordinated crates that
together form one coherent string-processing toolkit. Each sub-project
owns a slice of the mission; the umbrella keeps them coherent.

### In scope

**Comparison** — `stringcheese-compare`, `stringcheese-align`.
Given two sequences, produce a distance, similarity, alignment, or
match result whose semantics are precise, whose cost is inspectable,
and whose correctness is testable. Every metric declares its
mathematical properties (metric axioms, bounds, normalization
policy).

**Manipulation** — `stringcheese-manip`. Inspect, trim, case, split,
join, replace, normalize, pad, slice, find, escape, quote, line
handling, and templating. Four API levels (free functions, extension
trait, configured operations, `TextPipeline` IR) so both the
pleasant one-liner and the allocation-controlled hot loop are
first-class. See the [`stringcheese-manip` module docs](../crates/stringcheese-manip/src/lib.rs)
for the module map.

**Preprocessing** — `stringcheese-unicode`. Normalization
(NFC/NFD/NFKC/NFKD), case folding, grapheme-cluster segmentation,
diacritic stripping — the Unicode-aware primitives that comparison,
manipulation, and language layers all consume.

**Phonetic** — `stringcheese-phonetic`. Sound-alike keys (Soundex,
NYSIIS, Double Metaphone) with a `PhoneticEncoder` trait so
language-specific encoders can plug in via the pack crates.

**Fingerprinting & Chunking** — `stringcheese-cdc`. Rolling-hash
fingerprints (Rabin, polynomial, Gear) and FastCDC content-defined
chunking, exposed as a streaming state machine.

**Indexing** — `stringcheese-index`. BK-tree, VP-tree, and q-gram
inverted index for metric-space and set-similarity nearest-neighbor
queries. Metric-space structures enforce metric properties at
construction.

**Language-pack infrastructure** — `stringcheese-lang`. The
`Language` trait, `LanguageProvider` discovery trait, `Stemmer` /
`Collator` / `LanguagePhoneticEncoder` plugin points, shared helper
types (`Stopwords`, `SimpleTokenizer`), and a static `registry`
(linkme `distributed_slice`) each `stringcheese-<lang>` pack opts
into via `register_language!`. Data-only — no per-language
implementations live here. Callers picking a language at runtime
(user locale, config file, `Accept-Language` header) reach for
`registry::language(code)`; callers who name the pack at compile
time keep using the pack's `ENGLISH` / `GERMAN` / `FRENCH`
constant. Full BCP-47 fallback (`"pt-BR" → "pt"`) is a v0.2 follow-up.

**Language packs** — `stringcheese-<language>` (e.g.,
`stringcheese-en`, planned: `stringcheese-de`, `stringcheese-ja`, …).
Data-driven implementations of stemming, stopword lists,
language-specific phonetic encoders, tokenization rules, collation
tailoring, and morphological analysis — one opt-in crate per
supported language. The `stringcheese-en` pack ships in v0.1 with a
~150-word stopword list, the Porter (1980) stemmer, the default
whitespace-and-punctuation tokenizer, and a Soundex phonetic hookup.
Additional language packs land as the algorithm-family coverage
matures.

**Component-model globalization** (planned) — `stringcheese-icu-*`
WIT interfaces and data packs. Callers instantiate just the
interfaces they need (case mapping, collation, plural rules, date
formatting) and load only the locales they support. A proposed
compressed data-pack format (SCUD — StringCheese Unicode Data)
packages CLDR-derived tables at a fraction of ICU's binary size by
composing range deltas, adaptive paging, packed integers, and outer
Brotli/Zstd compression.

**Substrate** — `stringcheese-core`, `stringcheese-corpus`. Traits,
result newtypes, algorithm-variant descriptors, workspace/sequence
abstractions, and the golden-case validation schema every sub-project
uses.

### Not in scope for the umbrella

**Record linkage** — combining per-field comparisons into whole-record
match/non-match decisions, blocking strategies, learned or
probabilistic classifiers that consume per-field scores. StringCheese
supplies the per-field scores and the metric-space blocking indexes;
deciding whether two records refer to the same real-world entity is
a downstream concern.
See [record-linkage](https://github.com/tegmentum/record-linkage)
for the sibling library that implements the Fellegi-Sunter classifier
and sorted-neighborhood blocking on top of StringCheese. The sibling
will be renamed to `stringcheese-linkage` and moved under the
umbrella name in a follow-up wave; the substantive scope split
(compute per-field vs decide per-record) remains.

**Regex engines** — StringCheese's `find` / `replace` accept
`Pattern`s in the `str::find` sense (literals, closures, char sets).
Full regex is a separate library, not an umbrella responsibility.

**I/O and reader-driven pipelines** — manipulation and comparison
operate on in-memory `&str` / `&[u8]`. Streaming from a reader is a
downstream concern.

**Full ICU parity** — the WIT-based i18n interfaces target the
80/90/95 % of locale-aware use cases with pluggable, opt-in data
packs; parity with ICU's every corner (Java-only APIs, historical
calendar edge cases, deep transliteration graphs) is not the goal.
Callers who need that reach for ICU4X directly.

Historically, this repo shipped a `stringcheese-linkage` crate and a
`sorted_neighborhood` module in `stringcheese-index`; both were
extracted to the sibling record-linkage repo when the scope decision
crystallized. See the extraction commit for the migration record.

### Sub-project map

| Crate | Charter |
| --- | --- |
| `stringcheese` | Facade — re-exports every sub-project under one dependency |
| `stringcheese-core` | Traits, result types, descriptors, workspace/sequence abstractions |
| `stringcheese-corpus` | Golden-case schema, oracle framework, differential harness |
| `stringcheese-compare` | Comparison kernels: Levenshtein, Hamming, Jaro/Jaro-Winkler, Damerau/OSA, LCS, n-gram, set-similarity, MinHash, substring search |
| `stringcheese-align` | Pairwise alignment: Needleman-Wunsch, Smith-Waterman, edit scripts |
| `stringcheese-manip` | Manipulation: inspect/trim/case/split/join/replace/normalize/pad/slice/find/escape/quote/lines/template + `TextPipeline` IR |
| `stringcheese-unicode` | Preprocessing: NFC/NFD/NFKC/NFKD, case folding, graphemes, diacritics |
| `stringcheese-phonetic` | Phonetic keys: Soundex, NYSIIS, Double Metaphone (language-neutral core) |
| `stringcheese-cdc` | Rolling-hash fingerprints + FastCDC content-defined chunking |
| `stringcheese-index` | Metric-space and set-similarity indexes: BK-tree, VP-tree, q-gram inverted |
| `stringcheese-bench` | Criterion benchmarks + allocation-counting harness |
| `stringcheese-lang` | Language-pack infrastructure: `Language` trait, `Stemmer` / `Collator` / `LanguagePhoneticEncoder` plugin points, `Stopwords` and `SimpleTokenizer` helpers, plus a static `registry` (linkme distributed slice) each pack self-registers into via `register_language!` |
| `stringcheese-en` | English pack: ~150-word stopword list, Porter (1980) stemmer, simple tokenizer, Soundex phonetic hookup; self-registers into `stringcheese-lang::registry` as `"en"` |
| `stringcheese-de` | German pack: ~200-word stopword list, Snowball German stemmer, simple tokenizer, Kölner Phonetik (Postel 1969) hookup; self-registers into `stringcheese-lang::registry` as `"de"` |
| `stringcheese-fr` | French pack: ~200-word stopword list, Snowball French stemmer, elision-aware tokenizer, PHONEX phonetic hookup; self-registers into `stringcheese-lang::registry` as `"fr"` |
| `stringcheese-ja` | Japanese pack: ~120-word stopword list, character-type-based (dictionary-free) tokenizer, Kunrei-shiki (ISO 3602) romanization phonetic hookup, minimal polite/plural stemmer. First non-Latin-script pack — full morphological tokenization deferred (needs kuromoji-scale dictionary outside the wasm-first / offline-first envelope). Self-registers into `stringcheese-lang::registry` as `"ja"`. |
| `stringcheese-<lang>` | Additional language-specific implementations (planned; one opt-in crate per language) |
| `stringcheese-tokenizer` | Tokenizer/segmenter trait crate + built-in tokenizers (whitespace, delimiter, identifier, grapheme, n-gram). Foundation for downstream subword algorithm crates and model packs. See [docs/design/tokenizers.md](./design/tokenizers.md). |
| `stringcheese-tokenizer-bpe` | Data-neutral Byte-Pair Encoding (Sennrich, Haddow, Birch 2016) algorithm crate — caller supplies merge table and vocabulary. Substrate for the `stringcheese-tokenizer-tiktoken` model pack (shipped) and the planned `-huggingface` pack. |
| `stringcheese-tokenizer-tiktoken` | OpenAI tiktoken model tokenizer pack — `cl100k_base` (default feature), `p50k_base`, `r50k_base`, `o200k_base` shipped as SCUD-lite BPE data on top of `stringcheese-tokenizer-bpe`. Each variant behind its own Cargo feature; lazy-decode via `OnceLock`. Real OpenAI `mergeable_ranks` blobs are not committed for licence + repo-bloat reasons; the crate's `build.rs` synthesises a small stand-in tokenizer per variant and transcodes contributor-supplied plaintext blobs from `data/<variant>.tiktoken` into SCUD-lite deflate when present. See [docs/design/tokenizers.md § 6](./design/tokenizers.md#6-tiktoken-pack--stringcheese-tokenizer-tiktoken). |
| `stringcheese-tokenizer-*` | Additional subword-tokenizer algorithm crates (planned: `-wordpiece`, `-sentencepiece`) and pre-configured model packs (planned: `-huggingface`) |
| `stringcheese-icu-*` | WIT interfaces + SCUD data packs for i18n (planned) |

The scope boundary is a coherent commitment, not a fence against
convenience. Utilities that drift outside these charters belong in
downstream libraries. Utilities that would sit awkwardly across two
sub-projects (e.g., "manip needs to know something a lang pack knows")
are handled through explicit dependency edges, not by expanding a
crate's scope.

## Philosophy

Most comparison algorithms have existed for decades.

The innovation is not implementing Levenshtein yet again.

The innovation is providing:

- complete coverage
- excellent engineering
- consistent APIs
- explicit semantics
- performance transparency
- reusable infrastructure
- language awareness
- WebAssembly-first implementation

The library should become the canonical Rust toolkit for sequence comparison.

## Design Principles

### Preserve Semantics

The API should never erase semantic differences simply to create a uniform
interface.

- Distance is not similarity.
- Similarity is not probability.
- Scores are not metrics.
- Metric properties matter.
- Normalization policies matter.

Everything should remain explicit.

### Performance Is a Feature

Performance includes:

- runtime
- memory usage
- allocation count
- peak memory
- binary size
- WebAssembly footprint
- cache locality
- SIMD utilization

### WebAssembly First

The library is intended to be a core component within WasmOS, DuckLink,
SQLink, and future Tegmentum projects.

Every design decision should consider:

- browser
- WASI
- Component Model
- embedded
- no_std

## Architecture

The umbrella is a set of coordinated Rust crates in one workspace,
plus a WIT component-model surface and (planned) opt-in language and
i18n data packs.

```
stringcheese/                       — the workspace root
├── crates/
│   ├── stringcheese                — facade (re-exports every sub-project)
│   ├── stringcheese-core           — traits, result types, descriptors,
│   │                                 workspace/sequence abstractions
│   ├── stringcheese-corpus         — golden-case schema, oracle framework
│   │
│   ├── stringcheese-compare        — comparison kernels (edit distance,
│   │                                 similarity, n-gram, MinHash, search)
│   │     src/
│   │       ├── levenshtein/        — module per algorithm family
│   │       ├── hamming/            — (was 9 sibling crates before consolidation)
│   │       ├── jaro/
│   │       ├── damerau/
│   │       ├── lcs/
│   │       ├── ngram/
│   │       ├── search/
│   │       ├── set_similarity/
│   │       └── minhash/
│   │
│   ├── stringcheese-align          — pairwise alignment (NW, SW, edit scripts)
│   ├── stringcheese-manip          — inspect/trim/case/split/…/pipeline
│   │                                 (scaffold in v0.1; populates in
│   │                                 subsequent releases)
│   │
│   ├── stringcheese-unicode        — normalization, case folding, graphemes
│   ├── stringcheese-phonetic       — Soundex, NYSIIS, Double Metaphone
│   ├── stringcheese-cdc            — rolling-hash + FastCDC chunking
│   ├── stringcheese-index          — BK-tree, VP-tree, q-gram inverted
│   │
│   └── stringcheese-bench          — criterion + allocation-counting harness
│
├── component/                      — WebAssembly Component Model surface
│   ├── wit/stringcheese.wit        — interface definition
│   └── rust-host/                  — reference host binding
│
├── fuzz/                           — cargo-fuzz differential + axiom targets
├── bench-adapters/                 — head-to-head vs strsim, rapidfuzz, …
└── docs/                           — design docs, references, publish runbook

# Planned, not shipped in v0.1
crates/
  ├── stringcheese-en, -de, -fr, -ja, …   — one crate per supported language
  └── stringcheese-icu-*                    — WIT interfaces for i18n

data/
  └── *.scud                                — compressed CLDR-derived data
                                              packs (StringCheese Unicode Data)
```

Sub-projects depend upward, not sideways. `stringcheese-manip` uses
`stringcheese-unicode` and (for `find`/`replace`) `stringcheese-compare`;
`stringcheese-index` uses `stringcheese-compare` for the metrics it
indexes; the language packs use `stringcheese-phonetic` /
`stringcheese-unicode` / `stringcheese-manip`. The facade
`stringcheese` re-exports the public surface of every sub-project so
callers who don't need fine-grained dependency selection can add one
crate to `Cargo.toml`.

## Core Sequence Model

The library fundamentally compares sequences.

Possible sequence types include:

- bytes
- Unicode scalar values
- grapheme clusters
- tokens
- phonemes
- generic slices

Strings are simply one specialization.

## Comparison Categories

The library recognizes multiple categories.

### Distance

Lower is better.

Examples: Levenshtein, Hamming, Damerau, edit distance.

### Similarity

Higher is better.

Examples: Jaro, Jaro-Winkler, cosine, Dice, Jaccard similarity.

### Score

Neither distance nor similarity.

Examples: Smith-Waterman, Needleman-Wunsch, probabilistic linkage,
learned scoring models.

### Predicate

Examples: phonetic key equality, exact equality, prefix/suffix matching.

## Mathematical Properties

Algorithms should expose their mathematical guarantees.

- Metric
- Semimetric
- Pseudometric
- Quasimetric
- Divergence
- Similarity
- Kernel
- Score

Each implementation exposes:

- symmetry
- identity preservation
- triangle inequality
- boundedness
- normalization

This information is usable by indexing structures. Example: a BK-tree should
only accept true metrics.

## Result Types

The library avoids returning anonymous floating-point values.

Instead: `Distance<T>`, `Similarity<T>`, `Score<T>`, `NormalizedDistance`,
`NormalizedSimilarity`.

Conversions are explicit. No global rule such as `distance = 1 - similarity`
exists. Normalization policy must be specified.

### Normalization Policies

Examples for Levenshtein:

- divide by max length
- divide by total length
- custom

Normalization becomes an explicit strategy.

## Representation Layers

Algorithms should work over multiple representations:

- bytes
- Unicode scalars
- graphemes
- words
- tokens
- phonemes

The API should never silently choose.

## Algorithms

### Edit Distance
- Levenshtein
- Weighted Levenshtein
- Damerau-Levenshtein
- Optimal String Alignment
- Hamming
- Longest Common Subsequence
- Longest Common Substring

### Alignment
- Needleman-Wunsch
- Smith-Waterman
- Affine gap alignment
- Edit script reconstruction

### Similarity
- Jaro
- Jaro-Winkler
- Dice
- Jaccard
- Overlap coefficient
- Cosine similarity

### N-Gram Measures
- Dice
- Jaccard
- Cosine
- Weighted Jaccard
- Containment similarity

### Phonetic Matching

Phonetics is a first-class subsystem — not merely another comparison
function. Supported algorithms include:

- Soundex
- Refined Soundex
- Metaphone
- Double Metaphone
- NYSIIS
- Match Rating
- Cologne Phonetics
- Caverphone
- Daitch-Mokotoff
- Beider-Morse

### Multilingual Support

The library supports as many languages as practical. Language support is
modular:

- phonetic-germanic
- phonetic-romance
- phonetic-slavic
- phonetic-semitic
- phonetic-indic
- phonetic-cjk

Support includes language, script, and region. The API distinguishes native
script, transliteration, and pronunciation rules.

### Phoneme-Level Comparison

Long-term goal. Rather than comparing phonetic hashes, compare phoneme
sequences with phoneme edit distance. Supports multilingual matching.

## Unicode

Unicode is modular:

- NFC / NFD / NFKC / NFKD
- case folding
- grapheme segmentation
- diacritic removal
- transliteration

## Preprocessing Pipeline

Comparison is rarely performed on raw strings. Pipeline objects are
reusable:

    normalize -> case fold -> remove punctuation -> collapse whitespace
        -> tokenize -> phonetic encoding -> comparison

## N-Grams

N-grams are a representation layer — not merely a comparison algorithm.

Supported representations: character, byte, grapheme, token, phoneme,
skip-grams.

Policies: boundary markers, multiplicity, weighting, fixed N, variable N.

Representations: set, multiset, weighted vector.

## Fingerprinting

Separate subsystem:

- Rabin fingerprints
- Polynomial rolling hash
- Buzhash
- Gear hash

## Search Algorithms

- Rabin-Karp
- KMP
- Boyer-Moore
- Horspool
- Two-way search
- Aho-Corasick

## Content Defined Chunking

Support Rabin CDC and FastCDC. Streaming interfaces. Reusable boundaries.
No unnecessary allocation.

## Index Structures

Future subsystem:

- BK-tree
- VP-tree
- N-gram inverted index
- Prefix filtering
- Length filtering
- MinHash
- Locality-sensitive hashing

## Memory Philosophy

Memory is explicit. Every algorithm documents:

- runtime
- auxiliary memory
- allocation behavior
- workspace requirements

### Workspace Reuse

Essential for entity resolution, databases, and WebAssembly.

### Streaming APIs

Many algorithms support streaming: FastCDC, rolling hashes, Rabin-Karp,
tokenization, fingerprinting.

## SIMD

Optional. Supported backends: scalar, native SIMD, wasm SIMD.

SIMD must never change observable behavior.

## WebAssembly

Primary deployment target. Requirements:

- no_std core
- alloc optional
- deterministic memory
- streaming
- reusable workspaces
- feature-gated Unicode
- feature-gated phonetics

### Component Model

Future WIT interface. Supports comparison, prepared objects, reusable
preprocessing, workspace reuse.

## Explainability

Comparison results should explain themselves. Example:

    Normalization:  NFKC
    Representation: Grapheme
    Algorithm:      Jaro-Winkler
    Similarity:     0.94
    Language:       German
    Phonetic:       Double Metaphone
    Threshold:      Passed

Entity resolution benefits enormously from explainability.

## Benchmark Philosophy

Benchmark more than runtime:

- runtime
- allocations
- peak memory
- binary size
- Wasm size
- SIMD improvement
- throughput
- cold start
- warm performance

## Feature Flags

- core
- distance
- alignment
- phonetic
- phonetic-germanic
- phonetic-slavic
- unicode
- unicode-full
- fingerprint
- search
- chunking
- indexing
- simd
- parallel
- std
- alloc

## Public Goals

StringCheese should become:

- the definitive Rust comparison library
- the reference implementation for sequence comparison
- suitable for production-scale entity resolution
- usable in databases
- usable in browsers
- usable in Wasm components
- usable in embedded systems
- suitable for DuckLink and SQLink integration
- suitable for WasmOS infrastructure

## Version 0.1 Scope

Core infrastructure:

- Comparison abstractions
- Result types
- Mathematical property system
- Normalization framework
- Unicode preprocessing
- Levenshtein
- Damerau
- Hamming
- Jaro
- Jaro-Winkler
- Dice
- Jaccard
- Character and token n-grams
- Soundex
- Double Metaphone
- NYSIIS
- Workspace reuse
- SIMD where appropriate
- no_std core
- WebAssembly support
- Comprehensive benchmark suite

## Future Roadmap

### Version 0.2
- Smith-Waterman
- Needleman-Wunsch
- affine gaps
- phoneme representations
- multilingual phonetic packs
- BK-trees
- VP-trees
- FastCDC
- Rabin fingerprints
- Gear hash
- Rabin-Karp
- Buzhash
- streaming APIs

### Version 0.3
- probabilistic linkage primitives
- MinHash
- locality-sensitive hashing
- learned similarity models
- Component Model bindings
- database integration
- SQL operators
- DuckLink integration
- SQLink integration

## Guiding Principle

The defining characteristic of StringCheese is semantic precision. Existing
libraries generally expose algorithms. StringCheese exposes algorithms and their
meaning. Every comparison carries explicit information about:

- what was compared
- how it was normalized
- what mathematical guarantees apply
- what computational cost was incurred
- why two sequences matched

The library should be known not simply for the breadth of algorithms it
implements, but for making sequence comparison correct, explainable,
performant, multilingual, and practical across native and WebAssembly
environments.

---

# Validation, Golden Datasets, and Comparative Benchmarking

## Purpose

StringCheese must provide objective evidence that its implementations are:

- mathematically correct
- semantically well-defined
- compatible with published algorithm definitions
- consistent across native and WebAssembly targets
- competitive with existing libraries
- efficient in both runtime and memory usage

Correctness and performance validation are first-class deliverables. The
validation system should be substantial enough that it can independently
serve as a reference corpus for string-comparison implementations.

## Validation Strategy

Validation uses several complementary methods. No single method is
sufficient.

### Validation Layers

- Hand-authored canonical examples
- Exhaustive small-domain testing
- Property-based testing
- Differential testing against independent implementations
- Golden datasets
- Metamorphic testing
- Cross-backend consistency testing
- Performance and memory benchmarking
- Fuzzing
- Specification and paper conformance tests

### Canonical Test Vectors

Each algorithm includes canonical examples derived from original papers,
standards, widely cited textbook examples, authoritative reference
implementations, and manually verified edge cases.

Examples cover empty strings, identical strings, one empty string,
one-character differences, repeated symbols, transpositions, prefixes and
suffixes, Unicode, normalization-sensitive strings, asymmetric inputs,
maximum-distance cutoffs, integer overflow boundaries, long inputs.

Canonical vectors record the expected result and its derivation.

### Exhaustive Small-Domain Oracles

For algorithms where a straightforward implementation is practical, maintain
an intentionally simple oracle implementation. The oracle prioritizes
clarity and correctness over performance.

Then exhaustively generate all strings over small alphabets (e.g. `{a, b}`
lengths 0–8 or `{a, b, c}` lengths 0–6). Every optimized implementation must
agree with the oracle.

This is particularly important for banded edit distance, cutoff-aware
implementations, bit-parallel algorithms, SIMD implementations, compact
integer-cell variants, streaming implementations, and hashed n-gram
representations.

### Independent Oracle Implementations

Optimized implementations should not validate themselves. For important
algorithms, maintain at least two structurally independent implementations.
Agreement among implementations written from different formulations provides
stronger evidence than agreement among minor variants of the same code.

The oracle implementation resides in a validation-only crate and is not
compiled into normal library builds.

### Property-Based Testing

Metric properties (for algorithms declared as metrics):

    d(x, y) >= 0
    d(x, y) = 0 iff x = y
    d(x, y) = d(y, x)
    d(x, z) <= d(x, y) + d(y, z)

These are tested over generated sequences. Where properties depend on
configuration, tests generate only valid configurations or verify that
invalid configurations are rejected.

### Metamorphic Testing

Validates relationships between transformed inputs when exact expected
outputs are difficult to enumerate:

- Identity-preserving transformations (case folding, normalization)
- Prefix and suffix effects: `d(prefix + x, prefix + y) = d(x, y)`
- Symbol renaming (equality-only algorithms)
- Representation equivalence (prepared vs. unprepared)
- Backend equivalence (scalar = native SIMD = wasm SIMD)

### Differential Testing

Compares outputs against multiple independent libraries and language
ecosystems. The objective is not to blindly match every implementation — it
is to identify genuine defects, semantic ambiguities, normalization
differences, variant mismatches, and undocumented edge-case behavior.

Disagreement must not automatically cause StringCheese to imitate the majority
result. The implementation must follow its declared semantics and source
definition.

### Algorithm Variant Registry

Many algorithms have multiple incompatible definitions under the same name:

- restricted vs. unrestricted Damerau-Levenshtein
- optimal string alignment vs. full Damerau-Levenshtein
- several Levenshtein normalization formulas
- different Jaro matching-window definitions
- Jaro-Winkler prefix limits
- set vs. multiset Dice
- cosine distance vs. angular distance
- Soundex variants
- language-specific phonetic variants
- FastCDC normalization levels and masks

Each implementation has a stable variant identifier (`AlgorithmDescriptor`).
Golden datasets refer to the variant identifier rather than only the common
algorithm name.

## Golden Dataset Design

Golden datasets are versioned, machine-readable, and independently
consumable. Recommended formats: JSON Lines for readability, CBOR or
MessagePack for compact test execution, Parquet for large analytical
datasets, plain text manifests for provenance and licensing.

Each case includes: id, algorithm, variant, left, right, expected,
representation, normalization, source, and tags.

### Golden Dataset Categories

- Core edit-distance corpus (unit-cost, weighted, transpositions, unequal
  lengths, threshold boundaries, Unicode scalar and grapheme cases)
- Similarity corpus (Jaro/Jaro-Winkler examples, symmetry tests, prefix-boost
  boundaries, floating-point tolerances)
- N-gram corpus (all combinations of representation × n × padding × set/multiset)
- Phonetic corpus (multilingual, curated by algorithm applicability)
- Search corpus (Rabin-Karp/KMP/Boyer-Moore edge cases)
- Fingerprint corpus (known fingerprints, window transitions, rolling updates)
- Chunking corpus (FastCDC exact boundaries, chunk lengths, streaming
  vs. contiguous)
- Real-world corpora (personal names, company names, addresses,
  bibliographic records, multilingual text, OCR-like corruption)
- Regression corpus (every discovered bug becomes a permanent golden case)

### Dataset Provenance

Every dataset includes source, license, retrieval date, transformation
history, filtering rules, version, and cryptographic digest. Generated
datasets include random seed, generator version, and generator configuration.

### Floating-Point Validation

Floating-point algorithms require explicit comparison policy. Each algorithm
defines one of: exact bitwise equality, absolute tolerance, relative
tolerance, or ULP tolerance. Golden records store both the expected value
and comparison policy.

### Cross-Target Validation

Every release validates at least native scalar, native SIMD, wasm32-wasip1,
wasm32-unknown-unknown, WebAssembly SIMD, debug and release builds, and
32-bit and 64-bit targets where practical.

### Fuzzing

Fuzz targets include all public comparison functions, UTF-8 boundaries,
malformed byte-sequence APIs, custom cost tables, normalization pipelines,
prepared representations, streaming chunk boundaries, rolling hash state
transitions, and workspace sizing.

Important differential fuzz targets: optimized vs. oracle; scalar vs. SIMD;
contiguous vs. streaming; prepared vs. direct; native vs. Wasm.

## Performance Benchmarks

Correctness benchmarks and performance benchmarks remain distinct.

### Benchmark Dimensions

- latency, throughput, CPU time, wall-clock time
- allocations, total bytes allocated, peak resident memory
- scratch-memory requirement
- Wasm linear-memory growth
- compiled binary size, component size, instantiation time
- cold first-call latency vs. steady-state performance

### Input Dimensions

Benchmark across input length, alphabet size, edit distance, percentage
similarity, ASCII vs. multilingual Unicode, repeated symbols, random inputs,
natural-language inputs, short names vs. long documents, batch size,
threshold value, prepared vs. unprepared operation.

### Workload Modes

Single pair; one query against many candidates; all-pairs comparison;
thresholded filtering; top-k ranking; streaming input; prepared corpus;
index-assisted lookup.

### Comparative Library Benchmarking

Adapters live under `bench-adapters/{rust,python,java,javascript,cpp,go}/`.

Each adapter performs no unnecessary conversion inside timed regions,
preloads or prepares inputs consistently, separates startup cost from
steady-state cost, exposes allocation metrics where possible, and emits
results in a common machine-readable format.

Do not compare differently defined algorithms under the same label. Results
clearly identify non-equivalent variants.

### Pareto Analysis

Report Pareto frontiers rather than optimize for runtime alone. Dimensions:
latency, throughput, memory, allocations, binary size, implementation
capability, Unicode support, cutoff support, edit-script support, streaming
support.

## Continuous Integration Requirements

Every pull request runs: unit tests, canonical golden tests, property
tests, differential tests against internal oracles, regression corpus,
scalar/SIMD equivalence, native/Wasm equivalence, fuzz smoke tests,
benchmark compilation checks.

Nightly or scheduled CI runs: full external differential suite, large
golden datasets, long-running fuzzing, full comparative benchmarks, memory
benchmarks, binary-size tracking.

## Release Gates

A release does not proceed unless:

1. All golden datasets pass.
2. All declared mathematical properties pass generated tests.
3. All optimized implementations agree with their independent oracle.
4. Native and WebAssembly results agree.
5. Scalar and SIMD implementations agree.
6. No unresolved differential discrepancy is classified as a StringCheese defect.
7. Performance regressions beyond defined thresholds are reviewed.
8. Binary-size and memory regressions are reviewed.
9. Dataset and benchmark versions are recorded in the release manifest.

## Public Correctness Report

Each release publishes a machine-generated correctness report:

- Algorithms tested
- Variants tested
- Golden cases executed
- Generated cases executed
- External implementations compared
- Agreements
- Known semantic differences
- Known external discrepancies
- Fuzzing duration
- Targets tested
- Dataset versions

## Golden Dataset as a Project Asset

The golden corpus is a standalone deliverable. Structure:

    stringcheese-corpus/
        schema/
        edit-distance/
        similarity/
        ngram/
        phonetic/
        search/
        fingerprint/
        chunking/
        unicode/
        regression/
        tools/
        manifests/

The corpus is versioned independently from the Rust library.

## Implementation Sequence

### Phase 1
1. Define golden-case schema.
2. Build full-matrix edit-distance oracles.
3. Add exhaustive small-alphabet generators.
4. Add canonical examples.
5. Add property-based tests.
6. Add scalar vs. optimized differential tests.

### Phase 2
1. Build external benchmark adapter protocol.
2. Compare against selected Rust, Python, Java, and JavaScript implementations.
3. Add automated discrepancy classification.
4. Publish initial correctness report.

### Phase 3
1. Add multilingual phonetic corpora.
2. Add Unicode normalization corpus.
3. Add n-gram representation corpus.
4. Add native/Wasm equivalence harness.

### Phase 4
1. Add fingerprint and chunking datasets.
2. Add streaming split enumeration.
3. Add comparative performance dashboards.
4. Publish the corpus as a separately versioned project.

## Design Principle

StringCheese should never ask users to trust that an implementation is correct
because it is fast, widely used, or resembles a textbook implementation.
Correctness must be demonstrated through:

- independent derivation
- exhaustive testing
- differential comparison
- mathematical properties
- cross-platform consistency
- permanent regression datasets

The benchmark and golden-data infrastructure is part of the product, not
incidental test code. This gives the project a second defensible asset: not
just the Rust implementation, but a substantial, versioned sequence-comparison
conformance corpus that other libraries can test against.
