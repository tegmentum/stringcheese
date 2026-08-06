# Publishing StringCheese

Playbook for cutting a StringCheese release and pushing the initial
GitHub repository. Follow this end-to-end the first time; on subsequent
releases the GitHub-repo section is a no-op and only the crates.io
sequence matters.

## Pre-flight checklist

Before running any publish command:

- [ ] `cargo test --workspace --all-features --locked` — green.
- [ ] `cargo clippy --workspace --all-features --all-targets --locked -- -D warnings` — clean.
- [ ] `cargo fmt --all -- --check` — clean.
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked` — clean.
- [ ] `cargo publish --dry-run -p <crate> --allow-dirty --locked` — SUCCESS for
      every crate in the order below (the ship-rehearsal wave runs this
      sweep and records the result matrix).
- [ ] Working tree clean (`git status --short` empty).
- [ ] `CHANGELOG.md` has an entry for the version being cut; the
      `## [Unreleased]` heading has been renamed to the release tag.
- [ ] Every crate's `Cargo.toml` has `description`, `license`,
      `repository`, `categories`, and `keywords` filled in. The
      workspace root supplies `license`, `repository`, and `authors`
      via `workspace.package`; each crate must supply its own
      `description`, `categories`, and `keywords`.
- [ ] The workspace version in `Cargo.toml` matches the tag you intend
      to publish, and every publishable crate inherits it via
      `version.workspace = true`.

## GitHub repository (one-time)

The GitHub remote does not yet exist. Bootstrap it with the `gh` CLI:

```sh
# Assumes `gh auth login` has been completed.
gh repo create tegmentum/stringcheese \
    --public \
    --source=. \
    --description="Rigorous sequence comparison for Rust and WebAssembly" \
    --push
```

After the push, the `[![CI]]` badge in `README.md` will start
resolving; the `[![crates.io]]` and `[![docs.rs]]` badges start
resolving once the `stringcheese` facade is published for the first time.

## crates.io token (one-time)

```sh
# Generate a scoped API token at https://crates.io/settings/tokens
# with the "publish-new" and "publish-update" scopes. Then:
cargo login <token>
```

The token is stored in `~/.cargo/credentials.toml`; there is no
project-local configuration.

## Publish order

Every publishable crate is `0.1.0` and lives under
`crates/`. `stringcheese-bench` has `publish = false` and is deliberately
skipped. The dependency graph dictates the following strict order —
crates.io refuses to accept a crate whose declared dependencies do not
yet exist on the index.

1. `stringcheese-core` — substrate; depends on nothing in the workspace.
2. `stringcheese-corpus` — depends on `stringcheese-core`.
3. First-wave algorithm crates (parallelizable within this tier but
   sequenced here for clarity):
   1. `stringcheese-levenshtein`
   2. `stringcheese-hamming`
   3. `stringcheese-jaro`
   4. `stringcheese-damerau`
   5. `stringcheese-lcs`
   6. `stringcheese-ngram`
   7. `stringcheese-unicode`
   8. `stringcheese-phonetic`
   9. `stringcheese-align`
   10. `stringcheese-search`
   11. `stringcheese-cdc`
   12. `stringcheese-minhash`
4. Second-wave crates that depend on algorithm crates:
   1. `stringcheese-set-similarity` — depends on `stringcheese-ngram`.
   2. `stringcheese-index` — depends only on `stringcheese-core` at build
      time; the `levenshtein` / `damerau` requirements are `dev-dependencies`
      and do not gate publication.
5. `stringcheese` — the facade; re-exports every algorithm crate above.

## Publish sequence

crates.io's registry index can take a few seconds to propagate a newly
published crate before the next `cargo publish` can resolve it as a
dependency. Sleeping 10 seconds between crates is the conventional
safety margin.

```sh
set -euo pipefail

crates=(
    stringcheese-core
    stringcheese-corpus
    stringcheese-levenshtein
    stringcheese-hamming
    stringcheese-jaro
    stringcheese-damerau
    stringcheese-lcs
    stringcheese-ngram
    stringcheese-unicode
    stringcheese-phonetic
    stringcheese-align
    stringcheese-search
    stringcheese-cdc
    stringcheese-minhash
    stringcheese-set-similarity
    stringcheese-index
    stringcheese
)

for crate in "${crates[@]}"; do
    echo "==> publishing ${crate}"
    cargo publish -p "${crate}" --locked
    echo "    sleeping 10s for registry-index propagation"
    sleep 10
done
```

If a publish fails midway (network hiccup, index conflict, yanked
version), the surviving prefix is already on crates.io. Comment out
the completed crates in the loop, wait for the index to settle
(usually a minute), and resume from the first crate that had not yet
published.

## Post-publish

- Tag the release: `git tag v0.1.0 && git push --tags`.
- Verify each crate landed: browse `https://crates.io/crates/stringcheese`
  and the algorithm crates.
- Verify docs.rs successfully built the facade with all features:
  `https://docs.rs/stringcheese`. docs.rs uses each crate's default
  feature set unless overridden — the facade's default `std` feature
  transitively enables every sub-crate, so a plain build is
  sufficient.
- Rename the CHANGELOG's `## [Unreleased]` heading to
  `## [0.1.0] — YYYY-MM-DD` and open a fresh `## [Unreleased]` on top
  for the next cycle.

## What NOT to do

- Do not publish `stringcheese-bench`. It has `publish = false` for a
  reason (it depends on host-only benchmark tooling and is not
  intended to be consumed as a library).
- Do not bump crate versions individually. The workspace pins every
  crate to `version.workspace = true`; releases move in lockstep.
- Do not skip the dry-run sweep, even for a patch release. crates.io
  metadata rules change occasionally, and the sweep is the only place
  those changes are caught before an irreversible publish.
