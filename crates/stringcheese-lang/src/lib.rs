//! Language-pack infrastructure for the StringCheese toolkit.
//!
//! This crate is the umbrella's plug point for per-language behaviour —
//! stemming, stopword lists, tokenization, phonetic encoding, and
//! locale-aware collation. It defines the [`Language`] trait every
//! `stringcheese-<lang>` pack implements, the smaller [`Stemmer`],
//! [`Collator`], and [`LanguagePhoneticEncoder`] plugin traits a pack
//! can compose with, and two ready-made helper types
//! ([`Stopwords`] and [`SimpleTokenizer`]) so a pack that only needs
//! defaults can drop them in without hand-rolling either.
//!
//! # Design commitment
//!
//! Language packs are **opt-in**. This crate carries no built-in
//! language data — no stopword lists, no stemming tables, no
//! tokenizers. Each supported language ships its own crate
//! (`stringcheese-en`, `stringcheese-de`, `stringcheese-fr`, …) that
//! implements [`Language`]. Callers who need English pull in
//! `stringcheese-en` explicitly; callers who don't pay nothing (not a
//! byte of stopword list, not an entry in the code-page tables) at
//! either compile or runtime.
//!
//! The [`Language`] trait is also **data-driven, not
//! algorithm-driven**. The trait describes *what a language rule does*
//! (return the stopwords, stem this word, tokenize this text) rather
//! than *how it does it*. A pack can compose whatever internal
//! machinery it wants — a hand-written Porter stemmer, a delegated
//! Snowball step chain, a lookup-table lemmatizer — behind the same
//! trait surface.
//!
//! # Non-goals
//!
//! - No **silent** language detection. Detection MUST be explicit —
//!   see [`detect::LanguageDetector`] for the pluggable trait; ordinary
//!   string operations never dispatch through a detector on their own.
//!   The trait exists so callers can plug in a backend (whatlang, CLD3,
//!   fastText, a WebAssembly component) without the rest of the toolkit
//!   growing a dependency on any one implementation.
//! - No morphological analysis or POS tagging. The trait's stemming
//!   contract is intentionally weak: return an equivalence-class
//!   representative for the word, nothing more.
//! - No lexica or wordlists. This crate never embeds vocabulary; the
//!   only text it ships is the [`Stopwords`] wrapper's iteration over
//!   the caller-supplied slice.
//!
//! # Quick-start
//!
//! Use [`SimpleTokenizer`] to split text into whitespace-and-punctuation
//! delimited tokens without pulling in a language pack:
//!
//! ```
//! use stringcheese_lang::SimpleTokenizer;
//!
//! let tokens: Vec<&str> = SimpleTokenizer::new().tokenize("hello, world!").collect();
//! assert_eq!(tokens, ["hello", "world"]);
//! ```
//!
//! Or plug in a language pack — say `stringcheese-en` — and use the
//! full [`Language`] trait surface:
//!
//! ```ignore
//! use stringcheese_en::ENGLISH;
//! use stringcheese_lang::Language;
//!
//! assert!(ENGLISH.is_stopword("the"));
//! assert_eq!(ENGLISH.stem("running"), "run");
//! ```
//!
//! # Runtime language selection
//!
//! When the language isn't known at compile time — a user's locale
//! preference, a config file's `"lang": "de"`, a BCP-47 `Accept-Language`
//! header — reach for [`registry`] instead of naming the pack constant
//! directly:
//!
//! ```ignore
//! use stringcheese_lang::registry;
//!
//! let code = std::env::var("LANG").unwrap_or_else(|_| "en".into());
//! let lang = registry::language(&code)
//!     .expect("a supported language pack must be linked in");
//! let stem = lang.stem("caresses");
//! ```
//!
//! The registry is populated at link time by every language pack in
//! the dependency graph — packs opt in by invoking
//! [`register_language!`] once. Direct construction
//! (`stringcheese_en::ENGLISH.stem(word)`) stays the pleasant path
//! when the language is fixed; the registry is the answer when it
//! isn't. See the [`registry`] module docs for the trade-off.
//!
//! # Module map
//!
//! - [`language`] — the [`Language`] trait itself, plus the
//!   [`LanguageProvider`] discovery trait for callers who want to look
//!   a language up by BCP-47 code at runtime.
//! - [`detect`] — the [`LanguageDetector`] plugin trait and its
//!   [`LanguagePrediction`] return type. Ships no implementation;
//!   downstream adapter crates plug in whatlang / CLD3 / fastText /
//!   a WebAssembly component.
//! - [`registry`] — the static distributed-slice registry
//!   (`language(code)`, `languages()`) every pack opts into via
//!   [`register_language!`]. Populated at link time; no runtime init.
//! - [`stemmer`] — the [`Stemmer`] plugin trait a pack can compose.
//! - [`collator`] — the [`Collator`] plugin trait for locale-aware
//!   sort orders.
//! - [`phonetic`] — the object-safe [`LanguagePhoneticEncoder`]
//!   trait, plus a [`SoundexAdapter`](phonetic::SoundexAdapter)-style
//!   wrapper packs use to expose the phonetic crate's typed encoders
//!   through the trait's normalized `(primary, alternate)` return
//!   type.
//! - [`stopwords`] — the [`Stopwords`] wrapper with a case-insensitive
//!   `contains` check.
//! - [`tokenizer`] — the [`SimpleTokenizer`] whitespace-and-punctuation
//!   default tokenizer.

