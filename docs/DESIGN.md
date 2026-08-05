# Comparand — Design Document

Status: Design Proposal
Target Version: 0.1 (Foundation)

This document is the north-star vision for Comparand. It records the
philosophy, scope, algorithm coverage, memory model, and validation strategy
the project is being built against. Where the code has not yet caught up, this
document reflects intent rather than current state.

Subordinate design documents — comparison type system, preprocessing pipeline,
phonetic subsystem, n-gram and fingerprinting subsystem, WebAssembly/WIT
interface — will be added under `docs/` as their scope is fleshed out.

---

## Vision

Comparand is a comprehensive, high-performance Rust library for sequence
comparison. While its primary API targets strings, the underlying
architecture is designed around generic sequences.

The library emphasizes:

- Mathematical correctness
- Explicit semantics
- Performance
- Low memory overhead
- Predictable allocation behavior
- WebAssembly support
- Composability
- Explainability
- Multilingual support

Unlike existing libraries, the goal is not simply to expose implementations
of known algorithms. The goal is to provide a coherent, semantically rigorous
comparison framework where the meaning, properties, costs, and limitations
of every comparison are explicit.

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

```
comparand/
    core/           — sequence abstractions, traits, comparison types
    distance/
    similarity/
    alignment/
    phonetic/
    normalization/
    unicode/
    ngram/
    fingerprint/
    search/
    chunking/
    indexing/
    preprocessing/
    language/
    benchmarks/
    component/
```

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

Comparand should become:

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

The defining characteristic of Comparand is semantic precision. Existing
libraries generally expose algorithms. Comparand exposes algorithms and their
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

Comparand must provide objective evidence that its implementations are:

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

Disagreement must not automatically cause Comparand to imitate the majority
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
6. No unresolved differential discrepancy is classified as a Comparand defect.
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

    comparand-corpus/
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

Comparand should never ask users to trust that an implementation is correct
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
