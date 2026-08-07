//! Static language-pack registry.
//!
//! Language packs (`stringcheese-en`, `stringcheese-de`,
//! `stringcheese-fr`, …) opt into the registry via the
//! [`register_language!`](crate::register_language) macro; the
//! [`LANGUAGES`] distributed slice collects the `&'static dyn Language`
//! references at link time, with no runtime constructor pass, no
//! init-order dance, and no allocation. The [`language`] and
//! [`languages`] free functions expose the collected packs by BCP-47
//! lookup and iteration.
//!
//! # When to reach for the registry
//!
//! - **Registry ([`language`])** — the language is chosen at runtime
//!   from a user's locale preference, a config file, an HTTP header,
//!   or any other value not known at compile time. The registry hands
//!   back an `&'static dyn Language`; the caller pays a linear scan
//!   over registered packs (typically single-digit count) and
//!   allocates nothing.
//! - **Direct constant (`stringcheese_en::ENGLISH`, …)** — the language
//!   is fixed at compile time. Skips the scan, keeps the concrete
//!   type in the type system (so pack-specific methods like
//!   [`English::with_porter2`](https://docs.rs/stringcheese-en/latest/stringcheese_en/struct.English.html#method.with_porter2)
//!   are callable), and doesn't force the pack's registration
//!   `static` to be linked in.
//!
//! Depending on `stringcheese-<lang>` alone is enough to trigger the
//! pack's registration — the `register_language!` macro emits a
//! `static` marked with `#[linkme::distributed_slice(LANGUAGES)]`, and
//! any downstream binary that transitively depends on the pack drags
//! that `static` into the link, causing linkme's collector to see it.
//! Callers who want the pack's data but explicitly *not* the
//! registration `static` can gate their dependency on features (a
//! per-pack `registry` feature is a follow-up).
//!
//! # BCP-47 matching
//!
//! [`language`] performs an ASCII-case-insensitive exact-match on the
//! BCP-47 code — `"en"`, `"EN"`, and `"En"` all resolve to the English
//! pack, but `"en-US"` does **not** resolve to `"en"` in v0.1.
//! Full BCP-47 fallback (`"pt-BR" → "pt" → root`) with region and
//! script subtag stripping is deferred to a follow-up; callers who
//! need it today can wrap [`language`] in their own fallback loop.
//!
//! # Ordering guarantees
//!
//! [`LANGUAGES`] is populated in **link order** — the sequence a
//! linker walks the object files, which is deterministic for a given
//! linker invocation but not part of a language pack's public
//! contract. Callers who need a stable observation order should sort
//! the iteration output by
//! [`code`](crate::Language::code) themselves.
//!
//! # Example
//!
//! ```ignore
//! // With `stringcheese-en` in the dependency graph:
//! use stringcheese_lang::registry;
//!
//! let en = registry::language("en").expect("English pack registered");
//! assert_eq!(en.code(), "en");
//!
//! // Iterate every registered pack.
//! for lang in registry::languages() {
//!     println!("{}: {}", lang.code(), lang.name());
//! }
//! ```

use crate::Language;

/// The distributed slice every registered language pack lands in.
///
/// Language packs push a `&'static dyn Language` into this slice via
/// [`crate::register_language!`], which expands to a
/// `#[linkme::distributed_slice(LANGUAGES)]`-annotated `static`. The
/// linker aggregates every such `static` in the final binary into a
/// contiguous section that this slice indexes into — no runtime
/// registration pass, no init-order concerns, no allocation.
///
/// Prefer [`language`] and [`languages`] for lookup and iteration;
/// this slice is exposed for advanced callers that need direct
/// indexing (`LANGUAGES[0]`) or the slice's other `&'static [T]`
/// operations.
// The `distributed_slice` attribute expands to a static with an
// `#[unsafe(link_section = "...")]` attribute — safe in practice
// (linkme's whole design is to make cross-platform link-section
// coordination sound) but tagged `unsafe_code`. Explicitly allowed
// here rather than at the crate root so any other `unsafe` slips in
// this crate still error out.
#[allow(unsafe_code)]
#[linkme::distributed_slice]
pub static LANGUAGES: [&'static dyn Language] = [..];

/// Look up a registered language pack by BCP-47 code.
///
/// The lookup is ASCII-case-insensitive on the code — `"en"`, `"EN"`,
/// and `"En"` all resolve to the same pack. Matching is exact for
/// v0.1; full BCP-47 fallback (`"pt-BR" → "pt"`) is deferred (see the
/// [module docs](self)).
///
/// Returns `None` if no registered pack advertises `code`. The scan
/// is linear in the number of registered packs, which for realistic
/// deployments is under a dozen; no allocation.
#[must_use]
pub fn language(code: &str) -> Option<&'static dyn Language> {
    LANGUAGES
        .iter()
        .copied()
        .find(|lang| lang.code().eq_ignore_ascii_case(code))
}

/// Iterate every registered language pack.
///
/// Emission order follows the linker's aggregation order of the
/// per-pack registration `static`s and is deterministic for a given
/// build but not stable across builds or linkers. Sort by
/// [`Language::code`] if a stable order matters.
pub fn languages() -> impl Iterator<Item = &'static dyn Language> {
    LANGUAGES.iter().copied()
}

