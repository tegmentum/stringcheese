//! Icelandic language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Icelandic`] value that carries the Icelandic
//! stopword list, the [`IcelandicStemmer`] rule-based stemmer, the
//! whitespace-and-punctuation [`IcelandicTokenizer`], and an
//! [`IcelandicPhonex`] phonetic hookup. Callers grab the singleton
//! [`ICELANDIC`] `const` — no construction ceremony required — and
//! delegate through the [`Language`](stringcheese_lang::Language)
//! trait.
//!
//! Completes the Nordic quintet alongside `stringcheese-sv`
//! (Swedish), `stringcheese-no` (Norwegian Bokmål), `stringcheese-nn`
//! (Norwegian Nynorsk), and `stringcheese-da` (Danish).
//!
//! # Icelandic-specific letters
//!
//! The Icelandic alphabet extends the 26-letter Latin base with the
//! six long-vowel scalars `á`, `é`, `í`, `ó`, `ú`, `ý`, the two
//! dental-fricative letters `þ` (thorn, /θ/) and `ð` (eth, /ð/), and
//! the two extra vowels `æ` (front-open diphthong) and `ö` (rounded-
//! mid front). All ten occur in high-frequency vocabulary (`þú` "you",
//! `það` "it/that", `góður` "good", `Ísland` "Iceland", `Æsir`
//! "gods", `Björn` "bear/Björn"). The stemmer's suffix table runs on
//! `Vec<char>` (byte-index arithmetic would corrupt multi-byte
//! scalars); the phonetic encoder folds the letters into digraphs
//! (`þ → th`, `ð → dh`, `æ → ae`, `ö → oe`) so the classification
//! table stays ASCII-simple.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-is` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Icelandic stopword
//! table or the rule-based stemmer's code. Callers who need
//! Icelandic add `stringcheese-is = "0.1"` to their `Cargo.toml`
//! explicitly.
//!
//! The implementation choices are deliberately opinionated:
//!
//! * **Rule-based lightweight stemmer.** **Icelandic has no official
//!   Snowball stemmer** — Martin Porter's Snowball project covers the
//!   Nordic siblings Swedish / Norwegian / Danish but not Icelandic.
//!   Icelandic is fusional with rich noun / adjective declension (4
//!   cases × singular / plural × 3 genders) and strong / weak verb
//!   inflection, plus a definite article that agglutinates as a
//!   suffix. This pack ships a rule-based longest-match suffix
//!   stripper (see [`stemmer`]) that removes the most common
//!   inflectional endings — sufficient for IR-style keyword lookup
//!   but not a lemmatizer. Callers who need lemma-quality reduction
//!   should reach for a lexicon-backed pack.
//! * **~90-word stopword list.** The ranked head of Icelandic
//!   function words (articles / demonstratives / personal pronouns /
//!   prepositions / conjunctions / negation and modality adverbs)
//!   plus the full paradigms of the copula `vera` "to be", the
//!   auxiliary `hafa` "have", and the modals `skulu` / `vilja` /
//!   `geta` / `mega`.
//! * **PHONEX-Icelandic phonetic encoder.** A Soundex-shaped
//!   4-character encoder with Icelandic-tuned preprocessing
//!   (`þ → th`, `ð → dh`, `æ → ae`, `ö → oe`, `hv → kv` historical
//!   fold, silent `h`, plus the long-vowel accent folds) and the
//!   standard PHONEX classification table. See [`phonetic`] for the
//!   algorithm.
//! * **Simple tokenizer.** Icelandic, like its North Germanic
//!   siblings Swedish / Norwegian / Danish, is whitespace-and-
//!   punctuation delimited and requires no elision-splitting pass —
//!   [`IcelandicTokenizer`] is a transparent wrapper around
//!   [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Default Unicode collation.**
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Icelandic collation places `þ` after `z` and
//!   `æ`, `ö` in specific slots (the traditional order runs
//!   `... x y z þ æ ö`) — the standard Unicode root does not;
//!   callers who need a locale-tailored collator should reach for
//!   `icu_collator` via a [`stringcheese_lang::Collator`] impl of
//!   their own.
//!
//! # Deferred to a follow-up wave
//!
//! * **Lexicon-backed lemmatization.** Reducing `hafa` and `höfum`
//!   (1pl present, with u-umlaut on the stem vowel) to a single head
//!   form requires knowing the lexeme's paradigm — orthographic
//!   suffix stripping alone can't reverse u-umlaut. This pack emits
//!   `haf` and `höf` respectively.
//! * **Métaphone Icelandic.** A parallel encoder with a variable-
//!   length key; better for record-linkage precision, but heavier to
//!   reference-test and out of scope for the initial drop.
//! * **Compound-noun splitting.** Icelandic, like German / Danish /
//!   Norwegian, productively compounds nouns (`bókasafn = bóka +
//!   safn` "library"). Splitting them needs a compound-noun
//!   dictionary and is not part of the rule-based stemmer.
//! * **Preaspiration / `ll` fortition in the phonetic encoder.**
//!   Modern Icelandic contrasts geminate and preaspirated stops and
//!   fortifies `ll`/`nn` after long vowels; both need vowel-length
//!   knowledge that the spelling-only encoder does not have.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_is::ICELANDIC;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(ICELANDIC.code(), "is");
//! assert_eq!(ICELANDIC.name(), "Icelandic");
//! assert!(ICELANDIC.is_stopword("og"));
//! assert!(ICELANDIC.is_stopword("er"));
//! assert!(!ICELANDIC.is_stopword("fiskur"));
//!
//! let toks: Vec<&str> = ICELANDIC
//!     .tokenize("Hún hefur farið í búðina.")
//!     .collect();
//! assert_eq!(toks, ["Hún", "hefur", "farið", "í", "búðina"]);
//! ```
//!
//! # Module map
//!
//! - [`stemmer`] — the [`IcelandicStemmer`] rule-based stemmer.
//! - [`phonetic`] — [`IcelandicPhonex`] plus the
//!   [`IcelandicPhonexAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`IcelandicTokenizer`] wrapper.
//! - The [`Icelandic`] type and the [`ICELANDIC`] constant live in
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

