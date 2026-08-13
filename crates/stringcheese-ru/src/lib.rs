//! Russian language pack for the StringCheese toolkit.
//!
//! A zero-sized [`Russian`] value that carries the Russian stopword
//! list, the [`RussianSnowball`] stemmer, the whitespace-and-
//! punctuation [`RussianTokenizer`], and a [`RussianGost779B`]
//! transliteration phonetic hookup. Callers grab the singleton
//! [`RUSSIAN`] `const` — no construction ceremony required — and
//! delegate through the [`Language`](stringcheese_lang::Language)
//! trait.
//!
//! # Design commitment
//!
//! **This crate is opt-in.** The umbrella `stringcheese` facade does
//! *not* re-export `stringcheese-ru` — language packs are per-crate,
//! per-language dependencies, so a caller who only wants English or
//! only wants Levenshtein doesn't pay for the Russian stopword table
//! or the Snowball Russian stemmer's code. Callers who need Russian
//! add `stringcheese-ru = "0.1"` to their `Cargo.toml` explicitly.
//!
//! # First Cyrillic-script pack
//!
//! This is the first `stringcheese-<lang>` implementation for a
//! script written in Cyrillic. It exists to validate the shape of the
//! [`Language`](stringcheese_lang::Language) trait on the Cyrillic
//! alphabet, and to prove that StringCheese's byte/character-sequence
//! processing model handles Cyrillic-script inputs without any
//! special-case machinery.
//!
//! ## The Cyrillic-specific invariants
//!
//! Cyrillic is a **left-to-right script**. Unlike Arabic (the first
//! RTL pack), there are no display-order surprises; a Rust source
//! file containing `"Москва"` reads M-o-s-k-v-a in both logical and
//! display order. The one thing to remember is *storage width*:
//!
//! * **Every letter is 2 bytes in UTF-8.** The modern Russian
//!   alphabet plus `Ё` / `ё` all live in the range U+0400..=U+045F,
//!   which falls in UTF-8's 2-byte range (U+0080..=U+07FF). A word
//!   like `"стол"` (4 characters) is 8 bytes. Any code that mixes
//!   byte offsets with character-boundary logic will silently
//!   corrupt token or suffix boundaries. This crate operates
//!   exclusively on `Vec<char>` and [`str::chars`] iteration — never
//!   raw byte offsets. Callers of
//!   [`Language::stem`](stringcheese_lang::Language::stem) and
//!   [`Language::tokenize`](stringcheese_lang::Language::tokenize) never
//!   see the char-vs-byte distinction because the returned
//!   [`Cow<str>`](alloc::borrow::Cow) and token slices are always
//!   valid UTF-8; downstream callers that do byte-level arithmetic on
//!   the outputs must remember the 2x expansion factor.
//! * **No Turkic-fold concerns.** Cyrillic case-folding is
//!   well-behaved under Rust's default [`char::to_lowercase`]:
//!   `А → а`, `Ё → ё`, `Я → я`. There is no locale tailoring the way
//!   Turkish requires for the dotted / dotless-I distinction.
//! * **`ё` / `е` alternation is real.** Russian orthography
//!   historically drops the diaeresis on `ё` (rendering both `ёж`
//!   and `еж` the same word). The Snowball spec explicitly
//!   precomputes `ё → е` as a preprocessing step, and this crate
//!   follows suit — the stemmer folds `ё → е` before the region
//!   calculations, and the `is_stopword` override folds too so a
//!   query for `"ЁЛКА"` matches the plain-`е` stopword list.
//! * **UTF-8 code-point processing order is left-to-right.** No RTL
//!   concerns; the tokenizer emits tokens in reading order.
//!
//! The design choices are otherwise:
//!
//! * **Snowball Russian stemmer.** Martin Porter's Russian algorithm,
//!   documented at
//!   <https://snowballstem.org/algorithms/russian/stemmer.html>. The
//!   reference for Russian IR stemmers; Lucene's `RussianAnalyzer`
//!   and Elasticsearch's `russian` analyzer both descend from it.
//!   Russian is a fusional Indo-European language with rich
//!   inflectional morphology — case, number, gender, verb aspect,
//!   and tense — so the algorithm strips suffixes in four cascading
//!   steps (perfective gerund → reflexive → adjectival/verb/noun →
//!   derivational → undouble-nn/superlative/soft-sign). See
//!   [`snowball`] for the algorithm's rules.
//! * **~170-word stopword list.** The union of NLTK's `russian` list
//!   and the Snowball project's Russian stopword collection. Covers
//!   pronouns, demonstratives, interrogatives, conjunctions,
//!   prepositions, particles, high-frequency forms of *быть* /
//!   *мочь*, and quantifiers. See [`stopwords`].
//! * **GOST 7.79-B transliteration phonetic encoder.** A
//!   deterministic, ASCII-only Cyrillic → Latin mapping. Not a
//!   Soundex-family sound-alike encoder, but a stable equivalence
//!   class the phonetic subsystem accepts and downstream indexes
//!   can consume. Adapter name: `"gost-7.79-b"`. See [`phonetic`].
//!   The pack also opts into the cross-Slavic
//!   [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
//!   encoder from `stringcheese-phonetic` behind the
//!   `slavic-metaphone` Cargo feature — use
//!   [`RUSSIAN_WITH_SLAVIC_METAPHONE`] to get a pack whose
//!   [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
//!   returns the shared Slavic-family sound-alike key instead of the
//!   default transliteration. The task considered a PHONEX-Slavic
//!   sibling; that is deferred as a follow-up (see below).
//! * **Simple tokenizer.** Russian orthography uses ASCII spaces
//!   between orthographic words, and every letter of the modern
//!   Russian alphabet satisfies [`char::is_alphanumeric`], so the
//!   default splitter handles Russian word segmentation correctly
//!   out of the box. [`RussianTokenizer`] is a transparent wrapper
//!   around [`SimpleTokenizer`](stringcheese_lang::SimpleTokenizer).
//! * **Cyrillic-aware case fold.** Uses Rust's default
//!   [`char::to_lowercase`] — no Turkic-style tailoring. The
//!   `is_stopword` override folds under this rule (plus a
//!   `ё → е` fold) so uppercase Cyrillic queries match the plain
//!   stopword list.
//! * **Default Unicode collation.** Russian sorts under CLDR's
//!   Russian tailoring (mostly identical to code-point order in the
//!   Cyrillic block, so the default works). This pack does not
//!   carry the CLDR tailoring data;
//!   [`Language::collator`](stringcheese_lang::Language::collator)
//!   returns `None`. Callers who need locale-aware Russian
//!   collation should reach for `icu_collator`.
//!
//! # Deferred to a follow-up wave
//!
//! * **Ukrainian, Belarusian, Serbian, Bulgarian, Macedonian.** Each
//!   deserves its own `stringcheese-uk` / `-be` / `-sr` / `-bg` /
//!   `-mk` pack — different function-word inventories, different
//!   morphology, different subset of the extended Cyrillic block
//!   (Ukrainian `і ї є`, Belarusian `ў`, Serbian `љ њ ђ ћ џ`,
//!   Macedonian `ѓ ќ ѕ`).
//! * **PHONEX-Slavic phonetic encoder.** The shipped transliteration
//!   is a *transliteration* (deterministic character-level mapping),
//!   not a sound-alike encoder. The `slavic-metaphone` feature adds
//!   the cross-Slavic
//!   [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
//!   Metaphone-family encoder as an alternate; a Russian-specific
//!   PHONEX or Kolmogorov-Fedorov-shaped encoder is a further
//!   follow-up.
//! * **ISO 9 System A transliteration alongside GOST 7.79-B.** Would
//!   want it under a separate adapter for library-catalog interop.
//! * **Old orthography.** No `ѣ`, `і`, `ѳ`, `ѵ` handling — those
//!   scalars are passed through unchanged by the stemmer and
//!   tokenizer.
//! * **Full-vocabulary Snowball cross-verification.** The shipped
//!   reference-pair test embeds a hand-traced subset; the full
//!   `voc.txt` / `output.txt` cross-check is a follow-up.
//!
//! # Quick-start
//!
//! ```
//! use stringcheese_ru::RUSSIAN;
//! use stringcheese_lang::Language;
//!
//! assert_eq!(RUSSIAN.code(), "ru");
//! assert_eq!(RUSSIAN.name(), "Russian");
//! assert!(RUSSIAN.is_stopword("и"));
//! assert!(RUSSIAN.is_stopword("НЕ"));   // Cyrillic case-fold: НЕ → не.
//! assert!(RUSSIAN.is_stopword("ёж") == false); // ёж is not a stopword.
//! assert!(!RUSSIAN.is_stopword("собака"));
//! assert_eq!(RUSSIAN.stem("красивая"), "красив");
//! assert_eq!(RUSSIAN.stem("полезность"), "полезн");
//!
//! let toks: Vec<&str> = RUSSIAN
//!     .tokenize("Привет, мир! Москва — столица.")
//!     .collect();
//! assert_eq!(toks, ["Привет", "мир", "Москва", "столица"]);
//! ```
//!
//! # Module map
//!
//! - [`snowball`] — the [`RussianSnowball`] stemmer.
//! - [`phonetic`] — [`RussianGost779B`] plus the
//!   [`RussianGost779BAdapter`] the
//!   [`Language`](stringcheese_lang::Language) trait hands back.
//! - [`stopwords`] — the [`STOPWORDS`] list.
//! - [`tokenizer`] — the [`RussianTokenizer`] wrapper.
//! - The [`Russian`] type and the [`RUSSIAN`] constant live in this
//!   crate's root.

