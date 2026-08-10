//! Cache-directory resolution for the tiktoken parity harness.
//!
//! Blob resolution order (matches the crate-level module docs):
//!
//! 1. `$TIKTOKEN_PARITY_DATA_DIR/<variant>.tiktoken` — explicit
//!    override for CI runners that pre-populate the cache from
//!    somewhere the crate cannot reach.
//! 2. `$XDG_CACHE_HOME/stringcheese-tokenizer-tiktoken/<variant>.tiktoken`.
//! 3. `$HOME/.cache/stringcheese-tokenizer-tiktoken/<variant>.tiktoken`
//!    (Linux / macOS default).
//! 4. `%LOCALAPPDATA%\\stringcheese-tokenizer-tiktoken\\<variant>.tiktoken`
//!    (Windows fallback).

use std::env;
use std::path::PathBuf;

/// The environment variable an operator sets to point the harness at a
/// pre-populated cache directory. Overrides every other resolution
/// step.
pub const OVERRIDE_ENV: &str = "TIKTOKEN_PARITY_DATA_DIR";

/// The subdirectory the harness carves out inside whichever cache
/// root ends up chosen. Kept human-readable so a contributor
/// inspecting `~/.cache` can figure out where the bytes came from.
pub const CACHE_SUBDIR: &str = "stringcheese-tokenizer-tiktoken";

/// Return the cache directory the harness should read from and
/// write into for `variant`. The directory is not created here —
/// callers do that lazily when they actually need to write, so
/// read-only test runs against an override cache never touch the
/// filesystem outside `$TIKTOKEN_PARITY_DATA_DIR`.
///
/// # Errors
///
/// Returns an error message if neither the override env var, XDG
/// cache dir, `$HOME`, nor `%LOCALAPPDATA%` can be resolved — a
/// state that only occurs on stripped-down CI runners.
pub fn cache_dir() -> Result<PathBuf, String> {
    if let Some(v) = env::var_os(OVERRIDE_ENV) {
        return Ok(PathBuf::from(v));
    }
    if let Some(v) = env::var_os("XDG_CACHE_HOME") {
        let mut p = PathBuf::from(v);
        p.push(CACHE_SUBDIR);
        return Ok(p);
    }
    if let Some(v) = env::var_os("HOME") {
        let mut p = PathBuf::from(v);
        p.push(".cache");
        p.push(CACHE_SUBDIR);
        return Ok(p);
    }
    if let Some(v) = env::var_os("LOCALAPPDATA") {
        let mut p = PathBuf::from(v);
        p.push(CACHE_SUBDIR);
        return Ok(p);
    }
    Err(String::from(
        "could not resolve a cache directory: set TIKTOKEN_PARITY_DATA_DIR, XDG_CACHE_HOME, or HOME",
    ))
}

/// Convenience: the on-disk path a given variant's plaintext blob
/// resolves to under [`cache_dir`].
///
/// # Errors
///
/// Propagates the failure from [`cache_dir`].
pub fn variant_path(name: &str) -> Result<PathBuf, String> {
    let mut p = cache_dir()?;
    p.push(format!("{name}.tiktoken"));
    Ok(p)
}
