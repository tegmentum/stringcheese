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
// component/wit/stringcheese-tokenizer.wit

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

**Phase 4 — SCUD extension for BPE data packs.** Done when `cap-id = BPE_` is registered in the SCUD spec, the loader validates the BPE header, compression ratios are *measured* per variant and reported (§13), and `stringcheese-tokenizer-hf-build` regenerates SCUD packs from tiktoken's upstream blobs deterministically.

**Phase 5 — HF `tokenizers.json` parser, BPE-only.** Done when the parser accepts every `tokenizer.json` in a corpus of the top ~30 HF models (Llama, Mistral, Qwen, Phi, DeepSeek, ...) whose `model.type` is `BPE`; matches upstream `tokenizers-rs` output on ≥ 1 000 sample inputs per model; the `hf-to-scud` build tool converts a `tokenizer.json` to a SCUD pack that loads under `BpeTokenizer::from_scud`.

*Progress note (`ByteLevel` post-processor landed).* GPT-2 `tokenizer.json` blobs ship a `ByteLevel` post-processor whose sole runtime effect (in HF's own crate) is to trim leading `Ġ` characters from *character-space* offset spans when `trim_offsets: true`. This crate reports byte offsets rather than character offsets, and HF's own `process()` ignores the post-processor's `add_prefix_space` and `use_regex` fields (the pre-tokenizer's `add_prefix_space` is the one that governs encoding end-to-end). The `ByteLevel` post-processor is therefore materialised as a typed variant on `PostProcessor::ByteLevel` and applied as a **pure no-op** on both `ids` and `offsets` — the loader accepts the config, and `encode()` produces byte-identical output to a config with the post-processor omitted. This unblocks the four-checkpoint conformance corpus's `gpt2` case, which used to panic at load with `UnsupportedPostProcessor { type_name: "Other" }`.

*Progress note (untagged `model` fallback landed).* `openai-community/gpt2/tokenizer.json` v1.0 — still the shipped GPT-2 config — omits the `"type"` field on its `model` node, mirroring HF's own internal `#[serde(untagged)]` autodetection. Our `HfModel` used to require the tag and failed the whole parse with `missing field "type"` at load time, blocking the conformance corpus's `gpt2` case one layer earlier than the `ByteLevel` post-processor. The fix keeps HF's canonical tagged shape as the primary path (a small internal `HfModelTagged` mirrors the four `"BPE"` / `"WordPiece"` / `"Unigram"` / `"WordLevel"` variants) and adds a hand-rolled `Deserialize` on `HfModel` that falls back to [`HfBpeModel`] when the input has no `"type"` field **and** carries both a JSON-object `"vocab"` and an array `"merges"`. A typeless Unigram- or WordPiece-shape node is rejected explicitly rather than silently misclassified — no HF release ships those without the tag, so a missing tag on that shape is a signal of a malformed config, not a normal case to support. With this in place the `gpt2` conformance case runs 20/20 end-to-end against the real HF-hosted vocab.

**Phase 6 — WordPiece + SentencePiece + full HF spec.** Done when `stringcheese-tokenizer-wordpiece` matches upstream WordPiece on BERT-family corpora; `stringcheese-tokenizer-sentencepiece` handles Unigram (T5, mBART) and BPE (Llama, Mistral) variants; the HF parser dispatches correctly across all four `model.type` values; the normalizer/pre-tokenizer/decoder stacks are covered.

*Progress note (Unigram pipeline composition landed).* `stringcheese-tokenizer-hf`'s `UnigramTokenizer` now composes the full SentencePiece pipeline — `Precompiled` charsmap normalization → `Metaspace` pre-tokenizer → per-piece Viterbi → `RobertaProcessing` / `TemplateProcessing` post-processor — and implements the `Tokenizer` trait with a matching `decode` that reverses the Metaspace substitution. `to_unigram_tokenizer` attaches each layer verbatim from the parsed `tokenizer.json` (unwrapping a single-entry `Sequence` around a `Metaspace` pre-tokenizer, as real XLM-RoBERTa / Llama / T5 configs sometimes ship). `PostProcessor::RobertaProcessing` covers the XLM-RoBERTa / RoBERTa CLS/SEP splice; every field mirrors HF's on-disk shape verbatim so the loader routes straight through. The 4-checkpoint conformance corpus at `crates/stringcheese-tokenizer-hf/tests/conformance/` now exercises `xlm_roberta_base.json` through the full pipeline instead of skipping the post-processor.

*Progress note (Unigram `byte_fallback` landed).* `UnigramTokenizer` now honours SentencePiece's `byte_fallback: true` config: on construction, `to_unigram_tokenizer` scans the vocabulary for the 256 reserved `<0x00>`..`<0xFF>` surface strings and stores a `byte → id` map on the tokenizer; a missing token surfaces `HfConversionError::ByteFallbackTokensMissing { missing_count, first_missing_byte }` at conversion time rather than silently degrading at encode time. On the Viterbi path, an OOV character now routes through the byte-fallback lookup instead of the `UntokenizableChar` error — the character's UTF-8 bytes become a run of `<0xXX>` token ids, preferred over the `unk` fallback when both are configured (the ordering upstream SentencePiece uses). `UnigramTokenizer::decode` reverses this by accumulating consecutive byte-fallback ids into a UTF-8 buffer flushed as `String::from_utf8_lossy` when a non-byte-fallback token breaks the run. The scan works because every real SentencePiece vocab we surveyed (Llama-2 / Mistral / Qwen family) places its byte tokens at ids 3..258 with `<unk>`/`<s>`/`</s>` at 0..2 — but the runtime does not depend on that convention; it consumes whatever ids the scan finds. A new `unigram_byte_fallback_synth` conformance fixture (hand-crafted vocab shipped under `tests/conformance/vocabs/`) exercises 20 cases spanning ASCII OOV, 2/3/4-byte UTF-8 chars (Latin, Cyrillic, CJK, emoji), and mixed vocab+byte-fallback inputs; round-trip through `decode` reconstructs the original input for every case. *Red flag surfaced during implementation:* real Llama-2 and Mistral `tokenizer.json` blobs on the Hub today ship as `model.type == "BPE"` (not `"Unigram"`) with the same 256 byte-fallback tokens embedded — so this Unigram-runtime landing does not, on its own, wire byte-fallback for those checkpoints; the BPE-side of the same mechanism is a separate landing.

**Phase 7 — WIT interface + component-model integration; optional `hf-native` adapter.** Done when `component/wit/stringcheese-tokenizer.wit` parses under `wit-parser`, `wit-bindgen` produces a clean Rust host binding, and a reference `tokenizer-provider` component built from `CL100K_BASE` echoes correct encodings across the boundary under `wasmtime`; `stringcheese-tokenizer-hf-native` is published as a feature-gated adapter that delegates to `tokenizers-rs` for native targets.

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
