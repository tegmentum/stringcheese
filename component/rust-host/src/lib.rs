//! WebAssembly Component Model host for StringCheese.
//!
//! This crate implements the `stringcheese:core` WIT interface (see
//! `../wit/stringcheese.wit`) by delegating to the underlying algorithm
//! crates in the main StringCheese workspace. When built with
//! `cargo component build --release` the output is a valid WebAssembly
//! component that any Component-Model-capable host (Wasmtime, jco,
//! WasmCloud, …) can invoke without linking Rust.
//!
//! # What this file is
//!
//! The `wit_bindgen::generate!` invocation below reads the WIT file and
//! emits, at macro-expansion time, one Rust trait per WIT interface
//! (each named `Guest`, nested under
//! `exports::stringcheese::core::<interface>`). The `Component` unit
//! struct then `impl`s each of those traits, translating between the
//! wire types the generated code produces (`Vec<u8>`, `String`, `u32`,
//! `f64`, the generated `BoundedDistance` variant) and the
//! Rust-native types the algorithm crates work in (`Distance<u32>`,
//! `NormalizedSimilarity`, `Match`, `DoubleMetaphoneKey`, …). The
//! trailing `export!(Component)` macro is what wires the trait impls
//! into the component's export table.
//!
//! # Adding a function
//!
//! 1. Add the `func` declaration to `../wit/stringcheese.wit`.
//! 2. Rebuild — the `wit_bindgen::generate!` macro will emit a new
//!    trait method and the compiler will error until you implement it.
//! 3. Add the corresponding method to the `impl` block below, calling
//!    into the appropriate algorithm crate.
//! 4. Document the new function in `../README.md`.
//!
//! See `../README.md` for the full build and consumption workflow.

// `#![forbid(unsafe_code)]` is DELIBERATELY NOT set on this crate. The
// wit-bindgen macro expansion below emits low-level unsafe code that
// bridges the Component Model ABI — pointer bookkeeping, `realloc`
// wiring, raw list lowering. That unsafe is audited inside the
// wit-bindgen crate itself and cannot be replaced with safe Rust
// without abandoning the Component Model. The rest of this crate is
// safe Rust; the only `unsafe` reachable from a `cargo expand` here
// lives in wit-bindgen-generated symbols with `__` prefixes.
#![deny(unsafe_op_in_unsafe_fn)]
// The generated bindings emit a few patterns clippy pedantic finds
// noisy (`must_use_candidate` on Guest trait methods, camel-case
// enum arms lowered from kebab-case WIT). Allow them at the crate
// level so `cargo clippy -- -W clippy::pedantic` stays green on the
// hand-written code without wrestling the generated code.
#![allow(clippy::must_use_candidate)]
#![allow(clippy::pedantic)]

// Generate the interface trait definitions from the WIT file at build
// time. The macro emits (roughly):
//
//   pub mod exports {
//       pub mod stringcheese {
//           pub mod core {
//               pub mod distance   { pub trait Guest { ... }; pub enum BoundedDistance { ... } }
//               pub mod similarity { pub trait Guest { ... } }
//               pub mod search     { pub trait Guest { ... } }
//               pub mod phonetic   { pub trait Guest { ... } }
//           }
//       }
//   }
//
// plus an `export!` macro that installs a Component-implementing type
// into the component's export table.
wit_bindgen::generate!({
    world: "stringcheese-core",
    path: "../wit",
});

/// The unit struct every Guest trait is implemented on. Zero-sized;
/// state that a real long-lived component would carry (workspace pools,
/// automaton caches, …) lives inside the algorithm crates or is
/// re-created per call in this seed implementation.
struct Component;

// -------------------------------------------------------------------------
// distance interface
// -------------------------------------------------------------------------
//
// Every entry point rebuilds the required workspace on each call — this
// keeps the implementation simple and matches the trait-based one-shot
// APIs in the underlying crates. Callers that need batch throughput are
// better served by a future `resource batch-workspace { … }` in the WIT
// interface (see README "Path forward") than by a hidden global here.

impl exports::stringcheese::core::distance::Guest for Component {
    fn levenshtein(a: Vec<u8>, b: Vec<u8>) -> u32 {
        use stringcheese_core::DistanceMetric;
        stringcheese_compare::levenshtein::Levenshtein
            .distance(&a[..], &b[..])
            .into_inner()
    }

    fn levenshtein_within(
        a: Vec<u8>,
        b: Vec<u8>,
        cutoff: u32,
    ) -> exports::stringcheese::core::distance::BoundedDistance {
        use stringcheese_core::{BoundedDistance as RustBounded, BoundedDistanceMetric};
        use exports::stringcheese::core::distance::BoundedDistance as WitBounded;
        match stringcheese_compare::levenshtein::Levenshtein.distance_within(&a[..], &b[..], cutoff) {
            RustBounded::Within(d) => WitBounded::Within(d.into_inner()),
            RustBounded::Exceeded { cutoff } => WitBounded::Exceeded(cutoff),
        }
    }

    fn hamming(a: Vec<u8>, b: Vec<u8>) -> Result<u32, String> {
        // The Hamming trait impl panics on length mismatch; use the
        // fallible entry point and flatten the typed error to `string`
        // for the WIT boundary.
        stringcheese_compare::hamming::Hamming
            .try_distance(&a[..], &b[..])
            .map(|d| d.into_inner())
            .map_err(|e| {
                format!(
                    "Hamming: length mismatch (left = {}, right = {})",
                    e.left, e.right
                )
            })
    }

