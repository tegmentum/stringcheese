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
//! [`language`] walks the BCP-47 subtag fallback chain from the full
//! code down to the primary language subtag, stopping at the first
//! registered pack that matches. So with only `"pt"` registered:
//!
//! - `"pt"` → `PORTUGUESE` (exact match).
//! - `"pt-BR"` → `PORTUGUESE` (strip `-BR`, retry, match `"pt"`).
//! - `"pt-BR-x-informal"` → `PORTUGUESE` (strip `-informal`, `-x`,
//!   `-BR` in turn until `"pt"` matches).
//! - `"sr-Cyrl-RS"` → `SERBIAN` (strip `-RS`, retry, strip `-Cyrl`,
//!   match `"sr"`).
//! - `"xx-YY"` → `None` (no primary-language match after stripping).
//!
//! Comparisons are ASCII-case-insensitive — `"PT-br"` and `"pt-BR"`
//! resolve to the same pack. Callers that need strict exact-match
//! semantics (no fallback) can reach for [`language_exact`] instead.
//!
//! The fallback is a plain right-to-left subtag strip. It handles the
//! common region/script/variant/private-use (`-x-…`) and extension
//! (`-u-…`) shapes correctly — every subtag boundary is a hyphen, and
//! each strip step just removes the rightmost one — but does not
//! consult the IANA registry, so grandfathered irregular tags
//! (`i-klingon`, `en-GB-oed`) resolve via the same right-to-left walk
//! as everything else. That degrades to the exact match plus its
//! primary-subtag prefix; adequate for the tags callers see in
//! practice.
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
//! // BCP-47 fallback: "en-US" resolves to the "en" pack.
//! let en_us = registry::language("en-US").expect("falls back to en");
//! assert_eq!(en_us.code(), "en");
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

/// Look up a registered language pack by BCP-47 code, walking the
/// subtag fallback chain.
///
/// The lookup tries the full `code` first, and — on miss — strips the
/// rightmost hyphen-delimited subtag and retries, repeating until
/// either a registered pack matches or only the primary language
/// subtag remains and still doesn't match. So `"pt-BR"` falls back to
/// `"pt"`, `"sr-Cyrl-RS"` falls back to `"sr-Cyrl"` and then `"sr"`,
/// and `"pt-BR-x-informal"` walks all the way down to `"pt"`. All
/// comparisons are ASCII-case-insensitive.
///
/// Returns `None` if no subtag in the fallback chain matches a
/// registered pack. The scan is linear in the number of registered
/// packs (typically single-digit); no allocation. Callers that need
/// strict exact-match semantics — no fallback — should use
/// [`language_exact`] instead.
///
/// See the [module docs](self) for the full fallback rules and the
/// handful of edge cases (grandfathered tags, private-use `-x-`
/// subtags, extension `-u-` subtags).
#[must_use]
pub fn language(code: &str) -> Option<&'static dyn Language> {
    let mut remaining = code;
    loop {
        if remaining.is_empty() {
            return None;
        }
        if let Some(hit) = language_exact(remaining) {
            return Some(hit);
        }
        // Strip the rightmost `-<subtag>` and retry. The trimmed
        // prefix is still a well-formed BCP-47 subtag sequence (each
        // strip removes one subtag from the right), so the next
        // iteration's `language_exact` call is comparing apples to
        // apples with registered pack codes. If there is no hyphen
        // left, we've reached the primary language subtag and it
        // didn't match — the `?` returns `None` for the whole `fn`.
        let idx = remaining.rfind('-')?;
        remaining = &remaining[..idx];
    }
}

