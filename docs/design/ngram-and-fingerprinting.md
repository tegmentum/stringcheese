# N-Gram and Fingerprinting Subsystem

Status: Design
Applies to: StringCheese 0.1 and later
Related: [DESIGN.md](../DESIGN.md), [type-system.md](./type-system.md), [preprocessing-pipeline.md](./preprocessing-pipeline.md), [phonetic-subsystem.md](./phonetic-subsystem.md), [wasm-and-wit-interface.md](./wasm-and-wit-interface.md)

The design of StringCheese's n-gram generation and fingerprinting subsystems — the representation layer beneath set similarity, MinHash, LSH, n-gram inverted indexes, substring search, and content-defined chunking.

## N-grams are a representation layer

The common way to think about n-grams is: an input to Jaccard, Dice, and cosine similarity. That framing is too narrow.

An n-gram generator produces a *representation* of the input. The same gram set can drive:

- **Set similarity.** Jaccard, Dice, overlap coefficient, containment.
- **Weighted vector similarity.** Cosine similarity, weighted Jaccard.
- **MinHash and LSH.** Approximate set similarity at scale.
- **N-gram inverted indexes.** Candidate generation for high-recall filtering before an expensive edit-distance rescore.
- **Substring search preprocessing.** N-gram signatures gate Rabin-Karp scans over large corpora.
- **Explainability.** The intersection of two inputs' gram sets is a legible "why did these match" artifact.

StringCheese exposes generation and consumption as separate concerns: an `NGramGenerator` produces a gram sequence or set; downstream consumers (similarity kernels, MinHash sketches, inverted indexes) pull from that representation.

## Gram generation

An n-gram generator is parameterized by five orthogonal choices.

### 1. Symbol type — what a gram is over

- **Byte grams.** `[u8]` of length `n`. Fast, allocation-cheap, but insensitive to Unicode structure — `"naïve"` and `"naïve"` (NFC vs NFD) have different byte-gram sets.
- **Character grams.** `[char]` of length `n`, over Unicode scalar values. Unicode-correct at scalar granularity; still splits combining sequences.
- **Grapheme grams.** `[Grapheme]` of length `n`. Correct at grapheme granularity — a combined `é` is one grapheme regardless of composition.
- **Token grams.** `[Token]` of length `n`, over the output of a tokenizer. The classic "shingle" for document similarity.
- **Phoneme grams.** `[Phoneme]` of length `n`. Enables phoneme-aware n-gram matching where similarity is computed over phoneme sequences derived from the [phonetic subsystem](./phonetic-subsystem.md); useful for multilingual name matching where surface forms diverge but pronunciations align.

