# Language-pack DRY: reducing boilerplate across the 40-plus per-language crates

## Purpose

Waves 5-14 filled the workspace with 40+ `stringcheese-<bcp47>` crates
(`stringcheese-en`, `stringcheese-de`, ..., `stringcheese-zh`, most
recently `stringcheese-am`, `-ka`, `-pa`). Each pack was hand-authored
against the same skeleton and, in aggregate, carries several kilobytes
of near-identical shape — file docs, `#![deny(unsafe_code)]` rationale
paragraphs, `impl Language for X`, `register_language!` wiring,
`tests/registry_integration.rs`, `Cargo.toml` `[features]` stanzas,
`[dev-dependencies]` proptest wasm-gate comments, `meta::VERSION`
modules.

Adding a new pack means copying and mechanically editing all of it.
Every stanza duplicated across N crates is N places where a genuine
improvement (better docs on the wasm gate; a stricter smoke test;
a lint fix that ripples through the derive block) turns into N pull
requests.

This document inventories the boilerplate, proposes concrete
mechanisms for each cleanup, ranks them by value-over-churn, and
records the choice of proof-of-concept.

Related reading:

- [`docs/design/language-architecture.md`](./language-architecture.md) —
  the broader per-language-pack architecture, generator, and detection
  tier design. The DRY work here is a follow-up to §1 of that doc,
  which already anticipated that "LLM should design the abstraction
  and maybe implement 2-3 representative languages, then stop writing
  language implementations."
- [`docs/design/scope-and-decomposition.md`](./scope-and-decomposition.md) —
  the umbrella boundary and per-crate decomposition.

## Constraints

Any cleanup here has to preserve every existing invariant, not just
most of them:

- **Runtime crate weight.** Callers who depend on `stringcheese-am`
  must not silently pull in extra runtime crates or extra
  `[dependencies]`. All of the shared machinery has to live in a
  build-time or dev-only slot (either the existing
  `stringcheese-lang-gen` `[build-dependencies]` crate, the existing
  `stringcheese-lang` `[dependencies]` crate as a `#[macro_export]`ed
  macro, or a new `[dev-dependencies]`-only shim).
- **Wasm gate.** Every mechanism has to compile cleanly on
  `wasm32-wasip1` and the wasm-runtime CI job. Anything that expands
  to a `linkme::distributed_slice` static or names
  `stringcheese_lang::registry` must carry the same
  `not(target_family = "wasm")` gate the existing
  `register_language!` and `registry` do — see
  `crates/stringcheese-lang/src/lib.rs`.
- **`#![deny(unsafe_code)]`.** No new mechanism can force a per-pack
  `#[allow(unsafe_code)]` — the existing per-pack allow is scoped to
  the single `register_language!` site.
- **Compile-time cost.** Proc macros are OK where they buy real
  power; a `macro_rules!` macro is preferable for anything a decl
  macro can express, so packs don't grow a `syn`/`quote`
  compile-cost.
- **Debuggability.** A macro that hides tests from `cargo test
  --list` output would confuse callers. Macros that emit `#[test]
  fns` under fixed names satisfy this — each test still shows up in
  the standard output.

## Inventory

Reading `stringcheese-en`, `stringcheese-zh`, `stringcheese-am`,
`stringcheese-ka`, and `stringcheese-pa` alongside
`stringcheese-lang/src/registry.rs` and
`stringcheese-lang-gen/src/lib.rs`, the following shapes are
duplicated across ~44 language-pack crates. Line counts are for the
per-file boilerplate portion, not the pack-specific algorithm code.

