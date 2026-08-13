//! Spanish language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Spanish`] value that carries the Spanish stopword
//! list, the [`SpanishSnowball`] stemmer, the whitespace-and-punctuation
//! [`SpanishTokenizer`], and a [`SpanishPhonex`] phonetic hookup.
//! Callers grab the singleton [`SPANISH`] `const` — no construction
//! ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-es` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Spanish stopword table
//! or the Snowball stemmer's code. Callers who need Spanish add
//! `stringcheese-es = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately opinionated:
//!
//! * **Snowball Spanish stemmer.** Martin Porter's own Spanish
//!   algorithm, documented at
//!   <https://snowballstem.org/algorithms/spanish/stemmer.html>. The
//!   reference for Spanish IR stemmers; Lucene's `SpanishAnalyzer` and
//!   Elasticsearch's `spanish` analyzer both compose it. Includes
//!   Step 0 attached-pronoun stripping (`darme`, `haciéndola`) and the
//!   full 3-region (R1/R2/RV) suffix cascade.
//! * **~200-word stopword list.** The intersection of NLTK's `spanish`,
//!   the Snowball project's `spanish/stop.txt`, and Lucene's Spanish
//!   analyzer. Covers articles, prepositions, personal / demonstrative
//!   / possessive pronouns, coordinating conjunctions, and the
//!   high-frequency conjugations of `ser` / `estar` / `haber`.
//! * **PHONEX-Spanish phonetic encoder.** A Soundex-shaped 4-character
//!   encoder with Spanish-tuned preprocessing (`ñ → N`, `ll → L`,
//!   `qu → K`, `ch → X`, `rr → R`, `z → S`, `v → B`, silent `H`,
//!   accent folding) and a Spanish-tuned classification table. Chosen
//!   over a raw ASCII-folded English Soundex because the Spanish
//!   phonemes are meaningfully different from English (`v`/`b` merger,
//!   `z`/`s` merger via *seseo*, silent `h`, palatal `ñ` and `ll`),
//!   and chosen over a Métaphone-family encoder for reference-test
//!   simplicity — a fixed-width 4-character key is easier to lock in
//!   than a variable-length one. See [`phonetic`] for the algorithm.
//! * **Simple tokenizer.** Unlike French, Spanish has no clitic
//!   elision (`l'`, `d'`, `qu'`) and unlike German, no compound-noun
//!   agglutination — its orthography is whitespace-and-punctuation
//!   delimited. [`SpanishTokenizer`] is a transparent wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer); the
//!   Snowball stemmer handles enclitic-pronoun stripping (`darme`,
//!   `dárselo`) at the stem step, not at the tokenizer.
//! * **Default Unicode collation.** Spanish traditionally sorted
//!   `ch` as a separate letter between `c` and `d`, and `ll` between
//!   `l` and `m`; both were removed as separate glyphs from the DRAE
//!   alphabet in 1994. Modern collation follows the standard Unicode
//!   CLDR Spanish tailoring. This pack does not carry the CLDR
//!   tailoring data;
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need Spanish collation should reach
//!   for `icu_collator` (via a
//!   [`stringcheese_lang::Collator`] impl of their own).
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone Español.** A parallel encoder with a variable-length
//!   key; better for record-linkage precision, but heavier to
//!   reference-test and out of scope for the initial drop.
//! * **Beider-Morse Spanish.** A Sephardic-name-aware phonetic
//!   encoder; requires a substantial rule set out of scope for a
//!   starter pack.
//! * **CLDR-tailored Spanish collator.** Depends on an ICU-backed
//!   data table this pack deliberately does not ship.
//! * **Regional-variant stopword extensions.** No `vos` /
//!   Rioplatense-specific forms; no Peninsular-only spellings.
//! * **Verb-conjugation lemmatization.** Reducing `puse → poner`,
//!   `soy → ser`, `fui → ir` requires a lexicon; the shipped stemmer
//!   is a suffix-stripper only.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_es::SPANISH;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(SPANISH.code(), "es");
//! assert_eq!(SPANISH.name(), "Spanish");
//! assert!(SPANISH.is_stopword("el"));
//! assert!(SPANISH.is_stopword("y"));
//! assert!(!SPANISH.is_stopword("queso"));
//! assert_eq!(SPANISH.stem("hablando"), "habl");
//! assert_eq!(SPANISH.stem("niños"), "niñ");
//!
//! let toks: Vec<&str> = SPANISH
//!     .tokenize("¿Cómo estás? Bien, gracias.")
//!     .collect();
//! assert_eq!(toks, ["Cómo", "estás", "Bien", "gracias"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`SpanishSnowball`] stemmer.
//! - [`phonetic`] — [`SpanishPhonex`] plus the
//!   [`SpanishPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`SpanishTokenizer`] wrapper.
//! - The [`Spanish`] type and the [`SPANISH`] constant live in this
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

#[cfg(feature = "datetime-scud")]
pub mod datetime_data;
#[cfg(feature = "number-scud")]
pub mod number_data;
#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "plural-scud")]
pub mod plural_data;
#[cfg(feature = "alloc")]
pub mod snowball;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{SpanishPhonex, SpanishPhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::SpanishSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::SpanishTokenizer;

// -----------------------------------------------------------------------
// The Spanish language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::SpanishPhonexAdapter;
    use crate::snowball::SpanishSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::SpanishTokenizer;

    /// The Spanish language pack.
    ///
    /// Zero-sized; construct as [`Spanish`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`SPANISH`](crate::SPANISH) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Spanish;

    /// The static [`SpanishPhonexAdapter`] [`Spanish`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: SpanishPhonexAdapter = SpanishPhonexAdapter;

    impl Language for Spanish {
        fn code(&self) -> &'static str {
            "es"
        }

        fn name(&self) -> &'static str {
            "Spanish"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            SpanishSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(SpanishTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Spanish`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Spanish`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const SPANISH: Spanish = Spanish;
}

#[cfg(feature = "alloc")]
pub use pack::{SPANISH, Spanish};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("es")`) find Spanish without
// naming the crate. See `stringcheese_lang::registry` for the design
// and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(SPANISH);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-es` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