#![cfg_attr(not(feature = "std"), no_std)]
// `deny` rather than `forbid` because the `registry` module's
// `linkme::distributed_slice` expansion emits `#[unsafe(link_section
// = "...")]` and an `unsafe { ... }` typecheck block on each static
// (see linkme-impl). Those are the only permitted unsafe sites in
// this crate; every one carries an explicit
// `#[allow(unsafe_code)]` right above it, and the rest of the crate
// still fires the lint as an error.
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod capabilities;
pub mod collator;
#[cfg(feature = "alloc")]
pub mod detect;
#[cfg(feature = "alloc")]
pub mod language;
#[cfg(feature = "alloc")]
pub mod phonetic;
// `registry` and the `register_language!` macro rely on `linkme`'s
// `#[distributed_slice]`, whose expansion is only defined for a
// fixed set of target OSes (Linux, macOS, iOS, tvOS, Android,
// Fuchsia, illumos, FreeBSD, OpenBSD, PSP, Windows, UEFI, `none`) —
// wasm is not on that list, and the linkme proc-macro compile-errors
// with "distributed_slice is not implemented for this platform" if
// asked to expand there. Gating the module on
// `not(target_family = "wasm")` keeps the crate building on wasm
// targets (the `alloc + wasm` build still has `Language`, `Stopwords`,
// etc.); downstream callers that want the registry compile natively
// and get the full API. The `register_language!` macro carries the
// same gate on its emitted `static`.
#[cfg(all(feature = "alloc", not(target_family = "wasm")))]
pub mod registry;
#[cfg(feature = "alloc")]
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

pub use capabilities::LanguageCapabilities;
pub use collator::Collator;
#[cfg(feature = "alloc")]
pub use detect::{LanguageDetector, LanguagePrediction};
#[cfg(feature = "alloc")]
pub use language::{Language, LanguageProvider};
#[cfg(feature = "alloc")]
pub use phonetic::LanguagePhoneticEncoder;
#[cfg(feature = "alloc")]
pub use stemmer::Stemmer;
pub use stopwords::Stopwords;
pub use tokenizer::SimpleTokenizer;

/// Implementation-detail re-exports used by this crate's
/// exported macros. Not part of the public API — do not depend on any
/// path under `__private` directly.
#[doc(hidden)]
pub mod __private {
    pub use linkme;
}