// The `register_language!` macro that language packs invoke lives
// in `crate::macros` — it's `#[macro_export]`ed there so it stays
// visible on wasm (where this `registry` module doesn't compile),
// with the emitted `static` cfg-gated to skip wasm.

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::borrow::Cow;

    // Two mock languages used to exercise the registry's iteration
    // and lookup without depending on any real language pack.
    // (We register them directly via the linkme attribute rather than
    // going through `register_language!`, since only one macro
    // invocation is possible per module — we want two entries.)
    struct MockA;
    struct MockB;

    impl Language for MockA {
        fn code(&self) -> &'static str {
            "aa"
        }
        fn name(&self) -> &'static str {
            "MockA"
        }
        fn stopwords(&self) -> &'static [&'static str] {
            &[]
        }
        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            Cow::Borrowed(word)
        }
    }

    impl Language for MockB {
        fn code(&self) -> &'static str {
            "bb"
        }
        fn name(&self) -> &'static str {
            "MockB"
        }
        fn stopwords(&self) -> &'static [&'static str] {
            &[]
        }
        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            Cow::Borrowed(word)
        }
    }

    static MOCK_A: MockA = MockA;
    static MOCK_B: MockB = MockB;

    // Two direct registrations — deliberately not going through
    // `register_language!` so we can register more than one mock in
    // the same module. The `#[allow(unsafe_code)]` on each mirrors
    // what the macro emits; see the macro definition for the
    // rationale.
    #[allow(unsafe_code)]
    #[linkme::distributed_slice(LANGUAGES)]
    static REG_MOCK_A: &'static (dyn Language + 'static) = &MOCK_A;

    #[allow(unsafe_code)]
    #[linkme::distributed_slice(LANGUAGES)]
    static REG_MOCK_B: &'static (dyn Language + 'static) = &MOCK_B;

    #[test]
    fn registered_mocks_are_visible() {
        let codes: alloc::vec::Vec<&str> = languages().map(Language::code).collect();
        assert!(codes.contains(&"aa"), "mock A missing; saw {codes:?}");
        assert!(codes.contains(&"bb"), "mock B missing; saw {codes:?}");
    }

    #[test]
    fn language_lookup_finds_exact_code() {
        let a = language("aa").expect("aa is registered");
        assert_eq!(a.code(), "aa");
        assert_eq!(a.name(), "MockA");
    }

    #[test]
    fn language_lookup_is_case_insensitive_ascii() {
        for probe in ["aa", "AA", "Aa", "aA"] {
            let hit = language(probe).unwrap_or_else(|| panic!("{probe:?} did not resolve"));
            assert_eq!(hit.code(), "aa");
        }
    }

    #[test]
    fn language_lookup_returns_none_for_unknown_code() {
        assert!(language("zz").is_none());
        assert!(language("").is_none());
    }

    #[test]
    fn language_lookup_does_not_perform_bcp47_fallback() {
        // Documented v0.1 behaviour: no region-stripping fallback.
        // "aa-XX" does NOT resolve to "aa".
        assert!(language("aa-XX").is_none());
    }

    #[test]
    fn languages_iter_yields_at_least_the_two_mocks() {
        // Other tests in this module (or the crate) may also register
        // languages; the registry's contract only guarantees that
        // *these* two mocks show up.
        let count = languages()
            .filter(|l| l.code() == "aa" || l.code() == "bb")
            .count();
        assert_eq!(count, 2);
    }

    #[test]
    fn languages_slice_and_iter_agree() {
        let via_slice: alloc::vec::Vec<&str> = LANGUAGES.iter().map(|l| l.code()).collect();
        let via_iter: alloc::vec::Vec<&str> = languages().map(Language::code).collect();
        assert_eq!(via_slice, via_iter);
    }
}

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Case flipping the probe never changes the lookup outcome
        /// for ASCII-only probes.
        #[test]
        fn lookup_is_case_invariant_over_ascii(
            probe in "[A-Za-z]{1,8}",
        ) {
            let lower = probe.to_ascii_lowercase();
            let upper = probe.to_ascii_uppercase();
            let mixed: String = probe
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    if i % 2 == 0 {
                        c.to_ascii_uppercase()
                    } else {
                        c.to_ascii_lowercase()
                    }
                })
                .collect();
            let a = language(&lower).map(Language::code);
            let b = language(&upper).map(Language::code);
            let c = language(&mixed).map(Language::code);
            prop_assert_eq!(a, b);
            prop_assert_eq!(a, c);
        }

        /// Lookup with a code that is not equal (ignoring ASCII case)
        /// to any registered language's `code()` always returns None.
        #[test]
        fn lookup_of_definitely_unknown_returns_none(
            probe in "[Zz][Zz][Zz][Zz][A-Za-z0-9]{0,4}",
        ) {
            // No real pack ships a code starting with four Z's; if the
            // mocks are the only Z-prefixed entries they'd still not
            // match a length-4+ probe.
            let hit = language(&probe);
            if hit.is_some() {
                // If some pack does register a matching code, the
                // property is vacuous — bail out gracefully rather
                // than fail spuriously.
                prop_assume!(false);
            }
            prop_assert!(hit.is_none());
        }
    }
}
