# Comparand — WebAssembly Component Model layer

This directory is the Component-Model face of Comparand: a small WIT
interface and a Rust host that implements it, together forming a
WebAssembly component that any Component-Model-capable runtime
(Wasmtime, jco, WasmCloud, Spin, …) can load and call without linking
Rust.

The design document for this layer is
[../docs/design/wasm-and-wit-interface.md](../docs/design/wasm-and-wit-interface.md);
the wasm hardening the main library follows is documented in
[../docs/wasm-build-recipes.md](../docs/wasm-build-recipes.md). Read
those first if you need the "why" behind any decision below.

## Layout

```
component/
├── README.md                            (this file)
├── wit/
│   └── comparand.wit                    interface definition
└── rust-host/                           Rust host implementation
    ├── Cargo.toml                       standalone-workspace sentinel
    └── src/
        └── lib.rs                       wit-bindgen macro + Guest impls
```

`rust-host/Cargo.toml` opens with an empty `[workspace]` table so the
crate is not a member of the outer Comparand workspace. That matches
the pattern used by `fuzz/Cargo.toml` and
`bench-adapters/rust/Cargo.toml`; a `cargo` invocation from anywhere
inside `component/` resolves against a fresh, isolated `Cargo.lock`.

## The interface at a glance

Package: `comparand:core@0.1.0`. Four interfaces, sixteen functions,
one world.

| Interface     | Functions                                                                    | Wire shape                        |
|---------------|------------------------------------------------------------------------------|-----------------------------------|
| `distance`    | `levenshtein`, `levenshtein-within`, `hamming`, `osa`, `lcs-distance`        | `list<u8> × list<u8> → u32 / variant / result` |
| `similarity`  | `jaro`, `jaro-winkler`, `dice-bigrams`, `jaccard-bigrams`                    | `list<u8> × list<u8> → f64` in `[0, 1]`         |
| `search`      | `find-first`, `find-all`                                                     | `list<u8> × list<u8> → option<u32> / list<u32>` |
| `phonetic`    | `soundex`, `nysiis`, `double-metaphone-primary`                              | `string → string`                               |

See [`wit/comparand.wit`](wit/comparand.wit) for the precise
signatures, per-function documentation, and the `bounded-distance`
variant used by `levenshtein-within`.

### Sequence representation

The `distance`, `similarity`, and `search` interfaces exchange
`list<u8>` — raw bytes — because that is what the Comparand kernels
operate on. WIT `string` would force a UTF-8 validation on every call,
which is wasted work when the caller already has bytes and does not
help when the caller has arbitrary binary input.

The `phonetic` interface uses `string` because Soundex, NYSIIS, and
Double Metaphone are defined over characters, not bytes; the Rust
kernels accept `&str` and no re-encoding happens at the boundary.

### Error handling

The only fallible entry point in this seed layer is `distance.hamming`,
which returns `result<u32, string>`. The underlying Rust kernel
returns a typed `LengthMismatch` error; the host flattens that to a
diagnostic string for the WIT boundary. Every other function has no
precondition a well-formed input can violate, so they return their
value directly rather than wrapping it in `result`.

## Building

Prerequisites:

```bash
# Rust toolchain with the wasm target
rustup target add wasm32-wasip1

# cargo-component (build tool that turns a Rust crate into a component)
cargo install cargo-component

# wasm-tools (for validation, WIT dumps, disassembly)
cargo install wasm-tools
```

Build the component:

```bash
cd component/rust-host
cargo component build --release
```

The output lands at:

```
component/rust-host/target/wasm32-wasip1/release/comparand_component_host.wasm
```

Validate and inspect:

```bash
wasm-tools validate  target/wasm32-wasip1/release/comparand_component_host.wasm
wasm-tools component wit target/wasm32-wasip1/release/comparand_component_host.wasm
```

The `wit` subcommand round-trips the interface out of the produced
component and should show the same four exports as
[`wit/comparand.wit`](wit/comparand.wit).

### Notes on target choice

`cargo component build` defaults to `wasm32-wasip1`. The resulting
component imports a small set of WASI `0.2` interfaces (stdio,
filesystem, clocks) which Rust's `std` links against even when the
Comparand code never calls into them; a Wasmtime host provides those
by default. If you need a truly zero-WASI-import component for a
browser bundle, override the target in `.cargo/config.toml` inside
`rust-host/` to `wasm32-unknown-unknown` and switch `std` off across
the algorithm crates — the underlying kernels are `no_std + alloc`
capable, so this is a matter of feature toggles, not code changes. See
[../docs/wasm-build-recipes.md](../docs/wasm-build-recipes.md) for the
matrix of what compiles under `no_std`.

