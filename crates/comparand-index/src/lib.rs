//! Index structures for large-scale sequence matching with the Comparand
//! toolkit.
//!
//! # Status
//!
//! Placeholder. This crate will host BK-trees, VP-trees, n-gram inverted
//! indexes, prefix and length filters, `MinHash`, locality-sensitive hashing,
//! and sorted-neighborhood helpers. BK-tree acceptance is gated on the
//! algorithm supplying `MetricProperties::is_metric() == true`.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
