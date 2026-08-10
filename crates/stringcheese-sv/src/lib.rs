//! Swedish language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Swedish`] value that carries the Swedish stopword
//! list, the [`SwedishSnowball`] stemmer, the whitespace-and-punctuation
//! [`SwedishTokenizer`], and a [`SwedishPhonex`] phonetic hookup.
//! Callers grab the singleton [`SWEDISH`] `const` — no construction
//! ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-sv` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Swedish stopword table
//! or the Snowball stemmer's code. Callers who need Swedish add
//! `stringcheese-sv = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately opinionated:
//!
//! * **Snowball Swedish stemmer.** Martin Porter's own Swedish
//!   algorithm, documented at
//!   <https://snowballstem.org/algorithms/swedish/stemmer.html>. The
//!   reference for Swedish IR stemmers; Lucene's `SwedishAnalyzer`
//!   and Elasticsearch's `swedish` analyzer both compose it. Includes
//!   the full three-step cascade (main suffix / consonant-pair /
//!   other suffix) with the `et`-condition exclusion list and the
//!   `öst → ös` and `fullt → full` replacements.
//! * **~140-word stopword list.** Drawn from the Snowball project's
//!   `swedish/stop.txt` — the intersection of the ranked most-frequent
//!   forms with the full paradigms of `vara` / `ha` / `bli` / `kunna`
//!   / `skola` / `vilja` / `måste` / `göra`.
//! * **PHONEX-Swedish phonetic encoder.** A Soundex-shaped 4-character
//!   encoder with Swedish-tuned preprocessing (`sj → S`, `sk` before
//!   front vowels → `S`, `stj / skj → S`, `sch → S`, `tj / kj → C`,
//!   `k` before front vowels → `C`, `ch → S`, plus the vowel folds
//!   `å → o`, `ä → e`, `ö → e`) and a Swedish-tuned classification
//!   table (labial `B/P/F/V/W`, velar-palatal `C/K/G/Q/J/X`, dental
//!   `D/T`, sibilant `S/Z`). See [`phonetic`] for the algorithm.
//! * **Simple tokenizer.** Swedish, like German and Dutch, is
//!   whitespace-and-punctuation delimited and requires no
//!   elision-splitting pass — [`SwedishTokenizer`] is a transparent
//!   wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Sverigesvenska ("Sweden Swedish") as default.** The Snowball
//!   algorithm is neutral between Sverigesvenska (sv-SE) and Finland-
//!   Swedish (sv-FI); the shared standard vocabulary of the Snowball
//!   `swedish/stop.txt` distribution serves both varieties. The BCP-47
//!   code registered is the base `"sv"`; regional variants are
//!   deferred.
//! * **Case-insensitive stopword lookup on ASCII.** The default
//!   [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//!   implementation uses [`str::eq_ignore_ascii_case`]. Since Swedish
//!   letters `å ä ö` are outside the ASCII case-fold, a stopword
//!   spelled with an uppercase Swedish letter (`"Är"`) matches its
//!   lowercase entry only if the case-fold does not touch the accented
//!   character (as is the case for the stored lowercase forms `"är"`,
//!   `"där"`, `"över"`, etc. matched byte-for-byte against a lowercase
//!   input). Callers who need Unicode case-folded lookups can override
//!   the trait method or lower-case their input.
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Swedish alphabetical order sorts `å ä ö` at the
//!   end of the alphabet (unlike German where `ä` sorts near `a`);
//!   callers who need a locale-tailored collator should reach for
//!   `icu_collator` via a [`stringcheese_lang::Collator`] impl of
//!   their own.
//!
//! # Deferred to a follow-up wave
//!
//! * **Finland-Swedish (sv-FI) variant.** Regional-variant stopword
//!   extensions and Finland-Swedish pronunciation-specific PHONEX
//!   tuning.
//! * **Full-corpus cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens
//!   of thousands of pairs; the
//!   [`tests/snowball_reference.rs`](../../tests/snowball_reference.rs)
//!   test embeds a *subset* that exercises every step's happy path
//!   and each cascading rule. Full-corpus cross-verification is a
//!   follow-up.
//! * **Métaphone Swedish.** A parallel encoder with a variable-length
//!   key; better for record-linkage precision, but heavier to
//!   reference-test and out of scope for the initial drop.
//! * **Nordic Soundex.** A shared code across Swedish / Norwegian /
//!   Danish; a candidate for a `stringcheese-phonetic` cross-pack
//!   encoder.
//! * **Compound-noun splitting.** Swedish productively compounds nouns
//!   (`glass + bil → glassbil`, `barn + bok → barnbok`).
//!   Splitting them needs a compound-noun dictionary and is not part
//!   of the Snowball algorithm.
//! * **Verb-conjugation lemmatization.** Reducing `är → vara`,
//!   `gick → gå` requires a lexicon; the shipped stemmer is a
//!   suffix-stripper only.
//!
//! # Case-insensitivity on Swedish stems
//!
//! The [`SwedishSnowball`] stemmer lower-cases its input via
//! [`char::to_lowercase`] before applying any rules, so
//! `SwedishSnowball.stem("Flickorna") == "flick"`.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_sv::SWEDISH;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(SWEDISH.code(), "sv");
//! assert_eq!(SWEDISH.name(), "Swedish");
//! assert!(SWEDISH.is_stopword("och"));
//! assert!(SWEDISH.is_stopword("att"));
//! assert!(!SWEDISH.is_stopword("ost"));
//! assert_eq!(SWEDISH.stem("flickorna"), "flick");
//! assert_eq!(SWEDISH.stem("husen"), "hus");
//!
//! let toks: Vec<&str> = SWEDISH
//!     .tokenize("Katten sover på mattan.")
//!     .collect();
//! assert_eq!(toks, ["Katten", "sover", "på", "mattan"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`SwedishSnowball`] stemmer.
//! - [`phonetic`] — [`SwedishPhonex`] plus the [`SwedishPhonexAdapter`]
//!   the [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`SwedishTokenizer`] wrapper.
//! - The [`Swedish`] type and the [`SWEDISH`] constant live in this
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
pub use phonetic::{SwedishPhonex, SwedishPhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::SwedishSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::SwedishTokenizer;

// -----------------------------------------------------------------------
// The Swedish language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::SwedishPhonexAdapter;
    use crate::snowball::SwedishSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::SwedishTokenizer;

    /// The Swedish language pack.
    ///
    /// Zero-sized; construct as [`Swedish`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`SWEDISH`](crate::SWEDISH) constant.
    ///
    /// # Swedish-specific letters
    ///
    /// Swedish carries three letters beyond the base Latin 26: `å`,
    /// `ä`, `ö`. The stemmer treats them as vowels; the tokenizer
    /// treats them as word-internal; the phonetic encoder folds them
    /// to base vowels (`å → o`, `ä → e`, `ö → e`). The stopword list
    /// stores every entry lowercase — Swedish stems on lowercase input
    /// (case-insensitive
    /// [`Language::is_stopword`](Language::is_stopword) via the default
    /// trait implementation's ASCII case-fold).
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Swedish;

    /// The static [`SwedishPhonexAdapter`] [`Swedish`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: SwedishPhonexAdapter = SwedishPhonexAdapter;

    impl Language for Swedish {
        fn code(&self) -> &'static str {
            "sv"
        }

        fn name(&self) -> &'static str {
            "Swedish"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            SwedishSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(SwedishTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Swedish`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Swedish`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const SWEDISH: Swedish = Swedish;
}

#[cfg(feature = "alloc")]
pub use pack::{SWEDISH, Swedish};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("sv")`) find Swedish
// without naming the crate. See `stringcheese_lang::registry` for
// the design and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(SWEDISH);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-sv` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
