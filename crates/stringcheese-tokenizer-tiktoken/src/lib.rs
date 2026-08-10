//! `OpenAI` tiktoken model tokenizer pack for the StringCheese toolkit.
//!
//! This crate ships four canonical `OpenAI` BPE vocabularies —
//! `cl100k_base` (GPT-3.5 / GPT-4), `p50k_base` (Codex + some
//! GPT-3.5), `r50k_base` (GPT-3), and `o200k_base` (GPT-4o / o1) — as
//! [SCUD-lite] BPE data packs layered on top of the
//! [`stringcheese-tokenizer-hf`][bpe] algorithm crate.
//!
//! Each variant lives behind its own Cargo feature so a caller who
//! only needs `cl100k_base` never embeds the other three vocabularies.
//! The default feature set is `["cl100k_base"]`.
//!
//! [SCUD-lite]: crate::scud
//! [bpe]: stringcheese_tokenizer_hf
//!
//! # Public API
//!
//! Every enabled variant exposes a single `pub static`:
//!
//! ```no_run
//! # #[cfg(feature = "cl100k_base")] {
//! use stringcheese_tokenizer::Tokenizer;
//! use stringcheese_tokenizer_tiktoken::CL100K_BASE;
//!
//! let tokenizer = CL100K_BASE.get();
//! let enc = tokenizer.encode("Hello, world!").unwrap();
//! let round = tokenizer.decode(&enc.ids).unwrap();
//! assert_eq!(round, "Hello, world!");
//! # }
//! ```
//!
//! [`get`][TiktokenPack::get] lazily decompresses and parses the
//! embedded SCUD-lite pack the first time it is called, then hands
//! back a `&'static BpeTokenizer` on every subsequent call. The
//! decoded tokenizer is cached in a [`std::sync::OnceLock`], so the
//! parse cost is paid once per process.
//!
//! # Data provenance
//!
//! Real tiktoken `mergeable_ranks` blobs are **not** committed to this
//! repository — the file sizes (~5 MB each) and the licence review
//! that a bulk vendor would need are both out of scope for the
//! v0.2-track deliverable. When `data/<variant>.tiktoken` is not
//! present, the crate's `build.rs` synthesises a small deterministic
//! stand-in tokenizer per variant so the pipeline (SCUD-lite parse →
//! `BpeTokenizer` → `encode`/`decode`) is fully exercisable in tests.
//!
//! To ship a real pack, drop the upstream plaintext blob into
//! `data/<variant>.tiktoken` — the crate's `build.rs` will transcode
//! it into SCUD-lite deflate on the next build. See
//! [`data/README.md`](https://github.com/tegmentum/stringcheese) for
//! the file-format specifics.
//!
//! # `no_std`
//!
//! The runtime crate requires `std` for [`std::sync::OnceLock`]
//! and file I/O — SCUD-lite loading itself is `alloc`-only, but the
//! ergonomic `.get()` cache is std-typed. Contributors who need
//! `no_std` embedding can call [`scud::load_pack`] directly and manage
//! the cached tokenizer themselves.
//!
//! # References
//!
//! * <https://github.com/openai/tiktoken> — upstream `OpenAI` tokenizer.
//! * `docs/design/tokenizers.md` § 5–6 — SCUD-lite BPE format and the
//!   tiktoken pack's role in the tiered crate layout.

#![forbid(unsafe_code)]

extern crate alloc;

pub mod builder;
pub mod scud;

use std::sync::OnceLock;

use stringcheese_tokenizer::TokenizerError;
use stringcheese_tokenizer_hf::BpeTokenizer;

/// A pre-configured tiktoken variant packaged as an embedded SCUD-lite
/// blob plus a lazy-decode cache.
///
/// The public API is [`get`](Self::get) — call it whenever a
/// `&'static BpeTokenizer` is required. Loading is deferred: the
/// decompress + parse happens the first time `get` runs on a given
/// pack; every subsequent call returns the cached tokenizer with no
/// allocation.
pub struct TiktokenPack {
    /// The variant name — `"cl100k_base"`, `"o200k_base"`, etc.
    /// Callers can read this for logging or telemetry.
    pub name: &'static str,
    /// The deflate-compressed SCUD-lite pack, embedded via
    /// `include_bytes!`.
    compressed: &'static [u8],
    /// The one-time cache. Initialised on first call to `get`.
    once: OnceLock<BpeTokenizer>,
}

