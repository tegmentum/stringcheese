//! Portuguese language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Portuguese`] value that carries the Portuguese
//! stopword list, the [`PortugueseSnowball`] stemmer, the
//! whitespace-and-punctuation [`PortugueseTokenizer`], and a
//! [`PortuguesePhonex`] phonetic hookup. Callers grab the singleton
//! [`PORTUGUESE`] `const` — no construction ceremony required — and
//! delegate through the [`Language`](stringcheese_lang::Language) trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-pt` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Portuguese stopword table
//! or the Snowball stemmer's code. Callers who need Portuguese add
//! `stringcheese-pt = "0.1"` to their `Cargo.toml` explicitly.
//!
//! The implementation choices are deliberately opinionated:
//!
//! * **Snowball Portuguese stemmer.** Martin Porter's own Portuguese
//!   algorithm, documented at
//!   <https://snowballstem.org/algorithms/portuguese/stemmer.html>. The
//!   reference for Portuguese IR stemmers; Lucene's
//!   `PortugueseAnalyzer` and Elasticsearch's `portuguese` analyzer
//!   both compose it. Includes the prelude/postlude `ã`/`õ` placeholder
//!   mechanism (`ã → a~`, `õ → o~`) so verb-suffix rules like `-ão`
//!   do not chew into the nasal base of words like `cão`.
//! * **~200-word stopword list.** Drawn from the Snowball project's
//!   `portuguese/stop.txt` — the intersection of the ranked
//!   most-frequent forms with the full paradigms of `ser` / `estar` /
//!   `haver` / `ter`.
//! * **PHONEX-Portuguese phonetic encoder.** A Soundex-shaped
//!   4-character encoder with Portuguese-tuned preprocessing
//!   (`ç → S`, `lh → L`, `nh → N`, `ch → X`, `qu → K`, `ão` collapse,
//!   accent fold, silent `H`) and a Portuguese-tuned classification
//!   table. See [`phonetic`] for the algorithm.
//! * **Simple tokenizer.** Portuguese, like Spanish, is whitespace-and-
//!   punctuation delimited and requires no elision-splitting pass —
//!   [`PortugueseTokenizer`] is a transparent wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **European Portuguese as default.** The Snowball algorithm is
//!   neutral between Peninsular (pt-PT) and Brazilian (pt-BR)
//!   Portuguese — orthographic reforms have converged the two
//!   varieties enough that a single stemmer serves both. The BCP-47
//!   code registered is the base `"pt"`; see the deferred section for
//!   pt-BR / pt-PT specialization.
//! * **Default Unicode collation.** Modern Portuguese collation
//!   follows the standard Unicode CLDR Portuguese tailoring. This
//!   pack does not carry the CLDR tailoring data;
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need Portuguese collation should
//!   reach for `icu_collator` (via a
//!   [`stringcheese_lang::Collator`] impl of their own).
//!
//! # Deferred to a follow-up wave
//!
//! * **pt-BR / pt-PT specialization.** Regional-variant stopword
//!   extensions, Brazilian-specific verb paradigms (voseo-adjacent
//!   `você`-style conjugations already dominate the shipped
//!   paradigm), spelling-reform (Acordo Ortográfico) differences on
//!   `-ção` / `-cção` and `-cção` / `-tção` alternations.
//! * **Full-corpus cross-verification.** The Snowball project
//!   distributes `voc.txt` / `output.txt` reference files with tens
//!   of thousands of pairs; the
//!   [`tests/snowball_reference.rs`](../../tests/snowball_reference.rs)
//!   test embeds a *subset* that exercises every step's happy path
//!   and each cascading rule. Full-corpus cross-verification is a
//!   follow-up.
//! * **Métaphone Português.** A parallel encoder with a
//!   variable-length key; better for record-linkage precision, but
//!   heavier to reference-test and out of scope for the initial drop.
//! * **CLDR-tailored Portuguese collator.** Depends on an ICU-backed
//!   data table this pack deliberately does not ship.
//! * **Verb-conjugation lemmatization.** Reducing `fui → ir`,
//!   `sou → ser`, `tenho → ter` requires a lexicon; the shipped
//!   stemmer is a suffix-stripper only.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_pt::PORTUGUESE;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(PORTUGUESE.code(), "pt");
//! assert_eq!(PORTUGUESE.name(), "Portuguese");
//! assert!(PORTUGUESE.is_stopword("o"));
//! assert!(PORTUGUESE.is_stopword("e"));
//! assert!(!PORTUGUESE.is_stopword("queijo"));
//! assert_eq!(PORTUGUESE.stem("falando"), "fal");
//! assert_eq!(PORTUGUESE.stem("meninos"), "menin");
//!
//! let toks: Vec<&str> = PORTUGUESE
//!     .tokenize("Como está você? Bem, obrigado.")
//!     .collect();
//! assert_eq!(toks, ["Como", "está", "você", "Bem", "obrigado"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`PortugueseSnowball`] stemmer.
//! - [`phonetic`] — [`PortuguesePhonex`] plus the
//!   [`PortuguesePhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`PortugueseTokenizer`] wrapper.
//! - The [`Portuguese`] type and the [`PORTUGUESE`] constant live in
//!   this crate's root.

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
pub use phonetic::{PortuguesePhonex, PortuguesePhonexAdapter};
#[cfg(feature = "alloc")]
pub use snowball::PortugueseSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::PortugueseTokenizer;

// -----------------------------------------------------------------------
// The Portuguese language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::PortuguesePhonexAdapter;
    use crate::snowball::PortugueseSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::PortugueseTokenizer;

    /// The Portuguese language pack.
    ///
    /// Zero-sized; construct as [`Portuguese`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`PORTUGUESE`](crate::PORTUGUESE) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Portuguese;

    /// The static [`PortuguesePhonexAdapter`] [`Portuguese`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: PortuguesePhonexAdapter = PortuguesePhonexAdapter;

    impl Language for Portuguese {
        fn code(&self) -> &'static str {
            "pt"
        }

        fn name(&self) -> &'static str {
            "Portuguese"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            PortugueseSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(PortugueseTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Portuguese`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Portuguese`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other `stringcheese-<lang>`
    /// pack follows.
    pub const PORTUGUESE: Portuguese = Portuguese;
}

#[cfg(feature = "alloc")]
pub use pack::{PORTUGUESE, Portuguese};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("pt")`) find Portuguese
// without naming the crate. See `stringcheese_lang::registry` for the
// design and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(PORTUGUESE);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-pt` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
