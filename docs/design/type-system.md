# Comparison Type System

Status: Design
Applies to: StringCheese 0.1 and later
Related: [DESIGN.md](../DESIGN.md), [preprocessing-pipeline.md](./preprocessing-pipeline.md), [phonetic-subsystem.md](./phonetic-subsystem.md), [ngram-and-fingerprinting.md](./ngram-and-fingerprinting.md), [wasm-and-wit-interface.md](./wasm-and-wit-interface.md)

The reference specification for StringCheese's result-type hierarchy, metric traits, algorithm-descriptor scheme, sequence contract, workspace contract, and normalization policy — the substrate every algorithm crate builds against.

## Purpose

The type system exists for two reasons.

- **Preserve semantics.** Distance and similarity carry opposite directional meaning. A normalized value carries a normalization policy. A raw alignment score has no meaning without its scoring scheme. Erasing these distinctions to hand back a bare `f64` from every algorithm is what most libraries do; it is what StringCheese refuses to do.
- **Prevent silent misuse.** A value returned from a distance function cannot accidentally be passed to code expecting a similarity. A cutoff-exceeded result cannot accidentally be treated as if the true distance equalled the cutoff. A raw distance cannot accidentally be passed to an index structure that requires a metric. Each rule is enforced by a distinct nominal type or an inspectable descriptor, not by convention or documentation.

The system is intentionally shallow — a handful of concrete result types, three traits, one descriptor scheme, one sequence trait, one workspace trait, one policy enum. Every algorithm crate speaks these types and nothing else at its boundary.

## The result-type hierarchy

Every StringCheese comparison returns one of six result types. All six live in [`stringcheese_core::result`](../../crates/stringcheese-core/src/result.rs); this section is authoritative documentation for their intended shape.

### `Distance<T>`

- **Meaning.** A distance between two sequences. Lower is better; zero means identity under the metric's own notion of equality.
- **Invariants.** None enforced by the type itself. The algorithm's implementation is responsible for meeting the non-negativity, symmetry, triangle-inequality, and identity claims it makes through [`MetricProperties`](#metricproperties-and-metricclass).
- **Layout.** `#[repr(transparent)]`. A `Distance<u32>` has the same bit layout as a `u32`, so wrapping is free and unwrapping is free — the type is nominal, not runtime.
- **Default `T`.** `u32`. Unit-cost integer edit distances (Levenshtein, Damerau, Hamming, LCS) all fit; `u32` is portable across native and Wasm targets and does not carry the platform-word-size ambiguity of `usize`.
- **Why not a bare number.** A `u32` returned from Levenshtein and a `u32` returned from Hamming would be mutually assignable, freely mixed, and freely combined arithmetically — none of which is meaningful. Wrapping in `Distance<T>` makes distance a *sort* of value, not a number that happens to have come from a distance function.

### `Similarity<T>`

