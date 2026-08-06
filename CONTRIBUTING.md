# Contributing to StringCheese

Thank you for considering a contribution. StringCheese aims to be the
canonical Rust library for sequence comparison, which means the bar for
correctness, documentation, and API coherence is high. This document
describes how to work in the repository so a pull request lands with
minimum friction.

## Getting started

Clone and build:

```sh
git clone https://github.com/tegmentum/stringcheese.git
cd stringcheese
cargo test --workspace --all-features
```

A first-time build compiles roughly a dozen crates. When the run
completes you should see a series of green `test result: ok. …` lines
— one per crate's unit-test binary and one per crate's doc tests —
concluding with a final zero-failure summary. If any of those turn red,
stop and open an issue; the tree is expected to be green on `main`.

The workspace pins its minimum supported Rust version (MSRV) at
`1.85` and uses edition `2024`. Any Rust toolchain at or above that
version is fine for development.

## Project structure

The workspace is split into small, focused crates. The map is
maintained in [`README.md`](README.md); the short version is:

- **`stringcheese-core`** — traits, result types, `AlgorithmDescriptor`,
  `MetricProperties`, workspace and sequence abstractions. Every other
  crate depends on this one.
- **`stringcheese-corpus`** — golden-case schema, oracle framework,
  exhaustive generators, differential vocabulary. Test-only for the
  algorithm crates but a first-class published deliverable in its own
  right.
- **`stringcheese-compare`** — the consolidated comparison-kernel crate.
  One top-level module per algorithm family (`levenshtein`, `hamming`,
  `jaro`, `damerau`, `lcs`, `ngram`, `search`, `set_similarity`,
  `minhash`); each module is ownership-scoped so it can evolve
  independently within a single unit of publication.
- **`stringcheese-<subsystem>`** — sibling crates for the
  non-comparison subsystems: `stringcheese-unicode`,
  `stringcheese-phonetic`, `stringcheese-cdc`, `stringcheese-index`,
  `stringcheese-align`, `stringcheese-bench`.
- **`stringcheese`** — a thin facade that re-exports the stable public API.

Design context lives under [`docs/`](docs/):

- [`docs/DESIGN.md`](docs/DESIGN.md) — project vision, CI requirements,
  release gates.
- [`docs/design/type-system.md`](docs/design/type-system.md) — how
  descriptors, metric properties, and result types compose.
- [`docs/design/preprocessing-pipeline.md`](docs/design/preprocessing-pipeline.md)
- [`docs/design/phonetic-subsystem.md`](docs/design/phonetic-subsystem.md)
- [`docs/design/ngram-and-fingerprinting.md`](docs/design/ngram-and-fingerprinting.md)
- [`docs/design/wasm-and-wit-interface.md`](docs/design/wasm-and-wit-interface.md)

Read the relevant design doc before touching a subsystem — most API
questions are already answered there.

## Adding a new algorithm

This is the workflow most contributions follow. The order matters:
skipping steps causes rework.

1. **Pick a family.** Every algorithm belongs to an `AlgorithmFamily`
   in `stringcheese-core::descriptor`. If your algorithm does not fit an
   existing family, propose adding a variant to that enum in a separate
   PR before starting on the implementation. `AlgorithmFamily` is
   `#[non_exhaustive]` specifically so new variants are additive.

2. **Pick a variant slug.** Slugs are the stable string keys used in
   the golden corpus and public reporting. Convention is
   `<family>-<qualifier>`, lowercase, hyphenated:
   `levenshtein-unit`, `hamming-byte`, `jaro-winkler-default`. Once
   published a slug is API and cannot be renamed without a deprecation
   cycle.

3. **Decide DistanceMetric vs SimilarityMetric vs both.**
   `stringcheese-core` exposes distinct `DistanceMetric` and
   `SimilarityMetric` traits, plus normalized counterparts. Distance
   is not similarity; do not paper over the distinction by picking one
   and casting. If your algorithm is fundamentally a distance,
   implement `DistanceMetric` and let a caller convert. If it is
   fundamentally a similarity (Jaro is), implement `SimilarityMetric`.
   Some families support both — implement both if so, and document the
   relationship between them.

4. **Decide MetricProperties.** Every descriptor declares the
   mathematical properties its metric satisfies: reflexivity, symmetry,
   triangle inequality, boundedness, and so on. Be conservative — a
   claimed property becomes a property test. Refer to
   [`docs/design/type-system.md`](docs/design/type-system.md) for the
   semantics of each flag.

