//! Build script: locate the real `cl100k_base.tiktoken` plaintext,
//! verify its SHA-256, and stage it under `$OUT_DIR` for embedding.
//!
//! # Resolution order
//!
//! 1. `$STRINGCHEESE_CL100K_TIKTOKEN` — explicit path to a plaintext
//!    blob. CI populates this after downloading from `OpenAI`'s CDN.
//! 2. `$TIKTOKEN_PARITY_DATA_DIR/cl100k_base.tiktoken` — matches the
//!    override the sibling `stringcheese-tokenizer-tiktoken-conformance`
//!    harness reads from.
//! 3. `$XDG_CACHE_HOME/stringcheese-tokenizer-tiktoken/cl100k_base.tiktoken`.
//! 4. `$HOME/.cache/stringcheese-tokenizer-tiktoken/cl100k_base.tiktoken`
//!    (Linux/macOS default; also the path a developer who has already
//!    run the conformance harness ends up with).
//! 5. `%LOCALAPPDATA%\stringcheese-tokenizer-tiktoken\cl100k_base.tiktoken`
//!    (Windows fallback).
//!
//! # Modes
//!
//! * **Default (`parity-real-vocab` off)** — writes an empty
//!   placeholder blob to `$OUT_DIR/cl100k_base.tiktoken` so the
//!   crate's `include_bytes!` still compiles. The runtime library
//!   detects the empty blob and switches its API into stub mode
//!   (every call returns "parity-real-vocab feature required").
//! * **`parity-real-vocab` on** — a missing blob is a hard build
//!   failure with a message naming the env var and cache path. When
//!   the blob is present the SHA-256 is verified against the pinned
//!   constant and the bytes are staged into `$OUT_DIR` for embedding.
//!
//! # Why no HTTP fetch here
//!
//! The sibling `stringcheese-tokenizer-tiktoken-conformance` crate
//! already implements the SHA-256-verified fetch; wiring it in as a
//! build-dep would pull the multi-MB `tiktoken-rs` oracle plus `ureq`
//! into every build of this crate. The chosen contract is: CI (or a
//! developer) runs the conformance harness once to populate the
//! cache, and this crate's `build.rs` reads from that same cache.
//! Same bytes, same SHA-256, one downloader.

#![allow(clippy::doc_markdown)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The SHA-256 of the canonical `cl100k_base.tiktoken` plaintext
/// blob, mirroring
/// `stringcheese-tokenizer-tiktoken-conformance::variant::CL100K_BASE`.
/// A change here without a matching change in the sibling crate is a
/// hash drift the CI catches on the next parity run.
const CL100K_SHA256: &str = "223921b76ee99bde995b7ff738513eef100fb51d18c93597a113bcffe865b2a7";

fn main() {
    // Rebuild triggers. Any change to the input path or the
    // resolution env vars re-runs `build.rs`.
    println!("cargo:rerun-if-env-changed=STRINGCHEESE_CL100K_TIKTOKEN");
    println!("cargo:rerun-if-env-changed=TIKTOKEN_PARITY_DATA_DIR");
    println!("cargo:rerun-if-env-changed=XDG_CACHE_HOME");
    println!("cargo:rerun-if-env-changed=HOME");
    println!("cargo:rerun-if-env-changed=LOCALAPPDATA");
    println!("cargo:rerun-if-changed=build.rs");

    // Declare the cfg we may emit so `--check-cfg` on newer rustc
    // does not warn about an unknown cfg name.
    println!("cargo:rustc-check-cfg=cfg(stringcheese_cl100k_real_vocab)");

    let real_vocab_feature = env::var_os("CARGO_FEATURE_PARITY_REAL_VOCAB").is_some();
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let out_path = out_dir.join("cl100k_base.tiktoken");

    let outcome = stage_plaintext(&out_path);

    match outcome {
        Outcome::Embedded { path, size } => {
            println!("cargo:rerun-if-changed={}", path.display());
            println!("cargo:rustc-cfg=stringcheese_cl100k_real_vocab");
            println!(
                "cargo:warning=stringcheese-tokenizer-component-cl100k: embedded cl100k plaintext from {} ({size} bytes, SHA-256 verified)",
                path.display()
            );
        }
        Outcome::HashMismatch { path, got } => {
            assert!(
                !real_vocab_feature,
                "cl100k plaintext at {path} has SHA-256 {got}, expected {CL100K_SHA256}. \
                 Either the constant in build.rs is stale (bump in lockstep with \
                 stringcheese-tokenizer-tiktoken-conformance/src/variant.rs) or the \
                 cached blob was corrupted - delete it and re-run the parity harness.",
                path = path.display(),
            );
            println!(
                "cargo:warning=stringcheese-tokenizer-component-cl100k: found cl100k blob at {} but SHA-256 mismatch; falling back to stub build",
                path.display()
            );
            write_stub(&out_path);
        }
        Outcome::NotFound => {
            assert!(
                !real_vocab_feature,
                "parity-real-vocab feature enabled but no cl100k_base.tiktoken blob \
                 was found. Populate one of:\n  \
                 * $STRINGCHEESE_CL100K_TIKTOKEN pointing at a plaintext file, or\n  \
                 * $TIKTOKEN_PARITY_DATA_DIR/cl100k_base.tiktoken, or\n  \
                 * ~/.cache/stringcheese-tokenizer-tiktoken/cl100k_base.tiktoken\n\n\
                 The sibling conformance harness populates the last path automatically:\n  \
                 cargo test --manifest-path \
                 crates/stringcheese-tokenizer-tiktoken-conformance/Cargo.toml \
                 --features parity-real-vocab"
            );
            write_stub(&out_path);
        }
    }
}

