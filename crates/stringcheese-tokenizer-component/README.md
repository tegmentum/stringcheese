# stringcheese-tokenizer-component

Reference WebAssembly Component-Model packaging for the StringCheese
tokenizer subsystem. Wraps a `stringcheese-tokenizer-hf::BpeTokenizer`
built from a small hand-crafted character-BPE vocabulary behind the
shared `tegmentum:tokenizer@0.1.0` WIT contract so that any
component-model-capable host (Wasmtime, jco, WasmCloud, Spin, …) can
invoke `encode` / `decode` / `count` / `get-capabilities` without
linking Rust.

The WIT source lives at
[`component/wit/tokenizer/stringcheese-tokenizer.wit`](../../component/wit/tokenizer/stringcheese-tokenizer.wit);
the design commentary is in [`docs/design/tokenizers.md` §
8](../../docs/design/tokenizers.md) and the Phase-7 acceptance
criterion in [`docs/design/tokenizers.md` §
11](../../docs/design/tokenizers.md).

## Position in the tokenizer subsystem

Phase 7 of the tokenizer subsystem's phased plan. The design commits
to a `component/wit/tokenizer/stringcheese-tokenizer.wit` that parses cleanly
under `wit-parser`, `wit-bindgen` producing a clean Rust host
binding, and a **reference** `tokenizer-provider` component that
echoes correct encodings across the boundary under `wasmtime`. This
crate is that reference — the pre-configured `cl100k_base` /
`o200k_base` / HuggingFace variants will each ship as their own
component crates when they land, mirroring the feature-gated
`wit-component` pattern used here.

## Why a hand-crafted vocabulary

* **No real OpenAI / HuggingFace vocab bytes.** The workspace's
  session-standing constraint (see `CLAUDE.md`) forbids committing
  real tiktoken / HF vocab bytes into the tree. Real vocabs are
  fetched at test time by the
  [`stringcheese-tokenizer-tiktoken-conformance`](../stringcheese-tokenizer-tiktoken-conformance/)
  harness; nothing ships in-tree.
* **A minimal 261-token vocab is enough for the reference.** The
  goal is to demonstrate that the WIT boundary works end-to-end —
  one component built, loaded under wasmtime, echoing a correct
  encoding round-trip. A five-piece merge and a small special-token
  map exercise every code path the boundary depends on.
* **Deterministic and testable.** The vocab is a compile-time
  constant; the same encode input produces the same id sequence on
  host and wasm targets, so the wasmtime smoke test compares against
  a known-good fixture.

## Feature-gated WIT export

The WIT `Guest` impl in `src/wit.rs` and the pre-generated
`src/bindings.rs` are gated behind the `wit-component` cargo
feature. Without the gate, an umbrella crate that links multiple WIT
components together as plain `rlib`s would emit duplicate `export!`
symbols and fail to link — matches the pattern used by
`stringcheese-detect-{script,whatlang,lingua}` for their
`tegmentum:lang-detect` exports. The gate is
`cfg(all(target_family = "wasm", feature = "wit-component"))`
because the WIT export machinery only materialises on wasm.

## Build recipe

Prerequisites: `rustup target add wasm32-wasip1`, `wasm-tools`,
`wasmtime`. All are pinned by the CI job
(`.github/workflows/ci.yml → wasm-tokenizer-component`) so a local
reproducer needs the same versions.

```bash
# Standalone component build (wasm32-wasip1, feature-gated):
cargo build \
    -p stringcheese-tokenizer-component \
    --target wasm32-wasip1 \
    --features wit-component \
    --release

# Verify the produced .wasm exports the expected WIT world:
wasm-tools component wit \
    target/wasm32-wasip1/release/stringcheese_tokenizer_component.wasm

# Componentize (requires the WASI preview1 reactor adapter matching
# your wasmtime version — download from the wasmtime GitHub release
# and place under ~/.cache/stringcheese-tokenizer-component/):
wasm-tools component new \
    target/wasm32-wasip1/release/stringcheese_tokenizer_component.wasm \
    --adapt ~/.cache/stringcheese-tokenizer-component/wasi_snapshot_preview1.reactor.wasm \
    -o /tmp/tokenizer-component.wasm

# Invoke via wasmtime:
wasmtime run --dir=. --invoke 'encode("hello")' /tmp/tokenizer-component.wasm
# → ok({ids: [260], offsets: [{start: 0, end: 5}], ...})

# Run the full wasmtime smoke test suite:
cargo test -p stringcheese-tokenizer-component --features wit-component
```

## Sizes

Reference build under the workspace's tuned release profile:

| Artifact                     | Size    |
|------------------------------|---------|
| `wasm32-wasip1` core module  | ~241 KB |
| Componentized (with adapter) | ~262 KB |
| After `wasm-opt -Oz`         | ~161 KB |

Numbers are informational — the enforced size gate at
`.wasm-size-limits.toml` does not currently track this crate; a
future landing may add it once the reference tokenizer stabilises.

## Public API (native)

Callers who consume this crate as a plain `rlib` (no `wit-component`
feature) get the same tokenizer surface as an ordinary Rust API:

```rust
use stringcheese_tokenizer_component::{encode, decode, count, get_capabilities};

let enc = encode("hello").expect("reference vocab covers ASCII");
assert_eq!(enc.ids, vec![260]);
assert_eq!(decode(&enc.ids).unwrap(), "hello");
assert_eq!(count("hello").unwrap(), 1);

let caps = get_capabilities();
assert_eq!(caps.model_type, "bpe");
assert_eq!(caps.variant_id, "reference-character-bpe");
```

## Extending

To ship an additional tokenizer variant (e.g. `cl100k_base`,
`o200k_base`, or an HF `tokenizer.json` pack) as its own component:

1. Create a sibling crate `crates/stringcheese-tokenizer-component-<variant>/`
   mirroring this crate's layout.
2. Replace the hand-crafted vocab in `src/reference.rs` with a
   `BpeTokenizer` sourced from the appropriate loader
   (`stringcheese-tokenizer-tiktoken`, `stringcheese-tokenizer-hf::hf`, …).
3. Keep the same feature-gate pattern
   (`cfg(all(target_family = "wasm", feature = "wit-component"))`)
   so umbrella-crate consumers do not hit duplicate-export errors.
4. Add a CI job entry mirroring `wasm-tokenizer-component`.
