# StringCheese scope and decomposition

## Purpose

This document draws the boundary of what StringCheese covers and lays
out a decomposition that keeps the library cohesive as it grows. It
also captures the regex / pattern-matching subsystem as the specific
piece the current architecture is most obviously missing.

## Boundary

StringCheese covers operations whose primary input and output are
**strings or string-derived structure** — bytes, code points, grapheme
clusters, tokens, sequences, and the intermediate representations
those operations flow through. It stops at operations whose purpose is
to **infer what real-world thing a piece of text refers to**. Entity
resolution crosses that boundary and lives in its own separate library.

Entity resolution is the clearest example of what does not belong, but
the same criterion excludes NER, relation extraction, embeddings,
semantic similarity, spell-checking based on large dictionaries or
language models, full morphology, machine translation, and document
classification. Once an operation needs a model, a corpus, or world
knowledge, it stops being a string algorithm and starts being NLP.

## In-scope areas

The following areas fit naturally and are where StringCheese should
expand:

### Segmentation and boundary detection
Graphemes, words, sentences, lines, script runs, Unicode boundaries,
language-sensitive token boundaries. Fits the ICU4X / WIT-component
direction — segmentation is foundational infrastructure rather than
NLP inference.

### Normalization and canonicalization
Unicode normalization, case folding, whitespace normalization,
punctuation canonicalization, width folding, diacritic handling,
confusable handling, compatibility forms, filename / identifier
normalization. Go beyond merely exposing NFC / NFKC — provide
deliberate canonicalization pipelines for common jobs.

### Transliteration and script conversion
Latinization, Cyrillic↔Latin, kana / romaji, Arabic transliteration,
traditional↔simplified Chinese where appropriate, accent stripping,
ASCII approximations. ICU is powerful here but absurdly large for many
users — the language-component idea fits naturally.

### Collation and sorting
Locale-aware ordering, natural sorting, case / diacritic-sensitive
variants, numeric ordering, sort keys, stable byte-comparable
collation keys. Collation is strangely underrepresented in Rust
outside ICU-style libraries and has obvious database / Wasm utility.

### Searching and matching
Not ER-style fuzzy record linkage — **string-local** search: substring
algorithms, Rabin–Karp, KMP, Boyer–Moore / Horspool, Aho–Corasick,
approximate substring search, wildcard matching, globbing, bounded
edit-distance search. Connects directly to the comparison algorithms
already shipped.

### Diffing and sequence alignment
Myers diff, patience diff, longest common subsequence / substring,
edit scripts, patches, word- and grapheme-aware diff. Underlying
machinery overlaps heavily with string distance; the domain is still
fundamentally "compare two pieces of text."

### N-grams, shingles, fingerprints, and sketches
Character / word n-grams, MinHash, SimHash, winnowing, rolling hashes,
locality-sensitive fingerprints. Include the representation and
comparison machinery, stop before "find which database entity this
resembles." Gives scalable comparison options much cheaper than full
pairwise distances.

### Chunking and content-defined boundaries
FastCDC, Rabin fingerprints, fixed and semantic-ish string chunking,
line / paragraph chunkers. FastCDC initially looks file-oriented but
the underlying sequence-processing primitives are closely related
enough that it belongs when StringCheese is explicitly about
efficient textual sequences rather than merely `String` utility
functions.

### Tokenization — carefully scoped
Basic lexical tokenizers, Unicode / language tokenization, regex-like
token rules, byte / BPE / WordPiece / Unigram tokenizers, token-count
APIs. **Do not** bake specific model vocabularies into the core. The
tokenizer algorithm belongs; GPT / Llama vocab datasets are optional
packages / data artifacts.

### String statistics and characterization
Script detection, Unicode category histograms, entropy, character
frequency, alphabet detection, printable / control ratios, byte /
codepoint / grapheme lengths, lexical diversity, compressibility
estimates. Useful primitives for deciding which subsequent operation
to use.

### Identifiers and escaping
Slugification, identifier sanitization, shell / SQL / HTML / JSON /
URI escaping and unescaping, quoting, casing conventions,
basename / extension-ish string operations. Some of this is mundane,
but "Apache Commons `StringUtils` done properly for Unicode and Rust"
is part of StringCheese's value proposition.

