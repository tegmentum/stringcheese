//! Turkish language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Turkish`] value that carries the Turkish stopword
//! list, the [`TurkishSnowball`] stemmer, the whitespace-and-punctuation
//! [`TurkishTokenizer`], and a [`TurkishPhonex`] phonetic hookup.
//! Callers grab the singleton [`TURKISH`] `const` — no construction
//! ceremony required — and delegate through the
//! [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-tr` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Turkish stopword table
//! or the Snowball Turkish stemmer's code. Callers who need Turkish
//! add `stringcheese-tr = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately opinionated:
//!
//! * **Snowball Turkish stemmer.** The Eryiğit and Adalı (2004)
//!   algorithm, documented at
//!   <https://snowballstem.org/algorithms/turkish/stemmer.html>.
//!   The reference for Turkish IR stemmers; Lucene's `TurkishAnalyzer`
//!   and Elasticsearch's `turkish` analyzer both compose a variant.
//!   Turkish is agglutinative — a single orthographic word can carry
//!   nominal-verb, noun, and derivational suffixes stacked — so the
//!   algorithm strips in three passes with **vowel harmony** enforced
//!   on every candidate. See [`snowball`] for the algorithm's rules.
//! * **~180-word stopword list.** The intersection of NLTK's
//!   `turkish` list and the Snowball project's Turkish stopword
//!   collection. Covers pronouns (with the full case-suffixed forms
//!   as separate entries, since Turkish pronoun morphology is
//!   irregular), demonstratives, interrogatives, conjunctions,
//!   postpositions, and the high-frequency conjugations of *olmak*
//!   / *etmek* / *yapmak*. See [`stopwords`].
//! * **PHONEX-Turkish phonetic encoder.** A light Soundex-shape
//!   4-character encoder with Turkish-tuned preprocessing (dotted /
//!   dotless-`I` handled by the Turkic case-fold pass, then
//!   `ç → c`, `ğ → g`, `ı → i`, `ö → o`, `ş → s`, `ü → u` folds to
//!   ASCII). Chosen small because Turkish orthography is already
//!   phonetic; the win from a full Métaphone variant would be
//!   marginal for the additional complexity. See [`phonetic`].
//! * **Simple tokenizer.** Turkish has no clitic elision like French
//!   nor compound agglutination like German — its orthography is
//!   whitespace-and-punctuation delimited, and the six Turkish
//!   special letters (`ç`, `ğ`, `ı`, `İ`, `ö`, `ş`, `ü`) are all
//!   alphabetic under Unicode's classification. [`TurkishTokenizer`]
//!   is a transparent wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer); the
//!   Snowball stemmer handles suffix stripping at the stem step, not
//!   at the tokenizer.
//! * **Turkic case fold.** Turkish's dotted / dotless-`I` distinction
//!   is orthographically load-bearing: default Unicode folding lowers
//!   `I → i` and `İ → i + combining dot`, both of which are wrong
//!   under the Turkish locale. The pack ships a small [`case_fold`]
//!   helper that gets these two scalars right; the stemmer,
//!   phonetic encoder, and the crate's
//!   [`Language::is_stopword`](stringcheese_lang::Language::is_stopword)
//!   override all consult it. See [`case_fold`] for the rationale on *not*
//!   pulling in the (heavier) `stringcheese-unicode::case_fold_turkic`
//!   dependency for a single-scalar rewrite.
//! * **Default Unicode collation.** Turkish traditionally sorts
//!   `ç` after `c`, `ğ` after `g`, `ı` before `i`, `ö` after `o`,
//!   `ş` after `s`, `ü` after `u` — a locale-specific tailoring
//!   the standard Unicode CLDR Turkish tailoring encodes. This pack
//!   does not carry the CLDR tailoring data;
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need Turkish collation should reach
//!   for `icu_collator` (via a
//!   [`stringcheese_lang::Collator`] impl of their own).
//!
//! # Deferred to a follow-up wave
//!
//! * **Métaphone-shaped variable-length phonetic encoder.** A parallel
//!   variable-length key would be better for record-linkage precision;
//!   heavier to reference-test and out of scope for the initial drop.
//! * **CLDR-tailored Turkish collator.** Depends on ICU data this pack
//!   deliberately does not ship.
//! * **Verb-conjugation lemmatization.** Reducing `gördüm → görmek`,
//!   `verdim → vermek` needs a lexicon; the shipped stemmer is a
//!   suffix-stripper only.
//! * **Consonant-alternation restoration.** After stripping a suffix
//!   like `-ım` from `kitabım`, the residue is `kitab` — the surface
//!   form of the stem before the vowel-initial suffix, not the base
//!   `kitap`. Restoring the base needs a lexicon (many words are not
//!   subject to the alternation); the shipped stemmer returns the
//!   surface residue.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_tr::TURKISH;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(TURKISH.code(), "tr");
//! assert_eq!(TURKISH.name(), "Turkish");
//! assert!(TURKISH.is_stopword("ve"));
//! assert!(TURKISH.is_stopword("İÇİN"));  // Turkic case-fold: İ → i.
//! assert!(!TURKISH.is_stopword("peynir"));
//! assert_eq!(TURKISH.stem("kitaplar"), "kitap");
//! assert_eq!(TURKISH.stem("evden"), "ev");
//!
//! let toks: Vec<&str> = TURKISH
//!     .tokenize("Merhaba, dünya! İstanbul çok güzel.")
//!     .collect();
//! assert_eq!(toks, ["Merhaba", "dünya", "İstanbul", "çok", "güzel"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`TurkishSnowball`] stemmer.
//! - [`phonetic`] — [`TurkishPhonex`] plus the
//!   [`TurkishPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`TurkishTokenizer`] wrapper.
//! - [`case_fold`] — the Turkish-aware lowercasing helper.
//! - The [`Turkish`] type and the [`TURKISH`] constant live in this
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