| # | Location | Shape | Lines duplicated / pack | Multiplier |
|---|----------|-------|-------------------------|------------|
| 1 | `tests/registry_integration.rs` | File-level `#![cfg(...)]`, `use registry`, `KEEP_<CODE>` const, three `#[test] fn <lang>_pack_...` functions | ~40 lines | 44 packs |
| 2 | `src/lib.rs` `mod pack { ... }` for zero-config packs | `pub struct Foo; static ENC: ... = ...; impl Language for Foo { ... } pub const FOO: Foo = Foo;` | ~35 lines | ~35 packs (skips the -en / -de / -zh / -no shapes that carry builders) |
| 3 | `src/lib.rs` crate-level rationale docs | The `#![deny(unsafe_code)]` block comment explaining why `deny` and not `forbid` | ~10 lines | 44 packs |
| 4 | `src/lib.rs` `pub mod meta { pub const VERSION: &str = env!("CARGO_PKG_VERSION"); }` | Four-line boilerplate | ~4 lines | 44 packs |
| 5 | `Cargo.toml` | `[features] default = ["std"]; std = [...]; alloc = [...]; [dependencies] stringcheese-lang = { workspace = true }; stringcheese-phonetic = { workspace = true }; [dev-dependencies]; [target.'cfg(not(target_family = "wasm"))'.dev-dependencies] proptest = "1"` plus the wasm-gate rationale comment | ~15 lines | 44 packs |

Total duplicated boilerplate: on the order of **4 500 lines** across
44 packs, with the `tests/registry_integration.rs` alone accounting
for ~2 200 lines.

### What actually varies per pack

For opportunity #1 (the registry integration test), the deltas are:

- **BCP-47 code** (`"am"`, `"pa"`, `"en"`, ...) — a 2-3 char string
  literal.
- **Human-readable name** (`"Amharic"`, `"Punjabi"`, ...) — a string
  literal.
- **Pack type path** (`stringcheese_am::Amharic`, ...) — one
  identifier chain, twice: once for the `KEEP_<CODE>` const's type,
  once for the const's initializer value.
- **Pack singleton path** (`stringcheese_am::AMHARIC`) — one
  identifier chain.
- **Optional functional smoke** — usually two lines
  (`assert!(lang.is_stopword("..."))`, `assert_eq!(lang.stem(...), ...)`);
  scripts vary but the *shape* is fixed.
- **Optional extras** — `zh` asserts BCP-47 fallback from `zh-CN`
  and `zh-Hans-CN`; `no` / `nn` assert the macrolanguage `"no"` is
  **not** registered by either; `sr` runs the smoke closure over both
  Cyrillic and Latin surface forms. All optional; can be tacked on
  as regular `#[test] fn` alongside the macro invocation.

Every other line in the file — the header module docs paragraph, the
`#![cfg(not(target_family = "wasm"))]`, the `use
stringcheese_lang::registry`, the `KEEP_<CODE>` const's rationale
comment, the `#[allow(dead_code)]`, the `for probe in ["AM", "Am",
"aM"] { ... }` case-insensitive assertion — is identical.

## Opportunities

### Opportunity A — shared `pack_registry_smoke_test!` macro (highest value)

**Mechanism.** Add a `#[macro_export] macro_rules! pack_registry_smoke_test`
to `stringcheese-lang/src/lib.rs` (alongside `register_language!`
which already lives there). The macro expands to the three canonical
`#[test] fn`s plus the `KEEP_<CODE>` `const`, all of it wrapped in the
same `not(target_family = "wasm")` gate. The pack's
`tests/registry_integration.rs` becomes:

```rust
#![cfg(not(target_family = "wasm"))]

stringcheese_lang::pack_registry_smoke_test! {
    pack: stringcheese_am::AMHARIC,
    pack_ty: stringcheese_am::Amharic,
    code: "am",
    name: "Amharic",
    smoke: |lang| {
        assert!(lang.is_stopword("እና"));
        assert_eq!(lang.stem("ልጅኦች"), "ልጅ");
    },
}
```

That collapses each 47-line file to ~14 lines. Packs that need
extra assertions (`zh`, `no`, `nn`, `sr`) tack them on as plain
`#[test] fn`s after the macro invocation — the macro doesn't
close the namespace.

**Case-insensitive probes.** The macro computes an uppercase and a
mixed-case variant at runtime (`code.to_ascii_uppercase()` and an
alternating case walk); this drops the manually-authored `for probe in
["AM", "Am", "aM"]` array and works for codes of any length (the
existing `sr` / `am` / `pa` are two chars; `mkpt` / `mksr` in the
registry mocks are four).

**Tradeoffs.**