impl TiktokenPack {
    /// Return the cached [`BpeTokenizer`] for this variant, loading
    /// it from the embedded SCUD-lite pack on first call.
    ///
    /// # Panics
    ///
    /// Panics if the embedded pack fails to decompress or parse. This
    /// would only happen if the build script produced a malformed
    /// blob — a bug in this crate, not a caller-recoverable
    /// condition. Contributors who need a fallible surface should
    /// call [`try_get`](Self::try_get) instead.
    pub fn get(&'static self) -> &'static BpeTokenizer {
        self.once.get_or_init(|| {
            scud::load_pack(self.compressed).unwrap_or_else(|e| {
                panic!(
                    "tiktoken pack {name:?} failed to load: {e}",
                    name = self.name
                )
            })
        })
    }

    /// Fallible counterpart to [`get`](Self::get). Returns
    /// `Err(TokenizerError)` if the embedded pack fails to load.
    ///
    /// Once the pack has been loaded (successfully or otherwise) this
    /// method behaves identically to [`get`](Self::get) on subsequent
    /// calls.
    ///
    /// # Errors
    ///
    /// Returns [`TokenizerError::Other`] with a short message when
    /// decompression or SCUD-lite parsing fails.
    pub fn try_get(&'static self) -> Result<&'static BpeTokenizer, TokenizerError> {
        if let Some(existing) = self.once.get() {
            return Ok(existing);
        }
        let parsed = scud::load_pack(self.compressed)?;
        // Race: another thread may win the initialisation. Both
        // outcomes are equivalent — a `BpeTokenizer` from the same
        // input is bit-identical up to construction order.
        Ok(self.once.get_or_init(|| parsed))
    }

    /// The compressed byte size of the embedded pack, for size
    /// diagnostics.
    #[must_use]
    pub const fn compressed_size(&self) -> usize {
        self.compressed.len()
    }
}

impl core::fmt::Debug for TiktokenPack {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TiktokenPack")
            .field("name", &self.name)
            .field("compressed_size", &self.compressed.len())
            .field("loaded", &self.once.get().is_some())
            .finish()
    }
}

// ---------------------------------------------------------------------
// One `pub static` per enabled variant. Each is behind its own Cargo
// feature so a caller pays for exactly what they need.
// ---------------------------------------------------------------------

/// `cl100k_base` — GPT-3.5 / GPT-4 family.
#[cfg(feature = "cl100k_base")]
pub static CL100K_BASE: TiktokenPack = TiktokenPack {
    name: "cl100k_base",
    compressed: include_bytes!(concat!(env!("OUT_DIR"), "/cl100k_base.scud.mz")),
    once: OnceLock::new(),
};

/// `p50k_base` — Codex + some GPT-3.5 checkpoints.
#[cfg(feature = "p50k_base")]
pub static P50K_BASE: TiktokenPack = TiktokenPack {
    name: "p50k_base",
    compressed: include_bytes!(concat!(env!("OUT_DIR"), "/p50k_base.scud.mz")),
    once: OnceLock::new(),
};

/// `r50k_base` — GPT-3.
#[cfg(feature = "r50k_base")]
pub static R50K_BASE: TiktokenPack = TiktokenPack {
    name: "r50k_base",
    compressed: include_bytes!(concat!(env!("OUT_DIR"), "/r50k_base.scud.mz")),
    once: OnceLock::new(),
};

/// `o200k_base` — GPT-4o / o1.
#[cfg(feature = "o200k_base")]
pub static O200K_BASE: TiktokenPack = TiktokenPack {
    name: "o200k_base",
    compressed: include_bytes!(concat!(env!("OUT_DIR"), "/o200k_base.scud.mz")),
    once: OnceLock::new(),
};

// ---------------------------------------------------------------------
// Tests. Every test that touches a variant is feature-gated so a build
// with only `cl100k_base` still compiles cleanly.
// ---------------------------------------------------------------------

#[cfg(all(test, feature = "cl100k_base"))]
mod cl100k_tests {
    use super::CL100K_BASE;
    use stringcheese_tokenizer::Tokenizer;

    #[test]
    fn pack_loads() {
        let _tok = CL100K_BASE.get();
    }

    #[test]
    fn empty_input_encodes_to_zero_tokens() {
        let tok = CL100K_BASE.get();
        let enc = tok.encode("").unwrap();
        assert!(enc.is_empty());
        assert_eq!(tok.count("").unwrap(), 0);
    }