/// Look up a registered language pack by BCP-47 code, exact match
/// only.
///
/// ASCII-case-insensitive on the code (`"en"`, `"EN"`, and `"En"` all
/// resolve to the same pack), but performs **no** subtag fallback:
/// `"pt-BR"` returns `None` even when a `"pt"` pack is registered.
/// Reach for [`language`] when you want the fallback chain — this is
/// the escape hatch for callers whose semantics require the input
/// code to match a pack's advertised code verbatim.
#[must_use]
pub fn language_exact(code: &str) -> Option<&'static dyn Language> {
    if code.is_empty() {
        return None;
    }
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

    // Mock languages used to exercise the registry's iteration and
    // lookup without depending on any real language pack. We register
    // them directly via the linkme attribute rather than going
    // through `register_language!`, since only one macro invocation is
    // possible per module — we want more than one entry.
    //
    // The codes are chosen to double as BCP-47 fallback fixtures:
    // - `aa`, `bb` — legacy mocks for the pre-existing lookup tests.
    // - `mkpt` — a two-letter primary-language subtag used for
    //   `mkpt-BR`, `mkpt-BR-x-informal` fallback tests. We keep the
    //   probe out of the real ISO-639 range (`mk` is Macedonian; we
    //   use `mkpt` to avoid a collision with any downstream pack that
    //   might grow a real `mk` registration later).
    // - `mksr` — same idea for the `mksr-Latn`, `mksr-Cyrl-RS`
    //   script/region fallback tests.
    struct MockA;
    struct MockB;
    struct MockPt;
    struct MockSr;

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

    impl Language for MockPt {
        fn code(&self) -> &'static str {
            "mkpt"
        }
        fn name(&self) -> &'static str {
            "MockPt"
        }
        fn stopwords(&self) -> &'static [&'static str] {
            &[]
        }
        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            Cow::Borrowed(word)
        }
    }

    impl Language for MockSr {
        fn code(&self) -> &'static str {
            "mksr"
        }
        fn name(&self) -> &'static str {
            "MockSr"
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
    static MOCK_PT: MockPt = MockPt;
    static MOCK_SR: MockSr = MockSr;

    // Direct registrations — deliberately not going through
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

    #[allow(unsafe_code)]
    #[linkme::distributed_slice(LANGUAGES)]
    static REG_MOCK_PT: &'static (dyn Language + 'static) = &MOCK_PT;

    #[allow(unsafe_code)]
    #[linkme::distributed_slice(LANGUAGES)]
    static REG_MOCK_SR: &'static (dyn Language + 'static) = &MOCK_SR;

    #[test]
    fn registered_mocks_are_visible() {
        let codes: alloc::vec::Vec<&str> = languages().map(Language::code).collect();
        assert!(codes.contains(&"aa"), "mock A missing; saw {codes:?}");
        assert!(codes.contains(&"bb"), "mock B missing; saw {codes:?}");
        assert!(codes.contains(&"mkpt"), "mock Pt missing; saw {codes:?}");
        assert!(codes.contains(&"mksr"), "mock Sr missing; saw {codes:?}");
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
    fn language_lookup_walks_region_fallback() {
        // "mkpt-BR" → strip "-BR" → "mkpt" matches.
        let hit = language("mkpt-BR").expect("mkpt-BR falls back to mkpt");
        assert_eq!(hit.code(), "mkpt");
    }

    #[test]
    fn language_lookup_fallback_is_case_insensitive() {
        // Region subtags are usually UPPER-case in wire form; the
        // fallback lookup must still match a lower-case pack code.
        let hit = language("mkpt-br").expect("mkpt-br falls back to mkpt");
        assert_eq!(hit.code(), "mkpt");
        let hit = language("MKPT-BR").expect("MKPT-BR falls back to mkpt");
        assert_eq!(hit.code(), "mkpt");
    }

    #[test]
    fn language_lookup_walks_multi_level_fallback() {
        // `-x-informal` is a private-use extension; each of `-informal`,
        // `-x`, `-BR` is stripped in turn until "mkpt" matches.
        let hit = language("mkpt-BR-x-informal")
            .expect("mkpt-BR-x-informal falls back through multiple subtags");
        assert_eq!(hit.code(), "mkpt");
    }

    #[test]
    fn language_lookup_walks_script_fallback() {
        // Script subtags are conventionally Title-case in wire form.
        let hit = language("mksr-Latn").expect("mksr-Latn falls back to mksr");
        assert_eq!(hit.code(), "mksr");
    }

    #[test]
    fn language_lookup_walks_script_and_region_fallback() {
        // Two-level strip: `-RS` then `-Cyrl`.
        let hit = language("mksr-Cyrl-RS").expect("mksr-Cyrl-RS falls back to mksr");
        assert_eq!(hit.code(), "mksr");
    }

    #[test]
    fn language_lookup_returns_none_when_no_primary_match() {
        // Nothing registered under `xx`; fallback strips `-YY` and
        // still misses on the bare primary language subtag.
        assert!(language("xx-YY").is_none());
        assert!(language("zz-Latn-XX").is_none());
    }

    #[test]
    fn language_lookup_primary_only_still_works() {
        // Regression: the fallback chain must not break the plain
        // exact-match case that existed before subtag walking landed.
        let hit = language("mkpt").expect("mkpt is registered");
        assert_eq!(hit.code(), "mkpt");
    }

    #[test]
    fn language_exact_does_not_fall_back() {
        // The whole point of `language_exact` — it must not resolve a
        // regioned code onto its primary-language pack.
        assert!(
            language_exact("mkpt-BR").is_none(),
            "language_exact must NOT walk the fallback chain"
        );
        assert!(language_exact("mksr-Latn").is_none());
    }

    #[test]
    fn language_exact_finds_exact_matches() {
        let hit = language_exact("mkpt").expect("mkpt is registered");
        assert_eq!(hit.code(), "mkpt");
        // Case-insensitive still applies.
        let hit = language_exact("MKPT").expect("MKPT resolves case-insensitively");
        assert_eq!(hit.code(), "mkpt");
    }

    #[test]
    fn language_exact_rejects_empty() {
        assert!(language_exact("").is_none());
    }

    #[test]
    fn languages_iter_yields_at_least_the_registered_mocks() {
        // Other tests in this module (or the crate) may also register
        // languages; the registry's contract only guarantees that
        // *these* mocks show up.
        let count = languages()
            .filter(|l| matches!(l.code(), "aa" | "bb" | "mkpt" | "mksr"))
            .count();
        assert_eq!(count, 4);
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
        /// to any registered language's `code()` — and whose subtag
        /// prefixes are also not equal — always returns None.
        #[test]
        fn lookup_of_definitely_unknown_returns_none(
            probe in "[Zz][Zz][Zz][Zz][A-Za-z0-9]{0,4}",
        ) {
            // No real pack ships a code starting with four Z's, and
            // the fallback walk strips subtags from the right — so
            // the primary language subtag we ultimately probe still
            // starts with `zzzz`, which no registered pack matches.
            let hit = language(&probe);
            if hit.is_some() {
                // If some pack does register a matching code, the
                // property is vacuous — bail out gracefully rather
                // than fail spuriously.
                prop_assume!(false);
            }
            prop_assert!(hit.is_none());
        }

        /// If `language_exact(code)` resolves to some pack, then the
        /// broader `language(code)` resolves to the same pack — the
        /// fallback chain must not overshoot the exact match.
        #[test]
        fn exact_match_wins_over_fallback(
            probe in "[A-Za-z]{1,8}(-[A-Za-z0-9]{1,8}){0,3}",
        ) {
            if let Some(exact) = language_exact(&probe) {
                let via_fallback = language(&probe)
                    .expect("fallback lookup must at least find the exact match");
                prop_assert_eq!(exact.code(), via_fallback.code());
            }
        }

        /// The fallback lookup on a subtagged probe agrees with a
        /// manual walk that strips the rightmost `-…` segments one by
        /// one — i.e. our implementation and the specified algorithm
        /// produce the same result.
        #[test]
        fn fallback_matches_manual_walk(
            probe in "[A-Za-z]{1,4}(-[A-Za-z0-9]{1,4}){0,4}",
        ) {
            let manual = {
                let mut cur: &str = &probe;
                loop {
                    if cur.is_empty() {
                        break None;
                    }
                    if let Some(hit) = language_exact(cur) {
                        break Some(hit.code());
                    }
                    match cur.rfind('-') {
                        Some(i) => cur = &cur[..i],
                        None => break None,
                    }
                }
            };
            let via_impl = language(&probe).map(Language::code);
            prop_assert_eq!(manual, via_impl);
        }
    }
}