#[cfg(feature = "case-scud")]
pub mod case_data;
#[cfg(feature = "alloc")]
pub mod case_fold;
#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod snowball;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{TurkishPhonex, TurkishPhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::TurkishSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::TurkishTokenizer;

// -----------------------------------------------------------------------
// The Turkish language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::case_fold::eq_ignore_turkish_case;
    use crate::phonetic::TurkishPhonexAdapter;
    use crate::snowball::TurkishSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::TurkishTokenizer;

    /// The Turkish language pack.
    ///
    /// Zero-sized; construct as [`Turkish`] and reuse the value freely
    /// across threads and calls, or grab the crate-level
    /// [`TURKISH`](crate::TURKISH) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Turkish;

    /// The static [`TurkishPhonexAdapter`] [`Turkish`] hands back from
    /// [`phonetic_encoder`](Language::phonetic_encoder).
    static PHONEX: TurkishPhonexAdapter = TurkishPhonexAdapter;

    impl Language for Turkish {
        fn code(&self) -> &'static str {
            "tr"
        }

        fn name(&self) -> &'static str {
            "Turkish"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Turkish-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`]) to apply the Turkish case
        /// fold — a critical difference for inputs containing the
        /// dotted / dotless-`I` pair: `İSTANBUL` and `istanbul`
        /// match, `Istanbul` and `istanbul` do not (the Latin `I`
        /// folds to dotless `ı`, not to `i`).
        fn is_stopword(&self, word: &str) -> bool {
            STOPWORDS.iter().any(|s| eq_ignore_turkish_case(s, word))
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            TurkishSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(TurkishTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Turkish`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Turkish`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const TURKISH: Turkish = Turkish;
}

#[cfg(feature = "alloc")]
pub use pack::{TURKISH, Turkish};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("tr")`) find Turkish without
// naming the crate. See `stringcheese_lang::registry` for the design
// and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(TURKISH);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-tr` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