/// Result of the resolution walk. `Outcome::Embedded` writes to
/// `$OUT_DIR` as a side effect; the other two variants leave the
/// out-path untouched so the caller can decide whether to write a
/// stub or bail.
enum Outcome {
    /// SHA-verified plaintext already written to `$OUT_DIR`.
    Embedded { path: PathBuf, size: usize },
    /// Plaintext found but its SHA-256 did not match the pinned
    /// constant. Only a hard error under `parity-real-vocab`.
    HashMismatch { path: PathBuf, got: String },
    /// No candidate path resolved to an existing readable file.
    NotFound,
}

fn stage_plaintext(out_path: &Path) -> Outcome {
    for candidate in candidate_paths() {
        if !candidate.exists() {
            continue;
        }
        let bytes = match fs::read(&candidate) {
            Ok(b) if !b.is_empty() => b,
            _ => continue,
        };
        let hash = sha256_hex(&bytes);
        if !hash.eq_ignore_ascii_case(CL100K_SHA256) {
            return Outcome::HashMismatch {
                path: candidate,
                got: hash,
            };
        }
        let size = bytes.len();
        fs::write(out_path, &bytes).unwrap_or_else(|e| {
            panic!(
                "failed to stage cl100k plaintext at {}: {e}",
                out_path.display()
            )
        });
        return Outcome::Embedded {
            path: candidate,
            size,
        };
    }
    Outcome::NotFound
}

/// The ordered list of candidate paths. See the module docs for the
/// full resolution story.
fn candidate_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(v) = env::var_os("STRINGCHEESE_CL100K_TIKTOKEN") {
        out.push(PathBuf::from(v));
    }
    if let Some(v) = env::var_os("TIKTOKEN_PARITY_DATA_DIR") {
        out.push(PathBuf::from(v).join("cl100k_base.tiktoken"));
    }
    if let Some(v) = env::var_os("XDG_CACHE_HOME") {
        out.push(
            PathBuf::from(v)
                .join("stringcheese-tokenizer-tiktoken")
                .join("cl100k_base.tiktoken"),
        );
    }
    if let Some(v) = env::var_os("HOME") {
        out.push(
            PathBuf::from(v)
                .join(".cache")
                .join("stringcheese-tokenizer-tiktoken")
                .join("cl100k_base.tiktoken"),
        );
    }
    if let Some(v) = env::var_os("LOCALAPPDATA") {
        out.push(
            PathBuf::from(v)
                .join("stringcheese-tokenizer-tiktoken")
                .join("cl100k_base.tiktoken"),
        );
    }
    out
}

/// Write an empty stub blob so `include_bytes!` compiles. The
/// runtime detects the empty blob and switches to stub mode.
fn write_stub(out_path: &Path) {
    fs::write(out_path, b"").unwrap_or_else(|e| {
        panic!(
            "failed to write stub cl100k blob at {}: {e}",
            out_path.display()
        )
    });
}

/// Hex-encoded SHA-256 of `bytes`. Kept local to `build.rs` so the
/// runtime crate stays free of a `sha2` dep.
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut s = String::with_capacity(digest.len() * 2);
    for byte in digest {
        s.push(hex_nibble(byte >> 4));
        s.push(hex_nibble(byte & 0x0f));
    }
    s
}

fn hex_nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + n - 10) as char,
        _ => '?',
    }
}
