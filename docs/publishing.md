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
3. First-wave crates (parallelizable within this tier but sequenced here
   for clarity):
   1. `stringcheese-compare` — the consolidated comparison-kernel crate
      (Levenshtein, Hamming, Jaro/Jaro-Winkler, Damerau/OSA, LCS,
      n-gram, set similarity, substring search, MinHash/LSH).
   2. `stringcheese-unicode`
   3. `stringcheese-phonetic`
   4. `stringcheese-align`
   5. `stringcheese-cdc`
4. Second-wave crates that depend on first-wave crates:
   1. `stringcheese-index` — depends only on `stringcheese-core` at build
      time; the Levenshtein / OSA fixtures under `dev-dependencies`
      pull in `stringcheese-compare` but do not gate publication.
5. `stringcheese` — the facade; re-exports every published crate above.

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
    stringcheese-compare
    stringcheese-unicode
    stringcheese-phonetic
    stringcheese-align
    stringcheese-cdc
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

- Do not publish `stringcheese-bench` or `stringcheese-lang-test`.
  Both carry `publish = false` for a reason: the former depends on
  host-only benchmark tooling; the latter is a `version = "0.0.0"`
  integration-test host that pulls the shipped language packs
  together for a linkme distributed-slice sanity check and would
  reintroduce a dev-dep cycle if it were ever published.
- Do not bump crate versions individually. The workspace pins every
  crate to `version.workspace = true`; releases move in lockstep.
- Do not skip the dry-run sweep, even for a patch release. crates.io
  metadata rules change occasionally, and the sweep is the only place
  those changes are caught before an irreversible publish.

## Dry-run sweep results

Last sweep: `cargo publish --dry-run --locked --allow-dirty -p <crate>`
against every leaf crate (crates whose `[dependencies]` reference no
other workspace crate) plus a `cargo publish --dry-run --locked
--workspace --allow-dirty` over the full 94-crate publishable set.

**Leaf-crate dry-run (individual `-p` invocations, GREEN):**

- `stringcheese-core`
- `stringcheese-scud`
- `stringcheese-collate`
- `stringcheese-lang-gen`
- `stringcheese-detect-script`
- `stringcheese-simhash`
- `stringcheese-pattern`
- `stringcheese-ident`
- `stringcheese-escape`

Every workspace-inherited `stringcheese-*` dep in
`[workspace.dependencies]` already carries `version = "0.1.0"`
alongside its `path = "crates/..."`, so path-only dep entries are not
a blocker. All 93 workspace deps checked.

**Non-leaf `cargo publish --dry-run -p <crate>` (EXPECTED FAIL):**

Individual dry-runs of any crate with a workspace `stringcheese-*`
dep fail with `no matching package named 'stringcheese-*' found` —
cargo checks the crates.io index for every declared dep during the
"prepare local package for uploading" step, and the sibling workspace
crates are not published yet. This is not a manifest bug: it is the
expected behaviour of `cargo publish -p X` on a fresh workspace.
Non-leaves must either (a) wait for their deps to publish first, or
(b) be exercised via the workspace-scoped mode below.

**`cargo publish --dry-run --workspace` (GREEN — full sweep):**

Cargo 1.97's workspace-scoped publish packages crates in topological
dep order, feeding earlier `.crate` archives into later verification
steps so a downstream crate can resolve its sibling deps without them
being on the registry. The full 94-crate publishable set now runs
end-to-end (`Packaging` → `Verifying` → `Uploading … aborting upload
due to dry run`) with exit code 0. Prior to the
`stringcheese-lang-test` extraction (see below) the sweep tripped at
the 13th crate — `stringcheese-de` — with
`no matching package named 'stringcheese-lang' found`; root cause was
that `stringcheese-lang` carried dev-dependencies on
`stringcheese-{en,de,fr}` (for the multi-pack
`tests/registry_integration.rs` linkme test) while each of those
packs had a regular dependency back on `stringcheese-lang`. Cargo's
topological sort collapsed the resulting dev-dep cycle by deferring
`-lang` past its own dependents, tripping the sibling-dep check on
`-de`. The fix was to hoist the linkme integration test into
`crates/stringcheese-lang-test/` — a `publish = false`, `version =
"0.0.0"` crate that dev-depends on `-lang`, `-en`, `-de`, and `-fr`.
Nothing publishes from it and no publishable crate depends on it, so
the cycle is broken at the source. Individual manual publishes
(`cargo publish -p X`) still work the same way they always did (they
skip dev-dep resolution for non-workspace registries).

**Publish order for the first workspace release (dep-graph topo
sort):**