## Explicitly out of scope

- Entity resolution
- Named Entity Recognition
- Relation extraction
- Embeddings and semantic similarity
- Spell-checking based on large dictionaries or language models
- Full morphological analysis
- Machine translation
- Document classification

These provide a useful architectural boundary: they need models,
corpora, or world knowledge — the moment we start shipping those we
have stopped being a string toolkit.

## What StringCheese uniquely contributes

Existing string libraries consistently fall short not in the algorithms
themselves — most have existed for decades — but in **how they are
organized**. Most expose a flat collection of functions with little
guidance about when to use each one. StringCheese has several
opportunities to differentiate on that axis.

### 1. A unified comparison model

Instead of separate APIs like:

```rust
levenshtein()
jaro()
jaro_winkler()
```

algorithms implement a common trait, exposing properties such as:

- `Metric`
- `Similarity`
- `Distance`
- `Normalized`
- `Symmetric`
- `UnicodeAware`
- `TokenBased`
- `Phonetic`

Callers reason about algorithms **generically** — the property set
tells them whether an algorithm satisfies the triangle inequality,
whether it's normalized to `[0, 1]`, whether it operates on graphemes
or tokens, and so on.

### 2. Capability-based selection

Rather than requiring users to know algorithm names, let them express
intent:

```rust
compare()
    .typo_tolerant()
    .unicode()
    .fast()
```

The builder chooses an appropriate algorithm under the hood, while
still allowing explicit selection when needed. Intent, not incantation.

### 3. Common result types

Instead of every algorithm returning a different numeric type:

```rust
usize
f64
u32
bool
```

use richer result types with normalization helpers:

- `ComparisonResult`
- `Distance`
- `Similarity`
- `Confidence`

A `Distance` can be `.normalize()`d into a `Similarity`; a
`ComparisonResult` carries its algorithm's `Metric` / `Symmetric` /
`Normalized` properties so downstream code can validate assumptions.

### 4. Comprehensive benchmarks

Ship a real benchmark and correctness kit, not just criterion micro-
benchmarks buried in a `benches/` folder:

- Golden datasets
- Correctness tests against established implementations
- Multilingual corpora
- Performance benchmarks
- Memory benchmarks
- WebAssembly benchmarks

Makes the library both **trustworthy** and **measurable**.

### 5. A coherent module layout

The conceptual map, not an alphabetical list:

```
stringcheese
├── compare/
│   ├── edit
│   ├── phonetic
│   ├── token
│   ├── set
│   ├── sequence
│   ├── bio
│   └── metric
├── normalize/
├── unicode/
├── tokenize/
├── transform/
├── search/
├── diff/
├── chunk/
├── index/
├── hash/
├── case/
├── encode/
└── benchmark/
```

Users get a conceptual map instead of a long alphabetical list of
unrelated functions.

### The niche

Taken together with the emphasis on ergonomics, WebAssembly support,
multilingual correctness, and explicit distinctions like metric vs.
non-metric comparisons, StringCheese occupies a niche that isn't
currently well served: **a comprehensive, well-organized toolkit that
makes advanced string processing approachable without sacrificing
performance or rigor**.

## Decomposition

The conceptual pipeline:

```
                    ┌─ normalize
bytes → Unicode ────┼─ segment
                    ├─ transform / transliterate
                    ├─ tokenize
                    ├─ search
                    ├─ compare
                    ├─ align / diff
                    └─ fingerprint / chunk
                           │
                           ▼
                    application semantics
                    ─────────────────────
                    entity resolution
                    NLP / NER
                    search engines
                    databases
                    LLM tooling
```

Then language-specific modules supply tables and rules into those
operations. The unifying idea is not "string utilities" — it is
**a comprehensive toolbox for computational operations over text
sequences**.

## Reusable low-level machinery

A lot of these supposedly separate domains collapse onto a small
number of primitives:

- Iterators over bytes / codepoints / graphemes / tokens
- Rolling windows
- Rolling hashes
- Dynamic-programming matrices, bounded DP
- Prefix tables and automata
- Normalization streams
- Locale data

If StringCheese exposes those carefully — or at least uses a common
internal abstraction — an enormous surface area is implementable
without an enormous library.