- (+) No new crate, no new `[dev-dependencies]` entry. `stringcheese-lang`
  is already in every pack's `[dependencies]`, so `#[macro_export]`
  makes the macro visible in the pack's test binary for free.
- (+) `macro_rules!` — no proc-macro compile cost, no `syn`/`quote`.
- (+) Emitted test function names are fixed (`pack_is_registered`,
  `pack_registration_is_case_insensitive`,
  `pack_functions_through_registry`); each `registry_integration.rs`
  is its own test binary, so there's no cross-file name collision.
  Tests still show up individually in `cargo test --list`.
- (+) A future change to the smoke shape (say, a fourth
  post-registration assertion) lands in one place instead of 44.
- (-) The invocation ties per-pack test code slightly more tightly to
  `stringcheese-lang`. That's already the case — every pack depends
  on it — but it makes `stringcheese-lang` a slightly higher-blast-radius
  crate.
- (-) Macro-authored tests are marginally less legible to a first-time
  reader than hand-written ones. The macro's docstring documents what
  the four fields expand to.

**Migration cost.** ~44 files, ~3 minutes each (mechanical
replacement, then `cargo test -p stringcheese-<code>` to verify).
Total: **~2-3 hours** of mechanical work, or one afternoon
distributed across the packs, plus one commit per pack (or one big
commit).

### Opportunity B — `zero_config_pack!` macro for the plain `Language` impl

**Mechanism.** Add a `#[macro_export] macro_rules! zero_config_pack`
to `stringcheese-lang` that emits the whole `mod pack { ... }` block
for packs whose `Language` impl carries a bare stopword-slice, a
`Cow`-returning stemmer that delegates to a `Stemmer` type, a
`Box<dyn Iterator>` tokenizer, and one optional phonetic-encoder
adapter. The pack invokes:

```rust
stringcheese_lang::zero_config_pack! {
    struct: Amharic,
    const: AMHARIC,
    code: "am",
    name: "Amharic",
    stopwords: crate::stopwords::STOPWORDS,
    stemmer: crate::stemmer::LightAmharicStemmer,
    tokenizer: crate::tokenizer::AmharicTokenizer,
    phonetic: crate::phonetic::AmharicPhonexAdapter,
}
```

and gets the `pub struct`, the `impl Language`, the singleton `const`,
the private `static` for the phonetic adapter, and the `Copy / Clone /
Debug / Default / PartialEq / Eq / Hash` derives — all with the
correct `alloc`-gating.

**Tradeoffs.**

- (+) Removes ~35 lines of nearly-identical Language impl from ~35
  packs (~1 200 lines total).
- (+) A capabilities-driven change (say, adding a new `Language`
  trait method with a default impl and a per-pack override) becomes a
  three-line macro edit.
- (-) Only fits the *zero-config* shape. Packs with builders (`-en`
  with `English::with_porter2`, `-de` with a similar shape, `-zh`
  with the `no`/`nn` macrolanguage split, etc.) fall through to the
  hand-written form. Migration is **selective**, not blanket.
- (-) Higher blast-radius than #A: the macro touches every pack's
  runtime `Language` impl, not just its test binary. A macro bug
  compiles to a runtime bug in ~35 packs simultaneously; the mitigation
  is a compile-time test in `stringcheese-lang` proving the expansion
  is correct.