    #[test]
    fn hello_world_round_trip() {
        let tok = CL100K_BASE.get();
        // Note: the synthetic in-tree stand-in does NOT produce
        // tiktoken-identical ids. What it does guarantee is that
        // decode(encode(text)) == text for text that lies inside the
        // built-in whitespace pre-tokenizer's "no non-ASCII, no
        // dropped whitespace boundary" input class.
        let text = "Hello,world!";
        let enc = tok.encode(text).unwrap();
        let round = tok.decode(&enc.ids).unwrap();
        assert_eq!(round, text);
        assert!(!enc.is_empty());
    }

    #[test]
    fn count_and_encode_agree() {
        let tok = CL100K_BASE.get();
        for text in ["a", "hello", "world", "hi there", "cat"] {
            let n = tok.count(text).unwrap();
            let enc = tok.encode(text).unwrap();
            assert_eq!(n, enc.len(), "count/encode mismatch on {text:?}");
        }
    }

    #[test]
    fn utf8_round_trip_when_no_whitespace() {
        // Whitespace is dropped by the default whitespace pre-tokenizer,
        // so pick inputs without any.
        let tok = CL100K_BASE.get();
        for text in ["hello", "caf\u{e9}", "\u{4e2d}\u{6587}"] {
            let enc = tok.encode(text).unwrap();
            let round = tok.decode(&enc.ids).unwrap();
            assert_eq!(round, text);
        }
    }

    #[test]
    fn debug_reports_load_state() {
        // Take a fresh view: the pack is already loaded by earlier
        // tests in this module, but the debug fmt still prints its
        // name and compressed size.
        let s = format!("{CL100K_BASE:?}");
        assert!(s.contains("cl100k_base"));
    }
}

#[cfg(all(test, feature = "p50k_base"))]
mod p50k_tests {
    use super::P50K_BASE;
    use stringcheese_tokenizer::Tokenizer;

    #[test]
    fn pack_loads_and_round_trips() {
        let tok = P50K_BASE.get();
        let text = "hello";
        let round = tok.decode(&tok.encode(text).unwrap().ids).unwrap();
        assert_eq!(round, text);
    }
}

#[cfg(all(test, feature = "r50k_base"))]
mod r50k_tests {
    use super::R50K_BASE;
    use stringcheese_tokenizer::Tokenizer;

    #[test]
    fn pack_loads_and_round_trips() {
        let tok = R50K_BASE.get();
        let text = "hello";
        let round = tok.decode(&tok.encode(text).unwrap().ids).unwrap();
        assert_eq!(round, text);
    }
}

#[cfg(all(test, feature = "o200k_base"))]
mod o200k_tests {
    use super::O200K_BASE;
    use stringcheese_tokenizer::Tokenizer;

    #[test]
    fn pack_loads_and_round_trips() {
        let tok = O200K_BASE.get();
        let text = "hello";
        let round = tok.decode(&tok.encode(text).unwrap().ids).unwrap();
        assert_eq!(round, text);
    }
}

// ---------------------------------------------------------------------
// Property tests. Guarded off wasm because `proptest`'s transitive
// `wait-timeout` dep is `#[cfg(unix)]` / `#[cfg(windows)]` only. This
// matches the guard convention every other crate in the workspace
// uses.
// ---------------------------------------------------------------------

#[cfg(all(test, feature = "cl100k_base", not(target_family = "wasm")))]
mod properties {
    use super::CL100K_BASE;
    use proptest::prelude::*;
    use stringcheese_tokenizer::Tokenizer;

    fn arb_no_whitespace_ascii() -> impl Strategy<Value = String> {
        prop::collection::vec(any::<u8>().prop_map(|b| (b % 26) + b'a'), 0..40)
            .prop_map(|v| String::from_utf8(v).unwrap())
    }

    proptest! {
        #[test]
        fn round_trip_preserves_input(text in arb_no_whitespace_ascii()) {
            let tok = CL100K_BASE.get();
            let enc = tok.encode(&text).unwrap();
            let round = tok.decode(&enc.ids).unwrap();
            prop_assert_eq!(round, text);
        }
    }

    proptest! {
        #[test]
        fn count_matches_encode_length(text in arb_no_whitespace_ascii()) {
            let tok = CL100K_BASE.get();
            let n = tok.count(&text).unwrap();
            let enc = tok.encode(&text).unwrap();
            prop_assert_eq!(n, enc.len());
        }
    }

    proptest! {
        #[test]
        fn empty_input_produces_empty_encoding(_ in 0u32..1) {
            let tok = CL100K_BASE.get();
            let enc = tok.encode("").unwrap();
            prop_assert!(enc.is_empty());
        }
    }
}