## Consuming the component

### From JavaScript (jco)

```bash
# One-time: install the jco toolchain
npm install --save-dev @bytecodealliance/jco @bytecodealliance/preview2-shim

# Transpile the component to a native JavaScript module + WIT-typed .d.ts
npx jco transpile \
    component/rust-host/target/wasm32-wasip1/release/comparand_component_host.wasm \
    --out-dir generated
```

```javascript
import { distance, similarity, phonetic } from "./generated/comparand_component_host.js";

console.log(distance.levenshtein(
    new TextEncoder().encode("kitten"),
    new TextEncoder().encode("sitting"),
));                                                    // 3

const bounded = distance.levenshteinWithin(
    new TextEncoder().encode("hello"),
    new TextEncoder().encode("world"),
    2,
);
console.log(bounded);                                  // { tag: "exceeded", val: 2 }

console.log(similarity.jaroWinkler(
    new TextEncoder().encode("MARTHA"),
    new TextEncoder().encode("MARHTA"),
).toFixed(3));                                         // 0.961

console.log(phonetic.soundex("Robert"));               // "R163"
```

`jco` handles the WIT-to-JS name transformation (`levenshtein-within`
becomes `levenshteinWithin`, `find-all` becomes `findAll`, and so on)
and generates typed `.d.ts` files so an editor autocompletes the
interface. See the [jco docs](https://bytecodealliance.github.io/jco/)
for the full runtime setup, including how to shim the WASI imports if
you are targeting a browser rather than Node.

### From Rust (Wasmtime)

Sketch — full setup is out of scope for this README, but the shape
matches every other Wasmtime component embedding:

```rust
use wasmtime::{Engine, Store};
use wasmtime::component::{Component, Linker, bindgen};

bindgen!({
    world: "comparand-core",
    path: "../component/wit",
});

let engine = Engine::default();
let component = Component::from_file(
    &engine,
    "component/rust-host/target/wasm32-wasip1/release/comparand_component_host.wasm",
)?;
let mut linker = Linker::<()>::new(&engine);
wasmtime_wasi::add_to_linker_sync(&mut linker)?;    // for the WASI imports

let mut store = Store::new(&engine, ());
let bindings = ComparandCore::instantiate(&mut store, &component, &linker)?;
let d = bindings
    .comparand_core_distance()
    .call_levenshtein(&mut store, b"kitten", b"sitting")?;
assert_eq!(d, 3);
```

## What is exposed — and what isn't

### Exposed (10 functions)

Every function here has (a) a natural single-input-single-output shape
and (b) an obvious counterpart in the Comparand crates. This is the
"workhorse" surface: the algorithms a caller reaches for first when
they turn up needing sequence comparison from a non-Rust host.

- **Levenshtein**, **OSA**, and **LCS distance** — the three
  workhorse edit-distance metrics from `comparand-levenshtein`,
  `comparand-damerau`, and `comparand-lcs`.
- **Bounded Levenshtein** (`levenshtein-within`) — the same, plus a
  cutoff, mapped to the WIT `bounded-distance` variant.
- **Hamming** — with its length-mismatch error surfaced through
  `result<u32, string>`.
- **Jaro** and **Jaro–Winkler (classic)** — the two Jaro-family
  similarities from `comparand-jaro`.
- **Dice-over-bigrams** and **Jaccard-over-bigrams** — set
  similarities on character (byte) bigrams, using the n-gram
  generator from `comparand-ngram` as a small in-host shim
  (`byte_bigram_sets` in `src/lib.rs`) to build `GramSet<Vec<u8>>` on
  each side before calling into `comparand-set-similarity`.
- **KMP `find-first` / `find-all`** — substring search from
  `comparand-search`, defaulting to KMP because it has cheap
  preparation and predictable worst-case search time.
- **Soundex**, **NYSIIS**, **Double Metaphone (primary key)** — the
  three code-driven phonetic encoders from `comparand-phonetic`.

### Deliberately not exposed

The following algorithms live in the Rust library but are absent from
this component boundary. Each is a considered omission — adding any
of them requires more than a `func` declaration.

- **Full Damerau–Levenshtein.** Needs a `HashMap`, which needs
  `getrandom` for `RandomState`, which needs an explicit browser wire-up
  on `wasm32-unknown-unknown`. Off until a future revision adopts a
  hash function that does not require an OS RNG.
- **Alignment (Needleman–Wunsch, Smith–Waterman).** Returns a rich
  edit script that would need a `variant` per operation type plus a
  list of them. A worthwhile addition later; postponed until we have a
  concrete caller asking for it, so the wire shape is designed against
  a real workload.
- **Cosine similarity.** Would require a WIT-visible weighted-vector
  type. See "Path forward" below.
- **N-gram generator, MinHash, LSH.** Stateful; belong behind a
  `resource` type (guest-held handle) rather than a bare function.
- **BK-tree / VP-tree / q-gram index.** Stateful stores; the natural
  WIT shape is `resource index { insert; range-query; k-nearest; }`,
  which is bigger than one function and belongs in its own interface.
- **Streaming CDC, streaming rolling hashes.** Chunked WIT resources
  (per the design document) rather than one-shot functions.
- **Advanced Jaro–Winkler tuning.** Only the `classic` variant is
  exposed; a caller wanting a non-default `prefix-limit` or `scaling`
  wants a `record jaro-winkler-config` input, which is a design
  decision to make against a real use case rather than eagerly.
- **The full Double Metaphone key** (primary + alternate). Only
  `primary` is exposed; adding the alternate would return
  `record { primary: string, alternate: option<string> }`.
- **NARA-only Soundex refinement flags, NYSIIS truncation control.**
  Options behind config records; punt to a follow-up.
- **Unicode preprocessing (`comparand-unicode`).** Callers who need
  normalization or case folding across the WIT boundary want a
  dedicated `interface preprocessing { normalize-nfc, case-fold, … }`.
  Not in the seed layer.
- **Corpus, bench, and align facades.** These are consumer-side
  concerns of the Rust crate, not exposed algorithms.

Bias here is toward six-well-implemented over sixteen-with-quirks. The
next section describes how to bring the deferred algorithms in.

## Path forward

### Adding a plain function

1. Edit [`wit/comparand.wit`](wit/comparand.wit) — add a `func`
   declaration inside the appropriate interface (or add a new
   interface and `export` it from the `comparand-core` world).
2. `cd rust-host && cargo build --target wasm32-wasip1` — the
   `wit_bindgen::generate!` macro will re-expand and produce a new
   Guest trait method. The build fails with a "not all trait items
   implemented" error until you add the method.
3. Add the method body to `impl exports::comparand::core::<interface>::Guest for Component`
   in `src/lib.rs`, calling into the appropriate algorithm crate.
4. `cargo component build --release` — the produced `.wasm` now
   carries the new export.
5. Document the addition in the "Exposed" section of this README.

### Adding a stateful resource

The natural home for the deferred stateful things (indexes, workspace
pools, streaming CDC, prepared patterns) is WIT `resource` types.
Sketch:

```wit
interface search {
    resource prepared-pattern {
        constructor(pattern: list<u8>);
        find-first: func(haystack: list<u8>) -> option<u32>;
        find-all:   func(haystack: list<u8>) -> list<u32>;
    }
}
```

On the Rust side, wit-bindgen generates a `GuestPreparedPattern` trait
with `new`, `find_first`, `find_all` methods on a `Component`-owned
struct. The rest of the mapping mirrors the plain-function case.

Once one such resource exists as an in-repo reference, the deferred
BK-tree, VP-tree, workspace-pool, and MinHash resources follow the
same pattern and can be added in the same wave.

## Verification checklist for changes

Every PR touching `component/` should pass:

- `cd rust-host && cargo fmt -- --check`
- `cd rust-host && cargo clippy --target wasm32-wasip1 --all-targets -- -W clippy::pedantic`
- `cd rust-host && cargo component build --release`
- `wasm-tools validate rust-host/target/wasm32-wasip1/release/comparand_component_host.wasm`
- `wasm-tools component wit rust-host/target/wasm32-wasip1/release/comparand_component_host.wasm | grep "^  export comparand"`
  — should list every interface the world declares.

The last check catches silent export-table drift: if a Guest impl
is forgotten and the world declares more than the binary supplies,
`cargo component build` succeeds but the missing export never
appears here.