- (-) A future per-pack twist (e.g. "this one pack needs a custom
  `collator`") requires either falling back to the hand-written form
  or growing the macro's field list. The latter path leads to
  macro-input-schema drift over time.

**Migration cost.** ~35 packs × ~5 minutes each (macro rewrite plus
`cargo test`); ~3 hours total. But the migration is *risky enough* to
want a pass-per-pack review, so realistic budget: **~1 full day**.

### Opportunity C — shared `#![deny(unsafe_code)]` rationale block

**Mechanism.** Move the block comment from every pack's `src/lib.rs`
into `docs/design/language-pack-dry.md` (this document); replace the
per-pack comment with a one-liner like:

```rust
// See docs/design/language-pack-dry.md § "unsafe_code deny rationale"
// for why this crate uses `deny` and not `forbid`.
#![deny(unsafe_code)]
```

**Tradeoffs.**

- (+) Drops ~10 lines × 44 packs = **~440 lines** of duplicated
  rationale.
- (+) Future refinements to the rationale (say, a link to the
  eventual `forbid(unsafe_code)` + macro-scoped allow when Rust
  supports it) go in one place.
- (-) A pack maintainer skimming `src/lib.rs` has to click through
  to a design doc to see the rationale. Marginal ergonomic hit, but
  the block-comment already tells the reader "same as every other
  pack" — the doc-doc-block is more informative than 44 copies of the
  same paragraph.

**Migration cost.** Trivial — 44 sed-replaceable comment blocks.
**~1 hour** of mechanical work; no risk.

### Opportunity D — shared `Cargo.toml` `[features]` and `[dev-dependencies]`

**Mechanism.** Cargo has **no direct feature-inheritance mechanism**
across workspace members — `[features]` stanzas can't be pulled in
from `[workspace.package]` or `[workspace.dependencies]`. Two paths:

1. **Convention only:** a `docs/design/pack-cargo-toml.template.toml`
   that maintainers copy from. No compile-time enforcement; the
   duplication remains in the workspace but has a canonical source.
2. **`cargo xtask` linter:** a small xtask (`cargo xtask
   verify-pack-cargo-toml`) that diffs each `stringcheese-<lang>/Cargo.toml`
   against the template's shared sections and errors out on drift.
   This would run in CI.

**Tradeoffs.**

- (+) Cheap to author (~half a day for the xtask); free thereafter.
- (-) Doesn't reduce the *line count* in the workspace, only the
  drift risk. The DRY value here is smaller than #A / #B / #C.
- (-) Adds an xtask, another moving piece in the build.

**Migration cost.** Half a day to author the xtask; zero migration
churn (the `Cargo.toml` files stay as-is).

### Opportunity E — shared `pub mod meta { pub const VERSION }` module

**Mechanism.** Trivial — a `#[macro_export] macro_rules! pack_meta`
in `stringcheese-lang` that expands to the whole `pub mod meta { pub
const VERSION: &str = env!("CARGO_PKG_VERSION"); }` block, invoked as
`stringcheese_lang::pack_meta!();`. Or, alternatively, a `pub const
VERSION: &str` accessor on the `Language` trait itself (default impl:
`env!("CARGO_PKG_VERSION")` — but that would give the wrong crate's
version, so this path doesn't work; the macro is the only viable
form).

**Tradeoffs.**

- (+) Drops ~4 lines × 44 packs = **~180 lines**.
- (-) Value is small. `pub mod meta { const VERSION }` is already
  the smallest unit of boilerplate in the pack; hiding it behind a
  macro makes each pack marginally less self-documenting.

**Migration cost.** Trivial. **~30 minutes.**

## Ranking

Ranking by (value delivered) / (churn incurred):

| # | Opportunity | Lines saved | Churn (files) | Risk | Rank |
|---|-------------|-------------|---------------|------|------|
| A | `pack_registry_smoke_test!` macro | ~2 200 | 44 test files | Low (test-only) | **1** |
| C | Shared `unsafe_code` rationale | ~440 | 44 `src/lib.rs` files (comment only) | None | **2** |
| B | `zero_config_pack!` macro | ~1 200 | ~35 `src/lib.rs` files | Medium (runtime code) | **3** |
| D | Cargo.toml template + xtask | 0 (drift only) | 0 workspace files, +1 xtask | Low | **4** |
| E | `pack_meta!` macro | ~180 | 44 `src/lib.rs` files | None | **5** |

**A is the clear winner.** Half the total duplication for the lowest
risk (test-only, no runtime blast radius), and the mechanism is a
single decl macro in a crate the packs already depend on.

**B is the highest-payoff runtime change** but wants a
per-pack review pass, so it's a separate follow-up rather than the
proof-of-concept.

**C is a free follow-on to A** — same commit could take it too, but
splitting keeps each cleanup's rationale legible in the git log.

## Proof of concept

**Chosen: Opportunity A on `stringcheese-am`.**

Rationale:

- Highest value in the ranking table.
- Lowest risk — test-only, no runtime effect on any pack.
- `stringcheese-am` is the most recent pack (added 2026-08-08), so
  the diff is the freshest reference for the pattern.
- The macro's design surfaces every subtle piece (wasm-gate, KEEP
  const, case-insensitive probes, smoke closure) in one commit — a
  future maintainer rolling out to the other 43 packs can copy the
  pattern one file at a time and validate incrementally.

