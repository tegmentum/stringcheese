# Tokenizer Subsystem

Status: Design
Applies to: StringCheese 0.2 and later (design only; nothing here ships in 0.1)
Related: [DESIGN.md](../DESIGN.md), [wit-i18n.md](./wit-i18n.md), [preprocessing-pipeline.md](./preprocessing-pipeline.md), [ngram-and-fingerprinting.md](./ngram-and-fingerprinting.md), [wasm-and-wit-interface.md](./wasm-and-wit-interface.md), [type-system.md](./type-system.md)

The design of StringCheese's tokenizer subsystem — the `Tokenizer` and `Segmenter` trait taxonomy, the three-tier crate layout (abstractions, algorithms, pre-configured model packs), the SCUD extension for compressed BPE / WordPiece / Unigram data, the WIT interface for component-model callers, and the integration with the existing comparison, chunking, and language-pack surfaces. **No `stringcheese-tokenizer*` crates exist yet; this document fleshes out the umbrella charter's [pluggable representation-layer](../DESIGN.md) commitment into a family of coordinated crates.**

---

## 1. Motivation

### 1.1. Why tokenization belongs in a string-processing toolkit

Tokenization is where text stops being a stream of scalars and starts being a sequence of *symbols with semantics*. StringCheese already treats representation as a first-class citizen (see [DESIGN.md § Representation Layers](../DESIGN.md#representation-layers)) — a Levenshtein result on `&[u8]` is a different, incompatible answer from the same call on `&[char]` or on a slice of grapheme clusters — and tokens are the next layer up in that taxonomy. A slice of BPE token IDs, or of Unicode words, or of camelCase identifier pieces, is *another* representation, and every algorithm that already generalises over `Sequence<Item>` picks it up for free the moment the tokenizer exists.

The umbrella already commits (in the [`Sub-project map`](../DESIGN.md#sub-project-map)) to `stringcheese-lang` shipping a `SimpleTokenizer`. That tokenizer is deliberately minimal — whitespace-and-punctuation, alphanumeric-run boundaries — and is exactly what a stemming/stopword workflow needs. It is not what a caller reaches for when they want to count tokens against OpenAI's `cl100k_base` before an API call, chunk a generated response at natural boundaries for streaming display, load a Hugging Face `tokenizers.json` for a specific checkpoint and get bit-identical encodings, or run a distance metric between two prompt drafts under the tokenizer the target model actually uses.

### 1.2. The gap: neither raw StringCheese nor `tokenizers-rs` fills it

Hugging Face's [`tokenizers`](https://github.com/huggingface/tokenizers) crate is the mature reference implementation for subword tokenization in Rust — BPE, WordPiece, Unigram; the full `tokenizers.json` spec; battle-tested. For a native, online, host-privileged deployment it is the right dependency. For StringCheese's deployment shape it is the wrong dependency:

- **Wasm-hostile.** Transitive dependencies on `onig`, `esaxx-rs`, and other host-only crates whose bindgen shape is at odds with `wasm32-unknown-unknown`. A `no_std` build is not on the table.
- **Online-first ergonomics.** The paved path fetches model artifacts from the Hub at runtime. StringCheese's paved path is offline, embedded, deterministic.
- **No integration with StringCheese's algorithm surface.** The tokenizer's `Encoding` is not consumed by anything in `stringcheese-compare`, `stringcheese-cdc`, or `stringcheese-index`. A caller who wants token-Levenshtein between two encoded prompts writes the shim themselves.

What the StringCheese tokenizer subsystem gets that neither the raw toolkit nor `tokenizers-rs` alone gets: **(1)** token-aware algorithms out of the box — because every kernel is already generic over `Sequence<Item>`, exposing a `Tokenizer::Token` that satisfies the trait bounds makes token-Levenshtein, token-Jaccard, token-MinHash a straight substitution; **(2)** Wasm-first: `#![no_std]` with `alloc`, no host-only C dependencies, no `onig`; **(3)** offline-first, data-pack packaged: every model tokenizer ships as a SCUD file loaded from disk or embedded via `include_bytes!`.

### 1.3. Non-goals

- **Model registry.** StringCheese does not know that today's `gpt-4o` uses `o200k_base`; that mapping changes and is a policy layer above this subsystem.
- **Request-cost estimation.** Counting tokens is one input; wrapping messages in a provider chat format, image-token accounting, prompt-caching discounts are cost-model concerns.
- **Chat-format templating.** `<|im_start|>`, `[INST]`, and their friends are provider policies, not tokenization.
- **Tokenizer downloads.** SCUD files ship in the crate or beside the binary; the tokenizer crates never open a socket.
- **Training.** Inference-only. Callers who want to *build* a new tokenizer reach for `tokenizers-rs` (via the optional adapter, §3.4).

---

## 2. Two-trait taxonomy: `Tokenizer` vs `Segmenter`

The subsystem exposes two traits, not one. The split falls along a single axis — *round-trippability* — and is load-bearing because the invariant it captures underpins how downstream algorithms may compose with the output.

### 2.1. `Segmenter<Unit>`

A segmenter splits text into units without any commitment that the units can be re-joined into the original input. Grapheme iteration, UAX #29 word segmentation, sentence segmentation, character n-grams, byte n-grams are all segmenters.

```rust
// Proposed — not yet implemented.
// crates/stringcheese-tokenizer/src/segmenter.rs

pub trait Segmenter {
    /// Typically `&'a str` for span-preserving segmenters; an owned
    /// `String` for transforming segmenters.
    type Unit<'a> where Self: 'a;
    type Iter<'a>: Iterator<Item = Self::Unit<'a>> where Self: 'a;

    /// Segmenters are *not* required to be reversible: consuming the
    /// units and concatenating them may lose information (whitespace,
    /// discarded punctuation, casing).
    fn segment<'a>(&'a self, text: &'a str) -> Self::Iter<'a>;
}
```

Byte offsets into the original input are exposed through a companion trait, `SegmenterWithOffsets`, so a segmenter that yields owned units (a lower-casing word segmenter, say) can still surface the source range. Offsets are byte-oriented against the input `&str` — matching StringCheese's rule that the level of every span is explicit.

### 2.2. `Tokenizer<Token>`

A tokenizer commits to *round-trippability*: for a well-defined class of inputs, `decode(encode(text))` recovers `text`. That contract is what lets a caller reason about `count(text)` as a stable quantity, `decode(model_output)` as reconstitutable text, and one tokenizer being swapped for another as an operation that does not change the length of the corpus in symbols the algorithm sees.

```rust
// Proposed — not yet implemented.
// crates/stringcheese-tokenizer/src/tokenizer.rs

pub trait Tokenizer {
    /// `TokenId` for subword tokenizers, `&'a str` for word-level
    /// borrowing tokenizers, `String` for normalising tokenizers.
    type Token: PartialEq;
    type Encoding: Encoding<Token = Self::Token>;
    type EncodeError;
    type DecodeError;

    fn encode(&self, text: &str) -> Result<Self::Encoding, Self::EncodeError>;

    fn decode<'t>(
        &self,
        tokens: impl IntoIterator<Item = &'t Self::Token>,
    ) -> Result<alloc::string::String, Self::DecodeError>
    where
        Self::Token: 't;

    /// Count without materialising the full encoding.
    fn count(&self, text: &str) -> Result<usize, Self::EncodeError>;

    fn special_tokens(&self) -> &[(alloc::borrow::Cow<'static, str>, Self::Token)] {
        &[]
    }
}

pub trait Encoding {
    type Token;
    fn ids(&self) -> &[Self::Token];
    fn offsets(&self) -> &[core::ops::Range<u32>];
    fn special_mask(&self) -> &[bool];
    fn len(&self) -> usize { self.ids().len() }
}
```

The `decode(encode(text)) == text` invariant admits three well-defined exceptions:

- **Normalization.** If the tokenizer applies a lossy normalization step (NFC, NFKC, casefolding), decode returns the normalized form. The offsets record the *pre-normalization* byte positions, so a highlight-in-original-text UI still works.
- **Unknown-character replacement.** A byte outside the vocabulary is replaced by a documented unknown token (typically `<unk>`); decode surfaces the replacement string, not the original byte.
- **Truncation.** If the caller feeds a `max_length`-truncated encoding to `decode`, the reconstructed text is a prefix of the input. This is a caller-visible loss, not a tokenizer bug.

### 2.3. Bridge to `stringcheese-core::Sequence`

The pay-off of the trait split shows up when the tokenizer's output flows into the comparison surface. A `Tokenizer::Encoding::ids()` returns `&[Self::Token]`, which is already a `[T]` — and `stringcheese-core` already implements `IndexableSequence` for `[T]` (see [`sequence.rs`](../../crates/stringcheese-core/src/sequence.rs)). The generic Levenshtein kernel, the generic Jaro kernel, the generic MinHash — all consume a slice of `Token` values with no adapter layer:

- `Token: PartialEq` is the minimum bound and is enough for Levenshtein, Hamming, LCS, and the alignment kernels.
- `Token: Ord + Hash` is enough for MinHash bucketing, `GramSet` / `GramMultiSet` construction, and Jaccard/Dice-on-token-sets.
- `Token: Clone` is what n-gram windowing needs so it can materialise owned windows.

The concrete `TokenId = u32` used by every subword tokenizer satisfies all three bounds by construction. A caller wiring `CL100K_BASE.encode(prompt_a)?.ids()` and `CL100K_BASE.encode(prompt_b)?.ids()` into `stringcheese_compare::Levenshtein::default().distance(...)` gets a token-level edit distance with no plumbing.

Segmenters do not carry the same bridge: their output is `Unit<'a>`, which for the string-yielding segmenters is `&'a str`. A `&[&'a str]` is again an `IndexableSequence`, so word-level Jaccard falls out just as cleanly, but there is no invariant that lets a caller `decode` the output. The segmenter/tokenizer trait split is what makes the comparison surface unambiguous about *which* operations round-trip.

---

## 3. Crate layout

The subsystem is three tiers of crates plus one optional adapter — never one monolith. The tiering is the same principle that governs the ICU-alternative i18n subsystem (traits, algorithms, data — see [wit-i18n.md § WIT package layout](./wit-i18n.md#3-wit-package-layout)): each tier has one job, and each tier can be updated on its own release cadence without churning the tier above or below.

### 3.1. Tier 1 — abstractions and built-ins

**`stringcheese-tokenizer`** — the trait crate.

Ships the `Tokenizer` and `Segmenter` traits, the `Encoding` companion trait, the shared error taxonomy, and every built-in tokenizer that does not need a large data table (whitespace, delimiter, identifier splitters, n-gram wrappers, thin Unicode wrappers over `stringcheese-unicode`). `no_std` + `alloc`; no dependency on any tokenizer algorithm crate.

### 3.2. Tier 2 — algorithm implementations (data-neutral)

Each subword algorithm lives in its own crate so a caller who wants only BPE does not link the WordPiece or Unigram code, and so that a bug fix in the BPE encoder does not force a re-release of the WordPiece crate.

- **`stringcheese-tokenizer-hf`** — Byte-Pair Encoding. The caller supplies a merge table and a vocabulary; the crate implements the encoding loop. Also the substrate every tiktoken variant is built on.
- **`stringcheese-tokenizer-wordpiece`** — WordPiece (BERT-family). Same shape: caller supplies vocabulary + suffix rules.
- **`stringcheese-tokenizer-sentencepiece`** — SentencePiece, supporting the Unigram (T5/mBART) and BPE (Llama, Mistral, older Gemma checkpoints) variants. Whitespace-as-`▁` preprocessing is opt-in through configuration.

### 3.3. Tier 3 — pre-configured model tokenizers

Convenience crates that ship the algorithm from Tier 2 *plus* an embedded SCUD data pack for one or more well-known vocabularies. Each variant is behind a Cargo feature so a caller only pays for the packs they use.

- **`stringcheese-tokenizer-tiktoken`** — OpenAI tokenizers: `cl100k_base`, `r50k_base`, `p50k_base`, `p50k_edit`, `o200k_base`, and `gpt2`. Built on top of `stringcheese-tokenizer-hf`. Features: `cl100k`, `r50k`, `p50k`, `p50k-edit`, `o200k`, `gpt2`.
- **`stringcheese-tokenizer-huggingface`** — Hugging Face `tokenizers.json` spec parser. Loads any HF tokenizer artifact into whichever Tier-2 algorithm crate matches. Cargo features per algorithm family (`bpe`, `wordpiece`, `unigram`) select which implementations link.

### 3.4. Optional adapter

- **`stringcheese-tokenizer-hf-native`** — feature-gated adapter that delegates to upstream `tokenizers-rs`. This is the escape hatch for callers on native targets who want the full Hugging Face ecosystem (fast normalization pipelines, pre-tokenizer trees, decoders with edge-case coverage) without reimplementing anything. Behind a `hf-native` feature; the crate compiles to an empty module without it. Not usable on `wasm32-unknown-unknown`; that is the caller's problem, not the umbrella's.

### 3.5. Dependency diagram

```
                +------------------------------+
                |   stringcheese-tokenizer     |   Tier 1
                |   (traits + built-ins)       |
                +--------------+---------------+
                               ^
        +----------------+-----+------+-----------------+
        |                |            |                 |
+-------+------+ +-------+-----+ +----+---------+ +-----+---------+
|  ...-bpe     | | ...-word-   | | ...-sentence-| | ...-hf-native |
|              | |  piece      | |  piece       | |  (opt)        |   Tier 2
+-------+------+ +-------------+ +------+-------+ +---------------+
        ^                              ^
        |                              |
+-------+---------+          +---------+----------+
| ...-tiktoken    |          | ...-huggingface    |    Tier 3
| (SCUD + BPE)    |          | (tokenizers.json)  |
+-----------------+          +--------------------+
```

Edges point from a dependent to what it depends on. Tier 3 crates depend on Tier 2 algorithm crates and on `stringcheese-scud` (from the i18n subsystem — the SCUD loader is shared, not duplicated). No horizontal edges within a tier.

---

## 4. Built-in tokenizers

The built-ins live in `stringcheese-tokenizer` because none need an external data table larger than what `stringcheese-unicode` already carries. They are the "reach for it in five seconds without pulling in a model" set.

| Name | Trait | Notes |
| --- | --- | --- |
| `WhitespaceTokenizer` | `Segmenter<Unit = &str>` | Split on Unicode `White_Space`. |
| `DelimiterTokenizer` | `Segmenter<Unit = &str>` | Split on caller-supplied `char`s or a `&[char]` predicate. |
| `IdentifierTokenizer` | `Segmenter<Unit = &str>` | camelCase / PascalCase / snake_case / kebab-case / dotted-path / SCREAMING_SNAKE splitting with a configurable mode set. For code-adjacent workloads (searching identifiers, comparing symbol names). |
| `WordSegmenter` | `Segmenter<Unit = &str>` | UAX #29 word breaks. Delegates to `stringcheese_unicode::words` / `word_bounds`; both `WordsOnly` and `AllBoundaries` behaviours are exposed. |
| `SentenceSegmenter` | `Segmenter<Unit = &str>` | UAX #29 sentence breaks; delegates to `stringcheese_unicode::sentences`. |
| `GraphemeSegmenter` | `Segmenter<Unit = &str>` | UAX #29 extended grapheme clusters. Delegates to `stringcheese_unicode::graphemes`. |
| `NgramSegmenter<N>` | `Segmenter<Unit = &[T]>` | Character / byte / word / grapheme n-grams. Thin re-exposure of `stringcheese_compare::ngram::CharacterGrams` / `TokenGrams`. |
| `RegexTokenizer` | `Segmenter<Unit = &str>` | Split on a regex. Feature-gated (`regex`); accepts `regex-lite` patterns for Wasm-tight builds. |

None of the above satisfies `Tokenizer` — all are segmenters. Any caller reaching for "tokens I can decode back to text with a well-defined round-trip contract" is asking for a subword tokenizer, and the built-ins are the wrong layer for that. Two hybrid cases fill the gap: **`ByteTokenizer`** (`Tokenizer<Token = u8>`, encodes to UTF-8 bytes, decodes through `String::from_utf8` — useful as a testing sentinel and as the substrate for byte-level BPE) and **`CharTokenizer`** (`Tokenizer<Token = char>`, analogous, for character-level distance kernels that want `Tokenizer`'s introspection surface without a real vocabulary).

---

## 5. BPE algorithm and data packs

BPE — Byte-Pair Encoding — is the workhorse. tiktoken (and therefore every GPT-family tokenizer since GPT-2), Llama, Mistral, and roughly two-thirds of the open-weight ecosystem are BPE variants. `stringcheese-tokenizer-hf` is a data-neutral crate: it implements the algorithm and knows nothing about any specific vocabulary.

### 5.1. Algorithm description

BPE operates over three pieces of data: a **merge table** (an ordered list of pair merges, each with a rank — lower rank means earlier merge), a **vocabulary** (bijection between token IDs and their surface bytes, including the base-alphabet tokens and every merged pair), and an optional **special-token map** (surface strings that map to reserved IDs and never participate in merges — `<|endoftext|>`, `<|im_start|>`, and similar).

The encoding loop: optional pre-tokenizer regex splits the input into chunks; for each chunk, seed a `pieces` sequence with byte-level IDs of the chunk's UTF-8 bytes; repeatedly find the adjacent pair whose merge rank is lowest (highest priority) and replace it with the merged ID; stop when no adjacent pair is in the merge table; concatenate all chunks' pieces. The naive form is O(n²) per chunk; the production implementation uses a doubly-linked-list-plus-min-heap variant that pushes amortised complexity to O(n log n) and matches tiktoken's throughput. Neither variant needs unsafe; both are `no_std` + `alloc`.

Pre-tokenization is a critical detail. tiktoken's `cl100k_base` splits input on a specific regex (contractions, letters, numbers, and punctuation each get their own chunk) *before* the BPE loop runs. This is what keeps merges from crossing word boundaries and what makes the encoding stable across whitespace variations. `stringcheese-tokenizer-hf` accepts a pre-tokenizer as configuration: `None` for raw byte-level BPE (GPT-2 style), `Some(regex)` for the tiktoken variants, `Some(WhitespaceTokenizer)` for SentencePiece-shape callers.

Special-token handling runs *before* pre-tokenization: if the input contains a special-token surface string, that substring is atomised into its reserved ID and the surrounding text is processed separately. Callers who need to disable special-token recognition (for embedding user-provided text that legitimately contains `<|endoftext|>`) pass an `allowed_special` set at encode time, matching tiktoken's ergonomics.

### 5.2. Data-pack format — SCUD extension for BPE

The BPE data pack is a SCUD file with `cap-id = BPE_` (`0x42 0x50 0x45 0x20`). It reuses the SCUD outer envelope (magic / version / flags / header-len / body) exactly as documented in [wit-i18n.md § 4.1](./wit-i18n.md#41-file-layout), so the same `stringcheese-scud` loader validates it.

The BPE-specific body carries a fixed header (offsets and counts for pre-tokenizer regex, merge table, vocabulary, and special-token map, plus a `byte-alphabet-mode` byte selecting `BYTE`, `CHAR`, or `BPE_GPT2`) followed by four regions: the pre-tokenizer regex as UTF-8 (compiled at load time), the merge table (a `SequencePool` of merged-pair byte strings paralleled by a `PackedIntegers` array of ranks), the vocabulary (a `StringPool` with prefix compression paralleled by `PackedIntegers` token IDs), and the special-tokens map (a `StringPool` plus a reserved-ID `PackedIntegers` array). Every primitive is one of the SCUD structural primitives documented in [wit-i18n.md § 4.2](./wit-i18n.md#42-compression-primitives) — `SequencePool` for merged-pair strings (they share prefixes and suffixes prolifically), `StringPool` for the vocabulary, `PackedIntegers` for parallel rank arrays. The whole body is then subject to the outer Brotli/Zstd pass exactly as in the case-mapping packs.

Size projection for `cl100k_base` — the biggest Wave-1 tiktoken variant: raw `mergeable_ranks` is ~100 000 entries with ~5-byte average pair strings plus 4-byte ranks = ~1 MB; SCUD-encoded with `SequencePool` interning + `PackedIntegers` rank encoding: ~500–700 KB back-of-envelope; after outer Brotli: target **250–400 KB per pack**. `o200k_base` (200k entries) projects to **500–800 KB**. These are projections, not measurements — see §13. The design commits to *measuring* and reporting the actual ratios per-pack in the correctness report before the first release.

### 5.3. Loader API sketch

```rust
// Proposed — not yet implemented.
// crates/stringcheese-tokenizer-hf/src/lib.rs

pub struct BpeTokenizer {
    scud: stringcheese_scud::ScudFile,   // holds mmap / static slice
    pre_tokenizer: Option<CompiledRegex>,
    merges: MergeTableView<'static>,
    vocab: VocabularyView<'static>,
    special: SpecialTokensView<'static>,
}

impl BpeTokenizer {
    pub fn from_scud(scud: ScudFile) -> Result<Self, BpeLoadError>;
    /// Convenience for `include_bytes!` payloads.
    pub fn from_static(bytes: &'static [u8]) -> Result<Self, BpeLoadError>;
}

impl Tokenizer for BpeTokenizer { /* ... */ }
```

Header parsing is `O(1)`; the parsed views are references into the SCUD's zero-copy body, so `BpeTokenizer::from_scud` performs no allocation beyond the pre-tokenizer regex compile. That matches the invariant every SCUD-backed loader in the ecosystem holds.

---

## 6. tiktoken pack — `stringcheese-tokenizer-tiktoken`

The tiktoken pack is the flagship model-tokenizer crate: it ships the OpenAI BPE variants as SCUD data packs on top of `stringcheese-tokenizer-hf`.

### 6.1. Variants shipped

Wave 1: `cl100k_base` (GPT-3.5 / GPT-4), `o200k_base` (GPT-4o / o1), `r50k_base` (GPT-3), `p50k_base` (Codex + some GPT-3.5), `p50k_edit` (edit-mode Codex), and `gpt2` (GPT-2, still frequently referenced in research code). Each is behind its own Cargo feature; enabling `default-features = false` and no feature at all links only the algorithm layer.

### 6.2. Source data provenance

tiktoken publishes its `mergeable_ranks` blobs under a permissive licence in the upstream repo (<https://github.com/openai/tiktoken>). The build tool `stringcheese-tokenizer-tiktoken-build` (a workspace binary, not shipped in the runtime crate) fetches the blobs by their published SHA-256 hashes, validates the hashes, extracts merge tables and vocabularies, encodes them into SCUD, and writes the packs into `crates/stringcheese-tokenizer-tiktoken/data/`. The hashes are checked into the build tool so a re-run against upstream drift fails loudly rather than silently changing an embedded pack.

Alternatively, packs may be regenerated from tiktoken's Python `tiktoken.get_encoding("cl100k_base")` runtime dump — useful for cross-checking. The build tool supports both paths.

### 6.3. Embedding shape and public API

Each shipped SCUD pack is embedded via `include_bytes!` under its feature gate, mirroring the language-pack shape in [wit-i18n.md § 6](./wit-i18n.md#6-language-pack-integration). The public constant for each variant is a `once_cell::sync::Lazy<BpeTokenizer>` (or `once_cell::race::OnceBox` on `no_std` no-`sync` targets) initialised on first use:

```rust
// Proposed — not yet implemented.
use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_tiktoken::CL100K_BASE;

let enc = CL100K_BASE.encode("Hello, world!")?;
assert_eq!(enc.ids(), &[9906, 11, 1917, 0]);
let round_trip = CL100K_BASE.decode(enc.ids().iter())?;
assert_eq!(round_trip, "Hello, world!");

// The `count` fast path skips materialising the `Encoding`:
let n = CL100K_BASE.count(&user_prompt)?;
if n > 8192 { return Err(TooLong); }
```

The tiktoken crate is `#![no_std]` + `alloc`; no host-only dependencies.

---

## 7. Hugging Face `tokenizers.json` support

Hugging Face's tokenizer format (<https://huggingface.co/docs/tokenizers/api/tokenizer>) is a JSON serialisation of a `Tokenizer` object: normalizer, pre-tokenizer, model (BPE / WordPiece / Unigram / WordLevel), post-processor, and decoder, each with its own configuration record. Every model on the Hub that ships tokenizer configuration ships a `tokenizer.json` conforming to this spec.

`stringcheese-tokenizer-huggingface` parses the spec and constructs the corresponding Tier-2 algorithm instance:

- **Model**: dispatches to `stringcheese-tokenizer-hf`, `stringcheese-tokenizer-wordpiece`, or `stringcheese-tokenizer-sentencepiece` based on the `type` field.
- **Normalizer**: implemented as a `Segmenter`-style adapter over `stringcheese-unicode` (NFC / NFKC / lowercase / strip-accents), composed if multiple normalizers are stacked.
- **Pre-tokenizer**: dispatches to a built-in from `stringcheese-tokenizer` (whitespace / metaspace / byte-level / regex / punctuation).
- **Decoder**: paired with the pre-tokenizer; byte-level pre-tokenization needs a byte-level decoder so the round-trip contract holds.
- **Post-processor**: template-based special-token insertion (`[CLS] $A [SEP]`, etc.).

This is a substantial undertaking; the phasing (see §11) scopes it as multi-phase, with Phase 5 delivering BPE-only JSON parsing (which unlocks Llama, Mistral, most GPT-style checkpoints on the Hub) and Phase 6 completing the WordPiece and Unigram families.

The parser converts JSON to an in-memory `HuggingFaceConfig` value; that value is then handed to a `HuggingFaceTokenizer::from_config` constructor that materialises the appropriate Tier-2 instance. The intermediate representation is exposed so callers who want to inspect the config, save it to SCUD (see §7.1), or programmatically alter it before instantiation can do so.

### 7.1. HF-to-SCUD conversion

A caller who loads a HF tokenizer at runtime pays the JSON parse cost every startup. A build-time conversion tool (`hf-to-scud`) reads a `tokenizer.json` and produces an equivalent SCUD pack — same body layout as the tiktoken packs, plus a small header field recording the source normalizer/pre-tokenizer identifiers so the SCUD loader can reconstruct the pipeline. This is the recommended path for deployment: parse JSON once, ship SCUD.

---

## 8. WIT interface

The component-model boundary is a WIT interface following the shape established by [wit-i18n.md § 3.1](./wit-i18n.md#31-illustrative-wit-stringcheese-icu-case). **Illustrative — the exact IDL will be tuned during implementation.**

```wit
// Proposed — not yet implemented.
// component/wit/tokenizer/stringcheese-tokenizer.wit

package stringcheese:tokenizer@0.1.0;

interface types {
    /// Vocabulary index. `u32` covers every shipped tokenizer.
    type token-id = u32;

    /// Half-open byte range in the pre-normalization input.
    record range { start: u32, end: u32 }

    record encoding {
        ids:           list<token-id>,
        offsets:       list<range>,       // parallel to `ids`
        special-mask:  list<u8>,          // 1 iff the id is special
    }

    variant tokenizer-error {
        invalid-utf8(string),
        vocabulary-mismatch(string),
        allocation-failed,
        unknown-special-token(string),
        decode-produced-invalid-utf8,
        loader-failed(string),
    }

    /// Mirrors tiktoken's `allowed_special` ergonomics.
    variant special-policy {
        forbid,
        allow-all,
        allow-only(list<string>),
    }
}

interface tokenizer {
    use types.{token-id, encoding, tokenizer-error, special-policy};

    encode: func(text: string) -> result<encoding, tokenizer-error>;
    encode-with-policy: func(
        text: string, policy: special-policy,
    ) -> result<encoding, tokenizer-error>;
    decode: func(ids: list<token-id>) -> result<string, tokenizer-error>;
    count: func(text: string) -> result<u64, tokenizer-error>;

    special-tokens: func() -> list<tuple<string, token-id>>;
    add-special-token: func(
        text: string, id: option<token-id>,
    ) -> result<token-id, tokenizer-error>;
}

interface capabilities {
    /// Stable variant identifier — `"cl100k_base"`, `"o200k_base"`, ...,
    /// or the HF config's declared model name.
    variant-id: func() -> string;
    vocab-size: func() -> u32;
}

world tokenizer-provider {
    export types;
    export tokenizer;
    export capabilities;
}
```

The offset record is the most consequential design choice here. Every downstream operation that wants to relate token positions back to the input — highlighting, chunking at token boundaries, aligning two encodings for diff — needs it. The tiktoken CLI does not surface offsets; Hugging Face's `Encoding` does. StringCheese sides with HF here because the loss of information at the WIT boundary is irreversible: a caller who does not need offsets can ignore them, but a caller who needs them cannot reconstruct them from ids alone.

The `special-mask` is a `list<u8>` rather than a `list<bool>` because the WIT-to-canonical-ABI mapping for `list<bool>` is under-specified in some hosts; `u8` avoids that and reads identically at the caller.

---

## 9. Integration with the StringCheese surface

The tokenizer subsystem is designed so that every existing StringCheese algorithm crate picks it up without a bespoke integration layer. The interface is `Tokenizer::Token: PartialEq` (or `+ Ord + Hash` for the set/hash-based algorithms) — nothing more.

### 9.1. `stringcheese-compare`

Token-level Levenshtein / Jaro / Damerau on subword encodings falls out of the existing generic kernels — `IndexableSequence` is already implemented for `[T]`, and `Encoding::ids()` returns `&[TokenId]`:

```rust
// Proposed — not yet implemented.
use stringcheese_compare::Levenshtein;
use stringcheese_tokenizer_tiktoken::CL100K_BASE;

let a = CL100K_BASE.encode("The cat sat on the mat.")?;
let b = CL100K_BASE.encode("The cat sat on a mat.")?;
let d = Levenshtein::default().distance(a.ids(), b.ids());
```

Set-similarity kernels compose the same way — `GramSet::from(TokenGrams::new(3).grams(a.ids()))` gives token-shingles ready for Jaccard, Dice, or MinHash bucketing. No adapter, no shim.

### 9.2. `stringcheese-cdc`

FastCDC over token IDs (rather than bytes) chunks generated text at natural boundaries. A prompt-response cache keyed on token-level chunks reuses across every prompt that shares a prefix, regardless of whitespace. The `Chunker` trait already generalises over `Sequence<Item>`; wiring it to `Encoding::ids()` is one line at the call site.

### 9.3. `stringcheese-manip`

`TextPipeline` (the IR described in [preprocessing-pipeline.md](./preprocessing-pipeline.md)) grows a `Tokenize<T>` stage that terminates the string-shaped part of the pipeline and yields whatever `T::Token` type the caller supplied. Downstream stages consume the token slice — a `TokenFilter` for stopword removal in token space, a `TokenNormalize` for lowercasing per-token surface form — and terminate in an algorithm invocation.

### 9.4. `stringcheese-lang::Language::tokenize`

Today, `Language::tokenize` returns `Box<dyn Iterator<Item = &'a str> + 'a>` (see [`language.rs`](../../crates/stringcheese-lang/src/language.rs)). The migration path: introduce a `default_segmenter` method returning `Box<dyn Segmenter<Unit<'a> = &'a str> + 'a>`, deprecate `tokenize` in favour of it, and let packs override with any `Segmenter` — an `EnglishSegmenter` that handles contractions, a `GermanSegmenter` that does compound splitting, a `JapaneseSegmenter` that runs UAX #29 with the CLDR dictionary. The rename from `tokenize` (which now overloads two meanings) to `segment` clarifies the semantics. Callers who want a *subword tokenizer* for the language ask for `Language::subword_tokenizer(model: ModelHint) -> Option<&dyn Tokenizer<Token = TokenId>>` — see §10.

### 9.5. `stringcheese-index`

Q-gram inverted indexes over token grams (rather than character grams) are a straight substitution: the `GramSet<TokenId>` case already works. This unlocks fast candidate generation for token-level dedup — near-duplicate LLM prompts, plagiarism detection at the token level.

---

## 10. Language-pack integration

A language pack today ships a stopword list, a stemmer, and a default tokenizer. With the subsystem in place, packs grow one optional field: a *subword tokenizer accessor*.

The typical shape:

- `English::tokenize` (renamed to `segment`) returns the improved `EnglishSegmenter` — contraction-aware, sentence-boundary-aware, still borrowing `&str` slices for zero allocation. This is what stemming, stopword filtering, and word-count workflows consume.
- `English::subword_tokenizer(ModelHint::OpenAiGpt4)` returns `Some(&CL100K_BASE)` if the tiktoken feature is enabled — a language pack composes a model tokenizer without depending on tiktoken directly (it depends on the tiktoken feature being enabled at the top level).
- The `ModelHint` enum is *not* a model registry — it is a small enum of well-known families (`OpenAiGpt4`, `OpenAiGpt4o`, `Llama3`, `Mistral7B`) that the caller opts into. StringCheese-the-library does not commit to which OpenAI model uses what; the enum is a policy hook the caller sets.

Contrast in practice: an English stemming + stopword workflow uses `ENGLISH.segment(text).filter(|w| !ENGLISH.is_stopword(w)).map(|w| ENGLISH.stem(w))` — pure word-level. An "estimate my API cost" workflow uses `ENGLISH.subword_tokenizer(ModelHint::OpenAiGpt4).unwrap().count(text)?`. The two paths never interfere.

For languages whose default subword tokenizer diverges meaningfully (Japanese, Chinese, Korean — where a good subword tokenizer starts with a sentence-piece Unigram over pre-segmented CJK text), the pack composes accordingly. This is why the accessor is on the `Language` trait and not on a global registry: the pack is the natural owner of the "what is the right default tokenizer for this language" decision.

---

## 11. Phased implementation plan

Delivery is per phase, each phase self-contained enough that a caller who only needs the earlier phases never has to wait on the later ones. Every phase names its "done when" criteria.

**Phase 1 — `stringcheese-tokenizer` trait crate + built-ins.** Done when the `Tokenizer` / `Segmenter` / `Encoding` traits are published in `crates/stringcheese-tokenizer/src/`; `WhitespaceTokenizer`, `DelimiterTokenizer`, `IdentifierTokenizer`, `GraphemeSegmenter`, `NgramSegmenter`, `ByteTokenizer`, and `CharTokenizer` implement the traits and pass ≥ 100 golden vectors each; `WordSegmenter` and `SentenceSegmenter` are wired to `stringcheese_unicode::words` / `sentences` (both UAX #29 iterators shipped alongside Phase 1); the crate's `wasm32-unknown-unknown` footprint is reported in CI.

**Phase 2 — `stringcheese-tokenizer-hf` algorithm crate.** Done when the BPE encoder + decoder pass a hand-constructed merge-table corpus (~20 tables spanning "trivial two-token merge", "byte-level GPT-2 shape", "pre-tokenizer regex-driven tiktoken shape"); the O(n log n) linked-list-plus-heap implementation agrees with the naive O(n²) oracle over exhaustive short inputs; the special-token policy machinery matches tiktoken's `allowed_special` behaviour on their published test cases.

**Phase 3 — `stringcheese-tokenizer-tiktoken` with `cl100k_base` first.** Done when `cl100k_base` is embedded as a SCUD pack; `CL100K_BASE.encode(text)?.ids()` produces bit-identical output to Python `tiktoken.get_encoding("cl100k_base").encode(text)` over ≥ 10 000 diverse inputs (English, code, JSON, non-Latin scripts). `o200k_base` follows as a straight repeat of the build pipeline.

*Progress note (parity harness landed).* The bit-identical diff mechanism itself now lives in [`stringcheese-tokenizer-tiktoken-conformance`](../../crates/stringcheese-tokenizer-tiktoken-conformance/), a workspace-excluded crate that fetches OpenAI's `mergeable_ranks` blobs from the public CDN by SHA-256 at test time, caches them in `~/.cache/stringcheese-tokenizer-tiktoken/`, and diffs `stringcheese-tokenizer-hf`'s output against `tiktoken-rs` over a 200-input corpus. Run with `cargo test --manifest-path crates/stringcheese-tokenizer-tiktoken-conformance/Cargo.toml --features parity-real-vocab`; a non-blocking CI job wires the same invocation. First-run parity against the shipped harness: **`cl100k_base` 200/200**, **`o200k_base` 200/200** (both variants clean under their respective pre-tokenizer regexes; the harness carries an `o200k`-specific pattern local to itself, promoting it upstream into `stringcheese-tokenizer-hf` is a follow-on task). Scaling the corpus toward the 10 000-input target is a subsequent change that only touches the corpus module.

**Phase 4 — SCUD extension for BPE data packs. Obviated by direct `tokenizer.json` loading.** The original phase would have registered `cap-id = BPE_` in the SCUD spec, taught the loader to validate the BPE header, measured per-variant compression ratios (§13), and shipped a `stringcheese-tokenizer-hf-build` binary that regenerated SCUD packs from tiktoken's upstream blobs. **None of that shipped, and this phase is closed without landing it.** The two shifts that made the compression path unnecessary:

1. **`stringcheese-tokenizer-hf` reads `tokenizer.json` directly.** Phases 5–6 landed a full parser for the Hugging Face on-disk shape — normalizer / pre-tokenizer / model / post-processor / decoder — including untagged fallbacks for the three real HF checkpoints (`openai-community/gpt2`, `FacebookAI/xlm-roberta-base`, `google-bert/bert-base-multilingual-cased`) that ship without a `"type"` tag on their `model` node. Callers who need a tokenizer point the loader at the vendor's shipped file. There is no intermediate SCUD serialisation between the JSON on disk and the runtime tokenizer, and none is missed: real `tokenizer.json` blobs are 2–5 MB each and load in tens of milliseconds; the compression targets in §5.2 solved a problem the direct path never surfaces.
2. **The `stringcheese-tokenizer-tiktoken` pack settled on the tiktoken plaintext format, not a SCUD extension.** Real OpenAI `mergeable_ranks` blobs cannot live in-tree for licence + repo-bloat reasons, so the tiktoken pack's `build.rs` transcodes contributor-supplied plaintext blobs from `data/<variant>.tiktoken` at build time (and synthesises a small stand-in tokenizer per variant when the plaintext is absent). Real-vocab conformance runs in the workspace-excluded `stringcheese-tokenizer-tiktoken-conformance` crate, which fetches the blobs by SHA-256 under an opt-in `parity-real-vocab` feature. A SCUD-compressed pack would have added a build-time compressor, a runtime decompressor, and a header schema to a path that already round-trips through the vendor format.

The `stringcheese-scud` loader itself remains a live design item for the WIT-based i18n subsystem ([wit-i18n.md § 4](./wit-i18n.md#4-scud-data-pack-format-spec)); the tokenizer subsystem simply does not consume it. The **250–400 KB / 500–800 KB per-pack projections in §5.2 are therefore unmeasured and will not be measured** — the projection is a design-time artefact, not a release-gate metric on any shipped tokenizer variant.

**Phase 5 — HF `tokenizers.json` parser, BPE-only.** Done when the parser accepts every `tokenizer.json` in a corpus of the top ~30 HF models (Llama, Mistral, Qwen, Phi, DeepSeek, ...) whose `model.type` is `BPE`; matches upstream `tokenizers-rs` output on ≥ 1 000 sample inputs per model; the `hf-to-scud` build tool converts a `tokenizer.json` to a SCUD pack that loads under `BpeTokenizer::from_scud`.

*Progress note (`ByteLevel` post-processor landed).* GPT-2 `tokenizer.json` blobs ship a `ByteLevel` post-processor whose sole runtime effect (in HF's own crate) is to trim leading `Ġ` characters from *character-space* offset spans when `trim_offsets: true`. This crate reports byte offsets rather than character offsets, and HF's own `process()` ignores the post-processor's `add_prefix_space` and `use_regex` fields (the pre-tokenizer's `add_prefix_space` is the one that governs encoding end-to-end). The `ByteLevel` post-processor is therefore materialised as a typed variant on `PostProcessor::ByteLevel` and applied as a **pure no-op** on both `ids` and `offsets` — the loader accepts the config, and `encode()` produces byte-identical output to a config with the post-processor omitted. This unblocks the four-checkpoint conformance corpus's `gpt2` case, which used to panic at load with `UnsupportedPostProcessor { type_name: "Other" }`.

*Progress note (untagged `model` fallback landed).* `openai-community/gpt2/tokenizer.json` v1.0 — still the shipped GPT-2 config — omits the `"type"` field on its `model` node, mirroring HF's own internal `#[serde(untagged)]` autodetection. Our `HfModel` used to require the tag and failed the whole parse with `missing field "type"` at load time, blocking the conformance corpus's `gpt2` case one layer earlier than the `ByteLevel` post-processor. The fix keeps HF's canonical tagged shape as the primary path (a small internal `HfModelTagged` mirrors the four `"BPE"` / `"WordPiece"` / `"Unigram"` / `"WordLevel"` variants) and adds a hand-rolled `Deserialize` on `HfModel` that falls back to [`HfBpeModel`] when the input has no `"type"` field **and** carries both a JSON-object `"vocab"` and an array `"merges"`. A typeless Unigram- or WordPiece-shape node is rejected explicitly rather than silently misclassified — no HF release *initially surveyed* shipped those without the tag, so a missing tag on that shape was a signal of a malformed config, not a normal case to support. With this in place the `gpt2` conformance case runs 20/20 end-to-end against the real HF-hosted vocab. *(Subsequent landing widened this — see the typeless-Unigram note below — after `FacebookAI/xlm-roberta-base` was found shipping typeless Unigram in the wild.)*

*Progress note (typeless-Unigram fallback landed).* `FacebookAI/xlm-roberta-base/tokenizer.json` — the shipped xlm-r config — is the second real HF checkpoint we have found that omits `"type"` on its `model` node; unlike the GPT-2 blob, its `vocab` is a JSON *array* of `[surface, score]` pairs (with `unk_id: 3`), i.e. the Unigram shape rather than the BPE shape. The untagged-fallback branch of `HfModel::deserialize` now disambiguates on the JSON type of `"vocab"`: an object routes to [`HfBpeModel`] (needs a companion array `"merges"`), an array routes to [`HfUnigramModel`] (whose fields deserialise straight through). The prior negative test that pinned "typeless Unigram is rejected" was replaced with a positive test using the inline `{unk_id, vocab: [[surface, score], ...]}` shape xlm-r ships. With this in place the `xlm_roberta_base` conformance case runs 40/40 end-to-end against the real HF-hosted vocab (previously blocked 0/40 at load time). *Red flag surfaced during implementation:* `google-bert/bert-base-multilingual-cased/tokenizer.json` also ships typeless — but as `WordPiece` (dict `vocab` + `unk_token`, no `merges`). This wave leaves it rejected with the widened diagnostic; a follow-on landing that adds a typeless-`WordPiece` branch (disambiguating from typeless-BPE via `merges` present-or-absent) would unblock the `bert_base_multilingual_cased` conformance case similarly.

*Progress note (typeless-`WordPiece` fallback landed).* The follow-on the previous note flagged: `google-bert/bert-base-multilingual-cased/tokenizer.json` — the shipped mBERT config — is the third real HF checkpoint we have found that omits `"type"` on its `model` node. Its `vocab` is a JSON object (same shape as typeless-BPE), no `"merges"` array, and a mandatory `"unk_token"` string — the classic `WordPiece` shape without the tag. The untagged-fallback branch of `HfModel::deserialize` now runs a third disambiguation after the object-plus-merges (BPE) and array-vocab (Unigram) branches: an object `"vocab"` with `"merges"` **absent** and a string `"unk_token"` **present** routes to [`HfWordPieceModel`]. The `"merges"`-absent gate is what disambiguates it from typeless-BPE (BPE requires `merges`); the `"unk_token"`-present gate is what distinguishes an mBERT-shape config from a corrupt BPE that dropped its `merges` by author error — WordPiece's spec makes `unk_token` mandatory, so its absence on an object-vocab merges-less config is diagnostic of a broken shape rather than of WordPiece. The prior negative test that pinned "vocab object without merges is rejected" was tightened to require `unk_token` also absent; a new positive test exercises the mBERT-canonical inline shape `{vocab: {...}, unk_token: "[UNK]", continuing_subword_prefix: "##", max_input_chars_per_word: 100}`, plus a companion test that exercises the serde defaults on the two optional fields via the untagged path. With this in place the `bert_base_multilingual_cased` conformance case runs 40/40 end-to-end against the real HF-hosted vocab when materialised under one of the two lookup roots (previously blocked 0/40 at load time). *Red flag surfaced during implementation:* a legitimate BPE config that happens to omit `"merges"` by author error but *does* carry an `"unk_token"` (BPE permits an optional `unk_token`) would now be misclassified as WordPiece rather than rejected. No such shape has been observed on the Hub — every real typeless-BPE checkpoint we know about (only `openai-community/gpt2` v1.0 today) carries its `merges` — and the misclassification would still surface at conversion time via a `to_wordpiece_tokenizer` failure downstream, so the risk is deemed acceptable for the corpus expansion this landing enables.

**Phase 6 — WordPiece + SentencePiece + full HF spec.** Done when `stringcheese-tokenizer-wordpiece` matches upstream WordPiece on BERT-family corpora; `stringcheese-tokenizer-sentencepiece` handles Unigram (T5, mBART) and BPE (Llama, Mistral) variants; the HF parser dispatches correctly across all four `model.type` values; the normalizer/pre-tokenizer/decoder stacks are covered.

*Progress note (Unigram pipeline composition landed).* `stringcheese-tokenizer-hf`'s `UnigramTokenizer` now composes the full SentencePiece pipeline — `Precompiled` charsmap normalization → `Metaspace` pre-tokenizer → per-piece Viterbi → `RobertaProcessing` / `TemplateProcessing` post-processor — and implements the `Tokenizer` trait with a matching `decode` that reverses the Metaspace substitution. `to_unigram_tokenizer` attaches each layer verbatim from the parsed `tokenizer.json` (unwrapping a single-entry `Sequence` around a `Metaspace` pre-tokenizer, as real XLM-RoBERTa / Llama / T5 configs sometimes ship). `PostProcessor::RobertaProcessing` covers the XLM-RoBERTa / RoBERTa CLS/SEP splice; every field mirrors HF's on-disk shape verbatim so the loader routes straight through. The 4-checkpoint conformance corpus at `crates/stringcheese-tokenizer-hf/tests/conformance/` now exercises `xlm_roberta_base.json` through the full pipeline instead of skipping the post-processor.

*Progress note (`BertProcessing` post-processor landed).* `PostProcessor::BertProcessing { sep: (String, TokenId), cls: (String, TokenId) }` materialises HF's stock-BERT post-processor tag — the fixed `[CLS] $A [SEP]` splice every BERT / `DistilBERT` / `MobileBERT` / `ALBERT` checkpoint ships. Distinct from `RobertaProcessing` in that it carries no `trim_offsets` / `add_prefix_space` fields (HF's own `BertProcessing` type has none either). The loader routes `{"type": "BertProcessing", "sep": ["[SEP]", 102], "cls": ["[CLS]", 101]}` straight through via `HfPostProcessor::BertProcessing`. This closes the "BertProcessing belongs with the WordPiece / BERT landing" note from the earlier deferred-variants section.

*Progress note (`Sequence` post-processor landed).* `PostProcessor::Sequence(Vec<PostProcessor>)` composes nested post-processors: the primary encoding is threaded through each child's `apply` call left to right, and the final encoding is returned. Nested `Sequence` values are permitted — each recursive `apply` re-enters the top-level dispatch, and the loader recursively materialises children via `to_runtime_post_processor`. An empty child list is treated as identity (matches HF's own behaviour). Together with the `BertProcessing` landing above this exhausts the deferred post-processor variants — every honoured HF post-processor tag (`TemplateProcessing`, `BertProcessing`, `RobertaProcessing`, `ByteLevel`, `Sequence`) now materialises at conversion time; only the truly exotic tags (`WordPieceProcessing`, custom callables) still surface as `HfConversionError::UnsupportedPostProcessor`.

*Progress note (`WordLevel` model type landed).* `stringcheese-tokenizer-hf` now materialises `model.type == "WordLevel"` end-to-end. The new [`WordLevelTokenizer`](../../crates/stringcheese-tokenizer-hf/src/wordlevel.rs) runtime reuses the crate's `BpeVocabulary` for its surface ↔ id bijection and exposes `normalize -> pre-tokenize -> per-word lookup -> post-process` with the same three optional slots the `WordPiece` and `Unigram` runtimes carry (`with_normalizer`, `with_pre_tokenizer`, `with_post_processor`). OOV words emit the configured `unk_token_id` when one is present and surface `WordLevelEncodeError::UnknownWord` otherwise. The HF loader adds `HfWordLevelModel` (typed `{vocab, unk_token}` shape), a matching `HfTokenizer::WordLevel` enum arm, and a `to_wordlevel_tokenizer` conversion; the pre-tokenizer routes `WhitespaceSplit` (default), `Whitespace`, and `BertPreTokenizer` (and single-child `Sequence` wrappers) into their runtime counterparts. `to_bpe_tokenizer` still rejects the model type — the error variant switches from the shared `UnsupportedModel` to the dedicated `UnsupportedModelForBpe` for parity with the `WordPiece` / `Unigram` dispatch surface. No real HF `WordLevel` checkpoint conformance fixture landed with this wave (the family is niche and no widely-shipped checkpoint we could find is small enough to embed in-tree without pulling real vocab bytes); the hand-crafted 5-word vocab in the runtime tests plus an inline `MINIMAL_WORDLEVEL_JSON` in the loader tests cover the encode / decode / dispatch surface.

*Progress note (Unigram `byte_fallback` landed).* `UnigramTokenizer` now honours SentencePiece's `byte_fallback: true` config: on construction, `to_unigram_tokenizer` scans the vocabulary for the 256 reserved `<0x00>`..`<0xFF>` surface strings and stores a `byte → id` map on the tokenizer; a missing token surfaces `HfConversionError::ByteFallbackTokensMissing { missing_count, first_missing_byte }` at conversion time rather than silently degrading at encode time. On the Viterbi path, an OOV character now routes through the byte-fallback lookup instead of the `UntokenizableChar` error — the character's UTF-8 bytes become a run of `<0xXX>` token ids, preferred over the `unk` fallback when both are configured (the ordering upstream SentencePiece uses). `UnigramTokenizer::decode` reverses this by accumulating consecutive byte-fallback ids into a UTF-8 buffer flushed as `String::from_utf8_lossy` when a non-byte-fallback token breaks the run. The scan works because every real SentencePiece vocab we surveyed (Llama-2 / Mistral / Qwen family) places its byte tokens at ids 3..258 with `<unk>`/`<s>`/`</s>` at 0..2 — but the runtime does not depend on that convention; it consumes whatever ids the scan finds. A new `unigram_byte_fallback_synth` conformance fixture (hand-crafted vocab shipped under `tests/conformance/vocabs/`) exercises 20 cases spanning ASCII OOV, 2/3/4-byte UTF-8 chars (Latin, Cyrillic, CJK, emoji), and mixed vocab+byte-fallback inputs; round-trip through `decode` reconstructs the original input for every case. *Red flag surfaced during implementation:* real Llama-2 and Mistral `tokenizer.json` blobs on the Hub today ship as `model.type == "BPE"` (not `"Unigram"`) with the same 256 byte-fallback tokens embedded — so this Unigram-runtime landing does not, on its own, wire byte-fallback for those checkpoints; the BPE-side of the same mechanism is a separate landing.

*Progress note (productionization surface landed — `encode_batch` / `encode_pair` / `truncate` / `pad_batch`).* The `Tokenizer` trait grew three default-implemented methods (`encode_batch`, `encode_pair`, `count_batch`) whose defaults preserve backward compat — the sequential `encode_batch` default and the default `encode_pair` that concatenates two independent encodings and tags them with `type_ids = [0, ..., 1, ...]` mean every existing implementor (`BpeTokenizer`, `WordPieceTokenizer`, `WordLevelTokenizer`, `UnigramTokenizer`, plus any downstream user impl) gets the new surface for free without a source-level break. The three subword runtimes plus `WordLevelTokenizer` each override `encode_pair` to route through the new `PostProcessor::apply_pair` which walks the `TemplateProcessing::pair` template (or the `BertProcessing` / `RobertaProcessing` fixed splices — `[CLS] A [SEP] B [SEP]` / `<s> A </s></s> B </s>`) and populates `type_ids` per template slot. `Encoding` grew two new fields — `type_ids: Vec<u32>` (segment id per token, `0`/`1` under HF's convention) and `attention_mask: Vec<bool>` (real token / pad token) — both default-empty so the trait-shape encode path stays byte-identical for callers that never opt into pair or padding. Truncation lives in a new `stringcheese_tokenizer::truncation` module with `TruncationDirection { Left, Right }`, `TruncationStrategy { LongestFirst, OnlyFirst, OnlySecond, DoNotTruncate }`, `TruncationConfig { max_length, strategy, direction, stride }`, and free functions `truncate<T>(&mut Encoding<T>, &TruncationConfig)` (single-encoding scoped) and `truncate_pair<T>(&mut Encoding<T>, &mut Encoding<T>, &TruncationConfig)` (pair-scoped). Padding lives in `stringcheese_tokenizer::padding` with `PaddingDirection { Left, Right }`, `PaddingStrategy { BatchLongest, Fixed(usize) }`, `PaddingConfig<Token> { strategy, pad_id, pad_type_id, direction }`, and free functions `pad_batch<T: Clone>(&mut [Encoding<T>], &PaddingConfig<T>)` (batch-scoped) plus `pad<T: Clone>(&mut Encoding<T>, target_len, &PaddingConfig<T>)` (single-encoding scoped) — every populated per-token array grows in lockstep and `attention_mask` is synthesised for real tokens even when the encoding arrived without one. Each runtime gained `.with_truncation()` / `.with_padding()` builder methods; when set, `encode` applies truncation post-post-processor and `encode_batch` applies padding after the batch is fully encoded. The HF loader parses the previously-verbatim `truncation` and `padding` blocks into typed `HfTruncationParams` / `HfPaddingParams` structs (mirroring HF's on-disk shape including the tagged `{"Fixed": N}` alternative of the padding strategy) and attaches them to the runtime tokenizer automatically at conversion time — a config with a `truncation` block truncates by default; a config with a `padding` block pads by default. Test coverage: unit tests in the trait crate for each truncation/padding function (single + pair, both directions, all strategies), unit tests in the HF crate for `apply_pair` on each `PostProcessor` variant, per-runtime tests for the trait-method overrides on `BpeTokenizer` / `WordPieceTokenizer`, and loader tests for the HF truncation/padding parse-and-apply path (including the `Fixed`-tagged-object variant of the padding strategy).

*Progress note (Llama-2 decoder chain landed).* `Decoder` (in `stringcheese-tokenizer-hf`'s BPE module) grew five new variants that mirror HF's own per-token decoder trait: `Sequence(Vec<Decoder>)` composes stages left to right, `Replace { pattern: String, content: String }` performs literal search-and-replace per token (regex patterns surface `HfConversionError::UnsupportedDecoder` — every real HF checkpoint's decoder-side `Replace` uses a literal, so this is a real-world no-op), `Fuse` collapses the token list into a single-entry list joined with the empty separator, `Strip { content: char, start: usize, stop: usize }` removes up to `start` leading and `stop` trailing occurrences of `content` from each token, and `ByteFallback` reassembles runs of `<0xXX>` surface strings into UTF-8 (with one U+FFFD per invalid byte, matching HF's own per-invalid-byte replacement policy). The chain path runs on `Vec<String>` throughout — one entry per input id — and joins with `""` at the end; `BpeTokenizer::decode` routes ids through the chain when any of the five new variants is configured (the model-side byte-fallback reassembly is bypassed on this branch so the chain's own `ByteFallback` stage does the work without double-decoding). `UnigramTokenizer` and `WordPieceTokenizer` grew matching `with_decoder(Decoder)` / `decoder()` builders and route through the chain identically when configured; a checkpoint with no `decoder` block — or with a shape this crate does not materialise (real xlm-roberta-base ships a `Metaspace` decoder that falls to `HfDecoder::Other`) — keeps the runtime's per-family default decode. The HF loader adds `HfDecoder::Sequence` / `Replace` / `Fuse` / `Strip` / `ByteFallback` variants (`Fuse` and `ByteFallback` are bare tags; `Strip` validates that `content` is a single Unicode scalar; `Replace` validates that `pattern` is `HfPattern::String`) and materialises them via a new `to_runtime_decoder` helper that returns `Ok(None)` on unrecognised tags so forward-compat loading never fails on a decoder shape. The full Llama-2 chain `Sequence[Replace(▁→ ), ByteFallback, Fuse, Strip(' ', 1, 0)]` — every real Llama-2 / Mistral / Qwen `tokenizer.json` ships this exact shape — now materialises byte-for-byte into `Decoder::Sequence` and produces upstream-parity output. Test coverage: 14 new unit tests in `bpe.rs` (one per new variant plus the full Llama-2 chain plus a `BpeTokenizer::decode` end-to-end), 7 new HF-loader tests in `hf.rs` (the Llama-2 shape parses into the expected typed sequence, materialises into the runtime chain, each variant round-trips through parse+convert, malformed shapes surface `UnsupportedDecoder`, and unknown tags soft-fail); the `llama_2_7b.json` conformance fixture grew an `expected_decoded` field on each of the 20 cases (values captured under `.decode(ids, skip_special_tokens=False, clean_up_tokenization_spaces=False)` so the runner exercises the chain with the leading BOS `<s>` still in the id list), and the conformance runner asserts `tok.decode(expected_ids) == expected_decoded` whenever the field is present. The GPT-2 conformance path stays on `Decoder::ByteLevel` (the legacy byte-buffer decode); `Fuse` / `Strip` are not needed for byte-level BPE because the ByteLevel bijection already reverses per-byte-encoded strings. No decode-time interaction with the model-side byte-fallback beyond the routing switch: the chain path never touches `byte_fallback_byte_for`, so the model-side reassembly and the chain-side reassembly never run against the same run of ids.

*Progress note (BPE `byte_fallback` landed).* `BpeTokenizer` now mirrors the Unigram-side landing: a new `with_byte_fallback([TokenId; 256])` builder + `byte_fallback_enabled()` accessor + `byte_fallback: Option<Box<[TokenId; 256]>>` field. `to_bpe_tokenizer` scans the vocabulary for the same 256 reserved `<0xXX>` surface strings (uppercase or lowercase hex both accepted, matching the Unigram scan) and surfaces `HfConversionError::ByteFallbackTokensMissing` at conversion time when any are absent. The encode path changed shape: when byte-fallback is on we seed pieces per Unicode scalar value (character-BPE, not byte-BPE — the merge-loop delta is the same one the byte-level ByteLevel pipeline already used, so seeding per char was already an established branch of `encode_region_bpe`). After the merge loop, any surviving piece whose bytes are not in the vocab is fanned out into one reserved `<0xXX>` id per byte in forward order; the emitted offset range for each fanned id is one byte in the input. `BpeTokenizer::decode` accumulates consecutive byte-fallback ids and flushes as `String::from_utf8_lossy` — malformed runs become U+FFFD rather than failing the whole decode, matching the Unigram-side flush shape. A new `bpe_byte_fallback_synth` fixture (hand-crafted 276-entry character-BPE vocab, mirroring the Unigram-side synth fixture) exercises 20 cases end-to-end through the conformance runner (20/20 pass); a real-vocab `llama_2_7b` fixture with 20 diverse byte-fallback inputs (emoji / CJK / rare Unicode / mixed word + emoji) matches `transformers.AutoTokenizer.from_pretrained('NousResearch/Llama-2-7b-hf')` byte-for-byte (20/20 pass under `--features parity-real-vocab` with the real 1.8 MB `tokenizer.json` present under one of the two lookup roots — the fixture soft-skips otherwise, per the runner's convention). *Real Llama-2 loads end-to-end through `to_bpe_tokenizer`:* the shipped config carries `pre_tokenizer: null` (not the Metaspace we feared going in), a `Sequence[Prepend("▁"), Replace(" ", "▁")]` normalizer we already materialise verbatim, and a `TemplateProcessing` post-processor we already honour. Only the empty-input edge case (Llama-2's own fast tokenizer suppresses the bare `▁` from Prepend for pure-empty inputs; we emit both `<s>` and `▁`) was excluded from the fixture — an unrelated normalizer nuance orthogonal to the byte-fallback landing. Merge-loop integration was cleaner than expected: the existing per-char seed branch (used by the byte-level pipeline) generalised to byte-fallback with a single-line condition change, and the post-merge lookup gained one `else if let Some(bf) = ...` arm without touching the merge arena bookkeeping at all.

*Progress note (`Normalizer::Sequence` audit + `Strip` field-name fix landed).* An end-to-end audit confirmed `Normalizer::Sequence(Vec<Normalizer>)` was already fully wired: the runtime `normalize` fold threads text through each child left-to-right (empty sequence is identity, nested sequences flatten by transitive application), the loader parses `{"type": "Sequence", "normalizers": [...]}` via `HfNormalizer::Sequence { normalizers: Vec<HfNormalizer> }`, and `to_runtime_normalizer` recurses to materialise nested children. What the audit *did* uncover — and this landing fixes — is that `HfNormalizer::Strip`'s serde field names were `left` / `right` while HF's on-disk shape uses `strip_left` / `strip_right` (matching upstream [`tokenizers-rs`'s own `Strip` normalizer](https://github.com/huggingface/tokenizers/blob/main/tokenizers/src/normalizers/strip.rs)). Serde silently dropped the on-disk names and defaulted both to `true`, so a config that shipped `strip_left: false, strip_right: true` (deberta-v3-base / mdeberta-v3-base's `Sequence[Replace(Regex), NFC, Strip{strip_right}]`) actually got two-sided strip at runtime. The variant fields are renamed to `strip_left` / `strip_right` with `#[serde(rename = ..., alias = "left")]` / `alias = "right"` for backward compat, so both spellings still parse. New tests: `normalizer_deberta_v3_real_stack_loads_and_normalizes` (inline mini config, exact `Sequence[Replace(Regex), NFC, Strip{strip_right}]` stack, asserts both loader-side field parsing and runtime `Sequence`-of-three materialisation), `normalizer_strip_accepts_short_field_alias` (the `left` / `right` alias path), `normalizer_nested_sequence_loads_and_flattens_at_runtime` (loader nested-Sequence), `normalizer_empty_sequence_loads_as_identity`. Real-vocab conformance against `microsoft/deberta-v3-base` and `microsoft/mdeberta-v3-base` (produced via `transformers.AutoTokenizer.from_pretrained(...).save_pretrained(...)` on `transformers==5.14.1`; deberta-v3 does not publish a `tokenizer.json` directly): both configs now **load** end-to-end (previously failed at load with `UnsupportedNormalizer{type_name:"Replace(Regex)"}`), and encode 37/40 fixture cases correctly. The three remaining failures are all Metaspace-related, not Sequence-related: (1) case `whitespace-heavy` — Metaspace(prepend_scheme=always) emits an extra `▁` token when input begins with whitespace surviving the Replace(Regex) collapse (previously masked by the `strip_left` default-true bug, now exposed); (2) cases `cls-surface-form-raw` / `mask-surface-form-raw` — Metaspace emits spurious `▁` markers *around* pre-extracted special tokens (`[CLS]`, `[SEP]`, `[MASK]`) instead of treating the special token's boundary as suppressing the metaspace prefix on the neighbouring word. Both classes are candidates for a separate Metaspace/special-token-interaction landing.

**Phase 7 — WIT interface + component-model integration; optional `hf-native` adapter.** Done when `component/wit/tokenizer/stringcheese-tokenizer.wit` parses under `wit-parser`, `wit-bindgen` produces a clean Rust host binding, and a reference `tokenizer-provider` component built from `CL100K_BASE` echoes correct encodings across the boundary under `wasmtime`; `stringcheese-tokenizer-hf-native` is published as a feature-gated adapter that delegates to `tokenizers-rs` for native targets.

*Progress note (WIT interface + reference component landed).* [`component/wit/tokenizer/stringcheese-tokenizer.wit`](../../component/wit/tokenizer/stringcheese-tokenizer.wit) ships the canonical `tegmentum:tokenizer@0.1.0` package with a single `interface tokenizer` exporting `encode` / `decode` / `count` / `get-capabilities`, plus the shared `encoding` record (parallel `ids` / `offsets` / `special-mask` / `type-ids` / `attention-mask` arrays mirroring `stringcheese_tokenizer::Encoding`), a typed `tokenizer-error` variant, and a `capabilities` record carrying `model-type` / `variant-id` / `version` / `vocab-size` / `has-byte-fallback` / `has-special-tokens`. The WIT parses cleanly under `wit-parser` — asserted by a plain host unit test in the new [`stringcheese-tokenizer-component`](../../crates/stringcheese-tokenizer-component/) crate — and `wit-bindgen` produces a clean Rust host binding shipped verbatim as `src/bindings.rs` (regenerated with `wit-bindgen rust --runtime-path wit_bindgen_rt` so the runtime path matches the wit-bindgen-rt 0.44 dep the workspace already carries for the detect-* WIT components). The reference `tokenizer-provider` component builds under `cargo build -p stringcheese-tokenizer-component --target wasm32-wasip1 --features wit-component --release` — 241 KB raw core module → 262 KB after `wasm-tools component new --adapt wasi_snapshot_preview1.reactor.wasm` → 161 KB after `wasm-opt -Oz`. Under `wasmtime run --invoke`, `encode("hello")` returns the correct `ok({ids: [260], offsets: [{start: 0, end: 5}], ...})` for the hand-crafted 261-token character-BPE vocab embedded in the crate, and `decode([260])` round-trips to `ok("hello")`. Four wasmtime-driven integration smoke tests (`tests/component_smoke.rs`) exercise each of the four exports end-to-end; a fifth unit test asserts the on-disk WIT parses under `wit-parser` on every host `cargo test` run. The reference vocab uses a hand-crafted character-BPE seed (byte alphabet at 0..=255 plus five merged pieces + one `<|endoftext|>` special) rather than `CL100K_BASE` bytes — the workspace's session-standing constraint (see `CLAUDE.md`) forbids committing real OpenAI / HF vocab bytes into the tree; a follow-on `stringcheese-tokenizer-component-cl100k` (sibling crate) can layer the real vocab in via the same feature-gate pattern once the CDN-fetched blobs become build-time inputs. Feature-gating follows the `stringcheese-detect-{script,whatlang,lingua}` precedent: the WIT `Guest` impl and `bindings` module sit behind `cfg(all(target_family = "wasm", feature = "wit-component"))` so that an umbrella-crate build that links this crate as a plain `rlib` alongside other WIT components does not hit duplicate-export symbol errors at link time. A new `wasm-tokenizer-component` CI job builds the component and runs the wasmtime smoke test on every PR (pinned wasmtime 26.0.0 + wasm-tools 1.219.0; `continue-on-error: true` while the job stabilises, matching the wasm-runtime job's initial posture).

*Progress note (wasmtime smoke tests migrated to the in-process component API).* The four `tests/component_smoke.rs` end-to-end exports checks originally shelled out to `wasmtime run --invoke <expr>` on the componentised `.wasm`. That path stopped working when wasmtime 20+ removed `--invoke` for components (only core modules still accept it); the tests were `#[ignore]`-marked as a stop-gap. This landing rewrites them against the `wasmtime` crate as a dev-dep (`wasmtime = "26"`, `wasmtime-wasi = "26"`, both feature-trimmed to `runtime` + `cranelift` + `component-model` + `std` — no async fibers / pooling allocator / rayon / gc / wat), pinned to the same 26.x line the CI job already installs. Typed bindings are generated at test-compile time by `wasmtime::component::bindgen!({ world: "tokenizer-provider", path: "../../component/wit/tokenizer" })`, and each of the four tests calls the corresponding export through the typed handle (`call_encode` / `call_decode` / `call_count` / `call_get_capabilities`) and asserts on the returned Rust value (`Encoding` record for encode, `String` for decode, `u32` for count, `Capabilities` record for get-capabilities) — no more wave-string parsing. Trade-offs: the dev-dep graph grows by ~120 crates (cranelift, regalloc2, target-lexicon, wasmparser, wat, and friends) with a one-time cold-build cost of ~30s at the crate level; the runtime dep graph is unchanged (both new deps are `[dev-dependencies]` only). The `#[ignore]` attributes are removed, so `cargo test -p stringcheese-tokenizer-component --all-features --locked` now actually runs the four smoke tests on every host `cargo test` invocation.

*Progress note (`stringcheese-tokenizer-hf-native` real-vocab conformance landed).* The adapter now carries its own conformance runner ([`crates/stringcheese-tokenizer-hf-native/tests/conformance.rs`](../../crates/stringcheese-tokenizer-hf-native/tests/conformance.rs)) mirroring the sibling `stringcheese-tokenizer-hf` runner one-to-one: same 15 registered fixtures (`gpt2`, `cl100k_base`, `bert-base-uncased`, `xlm-roberta-base`, `distilbert-base-uncased`, `roberta-base`, `bert-base-multilingual-cased`, `bart-base`, `deberta-v3-base`, `mdeberta-v3-base`, `bpe-byte-fallback-synth`, `unigram-byte-fallback-synth`, `llama-2-7b-hf`, `mistral-7b-v0.1`, `qwen2-7b`), same `parity-real-vocab` feature-gate, same `$STRINGCHEESE_REAL_VOCABS_DIR` env-var lookup. Fixture JSON files and the local-fallback `vocabs/` directory are *not* duplicated — the runner reads both directly from the sibling crate's `tests/conformance/` tree via a relative path anchored at `CARGO_MANIFEST_DIR`, so a new fixture landing in the sibling automatically becomes visible here (a `fixtures_in_sync_with_sibling` meta-test asserts the equality between the sibling's fixture list on disk and this crate's `REGISTERED_FIXTURES`). The fixture-format loader itself is duplicated (`load_fixture`, `find_tokenizer_json`, `run_cases`) rather than lifted into a helper crate — those helpers live inside an integration test binary in the sibling and are not part of a public module. On a naked checkout the 15 per-fixture tests soft-skip and the meta-test passes; with the sibling crate's two committed synth vocabs on disk, 2/15 activate and pass byte-identically (16 tests total, 0 failed). With real HF `tokenizer.json` files provisioned at `$STRINGCHEESE_REAL_VOCABS_DIR`, additional fixtures activate — since this crate delegates to `tokenizers-rs` verbatim and `transformers.AutoTokenizer` (the fixture reference tool) is itself a Python binding over the same crate, a fixture failure surfaces a Python-wrapper-vs-Rust-surface divergence in the sibling's recorded reference ids rather than a bug in this crate. A new `hf-native-parity` CI job (`continue-on-error: true` for CDN outages) fetches a curated set of freely-accessible HF vocabs from `https://huggingface.co/<repo>/resolve/main/tokenizer.json` into a per-job cache dir and runs the suite under `--features parity-real-vocab`; every fixture whose vocab was successfully fetched executes.

*Progress note (`stringcheese-tokenizer-hf-native` adapter landed).* The native-only `hf-native` half of Phase 7 is shipped as [`stringcheese-tokenizer-hf-native`](../../crates/stringcheese-tokenizer-hf-native/): a thin adapter that wraps `tokenizers::Tokenizer` (Hugging Face's own tokenizers-rs) and implements the toolkit's own `stringcheese_tokenizer::Tokenizer` trait, delegating `encode` / `decode` / `encode_batch` / `encode_pair` / `count` byte-identically to upstream. The Encoding-shape mismatch is handled at the boundary — HF's `Encoding::get_ids` / `get_offsets` / `get_special_tokens_mask` / `get_type_ids` / `get_attention_mask` map straight across; HF's extra parallel arrays (`tokens`, `word_ids`, `overflowing`) are dropped and callers who need them reach into `HfNativeTokenizer::inner()`. Constructors mirror upstream: `HfNativeTokenizer::from_file(path)` / `from_bytes(bytes)` wrap the corresponding upstream loaders; `from_inner(HfLibTokenizer)` accepts an already-built value; `with_add_special_tokens(bool)` toggles the post-processor's specials-insertion policy on encode. The crate is native-only by construction — a top-level `#![cfg(not(target_family = "wasm"))]` guard compiles it to an empty module on wasm targets, and the tokenizers-rs dependency is target-gated to `cfg(not(target_family = "wasm"))` so a wasm build of any consumer never resolves it — because tokenizers-rs itself pulls `onig` (C library, PCRE-shaped regex engine) and `esaxx-rs` (C++ header-only suffix automaton) on its default feature set, neither of which cross-compiles to `wasm32-unknown-unknown`. Dependency footprint: ~80 unique transitive crates including `rayon`, `spm_precompiled`, `unicode-normalization-alignments`, `onig_sys` + `onig`, `esaxx-rs`, `nom`, `derive_builder`, and `monostate`; the `tokenizers` rlib alone weighs ~44 MB in a debug build — orders of magnitude larger than the wasm-friendly [`stringcheese-tokenizer-hf`](../../crates/stringcheese-tokenizer-hf/) alternative, which is the whole reason `hf-native` exists as a separate opt-in crate rather than a feature flag on the primary crate. Test coverage: 11 unit tests + 1 proptest exercising byte-identical parity vs upstream. Together with the WIT-tokenizer-component landing above, both halves of Phase 7's escape hatches now ship — native callers who value byte-identical HF parity above wasm portability get `hf-native`; wasm-first callers who want a language-and-runtime-agnostic component-model boundary get the WIT component.

*Progress note (real-vocab `cl100k_base` WIT component landed).* The follow-on the reference-component note called out — layering the real `cl100k_base` bytes into the same WIT `tokenizer-provider` shape via a feature-gated sibling crate — is now shipped as [`stringcheese-tokenizer-component-cl100k`](../../crates/stringcheese-tokenizer-component-cl100k/). The crate lives outside the top-level workspace (empty `[workspace]` at the top of its `Cargo.toml`, plus an `exclude` entry in the root manifest) so its opt-in `parity-real-vocab` feature never leaks into `cargo test --workspace --all-features` — the same isolation posture the sibling [`stringcheese-tokenizer-tiktoken-conformance`](../../crates/stringcheese-tokenizer-tiktoken-conformance/) parity harness takes for the same reason. Real bytes never enter the repo: the crate's `build.rs` walks a resolution list (`$STRINGCHEESE_CL100K_TIKTOKEN` → `$TIKTOKEN_PARITY_DATA_DIR/cl100k_base.tiktoken` → `$XDG_CACHE_HOME/stringcheese-tokenizer-tiktoken/cl100k_base.tiktoken` → `$HOME/.cache/stringcheese-tokenizer-tiktoken/cl100k_base.tiktoken` → `%LOCALAPPDATA%\stringcheese-tokenizer-tiktoken\cl100k_base.tiktoken`), verifies the plaintext blob's SHA-256 against the same `223921b76ee99bde995b7ff738513eef100fb51d18c93597a113bcffe865b2a7` constant the conformance harness pins in `variant.rs`, stages the bytes under `$OUT_DIR/cl100k_base.tiktoken`, and emits `--cfg=stringcheese_cl100k_real_vocab` so the runtime library switches from stub mode to the real tokenizer. Runtime construction reuses `stringcheese_tokenizer_tiktoken::builder::build_scud_from_tiktoken` (already exposed for the SCUD-lite pack path) to transcode plaintext to `(BpeVocabulary, BpeMergeTable)` and pairs it with `RegexPreTokenizer::new(TIKTOKEN_CANONICAL_PATTERN)` from `stringcheese-tokenizer-hf` — identical to the setup the parity harness proves is byte-equal to upstream. The stub mode (default features, no cache blob) writes an empty `$OUT_DIR/cl100k_base.tiktoken`, does not emit the cfg, and every `encode` / `decode` / `count` call returns `Cl100kTokenizerError::Other` naming the `parity-real-vocab` feature. Feature layering mirrors the reference crate: `wit-component` requires `parity-real-vocab` (a stub component would be pointless) and is itself gated on `cfg(all(target_family = "wasm", feature = "wit-component"))` so the crate can be linked as a plain `rlib` sibling without duplicate WIT-export symbols. Six host-side smoke tests (`tests/real_vocab_smoke.rs`) assert byte-identical parity with upstream tiktoken: `encode("Hello, world!") == [9906, 11, 1917, 0]`, `encode("hello") == [15339]`, empty-input handling, count↔encode agreement across ASCII + combining-accent inputs, decode round-trip, and the capabilities report. Local build-and-verify measurements (aarch64 macOS, Rust 1.97.1): raw core module 3,079,672 bytes; componentized (via `wasm-tools component new --adapt wasi_snapshot_preview1.reactor.wasm`) 3,102,400 bytes; module optimised with `wasm-opt -Oz --enable-bulk-memory` before componentization 2,792,000 bytes — the bulk (~1.68 MB) is the embedded plaintext blob itself, which does not compress under `wasm-opt`. A new `wasm-tokenizer-component-cl100k` CI job (marked `continue-on-error: true` to match the tiktoken-parity job's non-blocking posture while CDN dependencies stabilise) fetches the vocab via the conformance harness, builds the component under `wasm32-wasip1`, reports the raw + `wasm-opt`-optimised sizes, and runs the host-side smoke test. Follow-on siblings for `o200k_base` and per-model HF packs can layer over the same shape; the crate is the template for every "real vocab in the WIT boundary" packaging that follows.

Each phase produces a shippable artifact. Phase 1 alone gives callers a usable segmenter crate; phase 2 alone gives them a working BPE algorithm they can drive with their own tables.

---

## 12. Alternatives considered

- **Just use `tokenizers-rs`.** Discussed in §1.2. Right for native callers who accept its dependency profile and online-first defaults; wrong for a Wasm-first, offline-first, StringCheese-surface-integrated toolkit. The optional `stringcheese-tokenizer-hf-native` adapter serves the boundary case.
- **Build the subsystem inside `stringcheese-manip` as a sub-module.** Rejected: `Segmenter` and `Tokenizer` are foundational traits that `compare`, `cdc`, `index`, and `lang` depend on — burying them in `manip` inverts the dependency direction. The model packs also carry data payloads that should not be a `manip` transitive dependency.
- **Use ICU4X's `SegmenterProvider`.** Right for Unicode segmentation and the correct choice for a caller who already depends on ICU4X. Does not cover model tokenizers — BPE / WordPiece / SentencePiece are not in ICU4X's charter — so it solves at most half the problem. The Tier-1 `WordSegmenter` / `SentenceSegmenter` can delegate to ICU4X behind a feature gate where the two overlap.
- **Static-linked tokenizer data.** One tiktoken variant is 250–800 KB after SCUD + Brotli; four is 1–3 MB. Baking that into every consumer defeats the Wasm-first commitment. The per-pack Cargo feature is non-negotiable.
- **Drop `regex` for `regex-lite`.** Attractive for size but `regex-lite` does not support the Unicode property classes tiktoken's pre-tokenizer uses. A pluggable regex trait lets callers pick.
- **Skip byte-offset tracking to shrink the encoding record.** Rejected (see §8). The information is impossible to reconstruct downstream; carrying it always is the correct default. Callers who never inspect offsets can call `count` instead.

---

## 13. Open questions

- **Where do the trait definitions live long-term?** `stringcheese-tokenizer` for now. Once the subsystem stabilises, promote `Tokenizer` and `Segmenter` into `stringcheese-core` alongside `Sequence` so every substrate consumer can bound on them without depending on the tokenizer crate. Risk: churn to the foundation.
- **Shipping the SCUD build tool alongside the runtime crate, or pre-built SCUD only?** For tiktoken, the build tool is a workspace binary. For third-party pack contributors we may need to publish it as a standalone `cargo install`-able binary; the design does not yet commit either way.
- **Concurrent-tokenizer negotiation.** A caller who wants to compare two prompts under *different* tokenizers has two encodings with incompatible ID spaces. Do we expose a `TokenizerFamily` marker that makes cross-family algorithm invocations a compile error, or is the answer "callers compare on surface bytes when the tokenizers differ"? Load-bearing for multi-model workflows.
- **Actual SCUD compression ratios.** The 250–400 KB / 500–800 KB targets in §5.2 are back-of-envelope. Real measurements per variant are a release-gate item; the doc commits to reporting the *worst* observed ratio rather than a hero number.
- **Borrowed vs owned segmenter output.** `WhitespaceTokenizer` yields `&'a str` and allocates nothing; a `LowercasingSegmenter` must yield an owned `String`. The trait accommodates both via `Unit<'a>`, but the ergonomics of `dyn Segmenter` erase the GAT. May need a `BorrowedSegmenter` sub-trait for the zero-alloc path.
- **Shared WIT world with the i18n subsystem.** The tokenizer WIT world and `stringcheese-icu-break` both surface segmentation-like operations. Should they share a common `segmentation` interface both worlds `use`? Cleaner in the abstract; risks over-generalising two subsystems that share a surface but not a lifecycle.
- **Loader sharing.** The tokenizer subsystem reuses `stringcheese-scud` from the i18n subsystem. If that dependency edge proves awkward (i18n ships on its own cadence, tokenizer needs SCUD sooner), `stringcheese-scud` may need to hoist out of the i18n design into its own top-level crate.
- **Default special-token policy.** tiktoken's default is forbid-by-default at the API surface; Hugging Face's default *adds* BOS/EOS/CLS/SEP based on the post-processor template. The `special-policy` variant captures the surface but the default value is a policy call.
- **Model-vocabulary hash surfacing.** Should `capabilities.variant-id` return an opaque name (`"cl100k_base"`) or a content hash of the merge table + vocabulary? The latter is stronger for correctness (two packs from different sources are guaranteed equivalent iff their hashes match) but noisier for humans.

---

## 14. Cross-references

- Umbrella charter for the representation-layer commitment: [DESIGN.md § Vision](../DESIGN.md#vision), [DESIGN.md § Representation Layers](../DESIGN.md#representation-layers), [DESIGN.md § N-Grams](../DESIGN.md#n-grams).
- Sequence abstractions the tokenizer output flows into: [`crates/stringcheese-core/src/sequence.rs`](../../crates/stringcheese-core/src/sequence.rs).
- Existing simple word tokenizer for language packs: [`crates/stringcheese-lang/src/tokenizer.rs`](../../crates/stringcheese-lang/src/tokenizer.rs).
- English pack composition today: [`crates/stringcheese-en/src/lib.rs`](../../crates/stringcheese-en/src/lib.rs).
- Deferred UAX #29 splits the tokenizer built-ins will unlock: [`crates/stringcheese-manip/src/split/mod.rs`](../../crates/stringcheese-manip/src/split/mod.rs).
- N-gram representation surface that plugs directly into token-level sequences: [`crates/stringcheese-compare/src/ngram/mod.rs`](../../crates/stringcheese-compare/src/ngram/mod.rs).
- SCUD format the BPE / WordPiece / Unigram data packs extend: [wit-i18n.md § 4](./wit-i18n.md#4-scud-data-pack-format-spec).
- Preprocessing pipeline consumers of the `Tokenize` stage: [preprocessing-pipeline.md](./preprocessing-pipeline.md).
- WIT interface conventions the tokenizer WIT world follows: [wasm-and-wit-interface.md](./wasm-and-wit-interface.md).
- Result-type conventions (why `tokenizer-error` is a variant, not a string): [type-system.md](./type-system.md).

## 15. Prior art and references

- **BPE.** Sennrich, R., Haddow, B., & Birch, A. (2016). "Neural Machine Translation of Rare Words with Subword Units." *ACL 2016*, 1715–1725. arXiv:1508.07909, <https://arxiv.org/abs/1508.07909>. Adapts Gage (1994) below to NLP.
- **Gage, P. (1994).** "A New Algorithm for Data Compression." *The C Users Journal*, 12(2), 23–38. The original byte-pair-encoding compression algorithm.
- **WordPiece.** Schuster, M., & Nakajima, K. (2012). "Japanese and Korean voice search." *ICASSP 2012*, 5149–5152. DOI: <https://doi.org/10.1109/ICASSP.2012.6289079>. Popularised for NMT by Wu et al. (2016), "Google's Neural Machine Translation System," arXiv:1609.08144.
- **SentencePiece.** Kudo, T., & Richardson, J. (2018). "SentencePiece: A simple and language independent subword tokenizer." *EMNLP 2018 demo*, 66–71. arXiv:1808.06226, <https://arxiv.org/abs/1808.06226>. Unigram variant: Kudo, T. (2018), "Subword Regularization," arXiv:1804.10959.
- **tiktoken.** OpenAI's BPE tokenizer, <https://github.com/openai/tiktoken>. Reference for `cl100k_base`, `o200k_base`, and the pre-tokenizer regex.
- **tokenizers-rs.** Hugging Face's tokenizer library, <https://github.com/huggingface/tokenizers>. Reference for the `tokenizers.json` spec and the normalizer/pre-tokenizer/decoder composition model.
- **`tokenizers.json` spec.** <https://huggingface.co/docs/tokenizers/api/tokenizer>.
- **UAX #29 (Text Segmentation).** <https://www.unicode.org/reports/tr29/>. Grapheme, word, sentence break rules.
- **BCP 47 (language tags).** RFC 5646, <https://datatracker.ietf.org/doc/html/rfc5646>. Used at the language-pack integration boundary.
- **WebAssembly Component Model.** <https://github.com/WebAssembly/component-model>. **WIT format:** <https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md>.
- **Regex crates.** `regex`, <https://docs.rs/regex/>; `regex-lite`, <https://docs.rs/regex-lite/>. The pluggable regex-backend discussion is in §12.

Citations of primary sources (arXiv, DOI-bearing publications, official specs) are preferred over pre-trained knowledge; where the primary source is a corporate open-source repository, the repository URL is given directly.
