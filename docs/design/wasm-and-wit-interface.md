# WebAssembly and WIT Interface

Status: Design
Applies to: StringCheese 0.1 and later
Related: [DESIGN.md](../DESIGN.md), [type-system.md](./type-system.md), [preprocessing-pipeline.md](./preprocessing-pipeline.md), [phonetic-subsystem.md](./phonetic-subsystem.md), [ngram-and-fingerprinting.md](./ngram-and-fingerprinting.md)

The design of StringCheese's WebAssembly story — no_std discipline, feature-gate strategy, deterministic memory, binary-size targets, Wasm SIMD, Component Model interface, streaming across component boundaries, and cross-target validation.

## WebAssembly is a primary deployment target

WebAssembly support is not something the library retrofits onto an otherwise-native implementation. Every design decision is examined against browser, WASI, Component Model, embedded, and no_std targets.

The consequences run through the whole codebase:

- `stringcheese-core` is `#![cfg_attr(not(feature = "std"), no_std)]` and `#![forbid(unsafe_code)]`. Its default `std` feature is a convenience; disabling it produces a `no_std` build. Disabling both `std` and `alloc` leaves a pure no-alloc build with only the type-and-trait substrate.
- Algorithm crates that require dynamic buffers still work under `alloc`-only; they never assume `std`.
- Unicode tables and phonetic language packs are behind Cargo features so a browser-tab build can carry only what it uses.
- The public API returns concrete types (see [type-system.md](./type-system.md)) rather than `Box<dyn Error>` or `Arc<dyn Trait>`, which would inflate binary size and complicate no_std builds.
- No global mutable state, no lazy statics that require synchronization primitives — everything the library holds is either `const` or owned by the caller.

The rule for judgment calls: if a design choice hurts the Wasm build, it needs a specific justification. There is no "we can fix that in the Wasm crate later" — the Wasm build is a first-class citizen alongside native.

## The `no_std` discipline

Layer by layer.

### `stringcheese-core`

- `#![no_std]` in the default configuration when built without the `std` feature.
- `#![forbid(unsafe_code)]` unconditionally.
- Uses `alloc` for heap-allocating utilities (like `Vec`-backed workspaces), but algorithms and types themselves do not require `alloc`. A no-`alloc` build compiles cleanly and gives access to the type substrate for callers that build their own arena-backed algorithms.
- No `std::io`, no `std::sync`, no `std::thread`. Where a synchronization primitive is genuinely required (rare), it is behind a `std` feature gate.

### Algorithm crates

- Algorithms should be `no_std` compatible where practical. Levenshtein, Hamming, Jaro, Jaccard, cosine, MinHash, Rabin-Karp, KMP, Boyer-Moore, FastCDC, and the code-driven phonetic algorithms all have no legitimate `std` dependency.
- Weighted Levenshtein with runtime-loaded cost tables may need `alloc`; that is acceptable.
- Trigger-heavy Unicode tables (grapheme break rules, full case-folding tables) can require `alloc` and are pulled in only by the `unicode-full` feature.

### Phonetic language packs

- Small code-driven packs (Soundex, NYSIIS) are `no_std` + `alloc`.
- Large data-driven packs (Beider–Morse, Daitch–Mokotoff) may embed their rule tables as `include_bytes!` at compile time; still `no_std` + `alloc`, though the compiled tables take real space.
- A pack that requires runtime rule loading (from a file or network) is `std` — but no shipped pack should need this at the level StringCheese targets.

### `stringcheese-unicode`

- Basic normalization (NFC, NFD, simple case folding, whitespace collapse) is `no_std` + `alloc` and behind the `unicode` feature.
- Full grapheme segmentation, full-locale case folding, and diacritic classification require the large Unicode tables and gate behind `unicode-full`. Loading the tables may need `std::io` in some build configurations; `unicode-full` implies `std`.

## Feature-gate strategy

Cargo features carve StringCheese into layers so that a target with tight budgets — a browser tab, a Wasm component in a hot execution path, an embedded controller — carries only what it uses.

### The default set

Enabled by default:

- `std` (convenience default; the whole workspace works on native).
- `alloc` (implied by `std`; explicit for callers that disable `std`).
- `distance` (unit-cost edit-distance family).
- `similarity` (Jaro, Jaro-Winkler, Jaccard, Dice).
- `unicode` (basic NFC/NFD normalization, simple case folding).

Disabled by default (opt-in):

