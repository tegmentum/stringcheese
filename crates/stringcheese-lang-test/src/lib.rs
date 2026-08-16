//! Integration-test host crate for the `stringcheese-lang` registry.
//!
//! This crate has no runtime surface — the shell exists only so that
//! `tests/registry_integration.rs` can be a Cargo integration-test
//! binary that pulls `stringcheese-lang` together with the shipped
//! `stringcheese-{en,de,fr}` language packs and asserts that each
//! pack's `register_language!` invocation ends up in the linkme
//! `#[distributed_slice]` at link time.
//!
//! The test used to live in `crates/stringcheese-lang/tests/`, but
//! the dev-dependencies it needed on the pack crates formed a
//! dev-dep cycle with those packs' regular dependency on
//! `stringcheese-lang`. Cargo's `--workspace` publish mode topo-sorts
//! the whole graph in one pass and does not tolerate that cycle, so
//! the test was hoisted here into a `publish = false` crate. The
//! packs still depend on `-lang`; nothing depends on this crate;
//! nothing gets published from it. See `docs/publishing.md` for the
//! full story.
