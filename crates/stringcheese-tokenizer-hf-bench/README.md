# stringcheese-tokenizer-hf-bench

Comparative encode-throughput bench for `stringcheese-tokenizer-hf`
against upstream `tokenizers-rs` (Hugging Face) and `tiktoken-rs`
(`OpenAI`) on real vocabularies.

## Why a separate crate

The oracle dependencies (`tokenizers = "0.21"`, `tiktoken-rs = "0.6"`)
are ~2 minutes of fresh compile between them and drag a large
transitive tree into `Cargo.lock`. Keeping the bench inside
`stringcheese-bench` meant every `cargo test --workspace
--all-features --locked` CI run resolved and compiled both oracle
stacks even when nothing about the bench had changed. This crate is
**excluded from the top-level workspace** so those deps never travel
transitively; the main workspace's `--all-features` job stays
offline and lightweight, and this bench only compiles when a
contributor (or the dedicated CI job) explicitly requests it.

Mirrors the split shape used by
`stringcheese-tokenizer-tiktoken-conformance` for the same reason.

## Running

```sh
cargo bench \
  --manifest-path crates/stringcheese-tokenizer-hf-bench/Cargo.toml \
  --features parity-real-vocab \
  --bench tokenizer_hf
```

Filter to one group:

```sh
cargo bench \
  --manifest-path crates/stringcheese-tokenizer-hf-bench/Cargo.toml \
  --features parity-real-vocab \
  --bench tokenizer_hf -- gpt2
```

Three groups — gpt2 (BPE byte-level), cl100k_base (tiktoken shape),
llama_2_7b (SentencePiece byte_fallback + Metaspace) — at 1 KiB, 10
KiB, 100 KiB of deterministic English prose. See
`benches/tokenizer_hf.rs`'s module docs for the last-measured
baseline table and the rationale for each group.

The bench soft-skips per-group at runtime when the vocab is missing
(an `eprintln!` explains where to drop it), so enabling the feature
is safe on a naked checkout — no failure, just no numbers.

## Provisioning the vocabs

Real vocab bytes are **never** committed to this repository. The
bench looks up each checkpoint's `tokenizer.json` (and, for cl100k,
the tiktoken plaintext blob) via a two-root lookup:

* `tokenizer.json` (gpt2, llama-2-7b-hf):
  * `$STRINGCHEESE_REAL_VOCABS_DIR/<checkpoint>/tokenizer.json`
  * `crates/stringcheese-tokenizer-hf/tests/conformance/vocabs/<checkpoint>/tokenizer.json`
* `cl100k_base.tiktoken` plaintext:
  * `$TIKTOKEN_PARITY_DATA_DIR/cl100k_base.tiktoken`
  * `~/.cache/stringcheese-tokenizer-tiktoken/cl100k_base.tiktoken`

The second cl100k root is populated automatically the first time
the `stringcheese-tokenizer-tiktoken-conformance` suite runs with
its `parity-real-vocab` feature (it fetches + SHA-256-verifies the
blob from `OpenAI`'s CDN). For the HF-shape `tokenizer.json` files,
download from Hugging Face and drop them at either root — the
conformance suite uses the same convention.

## Never commits real bytes

Per the session-standing constraint, the raw upstream blobs never
enter this repository. The two lookup roots point either at a
contributor-provisioned cache or at the `stringcheese-tokenizer-hf`
fixture directory (which is itself `.gitignore`d for vocab
subdirectories).