#![cfg_attr(not(feature = "std"), no_std)]
// `deny` rather than `forbid` because the `stringcheese_lang::
// register_language!` invocation below expands to a `linkme`-backed
// static whose implementation is `unsafe`-tagged (safe in practice
// — that's linkme's whole design — but flagged by the
// `unsafe_code` lint). The macro emits an explicit
// `#[allow(unsafe_code)]` at the sole registration site; the rest
// of this crate is still lint-enforced no-`unsafe`. Same pattern as
// the other language packs.
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "case-scud")]
pub mod case_data;
#[cfg(feature = "collation-scud")]
pub mod collation_data;
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

#[cfg(all(feature = "alloc", feature = "slavic-metaphone"))]
pub use phonetic::SlavicMetaphoneAdapter;
#[cfg(feature = "alloc")]
pub use phonetic::{RussianGost779B, RussianGost779BAdapter};
#[cfg(feature = "alloc")]
pub use snowball::RussianSnowball;
pub use stopwords::STOPWORDS;
pub use tokenizer::RussianTokenizer;

// -----------------------------------------------------------------------
// The Russian language pack.
// -----------------------------------------------------------------------

#[cfg(feature = "alloc")]
mod pack {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;

    use stringcheese_lang::{Language, LanguagePhoneticEncoder};