- `alignment` (Smith-Waterman, Needleman-Wunsch — significantly larger).
- `phonetic` and the language packs `phonetic-germanic`, `phonetic-romance`, `phonetic-slavic`, `phonetic-semitic`, `phonetic-indic`, `phonetic-cjk`.
- `unicode-full` (large grapheme and case-folding tables).
- `fingerprint`, `search`, `chunking`, `indexing`.
- `simd` (native SIMD implementations).
- `wasm-simd` (WebAssembly SIMD implementations).
- `parallel` (Rayon-based parallel batch runners; not usable in single-threaded Wasm without threads).
- `component-model` (WIT-derived host bindings; pulls in `wit-bindgen`).

### Mutually exclusive

- `simd` and `wasm-simd` are alternatives; the build system selects one based on the compilation target. Enabling both is a build error rather than a silent selection.

### Contrast: `unicode` vs `unicode-full`

The distinction is deliberate.

- `unicode` — the operations most callers need: NFC/NFD normalization, simple case folding, whitespace detection. Table size is small (a few tens of kilobytes when compiled) and the feature is on by default.
- `unicode-full` — grapheme segmentation, extended case folding, script identification, diacritic removal. Table size is significantly larger (hundreds of kilobytes). Off by default; enabling it moves a Wasm build from "minimal" to "features".

Callers that need grapheme-level operations opt into `unicode-full` explicitly. This makes the trade-off visible: a browser build that requires graphemes accepts the size cost consciously.

### Feature interaction with phonetics