The differentiator: most libraries organize around **named
algorithms**; StringCheese organizes around **text representations and
operations, with algorithms as selectable implementations underneath**.
That is much more cohesive than accumulating 150 unrelated functions.

## Regex and the pattern subsystem

Regex fits StringCheese and belongs as a **major subsystem**, not "just
another utility." It sits squarely inside the boundary: pattern
matching over text sequences. Regex can become the general pattern
layer that unifies search, tokenization, extraction, splitting,
replacement, and validation.

The opportunity is not to wrap Rust's `regex` crate — it is to define
a **portable, Unicode-aware regex engine and IR** that works cleanly
across native Rust and Wasm.

### Layout

```
stringcheese
  compare/
  normalize/
  segment/
  search/
  pattern/
      regex
      glob
      wildcard
      literal
  tokenize/
  transform/
```

### The pattern abstraction

A caller should be able to write:

```rust
text.find(pattern)
text.matches(pattern)
text.split(pattern)
text.replace(pattern, replacement)
```

where `pattern` might be a literal, regex, glob, compiled automaton,
or a specialized matcher.

### Regex engine philosophy

**Favor a finite-automata-oriented engine** over reproducing PCRE.
Support the large useful regular subset; deliberately exclude
constructs that destroy predictable performance — especially
unrestricted backreferences and recursive patterns.

That gives:

- Guaranteed linear (or otherwise bounded) matching behaviour
- Deterministic resource use
- Straightforward Wasm execution
- DFA / NFA compilation options
- Serialization of compiled regexes
- SIMD opportunities
- Streaming matching
- Common machinery with Aho–Corasick and tokenization
- Safer use on untrusted patterns / text

Fits StringCheese's performance philosophy much better than a
backtracking engine.

### Unicode semantic units are first-class

Existing regex engines often make Unicode behaviour feel bolted on.
StringCheese makes the semantic unit **explicit**:

```rust
Regex::bytes(...)
Regex::codepoints(...)
Regex::graphemes(...)
```

or through compile options. This matters because `.`, quantifiers,
character classes, boundaries, and offsets all have ambiguous meanings
once Unicode enters the picture. StringCheese's regex API is
unusually precise about whether matching happens over bytes, scalar
values, grapheme clusters, or language-aware boundaries.

Unicode properties and segmentation are first-class in the syntax:

```
\p{Letter}
\p{Script=Greek}
\p{General_Category=Decimal_Number}

\b{word}
\b{grapheme}
\b{sentence}
```

Creates strong reuse with the ICU-like data components already in
motion.

### Regex as an IR target

The really interesting architectural possibility: regex becomes an IR
target. Higher-level facilities compile into the same matcher
representation.

```
literal search ─────┐
glob ───────────────┤
wildcards ──────────┤
regex ──────────────┼──> Pattern IR ──> NFA / DFA / SIMD matcher
token rules ────────┤
multi-pattern search┘
```

StringCheese isn't maintaining five unrelated search engines — it has
one **common pattern compiler with specialized fast paths**:

```
"foobar"          → memchr / SIMD substring
"a|b|c|d"         → Aho–Corasick
"[a-z]+"          → specialized automaton
general regex     → NFA / DFA
```

That is exactly the kind of implementation detail most string
libraries gloss over and where StringCheese distinguishes itself.

### One explicit boundary

**Don't chase PCRE compatibility for its own sake.** Bounded
lookaround may be reasonable; backreferences make the language
non-regular and radically alter the implementation model. Once we
support everything Perl does we have committed to a completely
different class of engine.

State the philosophy explicitly:

> **StringCheese regexes describe regular languages, not arbitrary
> Perl programs disguised as patterns.**

That keeps regex cohesive with the rest of the library rather than
allowing it to become its own universe.

## Wrap-the-ecosystem vs. reimplement-in-house

StringCheese sits on top of a rich Rust ecosystem, and one of the
recurring calls is whether a given subsystem should wrap an existing
crate or ship its own implementation. Neither answer is universally
right. The heuristic:

**Wrap the ecosystem when:**

- The upstream implementation is a **huge, complex engine** — regex,
  ICU4X, a WASM component model runtime. Reimplementing is a career,
  not a project, and the odds of matching the upstream's correctness
  and completeness are low.
