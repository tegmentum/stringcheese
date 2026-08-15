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

## Known follow-ups

* **~~wasmtime 26 → 46 major bump.~~** ***Landed.*** The six
  `stringcheese-*-component` crates now pin `wasmtime = "46"` /
  `wasmtime-wasi = "46"` for the WIT component-model smoke tests, up
  from `26`. This closed 19 advisories including the two
  9.0-critical sandbox-escape entries (RUSTSEC-2026-0095 Winch
  sandbox-escape memory, RUSTSEC-2026-0096 aarch64 Cranelift sandbox
  escape). The `wasip1 → wasip2` reactor adapter shipped alongside
  wasmtime 46 emits the `wasi:@0.2.12` import surface, up from the
  earlier `wasi:@0.2.1` — the WIT sources in `component/wit/` import
  no `wasi:` interfaces themselves, so the world bump lives in the
  adapter/host coupling only. wasmtime 46 requires rustc 1.94; the
  workspace `rust-version` stays at 1.88 (the component crates'
  public API compiles unchanged, only their dev-deps for the smoke
  tests need the newer toolchain) and the MSRV CI row excludes the
  six `*-component` crates from its test sweep. See the
  `.cargo/audit.toml` history for the full 19-advisory table this
  bump closed.
* **bio 2 → 4 bump.** Two unmaintained transitives (`custom_derive`,
  `fxhash`) enter the tree only through the `oracle-benches` feature of
  `stringcheese-align`. A bio major bump drops both.
* **atomic-polyfill.** `wasmtime 46 -> cranelift -> postcard ->
  heapless 0.7` still pulls in the unmaintained `atomic-polyfill`
  crate. `heapless 0.8` drops the polyfill; clearing this ignore
  needs upstream `postcard` and/or `wasmtime` to bump. Informational
  only, no CVE.