5. **Write the `AlgorithmDescriptor` const.** Put it in the crate's
   `algorithm` module, `pub const`, one per variant. This is the
   registry entry the corpus references and the release report cites.

6. **Wire golden cases to the corpus schema.** Add cases to the
   crate's `golden.rs` (or `golden/` submodule). Cases must use the
   schema from `stringcheese_corpus`, must reference the descriptor's
   variant slug, and must include enough coverage to demonstrate every
   documented edge case — empty inputs, identical inputs, maximally
   different inputs, unicode boundary cases where applicable.

7. **Add property tests for the metric axioms.** Use
   `proptest`. Every property your `MetricProperties` claims must be
   exercised: symmetry, identity of indiscernibles, triangle
   inequality, etc. Also add a differential test against the internal
   oracle if the crate has multiple kernels — the whole point of the
   oracle-plus-optimized split is that they must agree.

### Canonical examples

Each of the three shipped algorithms illustrates a different point on
the spectrum:

- [`crates/stringcheese-compare/src/hamming`](crates/stringcheese-compare/src/hamming) — the minimal
  case: single kernel, single descriptor. Start here to understand the
  smallest possible shape a module can take.
- [`crates/stringcheese-compare/src/levenshtein`](crates/stringcheese-compare/src/levenshtein) — the
  multi-kernel case: `full_matrix.rs` (oracle), `rolling_rows.rs`
  (production), `banded.rs` (Ukkonen cutoff variant), plus a shared
  `workspace.rs` for scratch-buffer reuse. Study the module split
  before splitting your own algorithm across files.
- [`crates/stringcheese-compare/src/jaro`](crates/stringcheese-compare/src/jaro) — the
  `FloatExpectation` case: because Jaro is a floating-point
  similarity, its golden cases and property tests use `FloatExpectation`
  to control comparison tolerance rather than exact equality.

## Style guide

- **Documentation first.** The workspace enables
  `missing_docs = warn`. Every `pub` item — module, type, function,
  const, variant — needs rustdoc. Reviewers will bounce PRs that add
  new public API without documentation.
- **No emoji.** In code, comments, commit messages, docs, changelog
  entries, or PR descriptions. This project treats prose as a load-
  bearing surface and emoji tend to erode it.
- **`#![forbid(unsafe_code)]`.** Every crate that ships an algorithm
  should forbid unsafe code at the crate root. If an implementation
  genuinely requires unsafe (SIMD intrinsics, for example), it belongs
  in a dedicated module gated behind a feature flag and reviewed
  separately.
- **Workspace lints.** `[workspace.lints]` in the root `Cargo.toml`
  enables the pedantic clippy pass and warns on `missing_docs` and
  `rust_2018_idioms`. Do not silence workspace lints in a crate; if a
  lint fires on a legitimate pattern, use `#[allow(…, reason = "…")]`
  with a real reason string at the smallest scope that works.