- The upstream is **mature and well-tested**. Reimplementing something
  well-audited is negative-value work — new bugs, no new capability.
- StringCheese's contribution is **UX / API / integration**, not
  algorithmic. `stringcheese-pattern-regex` is a thin adapter over
  `regex` because our value is the trait-uniform `Pattern` surface,
  the explicit `MatchUnit`, and the ergonomic constructors — not the
  finite-automata engine.

**Reimplement in-house when:**

- The algorithm is **small and well-understood** — Myers diff (~200
  lines), a Thompson NFA loop, a Unicode-block script classifier.
  Cost is bounded; value is real. (Shell wildcards did not clear
  this bar — `stringcheese-pattern`'s `Wildcard` and `Glob` wrap
  `globset` under the `Pattern` trait.)
- There's a **WASM-specific perf experimentation window**. WASM's
  perf profile diverges meaningfully from native: `wasm-simd128` is
  its own instruction set (not AVX / NEON), binary size matters more
  than in most native contexts, alloc is expensive relative to
  compute, no threading in the browser. An in-house implementation
  lets us tune data layout, SIMD lane widths, streaming vs. buffered,
  and small-binary-size variants for what WASM actually rewards.
- StringCheese's contribution could **include the perf differentiator
  itself**. `stringcheese-diff`'s Myers implementation is our own so
  we can add a `wasm-simd128`-tuned variant later; the API stays
  identical whichever backend is compiled in.

**Deliberately does not fit the wrap rule:**

- `stringcheese-pattern/literal.rs` wraps `memchr` — but `memchr` IS
  the SIMD story. Reimplementing it would forfeit the very perf
  differentiator we'd otherwise be reaching for. Wrap it.

State the rationale in the crate that makes the call, so the
decision survives context loss.

### SIMD substrate: StringZilla

