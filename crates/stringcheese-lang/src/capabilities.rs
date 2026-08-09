//! [`LanguageCapabilities`] — the runtime shape a data-driven pack's
//! generated code lands in.
//!
//! # Role
//!
//! The build-time generator [`stringcheese-lang-gen`] reads a per-
//! language `rules/<bcp47>.toml` and emits a `static CAPABILITIES:
//! LanguageCapabilities` value the per-language crate `include!`s
//! into its `src/lib.rs`. That generated value drives the crate's
//! [`Language`](crate::Language) trait impl for every accessor that
//! is data-driven (`code`, `name`, `stopwords`, `script`).
//! Algorithm-driven accessors (`stem`, `tokenize`, `collator`,
//! `phonetic_encoder`) stay hand-written and are wrapped alongside
//! the generated data in the crate's `Language` impl.
//!
//! [`stringcheese-lang-gen`]: https://crates.io/crates/stringcheese-lang-gen
//!
//! # Runtime cost
//!
//! Zero. The struct is a `Copy`-sized bag of `&'static str` and a
//! `&'static [&'static str]` slice; the emitted value is a `static`
//! and lives in read-only data. No allocations, no runtime setup.

/// Static description of a language pack's data-driven surface.
///
/// One value per per-language crate, emitted by
/// [`stringcheese-lang-gen`]'s `generate()` from a `rules/<bcp47>.toml`
/// source. See the module docs for how packs wire this into their
/// [`Language`](crate::Language) trait impls.
///
/// [`stringcheese-lang-gen`]: https://crates.io/crates/stringcheese-lang-gen
#[derive(Copy, Clone, Debug)]
pub struct LanguageCapabilities {
    /// BCP-47 primary language subtag (`"en"`, `"de"`, `"ja"`, …).
    ///
    /// This is the string [`Language::code`](crate::Language::code)
    /// returns.
    pub bcp47: &'static str,

    /// ISO 15924 script code (`"Latn"`, `"Cyrl"`, `"Hans"`, …).
    ///
    /// Not exposed through the [`Language`](crate::Language) trait
    /// today — carried in the capability record so future collator /
    /// segmenter defaults can key off it without another TOML lookup.
    pub script: &'static str,

    /// ICU4X locale identifier for locale-aware operations. Usually
    /// equal to `bcp47`; a pack can point at a more-specific ICU
    /// locale (e.g. `en-GB` for British collation) without changing
    /// its own registered code. Consumed by the optional
    /// `stringcheese-lang-icu` runtime helpers.
    pub icu: &'static str,

    /// Human-readable English name (`"English"`, `"German"`, …).
    ///
    /// Returned by [`Language::name`](crate::Language::name).
    pub name: &'static str,

    /// The pack's stopword list. Returned by
    /// [`Language::stopwords`](crate::Language::stopwords).
    pub stopwords: &'static [&'static str],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_capabilities_is_copy() {
        // Compile-time proof that a generated `static CAPABILITIES`
        // costs zero at every access — the field bag is `Copy`, so
        // every callsite gets a pointer-sized snapshot without any
        // deref-and-clone dance.
        static ENGLISH: LanguageCapabilities = LanguageCapabilities {
            bcp47: "en",
            script: "Latn",
            icu: "en",
            name: "English",
            stopwords: &["the", "and", "of"],
        };
        let copy: LanguageCapabilities = ENGLISH;
        assert_eq!(copy.bcp47, "en");
        assert_eq!(copy.stopwords.len(), 3);
    }
}
