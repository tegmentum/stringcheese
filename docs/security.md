# Supply-chain security

Two CI jobs sweep the workspace against public supply-chain policy on
every push and pull request: `security-audit` (RustSec advisories) and
`dep-policy` (bans / licenses / duplicate versions / registry sources).
Both are marked `continue-on-error: true` while the initial ignore list
bakes in — a red mark is a follow-up, not a merge blocker. Promote to
hard-fail once each job has stayed green across a couple of weeks of
merge traffic.

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

* **wasmtime 26 → 46 major bump.** All 19 wasmtime advisories the
  initial sweep surfaced are fixed only in `wasmtime >= 36` (the six
  `stringcheese-*-component` crates pin `wasmtime = "26"` for the WIT
  component-model smoke tests). The runtime loads only trusted, in-tree
  wasm modules, so the residual risk is bounded to the CI runner during
  test — but a coordinated bump (including the WASI `0.2.1 → 0.2.3`
  world upgrade across the six component crates) closes them out.
* **bio 2 → 4 bump.** Two unmaintained transitives (`custom_derive`,
  `fxhash`) enter the tree only through the `oracle-benches` feature of
  `stringcheese-align`. A bio major bump drops both.
