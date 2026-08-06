# WebAssembly Build Recipes

Status: Reference
Applies to: StringCheese 0.1 and later
Related: [DESIGN.md](./DESIGN.md), [design/wasm-and-wit-interface.md](./design/wasm-and-wit-interface.md)

This document records the exact `(crate × target × feature-set)` matrix that
StringCheese verifies in CI on every push. It is the executable, machine-checked
projection of the "WebAssembly is a primary deployment target" commitment
made in [design/wasm-and-wit-interface.md](./design/wasm-and-wit-interface.md).

The matrix is enforced by the `wasm` job in
[.github/workflows/ci.yml](../.github/workflows/ci.yml); the shape recorded
here is the source of truth for what "wasm-supported" means in this
workspace.

## Targets

Two wasm targets are exercised on every crate:

| Target                     | Std availability          | Rationale                                                                                                                      |
|----------------------------|---------------------------|--------------------------------------------------------------------------------------------------------------------------------|
| `wasm32-wasip1`            | Full std (WASI syscalls)  | Closer to native. Verifies the crate works in Wasmtime, Wasmer, and any WASI Preview 1 host — the target for edge/server wasm. |
| `wasm32-unknown-unknown`   | Core primitives only [^1] | Pure no-std substrate. The target every browser bundle ultimately hits; catches std leaks that `wasm32-wasip1` masks.          |

[^1]: `std` is nominally available on `wasm32-unknown-unknown` — the standard
      library builds — but many std APIs are stubs (thread spawn, file I/O,
      system time, RNG seeding). `cargo check` type-checks against them; a
      real call at runtime traps or panics. This matrix therefore proves
      compile-time compatibility; runtime behavior on
      `wasm32-unknown-unknown` still needs a real integration harness (see
      "Future work" below).

## Feature sets

Every crate is checked under three feature configurations:

| Label                          | Cargo flags                             | What it proves                                                                                                       |
|--------------------------------|-----------------------------------------|----------------------------------------------------------------------------------------------------------------------|
| `all-features`                 | `--all-features`                        | Every feature the crate declares is on. The heaviest possible dependency graph — catches transitive std leaks.       |
| `no-default+alloc`             | `--no-default-features --features alloc`| The canonical browser/embedded configuration: `no_std` with a heap. Every alloc-capable crate must succeed here.     |
| `no-default`                   | `--no-default-features`                 | Pure `no_std`, no heap. Substrate-only surface for embedded consumers that link StringCheese without an allocator.      |

## Crate coverage

The matrix runs against the workspace crates that ship code:

- `stringcheese` (facade)
- `stringcheese-core`
- `stringcheese-corpus`
- `stringcheese-compare` (the consolidated comparison-kernel crate:
  Levenshtein, Hamming, Jaro/Jaro-Winkler, Damerau/OSA, LCS, n-gram,
  set similarity, substring search, MinHash/LSH)
- `stringcheese-unicode`
- `stringcheese-phonetic`

Excluded:

- `stringcheese-bench` — the benchmark harness. Depends on host-only
  timing/IO machinery (criterion); benchmark code is not a wasm target.
  Excluded at the CI level via per-crate iteration rather than a
  workspace-wide `--exclude`.
- `stringcheese-cdc`, `stringcheese-index`, `stringcheese-align` — added
  to the matrix as their runtime cross-target validations land.

## Compatibility matrix

Every cell below is verified on every CI run. All 66 combinations pass
under the workspace's current source; a regression in any cell fails the
`wasm` job and blocks the merge.

| Crate                        | wasip1 all | wasip1 alloc | wasip1 core | unknown all | unknown alloc | unknown core |
|------------------------------|:----------:|:------------:|:-----------:|:-----------:|:-------------:|:------------:|
| `stringcheese`                  | PASS       | PASS         | PASS        | PASS        | PASS          | PASS         |
| `stringcheese-core`             | PASS       | PASS         | PASS        | PASS        | PASS          | PASS         |
| `stringcheese-corpus`           | PASS       | PASS         | PASS        | PASS        | PASS          | PASS         |
| `stringcheese-compare`          | PASS       | PASS         | PASS        | PASS [^2]   | PASS          | PASS         |
| `stringcheese-unicode`          | PASS       | PASS         | PASS        | PASS [^3]   | PASS          | PASS         |
| `stringcheese-phonetic`         | PASS       | PASS         | PASS        | PASS        | PASS          | PASS         |

[^2]: `stringcheese-compare`'s full-Damerau module uses
      `std::collections::HashMap` and is gated behind the `std` feature.
      `HashMap`'s type is available in the standard library on
      `wasm32-unknown-unknown`, so `cargo check` succeeds. Constructing a
      `HashMap` at runtime lazily initializes `RandomState`, which needs
      `getrandom` — and `getrandom` requires explicit browser/JS wiring on
      `wasm32-unknown-unknown` (see
      [`getrandom` docs](https://docs.rs/getrandom/latest/getrandom/#webassembly-support)).
      A crate that runs full-Damerau in a browser must add a top-level
      dependency on `getrandom` with the `js` feature — this crate does not
      make that choice for its callers.
[^3]: `stringcheese-unicode --all-features` pulls in `unicode-normalization`,
      `unicode-segmentation`, and `icu_casemap` (all `no_std + alloc` when
      built with `compiled_data`). All three compile cleanly on both wasm
      targets.

## Reproducing locally

The matrix is one shell loop. Reproduce it with:

```bash
rustup target add wasm32-unknown-unknown wasm32-wasip1

crates=(
  stringcheese stringcheese-core stringcheese-corpus
  stringcheese-compare stringcheese-unicode stringcheese-phonetic
)
targets=(wasm32-wasip1 wasm32-unknown-unknown)
flag_sets=(
  "--all-features"
  "--no-default-features --features alloc"
  "--no-default-features"
)

for crate in "${crates[@]}"; do
  for target in "${targets[@]}"; do
    for flags in "${flag_sets[@]}"; do
      echo ">>> $crate $target $flags"
      cargo check -p "$crate" --target "$target" $flags --locked
    done
  done
done
```

## What this matrix does not prove

`cargo check` is compile-time verification. Two runtime concerns remain and
are tracked separately:

- **Runtime execution.** `cargo check` type-checks against std stubs on
  `wasm32-unknown-unknown` — it does not confirm a stub is never called.
  A `cargo test` pass under `wasmtime` or `wasm-bindgen-test` (per the
  target-matrix commitment in
  [design/wasm-and-wit-interface.md § Cross-target validation](./design/wasm-and-wit-interface.md))
  is the next verification layer.
- **Binary size.** Compile success says nothing about the wasm binary
  size targets in [design/wasm-and-wit-interface.md § Binary size targets](./design/wasm-and-wit-interface.md).
  A `twiggy` / `wasm-opt` reporting harness is future work.

Both are called out here so a future maintainer does not treat a green
wasm CI job as full validation of the "WebAssembly first" claim — the
compile-time projection is the current, not the eventual, checkpoint.

## Adding a new crate to the matrix

1. Land the crate under `crates/stringcheese-<name>/` with proper `std` /
   `alloc` feature declarations (see the design document for the
   discipline every crate follows).
2. Confirm the 6 combinations locally with the loop above.
3. Add the crate name to the `crates` array in
   [.github/workflows/ci.yml](../.github/workflows/ci.yml)'s `wasm` job.
4. Add a row to the matrix table above.
5. If any combination cannot be made to pass, footnote it here and
   configure the CI loop to skip that combination explicitly (never with
   `continue-on-error`).