- **Conventional Commits.** Commit subjects follow
  [Conventional Commits 1.0](https://conventionalcommits.org/en/v1.0.0):
  `feat(jaro): …`, `fix(levenshtein): …`, `docs: …`, `chore: …`,
  `refactor(core): …`. Scope in parentheses is the crate short name
  (`core`, `hamming`, `corpus`, etc.) or a broader area (`docs`,
  `ci`).
- **Do not mention Claude, Anthropic, or any AI assistant** in code,
  commits, PR descriptions, changelogs, or docs. Contributions stand
  on their content, not their tooling.

## Unsafe code policy

- **Default: `#![forbid(unsafe_code)]` in every crate.** Every algorithm
  crate, every substrate crate, and every future addition starts with a
  crate-level `forbid(unsafe_code)`. That default is not aspirational —
  it is enforced at compile time, and CI will refuse to merge a crate
  that quietly removes it.
- **The one exception: `component/rust-host`.** The Wasm component-host
  crate uses `#![deny(unsafe_op_in_unsafe_fn)]` (not `forbid`) because
  the ABI plumbing `wit-bindgen` generates for lowering `list<u8>` and
  friends across the component boundary is `unsafe` at the language
  level. Every line of hand-written code in that crate is still safe
  Rust; the exception exists only so that macro-generated ABI code can
  compile without spraying `unsafe fn` wrappers through the surface. A
  file-level comment in `component/rust-host/src/lib.rs` records the
  rationale next to the attribute.
- **Rule for future work.** Any new crate defaults to
  `forbid(unsafe_code)`. If a genuine need for unsafe arises (SIMD
  intrinsics, FFI stubs, additional ABI plumbing, a hot path that must
  bypass a bounds check for measurable reason), the exception must be:
  1. **Scoped** — behind a dedicated feature flag when the unsafe is
     optional, or crate-scoped when it is intrinsic to the crate's
     purpose. Do not sprinkle `unsafe` blocks through an otherwise-safe
     crate.
  2. **Documented in-file** — a file-level comment above the crate-root
     attribute explains why the exception exists and what invariant the
     unsafe code upholds.
  3. **Documented here** — add a bullet to this list naming the crate,
     the attribute, and a one-line reason. A reviewer who has never
     seen the codebase should be able to enumerate every unsafe-using
     crate from this file alone.

## Testing

Every algorithm PR should include:

- **Unit tests.** Small, targeted, colocated with the code they
  exercise. Doc-tests count as unit tests for coverage purposes.
- **Property tests via `proptest`.** One test per declared metric
  property. See
  `crates/stringcheese-compare/src/hamming/property_tests.rs` for
  the pattern.
- **Differential tests (oracle vs optimized).** When a module has more
  than one kernel, one is the oracle and the others must agree with it
  on generated inputs. See `crates/stringcheese-compare/src/levenshtein`
  for the reference layout.
- **Golden cases.** Fixed input-and-expected-output records that
  survive refactors and document the intended behavior. Use the
  `stringcheese-corpus` schema so a shared runner can execute them.
- **Cross-target verification.** New algorithms must build under
  `--no-default-features` and `--no-default-features --features alloc`,
  and — until [`docs/design/wasm-and-wit-interface.md`](docs/design/wasm-and-wit-interface.md)
  says otherwise — should cross-compile to `wasm32-unknown-unknown`
  and `wasm32-wasip1`. The `wasm` CI job runs these checks with
  `continue-on-error: true` until the whole workspace is clean;
  aiming for green on your PR is nonetheless the expected default.
- **`no_std` compatibility.** If a crate declares `alloc`, its kernel
  must work without `std`. Compile with `cargo check -p <crate>
  --no-default-features --features alloc` before opening the PR.

## Running CI locally

CI runs these commands, in this order. Run them locally before you
open the PR:

```sh
cargo test    --workspace --all-features --locked
cargo clippy  --workspace --all-features --all-targets --locked -- -D warnings
cargo fmt     --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
```

For each crate that declares an `alloc` feature:

```sh
cargo check -p <crate> --no-default-features --locked
cargo check -p <crate> --no-default-features --features alloc --locked
```

For the wasm cross-compile checks (requires `rustup target add
wasm32-unknown-unknown wasm32-wasip1` once):

```sh
cargo check --workspace --exclude stringcheese-bench \
  --all-features --target wasm32-unknown-unknown --locked
cargo check --workspace --exclude stringcheese-bench \
  --all-features --target wasm32-wasip1 --locked
```

If any of these fail on `main` in a fresh clone, that is a bug worth
an issue — the tree should be clean.

## Pull request process

- **Branch off `main`.** Never commit to `main` directly.
- **Keep commits focused.** One logical change per commit. Squash
  fixup commits before opening the PR.
- **One algorithm per PR when possible.** A new `AlgorithmDescriptor`,
  its kernel(s), its golden cases, and its property tests form a
  natural unit. Bundling two algorithms doubles review time and halves
  the chance the second one lands.
- **Keep PRs reviewable in a single sitting.** A rough ceiling is
  about 500 lines net. Longer PRs need a strong argument for why they
  cannot be split.
- **Update the changelog.** Add an entry under `[Unreleased]` in
  [`CHANGELOG.md`](CHANGELOG.md) if the change is user-visible.
- **Fill in the PR template.** The template exists so reviewers can
  find the important bits without reading the whole diff.

## License

StringCheese is dual-licensed under [MIT](LICENSE-MIT) and
[Apache-2.0](LICENSE-APACHE). By submitting a contribution you agree
that it may be distributed under the same terms.
