//! Build script: locate T5's `tokenizer.json`, stage it under
//! `$OUT_DIR` for embedding.  Mirrors the resolution walk from the
//! sibling `stringcheese-tokenizer-component-whisper/build.rs`.
//!
//! # Resolution order
//!
//! 1. `$STRINGCHEESE_T5_TOKENIZER_JSON` — explicit path.
//! 2. `$XDG_CACHE_HOME/stringcheese-tokenizer-t5/tokenizer.json`.
//! 3. `$HOME/.cache/stringcheese-tokenizer-t5/tokenizer.json`
//!    (Linux / macOS default).
//! 4. `%LOCALAPPDATA%\stringcheese-tokenizer-t5\tokenizer.json`
//!    (Windows fallback).
//!
//! # Modes
//!
//! * **Default (`parity-real-vocab` off)** — writes an empty
//!   placeholder blob to `$OUT_DIR/tokenizer.json` so the crate's
//!   `include_bytes!` still compiles.  The runtime detects the
//!   empty blob and returns a `TokenizerError::Other` naming the
//!   feature gate on every operation.
//! * **`parity-real-vocab` on** — a missing blob is a hard build
//!   failure with a message naming the env var + cache path.
//!
//! # Why no SHA-256 pin
//!
//! Same reasoning as the whisper sibling — HuggingFace
//! `tokenizer.json` serialization is not byte-stable across
//! `transformers` / `tokenizers` versions.  Pinning a hash here
//! would create a false integrity gate.  The runtime relies on
//! `stringcheese-tokenizer-hf::hf::parse_tokenizer_json` failing
//! loudly on shape drift; a caller who wants a stronger check
//! runs a local parity test against a reference tokenization.

#![allow(clippy::doc_markdown)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=STRINGCHEESE_T5_TOKENIZER_JSON");
    println!("cargo:rerun-if-env-changed=XDG_CACHE_HOME");
    println!("cargo:rerun-if-env-changed=HOME");
    println!("cargo:rerun-if-env-changed=LOCALAPPDATA");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(stringcheese_t5_real_vocab)");

    let real_vocab_feature = env::var_os("CARGO_FEATURE_PARITY_REAL_VOCAB").is_some();
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let out_path = out_dir.join("tokenizer.json");

    if let Some(src) = resolve_source() {
        let bytes = match fs::read(&src) {
            Ok(b) if !b.is_empty() => b,
            _ => {
                assert!(
                    !real_vocab_feature,
                    "parity-real-vocab enabled but T5 tokenizer.json at {} \
                     is empty or unreadable",
                    src.display(),
                );
                write_stub(&out_path);
                return;
            }
        };
        let size = bytes.len();
        fs::write(&out_path, &bytes).unwrap_or_else(|e| {
            panic!(
                "failed to stage T5 tokenizer.json at {}: {e}",
                out_path.display()
            )
        });
        println!("cargo:rerun-if-changed={}", src.display());
        println!("cargo:rustc-cfg=stringcheese_t5_real_vocab");
        println!(
            "cargo:warning=stringcheese-tokenizer-component-t5: embedded T5 tokenizer.json from {} ({size} bytes)",
            src.display()
        );
    } else {
        assert!(
            !real_vocab_feature,
            "parity-real-vocab feature enabled but no T5 tokenizer.json blob \
             was found.  Populate one of:\n  \
             * $STRINGCHEESE_T5_TOKENIZER_JSON pointing at a file, or\n  \
             * ~/.cache/stringcheese-tokenizer-t5/tokenizer.json\n\n\
             Fetch from HF Hub (Xenova mirror ships the ONNX-compatible \
             tokenizer.json):\n  \
             mkdir -p ~/.cache/stringcheese-tokenizer-t5 && \\\n  \
             curl -L -o ~/.cache/stringcheese-tokenizer-t5/tokenizer.json \\\n  \
                 https://huggingface.co/Xenova/t5-small/resolve/main/tokenizer.json"
        );
        write_stub(&out_path);
    }
}

fn resolve_source() -> Option<PathBuf> {
    if let Some(v) = env::var_os("STRINGCHEESE_T5_TOKENIZER_JSON") {
        let p = PathBuf::from(v);
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(v) = env::var_os("XDG_CACHE_HOME") {
        let p = PathBuf::from(v)
            .join("stringcheese-tokenizer-t5")
            .join("tokenizer.json");
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(v) = env::var_os("HOME") {
        let p = PathBuf::from(v)
            .join(".cache")
            .join("stringcheese-tokenizer-t5")
            .join("tokenizer.json");
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(v) = env::var_os("LOCALAPPDATA") {
        let p = PathBuf::from(v)
            .join("stringcheese-tokenizer-t5")
            .join("tokenizer.json");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn write_stub(out_path: &Path) {
    fs::write(out_path, b"").unwrap_or_else(|e| {
        panic!(
            "failed to write stub T5 tokenizer.json at {}: {e}",
            out_path.display()
        )
    });
}