#[cfg(feature = "alloc")]
pub mod phonetic;
#[cfg(feature = "alloc")]
pub mod stemmer;
pub mod stopwords;
pub mod tokenizer;

#[cfg(all(test, feature = "std", not(target_family = "wasm")))]
mod properties;

#[cfg(feature = "alloc")]
pub use phonetic::{IcelandicPhonex, IcelandicPhonexAdapter};
#[cfg(feature = "alloc")]
pub use stemmer::IcelandicStemmer;
pub use stopwords::STOPWORDS;
pub use tokenizer::IcelandicTokenizer;

// -----------------------------------------------------------------------
// The Icelandic language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::IcelandicPhonexAdapter;
    use crate::stemmer::IcelandicStemmer;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::IcelandicTokenizer;

    /// The Icelandic language pack.
    ///
    /// Zero-sized; construct as [`Icelandic`] and reuse the value
    /// freely across threads and calls, or grab the crate-level
    /// [`ICELANDIC`](crate::ICELANDIC) constant.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Icelandic;

    /// The static [`IcelandicPhonexAdapter`] [`Icelandic`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder).
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static PHONEX: IcelandicPhonexAdapter = IcelandicPhonexAdapter;

    impl Language for Icelandic {
        fn code(&self) -> &'static str {
            "is"
        }

        fn name(&self) -> &'static str {
            "Icelandic"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            IcelandicStemmer.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(IcelandicTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            Some(&PHONEX)
        }
    }

    /// The singleton [`Icelandic`] language pack.
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Icelandic`] every time — the type is zero-sized, so the two
    /// forms are equivalent, but the constant is the intended entry
    /// point and matches the pattern every other
    /// `stringcheese-<lang>` pack follows.
    pub const ICELANDIC: Icelandic = Icelandic;
}

#[cfg(feature = "alloc")]
pub use pack::{ICELANDIC, Icelandic};

// Opt this pack into the shared `stringcheese_lang::registry` — a
// distributed slice populated at link time so callers picking a
// language by BCP-47 code at runtime
// (`stringcheese_lang::registry::language("is")`) find Icelandic
// without naming the crate. See `stringcheese_lang::registry` for the
// design and trade-offs.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(ICELANDIC);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-is` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