1. Tier 0 — substrate leaves (no workspace deps): `stringcheese-core`.
2. Tier 1 — direct dependents of `-core`:
   `stringcheese-corpus`, `stringcheese-scud`, `stringcheese-align`,
   `stringcheese-unicode`, `stringcheese-phonetic`,
   `stringcheese-lang-gen`. (Independent leaves that carry no
   workspace deps can slot in anywhere: `stringcheese-collate`,
   `stringcheese-detect-script`, `stringcheese-simhash`,
   `stringcheese-pattern`, `stringcheese-ident`,
   `stringcheese-escape`, `stringcheese-translit`,
   `stringcheese-winnowing`, `stringcheese-segment`,
   `stringcheese-minhash`, `stringcheese-detect-whatlang`,
   `stringcheese-detect-lingua`.)
3. Tier 2 — algorithm crates layered on Tier 1: `stringcheese-cdc`,
   `stringcheese-index`, `stringcheese-compare`, `stringcheese-diff`,
   `stringcheese-stats`, `stringcheese-ngram`,
   `stringcheese-normalize`, `stringcheese-textsplit`,
   `stringcheese-pattern-regex`, `stringcheese-detect`,
   `stringcheese-tokenizer`, `stringcheese-lang`.
4. Tier 3 — ICU-alternative capability crates:
   `stringcheese-icu-case`, `stringcheese-icu-collation`,
   `stringcheese-icu-plural`, `stringcheese-icu-number`,
   `stringcheese-icu-datetime`, `stringcheese-icu-segment`,
   `stringcheese-icu-linebreak`.
5. Tier 4 — language packs (each depends on `-lang`, `-phonetic`,
   plus optional `-icu-*`): `stringcheese-en`, `stringcheese-de`,
   `stringcheese-fr`, `stringcheese-es`, `stringcheese-it`,
   `stringcheese-pt`, `stringcheese-ro`, `stringcheese-nl`,
   `stringcheese-no`, `stringcheese-nn`, `stringcheese-pl`,
   `stringcheese-ja`, `stringcheese-ko`, `stringcheese-zh`,
   `stringcheese-am`, `stringcheese-ar`, `stringcheese-he`,
   `stringcheese-fa`, `stringcheese-hi`, `stringcheese-mr`,
   `stringcheese-bn`, `stringcheese-pa`, `stringcheese-id`,
   `stringcheese-ta`, `stringcheese-ml`, `stringcheese-tr`,
   `stringcheese-fi`, `stringcheese-et`, `stringcheese-ru`,
   `stringcheese-uk`, `stringcheese-be`, `stringcheese-bg`,
   `stringcheese-mk`, `stringcheese-sr`, `stringcheese-cs`,
   `stringcheese-da`, `stringcheese-is`, `stringcheese-sk`,
   `stringcheese-sv`, `stringcheese-hu`, `stringcheese-vi`,
   `stringcheese-th`, `stringcheese-el`, `stringcheese-hy`,
   `stringcheese-ka`.
6. Tier 5 — tokenizer packs:
   `stringcheese-tokenizer-hf`, `stringcheese-tokenizer-hf-native`,
   `stringcheese-tokenizer-tiktoken`.
7. Tier 6 — WIT component wrappers (depend on their Tier-3 sibling
   plus embedded language packs):
   `stringcheese-tokenizer-component`,
   `stringcheese-icu-case-component`,
   `stringcheese-icu-collation-component`,
   `stringcheese-icu-datetime-component`,
   `stringcheese-icu-segment-component`,
   `stringcheese-icu-linebreak-component`.
8. Tier 7 — the umbrella facade: `stringcheese`.

**Not published (`publish = false`, deliberately skipped by the
publish loop and the `--workspace` sweep):**

- `stringcheese-bench` — host-only bench harness. Held at
  `version.workspace = true` but the manifest sets `publish = false`
  so it never enters the topo sort.
- `stringcheese-lang-test` — integration-test host for the
  `stringcheese-lang` registry's linkme distributed slice; pulls
  `-lang`, `-en`, `-de`, `-fr` together at test-link time to prove
  each pack's `register_language!` invocation lands in
  `LANGUAGES`. Pinned at `version = "0.0.0"` and `publish = false`
  precisely because dev-depending on the shipped packs from
  `-lang` itself would reintroduce the topo-sort cycle described
  above.

Tiers must publish in order. Within a tier, order is free — the
`sleep 10` in the publish loop is enough for the registry index to
propagate before the next tier resolves its deps.

The nine-crate immediate ship set (Tier 0 through the umbrella,
skipping every language pack, ICU capability, tokenizer pack, and
component wrapper) is the sequence codified in the loop above; the
rest of the tree ships in follow-up waves as the ecosystem picks
them up.