/// Register a language pack into the [`registry::LANGUAGES`]
/// distributed slice.
///
/// Expands to a `static` marked with
/// `#[linkme::distributed_slice(LANGUAGES)]` that holds a
/// `&'static dyn Language` coerced from `&$lang`. The linker gathers
/// every such `static` in the final binary into the
/// [`registry::LANGUAGES`] slice; there is no runtime constructor
/// pass and no init-order dependency.
///
/// The macro consults `$crate::__private::linkme` for the linkme
/// re-export, so downstream packs never need `linkme` as a direct
/// dependency of their own.
///
/// Invoke this **at most once per crate** — the emitted `static`
/// uses a fixed name (`__STRINGCHEESE_LANGUAGE_REGISTRATION`) so a
/// second invocation in the same module would be a name collision.
///
/// # Platform
///
/// The macro's emitted `static` is gated on
/// `not(target_family = "wasm")`; on wasm targets the invocation
/// expands to nothing (linkme's `#[distributed_slice]` has no wasm
/// branch, and [`registry`] itself is not compiled there). The
/// invocation is still safe to write unconditionally.
///
/// # Example
///
/// ```ignore
/// // In stringcheese-en/src/lib.rs:
/// stringcheese_lang::register_language!(ENGLISH);
///
/// // Or with an explicit coercion, equivalent:
/// stringcheese_lang::register_language!(&ENGLISH as &dyn Language);
/// ```
///
/// The macro definition lives here at the crate root (rather than
/// inside [`registry`]) so that `#[macro_export]` keeps it visible
/// on wasm, where the registry module is cfg-out. The emitted
/// static carries its own wasm gate, so the macro is a no-op there.
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! register_language {
    ($lang:expr) => {
        // The `#[cfg(not(target_family = "wasm"))]` gate mirrors the
        // one on `stringcheese_lang::registry` itself — linkme's
        // `#[distributed_slice(...)]` expansion is undefined on
        // wasm, and the registry module isn't compiled there. On
        // wasm the macro expands to nothing; downstream packs can
        // still invoke it unconditionally without wasm-specific
        // gating of their own.
        //
        // `#[allow(unsafe_code)]` is emitted with the static because
        // linkme's element expansion contains an `unsafe fn`
        // typecheck helper — legitimate `unsafe` in linkme's own
        // implementation, but flagged by the `unsafe_code` lint at
        // the invocation site. Downstream packs can therefore keep
        // their crate-wide `deny(unsafe_code)` intact and still
        // invoke this macro.
        #[cfg(not(target_family = "wasm"))]
        #[allow(unsafe_code)]
        #[$crate::__private::linkme::distributed_slice($crate::registry::LANGUAGES)]
        #[linkme(crate = $crate::__private::linkme)]
        #[doc(hidden)]
        static __STRINGCHEESE_LANGUAGE_REGISTRATION: &'static (dyn $crate::Language + 'static) =
            &$lang;
    };
}

