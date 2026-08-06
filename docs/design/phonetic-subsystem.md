# Phonetic Subsystem

Status: Design
Applies to: StringCheese 0.1 and later
Related: [DESIGN.md](../DESIGN.md), [type-system.md](./type-system.md), [preprocessing-pipeline.md](./preprocessing-pipeline.md), [ngram-and-fingerprinting.md](./ngram-and-fingerprinting.md), [wasm-and-wit-interface.md](./wasm-and-wit-interface.md)

The design of StringCheese's phonetic matching subsystem — encoders, key comparison, language and script metadata, and the roadmap to phoneme-level comparison.

## Phonetics is first-class

Phonetic matching is not just another comparison function. It is a distinct subsystem with:

- Its own representation stack (encoded keys, and eventually phoneme sequences).
- Its own metadata requirements (language, script, and region).
- Its own composition contract (encoder + comparison, not a single black-box function).
- Its own modularity story (language-specific rule tables shipped as separate crates for feature-driven inclusion).

Treating phonetics as a facet of a generic string-comparison API loses these dimensions. StringCheese puts the subsystem alongside distance and similarity as a peer.

## Encoding vs comparison

Every phonetic match is two steps. Collapsing them obscures both.

1. **Encoding.** A `PhoneticEncoder` transforms input into a phonetic representation — historically a short string ("BRWN" for "Brown"), long-term a phoneme sequence.
2. **Comparison.** A comparator decides whether two encodings match. Historically this is string equality on the encoded key. Sometimes it is edit distance on the encoded key. Long-term, for phoneme representations, it is edit distance in phoneme space.

The two are separately identifiable.

- **Different encoders can share a comparator.** Soundex-encoded and NYSIIS-encoded keys can both be compared with string equality.
- **Different comparators can share an encoder.** Double-Metaphone keys can be compared with equality (fast, coarse) or with Levenshtein (slower, finer, tolerant of a single dropped consonant).
- **Explainability.** The explanation for a match names both stages — `"Metaphone (english-1) exact-key equality"` or `"Metaphone (english-1) key Levenshtein <= 1"`.
- **Correctness auditing.** A discrepancy against another library can be localized: same encoding? then the comparator disagrees. Different encoding? then the encoder disagrees. A single "phonetic-match" function forfeits both signals.

## Language, script, and region as first-class metadata

Soundex was designed for early-20th-century English surnames. Applying it to Chinese names in Han characters does not "not work well" — it produces meaningless output, because the algorithm's input model (ASCII letters representing English consonant classes) is not what it received.

The API surface makes this hard to forget.