- **Meaning.** A similarity between two sequences. Higher is better. The range is *not* constrained — cosine similarity may naturally live in `[-1, 1]`; weighted schemes can exceed `1.0`. Where a bounded `[0, 1]` similarity is required, [`NormalizedSimilarity`](#normalizedsimilarity) is the correct type.
- **Invariants.** None enforced.
- **Layout.** `#[repr(transparent)]`.
- **Default `T`.** `f64`. Most similarity algorithms are naturally continuous.
- **Why not a bare number.** Same as `Distance<T>`: the wrapper prevents accidental mixing across algorithms and across the distance/similarity boundary.

### `Score<T>`

- **Meaning.** A raw alignment or scoring value whose interpretation depends on the scoring scheme. Neither a distance nor a similarity in the canonical sense.
- **Invariants.** None; the sign, range, and monotonicity conventions are all specific to the scoring scheme.
- **Layout.** `#[repr(transparent)]`.
- **Default `T`.** `f64`.
- **Why it exists.** Smith–Waterman and Needleman–Wunsch return values that are not distances (they do not satisfy non-negativity or the triangle inequality) and not similarities in the `Similarity<T>` sense (they are not comparable across scoring schemes). Probabilistic linkage scores and learned scoring models sit in the same bucket. Collapsing them into `Similarity<T>` would make it too easy to feed them into infrastructure that expects a similarity's basic ordering behavior on arbitrary inputs.

### `NormalizedDistance`

- **Meaning.** A distance normalized to the closed interval `[0.0, 1.0]`. Zero means identity under the applied normalization policy; one means maximal dissimilarity.
- **Invariants.** *Enforced.* The checked constructor `NormalizedDistance::new(value: f64) -> Option<Self>` rejects non-finite values and values outside `[0.0, 1.0]`. A `new_unchecked` constructor exists for interior paths that have already proven the invariant; it is safe (the invariant is not memory-safety-related) but violation produces nonsensical downstream behavior.
- **Layout.** `#[repr(transparent)]` over `f64`.
- **Fixed `T`.** `f64`. A normalized distance is inherently a real number in `[0, 1]`; there is no benefit to parameterization.
- **Constants.** `IDENTITY = 0.0`, `MAXIMUM = 1.0`.
- **Relationship to normalization policy.** The type *guarantees the range*. It does not *record which policy produced the value*; that is [`NormalizationPolicy`](#normalizationpolicy)'s job, and it travels alongside the value at the layer that cares (typically the algorithm configuration or a pipeline descriptor).

### `NormalizedSimilarity`

- **Meaning.** A similarity normalized to `[0.0, 1.0]`. One means identity; zero means maximal dissimilarity.
- **Invariants.** *Enforced*, same as `NormalizedDistance`.
- **Layout.** `#[repr(transparent)]` over `f64`.
- **Constants.** `MINIMUM = 0.0`, `IDENTITY = 1.0`.
- **Why not just `1.0 - NormalizedDistance`.** See [Why there is no implicit distance↔similarity conversion](#why-there-is-no-implicit-distancesimilarity-conversion).

### `BoundedDistance<T>`

- **Meaning.** The result of a distance computation performed with a maximum-distance cutoff. Either the exact distance (if it lies at or below the cutoff) or the fact that the true distance exceeds the cutoff.
- **Layout.** An enum, *not* `#[repr(transparent)]`. The `Exceeded` variant carries the cutoff value.

```rust
pub enum BoundedDistance<T = u32> {
    Within(Distance<T>),
    Exceeded { cutoff: T },
}
```

- **Default `T`.** `u32`, matching `Distance<T>`.

## Why `BoundedDistance<T>` is a distinct enum, not `Option<Distance<T>>`

The two carry different meanings and support different downstream reasoning.

- `Option<Distance<T>>` reads as *the distance is either present or absent*. Absence usually means *not computed*, or *not applicable*, or *an error occurred* — the most common conventions across the ecosystem. Cutoff exceedance is none of these: the result *is* present, *is* well-defined, and *does* carry information (the cutoff that was exceeded).
- The `Exceeded { cutoff }` variant records *which cutoff was exceeded*. Diagnostic output, threshold-tuning tools, and re-runs at a wider cutoff all need this. `Option::None` would erase it.
- The `Within(Distance<T>)` variant carries a real `Distance<T>`, which means it inherits the entire distance-type API (formatting, comparison against other distances of the same `T`, normalization via `NormalizationPolicy`) without any wrapper-unwrapping ceremony.
- An enum with two named variants is more legible at call sites than a nested `Option`: `matches!(r, BoundedDistance::Exceeded { .. })` is clearer than `r.is_none()`.

## Why there is no implicit distance↔similarity conversion

StringCheese does not provide a global `From<NormalizedDistance> for NormalizedSimilarity` (or the reverse) and refuses to encode a rule such as `similarity = 1 - distance` in the type system.

The identity `similarity = 1 - distance` holds only when:

- both values were produced under the same normalization policy, and
- both values were produced on the *same input pair* by algorithms in the same family, and
- the algorithm actually defines its normalized similarity as the arithmetic complement of its normalized distance.

None of those three conditions is verifiable from the types. A `NormalizedDistance` carries no record of which policy or algorithm produced it (deliberately — that record lives at the pipeline level). Providing a lossy `From` impl would offer no guarantee that its output was meaningful for the input it received.

Where the conversion is meaningful for a specific algorithm, that algorithm exposes both a distance and a similarity method, or a single method that returns the correct type — the arithmetic complement is done at the algorithm level, once, where the preconditions are known.

## The trait hierarchy

Three traits, no more. Every algorithm implements exactly one of `DistanceMetric<S>` and `SimilarityMetric<S>` on its input sequence type `S`, and optionally `BoundedDistanceMetric<S>` if it supports cutoff-aware evaluation. Algorithms that return `Score<T>` implement neither: score-producing algorithms are exposed by their own inherent methods (alignment, probabilistic linkage) because their outputs are not interchangeable across scoring schemes and no useful generic infrastructure operates over them.

### `DistanceMetric<S>`

```rust
pub trait DistanceMetric<S: ?Sized> {
    type Output;
    fn distance(&self, left: &S, right: &S) -> Distance<Self::Output>;
    fn properties(&self) -> MetricProperties;
    fn class(&self) -> MetricClass;
}
```

- Configuration lives on `Self`. `distance` takes `&self`, so a single algorithm value can be shared across threads and calls; the configured substitution costs, thresholds, or preprocessing states do not change between calls.
- `properties` and `class` are queried at runtime because the same algorithm may present different guarantees under different configurations (e.g. weighted Levenshtein loses the triangle inequality if substitution costs violate it).

### `BoundedDistanceMetric<S>`

```rust
pub trait BoundedDistanceMetric<S: ?Sized>: DistanceMetric<S> {
    fn distance_within(
        &self,
        left: &S,
        right: &S,
        cutoff: Self::Output,
    ) -> BoundedDistance<Self::Output>;
}
```

- A super-trait of `DistanceMetric`. Every algorithm with cutoff support is also a plain distance metric; the reverse is not required.
- Kept as a separate trait because most callers do not need cutoffs, and because implementing cutoff-aware evaluation efficiently (banded matrices, early termination, Ukkonen-style bounds) is a substantially different implementation effort. Making it optional avoids forcing every algorithm to carry the machinery.

### `SimilarityMetric<S>`

```rust
pub trait SimilarityMetric<S: ?Sized> {
    type Output;
    fn similarity(&self, left: &S, right: &S) -> Similarity<Self::Output>;
    fn properties(&self) -> MetricProperties;
    fn class(&self) -> MetricClass;
}
```

- Parallel in shape to `DistanceMetric<S>` but does not subsume it. A similarity is *not* a distance with the sign flipped: bounded ranges differ, identity elements differ, monotonicity semantics differ.
- An algorithm that naturally produces a similarity implements this trait; if it *also* has a well-defined distance formulation, it implements `DistanceMetric<S>` separately with the arithmetic that the algorithm's own definition prescribes.

### Why the three are not collapsed

A single `Compare<S>` trait with a `Direction` associated type would technically cover all three, at the cost of erasing the very distinction the type system is built to preserve. The current split makes generic code state exactly what it accepts: `fn cluster<A: DistanceMetric<S>>(...)` will reject a similarity function, and vice versa. Cutoff-aware code can require `BoundedDistanceMetric<S>` without accidentally accepting a plain metric that will silently ignore the bound.

## `MetricProperties` and `MetricClass`

Two separate types cover a related-but-distinct pair of concerns.

### `MetricProperties`

A struct of five independent `bool` axioms:

```rust
pub struct MetricProperties {
    pub symmetric: bool,
    pub identity_of_indiscernibles: bool,
    pub triangle_inequality: bool,
    pub non_negative: bool,
    pub normalized: bool,
}
```

- **Symmetric.** `d(x, y) = d(y, x)`.
- **Identity of indiscernibles.** `d(x, y) = 0` iff `x = y` under the algorithm's chosen notion of equality.
- **Triangle inequality.** `d(x, z) ≤ d(x, y) + d(y, z)`.
- **Non-negative.** `d(x, y) ≥ 0`.
- **Normalized.** The algorithm's output is naturally bounded to `[0, 1]`.

Predefined constants — `METRIC`, `NORMALIZED_METRIC`, `PSEUDOMETRIC`, `SEMIMETRIC`, `QUASIMETRIC` — express the common combinations without requiring every algorithm to spell them out.

- **Copy + Eq + Hash.** Cheap to embed anywhere, comparable in test assertions, and usable as a map key. Properties are pure data.
- **Struct of `bool`s, not a bitset.** Each axiom corresponds to a definition in metric-space theory. A bitset would obscure that correspondence; the ergonomic cost of five `bool` fields is negligible, and the resulting code reads directly against the mathematical definitions.
- **Runtime value, not a marker trait.** Property claims depend on runtime configuration (weighted Levenshtein with per-cell costs may or may not satisfy the triangle inequality). Encoding them as trait bounds would force one impl per configuration or lose the distinction entirely.

### `MetricClass`

An `#[non_exhaustive]` enum of eight classifications:

```rust
pub enum MetricClass {
    Metric,
    Pseudometric,
    Semimetric,
    Quasimetric,
    Divergence,
    Similarity,
    Kernel,
    Score,
}
```

- The class is a summary label. `MetricProperties` is the underlying evidence.
- `Metric`, `Pseudometric`, `Semimetric`, `Quasimetric` map directly to the `MetricProperties` constants. `Divergence`, `Similarity`, `Kernel`, `Score` describe algorithms whose output does not fit the metric-space definitions and require separate reasoning downstream.
- **`#[non_exhaustive]`.** New classifications may need to be added as the algorithm coverage expands — probabilistic linkage may warrant its own class; kernel variants may split. Consumers must handle unknown classes with a wildcard arm.

### How the two relate

`class()` and `properties()` are both queryable on every algorithm trait. They should be internally consistent — an algorithm returning `MetricClass::Metric` must return `MetricProperties::METRIC` or `NORMALIZED_METRIC` — and StringCheese's property-based test suite exercises exactly the axioms the properties claim. A downstream consumer picks whichever is convenient:

- An index structure (BK-tree) queries `class` and refuses non-metrics.
- A validation harness queries `properties` to know which axioms to test.
- Explainability output shows both.

## Algorithm-variant registry

The registry pins down which specific definition of an algorithm an implementation follows. Many well-known names cover multiple incompatible definitions; the registry makes that explicit so a golden case for one variant cannot silently validate an implementation of another.

### `AlgorithmDescriptor`

```rust
pub struct AlgorithmDescriptor {
    pub family: AlgorithmFamily,
    pub variant: VariantId,
    pub version: DescriptorVersion,
    pub source: DefinitionSource,
}
```

Every algorithm implementation carries a `const AlgorithmDescriptor` describing it. The descriptor is constructible in `const` context so it can appear in no-alloc builds.

### `AlgorithmFamily`

- `#[non_exhaustive]` enum listing the broad families StringCheese covers — Levenshtein, Damerau–Levenshtein, Jaro, Soundex, Rabin-Karp, FastCDC, and so on. Two implementations in different families are not interchangeable.
- The enum is exhaustive for the algorithms StringCheese implements; being `#[non_exhaustive]` reserves the freedom to add new families without a major version bump.

### `VariantId`

```rust
pub struct VariantId(pub &'static str);
```

- A lowercase kebab-case slug identifying a variant within a family: `"unit-cost-unicode-scalars"`, `"restricted"`, `"unrestricted"`, `"prefix-limit-4"`, `"english-1918"`.
- **`&'static str`, not `String`.** The registry must work in `no_std` + no-`alloc` builds. Descriptors are compile-time constants; a heap-allocated `String` would forfeit both properties.
- **`&'static str`, not an enum.** Variants are open-ended. A phonetic algorithm may accumulate a variant per language over years; forcing every new variant to expand a central enum in `stringcheese-core` would either couple every algorithm crate to core version bumps or forbid variants outside core entirely. A slug is decentralized.
- The slug describes the *distinguishing* choice, not the family. `Levenshtein::unit-cost-unicode-scalars`, not `Levenshtein::levenshtein-standard`.

### `DescriptorVersion`

```rust
pub struct DescriptorVersion { pub major: u16, pub minor: u16, pub patch: u16 }
```

- Independent of the crate's semantic version. Identifies the version of the *variant's implementation*.
- **Major.** The algorithm's observable output changes for at least one input the previous version handled. Every golden case tied to the old version becomes suspect.
- **Minor.** The algorithm accepts strictly more inputs than before, or gains an optional feature that does not change output for previously valid inputs. Existing golden cases continue to hold.
- **Patch.** Implementation change with no observable output change (a bit-parallel rewrite of a scalar implementation, a SIMD path added behind a runtime dispatch). Existing golden cases continue to hold *and are expected to pass with the same bit-for-bit values*.

### `DefinitionSource`

An `#[non_exhaustive]` enum recording where the variant's semantics come from:

- `Paper { title, authors, year }` — the algorithm as defined in a specific publication.
- `Standard { name }` — as defined in a formal standard (e.g. `"Unicode 15.0"`).
- `ReferenceImplementation { name }` — as implemented by a widely used reference codebase.
- `IndependentlyDerived` — derived from the algorithm's mathematical definition without imitating another implementation.

The source informs discrepancy investigations: a disagreement with a `Paper` case is much stronger evidence of a StringCheese defect than a disagreement with a `ReferenceImplementation` case, which may carry the bug being compared against.

### Why golden cases reference descriptors, not names

A golden case that says "expected Damerau-Levenshtein distance = 3" is ambiguous — restricted-transposition and unrestricted-transposition variants disagree on many inputs. A golden case that says "expected `AlgorithmDescriptor { family: DamerauLevenshtein, variant: "restricted", version: 0.1.0, source: Paper { ... } }` distance = 3" pins the expectation exactly. See `GoldenCase<I, O>` in [`stringcheese-corpus`](../../crates/stringcheese-corpus/src/lib.rs); a case that references a different descriptor than the algorithm under test is a schema error, not a silent mismatch.

## `IndexableSequence`

```rust
pub trait IndexableSequence {
    type Item;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }
    fn get(&self, index: usize) -> Option<&Self::Item>;
}
```

The deliberately-narrow generic sequence contract used by algorithm kernels that do not care what the underlying representation is, only that it can be indexed in constant time.

- Implementations are required to be `O(1)` for both `len` and `get`. `&str` is not `O(1)`-indexable under any character-oriented interpretation (UTF-8 bytes, Unicode scalars, or graphemes each require decoding), and is intentionally excluded — see the next section.
- Provided impls: `[T]` and `[T; N]`. That is enough substrate for generic kernels; anything else materializes into a slice first.
- Specialized kernels for `&[u8]`, `&[char]`, grapheme slices, and token slices are provided by higher-level crates and usually outperform the fully generic path. The generic path is a correctness baseline, not the fast path.

### No `IndexableSequence` impl for `&str`

StringCheese refuses to make the representation choice for strings silently.

- Under UTF-8 bytes, `len("naïve") == 6`.
- Under Unicode scalar values, `len("naïve") == 5`.
- Under grapheme clusters, `len("naïve") == 5` — but `len("🇬🇧")` is `2` under scalars and `1` under graphemes.

An `impl IndexableSequence for &str` would have to pick one, and any choice would silently produce wrong answers for the other two interpretations. Callers convert the string to `&[u8]`, `&[char]`, or a grapheme slice explicitly; the [preprocessing pipeline](./preprocessing-pipeline.md) formalizes that conversion.

## `Workspace`

```rust
pub trait Workspace {
    fn ensure_capacity(&mut self, required: usize);
    fn capacity(&self) -> usize;
    fn shrink(&mut self) {}
}
```

- A caller-owned handle to per-algorithm scratch buffers, reusable across many comparisons.
- The unit of `capacity` is workspace-specific — cells for edit-distance rolling rows, rows for full-matrix alignment, bytes for tokenization buffers.
- The trait exposes only the common capacity-management operations. Concrete workspaces live alongside their algorithms and are used directly for the actual comparison call, not through the trait.

### Shape of a concrete workspace (Proposed — not yet implemented)

The first concrete workspace will land alongside Levenshtein. Illustrative sketch:

```rust
// Proposed — not yet implemented.
pub struct LevenshteinWorkspace {
    // Two rolling rows for the classical dynamic-programming variant.
    rows: alloc::vec::Vec<u32>,
    row_len: usize,
}

impl Workspace for LevenshteinWorkspace {
    fn ensure_capacity(&mut self, required: usize) {
        // required = max(len_left, len_right) + 1
        let need = 2usize.saturating_mul(required.saturating_add(1));
        if self.rows.capacity() < need {
            self.rows.reserve(need - self.rows.len());
        }
    }
    fn capacity(&self) -> usize { self.rows.capacity() / 2 }
    fn shrink(&mut self) { self.rows.shrink_to_fit(); }
}
```

The algorithm entry point then takes `&mut LevenshteinWorkspace` rather than `&mut dyn Workspace`, so the hot path stays monomorphic. The `Workspace` trait exists so generic pool / batch / arena infrastructure can manage many workspaces uniformly.

## `NormalizationPolicy`

```rust
#[non_exhaustive]
pub enum NormalizationPolicy {
    ByMaxLength,
    BySumLength,
    ByAlgorithmicMaximum,
    Custom,
}
```

- **A policy, not an applier.** This enum names the choice; the arithmetic that turns a raw `Distance<T>` into a `NormalizedDistance` lives on the algorithm that produced it, because different algorithms have different natural upper bounds.
- **`#[non_exhaustive]`.** Additional policies (per-character weights, corpus-wide maxima) may need to be added.
- **Why not a trait.** A trait would force every applier to be an object or a monomorphized parameter; an enum is inspectable, copyable, hashable, and trivially serializable — the properties that matter for explainability and for golden-case records.

## Anti-patterns explicitly rejected

- **Bare `f64` for similarity.** Loses the direction and the algorithm's identity. Rejected.
- **`String` names for algorithms.** Discourages precise variant identification and forfeits `no_std` compatibility. Rejected in favor of `AlgorithmDescriptor` with a `VariantId(&'static str)` slug.
- **Implicit `distance = 1 - similarity` conversion.** Only correct under a specific normalization policy on the same input pair with the same algorithm. Not verifiable at the type-system layer. Rejected.
- **Silent representation selection for `&str`.** Bytes, scalars, and graphemes give different answers. Rejected — callers commit to a representation, or the [preprocessing pipeline](./preprocessing-pipeline.md) commits on their behalf explicitly.
- **A single `Compare<S>` trait with a `Direction` associated type.** Erases the distance/similarity distinction at the trait bound. Rejected.
- **`Option<Distance<T>>` for cutoff-aware results.** Erases the cutoff. Rejected in favor of `BoundedDistance<T>`.
- **Behavior claimed by trait bound rather than by runtime descriptor.** Behavior often depends on runtime configuration. Rejected — properties are runtime values on the trait.

## API sketch: a fictional `PhoneticEditDistance`

The following illustrates the full pattern an algorithm implementation follows. It is intentionally fictional — no such algorithm ships with StringCheese — but every algorithm crate should be readable through this template.

```rust
use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion,
    Distance, DistanceMetric, MetricClass, MetricProperties, VariantId,
};

/// Edit distance in phoneme space: encode both sides to phoneme sequences,
/// then apply unit-cost Levenshtein over phonemes.
pub struct PhoneticEditDistance<E> {
    encoder: E,
}

impl<E> PhoneticEditDistance<E> {
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor::new(
        AlgorithmFamily::Levenshtein,
        VariantId("phoneme-unit-cost"),
        DescriptorVersion::new(0, 1, 0),
        DefinitionSource::IndependentlyDerived,
    );

    pub const fn new(encoder: E) -> Self { Self { encoder } }
}

// Encoder produces a phoneme slice; concrete `Phoneme` type lives in the
// phonetic subsystem — see phonetic-subsystem.md.
impl<E, S> DistanceMetric<S> for PhoneticEditDistance<E>
where
    S: ?Sized,
    E: PhoneticEncoder<Input = S>,
{
    type Output = u32;

    fn distance(&self, left: &S, right: &S) -> Distance<Self::Output> {
        let l = self.encoder.encode(left);
        let r = self.encoder.encode(right);
        // Actual work delegates to a phoneme-slice Levenshtein kernel.
        levenshtein_phonemes(&l, &r)
    }

    fn properties(&self) -> MetricProperties { MetricProperties::METRIC }
    fn class(&self) -> MetricClass { MetricClass::Metric }
}
```

Every algorithm implementation follows the same shape:

1. A `const DESCRIPTOR` that pins the variant.
2. A `DistanceMetric`, `BoundedDistanceMetric`, or `SimilarityMetric` implementation over an explicit sequence type.
3. `properties()` and `class()` returning values consistent with what the algorithm actually guarantees under the configuration on `Self`.
4. Optional companion workspace type for allocation-free repeated calls.
5. Golden cases in `stringcheese-corpus` keyed to `Self::DESCRIPTOR`.

Nothing about this template is specific to distance metrics — the equivalent for a similarity algorithm reads the same, with `SimilarityMetric<S>` and `Similarity<Self::Output>` in place of the distance types.