### The macro

Added to `crates/stringcheese-lang/src/lib.rs`:

```rust
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! pack_registry_smoke_test {
    (
        pack: $pack:path,
        pack_ty: $pack_ty:ty,
        code: $code:literal,
        name: $name:literal,
        smoke: | $lang_binding:ident | $smoke:block $(,)?
    ) => {
        // ... KEEP const, three #[test] fns, all wasm-gated ...
    };
}
```

Invocation in `crates/stringcheese-am/tests/registry_integration.rs`:

```rust
#![cfg(not(target_family = "wasm"))]

stringcheese_lang::pack_registry_smoke_test! {
    pack: stringcheese_am::AMHARIC,
    pack_ty: stringcheese_am::Amharic,
    code: "am",
    name: "Amharic",
    smoke: |lang| {
        assert!(lang.is_stopword("እና"));
        assert_eq!(lang.stem("ልጅኦች"), "ልጅ");
    },
}
```

### Verification

- `cargo test -p stringcheese-am --all-features --locked` — all
  `registry_integration` tests pass, alongside the pack's other
  hand-written tests.
- `cargo build -p stringcheese-am --target wasm32-wasip1
  --no-default-features --features alloc` — wasm build stays clean
  (the macro invocation is wasm-gated at the file level; the
  emitted code inside the macro is also wasm-gated).
- `RUSTFLAGS="-D warnings" cargo clippy -p stringcheese-am
  --all-targets --all-features --locked` — no new lints.
- `RUSTFLAGS="-D warnings" cargo clippy -p stringcheese-lang
  --all-targets --all-features --locked` — no new lints on the
  macro-owning crate either.

### Migration estimate if the human approves rollout

- ~43 remaining language packs.
- ~3-5 minutes per pack (open `tests/registry_integration.rs`,
  identify the smoke closure lines and any extras, rewrite as a
  macro invocation plus any tacked-on `#[test] fn`s, run `cargo test
  -p stringcheese-<code>`).
- Special cases needing per-pack judgment:
  - `stringcheese-zh` — carries two extra `#[test] fn`s for BCP-47
    fallback from `zh-CN` and `zh-Hans-CN`. Keep as-is beneath the
    macro invocation.
  - `stringcheese-no` / `-nn` — carry a
    `macrolanguage_no_is_not_registered_by_this_pack` test. Keep
    as-is beneath the macro invocation.
  - `stringcheese-sr` — smoke closure covers both Cyrillic and
    Latin. Fits the macro's `smoke:` field cleanly, just longer.
- Total estimate: **2-3 hours** of mechanical work; one commit per
  pack or one big commit; ~44 files touched; ~1 900 net lines
  removed.

## Follow-ups not covered here

- **Opportunity B (zero_config_pack!)** — separate design pass, once
  the smoke-test macro has settled in and given the workspace a
  reference for `stringcheese-lang`-owned macros.
- **Opportunity C (shared unsafe_code rationale)** — free follow-on
  to any pack that changes for another reason. Doesn't warrant its
  own rollout wave.
- **Opportunity D (Cargo.toml linter)** — worth doing when the
  workspace grows large enough that drift bites (say, a `[features]`
  addition needing to land in all 44 packs at once).
- **Opportunity E (pack_meta! macro)** — low priority; roll into
  Opportunity B's wave if B goes ahead.
- **Extend the `stringcheese-lang-gen` build-time generator** to emit
  the whole `mod pack { ... }` block (currently emits only the
  stopword slice and `CAPABILITIES`). This would obviate Opportunity
  B for the zero-config packs and centralize the pack shape in
  `stringcheese-lang-gen`. Deferred as a wave-16 topic.