- Every encoder declares the languages, scripts, and regions its rules were designed for as part of its type or descriptor.
- The [`AlgorithmDescriptor`](./type-system.md#algorithm-variant-registry) `VariantId` slug encodes the target: `"english-1918"`, `"english-refined"`, `"french-r1"`, `"german-cologne"`, `"jewish-daitch-mokotoff"`.
- A `PhoneticEncoder` implementation carries a `const APPLICABILITY: Applicability` value describing what it was designed for. Building a pipeline that feeds French-language input to an English-only encoder is not automatically prevented (the type system cannot know a string's language), but the mismatch is inspectable and can be surfaced in the explainability output.
- Where the language is known at the call site (e.g. from a database column), the pipeline builder should route to the appropriate encoder rather than picking one arbitrarily.

```rust
// Proposed — not yet implemented.
pub struct Applicability {
    pub languages: &'static [LanguageTag],   // e.g. &["en"]
    pub scripts: &'static [ScriptTag],        // e.g. &["Latn"]
    pub regions: &'static [RegionTag],        // e.g. &["US", "GB"]
    pub notes: &'static str,
}
```

Language, script, and region are separate: German written in Latin script and German written in Fraktur are the same language on paper but a different rendering; Serbian in Latin and Serbian in Cyrillic are the same language in two scripts. An algorithm designed for one script cannot be assumed to handle another without transliteration first.

## Primary vs secondary keys

Some phonetic algorithms produce more than one code per input.

- **Double Metaphone** yields a primary code and (sometimes) a secondary code, allowing for common regional pronunciation variance ("Schmidt" as primary "XMT" and secondary "SMT").
- **Beider–Morse** enumerates a set of candidate codes reflecting the many possible pronunciations of a name across languages.

The subsystem models multi-key encoders explicitly.

```rust
// Proposed — not yet implemented.
pub struct PhoneticCodes {
    pub primary: PhoneticKey,
    pub secondary: Option<PhoneticKey>,
    // Beider–Morse and similar high-cardinality encoders live in `extra`.
    pub extra: SmallVec<[PhoneticKey; 2]>,
}
```

Comparison logic considers all pairs. Two inputs match if *any* of their codes match under the comparator. The default `PhoneticMatcher::matches(a, b)` returns `true` iff at least one primary-primary, primary-secondary, secondary-primary, or secondary-secondary pair equals under the comparator; stricter modes ("primary only", "all must match") are opt-in configurations on the matcher.

Under a similarity comparator (not just equality), the aggregate similarity is the maximum over the code-pair grid — the closest pronunciation match wins. This is a monotone reduction that preserves `MetricClass::Similarity` semantics; it is not a metric even when the underlying key-comparator is one.

## Data-driven vs code-driven variants

Phonetic algorithms come in two shapes.

- **Code-driven.** Soundex, NYSIIS, and Match Rating are small procedures — a handful of transformation rules expressible as short match-and-replace loops directly in Rust. They live inside their algorithm crate with no external data.
- **Data-driven.** Daitch–Mokotoff and Beider–Morse are tables of hundreds to thousands of rules with contexts and language variants. Encoding them as inline Rust would be unmaintainable; the tables live as compile-time-embedded static data (or, feature-gated, as runtime-loaded data for very large tables) and the encoder is a small interpreter over the table.

The subsystem accommodates both without a common encoder machinery: `PhoneticEncoder` is a trait, and code-driven and data-driven implementations meet it however they need to. Where data tables are large enough to warrant it, the tables live in their own crate (`stringcheese-phonetic-beidermorse-data`) and are pulled in via a Cargo feature, so a minimal Wasm build does not carry them.

## The algorithm-variant registry for phonetic variants

Phonetic algorithms are a paradigm case for the [algorithm-variant registry](./type-system.md#algorithm-variant-registry). "Soundex" alone is at least three algorithms:

- The 1918 American original.
- Odell–Russell 1922, with slightly different consonant classes.
- "Refined Soundex", which extends the alphabet and drops the padding.

Each is a separate `AlgorithmDescriptor`:

```rust
// Proposed — not yet implemented.
pub const SOUNDEX_ENGLISH_1918: AlgorithmDescriptor = AlgorithmDescriptor::new(
    AlgorithmFamily::Soundex,
    VariantId("english-1918"),
    DescriptorVersion::new(0, 1, 0),
    DefinitionSource::Paper {
        title: "Method and means for classifying (US patent 1261167)",
        authors: "R. C. Russell",
        year: 1918,
    },
);

pub const SOUNDEX_REFINED: AlgorithmDescriptor = AlgorithmDescriptor::new(
    AlgorithmFamily::RefinedSoundex,
    VariantId("english-refined"),
    DescriptorVersion::new(0, 1, 0),
    DefinitionSource::ReferenceImplementation {
        name: "Apache Commons Codec RefinedSoundex",
    },
);
```

Golden cases reference the descriptor, so a case for `"english-1918"` cannot silently be run against `"english-refined"` — a class of confusion that plagues cross-library phonetic comparisons.

### Slug conventions for phonetic variants

- Prefer language-first: `"english-1918"`, `"french-r1"`, `"german-cologne"`.
- Where a paper's own designation is standard, use it: `"daitch-mokotoff-1985"`.
- Where a reference implementation is the canonical source, use its name: `"apache-refined"`.
- Never overload a slug across languages — `"main"` is a bad slug; `"english-main"` is fine.

## Modularity

Language support is packaged for feature-driven inclusion.

```
stringcheese-phonetic                  facade + language-agnostic infrastructure
    stringcheese-phonetic-germanic     English, German, Dutch, Yiddish
    stringcheese-phonetic-romance      French, Spanish, Italian, Portuguese
    stringcheese-phonetic-slavic       Russian, Polish, Czech, Ukrainian
    stringcheese-phonetic-semitic      Arabic, Hebrew
    stringcheese-phonetic-indic        Hindi, Bengali, Tamil, Telugu
    stringcheese-phonetic-cjk          Chinese, Japanese, Korean
```

Each language pack is a separate crate. The facade `stringcheese-phonetic` re-exports the subset selected by Cargo features. A minimal Wasm build for an English-only entity-resolution workload pulls in `stringcheese-phonetic-germanic` and nothing else — the Cyrillic transliteration tables never appear in the linked binary.

Language packs contain the encoder implementations, their `Applicability` metadata, their variant descriptors, and any embedded data tables. Cross-language comparison (Beider–Morse against a corpus of mixed-language names) pulls in every relevant pack.

## Long-term goal: phoneme-level comparison

The subsystem's endgame is comparison over phoneme sequences, not encoded keys.

- Encode `"José"` in Spanish phonology to `/x/ /o/ /s/ /e/`.
- Encode `"José"` in English phonology to `/h/ /oʊ/ /z/ /eɪ/`.
- Encode `"Hozay"` in English phonology to `/h/ /oʊ/ /z/ /eɪ/`.
- Compute an edit distance in phoneme space (with phoneme-weighted substitution costs — replacing a vowel with a nearby vowel costs less than replacing it with a consonant).

Phoneme-space comparison enables true multilingual matching: a database of names romanized inconsistently across sources (Chinese Pinyin variants, Arabic transliteration variants, Spanish diaspora spellings) becomes comparable by transcribing each entry into a language's phonology and comparing there.

Design implications for the subsystem:

- The `PhoneticEncoder` trait's associated output type must be general enough to be either a short key or a phoneme sequence — a `PhoneticKey` newtype for the short-key case, a `[Phoneme]` slice for the sequence case, unified under an associated `type Output`.
- Comparators in phoneme space are edit-distance algorithms (Levenshtein, Damerau, alignment) with a substitution-cost matrix derived from phoneme-feature distance. Reuse the existing [distance algorithms](./type-system.md#distancemetrics), instantiated over `Phoneme` symbols.
- Phoneme inventories are per-language. The subsystem accepts multiple phoneme representations (IPA is the target; ARPABET is a pragmatic ASCII intermediate for English) as long as each is well-defined.

Phoneme-level comparison is on the roadmap for version 0.2 (see [DESIGN.md § Future Roadmap](../DESIGN.md)). The version 0.1 subsystem ships with encoded-key comparison only, but the trait shape and the `PhoneticCodes` model are designed so the roadmap extension does not require a breaking change.

## API sketch

```rust
// Proposed — not yet implemented.

use stringcheese_core::{AlgorithmDescriptor, SimilarityMetric, NormalizedSimilarity};

/// A phonetic key: a short symbolic representation of an input's
/// pronunciation, opaque to callers.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PhoneticKey(pub SmallString<[u8; 12]>);

/// A phonetic encoder.
pub trait PhoneticEncoder {
    /// The kind of input this encoder consumes.
    type Input: ?Sized;

    /// The descriptor identifying which variant this encoder implements.
    fn descriptor(&self) -> AlgorithmDescriptor;

    /// The languages, scripts, and regions this encoder was designed for.
    fn applicability(&self) -> Applicability;

    /// Encode `input` to a set of phonetic codes.
    fn encode(&self, input: &Self::Input) -> PhoneticCodes;
}

/// A composed encoder + comparator that matches inputs by their phonetic
/// codes.
pub struct PhoneticMatcher<E, C> {
    encoder: E,
    comparator: C,
    mode: MatchMode,
}

pub enum MatchMode {
    /// Any code-pair match qualifies. Default.
    AnyPair,
    /// Only primary codes are considered.
    PrimaryOnly,
    /// Every code must match; used for strict deduplication.
    AllMustMatch,
}

impl<E: PhoneticEncoder, C: KeyComparator> PhoneticMatcher<E, C> {
    pub fn new(encoder: E, comparator: C) -> Self { /* ... */ }
    pub fn with_mode(self, mode: MatchMode) -> Self { /* ... */ }

    pub fn matches(&self, left: &E::Input, right: &E::Input) -> bool { /* ... */ }

    /// For similarity comparators, the aggregate similarity across all
    /// code pairs under the current mode.
    pub fn similarity(&self, left: &E::Input, right: &E::Input)
        -> NormalizedSimilarity
    where C: KeyComparator<Output = NormalizedSimilarity> { /* ... */ }
}

pub trait KeyComparator {
    type Output;
    fn compare(&self, a: &PhoneticKey, b: &PhoneticKey) -> Self::Output;
}

/// Equality of encoded keys — the classical phonetic-match comparator.
pub struct KeyEquality;
impl KeyComparator for KeyEquality {
    type Output = bool;
    fn compare(&self, a: &PhoneticKey, b: &PhoneticKey) -> bool { a == b }
}

/// Levenshtein over encoded keys — a fuzzier phonetic match tolerant of
/// single-symbol differences in the encoded form.
pub struct KeyLevenshtein { pub max: u32 }
impl KeyComparator for KeyLevenshtein {
    type Output = bool;
    fn compare(&self, a: &PhoneticKey, b: &PhoneticKey) -> bool {
        // Delegates to the edit-distance kernel over byte slices.
        levenshtein_bytes(a.0.as_bytes(), b.0.as_bytes())
            .into_inner() <= self.max
    }
}
```

The matcher composes naturally with the [preprocessing pipeline](./preprocessing-pipeline.md): a pipeline that ends in `.encode_phonetic(...)` transitions the representation from `&str` (or `&[Token]`) to `[PhoneticKey]`, at which point either the `KeyEquality` comparator (a predicate) or a `KeyLevenshtein` comparator (a distance) or a phonetic-similarity `SimilarityMetric` (currently a research direction) takes over.

### `MetricClass` for phonetic comparisons

- **Encoded-key equality.** `MetricClass::Metric` on the encoded-key domain (discrete equality is a metric); *not* a metric on the raw-input domain (many raw inputs collapse to the same key, breaking identity of indiscernibles). Reported classes should be read with the encoding stage in mind, as with all preprocessing (see [preprocessing-pipeline.md § Explainability](./preprocessing-pipeline.md#explainability-hooks)).
- **Encoded-key Levenshtein.** `MetricClass::Metric` on the encoded-key domain; the pipeline as a whole is not a metric on raw input for the same reason.
- **Multi-key aggregate similarity.** `MetricClass::Similarity`. The maximum-over-pairs reduction is monotone but not a metric — the aggregate can be `1.0` (a code pair matched exactly) while other code pairs disagree wildly, so identity of indiscernibles fails.

These distinctions matter for index structures (BK-trees over encoded-key Levenshtein are valid; BK-trees over the aggregate similarity are not).

## Cross-references

- The result types the subsystem uses (`Similarity`, `NormalizedSimilarity`, `Distance`) and the `MetricClass::Similarity` classification are defined in [type-system.md](./type-system.md).
- The pipeline stage that transitions to phonetic representation is described in [preprocessing-pipeline.md § Representation transitions](./preprocessing-pipeline.md#representation-transitions).
- Phoneme sequences can be tokenized into n-grams for a different flavor of phonetic matching; see [ngram-and-fingerprinting.md § Gram generation](./ngram-and-fingerprinting.md#gram-generation).
- Feature-gating the per-language phonetic packs is discussed in [wasm-and-wit-interface.md § Feature-gate strategy](./wasm-and-wit-interface.md#feature-gate-strategy).
