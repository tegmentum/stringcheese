# Preprocessing Pipeline

Status: Design
Applies to: Comparand 0.1 and later
Related: [DESIGN.md](../DESIGN.md), [type-system.md](./type-system.md), [phonetic-subsystem.md](./phonetic-subsystem.md), [ngram-and-fingerprinting.md](./ngram-and-fingerprinting.md), [wasm-and-wit-interface.md](./wasm-and-wit-interface.md)

The design of Comparand's composable normalization, tokenization, and encoding pipeline — the layer that turns raw inputs into whatever representation an algorithm actually compares.

## Motivation

Comparison is almost never performed on raw strings.

- `"Zoë"` and `"Zoë"` are byte-different but grapheme-identical. Without NFC normalization, an exact-match test fails and every edit-distance algorithm returns a nonzero result.
- `"Smith"` and `"SMITH"` are byte-different but the same name. Without case folding, phonetic algorithms trained on lowercase input silently misbehave.
- `"O'Brien"` and `"O Brien"` differ in punctuation. Without a punctuation policy, Jaccard on token sets returns different values than intended.
- `"encyclopædia"` and `"encyclopaedia"` differ under NFKC but are the same under a stricter compatibility policy.

Scoring quality is dominated by preprocessing choices — often by an order of magnitude more than by algorithm choice. Most libraries leave preprocessing scattered across callsites, hidden in helper functions, or absent entirely; results are then not reproducible across users of the same library.

Comparand treats preprocessing as a first-class, inspectable, reusable subsystem.

## The pipeline model

A pipeline is an ordered sequence of transforms culminating in an algorithm invocation.

```
raw input
    -> normalize          (Unicode form)
    -> case-fold          (locale-aware or default)
    -> strip punctuation  (whitelist / blacklist / Unicode category)
    -> collapse whitespace
    -> tokenize           (representation transition: string -> tokens)
    -> phonetic encode    (representation transition: tokens -> phonemes/keys)
    -> algorithm invocation
```

Each stage is a small, orthogonal transform. The transform of a stage is deterministic and side-effect-free; two runs of the same pipeline on the same input produce identical output.

The pipeline itself is a value: it can be built once, inspected, serialized (for explainability), and reused across many comparisons.

## What "prepared" means

For workloads that compare one query against many candidates — record linkage, deduplication, top-k search, autocomplete — the query's preprocessing is performed thousands of times if the pipeline is naive. A *prepared* value performs preprocessing once and reuses the result.

```rust
// Proposed — not yet implemented.
let pipeline = Comparator::new()
    .normalize(Normalization::Nfkc)
    .case_fold()
    .metric(JaroWinkler::default());

let query = pipeline.prepare("Zoë O'Brien");     // preprocess once
for candidate in &candidates {
    let sim = pipeline.compare_prepared(&query, candidate);
    // candidate is preprocessed on each call; query is not
}
```

A prepared value carries whatever downstream representation the algorithm consumes: a `Vec<char>`, a `Vec<Grapheme>`, a `TokenSlice`, a phonetic-key pair, an n-gram set. It also carries any algorithm-specific precomputation (bit vectors for bit-parallel edit distance, sorted gram sets for Jaccard) so the hot path is arithmetic, not allocation.

