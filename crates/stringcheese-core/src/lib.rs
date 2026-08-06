//! Foundational types and traits for the StringCheese sequence-comparison toolkit.
//!
//! This crate contains only the substrate that every other StringCheese crate
//! builds against. It does not implement any comparison algorithms itself;
//! algorithms live in dedicated crates ([`stringcheese-search`], [`stringcheese-cdc`],
//! and so on) that depend on this one.
//!
//! # What lives here
//!
//! - [`result`] — result types (`Distance<T>`, `Similarity<T>`, `Score<T>`,
//!   `NormalizedDistance`, `NormalizedSimilarity`, `BoundedDistance<T>`) that
//!   preserve semantics rather than collapsing every comparison to a bare
//!   `f64`.
//! - [`metric`] — the [`DistanceMetric`], [`BoundedDistanceMetric`], and
//!   [`SimilarityMetric`] traits, together with the [`MetricProperties`] and
//!   [`MetricClass`] descriptors an algorithm uses to declare its
//!   mathematical guarantees.
//! - [`descriptor`] — [`AlgorithmDescriptor`], [`AlgorithmFamily`], and
//!   [`VariantId`]: a stable identification scheme that pins down which
//!   *specific* variant of an algorithm an implementation follows.
//! - [`sequence`] — the [`IndexableSequence`] trait, deliberately narrow so
//!   that string comparison never silently commits to a representation.
//! - [`workspace`] — the [`Workspace`] trait for allocation-free repeated
//!   comparisons.
//! - [`normalization`] — [`NormalizationPolicy`], the explicit choice of
//!   how a raw distance is mapped into `[0.0, 1.0]`.
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` compatible. The default feature set enables
//! `std` for convenience; disabling default features leaves you with a
//! `no_std` build that still requires `alloc` for the heap-allocating
//! utilities. A pure no-alloc build (types and traits only) is available by
//! disabling both features.
//!
//! [`stringcheese-search`]: https://docs.rs/stringcheese-search
//! [`stringcheese-cdc`]: https://docs.rs/stringcheese-cdc

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

pub mod descriptor;
pub mod metric;
pub mod normalization;
pub mod result;
pub mod sequence;
pub mod workspace;

pub use descriptor::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};
pub use metric::{
    BoundedDistanceMetric, DistanceMetric, MetricClass, MetricProperties, SimilarityMetric,
};
pub use normalization::NormalizationPolicy;
pub use result::{
    BoundedDistance, Distance, NormalizedDistance, NormalizedSimilarity, Score, Similarity,
};
pub use sequence::IndexableSequence;
pub use workspace::Workspace;