Symbol type is decided by the [preprocessing pipeline's representation transitions](./preprocessing-pipeline.md#representation-transitions).

### 2. `N` — the gram length

- **Fixed `n`.** The common case: bigrams, trigrams, 4-grams.
- **Variable `n`.** A generator emits grams of multiple lengths, useful for hybrid similarity ("match on any 2..=4-gram").
- **Skip-grams.** A generator emits grams that omit one or more positions, capturing associations across intervening symbols.

### 3. Padding policy — how boundaries are handled

- **No padding.** Grams cover only positions where a full `n` window fits. `"kit"` under bigrams yields `{"ki", "it"}`.
- **Boundary markers.** Prepend and append a sentinel symbol. `"kit"` becomes `"$kit$"` under bigrams: `{"$k", "ki", "it", "t$"}`. Boundary markers preserve prefix and suffix signal; `"kit"` and `"skit"` share only `"ki"`, `"it"` in the unpadded scheme but agree on `"ki"`, `"it"`, `"t$"` in the padded one.
- **Character padding.** Boundary marker is a caller-chosen symbol (Unicode `U+0020`, ASCII `$`, out-of-band private-use character); each has trade-offs for false collisions with input content.

### 4. Multiplicity — set vs multiset vs weighted

- **Set.** Duplicate grams collapse. `"banana"` under bigrams yields `{"ba", "an", "na"}` regardless of the doubled `"an"` and `"na"`.
- **MultiSet.** Duplicates preserved as counts. `"banana"` yields `{"ba":1, "an":2, "na":2}`. Jaccard on multisets is a distinct measure from Jaccard on sets; both are useful, and neither is silently substituted for the other.
- **Weighted.** Grams carry caller-supplied weights (TF-IDF, phoneme-feature importance). The representation is a sparse `WeightedVec<Gram>`, and downstream similarity is weighted cosine or weighted Jaccard.

### 5. Ordering — set vs sequence

The same grams may need to be delivered as an unordered set (for Jaccard) or an ordered sequence (for streaming or for consumers that care about position). The generator's output type reflects the caller's choice.

## Representations — the Rust types

```rust
// Proposed — not yet implemented.

/// Deduplicated gram set. Backed by a hash set or a sorted `Vec`.
pub struct GramSet<G> { /* ... */ }

/// Multiset of grams with counts.
pub struct GramMultiSet<G> { /* ... */ }

/// Weighted sparse vector of grams.
pub struct WeightedGrams<G> {
    grams: Vec<G>,
    weights: Vec<f64>,
}
```

Every gram-consuming algorithm accepts one of the three explicitly. There is no automatic promotion between representations, and no default choice.

- Jaccard over a `GramSet<G>` and Jaccard over a `GramMultiSet<G>` are two different algorithms with different `AlgorithmDescriptor`s. They live under `AlgorithmFamily::Jaccard` with distinct `VariantId`s (`"set"` and `"multiset"`).
- Cosine similarity requires `WeightedGrams<G>`; the unweighted variant (with all weights = 1.0) is a legal special case but is still typed as `WeightedGrams<G>`.

## Padding policies — concrete rules

- **Delimiter symbol.** For byte and character grams, the pipeline uses a Unicode private-use codepoint by default (`U+F0FF`) so it cannot collide with legitimate input. For token grams the sentinel is a distinguished `Token::BeginOfSequence` / `Token::EndOfSequence`.
- **Symmetry.** The prefix and suffix markers are distinguishable by default — using the same marker at both ends creates artificial palindromic collisions.
- **Weighting under padding.** Boundary grams may be down-weighted when running weighted cosine; the pipeline exposes this as an opt-in configuration on the generator.

The choice of padding policy is inspectable in the generator's descriptor and in the pipeline description.

## Rabin fingerprints and rolling hashes as reusable primitives

Rabin fingerprints and rolling hashes appear in at least three places in StringCheese:

- **N-gram generation.** Hashing each gram to an integer for fast set operations, MinHash bucketing, and index-lookup keys.
- **Substring search.** Rabin-Karp scans a text for a pattern by maintaining a rolling hash of the current window.
- **Content-defined chunking (CDC).** FastCDC, Rabin CDC, and Gear all identify chunk boundaries using a rolling hash of the byte stream.

The temptation is to re-implement a rolling hash in each subsystem. The subsystems require the same primitive with slightly different consumers, and every reimplementation is an opportunity for a subtle rounding or masking discrepancy that then makes cross-subsystem interoperability harder than it should be.

The rule: never re-implement a rolling hash. The rolling-hash implementations live in `stringcheese-cdc` (their most performance-critical consumer) and are re-exported through `stringcheese-core`'s public API surface — or, more likely, through a thin `stringcheese-hash` re-export crate — for other subsystems to depend on.

The hash trait is small:

```rust
// Proposed — not yet implemented.
pub trait RollingHash: Clone {
    type Output: Copy + Eq + Hash;

    /// Initializes an empty hash state.
    fn new() -> Self;

    /// Ingests one symbol; symbol type is `u8` for byte streams.
    fn push(&mut self, symbol: u8);

    /// Removes a symbol from the trailing edge; must be called with the
    /// symbol that entered the window `window_size` symbols ago.
    fn pop(&mut self, symbol: u8, window_size: usize);

    /// Returns the current hash.
    fn digest(&self) -> Self::Output;
}
```

Each implementation is a small stateful struct: `RabinHash`, `PolynomialHash`, `Buzhash`, `GearHash`. Downstream consumers pick the one whose numerical properties suit their workload.

### Deterministic hashing for n-grams

N-gram generation uses a *deterministic* hash — a rolling hash configured to produce identical output on identical input, across native and Wasm targets and across debug and release builds. This is what makes MinHash sketches computed on one machine compatible with sketches computed on another. Wall-clock-seeded or ASLR-influenced hashes are excluded from the gram-generation path; hashes for user-facing set operations use only the deterministic-hash variants.

## Type-system interaction

The n-gram subsystem plugs into the [result-type hierarchy](./type-system.md#the-result-type-hierarchy) at well-defined points:

- **Jaccard over `GramSet<G>`.** Returns `NormalizedSimilarity` (naturally bounded to `[0, 1]`); `MetricClass::Metric` (Jaccard distance `1 - similarity` is a true metric).
- **Dice over `GramSet<G>`.** Returns `NormalizedSimilarity`; `MetricClass::Similarity` (Dice similarity is not a metric; the corresponding "distance" fails the triangle inequality).
- **Overlap coefficient over `GramSet<G>`.** Returns `NormalizedSimilarity`; `MetricClass::Similarity`.
- **Containment similarity over `GramSet<G>`.** Returns `NormalizedSimilarity`; `MetricClass::Similarity`; asymmetric — order of arguments matters.
- **Weighted cosine over `WeightedGrams<G>`.** Returns `Similarity<f64>` (not `NormalizedSimilarity`, because cosine may return values in `[-1, 1]` for signed weights); `MetricClass::Similarity`.
- **Weighted Jaccard over `WeightedGrams<G>`.** Returns `NormalizedSimilarity`; `MetricClass::Metric` (weighted Jaccard distance is a metric when weights are non-negative).

Each n-gram similarity implementation carries its own [`AlgorithmDescriptor`](./type-system.md#algorithm-variant-registry), where the `VariantId` slug captures the representation (`"set"`, `"multiset"`, `"weighted-cosine"`) and any padding policy (`"boundary-marker-pue"`, `"unpadded"`).

## Streaming

Gram generation should support lazy iteration so callers can early-exit on a threshold without paying for the full gram set.

```rust
// Proposed — not yet implemented.
pub trait NGramStream<G> {
    fn next_gram(&mut self) -> Option<G>;
}

// A cutoff-aware Jaccard consumer:
pub fn jaccard_similarity_at_least(
    left: impl NGramStream<G>,
    right: &GramSet<G>,
    threshold: NormalizedSimilarity,
) -> bool { /* ... */ }
```

The consumer maintains a partial intersection and union while streaming; as soon as the maximum achievable similarity (assuming all remaining grams match) falls below the threshold, iteration terminates. This is the n-gram analog of [`BoundedDistanceMetric`](./type-system.md#boundeddistancemetrics) — an early-terminating cutoff-aware variant of the fully-realized similarity call.

Streaming generation also composes with the query-then-corpus pattern: the query's gram set is materialized once; each candidate is streamed lazily and dropped as soon as the threshold decision is made.

## Fingerprint families

Four rolling-hash families ship or are planned. Each has a different profile.

- **Rabin fingerprints.** Polynomial hash over `GF(2)`, defined by an irreducible polynomial. Strong theoretical collision properties; the classical choice for CDC and for de-duplication. Slightly more expensive than the alternatives.
- **Polynomial rolling hash.** Horner-form polynomial over a prime field. Fast, well-understood, but adversarial inputs can be constructed that force collisions — fine for cryptographically-uninteresting workloads (n-gram sets, substring search) and unsuitable for adversary-controlled deduplication.
- **Buzhash.** Byte-indexed XOR-and-rotate hash. Very fast; excellent avalanche properties for typical inputs. The default choice for high-throughput CDC where the input distribution is not adversarial.
- **Gear hash.** Byte-indexed table with a single multiplication per symbol. The fastest rolling hash on modern superscalar CPUs; the primitive under FastCDC.

### When each is preferred

| Workload                                  | Preferred          |
|-------------------------------------------|--------------------|
| CDC over untrusted input, correctness-critical | Rabin          |
| CDC over trusted input, throughput-critical    | Gear (FastCDC) or Buzhash |
| Substring search (Rabin-Karp)             | Polynomial or Rabin |
| N-gram set hashing                        | Polynomial (deterministic seed) |
| MinHash bucketing                         | Polynomial (deterministic seed, distinct seed per hash function) |
| Fingerprint deduplication (git-like)      | Rabin              |

The choice is exposed as an algorithm variant, not a runtime configuration on a single opaque type — each family is its own struct with its own `AlgorithmDescriptor`.

## API sketch

```rust
// Proposed — not yet implemented.

use stringcheese_core::AlgorithmDescriptor;

/// Represents a source of grams over an input.
pub trait NGramGenerator {
    /// The symbol type inside each gram (`u8`, `char`, `Grapheme`, `Token`, `Phoneme`).
    type Symbol;
    /// The gram type (usually a small fixed-size or heap-allocated slice of `Symbol`).
    type Gram;
    /// The input the generator consumes.
    type Input: ?Sized;

    fn descriptor(&self) -> AlgorithmDescriptor;

    /// Materializes the full gram set.
    fn set(&self, input: &Self::Input) -> GramSet<Self::Gram>;

    /// Materializes the full gram multiset.
    fn multiset(&self, input: &Self::Input) -> GramMultiSet<Self::Gram>;

    /// Streams grams lazily.
    fn stream<'a>(&'a self, input: &'a Self::Input)
        -> impl NGramStream<Self::Gram> + 'a;
}

/// Configuration for a concrete generator.
pub struct NGramConfig {
    pub n: NLength,
    pub padding: Padding,
    pub deterministic_seed: u64,
}

pub enum NLength {
    Fixed(usize),
    Range { min: usize, max: usize },
    Skip { total_span: usize, chosen: usize }, // skip-grams
}

pub enum Padding {
    None,
    BoundaryMarker { begin: char, end: char },
    Token { begin: Token, end: Token },
}

// Concrete generators — one per symbol type.
pub struct CharacterGrams { pub config: NGramConfig }
pub struct ByteGrams { pub config: NGramConfig }
pub struct GraphemeGrams { pub config: NGramConfig }
pub struct TokenGrams<T: Tokenizer> { pub config: NGramConfig, pub tokenizer: T }
pub struct PhonemeGrams<E: PhoneticEncoder> {
    pub config: NGramConfig,
    pub encoder: E,
}
```

Each concrete generator implements `NGramGenerator` with a `Symbol` that reflects its name and a `Gram` type that is either a fixed-size array (`[Symbol; N]` for fixed-`n` generators) or a heap slice (`Box<[Symbol]>` for variable-`n` generators). The choice is made at generator construction, not per-call.

Each generator's descriptor pins the family (`AlgorithmFamily` gets one variant per generator family — this is a planned addition to the enum) and the variant, including `n`, padding, and deterministic seed. Golden cases for gram generation reference the descriptor, so an "n=3, unpadded, byte" golden case cannot be run against an "n=3, padded, character" generator by accident.

## Interaction with `Workspace`

Gram generators are candidates for [`Workspace`](./type-system.md#workspace) reuse.

- The `set()` and `multiset()` calls need a hash-table backing store; reusing it across many inputs (as in "generate grams for every row of a table") avoids repeated allocation.
- The streaming path needs a small ring buffer to hold the current window; reuse is cheap and worthwhile.
- MinHash sketch computation needs an array of `k` accumulators; reuse across inputs in a batch is essential for performance.

Concrete workspace types will live alongside their generators (`CharacterGramWorkspace`, `MinHashWorkspace`), each implementing `Workspace` for the capacity-management operations while exposing their specialized layout for the hot path.

## Cross-references

- The result types the subsystem returns (`NormalizedSimilarity`, `Similarity<f64>`) and the classifications the subsystem's algorithms carry are defined in [type-system.md](./type-system.md).
- The pipeline stages that transition to n-gram representations — including padding and multiplicity configuration — are described in [preprocessing-pipeline.md § Representation transitions](./preprocessing-pipeline.md#representation-transitions).
- Phoneme grams depend on the encoder trait defined in [phonetic-subsystem.md](./phonetic-subsystem.md); phoneme-space n-gram similarity is a research direction that combines this subsystem with the phonetic subsystem's phoneme-level roadmap.
- The rolling hashes shared with CDC and search live in `stringcheese-cdc`; the WebAssembly footprint of pulling in the full hash suite is discussed in [wasm-and-wit-interface.md § Feature-gate strategy](./wasm-and-wit-interface.md#feature-gate-strategy).
