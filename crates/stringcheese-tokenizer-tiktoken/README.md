# stringcheese-tokenizer-tiktoken

OpenAI tiktoken model tokenizer pack for the
[StringCheese](https://github.com/tegmentum/stringcheese) toolkit.

Ships four canonical OpenAI BPE vocabularies as [SCUD-lite] BPE data
packs layered on top of the
[`stringcheese-tokenizer-bpe`](https://crates.io/crates/stringcheese-tokenizer-bpe)
algorithm crate:

| Feature       | Variant       | Model families                |
| ------------- | ------------- | ----------------------------- |
| `cl100k_base` | `cl100k_base` | GPT-3.5, GPT-4                |
| `p50k_base`   | `p50k_base`   | Codex, some GPT-3.5           |
| `r50k_base`   | `r50k_base`   | GPT-3                         |
| `o200k_base`  | `o200k_base`  | GPT-4o, o1                    |

Each variant lives behind its own Cargo feature so a caller who only
needs `cl100k_base` never embeds the other three vocabularies. The
default feature set is `["cl100k_base"]` — the common case for a
call-site that just wants "count tokens for a GPT-4 prompt."

## Usage

```rust
use stringcheese_tokenizer::Tokenizer;
use stringcheese_tokenizer_tiktoken::CL100K_BASE;

let tokenizer = CL100K_BASE.get();  // decompress + parse on first call, cached thereafter
let enc = tokenizer.encode("Hello, world!")?;
let round = tokenizer.decode(&enc.ids)?;
assert_eq!(round, "Hello, world!");
```

## Data provenance

Real OpenAI `mergeable_ranks` blobs are **not committed** to this
repository — the file sizes (~5 MB each) and the licence review that a
bulk vendor would need are both out of scope for the v0.2-track
deliverable. When `data/<variant>.tiktoken` is not present, the
crate's `build.rs` synthesises a small deterministic stand-in
tokenizer per variant so the pipeline (SCUD-lite parse →
`BpeTokenizer` → `encode`/`decode`) is fully exercisable in tests.

See [`data/README.md`](./data/README.md) for how to drop real
plaintext `.tiktoken` blobs into the crate for a real build.

## Compression

SCUD-lite is deflate-compressed via
[`miniz_oxide`](https://crates.io/crates/miniz_oxide) — pure-Rust,
same decoder Rust's own `backtrace` crate uses, and works on every
`wasm32-*` target the workspace ships to.

## Licence

MIT OR Apache-2.0. The tiktoken data format itself is upstream
(<https://github.com/openai/tiktoken>) under a BSD-style licence; no
tiktoken source is vendored into this crate.

[SCUD-lite]: ./src/scud.rs