Each `phonetic-<region>` feature gates a language pack. Enabling `phonetic` alone enables the encoder trait and the `AnyPair` matcher machinery but no language rules. A build that turns on `phonetic-germanic` gets Soundex, NYSIIS, Metaphone, Double Metaphone, Caverphone, and Match Rating; a build that adds `phonetic-slavic` also gets Daitch–Mokotoff. See [phonetic-subsystem.md § Modularity](./phonetic-subsystem.md#modularity).

## Deterministic memory

The library's memory behavior must be predictable, especially in Wasm where linear memory growth is coarse and expensive.

- **Fixed-capacity workspaces.** [`Workspace`](./type-system.md#workspace) implementations grow to accommodate the largest input seen and then stay at that size across a batch. `Workspace::shrink()` is available for the case where the caller wants to release memory (typically at the end of a batch), but growth is monotonic during use.
- **No hidden allocations in hot loops.** Distance kernels do not allocate inside their inner loops. Any allocation happens before the loop starts (usually via `Workspace::ensure_capacity`).
- **Streaming APIs bound peak memory.** Streaming n-gram generation, streaming rolling-hash consumption, and streaming CDC chunking all operate over an input in constant additional memory beyond the caller's chosen window size.
- **Predictable pool exhaustion.** For long-running Wasm services that use `Workspace` pools, exhaustion is detectable (via `capacity()`), and callers can choose whether to grow or reject.
- **No hidden global caches.** The library holds no lazy singletons that expand across the process lifetime. Any per-invocation memoization is caller-owned.

## Binary size targets

StringCheese tracks Wasm binary size as a first-class metric alongside runtime and memory.

### Targets

- A minimal Wasm build of `stringcheese-core` + Levenshtein (unit-cost, bytes), with no `unicode`, no `alignment`, no `phonetic`, no SIMD, compiled with `wasm-opt -Oz`, should weigh **under 40 KB** compressed and **under 100 KB** uncompressed. These numbers are aspirational for the initial 0.1 release; they become gated in CI once measured.
- Adding `unicode` (basic normalization + simple case folding) should add **under 30 KB** compressed.
- Adding `unicode-full` (graphemes) is expected to add **50–200 KB** compressed depending on the tables included.
- Each phonetic language pack adds separately; the goal is under 20 KB compressed for a code-driven pack.

### Measurement

- `wasm-opt -Oz` for size; `wasm-opt -O3` for speed. CI publishes both.
- `twiggy top` and `twiggy dominators` on the release build to identify the largest contributors. The reports are checked in per release.
- `cargo bloat --target wasm32-unknown-unknown` for a Rust-symbol-level breakdown.

### Why it matters

- A browser bundle including StringCheese adds to page load time. The library needs to be a small fraction of the JavaScript budget most sites already carry.
- Wasm Component Model deployments (edge compute, serverless) charge per-artifact size on cold start. A 5 MB comparison component is disproportionate for any workload.
- Embedded Wasm runtimes have fixed linear-memory ceilings. A library that overspends the code segment leaves less room for input data.

## Wasm SIMD

- Wasm SIMD is an opt-in feature (`wasm-simd`) that must never change observable results.
- The [golden corpus](../../crates/stringcheese-corpus/src/lib.rs) runs on both the scalar and SIMD backends, and every case must agree bit-for-bit for integer results and within the case's declared [`FloatExpectation`](../../crates/stringcheese-corpus/src/lib.rs) tolerance for floating-point results. Disagreement is a release-blocking defect.
- The SIMD path is guarded by feature gates, not runtime dispatch. Wasm has no equivalent of native's `cpuid`; the target either has SIMD or it does not.
- SIMD implementations are optional; algorithms without an obvious SIMD win (small-alphabet automata, table-driven phonetic encoders) skip the SIMD path entirely rather than ship an obfuscated version for no gain.
- Cross-backend agreement is enforced by the differential test infrastructure described in [DESIGN.md § Metamorphic Testing](../DESIGN.md).

## Component Model / WIT

A WIT interface for StringCheese is planned for version 0.3 (see [DESIGN.md § Future Roadmap](../DESIGN.md)). This section sketches the intended shape.

### Interface sketch

```wit
// Proposed — not yet implemented.
package stringcheese:core@0.1.0;

interface descriptors {
    record algorithm-descriptor {
        family: string,
        variant: string,
        version: string,
        source: string,
    }
}

interface result-types {
    variant distance {
        integer(u32),
        real(f64),
    }

    variant similarity {
        real(f64),
    }

    variant bounded-distance {
        within(distance),
        exceeded(u32),
    }
}

interface comparison {
    use descriptors.{algorithm-descriptor};
    use result-types.{distance, similarity, bounded-distance};

    // Sequences committed to concrete representations at the interface.
    resource prepared-bytes {
        constructor(input: list<u8>);
    }
    resource prepared-scalars {
        constructor(input: string);   // WIT `string` is UTF-8 scalars
    }

    // Direct byte comparison.
    distance-bytes: func(
        descriptor: algorithm-descriptor,
        left: list<u8>,
        right: list<u8>,
    ) -> distance;

    // Prepared-side comparison.
    distance-prepared-bytes: func(
        descriptor: algorithm-descriptor,
        query: borrow<prepared-bytes>,
        candidate: list<u8>,
    ) -> distance;

    // Cutoff-aware.
    distance-within-bytes: func(
        descriptor: algorithm-descriptor,
        left: list<u8>,
        right: list<u8>,
        cutoff: u32,
    ) -> bounded-distance;
}

world stringcheese {
    export descriptors;
    export result-types;
    export comparison;
}
```

### Impedance mismatch

Rust's generic sequence types do not cross the WIT boundary. The interface must commit to concrete representations at each function; `IndexableSequence` is a Rust trait, and WIT has no equivalent.

- Byte comparison is exposed directly as `list<u8>`.
- Scalar (UTF-8) comparison is exposed as `string` (WIT strings are UTF-8 by definition; each function's docs specify whether "length" means bytes, scalars, or graphemes).
- Grapheme, token, and phoneme representations require host-side conversion. The intent is to expose them as opaque resources (`resource prepared-graphemes` etc.) that the host constructs from a `string`; the guest never sees the internal layout.
- Weighted-vector representations (for cosine on n-grams) are exposed as a record of two parallel lists.

Every function in the interface is monomorphic. Generic parameters that exist in the Rust API become separate WIT functions (`distance-bytes`, `distance-scalars`) rather than a single generic function.

### Prepared resources

- WIT `resource` types are the natural fit for [prepared values](./preprocessing-pipeline.md#what-prepared-means). A `prepared-bytes` is constructed once with the preprocessed input, retained across many `distance-prepared-bytes` calls, and finalized when dropped.
- Resources are held on the guest side (inside the StringCheese component). The host holds a handle. This keeps preprocessed representations from crossing the boundary repeatedly.

### Workspaces

- Workspace reuse is a guest-side implementation detail. The component maintains a workspace pool internally; each entry point picks a workspace, uses it, returns it to the pool.
- The host does not need to know about workspaces to benefit from them. A high-throughput host calling `distance-bytes` in a loop sees allocation-free performance without configuration.
- For fine-grained control the interface exposes an explicit `workspace-pool` resource, so the host can preallocate against a known peak load.

## Streaming across component boundaries

Streaming algorithms (rolling hashes, CDC, streaming n-gram generation, streaming Rabin-Karp) fit awkwardly across the WIT boundary: the natural API is "call `push(byte)` in a loop and receive events", but a WIT call per byte is unusable.

The interface exposes streaming through chunked calls with backpressure:

```wit
// Proposed — not yet implemented.
interface streaming-hash {
    resource rolling-hash {
        constructor(descriptor: algorithm-descriptor);
        push-chunk: func(bytes: list<u8>) -> list<u64>;   // one digest per byte in chunk
        digest: func() -> u64;
        reset: func();
    }
}

interface streaming-cdc {
    resource cdc-splitter {
        constructor(descriptor: algorithm-descriptor);
        push-chunk: func(bytes: list<u8>) -> list<u32>;   // boundary offsets within chunk
        finish: func() -> list<u32>;                       // any final boundary
    }
}
```

- Chunk size is negotiated by the host; a typical size (16 KiB) amortizes per-call overhead across many symbols.
- Backpressure is inherent — the host chooses when to call `push-chunk`. There is no in-flight buffer beyond what a single call carries.
- Incremental preprocessing (streaming normalization, streaming case folding) uses the same pattern where feasible; Unicode normalization has boundary conditions that make chunk-safe streaming non-trivial and the interface documents its guarantees precisely.

## Cross-target validation

Every release runs the [golden corpus](../../crates/stringcheese-corpus/src/lib.rs) on the full target matrix:

- `x86_64-unknown-linux-gnu` scalar
- `x86_64-unknown-linux-gnu` with native SIMD
- `aarch64-apple-darwin` scalar
- `aarch64-apple-darwin` with native SIMD
- `wasm32-wasip1` scalar
- `wasm32-wasip1` with Wasm SIMD
- `wasm32-unknown-unknown` scalar
- `wasm32-unknown-unknown` with Wasm SIMD
- Debug and release builds of each of the above where practical
- 32-bit targets where practical

Integer results must agree exactly across every target. Floating-point results must agree within the [`FloatExpectation`](../../crates/stringcheese-corpus/src/lib.rs) tolerance declared in each case. Disagreement across targets is a release-blocking defect, not a "known difference".

The scalar/SIMD equivalence tests double as portability tests: because the differential harness runs the same case on both, a Wasm SIMD implementation that drifts from the scalar reference is caught before it reaches a release.

## Component packaging

The right packaging granularity is not obvious.

- **Monolithic component.** One `stringcheese` component with every algorithm compiled in. Simplest to consume; worst binary-size story.
- **Per-family component.** One component per `AlgorithmFamily` (or per closely-related group). Composable — a host embeds only the families it needs — but requires the host to wire multiple components together.
- **Per-representation component.** One component for byte algorithms, one for scalar algorithms, one for graphemes, etc. Optimizes for hosts that commit to a single representation.

The plan for the initial 0.1 Component Model release:

- Ship a monolithic `stringcheese` component behind a Cargo feature set that matches the default library features. The size is bounded by the same feature gates as the Rust library.
- Ship separate `stringcheese-phonetic-<region>` components per language pack. Hosts that need multi-language phonetic matching compose them; hosts that need only English pull in only `stringcheese-phonetic-germanic`.
- Ship a `stringcheese-unicode-full` component that hosts pull in only when they need grapheme-level operations, so a bytes-and-scalars deployment does not carry the tables.

Per-family and per-representation packaging are options for later releases if size analysis shows the monolithic component is a substantial fraction of a target host's Wasm budget.

## Cross-references

- The result types crossed over the WIT boundary are defined in [type-system.md § The result-type hierarchy](./type-system.md#the-result-type-hierarchy).
- The prepared-value model used by the WIT `resource` types is described in [preprocessing-pipeline.md § What "prepared" means](./preprocessing-pipeline.md#what-prepared-means).
- Feature-gating for per-language phonetic packs is described alongside the packs themselves in [phonetic-subsystem.md § Modularity](./phonetic-subsystem.md#modularity).
- Streaming n-gram generation and rolling hashes, which underpin the streaming WIT resources, are described in [ngram-and-fingerprinting.md § Streaming](./ngram-and-fingerprinting.md#streaming).
- The WebAssembly requirements themselves are enumerated in [DESIGN.md § WebAssembly](../DESIGN.md).