/// Emit the canonical `tests/registry_integration.rs` skeleton for a
/// `stringcheese-<bcp47>` language pack.
///
/// Every per-language pack ships a `tests/registry_integration.rs`
/// that follows the same shape: it forces the pack's rlib into the
/// test binary's link so the pack's [`register_language!`] `static`
/// isn't stripped, then asserts that
/// [`registry::language`](crate::registry::language) resolves the
/// pack's BCP-47 code (case-insensitively) to a `&'static dyn
/// Language` with the expected `code()` and `name()`, and finally
/// runs a per-pack functional smoke test through the trait object.
///
/// This macro packages that shape as one `macro_rules!` invocation.
/// A pack whose smoke test is a stopword membership check plus a
/// stemmer call collapses its whole `registry_integration.rs` to
/// something like:
///
/// ```ignore
/// #![cfg(not(target_family = "wasm"))]
///
/// stringcheese_lang::pack_registry_smoke_test! {
///     pack: stringcheese_am::AMHARIC,
///     pack_ty: stringcheese_am::Amharic,
///     code: "am",
///     name: "Amharic",
///     smoke: |lang| {
///         assert!(lang.is_stopword("እና"));
///         assert_eq!(lang.stem("ልጅኦች"), "ልጅ");
///     },
/// }
/// ```
///
/// Packs that also want to assert BCP-47 fallback behaviour, the
/// absence of a macrolanguage registration, or any other extra
/// invariant tack those on as regular `#[test] fn`s alongside the
/// macro invocation — the macro does not close the module.
///
/// # Fields
///
/// - `pack: <expr>` — the pack's singleton `const` (e.g.
///   `stringcheese_am::AMHARIC`). Used as the initializer of the
///   private `KEEP` const that anchors the pack's rlib in the test
///   binary's link so linkme's registration `static` isn't stripped.
/// - `pack_ty: <ty>` — the pack's concrete type (e.g.
///   `stringcheese_am::Amharic`). Used as the type of the `KEEP`
///   const. Must be a `Language` implementor.
/// - `code: <str literal>` — the pack's BCP-47 primary language
///   subtag (e.g. `"am"`). The macro looks it up via
///   [`registry::language`](crate::registry::language), asserts
///   `code()` matches, and probes an all-uppercase and an
///   alternating-case variant to prove ASCII-case-insensitive
///   matching.
/// - `name: <str literal>` — the pack's human-readable name (e.g.
///   `"Amharic"`), asserted against `Language::name()`.
/// - `smoke: |<ident>| { <block> }` — a functional smoke closure
///   handed the resolved `&'static dyn Language`. Runs one or more
///   `assert!` / `assert_eq!` calls proving the registered object is
///   a real language pack, not just a shell.
///
/// # Emitted `#[test] fn`s
///
/// Three fixed names (each `registry_integration.rs` is its own test
/// binary, so no cross-file collision):
///
/// - `pack_is_registered` — code + name match the expected values.
/// - `pack_registration_is_case_insensitive` — case-flipped probes
///   still resolve.
/// - `pack_functions_through_registry` — runs the `smoke` closure.
///
/// Plus one private `KEEP` `const` (dead-code-allowed) whose sole
/// purpose is to name the pack's singleton so the linker doesn't
/// strip the object file that carries the pack's `register_language!`
/// static. Same trick the hand-written `registry_integration.rs`
/// files use.
///
/// # Platform
///
/// Every expanded item is wasm-gated with
/// `#[cfg(not(target_family = "wasm"))]` for the same reason
/// [`register_language!`] and [`registry`](crate::registry) are —
/// `linkme::distributed_slice` has no wasm branch. On wasm targets
/// the macro invocation compiles to nothing; callers who want an
/// unconditional wasm-safe posture can additionally scope the whole
/// `registry_integration.rs` under a file-level `#![cfg(not(target_family = "wasm"))]`.
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! pack_registry_smoke_test {
    (
        pack: $pack:path,
        pack_ty: $pack_ty:ty,
        code: $code:literal,
        name: $name:literal,
        smoke: | $lang_binding:ident | $smoke:block $(,)?
    ) => {
        // Force the pack's rlib into the test binary's link. A test
        // that only names `stringcheese_lang` items would leave the
        // pack's registration `static` outside the closure the linker
        // walks, hiding the fact that `register_language!` fired.
        // Naming the pack's singleton constant here keeps its object
        // file (and thus its `#[linkme::distributed_slice(...)]`
        // section) alive.
        #[cfg(not(target_family = "wasm"))]
        #[allow(dead_code)]
        const __STRINGCHEESE_PACK_KEEP: &$pack_ty = &$pack;

        #[cfg(not(target_family = "wasm"))]
        #[test]
        fn pack_is_registered() {
            let lang = $crate::registry::language($code).expect(concat!(
                "pack ",
                $code,
                " must be registered"
            ));
            assert_eq!(lang.code(), $code);
            assert_eq!(lang.name(), $name);
        }

        #[cfg(not(target_family = "wasm"))]
        #[test]
        fn pack_registration_is_case_insensitive() {
            // Compute case variants at runtime rather than baking a
            // fixed `["AM", "Am", "aM"]` array — works for codes of
            // any length (two-char primary subtags, three-char subtags
            // like the `mkpt` test mocks in `registry` itself, and
            // future scoped codes). `String` is in scope through the
            // std prelude — `cargo test` binaries always compile with
            // std even when the crate under test is `no_std`.
            let upper: ::std::string::String = $code.to_ascii_uppercase();
            let mixed: ::std::string::String = $code
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
            for probe in [upper.as_str(), mixed.as_str()] {
                assert!(
                    $crate::registry::language(probe).is_some(),
                    "{:?} did not resolve to the {} pack",
                    probe,
                    $name,
                );
            }
        }

        #[cfg(not(target_family = "wasm"))]
        #[test]
        fn pack_functions_through_registry() {
            let $lang_binding = $crate::registry::language($code).expect(concat!(
                "pack ",
                $code,
                " must be registered"
            ));
            $smoke
        }
    };
}

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-lang` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
