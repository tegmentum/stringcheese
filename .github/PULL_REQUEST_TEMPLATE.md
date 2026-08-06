<!--
Keep this short. The reviewer's job is faster if you tell them what
changed and why, and what to look at first.
-->

## What changed

<!-- One or two sentences. Reference the crate(s) touched. -->

## Why

<!-- Motivation. Link the issue if one exists. -->

## Algorithm / variant descriptors added or modified

<!--
For work that adds or changes an AlgorithmDescriptor, list each one as:

- family: <AlgorithmFamily variant>
  variant slug: <string>
  metric kind: distance / similarity / both
  properties: <MetricProperties summary>

Delete this section if no descriptor is affected.
-->

## Tests added

<!--
- Unit tests: …
- Property tests (proptest): axioms exercised: …
- Differential tests (oracle vs optimized): …
- Golden cases added to stringcheese-corpus: …
-->

## Feature flags touched

<!--
List each `[features]` entry added, changed, or removed. Note whether
default-features behavior changes.
-->

## WASM / no_std impact

<!--
- Does this compile with `--no-default-features`?
- Does this compile with `--no-default-features --features alloc`?
- Does this cross-compile to `wasm32-unknown-unknown` and `wasm32-wasip1`?
- If any answer is "no", say why and whether that is intentional.
-->

## Pre-merge checklist

- [ ] `cargo test --workspace --all-features` passes locally
- [ ] `cargo clippy --workspace --all-features --all-targets -- -D warnings` is clean
- [ ] `cargo fmt --all -- --check` passes
- [ ] Public items have rustdoc (`missing_docs = warn` is on)
- [ ] `CHANGELOG.md` updated under `[Unreleased]` if user-visible
- [ ] Commit messages follow Conventional Commits