For the *many candidates* side of the same workload, the pipeline exposes a batched preparation call that reuses a [`Workspace`](./type-system.md#workspace) across the batch:

```rust
// Proposed — not yet implemented.
let mut ws = pipeline.workspace();
let prepared_candidates: Vec<_> = candidates
    .iter()
    .map(|c| pipeline.prepare_with(c, &mut ws))
    .collect();
```

## Order-sensitivity

Preprocessing stages do not commute in the general case.

- **NFKC then case-fold ≠ case-fold then NFKC.** Compatibility decomposition may introduce characters that case-fold to different values than the pre-decomposition input.
- **Case-fold then strip diacritics ≠ strip diacritics then case-fold.** Some scripts case-fold differently depending on which combining marks remain.
- **Tokenize then case-fold ≠ case-fold then tokenize.** Tokenizers whose boundary rules are sensitive to case (rare for Latin, occasional for others) produce different token streams.
- **Case-fold then phonetic-encode ≠ phonetic-encode then case-fold.** Phonetic encoders defined over lowercase input produce meaningless output on uppercase input; encoding first then folding a key does not save you.

The pipeline is *inspectable* precisely so callers know what was actually computed. `pipeline.describe()` returns an ordered list of stage descriptors; the [explainability output](#explainability-hooks) includes it verbatim.

The pipeline does not silently reorder stages for performance. If reordering is safe (two commutative stages), the caller states it by writing them in the more efficient order. Comparand does not second-guess.

## Representation transitions

A pipeline picks the representation the algorithm consumes. Once a stage commits, downstream stages consume that representation.

| Stage type              | Input representation      | Output representation      |
|-------------------------|---------------------------|----------------------------|
| Unicode normalization   | `&str` (UTF-8)            | `&str` (UTF-8)             |
| Case folding            | `&str` (UTF-8)            | `&str` (UTF-8)             |
| Punctuation policy      | `&str` (UTF-8)            | `&str` (UTF-8)             |
| Whitespace collapse     | `&str` (UTF-8)            | `&str` (UTF-8)             |
| Scalar decomposition    | `&str` (UTF-8)            | `&[char]`                  |
| Grapheme segmentation   | `&str` (UTF-8)            | `&[Grapheme]`              |
| Tokenization            | `&str` (UTF-8)            | `&[Token]`                 |
| Phonetic encoding       | `&str` or `&[Token]`      | `&[PhoneticKey]` or `&[Phoneme]` |
| N-gram generation       | any of the above          | `Set<Gram>` / `MultiSet<Gram>` / `WeightedVec<Gram>` |

Representation transitions are the *only* places where the sequence type changes. Stages within a representation compose freely; transitions are checkpoints. Because the [type system](./type-system.md#no-indexablesequence-impl-for-str) refuses to make representation choices silently, every transition is explicit in the builder.

The pipeline chooses the *narrowest* transition compatible with the target algorithm:

- Levenshtein over Unicode scalars uses the scalar-decomposition transition, not grapheme segmentation, unless the caller explicitly asks for graphemes.
- Jaccard over token sets uses the tokenization transition; if the caller wants character grams instead, they pick the character-gram transition.

The choice is inspectable in `pipeline.describe()`.

## Composability with the type system

A pipeline is, at its coarsest, a function `(&Input, &Input) -> Distance<T>` (or `Similarity<T>`, or `Score<T>`, matching the algorithm's return shape). The composition respects the [result-type hierarchy](./type-system.md#the-result-type-hierarchy) — a pipeline that terminates in a distance algorithm returns a `Distance<T>`; a pipeline that terminates in a similarity algorithm returns a `Similarity<T>`; a pipeline that terminates in a normalized-distance-producing wrapper returns a `NormalizedDistance`.

A *reusable* pipeline holds the raw transform sequence plus, optionally, prepared state alongside it:

```rust
// Proposed — not yet implemented.
pub struct Comparator<S, T, A> {
    stages: Vec<Stage>,
    algorithm: A,
    _repr: PhantomData<(S, T)>,
}

impl<S, T, A: DistanceMetric<T>> Comparator<S, T, A> {
    pub fn compare(&self, left: &S, right: &S) -> Distance<A::Output> { ... }
    pub fn prepare(&self, input: &S) -> Prepared<T> { ... }
    pub fn compare_prepared(&self, l: &Prepared<T>, r: &Prepared<T>) -> Distance<A::Output> { ... }
}
```

The type parameters `S` (raw input) and `T` (representation the algorithm consumes) are separate: the builder threads them through as stages are added, and mismatches — a phonetic-encoding stage applied to `&[u8]` bytes, a byte-tokenizer feeding a grapheme-Levenshtein — become type errors at build time.

The [`MetricProperties`](./type-system.md#metricproperties) and [`MetricClass`](./type-system.md#metricclass) of the pipeline are those of the underlying algorithm. Preprocessing does not confer or destroy mathematical properties: if the algorithm is a metric, the composed pipeline is also a metric *over the preprocessed representation* — a subtle but important distinction, discussed under [Explainability hooks](#explainability-hooks).

## Interaction with workspaces

A pipeline owns any workspace the terminal algorithm requires. For repeated calls the caller obtains a workspace once and passes it back:

```rust
// Proposed — not yet implemented.
let pipeline = Comparator::new().case_fold().metric(Levenshtein::default());
let mut ws = pipeline.workspace();
for (a, b) in pairs {
    let d = pipeline.compare_with(a, b, &mut ws);
}
```

The pipeline's workspace is a distinct type from the algorithm's own workspace, though it typically contains one: pipeline stages that require intermediate buffers (tokenization, phonetic encoding, n-gram generation) each contribute their share, and the pipeline exposes a single handle.

For one-query-many-candidates workloads the pipeline exposes a *split* API where the query and the candidates use separate workspace regions to avoid interference:

```rust
// Proposed — not yet implemented.
let mut query_ws = pipeline.query_workspace();
let mut cand_ws = pipeline.candidate_workspace();
let query = pipeline.prepare_with(&q, &mut query_ws);
for c in candidates {
    let cand = pipeline.prepare_with(c, &mut cand_ws);
    let sim = pipeline.compare_prepared(&query, &cand);
}
```

## `Comparator` builder — API sketch

```rust
// Proposed — not yet implemented. All names subject to change; the shape is
// what matters for this design.

pub struct Comparator<Raw, Prepared, A> { /* ... */ }

impl Comparator<&str, &str, ()> {
    pub fn new() -> Self { /* ... */ }
}

impl<A> Comparator<&str, &str, A> {
    // Same-representation stages
    pub fn normalize(self, form: Normalization) -> Self { /* ... */ }
    pub fn case_fold(self) -> Self { /* ... */ }
    pub fn case_fold_with(self, locale: Locale) -> Self { /* ... */ }
    pub fn strip_punctuation(self, policy: PunctuationPolicy) -> Self { /* ... */ }
    pub fn collapse_whitespace(self) -> Self { /* ... */ }

    // Representation transitions — each returns a differently-parameterized
    // Comparator so downstream stage sets are gated by the type system.
    pub fn scalars(self) -> Comparator<&str, [char], A> { /* ... */ }
    pub fn graphemes(self) -> Comparator<&str, [Grapheme], A> { /* ... */ }
    pub fn tokenize(self, tokenizer: Tokenizer) -> Comparator<&str, [Token], A> { /* ... */ }
    pub fn encode_phonetic<E: PhoneticEncoder>(self, e: E)
        -> Comparator<&str, [PhoneticKey], A> { /* ... */ }
}

impl<Raw, Prep, A> Comparator<Raw, Prep, A> {
    // Attach an algorithm. The algorithm's input type must match Prep.
    pub fn metric<B: DistanceMetric<Prep>>(self, algo: B) -> Comparator<Raw, Prep, B> { /* ... */ }
    pub fn similarity<B: SimilarityMetric<Prep>>(self, algo: B)
        -> Comparator<Raw, Prep, B> { /* ... */ }
}

impl<Raw, Prep, A: DistanceMetric<Prep>> Comparator<Raw, Prep, A> {
    pub fn compare(&self, left: &Raw, right: &Raw) -> Distance<A::Output> { /* ... */ }
    pub fn prepare(&self, input: &Raw) -> Prepared<Prep> { /* ... */ }
    pub fn compare_prepared(&self, l: &Prepared<Prep>, r: &Prepared<Prep>)
        -> Distance<A::Output> { /* ... */ }

    pub fn describe(&self) -> PipelineDescription { /* ... */ }
    pub fn workspace(&self) -> PipelineWorkspace { /* ... */ }
}
```

### What `.compare(a, b)` does under the hood

For `Comparator::new().normalize(Nfkc).case_fold().collapse_whitespace().metric(JaroWinkler)`:

1. Apply NFKC to `a` and `b`, producing owned normalized strings (or borrowed if already normal).
2. Case-fold both, producing owned folded strings.
3. Collapse runs of Unicode-whitespace, producing owned strings.
4. Convert to the representation Jaro–Winkler consumes (scalar slices for the default variant).
5. Call `JaroWinkler::similarity(&scalars_a, &scalars_b)`.
6. Return the resulting `Similarity<f64>`.

Each stage's ownership is minimized: a stage that would be a no-op on a particular input (already NFKC, already lowercase) returns a borrow into the previous stage's output. Allocation happens only when a stage actually rewrites its input. The prepared path skips steps 1–4 for the prepared side entirely — the prepared value already carries the scalar slice.

## Explainability hooks

The pipeline should be able to describe itself in a form suitable for the explainability output shape sketched in [DESIGN.md § Explainability](../DESIGN.md).

```rust
// Proposed — not yet implemented.
pub struct PipelineDescription {
    pub stages: Vec<StageDescription>,
    pub representation: Representation,
    pub algorithm: AlgorithmDescriptor,
    pub properties: MetricProperties,
    pub class: MetricClass,
}
```

A rendered pipeline description looks like:

```
Normalization:   NFKC
Case-folding:    default (Unicode simple case fold)
Whitespace:      collapse runs (Unicode White_Space)
Representation:  Unicode scalar values
Algorithm:       Jaro-Winkler (variant "prefix-limit-4", version 0.1.0)
Class:           Similarity
Properties:      symmetric, non-negative, normalized
Result:          0.94
```

The algorithm's [`AlgorithmDescriptor`](./type-system.md#algorithm-variant-registry) and its `MetricProperties` come from the algorithm itself; the pipeline contributes only the preprocessing prefix. Consumers that need a stable identifier for the entire pipeline (for logging, corpus tagging, cache keys) can hash the ordered stage descriptions plus the descriptor.

**Note on properties under preprocessing.** A metric over raw strings need not remain a metric after case-folding *considered as a function of raw strings*: two byte-distinct strings that fold to the same value now have distance zero, breaking identity of indiscernibles. The composed pipeline is a metric *over the preprocessed representation* — which is exactly the domain the algorithm ran on. The `properties()` reported on the pipeline description are those of the algorithm as-run, and explanations should be read with the representation stage in mind.

## Cross-references

- The `NormalizationPolicy` enum defined in [type-system.md § NormalizationPolicy](./type-system.md#normalizationpolicy) is a *different concept* from Unicode normalization. `NormalizationPolicy` names the choice of how a raw distance is scaled into `[0, 1]`; `Normalization::Nfkc` (etc.) inside a pipeline names the choice of Unicode normal form applied to a string. The two never collide.
- Phonetic-encoding pipeline stages are described in detail in [phonetic-subsystem.md](./phonetic-subsystem.md).
- N-gram-generation pipeline stages, including the padding and multiplicity policies, are described in [ngram-and-fingerprinting.md](./ngram-and-fingerprinting.md).
- The pipeline model as it crosses the Component Model boundary — where generic sequence types cannot cross — is discussed in [wasm-and-wit-interface.md](./wasm-and-wit-interface.md).
