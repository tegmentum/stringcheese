# stringcheese-tokenizer-tiktoken-conformance

Bit-identical tiktoken parity harness for `stringcheese-tokenizer-hf`.

Implements Phase 3 of the tokenizer subsystem design (see
`docs/design/tokenizers.md` § 11): fetches OpenAI's real
`mergeable_ranks` blobs by SHA-256, feeds them into
`stringcheese-tokenizer-hf`, and diffs the output against the
[`tiktoken-rs`](https://crates.io/crates/tiktoken-rs) oracle over a
corpus of ~200 diverse inputs.

## Why a separate crate

The main `stringcheese-tokenizer-tiktoken` crate is `alloc`-only and
lightweight — no HTTP, no oracle, no test-only dependencies. This
harness pulls in `ureq`, `sha2`, and the multi-MB `tiktoken-rs`
oracle; it lives in its own crate so those deps never travel
transitively.

This crate is **excluded from the top-level workspace**
(see the root `Cargo.toml`). Cargo's `--all-features` turns on every
feature of every workspace member unconditionally, and the
`parity-real-vocab` feature needs to stay opt-in — enabling it kicks
off a network fetch on first run. Excluding the crate keeps the main
workspace's `cargo test --workspace --all-features --locked` offline.

## Running the parity suite

```sh
cargo test \
    --manifest-path crates/stringcheese-tokenizer-tiktoken-conformance/Cargo.toml \
    --features parity-real-vocab
```

First run fetches the two vocabularies (`cl100k_base` and
`o200k_base`) from
`https://openaipublic.blob.core.windows.net/encodings/*.tiktoken`,
verifies them against the SHA-256 hashes hard-coded in
`src/variant.rs`, and caches them into `~/.cache/stringcheese-tokenizer-tiktoken/`
(or `$XDG_CACHE_HOME/stringcheese-tokenizer-tiktoken/`,
or `$TIKTOKEN_PARITY_DATA_DIR` if set). Subsequent runs are offline.

### Environment variables

| Variable | Purpose |
| --- | --- |
| `TIKTOKEN_PARITY_DATA_DIR` | Explicit cache directory override — bypasses XDG / `$HOME` resolution. |
| `TIKTOKEN_PARITY_OFFLINE` | Refuse to touch the network; a cache miss becomes a hard error. |
| `TIKTOKEN_PARITY_ALLOW_UNVERIFIED` | Skip SHA-256 verification. Escape hatch for OpenAI hash rotation. |
| `TIKTOKEN_PARITY_STRICT` | Promote any divergence to a hard test failure. Off by default (Phase 3 posture — report, don't gate). |

## Bench

```sh
cargo bench \
    --manifest-path crates/stringcheese-tokenizer-tiktoken-conformance/Cargo.toml \
    --features parity-real-vocab
```

Measures `stringcheese-tokenizer-hf`'s encode throughput on 8 KiB
of prose using the real `cl100k_base` vocab, alongside
`tiktoken-rs`'s `encode_ordinary` on the same input. The two rows
under the criterion group are directly comparable.

## Never commits real bytes

Per the session-standing constraint, the raw OpenAI blobs never
enter this repository. The cache lives outside `target/` and outside
the repo tree; `.gitignore` already excludes the cache root (it is
under `$HOME/.cache/`, not the repo). This crate's `src/` and
`tests/` directories carry corpus strings and metadata, never
vocabulary bytes.
