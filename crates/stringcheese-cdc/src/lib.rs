//! Content-defined chunking and rolling-hash fingerprinting for the
//! StringCheese toolkit.
//!
//! This crate contains two cooperating subsystems:
//!
//! * **Rolling-hash fingerprints** — three families, each a small stateful
//!   struct that ingests bytes one at a time and reports a digest over the
//!   most recent `window` bytes: [`RabinFingerprint`], [`PolynomialHash`],
//!   and [`GearHash`]. All three implement the shared [`RollingHash`] trait
//!   so consumers can pick one at instantiation time and continue against a
//!   uniform interface.
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
//! * **Buzhash** — a byte-indexed XOR-and-rotate rolling hash. Not distinct
//!   enough from the polynomial rolling hash for a 0.1 slot; deferred.
//! * **SIMD backends** — every kernel here is scalar. SIMD variants live
//!   under a future `simd` feature flag and must produce byte-identical
//!   output to the scalar variant.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod cdc;
pub mod fingerprint;

#[cfg(test)]
mod golden;

#[cfg(test)]
mod property_tests;

pub use cdc::{ChunkBoundary, FastCdc, FastCdcConfig, FastCdcIter, FastCdcStream};
pub use fingerprint::{GearHash, RollingHash};
#[cfg(feature = "alloc")]
pub use fingerprint::{PolynomialHash, RabinFingerprint};
