//! Bit-identical tiktoken parity harness for
//! [`stringcheese-tokenizer-hf`](https://crates.io/crates/stringcheese-tokenizer-hf).
//!
//! This crate implements Phase 3 of the tokenizer subsystem design
//! (`docs/design/tokenizers.md` § 11). Its job is to answer the
//! question "does `BpeTokenizer` produce byte-identical output to
//! upstream `OpenAI` tiktoken across a diverse corpus?" — the same
//! bar Phase 3 sets, with ≥ 10 000 diverse inputs as the eventual
//! target and ~200 inputs as the tractable first slice.
//!
//! # Why a separate crate
//!
//! Two reasons the parity harness lives outside
//! `stringcheese-tokenizer-tiktoken`:
//!
//! 1. **Cargo `--all-features` isolation.** The main workspace's CI
//!    runs `cargo test --workspace --all-features --locked`, which
//!    turns on every feature of every workspace member. The parity
//!    harness needs network access (or a pre-populated cache) and
//!    pulls in the multi-MB `tiktoken-rs` oracle — neither should be
//!    on the default CI path. This crate is EXCLUDED from the
//!    top-level workspace so its `parity-real-vocab` feature stays
//!    truly opt-in.
//! 2. **Dependency hygiene.** Callers who ship
//!    `stringcheese-tokenizer-tiktoken` in production get a lean
//!    `alloc`-only crate with only a tiny deflate decoder attached.
//!    They should not transitively pull in HTTP, hash, or an oracle
//!    tokenizer just because their test builds want conformance
//!    coverage.
//!
//! # Fetch mechanism
//!
//! When the `parity-real-vocab` feature is enabled the harness:
//!
//! * Consults the cache first — `$XDG_CACHE_HOME/stringcheese-tokenizer-tiktoken/`
//!   or `~/.cache/stringcheese-tokenizer-tiktoken/` (or
//!   `$TIKTOKEN_PARITY_DATA_DIR` if set — useful for CI runners
//!   without a home directory).
//! * Falls back to HTTP `GET` from `OpenAI`'s public CDN, verified by
//!   the SHA-256 hash declared in [`variant::Variant`]. A mismatch
//!   is an error; the harness never accepts a blob whose hash the
//!   crate does not know.
//! * Never writes the raw bytes anywhere the crate might
//!   inadvertently commit. The cache lives outside the repo; the
//!   default `.gitignore` already excludes `target/`, and the cache
//!   root is not inside `target/`.
//!
//! The design doc's § 6.2 sketches this shape; the concrete cache
//! path convention follows the tiktoken Python client's own
//! `$TIKTOKEN_CACHE_DIR` idiom so a contributor who already has a
//! Python tiktoken cache can reuse it.
//!
//! # Usage
//!
//! ```no_run
//! # #[cfg(feature = "parity-real-vocab")] {
//! use stringcheese_tokenizer_tiktoken_conformance::{
//!     corpus, run_parity, variant::CL100K_BASE,
//! };
//!
//! let report = run_parity(&CL100K_BASE, corpus::CORPUS).unwrap();
//! assert_eq!(report.divergences.len(), 0, "{report:#?}");
//! # }
//! ```
//!
//! Without the feature the runtime surface still compiles (the
//! corpus, variant metadata, and cache helpers do not require the
//! oracle) so callers can hand-drive the plumbing.

#![forbid(unsafe_code)]

pub mod cache;
pub mod corpus;
pub mod variant;

#[cfg(feature = "parity-real-vocab")]
pub mod fetch;

#[cfg(feature = "parity-real-vocab")]
pub mod parity;

#[cfg(feature = "parity-real-vocab")]
pub use parity::{Divergence, ParityReport, build_stringcheese_tokenizer, run_parity};
