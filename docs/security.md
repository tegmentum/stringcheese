# Supply-chain security

Two CI jobs sweep the workspace against public supply-chain policy on
every push and pull request: `security-audit` (RustSec advisories) and
`dep-policy` (bans / licenses / duplicate versions / registry sources).
Both are hard-fail merge blockers — a red mark stops the PR. They were
promoted from `continue-on-error: true` once the wasmtime 26 -> 46
coordinated bump closed out the last 19-entry advisory tail (see
"Known follow-ups" below).

## What runs when

| Job              | Tool          | Config                     | Duration (cached) |
| ---------------- | ------------- | -------------------------- | ----------------- |
| `security-audit` | `cargo-audit` | `.cargo/audit.toml`        | ~1 s              |
| `dep-policy`     | `cargo-deny`  | `deny.toml`                | ~1 s              |

Both tools are cached in `~/.cargo/bin` on the CI runner and only
reinstall when the pinned version in the workflow env block changes.

## Local reproducer

```
cargo install --locked --version 0.22.0 cargo-audit
cargo install --locked --version 0.19.9 cargo-deny

# Advisory scan (RustSec DB).
cargo audit

# Full sweep (advisories + licenses + bans + sources).
cargo deny check
```

Both commands are workspace-root only. They pick up
`.cargo/audit.toml` and `deny.toml` automatically.

## Adding an exemption

Every ignore is temporary by policy. Adding one requires a one-line
rationale in the surrounding comment block AND a follow-up path (bump,
upstream migration, or dep swap).

* **`.cargo/audit.toml`** — for `cargo audit`. Comment above each ID
  explains why the advisory is accepted.
* **`deny.toml`** — the `[advisories].ignore` list mirrors
  `.cargo/audit.toml` one-for-one. Keep the two in lockstep or a green
  audit run may still trip the deny job.

Duplicate-version warnings are informational (`multiple-versions =
"warn"`); the top three offenders as of the initial pass are called
out in `deny.toml` and should be revisited when the ecosystem lands
majority-adopted upgrades.

## Unsafe-code policy

Every `unsafe {}` block and every body of an `unsafe fn` MUST carry a
`// SAFETY:` comment stating the invariant the caller (or the
surrounding call site) upholds. The rule is enforced by the workspace
lint

```toml
[workspace.lints.clippy]
undocumented_unsafe_blocks = "deny"
```

promoted from the pedantic-group `warn` default. CI already runs
`RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
--all-features --locked`, so a missing SAFETY comment fails the build
without any additional wiring. The initial promotion audited the ~262
existing `unsafe` sites in tree — every one already carried a comment;
enabling the lint at `deny` locks the invariant so a future patch
cannot silently regress.

**How to write the SAFETY comment.** One or two lines, placed on the
line(s) *immediately preceding* the `unsafe` block (after any
attributes on that block, not before them). State the invariant the
caller upholds — the CPU-feature check, the length precondition, the
lifetime, the pointer-provenance argument — using the same variable
names as the surrounding code so the reader can trace it. Example:

```rust
#[allow(unsafe_code, reason = "SIMD intrinsic wrappers are unsafe by declaration")]
// SAFETY: is_x86_feature_detected!("avx2") returned true; NEON is
// enabled via `#[target_feature]` and `off + BLOCK <= len` upholds
// the 16-byte read from `a`.
let simd_result = unsafe { simd::x86_avx2::distance(&a, &b) };
```

Two adjacent `unsafe { … }` blocks each need their own comment (the
lint checks per block, not per statement). A shared invariant can be
referenced tersely on the second block — `// SAFETY: as above, `k * 2
+ 1 < 64` under the loop bound.` — but the comment must be present.

**Machine-generated code.** The `src/bindings.rs` files that
`wit-bindgen` emits into every `*-component` crate contain many
`unsafe extern` blocks without SAFETY comments. Each such module is
declared with

```rust
#[allow(
    unsafe_code,
    unsafe_op_in_unsafe_fn,
    missing_docs,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::restriction
)]
mod bindings;
```

so the pedantic-group `undocumented_unsafe_blocks` lint is silenced
for generated code only. Do not re-emit the generated file to add
SAFETY comments — it will be overwritten the next time
`wit-bindgen` runs. If you add a NEW component crate, mirror the same
`#[allow]` on its `mod bindings;` declaration.

## Known follow-ups

* **~~wasmtime 26 → 46 major bump.~~** ***Landed.*** The 26 → 46
  jump closed 19 advisories including the two 9.0-critical
  sandbox-escape entries (RUSTSEC-2026-0095 Winch sandbox-escape
  memory, RUSTSEC-2026-0096 aarch64 Cranelift sandbox escape). The
  `wasip1 → wasip2` reactor adapter shipped alongside wasmtime 46
  emits the `wasi:@0.2.12` import surface, up from the earlier
  `wasi:@0.2.1` — the WIT sources in `component/wit/` import no
  `wasi:` interfaces themselves, so the world bump lives in the
  adapter/host coupling only.
* **~~wasmtime 46 → 47.0.3 minor bump.~~** ***Landed.*** The six
  `stringcheese-*-component` crates now pin `wasmtime = "47"` /
  `wasmtime-wasi = "47"`. 47.0.3 closes two additional advisories
  on top of 46.0.2: GHSA-hgjw-h833-99q9 (stores mixing up type
  indices between engines) and GHSA-2hw9-mc66-jc2q (preemption and
  traps during bulk operations breaking internal VM state). Both
  land in the runtime we exercise. Notes: 47.0.0 removed
  `wasi-common` and the wasi-threads support (we use `wasmtime-wasi`
  and don't run threaded wasm, so unaffected) and dropped several
  Cranelift `*_imm` / `stack_load` / `bxor_not` / `global_value`
  IR opcodes (we don't emit Cranelift IR ourselves, so unaffected).
  MSRV requirement stays at 1.94 for these dev-deps; the workspace
  `rust-version` remains 1.88 and the MSRV CI row continues to
  exclude the six `*-component` crates from its test sweep.
* **bio 2 → 4 bump.** Two unmaintained transitives (`custom_derive`,
  `fxhash`) enter the tree only through the `oracle-benches` feature of
  `stringcheese-align`. A bio major bump drops both.
* **atomic-polyfill.** `wasmtime 47 -> cranelift -> postcard ->
  heapless 0.7` still pulls in the unmaintained `atomic-polyfill`
  crate. `heapless 0.8` drops the polyfill; clearing this ignore
  needs upstream `postcard` and/or `wasmtime` to bump. Informational
  only, no CVE.
