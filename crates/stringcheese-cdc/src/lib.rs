//! Content-defined chunking and rolling-hash fingerprinting for the
//! StringCheese toolkit.
//!
//! This crate contains two cooperating subsystems:
//!
//! * **Rolling-hash fingerprints** — four families, each a small stateful
//!   struct that ingests bytes one at a time and reports a digest over the
//!   most recent `window` bytes: [`RabinFingerprint`], [`PolynomialHash`],
//!   [`Buzhash`], and [`GearHash`]. All four implement the shared
//!   [`RollingHash`] trait so consumers can pick one at instantiation time
//!   and continue against a uniform interface.
//! * **Content-defined chunking (CDC)** — algorithms that consume a byte
//!   stream and emit boundary offsets whose position depends on the *content*
//!   rather than absolute byte counts. Version 0.1 ships one CDC algorithm,
//!   [`FastCdc`], and reserves the family enum entry for the future Rabin-CDC
//!   sibling.
//!
//! # The rolling-hash primitive is shared
//!
//! Rolling hashes are the underlying primitive for CDC, for Rabin-Karp
//! substring search, and for n-gram hashing. The [design][design] identifies
//! this crate as their canonical home; other StringCheese crates will re-export
//! the trait rather than reimplement it. The [`RollingHash`] trait is
//! deliberately narrow — `new(window)`, `roll(byte)`, `digest()`, `reset()`
//! — so it fits every consumer without forcing them through a CDC-shaped
//! interface.
//!
//! [design]: https://github.com/tegmentum/stringcheese/blob/main/docs/design/ngram-and-fingerprinting.md
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. The default feature set enables
//! `std` for convenience; disabling default features leaves a `no_std` build
//! that still supports every fingerprint and the streaming `FastCDC` iterator.
//! The heap-allocating helpers on [`FastCdc`] (`chunk_boundaries_vec`) are
//! gated behind the `alloc` feature; the iterator-returning
//! `chunk_boundaries` is available without allocation.
//!
//! # Algorithms not shipped in 0.1
//!
//! * **Rabin CDC** — the pre-`FastCDC` generation of content-defined chunking
//!   algorithms. Superseded in practice by `FastCDC` on modern CPUs; deferred
//!   as future work.
//!
//! # SIMD backends (optional, `simd` feature)
//!
//! Each rolling hash carries a sibling `simd` module tree under
//! [`fingerprint::gear::simd`], [`fingerprint::buzhash::simd`],
//! [`fingerprint::polynomial::simd`], and [`fingerprint::rabin::simd`]. Each
//! tree exposes a `digest_of_slice` entry point that consumes a byte slice
//! and returns the same digest a scalar
//! [`RollingHash`]-driven `roll(byte)` sequence
//! would produce for the same input. Runtime dispatch on `x86_64` picks
//! AVX2 → SSE2; `aarch64` uses NEON; wasm32 uses SIMD128 when the
//! `simd128` target-feature is enabled at build time. Byte-identical
//! output against the scalar reference is the contract, anchored by
//! per-backend differential tests over short, chunk-boundary,
//! window-sized, and larger random inputs. See
//! [`fingerprint::gear::simd`] for the shared unsafe policy.

#![cfg_attr(not(feature = "std"), no_std)]
// The crate is `deny(unsafe_code)` rather than `forbid(unsafe_code)` so
// that the optional SIMD backends under `fingerprint::<hash>::simd`
// (only compiled with `--features simd`) can carry a documented
// module-scoped `#[allow(unsafe_code)]`. `deny` is enforced everywhere
// else — every module outside the SIMD sub-trees is expected to be
// safe Rust; the allow attribute must be added deliberately and comes
// with a `reason` explaining the exception.
#![deny(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod cdc;
pub mod fingerprint;

#[cfg(test)]
mod golden;

#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
mod property_tests;

pub use cdc::{ChunkBoundary, FastCdc, FastCdcConfig, FastCdcIter, FastCdcStream};
#[cfg(feature = "alloc")]
pub use fingerprint::{Buzhash, PolynomialHash, RabinFingerprint};
pub use fingerprint::{GearHash, RollingHash};
