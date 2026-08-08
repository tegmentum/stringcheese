//! Norwegian (Bokmål) language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Norwegian`] value that carries the Norwegian stopword
//! list, the [`NorwegianSnowball`] stemmer, the whitespace-and-
//! punctuation [`NorwegianTokenizer`], and a [`NorwegianPhonex`]
//! phonetic hookup. Callers grab the singleton [`NORWEGIAN`] `const` —
//! no construction ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Bokmål-only
//!
//! Norway has two official written standards:
//!
//! * **Bokmål** ("book tongue") — descended from written Danish, the
//!   more common of the two by usage (~85–90% of writers). This pack
//!   targets Bokmål.
//! * **Nynorsk** ("new Norwegian") — synthesized in the 19th century
//!   from western Norwegian dialects, used by ~10–15% of writers and
//!   mandated for a share of state broadcasting and school materials.
//!   Deferred to a separate `stringcheese-nn` sibling crate; Nynorsk
//!   has a different stopword inventory (`ikkje` vs. Bokmål `ikke`,
//!   `eg` vs. `jeg`, `me` vs. `vi`, …) and a distinct — though
//!   related — verb morphology that would over-stem or under-stem if
//!   run through the Bokmål algorithm.
//!
//! The BCP-47 tag registered by this pack is **`"nb"`** — the
//! IANA-registered code for Norwegian Bokmål. The macrolanguage tag
//! `"no"` (which subsumes both `nb` and `nn`) is deliberately not used
//! so that a future `stringcheese-nn` sibling can register `"nn"`
//! cleanly without either pack shadowing the other. Callers who look
//! up `"no"` will get `None`; callers who look up `"nb"` (any casing)
//! get this pack.
//!
//! # Norwegian-specific letters
//!
//! The Norwegian alphabet extends the 26-letter Latin base with three
//! letters at the end: `æ` (`ash`), `ø` (`o-slash`), and `å` (`a-ring`).
//! All three occur in high-frequency vocabulary (`være` "to be", `ønske`
//! "to wish", `får` "gets"). The Snowball vowel set includes all three,
//! and the PHONEX preprocessor folds them for phonetic key collapse
//! (see [`phonetic`]).
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-no` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Norwegian stopword table
//! or the Snowball stemmer's code. Callers who need Norwegian add
//! `stringcheese-no = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately opinionated:
//!
//! * **Snowball Norwegian stemmer.** Martin Porter's Snowball
//!   Norwegian algorithm, documented at
//!   <https://snowballstem.org/algorithms/norwegian/stemmer.html>.
//!   The reference stemmer used across Norwegian IR pipelines;
//!   Lucene's `NorwegianAnalyzer` and Elasticsearch's `norwegian`
//!   analyzer both descend from the same `norwegian.sbl` source.
//!   Three-step cascade: (1) main-suffix (nominal / adjectival / verb
//!   inflections plus the `-erte` / `-ert` → `-er` rewrite and the
//!   valid-s-ending `-s` strip); (2) consonant-pair `-dt` / `-vt`
//!   trailing-`t` strip; (3) derivational-suffix strip (`-leg` /
//!   `-eleg` / `-ig` / `-eig` / `-lig` / `-elig` / `-els` / `-lov` /
//!   `-elov` / `-slov` / `-hetslov`). R1 region adjusted so it never
//!   begins before char 3.
//! * **~160-word stopword list.** Drawn from the Snowball project's
//!   `norwegian/stop.txt` — the intersection of the ranked
//!   most-frequent forms with the full paradigms of the copula `være`
//!   / `har` / `vil` / `kan` / `skal` / `må` / `blir`.
//! * **PHONEX-Norwegian phonetic encoder.** A Soundex-shaped
//!   4-character encoder with Norwegian-tuned preprocessing (`skj →
//!   S`, `sk` before front vowels → `S`, `kj → C`, `k` before front
//!   vowels → `C`, `ch → S`, plus the `å → o` / `æ → e` / `ø → e`
//!   letter folds) and the standard PHONEX classification table. See
//!   [`phonetic`] for the algorithm.
//! * **Simple tokenizer.** Norwegian, like German and Dutch, is
//!   whitespace-and-punctuation delimited and requires no elision-
//!   splitting pass — [`NorwegianTokenizer`] is a transparent wrapper
//!   around [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Norwegian collation places `æ`, `ø`, `å` after
//!   `z` (in that order) — the standard Unicode root does not; callers
//!   who need a locale-tailored collator should reach for
//!   `icu_collator` via a [`stringcheese_lang::Collator`] impl of
//!   their own.
//!
//! # Deferred to a follow-up wave
//!
//! * **Nynorsk (`stringcheese-nn`).** Separate written standard;
//!   distinct stopword inventory, distinct verb morphology.
//! * **Danish (`stringcheese-da`) and Icelandic (`stringcheese-is`).**
//!   The other North Germanic Latin-script languages; Danish shares
//!   `æ ø å` with Bokmål but has its own Snowball stemmer, and
//!   Icelandic has additional letters `þ ð` and a much richer
//!   inflectional morphology that would need its own algorithm.
//! * **Full-corpus cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens
//!   of thousands of pairs; the
//!   [`tests/snowball_reference.rs`](../../tests/snowball_reference.rs)
//!   test embeds a *subset* that exercises every step's happy path
//!   and each cascading rule. Full-corpus cross-verification is a
//!   follow-up.
//! * **Métaphone Norwegian.** A parallel encoder with a variable-
//!   length key; better for record-linkage precision, but heavier to
//!   reference-test and out of scope for the initial drop.
//! * **Compound-noun splitting.** Norwegian, like German and Dutch,
//!   productively compounds nouns (`fotball + lag → fotballag`).
//!   Splitting them needs a compound-noun dictionary and is not part
//!   of the Snowball algorithm.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_no::NORWEGIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(NORWEGIAN.code(), "nb");
//! assert_eq!(NORWEGIAN.name(), "Norwegian Bokmål");
//! assert!(NORWEGIAN.is_stopword("og"));
//! assert!(NORWEGIAN.is_stopword("er"));
//! assert!(!NORWEGIAN.is_stopword("fisk"));
//!
//! let toks: Vec<&str> = NORWEGIAN
//!     .tokenize("Katten sover på matten.")
//!     .collect();
//! assert_eq!(toks, ["Katten", "sover", "på", "matten"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`NorwegianSnowball`] stemmer.
//! - [`phonetic`] — [`NorwegianPhonex`] plus the
//!   [`NorwegianPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`NorwegianTokenizer`] wrapper.
//! - The [`Norwegian`] type and the [`NORWEGIAN`] constant live in this
//!   crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
// `deny` rather than `forbid` because the `stringcheese_lang::
// register_language!` invocation below expands to a `linkme`-backed
// static whose implementation is `unsafe`-tagged (safe in practice
// — that's linkme's whole design — but flagged by the
// `unsafe_code` lint). The macro emits an explicit
// `#[allow(unsafe_code)]` at the sole registration site; the rest
// of this crate is still lint-enforced no-`unsafe`.
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod snowball;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{NorwegianPhonex, NorwegianPhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::NorwegianSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::NorwegianTokenizer;

// -----------------------------------------------------------------------
// The Norwegian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::NorwegianPhonexAdapter;
    use crate::snowball::NorwegianSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::NorwegianTokenizer;

    /// The Norwegian (Bokmål) language pack.
    ///
    /// Zero-sized; construct as [`Norwegian`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`NORWEGIAN`](crate::NORWEGIAN) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Norwegian;

    /// The static [`NorwegianPhonexAdapter`] [`Norwegian`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: NorwegianPhonexAdapter = NorwegianPhonexAdapter;

    impl Language for Norwegian {
        fn code(&self) -> &'static str {
            "nb"
        }

        fn name(&self) -> &'static str {
            "Norwegian Bokmål"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            NorwegianSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(NorwegianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Norwegian`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Norwegian`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const NORWEGIAN: Norwegian = Norwegian;
}

#[cfg(feature = "alloc")]
pub use pack::{NORWEGIAN, Norwegian};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("nb")`) find Norwegian
// Bokmål without naming the crate. See `stringcheese_lang::registry`
// for the design and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(NORWEGIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-no` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
