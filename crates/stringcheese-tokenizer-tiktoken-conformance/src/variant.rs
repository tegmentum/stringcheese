//! Metadata for the tiktoken variants covered by the parity harness.
//!
//! Each [`Variant`] carries three pieces of information — the
//! upstream URL from which `OpenAI` publishes the `mergeable_ranks`
//! blob, the SHA-256 hash the blob must match, and the canonical
//! variant name. The URLs and hashes are taken from
//! `openai/tiktoken`'s `tiktoken_ext/openai_public.py`; the harness
//! errors out loudly if `OpenAI` rotates a blob so a stale hash cannot
//! silently mis-match. Contributors who need to unblock a hash drift
//! set `TIKTOKEN_PARITY_ALLOW_UNVERIFIED=1` — that skips the SHA-256
//! check but still requires the blob to parse.

use core::fmt;

/// A tiktoken variant covered by the parity harness.
#[derive(Debug, Clone, Copy)]
pub struct Variant {
    /// Canonical variant name — `"cl100k_base"`, `"o200k_base"`, ....
    pub name: &'static str,
    /// Upstream CDN URL where the plaintext `mergeable_ranks` blob
    /// lives. Taken verbatim from `openai_public.py`.
    pub url: &'static str,
    /// The SHA-256 hash of the plaintext blob at [`Self::url`].
    /// Bit-for-bit identical to the hash `openai/tiktoken` verifies
    /// against in its own downloader.
    pub sha256_hex: &'static str,
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

/// `cl100k_base` — the GPT-3.5 / GPT-4 workhorse. First variant with
/// bit-exact parity in Phase 3's acceptance criteria.
pub const CL100K_BASE: Variant = Variant {
    name: "cl100k_base",
    url: "https://openaipublic.blob.core.windows.net/encodings/cl100k_base.tiktoken",
    sha256_hex: "223921b76ee99bde995b7ff738513eef100fb51d18c93597a113bcffe865b2a7",
};

/// `o200k_base` — the GPT-4o / o1 variant. The design doc calls this
/// out as the immediate follow-on target once `cl100k_base` lands.
pub const O200K_BASE: Variant = Variant {
    name: "o200k_base",
    url: "https://openaipublic.blob.core.windows.net/encodings/o200k_base.tiktoken",
    sha256_hex: "446a9538cb6c348e3516120d7c08b09f57c36495e2acfffe59a5bf8b0cfb1a2d",
};

/// The full set of variants the harness knows how to fetch and
/// verify. Ordered so the test suite reports `cl100k_base` first —
/// it is the workhorse and the one Phase 3's acceptance test uses.
pub const ALL: &[Variant] = &[CL100K_BASE, O200K_BASE];
