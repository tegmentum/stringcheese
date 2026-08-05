//! Comparand — rigorous sequence comparison for Rust and WebAssembly.
//!
//! This is the top-level facade. It re-exports Comparand's public API from
//! the underlying implementation crates so library consumers need only one
//! dependency and one `use` path.
//!
//! For an overview of the project's design, algorithm coverage, and
//! validation strategy, see the `DESIGN.md` document in the repository.
//!
//! # Status
//!
//! Version 0.1 is under initial development. The current release covers the
//! type-system substrate — result types, metric traits, algorithm-variant
//! descriptors, workspace and sequence abstractions, and the golden-case
//! validation schema. Concrete algorithm implementations arrive in
//! subsequent milestones.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

pub use comparand_core::*;

/// Metadata about this release.
pub mod meta {
    /// The `comparand` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