When an in-house implementation reaches the point of "we need
per-target SIMD", the substrate to reach for is [StringZilla]
(https://github.com/ashvardanian/StringZilla). It ships hand-
tuned kernels for x86 (AVX2 / AVX-512), ARM (NEON / SVE), and
WebAssembly (SIMD128), covering the primitives StringCheese cares
about most: substring search, byteset scanning, Hamming / edit
distance, and hashing.

**Why note it here rather than depend on it today.** Adding a
dependency this heavy is worth it only when a specific in-house
kernel earns the wrap under the [in-house implementations must
earn their place](#in-house-implementations-must-earn-their-place)
rule above. Concretely, the current at-risk points where the
StringZilla substrate would tip the argument:

- **`stringcheese-diff::algo::Myers`** — the inner v-vector loop is
  a natural fit for byte-parallel probes. If the "revisit trigger"
  in that crate's module docs fires without in-house perf work
  landing, the choice is `similar` (wrap) or StringZilla-backed
  (in-house with a real SIMD story). Without the substrate, wrap.
- **Future n-gram / fingerprint kernels** — MinHash, SimHash, and
  rolling-hash CDC all lean on byte-parallel probes.
- **Future edit-distance / alignment** — StringZilla's Levenshtein
  and Needleman-Wunsch kernels are the shape our subsystem would
  want anyway.

The substrate is *noted*, not *staged*. Any crate that reaches
for it commits to a benchmark that proves the wrap-vs-in-house
math and documents the target-per-CPU coverage cost.

### In-house implementations must earn their place

The rule is stricter than "could-plausibly-be-in-house." An in-house
implementation must be justified by **concrete planned work** —
identified perf targets, a specific WASM-SIMD experiment, a
measured binary-size win over the ecosystem alternative. "We might
want to experiment someday" is not a justification; it's a
placeholder for a real one.

Concretely, when a new subsystem lands with an in-house
implementation, the crate's module docs record:

- **What the specific in-house value is** (not "flexibility" —
  something measurable: a benchmark target, a WASM feature
  exercised, a design constraint the wrap doesn't satisfy).
- **The revisit trigger** — a window after which, absent progress
  on the planned work, the implementation should be replaced with
  a wrap. "Kept in-house pending SIMD experiment; revisit if no
  benchmark-tracked perf work lands by 2026-Q1."

Without both, the in-house implementation shouldn't ship.
Reimplementing a well-tested algorithm on speculation is negative-
value work — new bugs, no new capability, all maintenance.

## Implementation staging

This document describes the target shape. Landing each area is a
separate initiative — the crate structure evolves as work lands
rather than by fiat. In broad order of foundational value:

1. **Pattern IR + regex engine** — because it unifies so much of the
   search / tokenize / split / replace surface.
2. **Segmentation** — grapheme / word / sentence / line via ICU4X.
3. **Normalization pipelines** — beyond raw NFC / NFKC, deliberate
   named pipelines for common jobs. `stringcheese-unicode` ships
   the primitives (Unicode normalisation, case folding, diacritic
   stripping) plus a composable `PreprocessingPipeline` builder.
   `stringcheese-normalize` ships the presets: `identifier`
   (NFKC + case-fold + strip-diacritics), `display_safe`
   (strip-controls + NFC + collapse-whitespace), `search_key`
   (identifier + canonicalise-punctuation + collapse + trim),
   plus the small in-house primitives that back them
   (whitespace collapse, punctuation canonicalisation, control
   stripping).
4. **Transliteration** — via the language-component pattern already
   established by the language-pack work. `stringcheese-translit`
   ships the coordination point: a `Transliterator` trait,
   `DeunicodeTransliterator` (general any-script → ASCII via
   `deunicode`), `TableTransliterator` (classic char-to-string
   lookup pattern), a `Chained` combinator for pipeline
   composition, and a built-in `cyrillic_to_latin_iso9` table
   as a template. Language packs plug per-language romanizers
   into the same trait in follow-ups.
5. **Collation** — locale-aware sort keys.
   `stringcheese-collate` ships a `Collator` trait plus three
   implementations: `UcaCollator` (Unicode Collation Algorithm
   via `feruca`), `NaturalCollator` (numeric-run-aware wrapper —
   `file2 < file10`) that composes over any inner collator, and
   `AsciiCiCollator` (byte-level ASCII case-insensitive fast
   path). Callers who need CLDR-tailored variants swap in
   `icu_collator` behind the same trait.
6. **Diffing** — Myers / patience / grapheme-aware.
7. **N-grams and fingerprints** — MinHash, SimHash, winnowing.
   `stringcheese-ngram` ships sliding-window char / byte / token /
   grapheme grams (with optional padding). The fingerprint trio
   is complete: `stringcheese-minhash` (fixed-permutation MinHash
   sketches, Jaccard estimation, LSH banding),
   `stringcheese-simhash` (64-bit / 128-bit Charikar fingerprints
   with weighted-feature support, Hamming similarity,
   permutation-band LSH), and `stringcheese-winnowing`
   (Schleimer-Wilkerson-Aiken 2003 local-algorithm document
   fingerprints — the scheme used by Moss).
8. **Chunking** — `stringcheese-cdc` covers byte-oriented
   content-defined chunking (FastCDC / Buzhash / Rabin / GearHash).
   `stringcheese-textsplit` covers the text-oriented side that LLM
   RAG pipelines reach for: a `TextSplitter` trait, a LangChain-
   style `RecursiveSplitter` (separator list + size + overlap,
   preserving semantic boundaries), `ParagraphSplitter` (splits on
   `\n\n`, falls through to recursive for oversized paragraphs),
   and `SentenceSplitter` (groups whole sentences up to a byte
   budget, never mid-sentence cuts).
9. **Statistics / characterization** — script detection, entropy,
   histograms. `stringcheese-stats` ships entropy /
   `Histogram` (Unicode general category + coarse major-category
   roll-up) / `Ratios` / `Lengths` today.
10. **Identifier / escaping utilities** — the "Apache Commons done
    right" surface. `stringcheese-ident` ships `Case` / `to_case` /
    `Case::detect` (wrapping `heck`), `slugify` / `Slugger`
    (wrapping `deunicode`), and `Sanitizer`. `stringcheese-escape`
    ships `Escape` / `escape` / `unescape` for URI (wrapping
    `percent-encoding`), HTML (wrapping `html-escape`), JSON
    string body (in-house — small grammar, avoids `serde_json`
    footprint), and POSIX shell (wrapping `shlex`).
