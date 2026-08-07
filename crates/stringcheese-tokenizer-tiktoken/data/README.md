# tiktoken data

This directory is the drop-point for the real OpenAI `mergeable_ranks`
blobs. When the crate is built without those files present (the default
in-tree state — the OpenAI blobs are ~5 MB each and are not committed
here for both licence and repo-bloat reasons), the crate's `build.rs`
synthesises a **small deterministic stand-in tokenizer** for each
enabled feature and embeds *that* instead. The stand-in is enough for
the crate's unit tests and for downstream integration tests that only
need "a real `BpeTokenizer` handed back by the tiktoken pack"; it will
**not** produce tiktoken-identical encodings and callers who need those
must generate real packs (see below).

## Layout

Real tiktoken packs live under `data/<variant>.tiktoken` — the same
plaintext format tiktoken publishes. When present, `build.rs` reads
them, transcodes them into the SCUD-lite BPE format documented in
`src/scud.rs`, deflate-compresses the result, and writes the
`<variant>.scud.mz` blob into `OUT_DIR` for `include_bytes!`.

```
data/
├── README.md                    # this file
├── cl100k_base.tiktoken         # optional, contributor-supplied
├── p50k_base.tiktoken           # optional, contributor-supplied
├── r50k_base.tiktoken           # optional, contributor-supplied
└── o200k_base.tiktoken          # optional, contributor-supplied
```

## Obtaining real `mergeable_ranks` blobs

The upstream tiktoken repo (<https://github.com/openai/tiktoken>) hosts
each variant's blob under `tiktoken_ext/openai_public.py`. Two paths:

1. **From `pip install tiktoken`.** After Python has cached the blobs
   they live under `~/.cache/tiktoken/` (or `$TIKTOKEN_CACHE_DIR` if
   set). Copy each file into this directory, renaming to
   `<variant>.tiktoken` (the cache filenames are content hashes).

2. **Direct download.** The upstream URLs are stable and enumerated in
   `openai_public.py`. Fetch each variant's blob, verify against the
   published SHA-256 (also in `openai_public.py`), and drop the
   plaintext into this directory.

## The `.tiktoken` file format

Each line is `<base64(bytes)> <rank>`. The parser lives in
`build.rs::parse_tiktoken_plaintext`.

## Compression

SCUD-lite is deflate-compressed via [`miniz_oxide`][mz] — pure-Rust,
same decoder Rust's own `backtrace` crate uses, and works on every
`wasm32-*` target. Brotli would be marginally smaller but pulls in a C
dependency (`brotli-sys`) or a hand-rolled Rust decoder that is
substantially larger than `miniz_oxide` itself. The design doc's
compression discussion (`docs/design/tokenizers.md` § 5.2) leaves the
choice open; this crate picks deflate for the reasons above.

[mz]: https://docs.rs/miniz_oxide

## Regenerating packs by hand

If you have the plaintext `.tiktoken` files but want to inspect or
regenerate the compressed packs outside of `cargo build`, the
`stringcheese_tokenizer_tiktoken::builder` module (feature-gated
behind `std`, always on) exposes the pipeline as public functions:

```rust
use stringcheese_tokenizer_tiktoken::builder;

let plaintext = std::fs::read("data/cl100k_base.tiktoken")?;
let scud = builder::build_scud_from_tiktoken(&plaintext)?;
let compressed = builder::compress(&scud);
std::fs::write("out/cl100k_base.scud.mz", compressed)?;
```