    fn osa(a: Vec<u8>, b: Vec<u8>) -> u32 {
        use stringcheese_core::DistanceMetric;
        stringcheese_compare::damerau::Osa.distance(&a[..], &b[..]).into_inner()
    }

    fn lcs_distance(a: Vec<u8>, b: Vec<u8>) -> u32 {
        // `LcsDistance` exposes an inherent `distance` method in
        // addition to its `DistanceMetric` impl, so the trait does not
        // need to be in scope here — unlike the `levenshtein` and `osa`
        // paths above, which reach through the trait exclusively.
        stringcheese_compare::lcs::LcsDistance
            .distance(&a[..], &b[..])
            .into_inner()
    }
}

// -------------------------------------------------------------------------
// similarity interface
// -------------------------------------------------------------------------

impl exports::stringcheese::core::similarity::Guest for Component {
    fn jaro(a: Vec<u8>, b: Vec<u8>) -> f64 {
        stringcheese_compare::jaro::Jaro
            .similarity_normalized(&a[..], &b[..])
            .into_inner()
    }

    fn jaro_winkler(a: Vec<u8>, b: Vec<u8>) -> f64 {
        stringcheese_compare::jaro::JaroWinkler::classic()
            .similarity_normalized(&a[..], &b[..])
            .into_inner()
    }

    fn dice_bigrams(a: Vec<u8>, b: Vec<u8>) -> f64 {
        let (set_a, set_b) = byte_bigram_sets(&a, &b);
        stringcheese_compare::set_similarity::DiceOverSet
            .similarity_normalized(&set_a, &set_b)
            .into_inner()
    }

    fn jaccard_bigrams(a: Vec<u8>, b: Vec<u8>) -> f64 {
        let (set_a, set_b) = byte_bigram_sets(&a, &b);
        stringcheese_compare::set_similarity::JaccardOverSet
            .similarity_normalized(&set_a, &set_b)
            .into_inner()
    }
}

/// Build the two `GramSet<Vec<u8>>` values a `set-similarity` call
/// needs. Extracted into a helper because both Dice and Jaccard follow
/// the same three-step recipe (build generator → materialise sets on
/// each side) and doing it twice inline would obscure the actual metric
/// call.
///
/// Bigrams (`n = 2`) with no boundary padding are the shape most set-
/// similarity references discuss; a future iteration of the interface
/// can expose `n` and a padding policy as function parameters.
fn byte_bigram_sets(
    a: &[u8],
    b: &[u8],
) -> (
    stringcheese_compare::ngram::GramSet<Vec<u8>>,
    stringcheese_compare::ngram::GramSet<Vec<u8>>,
) {
    use stringcheese_compare::ngram::{CharacterGrams, GramSet, PaddingPolicy};
    // `try_new` cannot fail with n=2, but pattern-matching the
    // Result keeps the panic path explicit rather than hidden behind
    // `.unwrap()`.
    let generator = match CharacterGrams::<u8>::try_new(2, PaddingPolicy::None) {
        Ok(g) => g,
        Err(_) => unreachable!("n = 2 is > 0 by construction"),
    };
    let set_a = GramSet::from_generator(&generator, a);
    let set_b = GramSet::from_generator(&generator, b);
    (set_a, set_b)
}

// -------------------------------------------------------------------------
// search interface
// -------------------------------------------------------------------------
//
// Both find-first and find-all use KMP as the backing algorithm.
// KMP has cheap preparation (O(|pattern|)) and predictable worst-case
// search time (O(|haystack|)) — a sensible default for a one-shot API
// where the caller has not signalled any preference.

impl exports::stringcheese::core::search::Guest for Component {
    fn find_first(pattern: Vec<u8>, haystack: Vec<u8>) -> Option<u32> {
        use stringcheese_compare::search::{Kmp, SearchAlgorithm, SinglePatternSearch};
        let prep = Kmp::prepare(&pattern);
        Kmp::find(&prep, &haystack).map(|m| {
            // WIT `u32` matches most Wasm-land 32-bit indexing; the
            // conversion from Rust `usize` is lossy above 4 GiB, which
            // is a limit no Wasm haystack ever crosses. Saturate as a
            // defensive default.
            u32::try_from(m.position).unwrap_or(u32::MAX)
        })
    }

    fn find_all(pattern: Vec<u8>, haystack: Vec<u8>) -> Vec<u32> {
        use stringcheese_compare::search::{Kmp, SearchAlgorithm, SinglePatternSearch};
        let prep = Kmp::prepare(&pattern);
        Kmp::find_all(&prep, &haystack)
            .into_iter()
            .map(|m| u32::try_from(m.position).unwrap_or(u32::MAX))
            .collect()
    }
}

// -------------------------------------------------------------------------
// phonetic interface
// -------------------------------------------------------------------------
//
// Every encoder takes `&str` on the Rust side. The wit-bindgen macro
// hands the guest a `String` — pass it through as `&str` with a plain
// borrow, no re-encoding.

impl exports::stringcheese::core::phonetic::Guest for Component {
    fn soundex(name: String) -> String {
        stringcheese_phonetic::Soundex::encode(&name)
    }

    fn nysiis(name: String) -> String {
        stringcheese_phonetic::Nysiis::encode(&name)
    }

    fn double_metaphone_primary(name: String) -> String {
        stringcheese_phonetic::DoubleMetaphone::primary_only()
            .encode(&name)
            .primary
    }
}

// Register `Component` as the export table's implementer of every
// interface in the `stringcheese-core` world. This must appear exactly
// once, and after all Guest impls, or the resulting `.wasm` will fail
// its component-model validation.
export!(Component);