    use crate::phonetic::RussianGost779BAdapter;
    #[cfg(feature = "slavic-metaphone")]
    use crate::phonetic::SlavicMetaphoneAdapter;
    use crate::snowball::RussianSnowball;
    use crate::stopwords::STOPWORDS;
    use crate::tokenizer::RussianTokenizer;

    /// Which phonetic encoder this [`Russian`] instance uses.
    ///
    /// Modeled as an `enum` (rather than a `&'static dyn ...` reference)
    /// so the pack stays `Copy`, its constant constructors stay
    /// `const`-usable, and the shipped encoders stay a closed set. The
    /// [`SlavicMetaphone`](Self::SlavicMetaphone) variant is only
    /// available when the crate's `slavic-metaphone` Cargo feature is
    /// on; a build without that feature carries only the
    /// [`Gost779B`](Self::Gost779B) variant.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub enum RussianPhoneticChoice {
        /// The default GOST 7.79 System B Cyrillic → Latin
        /// transliteration adapter
        /// ([`RussianGost779BAdapter`](crate::phonetic::RussianGost779BAdapter)).
        #[default]
        Gost779B,
        /// The cross-Slavic
        /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
        /// Metaphone-family sound-alike encoder from
        /// `stringcheese-phonetic`, wrapped as
        /// [`SlavicMetaphoneAdapter`](crate::phonetic::SlavicMetaphoneAdapter).
        /// Only available when the crate's `slavic-metaphone` Cargo
        /// feature is on.
        #[cfg(feature = "slavic-metaphone")]
        SlavicMetaphone,
    }

    /// The Russian language pack.
    ///
    /// Carries a phonetic-encoder choice — the default (used by the
    /// [`RUSSIAN`](crate::RUSSIAN) constant) is the GOST 7.79-B
    /// transliteration; callers who want the cross-Slavic
    /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
    /// sound-alike encoder grab
    /// [`RUSSIAN_WITH_SLAVIC_METAPHONE`](crate::RUSSIAN_WITH_SLAVIC_METAPHONE)
    /// (behind the `slavic-metaphone` feature) or compose their own
    /// via [`with_slavic_metaphone_encoder`](Russian::with_slavic_metaphone_encoder).
    ///
    /// Two `Russian` values with different encoder choices are
    /// otherwise identical — same stopwords, same stemmer, same
    /// tokenizer, same code/name. The distinguishing piece is cheap
    /// (a small enum), so `Russian` remains cheap to copy and cheap to
    /// construct.
    ///
    /// See the [crate-level docs](crate) for the implementation
    /// choices and the roadmap.
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Russian {
        phonetic_choice: RussianPhoneticChoice,
    }

    impl Russian {
        /// Construct a `Russian` pack with the default GOST 7.79-B
        /// transliteration phonetic encoder.
        #[must_use]
        pub const fn new() -> Self {
            Self {
                phonetic_choice: RussianPhoneticChoice::Gost779B,
            }
        }

        /// Return a `Russian` pack whose
        /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
        /// hands back the default GOST 7.79-B transliteration adapter
        /// ([`RussianGost779BAdapter`](crate::phonetic::RussianGost779BAdapter)).
        ///
        /// This is the default; the method exists so a caller who
        /// composed a
        /// [`with_slavic_metaphone_encoder`](Self::with_slavic_metaphone_encoder)
        /// pack can undo that choice.
        #[must_use]
        pub const fn with_default_encoder(mut self) -> Self {
            self.phonetic_choice = RussianPhoneticChoice::Gost779B;
            self
        }

        /// Return a `Russian` pack whose
        /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
        /// hands back the cross-Slavic
        /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
        /// Metaphone-family sound-alike encoder from
        /// `stringcheese-phonetic`, wrapped as
        /// [`SlavicMetaphoneAdapter`](crate::phonetic::SlavicMetaphoneAdapter).
        ///
        /// Reach for the [`RUSSIAN_WITH_SLAVIC_METAPHONE`](crate::RUSSIAN_WITH_SLAVIC_METAPHONE)
        /// constant if you want the default pack with the
        /// Slavic-Metaphone encoder; this builder method lets a caller
        /// compose the encoder choice with future non-default pack
        /// pieces.
        ///
        /// Only available when the crate's `slavic-metaphone` Cargo
        /// feature is enabled.
        #[cfg(feature = "slavic-metaphone")]
        #[must_use]
        pub const fn with_slavic_metaphone_encoder(mut self) -> Self {
            self.phonetic_choice = RussianPhoneticChoice::SlavicMetaphone;
            self
        }
    }

    /// The static [`RussianGost779BAdapter`] [`Russian`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder) when the
    /// pack was built with the default choice.
    ///
    /// Kept as a `static` so
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// can return a reference with the required `'static`-friendly
    /// lifetime through a trait object.
    static GOST_779_B: RussianGost779BAdapter = RussianGost779BAdapter;

    /// The static [`SlavicMetaphoneAdapter`] [`Russian`] hands back
    /// from [`phonetic_encoder`](Language::phonetic_encoder) when the
    /// pack was built with
    /// [`RussianPhoneticChoice::SlavicMetaphone`].
    ///
    /// Kept as a `static` for the same reason as `GOST_779_B`.
    #[cfg(feature = "slavic-metaphone")]
    static SLAVIC_METAPHONE: SlavicMetaphoneAdapter = SlavicMetaphoneAdapter;

    /// Normalize a Cyrillic string for stopword comparison: lowercase
    /// under default Unicode rules and fold `ё → е`.
    ///
    /// The pack stores stopwords in their plain-`е` lowercase form; a
    /// query like `"ЁЛКА"` needs to fold to `"елка"` before the
    /// scan can match.
    fn normalize_for_stopword(word: &str) -> String {
        let mut out = String::with_capacity(word.len());
        for c in word.chars() {
            for lc in c.to_lowercase() {
                if lc == 'ё' {
                    out.push('е');
                } else {
                    out.push(lc);
                }
            }
        }
        out
    }

    impl Language for Russian {
        fn code(&self) -> &'static str {
            "ru"
        }

        fn name(&self) -> &'static str {
            "Russian"
        }

        fn stopwords(&self) -> &'static [&'static str] {
            STOPWORDS
        }

        /// Cyrillic-aware stopword membership.
        ///
        /// Overrides the default trait implementation (which uses
        /// [`str::eq_ignore_ascii_case`], missing every uppercase
        /// Cyrillic input) with a Unicode lowercase pass plus the
        /// `ё → е` fold — so `ЁЛКА` and `елка` both find the plain
        /// `елка` in the stopword list (were `елка` a stopword).
        fn is_stopword(&self, word: &str) -> bool {
            let normalized = normalize_for_stopword(word);
            STOPWORDS.contains(&normalized.as_str())
        }

        fn stem<'s>(&self, word: &'s str) -> Cow<'s, str> {
            RussianSnowball.stem(word)
        }

        fn tokenize<'a>(&self, text: &'a str) -> Box<dyn Iterator<Item = &'a str> + 'a> {
            Box::new(RussianTokenizer::new().tokenize(text))
        }

        fn phonetic_encoder(&self) -> Option<&dyn LanguagePhoneticEncoder> {
            match self.phonetic_choice {
                RussianPhoneticChoice::Gost779B => Some(&GOST_779_B),
                #[cfg(feature = "slavic-metaphone")]
                RussianPhoneticChoice::SlavicMetaphone => Some(&SLAVIC_METAPHONE),
            }
        }
    }

    /// The singleton [`Russian`] language pack (default GOST 7.79-B
    /// transliteration phonetic encoder).
    ///
    /// Callers reach for this constant rather than constructing
    /// [`Russian`] every time — the two forms are equivalent, but the
    /// constant is the intended entry point and matches the pattern
    /// every other `stringcheese-<lang>` pack follows.
    ///
    /// See [`RUSSIAN_WITH_SLAVIC_METAPHONE`](crate::RUSSIAN_WITH_SLAVIC_METAPHONE)
    /// for the same pack backed by the cross-Slavic
    /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
    /// encoder.
    pub const RUSSIAN: Russian = Russian::new();

    /// The singleton [`Russian`] language pack whose
    /// [`Language::phonetic_encoder`](stringcheese_lang::Language::phonetic_encoder)
    /// hands back the cross-Slavic
    /// [`SlavicMetaphone`](stringcheese_phonetic::SlavicMetaphone)
    /// Metaphone-family sound-alike encoder instead of the default
    /// GOST 7.79-B transliteration.
    ///
    /// Identical to [`RUSSIAN`](crate::RUSSIAN) in every respect
    /// except its phonetic encoder. Only available when the crate's
    /// `slavic-metaphone` Cargo feature is enabled — see the
    /// [`slavic_metaphone`](mod@stringcheese_phonetic::slavic_metaphone)
    /// module docs for the design trade-offs.
    #[cfg(feature = "slavic-metaphone")]
    pub const RUSSIAN_WITH_SLAVIC_METAPHONE: Russian =
        Russian::new().with_slavic_metaphone_encoder();
}

#[cfg(all(feature = "alloc", feature = "slavic-metaphone"))]
pub use pack::RUSSIAN_WITH_SLAVIC_METAPHONE;
#[cfg(feature = "alloc")]
pub use pack::{RUSSIAN, Russian, RussianPhoneticChoice};

// Register into `stringcheese-lang::registry` so callers who look up
// languages dynamically (`registry::language("ru")`) find this pack.
// alloc-gated because the pack constant itself is alloc-gated.
#[cfg(feature = "alloc")]
stringcheese_lang::register_language!(RUSSIAN);

/// Metadata about this release.
pub mod meta {
    /// The `stringcheese-ru` crate's semantic version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
}
