//! HTTP fetch + SHA-256 verification for the tiktoken parity harness.
//!
//! Only compiled when the `parity-real-vocab` feature is enabled.
//! Callers use [`load_variant`] as the single entry point: it hits
//! the cache first, then falls through to HTTP if the cache is empty
//! and the operator has not banned the network by setting
//! `TIKTOKEN_PARITY_OFFLINE=1`.
//!
//! # Never commits real bytes
//!
//! The audit constraint (see the crate-level README) is that raw
//! `OpenAI` blobs never enter the repository. The cache resolution
//! order in [`crate::cache`] deliberately picks paths outside the
//! project tree (`~/.cache/…` or `$TIKTOKEN_PARITY_DATA_DIR`) and
//! this module writes only to those paths. The workspace's
//! `.gitignore` already excludes `target/`; the cache lives outside
//! `target/`, and this module never writes anywhere else.

use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::cache;
use crate::variant::Variant;

/// Environment variable a caller sets to `1` to forbid the harness
/// from touching the network. When set the harness only reads from
/// the cache; a cache miss is a hard error.
pub const OFFLINE_ENV: &str = "TIKTOKEN_PARITY_OFFLINE";

/// Environment variable a caller sets to `1` to skip SHA-256
/// verification of the blob. Escape hatch for `OpenAI` hash rotation
/// while a contributor lands a fix; not for use in CI.
pub const ALLOW_UNVERIFIED_ENV: &str = "TIKTOKEN_PARITY_ALLOW_UNVERIFIED";

/// Fetch (or load from cache) the plaintext `mergeable_ranks` blob
/// for `variant`, verify its SHA-256, and return the raw bytes.
///
/// Resolution order:
///
/// 1. If the on-disk cache holds a byte-identical (SHA-matching)
///    copy, return it without any network call.
/// 2. Otherwise, if `TIKTOKEN_PARITY_OFFLINE=1` is set, error out.
/// 3. Otherwise, HTTP `GET` the URL, verify the SHA-256, and write
///    the blob into the cache for next time.
///
/// # Errors
///
/// Returns a descriptive error message on cache-directory
/// resolution failure, network failure, SHA mismatch, or filesystem
/// I/O error.
pub fn load_variant(variant: &Variant) -> Result<Vec<u8>, String> {
    let path = cache::variant_path(variant.name)?;
    if path.exists() {
        let bytes = fs::read(&path)
            .map_err(|e| format!("failed to read cached {}: {e}", path.display()))?;
        if hash_matches(&bytes, variant.sha256_hex) {
            return Ok(bytes);
        }
        // Cached bytes exist but do not match. Fall through to
        // re-fetch so a partial-write or corrupted cache heals
        // itself on the next run.
    }

    if env::var_os(OFFLINE_ENV).is_some() {
        return Err(format!(
            "cache miss for {} at {} and TIKTOKEN_PARITY_OFFLINE=1 forbids network access",
            variant.name,
            path.display()
        ));
    }

    let bytes = http_get(variant.url)?;
    if !hash_matches(&bytes, variant.sha256_hex) {
        return Err(format!(
            "SHA-256 mismatch after downloading {}: expected {}, got {} — either the crate's \
             hash is stale (bump the constant in src/variant.rs) or the CDN served a different \
             blob (set TIKTOKEN_PARITY_ALLOW_UNVERIFIED=1 to force through)",
            variant.name,
            variant.sha256_hex,
            hex_digest(&bytes),
        ));
    }
    write_cache(&path, &bytes)?;
    Ok(bytes)
}

fn write_cache(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create cache dir {}: {e}", parent.display()))?;
    }
    fs::write(path, bytes)
        .map_err(|e| format!("failed to write cache file {}: {e}", path.display()))?;
    Ok(())
}

fn hash_matches(bytes: &[u8], expected_hex: &str) -> bool {
    if env::var_os(ALLOW_UNVERIFIED_ENV).is_some() {
        return true;
    }
    hex_digest(bytes).eq_ignore_ascii_case(expected_hex)
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for byte in out {
        s.push(nibble_hex(byte >> 4));
        s.push(nibble_hex(byte & 0x0f));
    }
    s
}

fn nibble_hex(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + n - 10) as char,
        _ => '?',
    }
}

fn http_get(url: &str) -> Result<Vec<u8>, String> {
    // `ureq` is synchronous — the harness is a test-shape caller, not
    // a hot path. A 60-second timeout is generous for the ~1-5 MB
    // blobs we fetch.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(60))
        .build();
    let resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("HTTP GET {url} failed: {e}"))?;
    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    reader
        .read_to_end(&mut buf)
        .map_err(|e| format!("failed to read response body from {url}: {e}"))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_digest_matches_known_vector() {
        // Empty-input SHA-256 vector — RFC 6234 test vector 1.
        assert_eq!(
            hex_digest(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // "abc" — RFC 6234 test vector 2.
        assert_eq!(
            hex_digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn nibble_hex_covers_all_digits() {
        for i in 0u8..16 {
            let c = nibble_hex(i);
            assert!(c.is_ascii_hexdigit(), "nibble {i} produced non-hex {c}");
        }
    }
}
